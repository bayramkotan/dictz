//! Search-box suggestions and "did you mean" without blocking the window.
//!
//! [`SuggestWorker`] runs on its own thread with its own read-only
//! connection to the index. It answers "suggest" (prefix hits filled up
//! with fuzzy matches) and "fuzzy" requests, and builds the fuzzy candidate
//! list ("warm") — every headword of the enabled dictionaries, in list
//! order, duplicates removed — read from the index, once per set of
//! dictionaries (each dictionary's own list is kept, so switching one on or
//! off does not read the others again). Only the newest request of each
//! kind is answered; older ones are dropped.
//!
//! Mirrors `dictz/core/suggest.py`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use rusqlite::{Connection, OpenFlags};

use crate::fuzzy::{self, Prepared};
use crate::index::search_headword_rows;
use crate::query::Suggestion;

/// The enabled dictionaries, in list order: name and a stamp that changes
/// when the dictionary is opened again (`QueryManager::enabled_snapshot`).
pub type Snapshot = Vec<(String, u64)>;

/// Prefix hits ranked closest-first, filled up with fuzzy matches when there
/// are fewer than `fuzzy_fill_below` (`QueryManager::suggest_with`).
pub fn merge_suggestions(
    prefix: &str,
    prefix_words: Vec<String>,
    fuzzy: impl FnOnce() -> Vec<(String, f64)>,
    limit: usize,
    fuzzy_fill_below: usize,
) -> Vec<Suggestion> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut words: Vec<String> = Vec::new();
    for w in prefix_words {
        if seen.insert(w.clone()) {
            words.push(w);
        }
    }
    fuzzy::rank(prefix, &mut words);
    let mut out: Vec<Suggestion> = words
        .into_iter()
        .map(|word| Suggestion { word, fuzzy: false })
        .collect();
    if out.len() < fuzzy_fill_below {
        for (w, _) in fuzzy() {
            if out.len() >= limit {
                break;
            }
            if seen.insert(w.clone()) {
                out.push(Suggestion {
                    word: w,
                    fuzzy: true,
                });
            }
        }
    }
    out
}

/// Settings of one "suggest" or "fuzzy" request.
#[derive(Debug, Clone, Copy)]
pub struct Params {
    /// Suggestions shown (`search.suggestion_limit`); for "fuzzy" the number
    /// of matches.
    pub limit: usize,
    /// Fill up with fuzzy matches below this many prefix hits.
    pub fuzzy_fill_below: usize,
    /// Fuzzy matches at most.
    pub fuzzy_limit: usize,
    /// Minimum fuzzy score.
    pub fuzzy_cutoff: f64,
}

/// What the worker sends back.
#[derive(Debug, Clone)]
pub enum Answer {
    /// The word list for `query`.
    Suggest {
        /// The search box text it was asked for.
        query: String,
        /// Suggestions.
        list: Vec<Suggestion>,
    },
    /// "Did you mean" for `query`.
    Fuzzy {
        /// The word.
        query: String,
        /// Matches, best first.
        list: Vec<(String, f64)>,
    },
}

type Job = (String, Snapshot, Params);

#[derive(Default)]
struct Jobs {
    suggest: Option<Job>,
    fuzzy: Option<Job>,
    warm: Option<Snapshot>,
    stop: bool,
}

type Shared = Arc<Mutex<Option<(Snapshot, Arc<Prepared>)>>>;

/// The suggestion thread.
pub struct SuggestWorker {
    jobs: Arc<(Mutex<Jobs>, Condvar)>,
    rx: Receiver<Answer>,
    shared: Shared,
    handle: Option<JoinHandle<()>>,
}

impl SuggestWorker {
    /// Start the thread on the index at `db`; `wake` is called after each
    /// answer (the window asks for a repaint).
    pub fn start(db: PathBuf, wake: Box<dyn Fn() + Send>) -> Self {
        let jobs: Arc<(Mutex<Jobs>, Condvar)> = Arc::default();
        let shared: Shared = Arc::default();
        let (tx, rx) = channel();
        let (j, s) = (jobs.clone(), shared.clone());
        let handle = std::thread::Builder::new()
            .name("dictz-suggest".into())
            .spawn(move || Thread::new(db, s, tx, wake).run(j))
            .ok();
        Self {
            jobs,
            rx,
            shared,
            handle,
        }
    }

    fn with_jobs(&self, f: impl FnOnce(&mut Jobs)) {
        let (m, cv) = &*self.jobs;
        if let Ok(mut j) = m.lock() {
            f(&mut j);
            cv.notify_one();
        }
    }

    /// Ask for the word list of `query`.
    pub fn suggest(&self, query: &str, dicts: Snapshot, p: Params) {
        let job = (query.to_string(), dicts, p);
        self.with_jobs(|j| j.suggest = Some(job));
    }

    /// Ask for "did you mean" for `word` (`p.limit` matches, `p.fuzzy_cutoff`).
    pub fn fuzzy(&self, word: &str, dicts: Snapshot, p: Params) {
        let job = (word.to_string(), dicts, p);
        self.with_jobs(|j| j.fuzzy = Some(job));
    }

    /// Build the candidate list for `dicts` now, before anyone needs it.
    pub fn warm(&self, dicts: Snapshot) {
        self.with_jobs(|j| j.warm = Some(dicts));
    }

    /// The candidate list for `dicts` when it is built (never waits).
    pub fn ready(&self, dicts: &Snapshot) -> Option<Arc<Prepared>> {
        let g = self.shared.try_lock().ok()?;
        match g.as_ref() {
            Some((k, p)) if k == dicts => Some(p.clone()),
            _ => None,
        }
    }

    /// The next answer, if one came.
    pub fn try_recv(&self) -> Option<Answer> {
        self.rx.try_recv().ok()
    }

