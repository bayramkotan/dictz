//! dictz command line (Rust edition) — the same commands, options and output
//! as the Python edition's `dictz/cli.py`.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};

use crate::history::HistoryStore;
use crate::index::{DictionaryRow, IndexStore};
use crate::paths::{user_data_dir, HISTORY_DB, INDEX_DB};
use crate::query::{scan_folder, QueryManager, RestoreOptions};
use crate::settings::{Kind, Preferences, SCHEMA, SECTIONS, SETTINGS_FILE};
use crate::{fuzzy, html, DictionaryEntry, VERSION};

/// (command, description) — same list and order as the Python edition.
pub const COMMANDS: &[(&str, &str)] = &[
    ("gui", "open the window (default)"),
    ("lookup", "look a word up and print the articles"),
    ("suggest", "headwords starting with a prefix (+ fuzzy fill)"),
    ("fuzzy", "\"did you mean\" candidates for a word"),
    ("random", "a random article"),
    ("dicts", "list dictionaries"),
    ("add", "add dictionary files or folders"),
    ("remove", "remove dictionaries from dictz (files are kept)"),
    ("enable", "enable dictionaries"),
    ("disable", "disable dictionaries"),
    ("move", "change a dictionary's position"),
    ("info", "details of a dictionary (name or file)"),
    ("reindex", "rebuild the search index"),
    ("scan", "find dictionary files in folders"),
    ("dump", "export a dictionary as tsv / json / txt"),
    ("history", "lookup history"),
    ("favorites", "favorite words"),
    ("config", "show and change settings"),
    ("paths", "where dictz keeps its files"),
    ("doctor", "check the installation"),
    ("selftest", "headless self-test (exit 0 = OK)"),
    ("update", "update this copy of dictz"),
    ("version", "print the version"),
    ("completion", "shell completion script"),
    ("shell", "interactive lookup prompt"),
    (
        "show",
        "bring the running window to the front (starts dictz if needed)",
    ),
    ("popup", "look a word up in the running dictz's scan popup"),
    ("watch", "turn clipboard / selection scanning on or off"),
    (
        "shortcut",
        "create desktop and menu shortcuts (--remove deletes them)",
    ),
    ("help", "show help"),
];

/// Command aliases.
pub const ALIASES: &[(&str, &str)] = &[
    ("l", "lookup"),
    ("s", "suggest"),
    ("ls", "dicts"),
    ("rm", "remove"),
    ("fav", "favorites"),
    ("upgrade", "update"),
    ("f", "fuzzy"),
];

const EDITION: &str = "Rust edition";

/// `true` when `args` ask for a CLI command (not just files for the GUI).
/// Every spelling of "print the version" people type.
pub const VERSION_FLAGS: &[&str] = &[
    "-v",
    "-V",
    "--version",
    "--Version",
    "--VERSION",
    "-version",
    "-Version",
    "-VERSION",
];

/// `dct --Version` / `dct -version` → `--version` (before clap sees them).
pub fn normalize_args(args: Vec<String>) -> Vec<String> {
    args.into_iter()
        .map(|a| {
            if VERSION_FLAGS.contains(&a.as_str()) {
                "--version".to_string()
            } else {
                a
            }
        })
        .collect()
}

