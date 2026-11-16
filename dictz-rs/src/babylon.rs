//! Babylon (`.bgl`) parser.
//!
//! A native port of PyGlossary's Babylon reader
//! (`pyglossary/plugins/babylon_bgl`), which the Python edition uses:
//! the same block layout, charset detection, `<charset>` tags, entity
//! handling, definition trailing fields (part of speech, title,
//! transcription) and entry filters, so both editions produce the same
//! headwords and definitions.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::text::{
    decode_ignore, decode_replace, entity_codepoint, is_persian, py_lstrip, py_strip,
    pyglossary_entry_filters, xml_escape,
};
use crate::{Dictionary, DictionaryMetadata, DictzError, WordTable};

/// A Babylon dictionary.
pub struct BabylonDictionary {
    path: PathBuf,
    meta: DictionaryMetadata,
    words: WordTable,
    defs: Vec<String>,
    loaded: bool,
}

impl BabylonDictionary {
    /// Prepare to read `path`.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let meta = DictionaryMetadata {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            file_path: path.clone(),
            format: "babylon".into(),
            ..Default::default()
        };
        Self {
            path,
            meta,
            words: WordTable::new(),
            defs: Vec::new(),
            loaded: false,
        }
    }
}

impl Dictionary for BabylonDictionary {
    fn load(&mut self) -> Result<(), DictzError> {
        if self.loaded {
            return Ok(());
        }
        let raw = std::fs::read(&self.path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                DictzError::NotFound(self.path.display().to_string())
            } else {
                DictzError::Io(e)
            }
        })?;
        let bgl = parse_bgl(&raw)
            .map_err(|m| DictzError::Malformed(format!("{}: {}", self.path.display(), m)))?;
        if !bgl.title.is_empty() {
            self.meta.name = bgl.title.clone();
        }
        self.meta.author = bgl.author.clone();
        self.meta.description = bgl.description.clone();
        self.meta.source_language = bgl.source_lang.clone();
        self.meta.target_language = bgl.target_lang.clone();
        for (words, defi) in bgl.entries {
            for w in words {
                let w = py_strip(&w);
                if w.is_empty() {
                    continue;
                }
                if let Some(slot) = self.words.insert(w) {
                    debug_assert_eq!(slot, self.defs.len());
                    self.defs.push(defi.clone());
                }
            }
        }
        self.words.finish();
        self.meta.word_count = self.words.len();
        self.loaded = true;
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }

    fn metadata(&self) -> &DictionaryMetadata {
        &self.meta
    }

    fn metadata_mut(&mut self) -> &mut DictionaryMetadata {
        &mut self.meta
    }

    fn words(&self) -> &WordTable {
        &self.words
    }

    fn definition(&self, headword: &str) -> Option<String> {
        self.defs.get(self.words.slot(headword)?).cloned()
    }

    fn close(&mut self) {
        self.words.clear();
        self.defs.clear();
        self.loaded = false;
    }
}

/// Everything read from a `.bgl` file.
#[derive(Debug, Default)]
pub struct ParsedBgl {
    /// Glossary title.
    pub title: String,
    /// Author.
    pub author: String,
    /// Description.
    pub description: String,
    /// Source language name.
    pub source_lang: String,
    /// Target language name.
    pub target_lang: String,
    /// Encoding used for headwords.
    pub source_encoding: String,
    /// Encoding used for definitions.
    pub target_encoding: String,
    /// (headword + alternates, HTML definition), after PyGlossary's entry filters.
    pub entries: Vec<(Vec<String>, String)>,
}

fn uint_from_bytes(b: &[u8]) -> u64 {
    b.iter().fold(0u64, |acc, &x| (acc << 8) | x as u64)
}

fn language(code: u64) -> Option<(&'static str, &'static str)> {
    LANGUAGES
        .iter()
        .find(|(c, _, _)| *c as u64 == code)
        .map(|(_, n, e)| (*n, *e))
}

fn charset(code: u8) -> Option<&'static str> {
    CHARSETS.iter().find(|(c, _)| *c == code).map(|(_, e)| *e)
}

/// Decompress the gzip stream that follows the 6-byte BGL header.
/// CRC errors are ignored (many real files have a broken trailer), so the
/// deflate stream is read directly and whatever was decoded is kept.
fn inflate_gzip_member(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 10 || data[0] != 0x1f || data[1] != 0x8b || data[2] != 8 {
        return Err("invalid gzip header".into());
    }
    let flags = data[3];
    let mut pos = 10usize;
    if flags & 0x04 != 0 {
        if pos + 2 > data.len() {
            return Err("truncated gzip header".into());
        }
        let xlen = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2 + xlen;
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
    if pos > data.len() {
        return Err("truncated gzip header".into());
    }
    let mut out = Vec::new();
    let mut dec = flate2::read::DeflateDecoder::new(&data[pos..]);
    // Keep whatever was decoded even if the stream ends early.
    let _ = dec.read_to_end(&mut out);
    Ok(out)
}

struct Block<'a> {
    kind: u8,
    data: &'a [u8],
}

