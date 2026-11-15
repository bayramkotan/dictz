//! Small text helpers shared by the parsers.
//!
//! Several of these reproduce the exact behaviour of the Python standard
//! library / PyGlossary so that the Rust and Python editions produce the same
//! headwords and definitions for the same file.

use encoding_rs::Encoding;

/// Python's `xml.sax.saxutils.escape`: `&`, `<`, `>`.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Inverse of [`xml_escape`].
pub fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Python's `xml.sax.saxutils.quoteattr`.
pub fn quoteattr(s: &str) -> String {
    let mut data = xml_escape(s)
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
        .replace('\t', "&#9;");
    if data.contains('"') {
        if data.contains('\'') {
            data = data.replace('"', "&quot;");
            format!("\"{}\"", data)
        } else {
            format!("'{}'", data)
        }
    } else {
        format!("\"{}\"", data)
    }
}

/// Python's `str.isspace` for one character.
pub fn py_is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python's `str.strip()`.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(py_is_space)
}

/// Python's `str.lstrip()`.
pub fn py_lstrip(s: &str) -> &str {
    s.trim_start_matches(py_is_space)
}

/// Python's `str.rstrip()`.
pub fn py_rstrip(s: &str) -> &str {
    s.trim_end_matches(py_is_space)
}

/// Map a Python codec name (as used by PyGlossary) to an `encoding_rs` encoding.
pub fn encoding_for(name: &str) -> &'static Encoding {
    let n = name.to_ascii_lowercase().replace('_', "-");
    match n.as_str() {
        "utf-8" | "utf8" => encoding_rs::UTF_8,
        "cp1250" => encoding_rs::WINDOWS_1250,
        "cp1251" => encoding_rs::WINDOWS_1251,
        "cp1252" | "latin1" | "latin-1" => encoding_rs::WINDOWS_1252,
        "cp1253" => encoding_rs::WINDOWS_1253,
        "cp1254" => encoding_rs::WINDOWS_1254,
        "cp1255" => encoding_rs::WINDOWS_1255,
        "cp1256" => encoding_rs::WINDOWS_1256,
        "cp1257" => encoding_rs::WINDOWS_1257,
        "cp874" => encoding_rs::WINDOWS_874,
        "cp932" | "shift-jis" => encoding_rs::SHIFT_JIS,
        "cp936" | "gbk" | "gb2312" => encoding_rs::GBK,
        "gb18030" => encoding_rs::GB18030,
        "cp949" | "euc-kr" => encoding_rs::EUC_KR,
        "cp950" | "big5" => encoding_rs::BIG5,
        "utf-16" | "utf-16-le" | "utf16" => encoding_rs::UTF_16LE,
        "utf-16-be" => encoding_rs::UTF_16BE,
        other => Encoding::for_label(other.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252),
    }
}

/// Decode with Python's `errors="replace"` behaviour (U+FFFD for bad bytes).
pub fn decode_replace(bytes: &[u8], encoding: &str) -> String {
    let (text, _) = encoding_for(encoding).decode_without_bom_handling(bytes);
    text.into_owned()
}

/// Decode with Python's `errors="ignore"` behaviour (bad bytes dropped).
pub fn decode_ignore(bytes: &[u8], encoding: &str) -> String {
    let (text, had_errors) = encoding_for(encoding).decode_without_bom_handling(bytes);
    if had_errors {
        text.chars().filter(|&c| c != '\u{FFFD}').collect()
    } else {
        text.into_owned()
    }
}

/// Look up an HTML entity name (case-sensitive) in PyGlossary's table.
pub fn entity_codepoint(name: &str) -> Option<u32> {
    NAMED_ENTITIES
        .binary_search_by(|(k, _)| (*k).cmp(name))
        .ok()
        .map(|i| NAMED_ENTITIES[i].1)
}

/// PyGlossary's Persian cleanup (`persian_utils.faEditStr`), applied to
/// headwords and definitions of dictionaries whose source or target language
/// is Persian.
pub fn fa_edit(s: &str) -> String {
    let s = s
        .replace('\u{064A}', "\u{06CC}")
        .replace('\u{0643}', "\u{06A9}")
        .replace('\u{06C2}', "\u{0647}\u{0654}")
        .replace('\u{06C1}', "\u{0647}");
    let ch = "\u{060C}";
    let s = s.replace(&format!(" {}", ch), ch);
    let s = s.replace(ch, &format!("{} ", ch));
    let s = s.replace(&format!("{}  ", ch), &format!("{} ", ch));
    s.strip_suffix(' ').map(str::to_string).unwrap_or(s)
}