/// `dct` when started as dct, else `dictz`.
pub fn program_name() -> &'static str {
    let name = std::env::args()
        .next()
        .map(|a| {
            std::path::Path::new(&a)
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if name == "dct" {
        "dct"
    } else {
        "dictz"
    }
}

/// `true` when the arguments ask for the command line rather than the window.
pub fn is_cli_invocation(args: &[String]) -> bool {
    match args.first() {
        None => false,
        Some(first) => {
            first.starts_with('-')
                || COMMANDS.iter().any(|(c, _)| c == first)
                || ALIASES.iter().any(|(a, _)| a == first)
        }
    }
}

#[derive(Parser)]
#[command(
    name = "dictz",
    disable_version_flag = true,
    about = "dictz — offline dictionary (StarDict, DSL, Babylon). Without a command the window opens.",
    after_help = "`dct` is the short name of `dictz`. Same commands in the Python edition (pip install dictz). `dictz <command> --help` for details."
)]
struct Cli {
    /// print the version and exit (also -V, --Version, -version)
    #[arg(short = 'v', short_alias = 'V', long = "version")]
    version: bool,
    #[arg(long, hide = true)]
    selftest: bool,
    /// use another data folder (also DICTZ_DATA_DIR)
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    /// no colors (also NO_COLOR)
    #[arg(long, global = true)]
    no_color: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// open the window (default)
    Gui {
        /// dictionary files to add first
        files: Vec<PathBuf>,
        /// open the scan popup with WORD (empty = clipboard)
        #[arg(long, value_name = "WORD")]
        popup: Option<String>,
    },
    /// look a word up and print the articles
    #[command(alias = "l")]
    Lookup(LookupArgs),
    /// headwords starting with a prefix (+ fuzzy fill)
    #[command(alias = "s")]
    Suggest {
        #[arg(required = true)]
        prefix: Vec<String>,
        #[arg(short = 'n', long, default_value_t = 30, value_name = "N")]
        limit: usize,
        /// prefix matches only
        #[arg(long)]
        no_fuzzy: bool,
        #[arg(long)]
        json: bool,
    },
    /// "did you mean" candidates for a word
    #[command(alias = "f")]
    Fuzzy {
        #[arg(required = true)]
        word: Vec<String>,
        #[arg(short = 'n', long, value_name = "N")]
        limit: Option<usize>,
        /// minimum similarity 0–100
        #[arg(short = 'c', long, value_name = "PCT")]
        cutoff: Option<f64>,
        #[arg(long)]
        json: bool,
    },
    /// a random article
    Random {
        #[arg(short = 'd', long = "dict", value_name = "NAME")]
        dict: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// list dictionaries
    #[command(alias = "ls")]
    Dicts {
        #[arg(long)]
        json: bool,
        /// names only, one per line
        #[arg(long)]
        names: bool,
    },
    /// add dictionary files or folders
    Add {
        /// dictionary files or folders
        #[arg(required = true, value_name = "PATH")]
        paths: Vec<PathBuf>,
        /// search folders recursively
        #[arg(short = 'r', long)]
        recursive: bool,
        /// rebuild the index even if known
        #[arg(long)]
        reindex: bool,
        /// add but keep disabled
        #[arg(long)]
        disabled: bool,
    },
    /// remove dictionaries from dictz (files are kept)
    #[command(alias = "rm")]
    Remove {
        #[arg(required = true, value_name = "NAME")]
        names: Vec<String>,
    },
    /// enable dictionaries
    Enable {
        #[arg(required = true, value_name = "NAME")]
        names: Vec<String>,
    },
    /// disable dictionaries
    Disable {
        #[arg(required = true, value_name = "NAME")]
        names: Vec<String>,
    },
    /// change a dictionary's position
    Move {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(value_name = "up|down|top|bottom|POSITION")]
        r#where: String,
    },
    /// details of a dictionary (name or file)
    Info {
        #[arg(required = true, value_name = "NAME|FILE")]
        target: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// rebuild the search index
    Reindex {
        /// default: all
        #[arg(value_name = "NAME")]
        names: Vec<String>,
    },
    /// find dictionary files in folders
    Scan {
        #[arg(required = true, value_name = "FOLDER")]
        folders: Vec<PathBuf>,
        #[arg(long)]
        no_recursive: bool,
        /// add the new ones
        #[arg(long)]
        add: bool,
    },
    /// export a dictionary as tsv / json / txt
    Dump {
        #[arg(value_name = "NAME|FILE")]
        target: String,
        #[arg(short = 'F', long, value_enum, default_value = "tsv")]
        format: DumpFormat,
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// lookup history
    History {
        #[arg(short = 'n', long, default_value_t = 50, value_name = "N")]
        limit: usize,
        #[arg(long)]
        json: bool,
        /// delete all history
        #[arg(long)]
        clear: bool,
        /// delete one word from the history
        #[arg(long, value_name = "WORD")]
        remove: Option<String>,
    },
    /// favorite words
    #[command(alias = "fav")]
    Favorites {
        #[arg(value_enum)]
        action: Option<FavAction>,
        #[arg(value_name = "WORD")]
        words: Vec<String>,
        #[arg(short = 'n', long, default_value_t = 500, value_name = "N")]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// show and change settings
    Config {
        #[arg(value_enum)]
        action: Option<ConfigAction>,
        #[arg(value_name = "KEY|FILE")]
        key: Option<String>,
        #[arg(value_name = "VALUE")]
        value: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// where dictz keeps its files
    Paths {
        #[arg(long)]
        json: bool,
    },
    /// check the installation
    Doctor,
    /// headless self-test (exit 0 = OK)
    Selftest,
    /// update this copy of dictz
    #[command(alias = "upgrade")]
    Update {
        /// only check, do not install
        #[arg(long)]
        check: bool,
        /// do not ask
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// print the version
    Version {
        #[arg(long)]
        json: bool,
    },
    /// shell completion script
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// interactive lookup prompt
    Shell,
    /// bring the running window to the front (starts dictz if needed)
    Show,
    /// look a word up in the running dictz's scan popup
    Popup {
        /// default: the clipboard text
        word: Vec<String>,
    },
    /// turn clipboard / selection scanning on or off
    Watch {
        #[arg(value_enum, default_value = "status")]
        state: WatchState,
    },
    /// create desktop and menu shortcuts (--remove deletes them)
    Shortcut {
        /// delete the shortcuts
        #[arg(long)]
        remove: bool,
        /// only the desktop shortcut
        #[arg(long)]
        desktop_only: bool,
        /// only the menu / Start menu entry
        #[arg(long)]
        menu_only: bool,
    },
}

#[derive(Args)]
struct LookupArgs {
    #[arg(required = true)]
    word: Vec<String>,
    /// only this dictionary (repeatable)
    #[arg(short = 'd', long = "dict", value_name = "NAME")]
    dict: Vec<String>,
    /// all headwords starting with WORD
    #[arg(short = 'p', long)]
    prefix: bool,
    /// when nothing matches, show the closest headword's article
    #[arg(short = 'f', long)]
    fuzzy: bool,
    /// no did-you-mean list
    #[arg(long)]
    no_fuzzy: bool,
    /// at most N articles
    #[arg(short = 'n', long, value_name = "N")]
    limit: Option<usize>,
    /// JSON output
    #[arg(long)]
    json: bool,
    /// print the raw HTML definitions
    #[arg(long)]
    html: bool,
    /// do not record in the history
    #[arg(long)]
    no_history: bool,
    /// counts and timing on stderr
    #[arg(long)]
    verbose: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum DumpFormat {
    Tsv,
    Json,
    Txt,
}

#[derive(Clone, Copy, ValueEnum, PartialEq)]
enum FavAction {
    List,
    Add,
    Remove,
    Clear,
}

#[derive(Clone, Copy, ValueEnum, PartialEq)]
enum ConfigAction {
    List,
    Get,
    Set,
    Reset,
    Path,
    Export,
    Import,
}

#[derive(Clone, Copy, ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

#[derive(Clone, Copy, ValueEnum)]
enum WatchState {
    On,
    Off,
    Toggle,
    Status,
}

/// What `main` should do after the CLI ran.
pub enum Outcome {
    /// Exit with this code.
    Exit(i32),
    /// Open the window, adding these files first; `popup` = open the scan
    /// popup with this word (empty = clipboard).
    Gui {
        /// Dictionary files.
        files: Vec<PathBuf>,
        /// Scan popup word.
        popup: Option<String>,
    },
}

fn start_detached(extra: &[&str]) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("gui").args(extra);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000008 | 0x00000200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    match cmd.spawn() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("error: cannot start dictz: {}", e);
            false
        }
    }
}

fn remote(ctx: &Ctx, msg: Value) -> Option<Value> {
    crate::instance::send(&ctx.data_dir, &msg, std::time::Duration::from_secs(2))
}

fn cmd_show(ctx: &Ctx) -> i32 {
    if remote(ctx, json!({"cmd": "show"}))
        .map(|a| a["ok"] == true)
        .unwrap_or(false)
    {
        return 0;
    }
    println!("dictz is not running; starting it.");
    if start_detached(&[]) {
        0
    } else {
        1
    }
}

fn cmd_popup(ctx: &Ctx, word: Vec<String>) -> i32 {
    let word = word.join(" ").trim().to_string();
    match remote(ctx, json!({"cmd": "popup", "word": word})) {
        Some(a) if a["ok"] == true => 0,
        Some(a) => {
            eprintln!("error: {}", a["error"].as_str().unwrap_or("failed"));
            1
        }
        None => {
            println!("dictz is not running; starting it.");
            if start_detached(&["--popup", &word]) {
                0
            } else {
                1
            }
        }
    }
}

fn cmd_shortcut(ctx: &Ctx, remove: bool, desktop: bool, menu: bool) -> i32 {
    if remove {
        let paths = crate::desktop::remove_shortcuts(desktop, menu);
        for p in &paths {
            println!("removed: {}", p.display());
        }
        if paths.is_empty() {
            println!("No dictz shortcuts found.");
        }
        return 0;
    }
    match crate::desktop::create_shortcuts(&ctx.data_dir, desktop, menu) {
        Ok(paths) => {
            for p in &paths {
                println!("created: {}", p.display());
            }
            if paths.is_empty() {
                println!("No desktop or menu folder found.");
                return 1;
            }
            0
        }
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}

fn cmd_watch(ctx: &Ctx, state: WatchState) -> i32 {
    let state = match state {
        WatchState::On => "on",
        WatchState::Off => "off",
        WatchState::Toggle => "toggle",
        WatchState::Status => "status",
    };
    match remote(ctx, json!({"cmd": "scan", "state": state})) {
        None => {
            eprintln!("error: dictz is not running (start it with `dictz` or `dictz show`)");
            1
        }
        Some(a) if a["ok"] != true => {
            eprintln!("error: {}", a["error"].as_str().unwrap_or("failed"));
            1
        }
        Some(a) => {
            println!("scanning: {}", if a["scan"] == true { "on" } else { "off" });
            0
        }
    }
}

// ------------------------------------------------------------------ output

struct Out {
    color: bool,
}

impl Out {
    fn c(&self, t: &str, code: &str) -> String {
        if self.color {
            format!("\x1b[{}m{}\x1b[0m", code, t)
        } else {
            t.to_string()
        }
    }
    fn bold(&self, t: &str) -> String {
        self.c(t, "1")
    }
    fn head(&self, t: &str) -> String {
        self.c(t, "1;36")
    }
    fn dim(&self, t: &str) -> String {
        self.c(t, "2")
    }
    fn err(&self, t: &str) -> String {
        self.c(t, "1;31")
    }
}

fn use_color(no_color: bool) -> bool {
    !no_color && std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn group(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{}", out)
    } else {
        out
    }
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - n))
    }
}

fn table(rows: &[Vec<String>], header: &[&str]) -> String {
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let line = |cells: Vec<String>| {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| pad(c, widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let mut lines = vec![line(header.iter().map(|s| s.to_string()).collect())];
    for r in rows {
        lines.push(line(r.clone()));
    }
    lines.join("\n")
}

/// Python's `f"{v:g}"` for the values settings use.
fn fmt_g(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{:.0}", v)
    } else {
        let s = format!("{:.6}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn fmt_value(v: &Value) -> String {
    match v {
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Array(_) => python_json_inline(v),
        Value::Number(n) => {
            if n.is_f64() {
                fmt_g(n.as_f64().unwrap_or(0.0))
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// `json.dumps(v, ensure_ascii=False)` (", " and ": " separators).
fn python_json_inline(v: &Value) -> String {
    match v {
        Value::Array(a) => format!(
            "[{}]",
            a.iter()
                .map(python_json_inline)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", Value::from(k.as_str()), python_json_inline(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    }
}

// ------------------------------------------------------------------ context

struct Ctx {
    data_dir: PathBuf,
    index: Option<IndexStore>,
    history: Option<HistoryStore>,
    prefs: Option<Preferences>,
}

fn die(msg: impl AsRef<str>) -> ! {
    eprintln!("{}", msg.as_ref());
    std::process::exit(1)
}

fn repr(s: &str) -> String {
    crate::settings::pyrepr(&Value::from(s))
}

impl Ctx {
    fn index(&mut self) -> &mut IndexStore {
        if self.index.is_none() {
            let i = IndexStore::open(&self.data_dir.join(INDEX_DB))
                .unwrap_or_else(|e| die(format!("error: {}", e)));
            self.index = Some(i);
        }
        self.index.as_mut().unwrap()
    }
    fn history(&mut self) -> &HistoryStore {
        if self.history.is_none() {
            let h = HistoryStore::open(&self.data_dir.join(HISTORY_DB))
                .unwrap_or_else(|e| die(format!("error: {}", e)));
            self.history = Some(h);
        }
        self.history.as_ref().unwrap()
    }
    fn prefs(&mut self) -> &mut Preferences {
        if self.prefs.is_none() {
            self.prefs = Some(Preferences::load(&self.data_dir.join(SETTINGS_FILE)));
        }
        self.prefs.as_mut().unwrap()
    }
    fn rows(&mut self) -> Vec<DictionaryRow> {
        self.index().list_dictionaries(false).unwrap_or_default()
    }
    /// QueryManager with the enabled (or the given) dictionaries loaded.
    fn qm(&mut self, names: Option<Vec<String>>, load: bool) -> QueryManager {
        let index = self.index.take().unwrap_or_else(|| {
            IndexStore::open(&self.data_dir.join(INDEX_DB))
                .unwrap_or_else(|e| die(format!("error: {}", e)))
        });
        let mut qm = QueryManager::new(Some(index));
        if load {
            let enabled_only = names.is_none();
            let (_, errors) = qm.restore_with(
                &RestoreOptions {
                    names,
                    enabled_only,
                    reindex_changed: false,
                },
                |_, _, _| {},
            );
            for e in errors {
                eprintln!("warning: {}", e);
            }
        }
        qm
    }
    /// Exact name, else case-insensitive, else unique substring.
    fn resolve_names(&mut self, wanted: &[String]) -> Vec<String> {
        let names: Vec<String> = self.rows().into_iter().map(|r| r.name).collect();
        let mut out = Vec::new();
        for w in wanted {
            if names.contains(w) {
                out.push(w.clone());
                continue;
            }
            let ci: Vec<&String> = names
                .iter()
                .filter(|n| n.to_lowercase() == w.to_lowercase())
                .collect();
            let sub: Vec<&String> = names
                .iter()
                .filter(|n| n.to_lowercase().contains(&w.to_lowercase()))
                .collect();
            if ci.len() == 1 {
                out.push(ci[0].clone());
            } else if sub.len() == 1 {
                out.push(sub[0].clone());
            } else if sub.is_empty() {
                die(format!(
                    "error: no dictionary named {} (see `dictz dicts`)",
                    repr(w)
                ));
            } else {
                die(format!(
                    "error: {} matches several dictionaries: {}",
                    repr(w),
                    sub.iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        out
    }
}

fn article_text(definition: &str) -> String {
    html::to_text(definition)
}

fn print_articles(entries: &[DictionaryEntry], out: &Out) {
    let mut last: Option<&str> = None;
    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    for e in entries {
        if last != Some(e.dictionary_id.as_str()) {
            last = Some(e.dictionary_id.as_str());
            let _ = writeln!(w, "{}", out.head(&format!("== {} ==", e.dictionary_id)));
        }
        let _ = writeln!(w, "{}", out.bold(&e.headword));
        for line in article_text(&e.definition).split('\n') {
            if line.is_empty() {
                let _ = writeln!(w);
            } else {
                let _ = writeln!(w, "  {}", line);
            }
        }
        let _ = writeln!(w);
    }
}

fn entry_json(e: &DictionaryEntry) -> Value {
    json!({"dictionary": e.dictionary_id, "headword": e.headword, "definition": e.definition,
           "text": article_text(&e.definition)})
}

// ------------------------------------------------------------------ commands

fn cmd_lookup(ctx: &mut Ctx, a: LookupArgs, out: &Out) -> i32 {
    let word = a.word.join(" ").trim().to_string();
    let names = if a.dict.is_empty() {
        None
    } else {
        Some(ctx.resolve_names(&a.dict))
    };
    let (prefix_fallback, use_fuzzy, fz_limit, fz_cutoff, max_articles, record) = {
        let p = ctx.prefs();
        (
            p.bool("search.prefix_fallback"),
            p.bool("search.fuzzy"),
            p.int("search.fuzzy_limit") as usize,
            p.float("search.fuzzy_cutoff"),
            p.int("search.max_articles") as usize,
            p.bool("history.enabled"),
        )
    };
    let limit = a.limit.unwrap_or(max_articles);
    let mut qm = ctx.qm(names.clone(), true);
    let started = Instant::now();
    let mut entries = qm.search_in(&word, !a.prefix, names.as_deref(), Some(limit));
    if entries.is_empty() && !a.prefix && prefix_fallback {
        entries = qm.search_in(&word, false, names.as_deref(), Some(limit));
    }
    let mut cands = Vec::new();
    if entries.is_empty() && !a.no_fuzzy && use_fuzzy {
        cands = qm.fuzzy(&word, fz_limit, fz_cutoff);
        if a.fuzzy && !cands.is_empty() {
            entries = qm.search_in(&cands[0].0, true, names.as_deref(), Some(limit));
        }
    }
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    ctx.index = qm.index.take();
    if record && !entries.is_empty() && !a.no_history {
        let _ = ctx.history().record(&word, "");
    }
    if a.json {
        print_json(&Value::Array(entries.iter().map(entry_json).collect()));
        return if entries.is_empty() { 1 } else { 0 };
    }
    if entries.is_empty() {
        eprintln!("{}", out.err(&format!("No results for '{}'.", word)));
        if !cands.is_empty() {
            eprintln!(
                "Did you mean: {}",
                cands
                    .iter()
                    .map(|(w, s)| format!("{} ({:.0}%)", w, s))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        return 1;
    }
    if a.html {
        for e in &entries {
            println!(
                "<!-- {} -->\n<h3>{}</h3>\n{}\n",
                e.dictionary_id, e.headword, e.definition
            );
        }
    } else {
        print_articles(&entries, out);
    }
    if a.verbose {
        let mut ds: Vec<&str> = entries.iter().map(|e| e.dictionary_id.as_str()).collect();
        ds.sort();
        ds.dedup();
        eprintln!(
            "{}",
            out.dim(&format!(
                "{} articles in {} dictionaries, {:.0} ms",
                entries.len(),
                ds.len(),
                elapsed
            ))
        );
    }
    0
}

fn enabled_names(ctx: &mut Ctx) -> Vec<String> {
    ctx.rows()
        .into_iter()
        .filter(|r| r.enabled)
        .map(|r| r.name)
        .collect()
}

fn cmd_suggest(
    ctx: &mut Ctx,
    prefix: Vec<String>,
    limit: usize,
    no_fuzzy: bool,
    as_json: bool,
) -> i32 {
    let (use_fuzzy, fill, fz_limit, fz_cutoff) = {
        let p = ctx.prefs();
        (
            p.bool("search.fuzzy"),
            p.int("search.fuzzy_fill_below") as usize,
            p.int("search.fuzzy_limit") as usize,
            p.float("search.fuzzy_cutoff"),
        )
    };
    let enabled = enabled_names(ctx);
    let prefix = prefix.join(" ").trim().to_string();
    let rows = ctx
        .index()
        .search_headword(&prefix, limit, Some(&enabled))
        .unwrap_or_default();
    let mut words: Vec<String> = Vec::new();
    for (w, _) in rows {
        if !words.contains(&w) {
            words.push(w);
        }
    }
    fuzzy::rank(&prefix, &mut words);
    let mut items: Vec<(String, bool)> = words.iter().map(|w| (w.clone(), false)).collect();
    if use_fuzzy && !no_fuzzy && items.len() < fill {
        let cands = ctx.index().headwords(&enabled).unwrap_or_default();
        let mut seen: std::collections::HashSet<String> = words.into_iter().collect();
        for (w, _) in fuzzy::extract(
            &prefix,
            cands.iter().map(String::as_str),
            fz_limit,
            fz_cutoff,
        ) {
            if items.len() >= limit {
                break;
            }
            if seen.insert(w.clone()) {
                items.push((w, true));
            }
        }
    }
    if as_json {
        print_json(&Value::Array(
            items
                .iter()
                .map(|(w, f)| json!({"word": w, "fuzzy": f}))
                .collect(),
        ));
    } else {
        for (w, f) in &items {
            println!("{}{}", if *f { "≈ " } else { "" }, w);
        }
    }
    if items.is_empty() {
        1
    } else {
        0
    }
}

fn cmd_fuzzy(
    ctx: &mut Ctx,
    word: Vec<String>,
    limit: Option<usize>,
    cutoff: Option<f64>,
    as_json: bool,
) -> i32 {
    let (dl, dc) = {
        let p = ctx.prefs();
        (
            p.int("search.fuzzy_limit") as usize,
            p.float("search.fuzzy_cutoff"),
        )
    };
    let enabled = enabled_names(ctx);
    let word = word.join(" ").trim().to_string();
    let cands = ctx.index().headwords(&enabled).unwrap_or_default();
    let hits = fuzzy::extract(
        &word,
        cands.iter().map(String::as_str),
        limit.unwrap_or(dl),
        cutoff.unwrap_or(dc),
    );
    if as_json {
        print_json(&Value::Array(
            hits.iter()
                .map(|(w, s)| json!({"word": w, "score": s}))
                .collect(),
        ));
    } else {
        for (w, s) in &hits {
            println!("{:6.2}  {}", s, w);
        }
    }
    if hits.is_empty() {
        1
    } else {
        0
    }
}

fn cmd_random(ctx: &mut Ctx, dict: Option<String>, as_json: bool, out: &Out) -> i32 {
    let names = dict.map(|d| ctx.resolve_names(&[d]));
    let qm = ctx.qm(names.clone(), true);
    let Some(e) = qm.random_entry(names.as_ref().map(|n| n[0].as_str())) else {
        eprintln!("No dictionaries.");
        return 1;
    };
    if as_json {
        print_json(&entry_json(&e));
    } else {
        print_articles(&[e], out);
    }
    0
}

fn row_json(i: usize, r: &DictionaryRow) -> Value {
    json!({"position": i + 1, "name": r.name, "enabled": r.enabled, "format": r.format, "words": r.word_count,
           "path": r.path.to_string_lossy(), "source_language": r.source_language,
           "target_language": r.target_language, "exists": r.path.exists()})
}

fn cmd_dicts(ctx: &mut Ctx, as_json: bool, names: bool) -> i32 {
    let rows = ctx.rows();
    if names {
        for r in &rows {
            println!("{}", r.name);
        }
        return 0;
    }
    if as_json {
        print_json(&Value::Array(
            rows.iter()
                .enumerate()
                .map(|(i, r)| row_json(i, r))
                .collect(),
        ));
        return 0;
    }
    if rows.is_empty() {
        println!("No dictionaries. Add one with: dictz add FILE");
        return 0;
    }
    let t: Vec<Vec<String>> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            vec![
                (i + 1).to_string(),
                if r.enabled { "yes" } else { "no" }.into(),
                r.name.clone(),
                r.format.clone(),
                group(r.word_count),
                format!(
                    "{}{}",
                    r.path.display(),
                    if r.path.exists() { "" } else { "  (missing)" }
                ),
            ]
        })
        .collect();
    println!(
        "{}",
        table(&t, &["#", "on", "name", "format", "words", "path"])
    );
    let total: i64 = rows
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.word_count)
        .sum();
    println!(
        "\n{} of {} enabled · {} words",
        rows.iter().filter(|r| r.enabled).count(),
        rows.len(),
        group(total)
    );
    0
}

fn expand_paths(paths: &[PathBuf], recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        let p = &std::path::absolute(p).unwrap_or_else(|_| p.clone());
        if p.is_dir() {
            out.extend(scan_folder(p, recursive));
        } else {
            out.push(p.clone());
        }
    }
    out
}

fn renumber(ctx: &mut Ctx, order: &[String]) {
    let _ = ctx.index().set_order(order);
}

fn cmd_add(
    ctx: &mut Ctx,
    paths: &[PathBuf],
    recursive: bool,
    reindex: bool,
    disabled: bool,
) -> i32 {
    let files = expand_paths(paths, recursive);
    if files.is_empty() {
        eprintln!("No dictionary files found.");
        return 1;
    }
    let mut rc = 0;
    for f in files {
        let mut qm = ctx.qm(None, false);
        let res = qm.add_dictionary(&f, reindex);
        let info = res.as_ref().ok().and_then(|n| {
            qm.dictionary(n)
                .map(|d| (d.metadata().format.clone(), d.metadata().word_count))
        });
        ctx.index = qm.index.take();
        match res {
            Ok(name) => {
                // dictionaries.json keeps the order (new ones at the end)
                if disabled {
                    let _ = ctx.index().set_enabled(&name, false);
                }
                let (fmt, words) = info.unwrap_or_default();
                println!("added: {} ({}, {} words)", name, fmt, group(words as i64));
            }
            Err(e) => {
                eprintln!("error: {}: {}", f.display(), e);
                rc = 1;
            }
        }
    }
    rc
}

fn cmd_move(ctx: &mut Ctx, name: &str, where_: &str) -> i32 {
    let name = ctx.resolve_names(&[name.to_string()]).remove(0);
    let mut names: Vec<String> = ctx.rows().into_iter().map(|r| r.name).collect();
    let i = names.iter().position(|n| *n == name).unwrap();
    let j: isize = match where_.to_lowercase().as_str() {
        "up" => i as isize - 1,
        "down" => i as isize + 1,
        "top" => 0,
        "bottom" => names.len() as isize - 1,
        other => match other.parse::<isize>() {
            Ok(p) => p - 1,
            Err(_) => die("error: position must be up, down, top, bottom or a number"),
        },
    };
    let j = j.clamp(0, names.len() as isize - 1) as usize;
    let n = names.remove(i);
    names.insert(j, n);
    renumber(ctx, &names);
    println!("{}: position {}", name, j + 1);
    0
}

fn resolve_target(ctx: &mut Ctx, target: &str) -> PathBuf {
    let p = PathBuf::from(target);
    if p.exists() {
        return p;
    }
    let name = ctx.resolve_names(&[target.to_string()]).remove(0);
    ctx.index()
        .get_dictionary(&name)
        .ok()
        .flatten()
        .map(|r| r.path)
        .unwrap_or(p)
}

fn local_time(secs: f64) -> String {
    // UTC (no timezone database without extra dependencies)
    let s = secs as i64;
    let days = s.div_euclid(86400);
    let rem = s.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60
    )
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A dictionary in the list opens from its article store; any other file is parsed.
fn open_known(
    ctx: &mut Ctx,
    path: &Path,
) -> (
    Box<dyn crate::Dictionary>,
    Option<crate::index::DictionaryRow>,
) {
    let me = crate::registry::norm_path(path);
    let row = ctx
        .rows()
        .into_iter()
        .find(|r| crate::registry::norm_path(&r.path) == me);
    let opened = match (&row, ctx.index().data_dir()) {
        (Some(_), Some(dir)) => crate::articles::open_cached(path, &dir, false).map(|(d, _)| d),
        _ => QueryManager::open_file(path),
    };
    let mut d = opened.unwrap_or_else(|e| die(format!("error: {}", e)));
    if let Some(r) = &row {
        d.metadata_mut().name = r.name.clone();
    }
    (d, row)
}

fn cmd_info(ctx: &mut Ctx, target: Vec<String>, as_json: bool) -> i32 {
    let path = resolve_target(ctx, &target.join(" "));
    let (d, row) = open_known(ctx, &path);
    let m = d.metadata();
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let info = json!({
        "name": m.name, "format": m.format, "words": m.word_count, "path": path.to_string_lossy(),
        "source_language": m.source_language, "target_language": m.target_language,
        "author": m.author, "description": m.description, "size_bytes": size,
        "modified": local_time(crate::index::file_mtime(&path)),
        "in_dictz": row.is_some(), "enabled": row.map(|r| r.enabled).unwrap_or(false),
    });
    if as_json {
        print_json(&info);
        return 0;
    }
    for (k, v) in info.as_object().unwrap() {
        let s = match v {
            Value::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if !s.is_empty() {
            println!("{} {}", pad(&k.replace('_', " "), 16), s);
        }
    }
    0
}

fn cmd_reindex(ctx: &mut Ctx, names: Vec<String>) -> i32 {
    let names = if names.is_empty() {
        ctx.rows().into_iter().map(|r| r.name).collect()
    } else {
        ctx.resolve_names(&names)
    };
    let mut rc = 0;
    for n in names {
        let Some(row) = ctx.index().get_dictionary(&n).ok().flatten() else {
            continue;
        };
        let dir = ctx.index().data_dir();
        let opened = match dir {
            Some(dir) => crate::articles::open_cached(&row.path, &dir, true).map(|(d, _)| d),
            None => QueryManager::open_file(&row.path),
        };
        match opened.map(|mut d| {
            d.metadata_mut().name = n.clone();
            d
        }) {
            Ok(d) => match ctx.index().index_dictionary(d.as_ref()) {
                Ok(c) => println!("reindexed: {} ({} headwords)", n, group(c as i64)),
                Err(e) => {
                    eprintln!("error: {}: {}", n, e);
                    rc = 1;
                }
            },
            Err(e) => {
                eprintln!("error: {}: {}", n, e);
                rc = 1;
            }
        }
    }
    rc
}

fn cmd_scan(ctx: &mut Ctx, folders: &[PathBuf], no_recursive: bool, add: bool) -> i32 {
    let files = expand_paths(folders, !no_recursive);
    let canon = |p: &Path| crate::registry::norm_path(p);
    let known: Vec<String> = ctx.rows().into_iter().map(|r| canon(&r.path)).collect();
    for f in &files {
        let mark = if known.contains(&canon(f)) {
            "added"
        } else {
            "new"
        };
        println!("{:6} {}", mark, f.display());
    }
    if add {
        let new: Vec<PathBuf> = files
            .iter()
            .filter(|f| !known.contains(&canon(f)))
            .cloned()
            .collect();
        if !new.is_empty() {
            return cmd_add(ctx, &new, false, false, false);
        }
    }
    0
}

fn cmd_dump(ctx: &mut Ctx, target: &str, format: DumpFormat, output: Option<PathBuf>) -> i32 {
    let path = resolve_target(ctx, target);
    let (d, _row) = open_known(ctx, &path);
    let mut buf = String::new();
    match format {
        DumpFormat::Json => {
            let arr: Vec<Value> = d
                .entries()
                .iter()
                .map(|e| json!({"headword": e.headword, "definition": e.definition}))
                .collect();
            let mut w = Vec::new();
            let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
            let mut ser = serde_json::Serializer::with_formatter(&mut w, fmt);
            serde::Serialize::serialize(&Value::Array(arr), &mut ser).ok();
            buf.push_str(&String::from_utf8_lossy(&w));
            buf.push('\n');
        }
        DumpFormat::Tsv => {
            for e in d.entries() {
                let defi = e
                    .definition
                    .replace('\\', "\\\\")
                    .replace('\t', "\\t")
                    .replace('\n', "\\n");
                buf.push_str(&format!("{}\t{}\n", e.headword, defi));
            }
        }
        DumpFormat::Txt => {
            for e in d.entries() {
                buf.push_str(&e.headword);
                buf.push('\n');
                for line in article_text(&e.definition).split('\n') {
                    if line.is_empty() {
                        buf.push('\n');
                    } else {
                        buf.push_str(&format!("  {}\n", line));
                    }
                }
                buf.push('\n');
            }
        }
    }
    match output {
        Some(o) => {
            if let Err(e) = std::fs::write(&o, buf) {
                die(format!("error: {}", e));
            }
            eprintln!(
                "wrote {} entries to {}",
                group(d.metadata().word_count as i64),
                o.display()
            );
        }
        None => {
            let _ = std::io::stdout().write_all(buf.as_bytes());
        }
    }
    0
}

fn cmd_config(
    ctx: &mut Ctx,
    action: Option<ConfigAction>,
    key: Option<String>,
    value: Option<String>,
    as_json: bool,
    out: &Out,
) -> i32 {
    let p = ctx.prefs();
    match action.unwrap_or(ConfigAction::List) {
        ConfigAction::Path => println!(
            "{}",
            p.path()
                .map(|x| x.display().to_string())
                .unwrap_or_default()
        ),
        ConfigAction::Get => {
            let k = key.unwrap_or_default();
            match p.get(&k) {
                Some(v) => println!("{}", fmt_value(v)),
                None => die(format!(
                    "error: unknown setting {} (see `dictz config list`)",
                    repr(&k)
                )),
            }
        }
        ConfigAction::Set => {
            let (Some(k), Some(v)) = (key, value) else {
                die("error: usage: dictz config set KEY VALUE")
            };
            let is_list = crate::settings::setting(&k)
                .map(|s| s.kind == Kind::List)
                .unwrap_or(false);
            let val = if is_list && v.trim().starts_with('[') {
                serde_json::from_str(&v).unwrap_or_else(|e| die(format!("error: {}", e)))
            } else {
                Value::from(v)
            };
            match p.set(&k, val) {
                Ok(stored) => {
                    if let Err(e) = p.save() {
                        die(format!("error: {}", e));
                    }
                    println!("{} = {}", k, fmt_value(&stored));
                }
                Err(e) => die(format!("error: {}", e)),
            }
        }
        ConfigAction::Reset => {
            if let Err(e) = p.reset(key.as_deref()) {
                die(format!("error: {}", e));
            }
            let _ = p.save();
            println!("reset: {}", key.unwrap_or_else(|| "all settings".into()));
        }
        ConfigAction::Export => {
            let data = serde_json::to_string_pretty(&p.export()).unwrap_or_default();
            match key {
                Some(f) => {
                    if let Err(e) = std::fs::write(&f, format!("{}\n", data)) {
                        die(format!("error: {}", e));
                    }
                    println!("exported to {}", f);
                }
                None => println!("{}", data),
            }
        }
        ConfigAction::Import => {
            let Some(f) = key else {
                die("error: usage: dictz config import FILE")
            };
            let text = std::fs::read_to_string(&f).unwrap_or_else(|e| die(format!("error: {}", e)));
            let data: Value =
                serde_json::from_str(&text).unwrap_or_else(|e| die(format!("error: {}", e)));
            let errors = p.import(&data);
            let _ = p.save();
            for e in errors {
                eprintln!("warning: {}", e);
            }
            println!("imported {}", f);
        }
        ConfigAction::List => {
            if as_json {
                let arr: Vec<Value> = SCHEMA
                    .iter()
                    .map(|s| {
                        json!({"key": s.key, "value": p.get(s.key).cloned().unwrap_or(Value::Null),
                               "default": s.default.value(), "kind": s.kind.as_str(), "label": s.label})
                    })
                    .collect();
                print_json(&Value::Array(arr));
                return 0;
            }
            for (sec, title) in SECTIONS {
                println!("{}", out.head(&format!("[{}] {}", sec, title)));
                for s in SCHEMA
                    .iter()
                    .filter(|s| s.key.split('.').next() == Some(*sec))
                {
                    let v = p.get(s.key).cloned().unwrap_or(Value::Null);
                    let d = s.default.value();
                    let changed = match (&v, &d) {
                        (Value::Number(a), Value::Number(b)) => a.as_f64() != b.as_f64(),
                        _ => v != d,
                    };
                    let extra = if s.kind == Kind::Choice {
                        format!(
                            " ({})",
                            s.choices.iter().map(|c| c.0).collect::<Vec<_>>().join("|")
                        )
                    } else if let (Some(mn), Some(mx)) = (s.min, s.max) {
                        if s.kind == Kind::Int {
                            format!(" ({}–{})", mn as i64, mx as i64)
                        } else {
                            format!(" ({}–{})", fmt_g(mn), fmt_g(mx))
                        }
                    } else {
                        String::new()
                    };
                    let line = format!(
                        "  {} {} {}{}",
                        pad(s.key, 34),
                        pad(&fmt_value(&v), 22),
                        s.label,
                        extra
                    );
                    println!("{}", if changed { out.bold(&line) } else { line });
                }
                println!();
            }
        }
    }
    0
}

fn paths_json(ctx: &Ctx) -> Value {
    let d = &ctx.data_dir;
    json!({"data_dir": d.to_string_lossy(), "settings": d.join(SETTINGS_FILE).to_string_lossy(),
           "index_db": d.join(INDEX_DB).to_string_lossy(), "history_db": d.join(HISTORY_DB).to_string_lossy(),
           "media_cache": d.join("media-cache").to_string_lossy(),
           "executable": std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()})
}

fn cmd_doctor(ctx: &mut Ctx, out: &Out) -> i32 {
    println!("{}", out.head("dictz doctor"));
    println!("  version      {} ({})", VERSION, EDITION);
    #[cfg(feature = "update")]
    {
        let i = crate::updater::detect_install();
        let kind = match i.kind {
            crate::updater::InstallKind::Cargo => "cargo",
            crate::updater::InstallKind::Portable => "portable",
        };
        println!("  install      {} — {}", kind, i.detail);
    }
    println!(
        "  platform     {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "  features     {}",
        [
            ("gui", cfg!(feature = "gui")),
            ("audio", cfg!(feature = "audio")),
            ("update", cfg!(feature = "update"))
        ]
        .iter()
        .filter(|f| f.1)
        .map(|f| f.0)
        .collect::<Vec<_>>()
        .join(", ")
    );
    for (k, v) in paths_json(ctx).as_object().unwrap() {
        println!("  {} {}", pad(k, 12), v.as_str().unwrap_or(""));
    }
    let (d, e) = ctx.index().stats().unwrap_or((0, 0));
    println!("  index        {} dictionaries, {} headwords", d, group(e));
    let lookups = ctx.history().count().unwrap_or(0);
    let favs = ctx
        .history()
        .favorites(100000)
        .map(|f| f.len())
        .unwrap_or(0);
    println!(
        "  history      {} lookups, {} favorites",
        group(lookups),
        favs
    );
    let mut problems = 0;
    for r in ctx.rows() {
        if !r.path.exists() {
            problems += 1;
            println!(
                "{}",
                out.err(&format!(
                    "  missing file: {} -> {}",
                    r.name,
                    r.path.display()
                ))
            );
        }
    }
    println!("{}", out.head("selftest"));
    let rc = crate::selftest::run();
    if rc != 0 || problems > 0 {
        1
    } else {
        0
    }
}

/// Shell completion script — identical to the Python edition's.
pub fn completion_script(shell: &str) -> &'static str {
    match shell {
        "bash" => include_str!("completion/bash.txt"),
        "zsh" => include_str!("completion/zsh.txt"),
        "fish" => include_str!("completion/fish.txt"),
        _ => include_str!("completion/powershell.txt"),
    }
}

fn cmd_shell(ctx: &mut Ctx, out: &Out) -> i32 {
    let record = ctx.prefs().bool("history.enabled");
    let mut qm = ctx.qm(None, true);
    println!(
        "dictz {} — {} dictionaries. Type a word; :help for commands, :q to quit.",
        VERSION,
        qm.loaded_names().len()
    );
    let stdin = std::io::stdin();
    loop {
        print!("dictz> ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            ":q" | ":quit" | ":exit" => break,
            ":h" | ":help" | "?" => {
                println!(":fuzzy WORD   did-you-mean candidates\n:suggest P    prefix suggestions\n:dicts        list dictionaries\n:random       random article\n:q            quit");
                continue;
            }
            ":dicts" => {
                for n in qm.loaded_names() {
                    println!("{}{}", if qm.is_enabled(&n) { "  " } else { "- " }, n);
                }
                continue;
            }
            ":random" => {
                if let Some(e) = qm.random_entry(None) {
                    print_articles(&[e], out);
                }
                continue;
            }
            _ => {}
        }
        if let Some(w) = line.strip_prefix(":fuzzy ") {
            for (w, s) in qm.fuzzy(w.trim(), fuzzy::DEFAULT_LIMIT, fuzzy::DEFAULT_CUTOFF) {
                println!("{:6.2}  {}", s, w);
            }
            continue;
        }
        if let Some(p) = line.strip_prefix(":suggest ") {
            for s in qm.suggest(p.trim(), 30) {
                println!("{}{}", if s.fuzzy { "≈ " } else { "" }, s.word);
            }
            continue;
        }
        let mut entries = qm.search(line, true);
        if entries.is_empty() {
            entries = qm.search(line, false);
        }
        if !entries.is_empty() {
            print_articles(&entries, out);
            if record {
                if let Some(h) = &ctx.history {
                    let _ = h.record(line, "");
                } else if let Ok(h) = HistoryStore::open(&ctx.data_dir.join(HISTORY_DB)) {
                    let _ = h.record(line, "");
                    ctx.history = Some(h);
                }
            }
        } else {
            let cands = qm.fuzzy(line, fuzzy::DEFAULT_LIMIT, fuzzy::DEFAULT_CUTOFF);
            let tail = if cands.is_empty() {
                String::new()
            } else {
                format!(
                    " Did you mean: {}",
                    cands
                        .iter()
                        .map(|c| c.0.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            println!(
                "{}{}",
                out.err(&format!("No results for '{}'.", line)),
                tail
            );
        }
    }
    0
}

/// Run the CLI with `args` (without the program name).
pub fn run(args: Vec<String>) -> Outcome {
    use clap::{CommandFactory, FromArgMatches};
    let prog = program_name();
    let mut full = vec![prog.to_string()];
    full.extend(normalize_args(args));
    let parsed = Cli::command()
        .name(prog)
        .bin_name(prog)
        .try_get_matches_from(&full)
        .and_then(|m| Cli::from_arg_matches(&m));
    let cli = match parsed {
        Ok(c) => c,
        Err(e) => {
            let code = if e.use_stderr() { 2 } else { 0 };
            let _ = e.print();
            return Outcome::Exit(code);
        }
    };
    if let Some(d) = &cli.data_dir {
        std::env::set_var("DICTZ_DATA_DIR", d);
    }
    if cli.version {
        println!("dictz {} ({})", VERSION, EDITION);
        return Outcome::Exit(0);
    }
    if cli.selftest {
        return Outcome::Exit(crate::selftest::run());
    }
    let out = Out {
        color: use_color(cli.no_color),
    };
    let mut ctx = Ctx {
        data_dir: user_data_dir(),
        index: None,
        history: None,
        prefs: None,
    };
    let code = match cli.command {
        None => {
            return Outcome::Gui {
                files: Vec::new(),
                popup: None,
            }
        }
        Some(Cmd::Gui { files, popup }) => return Outcome::Gui { files, popup },
        Some(Cmd::Show) => cmd_show(&ctx),
        Some(Cmd::Popup { word }) => cmd_popup(&ctx, word),
        Some(Cmd::Watch { state }) => cmd_watch(&ctx, state),
        Some(Cmd::Shortcut {
            remove,
            desktop_only,
            menu_only,
        }) => cmd_shortcut(&ctx, remove, !menu_only, !desktop_only),
        Some(Cmd::Lookup(a)) => cmd_lookup(&mut ctx, a, &out),
        Some(Cmd::Suggest {
            prefix,
            limit,
            no_fuzzy,
            json,
        }) => cmd_suggest(&mut ctx, prefix, limit, no_fuzzy, json),
        Some(Cmd::Fuzzy {
            word,
            limit,
            cutoff,
            json,
        }) => cmd_fuzzy(&mut ctx, word, limit, cutoff, json),
        Some(Cmd::Random { dict, json }) => cmd_random(&mut ctx, dict, json, &out),
        Some(Cmd::Dicts { json, names }) => cmd_dicts(&mut ctx, json, names),
        Some(Cmd::Add {
            paths,
            recursive,
            reindex,
            disabled,
        }) => cmd_add(&mut ctx, &paths, recursive, reindex, disabled),
        Some(Cmd::Remove { names }) => {
            for n in ctx.resolve_names(&names) {
                let _ = ctx.index().remove_dictionary(&n);
                println!("removed: {}", n);
            }
            0
        }
        Some(Cmd::Enable { names }) => {
            for n in ctx.resolve_names(&names) {
                let _ = ctx.index().set_enabled(&n, true);
                println!("enabled: {}", n);
            }
            0
        }
        Some(Cmd::Disable { names }) => {
            for n in ctx.resolve_names(&names) {
                let _ = ctx.index().set_enabled(&n, false);
                println!("disabled: {}", n);
            }
            0
        }
        Some(Cmd::Move { name, r#where }) => cmd_move(&mut ctx, &name, &r#where),
        Some(Cmd::Info { target, json }) => cmd_info(&mut ctx, target, json),
        Some(Cmd::Reindex { names }) => cmd_reindex(&mut ctx, names),
        Some(Cmd::Scan {
            folders,
            no_recursive,
            add,
        }) => cmd_scan(&mut ctx, &folders, no_recursive, add),
        Some(Cmd::Dump {
            target,
            format,
            output,
        }) => cmd_dump(&mut ctx, &target, format, output),
        Some(Cmd::History {
            limit,
            json,
            clear,
            remove,
        }) => {
            let h = ctx.history();
            if clear {
                let _ = h.clear_history();
                println!("history cleared");
            } else if let Some(w) = remove {
                let _ = h.remove_history(&w);
            } else {
                let words = h.recent(limit).unwrap_or_default();
                if json {
                    print_json(&json!(words));
                } else {
                    for w in words {
                        println!("{}", w);
                    }
                }
            }
            0
        }
        Some(Cmd::Favorites {
            action,
            words,
            limit,
            json,
        }) => {
            let h = ctx.history();
            match action.unwrap_or(FavAction::List) {
                FavAction::Add => {
                    for w in words {
                        let _ = h.add_favorite(&w);
                        println!("added: {}", w);
                    }
                }
                FavAction::Remove => {
                    for w in words {
                        let _ = h.remove_favorite(&w);
                        println!("removed: {}", w);
                    }
                }
                FavAction::Clear => {
                    let _ = h.clear_favorites();
                    println!("favorites cleared");
                }
                FavAction::List => {
                    let ws = h.favorites(limit).unwrap_or_default();
                    if json {
                        print_json(&json!(ws));
                    } else {
                        for w in ws {
                            println!("{}", w);
                        }
                    }
                }
            }
            0
        }
        Some(Cmd::Config {
            action,
            key,
            value,
            json,
        }) => cmd_config(&mut ctx, action, key, value, json, &out),
        Some(Cmd::Paths { json }) => {
            let p = paths_json(&ctx);
            if json {
                print_json(&p);
            } else {
                for (k, v) in p.as_object().unwrap() {
                    println!("{} {}", pad(k, 12), v.as_str().unwrap_or(""));
                }
            }
            0
        }
        Some(Cmd::Doctor) => cmd_doctor(&mut ctx, &out),
        Some(Cmd::Selftest) => crate::selftest::run(),
        Some(Cmd::Update { check, yes }) => {
            #[cfg(feature = "update")]
            {
                crate::updater::run_update(check, yes)
            }
            #[cfg(not(feature = "update"))]
            {
                let _ = (check, yes);
                println!(
                    "Built without the `update` feature; run: cargo install dictz --locked --force"
                );
                1
            }
        }
        Some(Cmd::Version { json }) => {
            if json {
                print_json(
                    &json!({"version": VERSION, "edition": "rust", "rust": env!("CARGO_PKG_RUST_VERSION")}),
                );
            } else {
                println!("dictz {} ({})", VERSION, EDITION);
            }
            0
        }
        Some(Cmd::Completion { shell }) => {
            let name = match shell {
                Shell::Bash => "bash",
                Shell::Zsh => "zsh",
                Shell::Fish => "fish",
                Shell::Powershell => "powershell",
            };
            print!("{}", completion_script(name));
            0
        }
        Some(Cmd::Shell) => cmd_shell(&mut ctx, &out),
    };
    Outcome::Exit(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_rules() {
        assert!(!is_cli_invocation(&[]));
        assert!(is_cli_invocation(&["lookup".into()]));
        assert!(is_cli_invocation(&["-V".into()]));
        assert!(is_cli_invocation(&["ls".into()]));
        assert!(!is_cli_invocation(&["En-Tr.dsl".into()]));
    }

    #[test]
    fn formatting() {
        assert_eq!(group(1234567), "1,234,567");
        assert_eq!(fmt_g(60.0), "60");
        assert_eq!(fmt_g(1.6), "1.6");
        assert_eq!(fmt_value(&json!(["a", "b"])), "[\"a\", \"b\"]");
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19723), (2024, 1, 1));
        Cli::command_for_test();
    }

    impl Cli {
        fn command_for_test() {
            use clap::CommandFactory;
            Cli::command().debug_assert();
        }
    }
}
