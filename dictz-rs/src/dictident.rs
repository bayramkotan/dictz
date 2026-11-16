//! What a dictionary looks like in the window: its icon, colour, monogram and
//! language pair.
//!
//! Same results as the Python edition (`dictz/core/dictident.py`); the parity
//! golden `tests/data/parity/dictident_golden.json` checks both.
//!
//! * **Icon**, first match wins:
//!   1. a picture next to the dictionary with the same name (`Oxford.ifo` →
//!      `Oxford.ico` / `.png` / `.bmp` / `.jpg` / `.gif`), as GoldenDict does;
//!   2. the icon stored inside a Babylon `.bgl` file;
//!   3. otherwise a generated badge: the monogram on the dictionary's colour.
//! * **Colour**: picked from the name (FNV-1a), stable everywhere.
//! * **Language pair**: the dictionary's own metadata, or else its name
//!   (`En2Tr`, `English-Turkish`, `İngilizce → Türkçe`).

use std::io::Read;
use std::path::{Path, PathBuf};

/// Picture extensions looked for next to a dictionary, in this order.
pub const ICON_EXTENSIONS: [&str; 6] = [".ico", ".png", ".bmp", ".jpg", ".jpeg", ".gif"];

const LANGS: &[(&str, &[&str])] = &[
    (
        "EN",
        &[
            "english",
            "en",
            "eng",
            "ingilizce",
            "ing",
            "englisch",
            "anglais",
        ],
    ),
    (
        "TR",
        &[
            "turkish",
            "tr",
            "tur",
            "türkçe",
            "turkce",
            "türk",
            "turk",
            "türkisch",
            "turc",
        ],
    ),
    (
        "DE",
        &[
            "german", "de", "deu", "ger", "deutsch", "almanca", "alm", "allemand",
        ],
    ),
    (
        "FR",
        &[
            "french",
            "fr",
            "fra",
            "fre",
            "français",
            "francais",
            "fransızca",
            "fransizca",
        ],
    ),
    (
        "ES",
        &["spanish", "es", "spa", "español", "espanol", "ispanyolca"],
    ),
    ("IT", &["italian", "it", "ita", "italiano", "italyanca"]),
    (
        "PT",
        &["portuguese", "pt", "por", "português", "portekizce"],
    ),
    (
        "NL",
        &[
            "dutch",
            "nl",
            "nld",
            "nederlands",
            "flemenkçe",
            "hollandaca",
        ],
    ),
    ("RU", &["russian", "ru", "rus", "русский", "rusça", "rusca"]),
    ("UK", &["ukrainian", "uk", "ukr", "українська", "ukraynaca"]),
    ("PL", &["polish", "pl", "pol", "polski", "lehçe"]),
    ("EL", &["greek", "el", "ell", "gre", "yunanca"]),
    ("LA", &["latin", "la", "lat", "latince"]),
    (
        "AR",
        &["arabic", "ar", "ara", "العربية", "arapça", "arapca"],
    ),
    (
        "FA",
        &[
            "persian",
            "fa",
            "fas",
            "per",
            "farsi",
            "farsça",
            "farsca",
            "فارسی",
        ],
    ),
    ("OTA", &["ottoman", "ota", "osmanlıca", "osmanlica"]),
    ("HE", &["hebrew", "he", "heb", "ibranice"]),
    ("ZH", &["chinese", "zh", "zho", "chi", "中文", "çince"]),
    ("JA", &["japanese", "ja", "jpn", "日本語", "japonca"]),
    ("KO", &["korean", "ko", "kor", "한국어", "korece"]),
    ("AZ", &["azerbaijani", "az", "aze", "azerice", "azərbaycan"]),
    ("KK", &["kazakh", "kk", "kaz", "kazakça"]),
    ("UZ", &["uzbek", "uz", "uzb", "özbekçe"]),
    ("KU", &["kurdish", "ku", "kur", "kürtçe"]),
    ("HI", &["hindi", "hi", "hin"]),
    ("SV", &["swedish", "sv", "swe", "isveççe"]),
    ("FI", &["finnish", "fi", "fin", "fince"]),
    ("HU", &["hungarian", "hu", "hun", "macarca"]),
    ("CS", &["czech", "cs", "ces", "cze", "çekçe"]),
    ("RO", &["romanian", "ro", "ron", "rum", "romence"]),
    ("BG", &["bulgarian", "bg", "bul", "bulgarca"]),
    ("SR", &["serbian", "sr", "srp", "sırpça"]),
    ("EO", &["esperanto", "eo", "epo"]),
];

const FILLER: &[&str] = &[
    "dictionary",
    "dict",
    "dic",
    "sözlük",
    "sözlüğü",
    "sozluk",
    "sozlugu",
    "lexicon",
    "glossary",
    "the",
    "of",
    "and",
    "to",
    "for",
    "ve",
    "v",
    "big",
    "full",
];