    /// Stop the thread (it finishes the request it is working on).
    pub fn stop(&mut self) {
        self.with_jobs(|j| {
            j.stop = true;
            j.suggest = None;
            j.fuzzy = None;
            j.warm = None;
        });
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for SuggestWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Thread {
    db: PathBuf,
    con: Option<Connection>,
    per: HashMap<(String, u64), Arc<Vec<String>>>,
    shared: Shared,
    tx: Sender<Answer>,
    wake: Box<dyn Fn() + Send>,
}

enum Work {
    Suggest(Job),
    Fuzzy(Job),
    Warm(Snapshot),
}

impl Thread {
    fn new(db: PathBuf, shared: Shared, tx: Sender<Answer>, wake: Box<dyn Fn() + Send>) -> Self {
        Self {
            db,
            con: None,
            per: HashMap::new(),
            shared,
            tx,
            wake,
        }
    }

    fn run(mut self, jobs: Arc<(Mutex<Jobs>, Condvar)>) {
        let (m, cv) = &*jobs;
        loop {
            let work = {
                let Ok(mut j) = m.lock() else { return };
                loop {
                    if j.stop {
                        return;
                    }
                    // suggestions first: they are what the user is waiting for
                    if let Some(x) = j.suggest.take() {
                        break Work::Suggest(x);
                    }
                    if let Some(x) = j.fuzzy.take() {
                        break Work::Fuzzy(x);
                    }
                    if let Some(x) = j.warm.take() {
                        break Work::Warm(x);
                    }
                    j = match cv.wait(j) {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                }
            };
            let answer = match work {
                Work::Warm(d) => {
                    self.candidates(&d);
                    None
                }
                Work::Fuzzy((q, d, p)) => {
                    let list = self.fuzzy(&q, &d, p.limit, p.fuzzy_cutoff);
                    let stale = m.lock().map(|j| j.fuzzy.is_some()).unwrap_or(true);
                    (!stale).then_some(Answer::Fuzzy { query: q, list })
                }
                Work::Suggest((q, d, p)) => {
                    let list = self.suggest(&q, &d, p);
                    let stale = m.lock().map(|j| j.suggest.is_some()).unwrap_or(true);
                    (!stale).then_some(Answer::Suggest { query: q, list })
                }
            };
            if let Some(a) = answer {
                if self.tx.send(a).is_err() {
                    return;
                }
                (self.wake)();
            }
        }
    }

    fn con(&mut self) -> Option<&Connection> {
        if self.con.is_none() && self.db.is_file() {
            self.con = Connection::open_with_flags(
                &self.db,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .ok();
        }
        self.con.as_ref()
    }

    /// The candidate list for `dicts`, built from the index when needed.
    fn candidates(&mut self, dicts: &Snapshot) -> Arc<Prepared> {
        if let Ok(g) = self.shared.lock() {
            if let Some((k, p)) = g.as_ref() {
                if k == dicts {
                    return p.clone();
                }
            }
        }
        let wanted: HashSet<&(String, u64)> = dicts.iter().collect();
        self.per.retain(|k, _| wanted.contains(k));
        let missing: HashMap<String, u64> = dicts
            .iter()
            .filter(|k| !self.per.contains_key(*k))
            .cloned()
            .collect();
        if !missing.is_empty() {
            let mut lists: HashMap<String, Vec<String>> = HashMap::new();
            if let Some(con) = self.con() {
                // one pass over the index: its rows are each dictionary's
                // headwords in sorted order (as `words().sorted()`)
                if let Ok(mut st) =
                    con.prepare("SELECT headword, dictionary FROM entries ORDER BY rowid")
                {
                    if let Ok(rows) =
                        st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                    {
                        for (w, d) in rows.flatten() {
                            if missing.contains_key(&d) {
                                lists.entry(d).or_default().push(w);
                            }
                        }
                    }
                }
            }
            for (name, stamp) in missing {
                let l = lists.remove(&name).unwrap_or_default();
                self.per.insert((name, stamp), Arc::new(l));
            }
        }
        let mut seen: HashSet<&str> = HashSet::new();
        let mut words: Vec<String> = Vec::new();
        for k in dicts {
            if let Some(l) = self.per.get(k) {
                for w in l.iter() {
                    if seen.insert(w.as_str()) {
                        words.push(w.clone());
                    }
                }
            }
        }
        let p = Arc::new(Prepared::new(words));
        if let Ok(mut g) = self.shared.lock() {
            *g = Some((dicts.clone(), p.clone()));
        }
        p
    }

    fn fuzzy(
        &mut self,
        word: &str,
        dicts: &Snapshot,
        limit: usize,
        cutoff: f64,
    ) -> Vec<(String, f64)> {
        let word = word.trim();
        if word.is_empty() {
            return Vec::new();
        }
        let c = self.candidates(dicts);
        fuzzy::extract_prepared(word, &c, limit, cutoff)
    }

    fn suggest(&mut self, query: &str, dicts: &Snapshot, p: Params) -> Vec<Suggestion> {
        let prefix = query.trim().to_string();
        if prefix.is_empty() {
            return Vec::new();
        }
        let names: Vec<String> = dicts.iter().map(|(n, _)| n.clone()).collect();
        let words: Vec<String> = self
            .con()
            .and_then(|c| search_headword_rows(c, &prefix, p.limit, Some(&names)).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|(w, _)| w)
            .collect();
        let unique = words.iter().collect::<HashSet<_>>().len();
        let fz = if unique < p.fuzzy_fill_below {
            self.fuzzy(&prefix, dicts, p.fuzzy_limit, p.fuzzy_cutoff)
        } else {
            Vec::new()
        };
        merge_suggestions(&prefix, words, move || fz, p.limit, p.fuzzy_fill_below)
    }
}
