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

#[test]
fn article_text_matches_python_edition() {
    let g: Value = serde_json::from_str(
        &std::fs::read_to_string(parity_dir().join("text_golden.json")).unwrap(),
    )
    .unwrap();
    for row in g.as_array().unwrap() {
        let (html, want) = (row[0].as_str().unwrap(), row[1].as_str().unwrap());
        assert_eq!(dictz::html::to_text(html), want, "to_text({:?})", html);
    }
}

#[test]
fn settings_schema_matches_python_edition() {
    use dictz::settings::SCHEMA;
    let g: Value = serde_json::from_str(
        &std::fs::read_to_string(parity_dir().join("settings_schema.json")).unwrap(),
    )
    .unwrap();
    let rows = g.as_array().unwrap();
    assert_eq!(rows.len(), SCHEMA.len(), "number of settings");
    for (row, s) in rows.iter().zip(SCHEMA) {
        assert_eq!(row["key"].as_str().unwrap(), s.key);
        assert_eq!(row["kind"].as_str().unwrap(), s.kind.as_str(), "{}", s.key);
        assert_eq!(row["label"].as_str().unwrap(), s.label, "{}", s.key);
        assert_eq!(row["help"].as_str().unwrap(), s.help, "{}", s.key);
        let d = s.default.value();
        if d.is_number() {
            assert_eq!(row["default"].as_f64(), d.as_f64(), "{}", s.key);
        } else {
            assert_eq!(row["default"], d, "{}", s.key);
        }
        assert_eq!(row.get("min").and_then(Value::as_f64), s.min, "{}", s.key);
        assert_eq!(row.get("max").and_then(Value::as_f64), s.max, "{}", s.key);
        let choices: Vec<(String, String)> = row
            .get("choices")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|c| (c[0].as_str().unwrap().into(), c[1].as_str().unwrap().into()))
                    .collect()
            })
            .unwrap_or_default();
        let ours: Vec<(String, String)> = s
            .choices
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        assert_eq!(choices, ours, "{}", s.key);
    }
}

#[cfg(feature = "cli")]
#[test]
fn completion_scripts_match_python_edition() {
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let want = std::fs::read_to_string(parity_dir().join(format!("completion_{}.txt", shell)))
            .unwrap();
        assert_eq!(
            dictz::cli::completion_script(shell),
            want.as_str(),
            "{}",
            shell
        );
    }
}

#[cfg(feature = "cli")]
#[test]
fn cli_commands_match_python_edition() {
    let want: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(parity_dir().join("cli_commands.json")).unwrap(),
    )
    .unwrap();
    let theirs: Vec<(&str, &str)> = want
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c[0].as_str().unwrap(), c[1].as_str().unwrap()))
        .collect();
    let ours: Vec<(&str, &str)> = dictz::cli::COMMANDS.to_vec();
    assert_eq!(theirs, ours);
}

#[test]
fn shortcuts_match_python_edition() {
    let want: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(parity_dir().join("shortcuts.json")).unwrap(),
    )
    .unwrap();
    let theirs: Vec<(&str, &str)> = want
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c[0].as_str().unwrap(), c[1].as_str().unwrap()))
        .collect();
    assert_eq!(theirs, dictz::shortcuts::SHORTCUTS.to_vec());
}

#[test]
fn scan_text_and_hotkeys_match_python_edition() {
    let g: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(parity_dir().join("scan_golden.json")).unwrap(),
    )
    .unwrap();
    for case in g["clean"].as_array().unwrap() {
        let text = case[0].as_str().unwrap();
        let want = case[1].as_str();
        assert_eq!(
            dictz::scantext::clean_scan_text(text, 80).as_deref(),
            want,
            "{:?}",
            text
        );
    }
    let mut d = dictz::scantext::DoubleCopy::new(500.0);
    for case in g["double_copy_500"].as_array().unwrap() {
        let got = d.feed(case[0].as_str().unwrap(), case[1].as_f64().unwrap());
        assert_eq!(got, case[2].as_bool().unwrap(), "{:?}", case);
    }
    for case in g["hotkeys"].as_array().unwrap() {
        let text = case[0].as_str().unwrap();
        let want = &case[1];
        match dictz::hotkeys::parse_hotkey(text) {
            Ok(h) => {
                assert_eq!(want["ok"], true, "{}", text);
                let mods: Vec<&str> = want["mods"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| m.as_str().unwrap())
                    .collect();
                assert_eq!(h.mods, mods, "{}", text);
                assert_eq!(h.key, want["key"].as_str().unwrap(), "{}", text);
                assert_eq!(h.to_string(), want["text"].as_str().unwrap(), "{}", text);
            }
            Err(e) => {
                assert_eq!(want["ok"], false, "{}", text);
                assert_eq!(e, want["error"].as_str().unwrap(), "{}", text);
            }
        }
    }
    let icon = dictz::icon::app_icon_rgba(64);
    assert_eq!(
        format!("{:016x}", dictz::icon::fnv1a64(&icon)),
        g["icon_fnv1a64"].as_str().unwrap()
    );
}

#[test]
fn dictionary_identity_matches_python() {
    let g: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(parity_dir().join("dictident_golden.json")).unwrap(),
    )
    .unwrap();
    use dictz::dictident as di;
    for case in g["names"].as_array().unwrap() {
        let name = case[0].as_str().unwrap();
        assert_eq!(
            di::monogram(name),
            case[1].as_str().unwrap(),
            "monogram {:?}",
            name
        );
        assert_eq!(
            di::color(name),
            case[2].as_str().unwrap(),
            "color {:?}",
            name
        );
        let want: Vec<String> = case[3]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let got = di::languages_from_name(name)
            .map(|(a, b)| vec![a, b])
            .unwrap_or_default();
        assert_eq!(got, want, "languages {:?}", name);
    }
    for case in g["language_code"].as_array().unwrap() {
        let c = case[0].as_str().unwrap();
        assert_eq!(di::language_code(c), case[1].as_str().unwrap(), "{:?}", c);
    }
    for case in g["pair"].as_array().unwrap() {
        let (a, b, n) = (
            case[0].as_str().unwrap(),
            case[1].as_str().unwrap(),
            case[2].as_str().unwrap(),
        );
        let want: Vec<String> = case[3]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let got = di::language_pair(a, b, n)
            .map(|(x, y)| vec![x, y])
            .unwrap_or_default();
        assert_eq!(got, want, "{:?}", case);
    }
    let icon = di::bgl_icon(&parity_dir().join("sample.bgl")).unwrap_or_default();
    assert_eq!(
        format!("{:016x}", di::fnv1a64(&icon)),
        g["bgl_icon_fnv1a64"].as_str().unwrap()
    );
    let (_, ext) = di::icon_bytes(&parity_dir().join("sample.bgl")).unwrap();
    assert_eq!(ext, ".ico");
}
