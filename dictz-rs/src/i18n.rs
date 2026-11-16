//! Interface languages — same catalogue and rules as the Python edition
//! (`dictz/core/i18n.py`).
//!
//! The interface is written in Turkish; [`tr`] returns a text in the chosen
//! language. The catalogue (`src/i18n/catalog.json`, generated from
//! `dictz/resources/i18n/*.json` by `tests/data/parity/make_samples.py`) maps
//! the Turkish text to its translation; a missing entry falls back to English,
//! then to the Turkish text. `{name}` placeholders are filled by [`tr_args`].
//! An `&` (Qt menu mnemonic) is ignored when looking up and put back in front.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// (code, name in its own language, English name, right-to-left).
pub const LANGUAGES: [(&str, &str, &str, bool); 25] = [
    ("tr", "Türkçe", "Turkish", false),
    ("en", "English", "English", false),
    ("az", "Azərbaycan türkcəsi", "Azerbaijani Turkish", false),
    ("de", "Deutsch", "German", false),
    ("fr", "Français", "French", false),
    ("es", "Español", "Spanish", false),
    ("it", "Italiano", "Italian", false),
    ("pt", "Português", "Portuguese", false),
    ("nl", "Nederlands", "Dutch", false),
    ("pl", "Polski", "Polish", false),
    ("ru", "Русский", "Russian", false),
    ("uk", "Українська", "Ukrainian", false),
    ("ar", "العربية", "Arabic", true),
    ("fa", "فارسی", "Persian", true),
    ("ur", "اردو", "Urdu", true),
    ("kk", "Қазақша", "Kazakh", false),
    ("ky", "Кыргызча", "Kyrgyz", false),
    ("uz", "Oʻzbekcha", "Uzbek", false),
    ("tk", "Türkmençe", "Turkmen", false),
    ("zh", "中文", "Chinese", false),
    ("ja", "日本語", "Japanese", false),
    ("ko", "한국어", "Korean", false),
    ("id", "Bahasa Indonesia", "Indonesian", false),
    ("ms", "Bahasa Melayu", "Malay", false),
    ("hi", "हिन्दी", "Hindi", false),
];

type Catalogs = HashMap<String, HashMap<String, String>>;

fn catalogs() -> &'static Catalogs {
    static C: OnceLock<Catalogs> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str(include_str!("i18n/catalog.json")).unwrap_or_default())
}

fn current_lock() -> &'static RwLock<String> {
    static L: OnceLock<RwLock<String>> = OnceLock::new();
    L.get_or_init(|| RwLock::new("tr".to_string()))
}

/// `tr_TR.UTF-8` / `Turkish_Türkiye` / `tr-TR` → `tr` (empty when unknown).
pub fn code_of(name: &str) -> String {
    let head = name
        .split(':')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .replace('-', "_");
    let head = head.split('_').next().unwrap_or("").trim().to_lowercase();
    if LANGUAGES.iter().any(|l| l.0 == head) {
        return head;
    }
    LANGUAGES
        .iter()
        .find(|l| l.2.to_lowercase() == head)
        .map(|l| l.0.to_string())
        .unwrap_or_default()
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn os_locale() -> String {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buf = [0u16; 85];
    // SAFETY: the buffer is LOCALE_NAME_MAX_LENGTH (85) wide characters long.
    let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if n > 1 {
        String::from_utf16_lossy(&buf[..(n - 1) as usize])
    } else {
        String::new()
    }
}

#[cfg(target_os = "macos")]
fn os_locale() -> String {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLocale"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn os_locale() -> String {
    String::new()
}

/// The system's language code if dictz has it, otherwise `en`.
pub fn system_language() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() && v != "C" && v != "POSIX" && v != "C.UTF-8" {
                let c = code_of(&v);
                return if c.is_empty() { "en".into() } else { c };
            }
        }
    }
    let c = code_of(&os_locale());
    if c.is_empty() {
        "en".into()
    } else {
        c
    }
}