fn read_blocks(buf: &[u8]) -> Vec<Block<'_>> {
    let mut blocks = Vec::new();
    let mut pos = 0usize;
    while pos < buf.len() {
        let first = buf[pos];
        pos += 1;
        let kind = first & 0xF;
        let mut len = (first >> 4) as usize;
        if len < 4 {
            let n = len + 1;
            if pos + n > buf.len() {
                break;
            }
            len = uint_from_bytes(&buf[pos..pos + n]) as usize;
            pos += n;
        } else {
            len -= 4;
        }
        let end = (pos + len).min(buf.len());
        blocks.push(Block {
            kind,
            data: &buf[pos..end],
        });
        pos = end;
    }
    blocks
}

/// Parse a whole `.bgl` file.
pub fn parse_bgl(raw: &[u8]) -> Result<ParsedBgl, String> {
    if raw.len() < 6
        || !(raw[..4] == [0x12, 0x34, 0x00, 0x01] || raw[..4] == [0x12, 0x34, 0x00, 0x02])
    {
        return Err("invalid Babylon header".into());
    }
    let gz_offset = uint_from_bytes(&raw[4..6]) as usize;
    if gz_offset < 6 || gz_offset >= raw.len() {
        return Err(format!("invalid gzip header position: {}", gz_offset));
    }
    let buf = inflate_gzip_member(&raw[gz_offset..])?;
    let blocks = read_blocks(&buf);

    // ---- pass 1: info
    let mut default_charset: Option<&str> = None;
    let mut source_charset: Option<&str> = None;
    let mut target_charset: Option<&str> = None;
    let mut source_lang: Option<(&str, &str)> = None;
    let mut target_lang: Option<(&str, &str)> = None;
    let mut utf8 = false;
    let mut info_bytes: Vec<(&str, &[u8])> = Vec::new();
    for b in &blocks {
        if b.data.is_empty() {
            continue;
        }
        match b.kind {
            0 => {
                if b.data[0] == 8 && b.data.len() > 1 {
                    default_charset = charset(b.data[1]);
                }
            }
            3 => {
                if b.data.len() < 2 {
                    continue;
                }
                let code = uint_from_bytes(&b.data[..2]);
                let value = &b.data[2..];
                if value.is_empty() {
                    continue;
                }
                match code {
                    0x01 => info_bytes.push(("title", value)),
                    0x02 => info_bytes.push(("author", value)),
                    0x09 => info_bytes.push(("description", value)),
                    0x07 => {
                        if let Some(l) = language(uint_from_bytes(value)) {
                            source_lang = Some(l);
                        }
                    }
                    0x08 => {
                        if let Some(l) = language(uint_from_bytes(value)) {
                            target_lang = Some(l);
                        }
                    }
                    0x11 => utf8 = uint_from_bytes(value) & 0x8000 != 0,
                    0x1A => {
                        if let Some(c) = charset(value[0]) {
                            source_charset = Some(c);
                        }
                    }
                    0x1B => {
                        if let Some(c) = charset(value[0]) {
                            target_charset = Some(c);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    let default_enc = default_charset.unwrap_or("cp1252");
    let source_enc = if utf8 {
        "utf-8"
    } else if let Some(c) = source_charset {
        c
    } else if let Some((_, e)) = source_lang {
        e
    } else {
        default_enc
    };
    let target_enc = if utf8 {
        "utf-8"
    } else if let Some(c) = target_charset {
        c
    } else if let Some((_, e)) = target_lang {
        e
    } else {
        default_enc
    };

    let mut out = ParsedBgl {
        source_lang: source_lang.map(|l| l.0.to_string()).unwrap_or_default(),
        target_lang: target_lang.map(|l| l.0.to_string()).unwrap_or_default(),
        source_encoding: source_enc.to_string(),
        target_encoding: target_enc.to_string(),
        ..Default::default()
    };
    for (key, value) in info_bytes {
        let text = decode_replace(value, target_enc);
        let text = text.trim_matches('\0').to_string();
        match key {
            "title" => out.title = text,
            "author" => out.author = text,
            "description" => out.description = text,
            _ => {}
        }
    }
    let persian = is_persian(&out.source_lang) || is_persian(&out.target_lang);
    let rd = Reader {
        source_enc,
        target_enc,
    };

    // ---- pass 2: entries
    for b in &blocks {
        if b.data.is_empty() {
            continue;
        }
        let parsed = match b.kind {
            11 => rd.entry_type11(b.data),
            1 | 7 | 10 | 13 => rd.entry_basic(b.data),
            _ => None,
        };
        let Some((word, alts, defi)) = parsed else {
            continue;
        };
        let mut words = vec![word];
        words.extend(alts);
        if let Some(e) = pyglossary_entry_filters(words, defi, persian) {
            out.entries.push(e);
        }
    }
    Ok(out)
}

struct Reader<'a> {
    source_enc: &'a str,
    target_enc: &'a str,
}

impl Reader<'_> {
    fn entry_basic(&self, d: &[u8]) -> Option<(String, Vec<String>, String)> {
        let mut pos = 0usize;
        if pos + 1 > d.len() {
            return None;
        }
        let len = d[pos] as usize;
        pos += 1;
        if pos + len > d.len() {
            return None;
        }
        let word = self.process_key(&d[pos..pos + len]);
        let word = py_strip(&word).to_string();
        pos += len;
        if pos + 2 > d.len() {
            return None;
        }
        let len = uint_from_bytes(&d[pos..pos + 2]) as usize;
        pos += 2;
        if pos + len > d.len() {
            return None;
        }
        let defi = self.process_defi(&d[pos..pos + len]);
        pos += len;
        let mut alts = BTreeSet::new();
        while pos < d.len() {
            let len = d[pos] as usize;
            pos += 1;
            if pos + len > d.len() {
                return None;
            }
            alts.insert(self.process_alt_key(&d[pos..pos + len]));
            pos += len;
        }
        alts.remove(&word);
        Some((word, alts.into_iter().collect(), defi))
    }

    fn entry_type11(&self, d: &[u8]) -> Option<(String, Vec<String>, String)> {
        let mut pos = 0usize;
        if pos + 5 > d.len() {
            return None;
        }
        let wlen = uint_from_bytes(&d[pos..pos + 5]) as usize;
        pos += 5;
        if pos + wlen > d.len() {
            return None;
        }
        let word = self.process_key(&d[pos..pos + wlen]);
        pos += wlen;
        if pos + 4 > d.len() {
            return None;
        }
        let count = uint_from_bytes(&d[pos..pos + 4]);
        pos += 4;
        let mut alts = BTreeSet::new();
        for _ in 0..count {
            if pos + 4 > d.len() {
                return None;
            }
            let alen = uint_from_bytes(&d[pos..pos + 4]) as usize;
            pos += 4;
            if alen == 0 {
                break;
            }
            if pos + alen > d.len() {
                return None;
            }
            alts.insert(self.process_alt_key(&d[pos..pos + alen]));
            pos += alen;
        }
        alts.remove(&word);
        let end4 = (pos + 4).min(d.len());
        let dlen = uint_from_bytes(&d[pos.min(d.len())..end4]) as usize;
        pos += 4;
        if pos + dlen > d.len() {
            return None;
        }
        let defi = self.process_defi(&d[pos..pos + dlen]);
        Some((word, alts.into_iter().collect(), defi))
    }

    fn process_key(&self, b: &[u8]) -> String {
        let b = strip_dollar_indexes(b);
        let mut u = decode_ignore(&b, self.source_enc);
        u = replace_html_entries(&u, false);
        u = strip_html_tags(&u);
        u = remove_control_chars(&u);
        u = remove_newlines(&u);
        py_lstrip(&u).to_string()
    }

    fn process_alt_key(&self, b: &[u8]) -> String {
        let b = strip_dollar_indexes(b);
        let mut u = decode_ignore(&b, self.source_enc);
        u = strip_slash_alt_key(&u);
        u = strip_html_tags(&u);
        u = replace_html_entries(&u, false);
        u = remove_control_chars(&u);
        u = remove_newlines(&u);
        py_lstrip(&u).to_string()
    }

    fn process_defi(&self, b_defi: &[u8]) -> String {
        let f = collect_defi_fields(b_defi);
        let mut u_defi = decode_charset_tags(f.defi, self.target_enc, self.source_enc);
        u_defi = u_defi.replace(['\u{1e}', '\u{1f}'], "");
        u_defi = replace_html_entries(&u_defi, true);
        u_defi = remove_control_chars(&u_defi);
        u_defi = normalize_newlines(&u_defi);
        let u_defi = py_strip(&u_defi).to_string();
        let src = |b: &[u8]| {
            let t = decode_charset_tags(b, self.source_enc, self.source_enc);
            remove_control_chars(&replace_html_entries(&t, true))
        };
        let u_title = f.title.map(src).unwrap_or_default();
        let u_title_trans = f.title_trans.map(src).unwrap_or_default();
        let u_t50 = match (f.t50, f.code_t50) {
            (Some(b), 0x1B) => src(b),
            _ => String::new(),
        };
        let u_t60 = match (f.t60, f.code_t60) {
            (Some(b), 0x1B) => src(b),
            _ => String::new(),
        };
        let mut s = String::new();
        let pos = f.part_of_speech.unwrap_or("");
        if !pos.is_empty() || !u_title.is_empty() {
            if !pos.is_empty() {
                s.push_str(&format!(
                    "<font color=\"#007000\">{}</font>",
                    xml_escape(pos)
                ));
            }
            if !u_title.is_empty() {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str(&u_title);
            }
            s.push_str("<br>\n");
        }
        if !u_title_trans.is_empty() {
            s.push_str(&u_title_trans);
            s.push_str("<br>\n");
        }
        if !u_t50.is_empty() {
            s.push_str(&format!("[{}]<br>\n", u_t50));
        }
        if !u_t60.is_empty() {
            s.push_str(&format!("[{}]<br>\n", u_t60));
        }
        s.push_str(&u_defi);
        if let Some(x) = s.strip_suffix("<br>") {
            s = x.to_string();
        }
        if let Some(x) = s.strip_suffix("<BR>") {
            s = x.to_string();
        }
        s
    }
}

#[derive(Default)]
struct DefiFields<'a> {
    defi: &'a [u8],
    part_of_speech: Option<&'static str>,
    title: Option<&'a [u8]>,
    title_trans: Option<&'a [u8]>,
    t50: Option<&'a [u8]>,
    code_t50: u8,
    t60: Option<&'a [u8]>,
    code_t60: u8,
}

fn find_fields_start(b: &[u8]) -> Option<usize> {
    if b.len() < 2 {
        return None;
    }
    let last = b.len() - 1; // the last byte is never a field start
    let mut from = 0usize;
    loop {
        let idx = b[from..last]
            .iter()
            .position(|&x| x == 0x14)
            .map(|p| p + from)?;
        if b[idx + 1] != 0x20 {
            return Some(idx);
        }
        from = idx + 1;
    }
}

fn collect_defi_fields(b: &[u8]) -> DefiFields<'_> {
    let mut f = DefiFields {
        defi: b,
        ..Default::default()
    };
    let Some(d0) = find_fields_start(b) else {
        return f;
    };
    f.defi = &b[..d0];
    let n = b.len();
    let mut i = d0 + 1;
    while i < n {
        match b[i] {
            0x02 => {
                if i + 1 >= n {
                    return f;
                }
                match PARTS_OF_SPEECH.iter().find(|(c, _)| *c == b[i + 1]) {
                    Some((_, p)) => f.part_of_speech = *p,
                    None => return f,
                }
                i += 2;
            }
            0x06 => {
                if i + 1 >= n {
                    return f;
                }
                i += 2;
            }
            0x07 => {
                if i + 3 > n {
                    return f;
                }
                i += 3;
            }
            0x13 | 0x1A => {
                if i + 1 >= n {
                    return f;
                }
                let len = b[i + 1] as usize;
                i += 2;
                if len == 0 {
                    continue;
                }
                if i + len > n {
                    return f;
                }
                i += len;
            }
            0x18 => {
                if i + 1 >= n {
                    return f;
                }
                i += 1;
                let len = b[i] as usize;
                i += 1;
                if len == 0 {
                    continue;
                }
                if i + len > n {
                    return f;
                }
                f.title = Some(&b[i..i + len]);
                i += len;
            }
            0x28 => {
                if i + 2 >= n {
                    return f;
                }
                i += 1;
                let len = uint_from_bytes(&b[i..i + 2]) as usize;
                i += 2;
                if len == 0 {
                    continue;
                }
                if i + len > n {
                    return f;
                }
                f.title_trans = Some(&b[i..i + len]);
                i += len;
            }
            0x40..=0x4F => {
                let len = (b[i] - 0x3F) as usize;
                if i + 2 + len > n {
                    return f;
                }
                i += 2 + len;
            }
            0x50 => {
                if i + 2 >= n {
                    return f;
                }
                f.code_t50 = b[i + 1];
                let len = b[i + 2] as usize;
                i += 3;
                if len == 0 {
                    continue;
                }
                if i + len > n {
                    return f;
                }
                f.t50 = Some(&b[i..i + len]);
                i += len;
            }
            0x60 => {
                if i + 4 > n {
                    return f;
                }
                f.code_t60 = b[i + 1];
                i += 2;
                let len = uint_from_bytes(&b[i..i + 2]) as usize;
                i += 2;
                if len == 0 {
                    continue;
                }
                if i + len > n {
                    return f;
                }
                f.t60 = Some(&b[i..i + len]);
                i += len;
            }
            _ => return f,
        }
    }
    f
}

/// Remove Babylon `$123$` index markers from a headword.
pub fn strip_dollar_indexes(b: &[u8]) -> Vec<u8> {
    let find = |from: usize| {
        b.get(from..)
            .and_then(|s| s.iter().position(|&x| x == b'$'))
            .map(|p| p + from)
    };
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0usize;
    loop {
        let Some(d0) = find(i) else {
            out.extend_from_slice(&b[i.min(b.len())..]);
            break;
        };
        let Some(d1) = find(d0 + 1) else {
            out.extend_from_slice(&b[i..]);
            break;
        };
        if d1 == d0 + 1 {
            out.extend_from_slice(&b[i..d0]);
            i = d1 + 1;
            while i < b.len() && b[i] == b'$' {
                i += 1;
            }
            if i >= b.len() {
                break;
            }
            continue;
        }
        if b[d0 + 1..d1].iter().any(|c| !c.is_ascii_digit()) {
            out.extend_from_slice(&b[i..d1]);
            i = d1;
            continue;
        }
        out.extend_from_slice(&b[i..d0]);
        i = d1 + 1;
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `(^|\s)/(\w)` → `\1\2`.
fn strip_slash_alt_key(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for i in 0..c.len() {
        if c[i] == '/'
            && (i == 0 || c[i - 1].is_whitespace())
            && i + 1 < c.len()
            && is_word_char(c[i + 1])
        {
            continue;
        }
        out.push(c[i]);
    }
    out
}

/// PyGlossary `stripHtmlTags`: `(?:<[/a-zA-Z].*?(?:>|$))+` → " ".
fn strip_html_tags(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    // `$` matches at the very end, or before a final "\n".
    let at_dollar = |p: usize| p == n || (p == n - 1 && c[p] == '\n');
    let match_tag = |p: usize| -> Option<usize> {
        if p + 1 >= n || c[p] != '<' || !(c[p + 1] == '/' || c[p + 1].is_ascii_alphabetic()) {
            return None;
        }
        let mut q = p + 2;
        loop {
            if q < n && c[q] == '>' {
                return Some(q + 1);
            }
            if at_dollar(q) {
                return Some(q);
            }
            if q >= n || c[q] == '\n' {
                return None;
            }
            q += 1;
        }
    };
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < n {
        if let Some(mut end) = match_tag(i) {
            while let Some(e2) = match_tag(end) {
                end = e2;
            }
            out.push(' ');
            i = end;
            continue;
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// PyGlossary `removeControlChars`: `[\x00-\x08\x0c\x0e-\x1f]`.
fn remove_control_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| !matches!(c, '\u{0}'..='\u{8}' | '\u{c}' | '\u{e}'..='\u{1f}'))
        .collect()
}

fn collapse_newlines(s: &str, with: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if c == '\r' || c == '\n' {
            if !in_run {
                out.push_str(with);
                in_run = true;
            }
        } else {
            in_run = false;
            out.push(c);
        }
    }
    out
}

fn remove_newlines(s: &str) -> String {
    collapse_newlines(s, " ")
}

fn normalize_newlines(s: &str) -> String {
    collapse_newlines(s, "\n")
}

/// PyGlossary entity replacement. Keys: `(?:&#x|&#|&)(\w+);` (semicolon
/// required, result not escaped). Definitions: `;` optional and the result
/// is XML-escaped.
fn replace_html_entries(s: &str, defi: bool) -> String {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < n {
        if c[i] != '&' {
            out.push(c[i]);
            i += 1;
            continue;
        }
        // try "&#x", "&#", "&" in that order
        let prefixes: [(usize, u8); 3] = [(3, 16), (2, 10), (1, 0)];
        let mut done = false;
        for (plen, radix) in prefixes {
            if i + plen > n {
                continue;
            }
            let ok_prefix = match plen {
                3 => c[i + 1] == '#' && (c[i + 2] == 'x' || c[i + 2] == 'X'),
                2 => c[i + 1] == '#',
                _ => true,
            };
            if !ok_prefix {
                continue;
            }
            let mut j = i + plen;
            while j < n && is_word_char(c[j]) {
                j += 1;
            }
            if j == i + plen {
                continue;
            }
            let has_semi = j < n && c[j] == ';';
            if !defi && !has_semi {
                continue;
            }
            let end = if has_semi { j + 1 } else { j };
            let name: String = c[i + plen..j].iter().collect();
            let text: String = c[i..end].iter().collect();
            let res: Option<String> = if radix != 0 {
                let code = u32::from_str_radix(&name, radix as u32)
                    .ok()
                    .filter(|&v| v > 0);
                Some(
                    code.and_then(char::from_u32)
                        .unwrap_or('\u{FFFD}')
                        .to_string(),
                )
            } else {
                entity_codepoint(&name.to_lowercase())
                    .and_then(char::from_u32)
                    .map(|ch| ch.to_string())
            };
            match res {
                Some(r) => out.push_str(&if defi { xml_escape(&r) } else { r }),
                None => out.push_str(&text),
            }
            i = end;
            done = true;
            break;
        }
        if !done {
            out.push('&');
            i += 1;
        }
    }
    out
}

/// PyGlossary `replaceAsciiCharRefs`: `&#NNN;` with 128 ≤ NNN ≤ 255 → raw byte.
fn replace_ascii_char_refs(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'&' && i + 1 < b.len() && b[i + 1] == b'#' {
            let mut j = i + 2;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j > i + 2 && j < b.len() && b[j] == b';' {
                let num = std::str::from_utf8(&b[i + 2..j])
                    .ok()
                    .and_then(|s| s.parse::<i64>().ok());
                if let Some(code) = num.filter(|&c| (128..=255).contains(&c)) {
                    out.push(code as u8);
                    i = j + 1;
                    continue;
                }
                out.extend_from_slice(&b[i..=j]);
                i = j + 1;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Match `<charset\s+c=['"]?(\w)"?>` or `</charset>` at `p` (case-insensitive).
/// Returns (end, Some(type char)) or (end, None) for the closing tag.
fn match_charset_tag(b: &[u8], p: usize) -> Option<(usize, Option<u8>)> {
    let rest = &b[p..];
    let ieq =
        |a: &[u8], lit: &[u8]| a.len() >= lit.len() && a[..lit.len()].eq_ignore_ascii_case(lit);
    if ieq(rest, b"</charset>") {
        return Some((p + 10, None));
    }
    if !ieq(rest, b"<charset") {
        return None;
    }
    let mut q = p + 8;
    let ws = |x: u8| matches!(x, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let ws_start = q;
    while q < b.len() && ws(b[q]) {
        q += 1;
    }
    if q == ws_start || q + 2 > b.len() || !b[q].eq_ignore_ascii_case(&b'c') || b[q + 1] != b'=' {
        return None;
    }
    q += 2;
    if q < b.len() && (b[q] == b'\'' || b[q] == b'"') {
        q += 1;
    }
    if q >= b.len() || !(b[q].is_ascii_alphanumeric() || b[q] == b'_') {
        return None;
    }
    let t = b[q];
    q += 1;
    if q < b.len() && b[q] == b'"' {
        q += 1;
    }
    if q < b.len() && b[q] == b'>' {
        Some((q + 1, Some(t)))
    } else {
        None
    }
}

fn decode_text_block(enc: &str, b: &[u8]) -> String {
    if enc == "babylon-reference" {
        let mut s = String::new();
        for r in b.split(|&x| x == b';') {
            if r.len() == 4 && r.iter().all(|x| x.is_ascii_hexdigit()) {
                let v = u32::from_str_radix(std::str::from_utf8(r).unwrap(), 16).unwrap();
                if let Some(ch) = char::from_u32(v) {
                    s.push(ch);
                }
            }
        }
        return s;
    }
    if enc == "cp1252" {
        return decode_replace(&replace_ascii_char_refs(b), enc);
    }
    decode_replace(b, enc)
}

/// Decode text that may contain Babylon `<charset c=…>` sections.
fn decode_charset_tags(b: &[u8], default_enc: &str, source_enc: &str) -> String {
    let mut out = String::new();
    let mut stack: Vec<String> = Vec::new();
    let mut seg_start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'<' {
            if let Some((end, kind)) = match_charset_tag(b, i) {
                let enc = stack
                    .last()
                    .map(String::as_str)
                    .unwrap_or(default_enc)
                    .to_string();
                out.push_str(&decode_text_block(&enc, &b[seg_start..i]));
                match kind {
                    None => {
                        stack.pop();
                    }
                    Some(t) => {
                        let e = match t.to_ascii_lowercase() {
                            b't' => "babylon-reference".to_string(),
                            b'u' => "utf-8".to_string(),
                            b'k' | b'e' => source_enc.to_string(),
                            b'g' => "gbk".to_string(),
                            _ => default_enc.to_string(),
                        };
                        stack.push(e);
                    }
                }
                i = end;
                seg_start = end;
                continue;
            }
        }
        i += 1;
    }
    let enc = stack
        .last()
        .map(String::as_str)
        .unwrap_or(default_enc)
        .to_string();
    out.push_str(&decode_text_block(&enc, &b[seg_start..]));
    out
}

// ------------------------------------------------------------- writer

/// Minimal `.bgl` writer (used by tests and `--selftest`): UTF-8 glossary
/// with a title, languages and type-1 entries with optional alternates.
pub fn write_bgl(
    path: &Path,
    title: &str,
    source_lang_code: u8,
    target_lang_code: u8,
    entries: &[(&str, &str, &[&str])],
) -> std::io::Result<()> {
    fn block(out: &mut Vec<u8>, kind: u8, data: &[u8]) {
        let len = data.len();
        if len < 12 {
            out.push((((len + 4) as u8) << 4) | kind);
        } else if len < 256 {
            out.push(kind); // length field of 1 byte
            out.push(len as u8);
        } else {
            out.push((1 << 4) | kind); // length field of 2 bytes
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
        out.extend_from_slice(data);
    }
    let mut body = Vec::new();
    let info = |code: u16, value: &[u8], body: &mut Vec<u8>| {
        let mut d = code.to_be_bytes().to_vec();
        d.extend_from_slice(value);
        block(body, 3, &d);
    };
    info(0x11, &0x8000u32.to_be_bytes(), &mut body); // utf8Encoding flag
    info(0x01, title.as_bytes(), &mut body);
    info(0x07, &[0, 0, 0, source_lang_code], &mut body);
    info(0x08, &[0, 0, 0, target_lang_code], &mut body);
    for (word, defi, alts) in entries {
        let mut d = vec![word.len() as u8];
        d.extend_from_slice(word.as_bytes());
        d.extend_from_slice(&(defi.len() as u16).to_be_bytes());
        d.extend_from_slice(defi.as_bytes());
        for a in *alts {
            d.push(a.len() as u8);
            d.extend_from_slice(a.as_bytes());
        }
        block(&mut body, 1, &d);
    }
    block(&mut body, 4, &[]);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut gz, &body)?;
    let gz = gz.finish()?;
    let mut file = vec![0x12, 0x34, 0x00, 0x01, 0x00, 0x06];
    file.extend_from_slice(&gz);
    std::fs::write(path, file)
}

/// Babylon language codes (PyGlossary `bgl_language.languageByCode`): (code, name, encoding).
pub(crate) static LANGUAGES: &[(u32, &str, &str)] = &[
    (0, "English", "cp1252"),
    (1, "French", "cp1252"),
    (2, "Italian", "cp1252"),
    (3, "Spanish", "cp1252"),
    (4, "Dutch", "cp1252"),
    (5, "Portuguese", "cp1252"),
    (6, "German", "cp1252"),
    (7, "Russian", "cp1251"),
    (8, "Japanese", "cp932"),
    (9, "Chinese", "cp950"),
    (10, "Chinese", "cp936"),
    (11, "Greek", "cp1253"),
    (12, "Korean", "cp949"),
    (13, "Turkish", "cp1254"),
    (14, "Hebrew", "cp1255"),
    (15, "Arabic", "cp1256"),
    (16, "Thai", "cp874"),
    (17, "Other", "cp1252"),
    (18, "Chinese", "cp936"),
    (19, "Chinese", "cp950"),
    (20, "Other Eastern-European languages", "cp1250"),
    (21, "Other Western-European languages", "cp1252"),
    (22, "Other Russian languages", "cp1251"),
    (23, "Other Japanese languages", "cp932"),
    (24, "Other Baltic languages", "cp1257"),
    (25, "Other Greek languages", "cp1253"),
    (26, "Other Korean dialects", "cp949"),
    (27, "Other Turkish dialects", "cp1254"),
    (28, "Other Thai dialects", "cp874"),
    (29, "Polish", "cp1250"),
    (30, "Hungarian", "cp1250"),
    (31, "Czech", "cp1250"),
    (32, "Lithuanian", "cp1257"),
    (33, "Latvian", "cp1257"),
    (34, "Catalan", "cp1252"),
    (35, "Croatian", "cp1250"),
    (36, "Serbian", "cp1250"),
    (37, "Slovak", "cp1250"),
    (38, "Albanian", "cp1252"),
    (39, "Urdu", "cp1256"),
    (40, "Slovenian", "cp1250"),
    (41, "Estonian", "cp1252"),
    (42, "Bulgarian", "cp1250"),
    (43, "Danish", "cp1252"),
    (44, "Finnish", "cp1252"),
    (45, "Icelandic", "cp1252"),
    (46, "Norwegian", "cp1252"),
    (47, "Romanian", "cp1252"),
    (48, "Swedish", "cp1252"),
    (49, "Ukrainian", "cp1251"),
    (50, "Belarusian", "cp1251"),
    (51, "Persian", "cp1256"),
    (52, "Basque", "cp1252"),
    (53, "Macedonian", "cp1250"),
    (54, "Afrikaans", "cp1252"),
    (55, "Faroese", "cp1252"),
    (56, "Latin", "cp1252"),
    (57, "Esperanto", "cp1254"),
    (58, "Tamazight", "cp1252"),
    (59, "Armenian", "cp1252"),
    (60, "Hindi", "cp1252"),
    (61, "Somali", "cp1252"),
];
/// Babylon charset codes (PyGlossary `bgl_charset.charsetByCode`).
pub(crate) static CHARSETS: &[(u8, &str)] = &[
    (0x41, "cp1252"),
    (0x42, "cp1252"),
    (0x43, "cp1250"),
    (0x44, "cp1251"),
    (0x45, "cp932"),
    (0x46, "cp950"),
    (0x47, "cp936"),
    (0x48, "cp1257"),
    (0x49, "cp1253"),
    (0x4A, "cp949"),
    (0x4B, "cp1254"),
    (0x4C, "cp1255"),
    (0x4D, "cp1256"),
    (0x4E, "cp874"),
];
/// Babylon part-of-speech codes (PyGlossary `bgl_pos.partOfSpeechByCode`).
pub(crate) static PARTS_OF_SPEECH: &[(u8, Option<&str>)] = &[
    (0x30, Some("noun")),
    (0x31, Some("adjective")),
    (0x32, Some("verb")),
    (0x33, Some("adverb")),
    (0x34, Some("interjection")),
    (0x35, Some("pronoun")),
    (0x36, Some("preposition")),
    (0x37, Some("conjunction")),
    (0x38, Some("suffix")),
    (0x39, Some("prefix")),
    (0x3A, Some("article")),
    (0x3B, Some("")),
    (0x3C, Some("abbreviation")),
    (0x3D, Some("masculine noun and adjective")),
    (0x3E, Some("feminine noun and adjective")),
    (0x3F, Some("masculine and feminine noun and adjective")),
    (0x40, Some("feminine noun")),
    (0x41, Some("masculine and feminine noun")),
    (0x42, Some("masculine noun")),
    (0x43, Some("numeral")),
    (0x44, Some("participle")),
    (0x45, None),
    (0x46, None),
    (0x47, None),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dollar_indexes() {
        assert_eq!(strip_dollar_indexes(b"word$123$"), b"word");
        assert_eq!(strip_dollar_indexes(b"a$12$ b"), b"a b");
        assert_eq!(strip_dollar_indexes(b"US$ 5"), b"US$ 5");
        assert_eq!(strip_dollar_indexes(b"x$$"), b"x");
    }

    #[test]
    fn html_helpers() {
        assert_eq!(strip_html_tags("a<b>c</b>d"), "a c d");
        assert_eq!(strip_html_tags("a<font x=1>"), "a ");
        assert_eq!(
            replace_html_entries("&eacute;&#65;&#x42;&lt;", false),
            "éAB<"
        );
        assert_eq!(
            replace_html_entries("&lt;b&gt;&nbsp", true),
            "&lt;b&gt;\u{a0}"
        );
        assert_eq!(replace_html_entries("&csdot; &", true), "&csdot; &");
        assert_eq!(strip_slash_alt_key("/abc x /d"), "abc x d");
        assert_eq!(remove_control_chars("a\u{1}b\tc"), "ab\tc");
        assert_eq!(normalize_newlines("a\r\n\nb"), "a\nb");
    }

    #[test]
    fn charset_tags() {
        let s = decode_charset_tags(b"x<charset c=T>00E9;0041;</charset>y", "cp1252", "cp1252");
        assert_eq!(s, "x\u{e9}Ay");
        let s = decode_charset_tags(b"<CHARSET c=U>\xc3\xbc</CHARSET>\xfc", "cp1254", "cp1254");
        assert_eq!(s, "\u{fc}\u{fc}");
    }

    #[test]
    fn defi_fields() {
        let mut b = b"main text".to_vec();
        b.extend_from_slice(&[0x14, 0x02, 0x30]);
        let f = collect_defi_fields(&b);
        assert_eq!(f.defi, b"main text");
        assert_eq!(f.part_of_speech, Some("noun"));
    }

    #[test]
    fn roundtrip_writer() {
        let d = std::env::temp_dir().join(format!("dictz-bgl-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("t.bgl");
        write_bgl(
            &f,
            "Tiny BGL",
            0,
            13,
            &[("apple", "elma<br>", &["apples"]), ("book", "kitap", &[])],
        )
        .unwrap();
        let mut bgl = BabylonDictionary::new(&f);
        bgl.load().unwrap();
        assert_eq!(bgl.metadata().name, "Tiny BGL");
        assert_eq!(bgl.metadata().source_language, "English");
        assert_eq!(bgl.metadata().target_language, "Turkish");
        assert_eq!(bgl.lookup("apples", true)[0].definition, "elma");
        assert_eq!(bgl.lookup("book", true)[0].definition, "kitap");
    }
}
