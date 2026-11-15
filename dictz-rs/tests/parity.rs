//! The Rust parsers must read the shared samples exactly like the Python
//! edition does. Golden files: `../tests/data/parity/*_golden.json`
//! (regenerate with `python tests/data/parity/make_samples.py`).

use std::path::PathBuf;

use dictz::query::open_dictionary;
use serde_json::Value;

fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("data")
        .join("parity")
}

fn check(sample: &str, golden: &str) {
    let dir = parity_dir();
    let g: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(golden)).unwrap()).unwrap();
    let mut d = open_dictionary(&dir.join(sample)).unwrap();
    d.load().unwrap();
    let m = d.metadata();
    let gm = &g["meta"];
    assert_eq!(m.name, gm["name"].as_str().unwrap());
    assert_eq!(m.author, gm["author"].as_str().unwrap());
    assert_eq!(m.description, gm["description"].as_str().unwrap());
    assert_eq!(m.source_language, gm["source"].as_str().unwrap());
    assert_eq!(m.target_language, gm["target"].as_str().unwrap());
    assert_eq!(m.word_count as u64, gm["count"].as_u64().unwrap());
    let ours: Vec<(String, String)> = d
        .entries()
        .into_iter()
        .map(|e| (e.headword, e.definition))
        .collect();
    let theirs: Vec<(String, String)> = g["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e[0].as_str().unwrap().to_string(),
                e[1].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(ours.len(), theirs.len(), "entry count");
    for (a, b) in ours.iter().zip(&theirs) {
        assert_eq!(a, b);
    }
}

#[test]
fn dsl_matches_python_edition() {
    check("sample.dsl", "dsl_golden.json");
}

#[test]
fn babylon_matches_python_edition() {
    check("sample.bgl", "bgl_golden.json");
}

#[test]
fn fuzzy_matches_rapidfuzz() {
    let g: Value = serde_json::from_str(
        &std::fs::read_to_string(parity_dir().join("fuzzy_golden.json")).unwrap(),
    )
    .unwrap();
    for row in g.as_array().unwrap() {
        let (a, b, want) = (
            row[0].as_str().unwrap(),
            row[1].as_str().unwrap(),
            row[2].as_f64().unwrap(),
        );
        assert_eq!(dictz::fuzzy::ratio(a, b), want, "ratio({:?}, {:?})", a, b);
    }
}

#[test]
fn dsl_media_from_zip() {
    let dir = parity_dir();
    let r = dictz::media::MediaResolver::for_dictionary(&dir.join("sample.dsl"));
    let png = r
        .read("apple.png")
        .expect("apple.png in sample.dsl.files.zip");
    assert!(png.starts_with(b"\x89PNG"));
    let wav = r
        .read("apple.wav")
        .expect("apple.wav matched case-insensitively by base name");
    assert!(wav.starts_with(b"RIFF"));
}