/// `auto` → the system language; unknown codes → the system language.
pub fn resolve(setting: &str) -> String {
    if LANGUAGES.iter().any(|l| l.0 == setting) {
        setting.to_string()
    } else {
        system_language()
    }
}

/// Switch the interface language; returns the code in use.
pub fn set_language(setting: &str) -> String {
    let code = resolve(setting);
    if let Ok(mut w) = current_lock().write() {
        *w = code.clone();
    }
    code
}

/// The language in use.
pub fn current() -> String {
    current_lock()
        .read()
        .map(|s| s.clone())
        .unwrap_or_else(|_| "tr".into())
}

/// `true` for Arabic, Persian and Urdu.
pub fn is_rtl(code: &str) -> bool {
    LANGUAGES.iter().any(|l| l.0 == code && l.3)
}

/// The language's own name (`Deutsch`).
pub fn native_name(code: &str) -> &'static str {
    LANGUAGES
        .iter()
        .find(|l| l.0 == code)
        .map(|l| l.1)
        .unwrap_or("")
}

fn lookup(lang: &str, key: &str) -> Option<&'static str> {
    let c = catalogs();
    c.get(lang)
        .and_then(|m| m.get(key))
        .or_else(|| {
            if lang == "tr" || lang == "en" {
                None
            } else {
                c.get("en").and_then(|m| m.get(key))
            }
        })
        .map(|s| s.as_str())
}

/// `text` (Turkish) in language `lang`.
pub fn tr_in(lang: &str, text: &str) -> String {
    if lang == "tr" {
        return text.to_string();
    }
    let mnemonic = text.contains('&')
        && !text.contains("&&")
        && !text.contains("&lt;")
        && !text.contains("&gt;");
    if mnemonic {
        let key = text.replace('&', "");
        return match lookup(lang, &key) {
            Some(s) => format!("&{}", s),
            None => text.to_string(),
        };
    }
    lookup(lang, text).unwrap_or(text).to_string()
}

/// `text` (Turkish) in the interface language.
///
/// With the `gui` feature, Arabic, Persian and Urdu come back shaped and in
/// visual order, because egui draws glyphs left to right without shaping.
pub fn tr(text: &str) -> String {
    let lang = current();
    let out = tr_in(&lang, text);
    #[cfg(feature = "gui")]
    if is_rtl(&lang) {
        return visual(&out);
    }
    out
}

/// Arabic-script text shaped (presentation forms) and reordered for a
/// left-to-right renderer, line by line.
#[cfg(feature = "gui")]
pub fn visual(text: &str) -> String {
    use unicode_bidi::{BidiInfo, Level};
    let shaped = ar_reshaper::reshape_line(text);
    shaped
        .split('\n')
        .map(|line| {
            let info = BidiInfo::new(line, Some(Level::rtl()));
            match info.paragraphs.first() {
                Some(para) => info.reorder_line(para, para.range.clone()).into_owned(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`tr`] with `{name}` placeholders filled in.
pub fn tr_args(text: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let lang = current();
    let mut out = tr_in(&lang, text);
    for (k, v) in args {
        out = out.replace(&format!("{{{}}}", k), &v.to_string());
    }
    #[cfg(feature = "gui")]
    if is_rtl(&lang) {
        return visual(&out);
    }
    out
}

/// All Turkish keys of the catalogue (for tests).
pub fn keys(lang: &str) -> Vec<String> {
    catalogs()
        .get(lang)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_fallbacks() {
        assert_eq!(code_of("tr_TR.UTF-8"), "tr");
        assert_eq!(code_of("Turkish_Türkiye"), "tr");
        assert_eq!(code_of("de-DE"), "de");
        assert_eq!(code_of("xx_YY"), "");
        assert_eq!(tr_in("tr", "&Dosya"), "&Dosya");
        assert_eq!(tr_in("en", "&Dosya"), "&File");
        assert_eq!(tr_in("en", "Bilinmeyen metin"), "Bilinmeyen metin");
        assert!(is_rtl("ar") && !is_rtl("tr"));
    }
}