/// `true` when a language name/code means Persian.
pub fn is_persian(lang: &str) -> bool {
    matches!(
        lang.trim().to_ascii_lowercase().as_str(),
        "persian" | "farsi" | "fa" | "fas" | "per"
    )
}

/// PyGlossary's default entry filters, as applied when the Python edition
/// reads DSL / Babylon files: trim words and definition, drop `\r`, drop
/// trailing `<br>`, skip empty headwords and empty definitions, remove empty
/// and duplicate alternate words. Returns `None` for a skipped entry.
pub fn pyglossary_entry_filters(
    words: Vec<String>,
    defi: String,
    persian: bool,
) -> Option<(Vec<String>, String)> {
    let mut words: Vec<String> = words
        .iter()
        .map(|w| py_strip(w).replace('\r', ""))
        .collect();
    let mut defi = py_strip(&defi).to_string();
    while defi.ends_with("<BR>") || defi.ends_with("<br>") {
        defi.truncate(defi.len() - 4);
    }
    defi = defi.replace('\r', "");
    if words.iter().all(|w| w.is_empty()) {
        return None;
    }
    if persian {
        words = words.iter().map(|w| fa_edit(w)).collect();
        defi = fa_edit(&defi);
    }
    if defi.is_empty() {
        return None;
    }
    if words.len() > 1 {
        let mut seen = std::collections::HashSet::new();
        words.retain(|w| !w.is_empty() && seen.insert(w.clone()));
    }
    if words.is_empty() {
        return None;
    }
    Some((words, defi))
}