const SEPARATORS: &[&str] = &[
    "-", "–", "—", "2", "→", "->", ">", "/", "_", "to", "<>", "↔", "|", "‑",
];

/// Lower case, without the dot that `İ` lower-cased leaves behind.
pub fn fold(s: &str) -> String {
    s.to_lowercase().replace('\u{307}', "")
}

/// `English` / `en` / `Türkçe` → `EN` / `TR`; empty when unknown.
pub fn language_code(name: &str) -> &'static str {
    let f = fold(name.trim());
    for (code, words) in LANGS {
        if words.iter().any(|w| fold(w) == f) {
            return code;
        }
    }
    ""
}

/// (gap before the word, word) for every run of letters.
fn tokens(name: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (mut gap, mut word) = (String::new(), String::new());
    for ch in name.chars() {
        if ch.is_alphabetic() {
            word.push(ch);
        } else {
            if !word.is_empty() {
                out.push((std::mem::take(&mut gap), std::mem::take(&mut word)));
            }
            gap.push(ch);
        }
    }
    if !word.is_empty() {
        out.push((gap, word));
    }
    out
}

/// `Adil Hindistan En2Tr` → (`EN`, `TR`); `None` when the name has no pair.
pub fn languages_from_name(name: &str) -> Option<(String, String)> {
    let toks = tokens(name);
    let mut i = 0;
    while i + 1 < toks.len() {
        let a = &toks[i].1;
        let mut j = i + 1;
        let mut gap = toks[j].0.clone();
        if fold(&toks[j].1) == "to" && j + 1 < toks.len() {
            gap = format!("{}to{}", gap, toks[j + 1].0);
            j += 1;
        }
        let sep: String = gap.split_whitespace().collect();
        if SEPARATORS.contains(&sep.as_str()) || SEPARATORS.contains(&fold(&sep).as_str()) {
            let (src, dst) = (language_code(a), language_code(&toks[j].1));
            if !src.is_empty() && !dst.is_empty() {
                return Some((src.to_string(), dst.to_string()));
            }
        }
        i += 1;
    }
    None
}

fn upper3(s: &str) -> String {
    s.trim().chars().take(3).collect::<String>().to_uppercase()
}

/// The pair to show: the dictionary's own languages first, then its name.
pub fn language_pair(source: &str, target: &str, name: &str) -> Option<(String, String)> {
    if !source.trim().is_empty() && !target.trim().is_empty() {
        let src = match language_code(source) {
            "" => upper3(source),
            c => c.to_string(),
        };
        let dst = match language_code(target) {
            "" => upper3(target),
            c => c.to_string(),
        };
        return Some((src, dst));
    }
    languages_from_name(name)
}

/// One or two letters for the generated badge.
pub fn monogram(name: &str) -> String {
    let words: Vec<String> = tokens(name).into_iter().map(|t| t.1).collect();
    let meaningful: Vec<&String> = words
        .iter()
        .filter(|w| !FILLER.contains(&fold(w).as_str()) && language_code(w).is_empty())
        .collect();
    let pick: Vec<&String> = if meaningful.is_empty() {
        words.iter().collect()
    } else {
        meaningful
    };
    let first = |w: &str| w.chars().next().map(|c| c.to_string()).unwrap_or_default();
    match pick.len() {
        0 => "?".to_string(),
        1 => {
            let mut cs = pick[0].chars();
            let a = cs.next().map(|c| c.to_string()).unwrap_or_default();
            let b = cs.next().map(|c| c.to_string()).unwrap_or_default();
            format!("{}{}", a.to_uppercase(), b.to_lowercase())
        }
        _ => format!("{}{}", first(pick[0]), first(pick[1])).to_uppercase(),
    }
}

/// FNV-1a 64-bit.
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

fn hsl_rgb(h: f64, s: f64, l: f64) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r, g, b) = match (hp as i64).rem_euclid(6) {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f64| ((v + m) * 255.0 + 0.5) as u8;
    [to(r), to(g), to(b)]
}

/// The dictionary's own colour as RGB, stable for a name.
pub fn color_rgb(name: &str) -> [u8; 3] {
    let h = fnv1a64(name.as_bytes());
    let hue = (h % 360) as f64;
    let sat = 0.50 + ((h >> 16) % 20) as f64 / 100.0;
    let light = 0.40 + ((h >> 32) % 10) as f64 / 100.0;
    hsl_rgb(hue, sat, light)
}

