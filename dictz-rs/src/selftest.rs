//! Headless self-test: `dictz --selftest`.
//!
//! The same checks as the Python edition (`dictz/selftest.py`). CI runs it
//! against every binary before a release is published. Exit code 0 = all
//! checks passed. The report is printed and, when `DICTZ_SELFTEST_REPORT` is
//! set, also written to that file (the Windows GUI build has no console).

use std::path::{Path, PathBuf};

use crate::babylon::write_bgl;
use crate::dsl::write_dsl_utf16;
use crate::fuzzy;
use crate::index::IndexStore;
use crate::media::MediaResolver;
use crate::query::QueryManager;
use crate::stardict::write_stardict;
use crate::VERSION;

type Check = fn(&Path) -> Result<String, String>;

fn stardict(dir: &Path) -> Result<String, String> {
    let ifo = write_stardict(
        &dir.join("sd"),
        "selftest-stardict",
        &[("apple", "elma"), ("book", "kitap")],
    )
    .map_err(|e| e.to_string())?;
    let mut qm = QueryManager::new(None);
    qm.add_dictionary(&ifo, false).map_err(|e| e.to_string())?;
    let hits = qm.search("apple", true);
    match hits.first() {
        Some(h) if h.definition.contains("elma") => Ok(h.definition.clone()),
        _ => Err(format!("bad lookup: {:?}", hits)),
    }
}

fn dsl(dir: &Path) -> Result<String, String> {
    let f = dir.join("t.dsl");
    write_dsl_utf16(
        &f,
        "\u{feff}#NAME \"selftest-dsl\"\n#INDEX_LANGUAGE \"English\"\n#CONTENTS_LANGUAGE \"Turkish\"\n\n\
         apple\n\t[trn]elma[/trn]\n\nbook\n\t[trn]kitap[/trn]\n",
    )
    .map_err(|e| e.to_string())?;
    let mut qm = QueryManager::new(None);
    qm.add_dictionary(&f, false).map_err(|e| e.to_string())?;
    let hits = qm.search("book", true);
    match hits.first() {
        Some(h) if h.definition.contains("kitap") => Ok("book -> kitap".into()),
        _ => Err(format!("bad lookup: {:?}", hits)),
    }
}

fn babylon(dir: &Path) -> Result<String, String> {
    let f = dir.join("t.bgl");
    write_bgl(
        &f,
        "selftest-bgl",
        0,
        13,
        &[("apple", "elma", &["apples"]), ("book", "kitap", &[])],
    )
    .map_err(|e| e.to_string())?;
    let mut qm = QueryManager::new(None);
    qm.add_dictionary(&f, false).map_err(|e| e.to_string())?;
    let hits = qm.search("apples", true);
    match hits.first() {
        Some(h) if h.definition == "elma" => Ok("apples -> elma".into()),
        _ => Err(format!("bad lookup: {:?}", hits)),
    }
}

fn sqlite_fts5(dir: &Path) -> Result<String, String> {
    let ifo = write_stardict(
        &dir.join("fts"),
        "selftest-fts",
        &[("café", "x"), ("cafeteria", "y")],
    )
    .map_err(|e| e.to_string())?;
    let mut qm = QueryManager::new(Some(
        IndexStore::open(&dir.join("i.db")).map_err(|e| e.to_string())?,
    ));
    qm.add_dictionary(&ifo, false).map_err(|e| e.to_string())?;
    let s = qm.suggest("cafe", 10);
    if s.iter().filter(|s| !s.fuzzy).count() == 2 {
        Ok("FTS5 prefix search".into())
    } else {
        Err(format!("unexpected suggestions: {:?}", s))
    }
}

fn fuzzy_matching(_dir: &Path) -> Result<String, String> {
    let r = fuzzy::ratio("aple", "apple");
    if r == 88.88888888888889 {
        Ok(format!("ratio(aple, apple) = {:.2}", r))
    } else {
        Err(format!("unexpected ratio {}", r))
    }
}

fn media(dir: &Path) -> Result<String, String> {
    use std::io::Write;
    let zf = std::fs::File::create(dir.join("m.dsl.files.zip")).map_err(|e| e.to_string())?;
    let mut zw = zip::ZipWriter::new(zf);
    zw.start_file("pic.png", zip::write::SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    zw.write_all(b"\x89PNG").map_err(|e| e.to_string())?;
    zw.finish().map_err(|e| e.to_string())?;
    let r = MediaResolver::for_dictionary(&dir.join("m.dsl"));
    match r.read("PIC.png") {
        Some(b) if b == b"\x89PNG" => Ok("zip lookup".into()),
        other => Err(format!("media not found: {:?}", other.map(|b| b.len()))),
    }
}

/// Run all checks; returns the process exit code.
pub fn run() -> i32 {
    let mut lines = vec![
        format!("dictz {} selftest", VERSION),
        format!(
            "rust edition, {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    ];
    let dir: PathBuf = std::env::temp_dir().join(format!("dictz-selftest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let checks: [(&str, Check); 6] = [
        ("StarDict parser", stardict),
        ("DSL parser", dsl),
        ("Babylon parser", babylon),
        ("SQLite FTS5 index", sqlite_fts5),
        ("Fuzzy matching", fuzzy_matching),
        ("Media resolver", media),
    ];
    let mut failures = 0;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        lines.push(format!("FAIL  temp dir: {}", e));
        failures += 1;
    } else {
        for (name, f) in checks {
            match std::panic::catch_unwind(|| f(&dir)) {
                Ok(Ok(detail)) => lines.push(format!("PASS  {}  ({})", name, detail)),
                Ok(Err(e)) => {
                    failures += 1;
                    lines.push(format!("FAIL  {}: {}", name, e));
                }
                Err(_) => {
                    failures += 1;
                    lines.push(format!("FAIL  {}: panic", name));
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    lines.push(if failures == 0 {
        "RESULT: OK".to_string()
    } else {
        format!("RESULT: {} FAILED", failures)
    });
    let report = lines.join("\n");
    println!("{}", report);
    if let Some(out) = std::env::var_os("DICTZ_SELFTEST_REPORT") {
        let _ = std::fs::write(out, format!("{}\n", report));
    }
    if failures == 0 {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn selftest_passes() {
        assert_eq!(super::run(), 0);
    }
}