/// HTML entity names known to PyGlossary (`pyglossary.html_utils.name2codepoint`), sorted.
pub(crate) static NAMED_ENTITIES: &[(&str, u32)] = &[
    ("AElig", 0x00C6),
    ("Aacute", 0x00C1),
    ("Acirc", 0x00C2),
    ("Agrave", 0x00C0),
    ("Alpha", 0x0391),
    ("Aring", 0x00C5),
    ("Atilde", 0x00C3),
    ("Auml", 0x00C4),
    ("Beta", 0x0392),
    ("Ccedil", 0x00C7),
    ("Chi", 0x03A7),
    ("Dagger", 0x2021),
    ("Delta", 0x0394),
    ("ETH", 0x00D0),
    ("Eacute", 0x00C9),
    ("Ecirc", 0x00CA),
    ("Egrave", 0x00C8),
    ("Epsilon", 0x0395),
    ("Eta", 0x0397),
    ("Euml", 0x00CB),
    ("Gamma", 0x0393),
    ("Iacute", 0x00CD),
    ("Icirc", 0x00CE),
    ("Igrave", 0x00CC),
    ("Iota", 0x0399),
    ("Iuml", 0x00CF),
    ("Kappa", 0x039A),
    ("Lambda", 0x039B),
    ("Mu", 0x039C),
    ("Ntilde", 0x00D1),
    ("Nu", 0x039D),
    ("OElig", 0x0152),
    ("Oacute", 0x00D3),
    ("Ocirc", 0x00D4),
    ("Ograve", 0x00D2),
    ("Omega", 0x03A9),
    ("Omicron", 0x039F),
    ("Oslash", 0x00D8),
    ("Otilde", 0x00D5),
    ("Ouml", 0x00D6),
    ("Phi", 0x03A6),
    ("Pi", 0x03A0),
    ("Prime", 0x2033),
    ("Psi", 0x03A8),
    ("Rho", 0x03A1),
    ("Scaron", 0x0160),
    ("Sigma", 0x03A3),
    ("THORN", 0x00DE),
    ("Tau", 0x03A4),
    ("Theta", 0x0398),
    ("Uacute", 0x00DA),
    ("Ucirc", 0x00DB),
    ("Ugrave", 0x00D9),
    ("Upsilon", 0x03A5),
    ("Uuml", 0x00DC),
    ("Xi", 0x039E),
    ("Yacute", 0x00DD),
    ("Yuml", 0x0178),
    ("Zeta", 0x0396),
    ("aacute", 0x00E1),
    ("acirc", 0x00E2),
    ("acute", 0x00B4),
    ("aelig", 0x00E6),
    ("agrave", 0x00E0),
    ("alefsym", 0x2135),
    ("alpha", 0x03B1),
    ("amp", 0x0026),
    ("and", 0x2227),
    ("ang", 0x2220),
    ("aring", 0x00E5),
    ("asymp", 0x2248),
    ("atilde", 0x00E3),
    ("auml", 0x00E4),
    ("bdquo", 0x201E),
    ("beta", 0x03B2),
    ("brvbar", 0x00A6),
    ("bull", 0x2022),
    ("cap", 0x2229),
    ("ccedil", 0x00E7),
    ("cedil", 0x00B8),
    ("cent", 0x00A2),
    ("chi", 0x03C7),
    ("circ", 0x02C6),
    ("clubs", 0x2663),
    ("cong", 0x2245),
    ("copy", 0x00A9),
    ("crarr", 0x21B5),
    ("cup", 0x222A),
    ("curren", 0x00A4),
    ("dArr", 0x21D3),
    ("dagger", 0x2020),
    ("darr", 0x2193),
    ("deg", 0x00B0),
    ("delta", 0x03B4),
    ("diams", 0x2666),
    ("divide", 0x00F7),
    ("eacute", 0x00E9),
    ("ecirc", 0x00EA),
    ("egrave", 0x00E8),
    ("empty", 0x2205),
    ("emsp", 0x2003),
    ("ensp", 0x2002),
    ("epsilon", 0x03B5),
    ("equiv", 0x2261),
    ("eta", 0x03B7),
    ("eth", 0x00F0),
    ("etilde", 0x1EBD),
    ("euml", 0x00EB),
    ("euro", 0x20AC),
    ("exist", 0x2203),
    ("fnof", 0x0192),
    ("forall", 0x2200),
    ("frac12", 0x00BD),
    ("frac13", 0x2153),
    ("frac14", 0x00BC),
    ("frac23", 0x2154),
    ("frac34", 0x00BE),
    ("frasl", 0x2044),
    ("gamma", 0x03B3),
    ("ge", 0x2265),
    ("gt", 0x003E),
    ("hArr", 0x21D4),
    ("harr", 0x2194),
    ("hearts", 0x2665),
    ("hellip", 0x2026),
    ("iacute", 0x00ED),
    ("icirc", 0x00EE),
    ("iexcl", 0x00A1),
    ("igrave", 0x00EC),
    ("image", 0x2111),
    ("infin", 0x221E),
    ("int", 0x222B),
    ("iota", 0x03B9),
    ("iquest", 0x00BF),
    ("isin", 0x2208),
    ("itilde", 0x0129),
    ("iuml", 0x00EF),
    ("kappa", 0x03BA),
    ("lArr", 0x21D0),
    ("lambda", 0x03BB),
    ("lang", 0x2329),
    ("laquo", 0x00AB),
    ("larr", 0x2190),
    ("lceil", 0x2308),
    ("ldash", 0x2013),
    ("ldquo", 0x201C),
    ("le", 0x2264),
    ("lfloor", 0x230A),
    ("lowast", 0x2217),
    ("loz", 0x25CA),
    ("lrm", 0x200E),
    ("lsaquo", 0x2039),
    ("lsquo", 0x2018),
    ("lt", 0x003C),
    ("macr", 0x00AF),
    ("mdash", 0x2014),
    ("micro", 0x00B5),
    ("middot", 0x00B7),
    ("minus", 0x2212),
    ("mu", 0x03BC),
    ("nabla", 0x2207),
    ("nbsp", 0x00A0),
    ("ndash", 0x2013),
    ("ne", 0x2260),
    ("ni", 0x220B),
    ("not", 0x00AC),
    ("notin", 0x2209),
    ("nsub", 0x2284),
    ("ntilde", 0x00F1),
    ("nu", 0x03BD),
    ("oacute", 0x00F3),
    ("ocirc", 0x00F4),
    ("oelig", 0x0153),
    ("ograve", 0x00F2),
    ("oline", 0x203E),
    ("omega", 0x03C9),
    ("omicron", 0x03BF),
    ("oplus", 0x2295),
    ("or", 0x2228),
    ("ordf", 0x00AA),
    ("ordm", 0x00BA),
    ("oslash", 0x00F8),
    ("otilde", 0x00F5),
    ("otimes", 0x2297),
    ("ouml", 0x00F6),
    ("para", 0x00B6),
    ("part", 0x2202),
    ("permil", 0x2030),
    ("perp", 0x22A5),
    ("phi", 0x03C6),
    ("pi", 0x03C0),
    ("piv", 0x03D6),
    ("plusmn", 0x00B1),
    ("pound", 0x00A3),
    ("prime", 0x2032),
    ("prod", 0x220F),
    ("prop", 0x221D),
    ("psi", 0x03C8),
    ("quot", 0x0022),
    ("rArr", 0x21D2),
    ("radic", 0x221A),
    ("rang", 0x232A),
    ("raquo", 0x00BB),
    ("rarr", 0x2192),
    ("rceil", 0x2309),
    ("rdquo", 0x201D),
    ("real", 0x211C),
    ("reg", 0x00AE),
    ("rfloor", 0x230B),
    ("rho", 0x03C1),
    ("rlm", 0x200F),
    ("rsaquo", 0x203A),
    ("rsquo", 0x2019),
    ("sbquo", 0x201A),
    ("scaron", 0x0161),
    ("sdot", 0x22C5),
    ("sect", 0x00A7),
    ("shy", 0x00AD),
    ("sigma", 0x03C3),
    ("sigmaf", 0x03C2),
    ("sim", 0x223C),
    ("spades", 0x2660),
    ("sub", 0x2282),
    ("sube", 0x2286),
    ("sum", 0x2211),
    ("sup", 0x2283),
    ("sup1", 0x00B9),
    ("sup2", 0x00B2),
    ("sup3", 0x00B3),
    ("supe", 0x2287),
    ("szlig", 0x00DF),
    ("tau", 0x03C4),
    ("there4", 0x2234),
    ("theta", 0x03B8),
    ("thetasym", 0x03D1),
    ("thinsp", 0x2009),
    ("thorn", 0x00FE),
    ("tilde", 0x02DC),
    ("times", 0x00D7),
    ("trade", 0x2122),
    ("uArr", 0x21D1),
    ("uacute", 0x00FA),
    ("uarr", 0x2191),
    ("ucirc", 0x00FB),
    ("ugrave", 0x00F9),
    ("uml", 0x00A8),
    ("upsih", 0x03D2),
    ("upsilon", 0x03C5),
    ("uring", 0x016F),
    ("utilde", 0x0169),
    ("uuml", 0x00FC),
    ("weierp", 0x2118),
    ("wring", 0x1E98),
    ("xfrac13", 0x2153),
    ("xi", 0x03BE),
    ("yacute", 0x00FD),
    ("ycirc", 0x0177),
    ("yen", 0x00A5),
    ("ygrave", 0x1EF3),
    ("yring", 0x1E99),
    ("ytilde", 0x1EF9),
    ("yuml", 0x00FF),
    ("zeta", 0x03B6),
    ("zwj", 0x200D),
    ("zwnj", 0x200C),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_roundtrip() {
        assert_eq!(xml_escape("a<b>&c"), "a&lt;b&gt;&amp;c");
        assert_eq!(xml_unescape("a&lt;b&gt;&amp;amp;"), "a<b>&amp;");
    }

    #[test]
    fn quoteattr_matches_python() {
        assert_eq!(quoteattr("bword://x"), "\"bword://x\"");
        assert_eq!(quoteattr("a\"b"), "'a\"b'");
        assert_eq!(quoteattr("a\"b'c"), "\"a&quot;b'c\"");
        assert_eq!(quoteattr("a&b"), "\"a&amp;b\"");
    }

    #[test]
    fn python_strip() {
        assert_eq!(py_strip("\u{a0} x \u{1f}\n"), "x");
        assert_eq!(py_lstrip("  x "), "x ");
    }

    #[test]
    fn entities_sorted_and_found() {
        assert!(NAMED_ENTITIES.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(entity_codepoint("amp"), Some(0x26));
        assert_eq!(entity_codepoint("eacute"), Some(0xE9));
        assert_eq!(entity_codepoint("nope"), None);
    }

    #[test]
    fn decode_modes() {
        assert_eq!(decode_replace(b"\xfcz\xfcm", "cp1254"), "üzüm");
        assert_eq!(decode_ignore(b"a\xffb", "utf-8"), "ab");
        assert_eq!(decode_replace(b"a\xffb", "utf-8"), "a\u{FFFD}b");
    }

    #[test]
    fn filters() {
        let r = pyglossary_entry_filters(
            vec![" a ".into(), "a".into(), "".into()],
            " x<br><br>".into(),
            false,
        );
        assert_eq!(r, Some((vec!["a".to_string()], "x".to_string())));
        assert!(pyglossary_entry_filters(vec!["a".into()], "  ".into(), false).is_none());
    }
}
