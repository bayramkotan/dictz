//! Languages: ISO 639 codes and names (`languages.json`, the ISO 639-3
//! table; the same file as the Python edition's
//! `dictz/resources/languages.json`).
//!
//! [`norm`] turns whatever a catalog says — `eng`, `en`, `English`,
//! `İngilizce`, `Greek, Modern (1453-)` — into one code: the ISO 639-1 code
//! when the language has one, else the ISO 639-3 code, else ''.

use std::collections::HashMap;
use std::sync::OnceLock;

const FILE: &str = include_str!("languages.json");

const ALIASES: &[(&str, &str)] = &[
    ("ingilizce", "en"),
    ("türkçe", "tr"),
    ("turkce", "tr"),
    ("almanca", "de"),
    ("fransızca", "fr"),
    ("fransizca", "fr"),
    ("ispanyolca", "es"),
    ("italyanca", "it"),
    ("rusça", "ru"),
    ("rusca", "ru"),
    ("arapça", "ar"),
    ("arapca", "ar"),
    ("farsça", "fa"),
    ("farsca", "fa"),
    ("farsi", "fa"),
    ("osmanlıca", "ota"),
    ("osmanlica", "ota"),
    ("japonca", "ja"),
    ("çince", "zh"),
    ("cince", "zh"),
    ("korece", "ko"),
    ("azerice", "az"),
    ("kürtçe", "ku"),
    ("kurtce", "ku"),
    ("yunanca", "el"),
    ("latince", "la"),
    ("greek", "el"),
    ("modern greek", "el"),
    ("ancient greek", "grc"),
    ("chinese", "zh"),
    ("mandarin", "zh"),
    ("malay", "ms"),
    ("persian", "fa"),
    ("kurdish", "ku"),
    ("swahili", "sw"),
    ("nepali", "ne"),
    ("serbo-croatian", "sh"),
    ("norwegian", "no"),
    ("albanian", "sq"),
    ("estonian", "et"),
    ("latvian", "lv"),
    ("uzbek", "uz"),
    ("mongolian", "mn"),
    ("quechua", "qu"),
    ("pashto", "ps"),
    ("punjabi", "pa"),
    ("panjabi", "pa"),
    ("oriya", "or"),
    ("old english", "ang"),
    ("middle english", "enm"),
    ("old norse", "non"),
    ("old french", "fro"),
    ("middle french", "frm"),
    ("old high german", "goh"),
    ("middle high german", "gmh"),
    ("classical syriac", "syc"),
    ("ottoman turkish", "ota"),
    ("abkhaz", "ab"),
    ("kyrgyz", "ky"),
    ("kirghiz", "ky"),
    ("uyghur", "ug"),
    ("uighur", "ug"),
    ("tatar", "tt"),
    ("bashkir", "ba"),
    ("chuvash", "cv"),
    ("turkmen", "tk"),
    ("kazakh", "kk"),
    ("azerbaijani", "az"),
    ("crimean tatar", "crh"),
    ("gagauz", "gag"),
    ("yakut", "sah"),
    ("tuvan", "tyv"),
];

struct Table {
    by_code: HashMap<String, (String, String, String)>,
    by_name: HashMap<String, String>,
}

fn name_keys(name: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for n in name.to_lowercase().split("; ") {
        keys.push(n.to_string());
        let base = n.split(" (").next().unwrap_or(n).to_string();
        keys.push(base.clone());
        if let Some((a, b)) = base.split_once(", ") {
            keys.push(format!("{} {}", b, a));
            keys.push(a.to_string());
        }
    }
    keys
}

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        let rows: Vec<(String, String, String)> = serde_json::from_str(FILE).unwrap_or_default();
        let mut by_code = HashMap::new();
        let mut by_name: HashMap<String, String> = HashMap::new();
        for (a3, a2, name) in rows {
            let code = if a2.is_empty() {
                a3.clone()
            } else {
                a2.clone()
            };
            by_code.insert(a3.clone(), (a3.clone(), a2.clone(), name.clone()));
            if !a2.is_empty() {
                by_code.insert(a2.clone(), (a3.clone(), a2.clone(), name.clone()));
            }
            for key in name_keys(&name) {
                let take = match by_name.get(&key) {
                    None => true,
                    Some(old) => !a2.is_empty() && old.chars().count() == 3,
                };
                if take {
                    by_name.insert(key, code.clone());
                }
            }
        }
        for (k, v) in ALIASES {
            by_name.insert(k.to_string(), v.to_string());
        }
        Table { by_code, by_name }
    })
}

/// One language code for `value` ('' when unknown).
pub fn norm(value: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        return String::new();
    }
    let t = table();
    let low = v.to_lowercase().replace('_', "-");
    if let Some((a3, a2, _)) = t.by_code.get(&low) {
        return if a2.is_empty() {
            a3.clone()
        } else {
            a2.clone()
        };
    }
    let head = low.split('-').next().unwrap_or("");
    if (head.chars().count() == 2 || head.chars().count() == 3) && t.by_code.contains_key(head) {
        let (a3, a2, _) = &t.by_code[head];
        return if a2.is_empty() {
            a3.clone()
        } else {
            a2.clone()
        };
    }
    let folded = low.replace('\u{0307}', "");
    if let Some(c) = t.by_name.get(&folded) {
        return c.clone();
    }
    let base = folded.split(" (").next().unwrap_or("");
    t.by_name.get(base).cloned().unwrap_or_default()
}

/// English name of a code, without the table's notes ('' when unknown).
pub fn name(code: &str) -> String {
    match table().by_code.get(&code.to_lowercase()) {
        Some((_, _, n)) => n
            .split(" (")
            .next()
            .unwrap_or("")
            .split("; ")
            .next()
            .unwrap_or("")
            .to_string(),
        None => String::new(),
    }
}

/// `English (en)`; the code itself when the name is unknown.
pub fn label(code: &str) -> String {
    let n = name(code);
    if n.is_empty() {
        code.to_string()
    } else {
        format!("{} ({})", n, code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_names() {
        assert_eq!(norm("eng"), "en");
        assert_eq!(norm("Türkçe"), "tr");
        assert_eq!(norm("Akkadian"), "akk");
        assert_eq!(norm("pt-BR"), "pt");
        assert_eq!(norm("Ancient Greek"), "grc");
        assert_eq!(label("en"), "English (en)");
        assert_eq!(norm("xx-yy-zz"), "");
    }
}
