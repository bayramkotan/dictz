//! "Did you mean" matching.
//!
//! The score is RapidFuzz's `fuzz.ratio` (normalised InDel similarity × 100)
//! on lower-cased strings, so the Python edition (which calls RapidFuzz) and
//! this crate rank candidates identically.

/// Default minimum score for a fuzzy suggestion.
pub const DEFAULT_CUTOFF: f64 = 60.0;
/// Default number of fuzzy suggestions.
pub const DEFAULT_LIMIT: usize = 10;

fn lcs_len(a: &[char], b: &[char]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let mut prev = vec![0usize; short.len() + 1];
    let mut cur = vec![0usize; short.len() + 1];
    for &lc in long {
        for (j, &sc) in short.iter().enumerate() {
            cur[j + 1] = if lc == sc {
                prev[j] + 1
            } else {
                cur[j].max(prev[j + 1])
            };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[short.len()]
}

/// RapidFuzz `fuzz.ratio(a, b)` on already lower-cased character slices.
pub fn ratio_chars(a: &[char], b: &[char]) -> f64 {
    let lensum = a.len() + b.len();
    if lensum == 0 {
        return 100.0;
    }
    let dist = lensum - 2 * lcs_len(a, b);
    let norm_dist = dist as f64 / lensum as f64;
    100.0 * (1.0 - norm_dist)
}

/// Case-insensitive similarity of two strings, 0–100.
pub fn ratio(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    ratio_chars(&a, &b)
}

/// Best `limit` candidates scoring at least `cutoff`, best first; ties keep
/// candidate order (RapidFuzz `process.extract` semantics).
pub fn extract<'a, I>(query: &str, candidates: I, limit: usize, cutoff: f64) -> Vec<(String, f64)>
where
    I: IntoIterator<Item = &'a str>,
{
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let mut scored: Vec<(usize, &str, f64)> = Vec::new();
    for (i, cand) in candidates.into_iter().enumerate() {
        let c: Vec<char> = cand.to_lowercase().chars().collect();
        // A length check skips hopeless candidates without the LCS pass.
        let best_possible = {
            let (x, y) = (q.len(), c.len());
            let lensum = x + y;
            if lensum == 0 {
                100.0
            } else {
                100.0 * (2 * x.min(y)) as f64 / lensum as f64
            }
        };
        if best_possible + 1e-9 < cutoff {
            continue;
        }
        let s = ratio_chars(&q, &c);
        if s >= cutoff {
            scored.push((i, cand, s));
        }
    }
    scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap().then(a.0.cmp(&b.0)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, w, s)| (w.to_string(), s))
        .collect()
}

/// A fuzzy candidate list with its lower-cased copy and character counts,
/// made once and searched many times ([`extract_prepared`]).
#[derive(Debug, Default, Clone)]
pub struct Prepared {
    /// The headwords, in candidate order.
    pub words: Vec<String>,
    lowered: Vec<String>,
    lens: Vec<u32>,
}

impl Prepared {
    /// Prepare `words` (kept in this order).
    pub fn new(words: Vec<String>) -> Self {
        let lowered: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
        let lens = lowered.iter().map(|w| w.chars().count() as u32).collect();
        Self {
            words,
            lowered,
            lens,
        }
    }

    /// Number of candidates.
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// `true` when there are no candidates.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

/// [`extract`] on a [`Prepared`] list: the same results, without lower-casing
/// every candidate on every call; candidates whose length alone rules them
/// out are skipped before any work.
pub fn extract_prepared(
    query: &str,
    cands: &Prepared,
    limit: usize,
    cutoff: f64,
) -> Vec<(String, f64)> {
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let x = q.len();
    let mut buf: Vec<char> = Vec::new();
    let mut scored: Vec<(usize, f64)> = Vec::new();
    for (i, (lw, &n)) in cands.lowered.iter().zip(&cands.lens).enumerate() {
        let y = n as usize;
        let lensum = x + y;
        let best_possible = if lensum == 0 {
            100.0
        } else {
            100.0 * (2 * x.min(y)) as f64 / lensum as f64
        };
        if best_possible + 1e-9 < cutoff {
            continue;
        }
        buf.clear();
        buf.extend(lw.chars());
        let s = ratio_chars(&q, &buf);
        if s >= cutoff {
            scored.push((i, s));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    scored
        .into_iter()
        .take(limit)
        .map(|(i, s)| (cands.words[i].clone(), s))
        .collect()
}

/// Order prefix suggestions closest-first: score descending, then shorter,
/// then case-insensitive alphabetical, then exact string.
pub fn rank(query: &str, words: &mut [String]) {
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let mut keyed: Vec<(f64, usize, String, String)> = words
        .iter()
        .map(|w| {
            let lw = w.to_lowercase();
            let c: Vec<char> = lw.chars().collect();
            (ratio_chars(&q, &c), w.chars().count(), lw, w.clone())
        })
        .collect();
    keyed.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap()
            .then(a.1.cmp(&b.1))
            .then(a.2.cmp(&b.2))
            .then(a.3.cmp(&b.3))
    });
    for (slot, k) in words.iter_mut().zip(keyed) {
        *slot = k.3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_matches_rapidfuzz() {
        // values computed with rapidfuzz.fuzz.ratio
        assert_eq!(ratio("apple", "apple"), 100.0);
        assert_eq!(ratio("aple", "apple"), 88.88888888888889);
        assert_eq!(ratio("kitap", "kitab"), 80.0);
        assert_eq!(ratio("", ""), 100.0);
        assert_eq!(ratio("abc", ""), 0.0);
        assert_eq!(ratio("Apple", "APPLE"), 100.0);
    }

    #[test]
    fn extract_orders_and_cuts() {
        let words = ["apple", "apply", "maple", "zebra", "ample"];
        let r = extract("appel", words.iter().copied(), 3, 60.0);
        assert_eq!(r[0].0, "apple");
        assert!(r.iter().all(|(_, s)| *s >= 60.0));
        assert!(!r.iter().any(|(w, _)| w == "zebra"));
    }

    #[test]
    fn prepared_gives_the_same_as_extract() {
        let words: Vec<String> = ["Apple", "apply", "maple", "zebra", "ample", "Äpfel", "a"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let p = Prepared::new(words.clone());
        for q in ["appel", "APL", "äpf", "z", ""] {
            assert_eq!(
                extract_prepared(q, &p, 10, 40.0),
                extract(q, words.iter().map(String::as_str), 10, 40.0),
                "{q}"
            );
        }
    }

    #[test]
    fn rank_closest_first() {
        let mut v = vec!["carpet".to_string(), "car".to_string(), "cart".to_string()];
        rank("car", &mut v);
        assert_eq!(v, vec!["car", "cart", "carpet"]);
    }
}