/// The dictionary's own colour as `#rrggbb`.
pub fn color(name: &str) -> String {
    let [r, g, b] = color_rgb(name);
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn stem(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let low = name.to_lowercase();
    for ext in [".dsl.dz", ".dsl", ".ifo", ".bgl", ".dz", ".gz"] {
        if low.ends_with(ext) && name.len() >= ext.len() {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A picture next to the dictionary file with the same name, if any.
pub fn icon_file(path: &Path) -> Option<PathBuf> {
    let stem = stem(path);
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut siblings = std::collections::HashMap::new();
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            siblings.insert(e.file_name().to_string_lossy().to_lowercase(), e.path());
        }
    }
    ICON_EXTENSIONS.iter().find_map(|ext| {
        siblings
            .get(&format!("{}{}", stem, ext).to_lowercase())
            .cloned()
    })
}

/// The icon stored in a Babylon `.bgl` file (info block 0x0B).
pub fn bgl_icon(path: &Path) -> Option<Vec<u8>> {
    let raw = std::fs::read(path).ok()?;
    if raw.len() < 6 || !(raw[..4] == [0x12, 0x34, 0, 1] || raw[..4] == [0x12, 0x34, 0, 2]) {
        return None;
    }
    let gz = ((raw[4] as usize) << 8) | raw[5] as usize;
    let data = raw.get(gz..)?;
    if data.len() < 10 || data[..3] != [0x1f, 0x8b, 8] {
        return None;
    }
    let flags = data[3];
    let mut pos = 10usize;
    if flags & 0x04 != 0 {
        let x = data.get(pos..pos + 2)?;
        pos += 2 + u16::from_le_bytes([x[0], x[1]]) as usize;
    }
    for bit in [0x08u8, 0x10] {
        if flags & bit != 0 {
            while pos < data.len() && data[pos] != 0 {
                pos += 1;
            }
            pos += 1;
        }
    }
    if flags & 0x02 != 0 {
        pos += 2;
    }
    let mut dec = flate2::read::DeflateDecoder::new(data.get(pos..)?);
    let mut buf: Vec<u8> = Vec::new();
    let mut done = false;
    // inflate only as far as needed: the info blocks are at the start
    let mut need = |buf: &mut Vec<u8>, n: usize| -> bool {
        let mut chunk = [0u8; 65536];
        while buf.len() < n && !done {
            match dec.read(&mut chunk) {
                Ok(0) | Err(_) => done = true,
                Ok(k) => buf.extend_from_slice(&chunk[..k]),
            }
        }
        buf.len() >= n
    };
    let mut pos = 0usize;
    while need(&mut buf, pos + 1) {
        let first = buf[pos];
        pos += 1;
        let kind = first & 0xF;
        let mut len = (first >> 4) as usize;
        if len < 4 {
            let n = len + 1;
            if !need(&mut buf, pos + n) {
                break;
            }
            len = buf[pos..pos + n]
                .iter()
                .fold(0usize, |a, &b| (a << 8) | b as usize);
            pos += n;
        } else {
            len -= 4;
        }
        if matches!(kind, 1 | 7 | 10 | 11) {
            break;
        }
        need(&mut buf, pos + len);
        let end = (pos + len).min(buf.len());
        let block = &buf[pos..end];
        pos = end;
        if kind == 3 && block.len() > 2 && ((block[0] as u16) << 8 | block[1] as u16) == 0x0B {
            return Some(block[2..].to_vec());
        }
        if buf.len() > (32 << 20) {
            break;
        }
    }
    None
}

/// (file contents, lower-case extension) of the dictionary's real icon.
pub fn icon_bytes(path: &Path) -> Option<(Vec<u8>, String)> {
    if let Some(p) = icon_file(path) {
        if let Ok(b) = std::fs::read(&p) {
            let ext = p
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
                .unwrap_or_default();
            return Some((b, ext));
        }
    }
    if path.to_string_lossy().to_lowercase().ends_with(".bgl") {
        if let Some(b) = bgl_icon(path).filter(|b| !b.is_empty()) {
            let ext = if b.starts_with(b"\x89PNG\r\n\x1a\n") {
                ".png"
            } else {
                ".ico"
            };
            return Some((b, ext.to_string()));
        }
    }
    None
}

/// `stardict` → `StarDict`, `dsl` → `DSL`, `babylon` → `BGL`.
pub fn format_label(fmt: &str) -> String {
    match fmt.to_lowercase().as_str() {
        "stardict" => "StarDict".into(),
        "dsl" => "DSL".into(),
        "babylon" => "BGL".into(),
        _ => fmt.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_next_to_dictionary() {
        let d = std::env::temp_dir().join(format!("dictz-ident-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("Lingvo.dsl.dz"), b"x").unwrap();
        assert!(icon_file(&d.join("Lingvo.dsl.dz")).is_none());
        std::fs::write(d.join("Lingvo.PNG"), b"png").unwrap();
        std::fs::write(d.join("Lingvo.bmp"), b"bmp").unwrap();
        let (b, ext) = icon_bytes(&d.join("Lingvo.dsl.dz")).unwrap();
        assert_eq!((b.as_slice(), ext.as_str()), (&b"png"[..], ".png"));
        std::fs::write(d.join("Lingvo.ico"), b"ico").unwrap();
        assert_eq!(icon_bytes(&d.join("Lingvo.dsl.dz")).unwrap().1, ".ico");
        std::fs::remove_dir_all(&d).ok();
    }
}
