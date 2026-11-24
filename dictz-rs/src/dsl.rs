//! ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`) parser.
//!
//! A native port of PyGlossary's DSL reader (`pyglossary/plugins/dsl`), which
//! the Python edition uses. Headwords and HTML definitions are produced the
//! same way, so both editions show the same articles; the shared golden file
//! `tests/data/parity/dsl_golden.json` checks this in both test suites.
//!
//! One deliberate difference: headwords are stored unescaped
//! (`rock & roll`, not `rock &amp; roll`). The Python edition unescapes
//! PyGlossary's headwords the same way.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::text::{is_persian, py_strip, pyglossary_entry_filters, quoteattr, xml_escape};
use crate::{read_maybe_gzip, Dictionary, DictionaryMetadata, DictzError, WordTable};

/// A DSL dictionary.
pub struct DslDictionary {
    path: PathBuf,
    meta: DictionaryMetadata,
    words: WordTable,
    defs: Vec<String>,
    resources: BTreeSet<String>,
    loaded: bool,
}

impl DslDictionary {
    /// Prepare to read `path` (`.dsl` or `.dsl.dz`).
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let meta = DictionaryMetadata {
            name: stem(&path),
            file_path: path.clone(),
            format: "dsl".into(),
            ..Default::default()
        };
        Self {
            path,
            meta,
            words: WordTable::new(),
            defs: Vec::new(),
            resources: BTreeSet::new(),
            loaded: false,
        }
    }

    /// Media file names referenced by `[s]` tags.
    pub fn resources(&self) -> &BTreeSet<String> {
        &self.resources
    }
}

fn stem(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_lowercase();
    for ext in [".dsl.dz", ".dsl", ".dz"] {
        if lower.ends_with(ext) {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or(name)
}

impl Dictionary for DslDictionary {
    fn load(&mut self) -> Result<(), DictzError> {
        if self.loaded {
            return Ok(());
        }
        let raw = read_maybe_gzip(&self.path)?;
        let text = decode_dsl(&raw);
        let parsed = parse_dsl(&text);
        if !parsed.name.is_empty() {
            self.meta.name = parsed.name.clone();
        }
        self.meta.source_language = parsed.index_language.clone();
        self.meta.target_language = parsed.contents_language.clone();
        let persian = is_persian(&parsed.index_language) || is_persian(&parsed.contents_language);
        for (terms, defi) in parsed.entries {
            let Some((terms, defi)) = pyglossary_entry_filters(terms, defi, persian) else {
                continue;
            };
            for term in terms {
                if let Some(slot) = self.words.insert(&term) {
                    debug_assert_eq!(slot, self.defs.len());
                    self.defs.push(defi.clone());
                }
            }
        }
        self.resources = parsed.resources;
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

/// Decode DSL bytes: BOM → UTF-8 / UTF-16LE / UTF-16BE; otherwise UTF-16 is
/// guessed from NUL bytes, then UTF-8, then Windows-1251 (Lingvo's legacy
/// default). Line endings are normalised to `\n`.
pub fn decode_dsl(raw: &[u8]) -> String {
    let text: String = if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&raw[3..]).into_owned()
    } else if raw.starts_with(&[0xFF, 0xFE]) {
        encoding_rs::UTF_16LE
            .decode_without_bom_handling(&raw[2..])
            .0
            .into_owned()
    } else if raw.starts_with(&[0xFE, 0xFF]) {
        encoding_rs::UTF_16BE
            .decode_without_bom_handling(&raw[2..])
            .0
            .into_owned()
    } else {
        let probe = &raw[..raw.len().min(4096)];
        let even_nuls = probe.iter().step_by(2).filter(|&&b| b == 0).count();
        let odd_nuls = probe.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
        if odd_nuls > probe.len() / 8 {
            encoding_rs::UTF_16LE
                .decode_without_bom_handling(raw)
                .0
                .into_owned()
        } else if even_nuls > probe.len() / 8 {
            encoding_rs::UTF_16BE
                .decode_without_bom_handling(raw)
                .0
                .into_owned()
        } else {
            match std::str::from_utf8(raw) {
                Ok(s) => s.to_string(),
                Err(_) => encoding_rs::WINDOWS_1251
                    .decode_without_bom_handling(raw)
                    .0
                    .into_owned(),
            }
        }
    };
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Result of parsing a whole DSL text.
#[derive(Debug, Default)]
pub struct ParsedDsl {
    /// `#NAME`.
    pub name: String,
    /// `#INDEX_LANGUAGE`.
    pub index_language: String,
    /// `#CONTENTS_LANGUAGE`.
    pub contents_language: String,
    /// (headwords, HTML definition) per article, before entry filters.
    pub entries: Vec<(Vec<String>, String)>,
    /// Media file names referenced by `[s]` tags.
    pub resources: BTreeSet<String>,
}

fn unwrap_quotes(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    if c.len() >= 2 && (c[0] == '"' || c[0] == '\'') && c[c.len() - 1] == c[0] {
        c[1..c.len() - 1].iter().collect()
    } else {
        s.to_string()
    }
}

fn header_value(line: &str, skip: usize) -> String {
    let rest: String = line.chars().skip(skip).collect();
    py_strip(&rest).to_string()
}

/// Parse decoded DSL text (header + articles).
pub fn parse_dsl(text: &str) -> ParsedDsl {
    let mut out = ParsedDsl::default();
    // Lines keep their trailing "\n", like Python's file iteration.
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut i = 0;
    let mut buffered: Option<String> = None;
    while i < lines.len() {
        let line = crate::text::py_rstrip(lines[i])
            .trim_start_matches('\u{feff}')
            .to_string();
        i += 1;
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('#') {
            buffered = Some(line);
            break;
        }
        if line.starts_with("#NAME") {
            out.name = unwrap_quotes(&unwrap_quotes(&header_value(&line, 6)));
        } else if line.starts_with("#INDEX_LANGUAGE") {
            out.index_language = unwrap_quotes(&header_value(&line, 16));
        } else if line.starts_with("#CONTENTS_LANGUAGE") {
            out.contents_language = unwrap_quotes(&header_value(&line, 19));
        }
    }
    let rest = lines.split_off(i);
    let mut all: Vec<String> = Vec::new();
    if let Some(b) = buffered {
        all.push(b);
    }
    all.extend(rest.into_iter().map(str::to_string));

    let mut term_lines: Vec<String> = Vec::new();
    let mut text_lines: Vec<String> = Vec::new();
    for line in all {
        if py_strip(&line).is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            text_lines.push(line);
            continue;
        }
        if !text_lines.is_empty() {
            if let Some(e) = parse_entry_block(&term_lines, &text_lines, &mut out.resources) {
                out.entries.push(e);
            }
            term_lines.clear();
            text_lines.clear();
        }
        term_lines.push(line);
    }
    if !text_lines.is_empty() {
        if let Some(e) = parse_entry_block(&term_lines, &text_lines, &mut out.resources) {
            out.entries.push(e);
        }
    }
    out
}

fn parse_entry_block(
    term_lines: &[String],
    text_lines: &[String],
    resources: &mut BTreeSet<String>,
) -> Option<(Vec<String>, String)> {
    let mut terms: Vec<String> = Vec::new();
    let mut titles: Vec<String> = Vec::new();
    for line in term_lines {
        let Some(t) = transform_title(line) else {
            continue;
        };
        let term = py_strip(&t.output).to_string();
        let alt = py_strip(&t.output_alt).to_string();
        let title = py_strip(&t.title).to_string();
        if title != xml_escape(&term) {
            titles.push(format!("<b>{}</b>", title));
        }
        if alt != term {
            terms.push(term);
            terms.push(alt);
        } else {
            terms.push(term);
        }
    }
    let first = terms.first()?.clone();
    let body: String = text_lines.concat();
    let mut defi = match Transformer::new(&body, &first).transform() {
        Ok((html, res)) => {
            resources.extend(res);
            py_strip(&html).to_string()
        }
        Err(_) => String::new(),
    };
    if !titles.is_empty() {
        titles.push(defi);
        defi = titles.join("<br/>");
    }
    Some((terms, defi))
}

// ------------------------------------------------------------ transformer

/// Remove `{{ comments }}` (PyGlossary: `\{\{([^}]*)\}\}`).
fn remove_comments(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '{' && i + 1 < c.len() && c[i + 1] == '{' {
            let mut j = i + 2;
            while j < c.len() && c[j] != '}' {
                j += 1;
            }
            if j + 1 < c.len() && c[j] == '}' && c[j + 1] == '}' {
                i = j + 2;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Lex {
    Root,
    RootNewline,
    Backslash,
    Tag,
    TagAttr,
    TagAttrValue,
}

/// DSL article markup → HTML, exactly like PyGlossary's `Transformer`.
pub struct Transformer {
    input: Vec<char>,
    start: usize,
    pos: usize,
    output: String,
    attrs: Vec<(String, Option<String>)>,
    attr_name: String,
    current_key: String,
    example_color: String,
    audio: bool,
    res_files: BTreeSet<String>,
}

impl Transformer {
    /// `text`: article body; `current_key`: headword substituted for `~`.
    pub fn new(text: &str, current_key: &str) -> Self {
        Self {
            input: remove_comments(text).chars().collect(),
            start: 0,
            pos: 0,
            output: String::new(),
            attrs: Vec::new(),
            attr_name: String::new(),
            current_key: current_key.to_string(),
            example_color: "steelblue".into(),
            audio: true,
            res_files: BTreeSet::new(),
        }
    }

    fn end(&self) -> bool {
        self.pos >= self.input.len()
    }
    fn next(&mut self) -> char {
        let c = self.input[self.pos];
        self.pos += 1;
        c
    }
    fn reset_buf(&mut self) {
        self.start = self.pos;
        self.attr_name.clear();
        self.attrs.clear();
    }
    fn follows(&self, st: &str) -> bool {
        for (p, c) in (self.pos..).zip(st.chars()) {
            if p >= self.input.len() || self.input[p] != c {
                return false;
            }
        }
        true
    }
    fn skip_any(&mut self, chars: &str) {
        while self.pos < self.input.len() && chars.contains(self.input[self.pos]) {
            self.pos += 1;
        }
    }
    fn add_text(&mut self, s: &str) {
        self.output.push_str(&xml_escape(s));
    }
    fn set_attr(&mut self, name: String, value: Option<String>) {
        if let Some(slot) = self.attrs.iter_mut().find(|(k, _)| *k == name) {
            slot.1 = value;
        } else {
            self.attrs.push((name, value));
        }
    }
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.as_deref())
    }

    /// Run the transformation. Returns (html, referenced media files).
    pub fn transform(mut self) -> Result<(String, BTreeSet<String>), String> {
        let mut lex = Some(Lex::Root);
        while let Some(state) = lex {
            lex = match state {
                Lex::Root => self.lex_root(),
                Lex::RootNewline => self.lex_root_newline(),
                Lex::Backslash => self.lex_backslash(),
                Lex::Tag => self.lex_tag()?,
                Lex::TagAttr => self.lex_tag_attr(),
                Lex::TagAttrValue => self.lex_tag_attr_value()?,
            };
        }
        Ok((self.output, self.res_files))
    }

    fn lex_root(&mut self) -> Option<Lex> {
        if self.end() {
            return None;
        }
        let c = self.next();
        if self.end() {
            self.add_text(&c.to_string());
            return None;
        }
        match c {
            '\\' => Some(Lex::Backslash),
            '[' => {
                self.reset_buf();
                Some(Lex::Tag)
            }
            ']' => {
                self.next();
                if self.follows("[") {
                    self.next();
                }
                self.output.push(']');
                self.reset_buf();
                Some(Lex::Root)
            }
            '~' => {
                let key = self.current_key.clone();
                self.add_text(&key);
                self.reset_buf();
                Some(Lex::Root)
            }
            '\n' => Some(Lex::RootNewline),
            '<' if self.follows("<") => {
                self.next();
                self.lex_ref_text()
            }
            _ => {
                self.add_text(&c.to_string());
                self.reset_buf();
                Some(Lex::Root)
            }
        }
    }

    fn lex_root_newline(&mut self) -> Option<Lex> {
        self.skip_any(" \t");
        if !self.follows("[m") {
            self.output.push_str("<br/>");
        }
        self.reset_buf();
        Some(Lex::Root)
    }

    fn lex_backslash(&mut self) -> Option<Lex> {
        if self.end() {
            return None;
        }
        let c = self.next();
        if c == ' ' {
            self.output.push_str("&nbsp;");
        } else if (c == '<' || c == '>') && self.follows(&c.to_string()) {
            self.next();
            self.add_text(&format!("{}{}", c, c));
        } else {
            self.add_text(&c.to_string());
        }
        self.reset_buf();
        Some(Lex::Root)
    }

    fn lex_tag(&mut self) -> Result<Option<Lex>, String> {
        if self.end() {
            return Err(format!("'[' not closed near pos {} in lexTag", self.pos));
        }
        let c = self.next();
        if c == '[' {
            self.output.push('[');
            self.reset_buf();
            return Ok(Some(Lex::Root));
        }
        if c == ' ' || c == '\t' {
            self.skip_any(" \t");
            return Ok(Some(Lex::TagAttr));
        }
        if c == ']' {
            let tag: String = self.input[self.start..self.pos - 1].iter().collect();
            if tag.is_empty() {
                return Err(format!("empty tag near pos {}", self.pos));
            }
            return self.process_tag(&tag);
        }
        Ok(Some(Lex::Tag))
    }

    fn lex_tag_attr(&mut self) -> Option<Lex> {
        if self.end() {
            let n = std::mem::take(&mut self.attr_name);
            self.set_attr(n, None);
            self.reset_buf();
            return Some(Lex::Root);
        }
        let c = self.next();
        if c == ']' {
            let n = self.attr_name.clone();
            self.set_attr(n, None);
            self.pos -= 1;
            return Some(Lex::Tag);
        }
        if c == '=' {
            self.skip_any(" \t");
            return Some(Lex::TagAttrValue);
        }
        self.attr_name.push(c);
        Some(Lex::TagAttr)
    }

    fn lex_tag_attr_value(&mut self) -> Result<Option<Lex>, String> {
        let err = |p: usize| format!("'[' not closed near pos {} in lexTagAttrValue", p);
        if self.end() {
            return Err(err(self.pos));
        }
        let c = self.next();
        let mut quote: Option<char> = None;
        let mut value = String::new();
        if c == '\'' || c == '"' {
            if self.end() {
                return Err(err(self.pos));
            }
            quote = Some(c);
        } else {
            value.push(c);
        }
        loop {
            if self.end() {
                return Err(err(self.pos));
            }
            let mut c = self.next();
            if c == '\\' {
                if self.end() {
                    return Err(err(self.pos));
                }
                c = self.next();
                value.push(c);
                continue;
            }
            if c == ']' {
                self.pos -= 1;
                break;
            }
            if Some(c) == quote {
                break;
            }
            if quote.is_none() && (c == ' ' || c == '\t') {
                break;
            }
            value.push(c);
        }
        let n = self.attr_name.clone();
        self.set_attr(n, Some(value));
        Ok(Some(Lex::Tag))
    }

    fn process_tag(&mut self, tag: &str) -> Result<Option<Lex>, String> {
        self.attr_name.clear();
        if let Some(close) = tag.strip_prefix('/') {
            return self.process_tag_close(close);
        }
        let tag = tag.split(' ').next().unwrap_or("");
        match tag {
            "ref" => return Ok(self.lex_ref_text()),
            "url" => return Ok(self.lex_url_text()),
            "s" => return Ok(self.lex_s()),
            _ => {}
        }
        if tag.starts_with('m') {
            let mut padding = "0.3".to_string();
            if tag.chars().count() > 1 {
                padding = tag.chars().skip(1).collect();
                if padding == "0" {
                    padding = "0.3".into();
                }
            }
            self.output.push_str(&format!(
                "<p style=\"padding-left:{}em;margin:0\">",
                padding
            ));
        } else {
            match tag {
                "c" => {
                    let color = self
                        .attrs
                        .iter()
                        .find(|(_, v)| v.is_none())
                        .map(|(k, _)| k.clone())
                        .unwrap_or_else(|| "green".into());
                    self.output.push_str(&format!("<font color=\"{}\">", color));
                }
                "*" => self.output.push_str("<span class=\"sec\">"),
                "ex" => {
                    let c = self.example_color.clone();
                    self.output
                        .push_str(&format!("<span class=\"ex\"><font color=\"{}\">", c));
                }
                "t" => self
                    .output
                    .push_str("<font face=\"Helvetica\" class=\"dsl_t\">"),
                "p" => self
                    .output
                    .push_str("<i class=\"p\"><font color=\"green\">"),
                "i" => self.output.push_str("<i>"),
                "b" => self.output.push_str("<b>"),
                "u" => self.output.push_str("<u>"),
                "'" => self.output.push_str("<u class=\"accent\">"),
                "sup" => self.output.push_str("<sup>"),
                "sub" => self.output.push_str("<sub>"),
                _ => {} // trn, !trn, trs, !trs, lang, com and unknown tags
            }
        }
        self.reset_buf();
        Ok(Some(Lex::Root))
    }

    fn process_tag_close(&mut self, tag: &str) -> Result<Option<Lex>, String> {
        if tag.is_empty() {
            return Err("empty close tag".into());
        }
        let html = match tag {
            "m" => "</p>",
            "b" => "</b>",
            "u" | "'" => "</u>",
            "i" => "</i>",
            "sup" => "</sup>",
            "sub" => "</sub>",
            "c" | "t" => "</font>",
            "p" => "</font></i>",
            "*" => "</span>",
            "ex" => "</font></span>",
            _ => "",
        };
        self.output.push_str(html);
        self.reset_buf();
        Ok(Some(Lex::Root))
    }

    fn lex_ref_text(&mut self) -> Option<Lex> {
        if self.end() {
            return None;
        }
        let mut text = String::new();
        while !self.end() {
            let c = self.next();
            if c == '\\' {
                if self.end() {
                    break;
                }
                let n = self.next();
                text.push(n);
                continue;
            }
            if c == '[' {
                self.pos -= 1;
                break;
            }
            if c == '>' && self.follows(">") {
                self.next();
                break;
            }
            text.push(c);
        }
        let target = match self.attr("target") {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => text.clone(),
        };
        self.output.push_str(&format!(
            "<a href={}>{}</a>",
            quoteattr(&format!("bword://{}", target)),
            xml_escape(&text)
        ));
        self.reset_buf();
        Some(Lex::Root)
    }

    fn lex_url_text(&mut self) -> Option<Lex> {
        if self.end() {
            return None;
        }
        let mut text = String::new();
        while !self.end() {
            let c = self.next();
            if c == '\\' {
                if self.end() {
                    break;
                }
                let n = self.next();
                text.push(n);
                continue;
            }
            if c == '[' {
                self.pos -= 1;
                break;
            }
            text.push(c);
        }
        let mut target = match self.attr("target") {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => text.clone(),
        };
        if !target.contains("://") {
            target = format!("http://{}", target);
        }
        self.output.push_str(&format!(
            "<a href={}>{}</a>",
            quoteattr(&target),
            xml_escape(&text)
        ));
        self.reset_buf();
        Some(Lex::Root)
    }

    fn lex_s(&mut self) -> Option<Lex> {
        if self.end() {
            return None;
        }
        let mut fname = String::new();
        while !self.end() {
            let c = self.next();
            if c == '[' {
                self.pos -= 1;
                break;
            }
            fname.push(c);
        }
        // any letter case (Lingvo packs have "Apple.WAV"), and every sound /
        // picture kind dictz plays or shows (PyGlossary knew only wav / mp3)
        let ext = py_splitext_ext(&fname).to_lowercase();
        match ext.as_str() {
            e if crate::media::AUDIO_EXTENSIONS.contains(&e) => {
                if self.audio {
                    self.output.push_str(&format!(
                        "<object type=\"audio/x-wav\" data=\"{}\" width=\"40\" height=\"40\">\
                         <param name=\"autoplay\" value=\"false\" /></object>",
                        fname
                    ));
                }
            }
            e if crate::media::IMAGE_EXTENSIONS.contains(&e) => {
                self.output.push_str(&format!(
                    "<img align=\"top\" src=\"{}\" alt=\"{}\" />",
                    fname, fname
                ));
            }
            _ => {}
        }
        self.res_files.insert(fname);
        self.reset_buf();
        Some(Lex::Root)
    }
}

/// Extension as Python's `os.path.splitext(name)[1].lstrip(".")` (case kept).
fn py_splitext_ext(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let trimmed = base.trim_start_matches('.');
    match trimmed.rfind('.') {
        Some(i) => trimmed[i + 1..].to_string(),
        None => String::new(),
    }
}

// ------------------------------------------------------------ title

/// Headword line transformed like PyGlossary's `TitleTransformer`, except that
/// `output` / `output_alt` are unescaped text (`title` stays HTML).
pub struct TitleResult {
    /// Indexed headword (text in `(…)` kept, `{…}` dropped).
    pub output: String,
    /// Alternate headword (both `(…)` and `{…}` dropped).
    pub output_alt: String,
    /// HTML title shown above the article when it differs from the headword.
    pub title: String,
}

/// Transform one headword line. `None` when the line has a markup error.
pub fn transform_title(line: &str) -> Option<TitleResult> {
    let input: Vec<char> = line.chars().collect();
    let mut pos = 0usize;
    let mut r = TitleResult {
        output: String::new(),
        output_alt: String::new(),
        title: String::new(),
    };
    // add_text  -> output + title ;  add_text2 -> output + output_alt + title
    macro_rules! add_text {
        ($s:expr) => {{
            let s: &str = $s;
            r.output.push_str(s);
            r.title.push_str(&xml_escape(s));
        }};
    }
    macro_rules! add_text2 {
        ($s:expr) => {{
            let s: &str = $s;
            r.output.push_str(s);
            r.output_alt.push_str(s);
            r.title.push_str(&xml_escape(s));
        }};
    }
    loop {
        if pos >= input.len() {
            break;
        }
        let c = input[pos];
        pos += 1;
        if pos >= input.len() {
            add_text2!(&c.to_string());
            break;
        }
        match c {
            '\\' => {
                let n = input[pos];
                pos += 1;
                add_text2!(&n.to_string());
            }
            '(' => {
                let mut closed = false;
                while pos < input.len() {
                    let c = input[pos];
                    pos += 1;
                    if c == '\\' {
                        if pos >= input.len() {
                            break;
                        }
                        let n = input[pos];
                        pos += 1;
                        add_text!(&format!("\\{}", n));
                        continue;
                    }
                    if c == ')' {
                        closed = true;
                        break;
                    }
                    add_text!(&c.to_string());
                }
                if !closed {
                    break;
                }
            }
            '{' => {
                let start = pos;
                let mut closed = false;
                while pos < input.len() {
                    let c = input[pos];
                    pos += 1;
                    if c == '\\' {
                        if pos >= input.len() {
                            break;
                        }
                        pos += 1;
                        continue;
                    }
                    if c == '}' {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    break;
                }
                let inner: String = input[start..pos - 1].iter().collect();
                match Transformer::new(&inner, "").transform() {
                    Ok((html, _)) => r.title.push_str(&html),
                    Err(_) => return None,
                }
            }
            _ => add_text2!(&c.to_string()),
        }
    }
    Some(r)
}

/// Write a DSL file as UTF-16LE with BOM (used by tests and `--selftest`).
pub fn write_dsl_utf16(path: &Path, text: &str) -> std::io::Result<()> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(s: &str) -> String {
        Transformer::new(s, "key").transform().unwrap().0
    }

    #[test]
    fn markup_to_html() {
        assert_eq!(tr("[b]x[/b]"), "<b>x</b>");
        assert_eq!(tr("[m1]a[/m]\n\t[m2]b[/m]"), "<p style=\"padding-left:1em;margin:0\">a</p><p style=\"padding-left:2em;margin:0\">b</p>");
        assert_eq!(tr("a\n\tb"), "a<br/>b");
        assert_eq!(tr("[c red]x[/c]"), "<font color=\"red\">x</font>");
        assert_eq!(tr("[c]x[/c]"), "<font color=\"green\">x</font>");
        assert_eq!(tr("[ref]go[/ref]"), "<a href=\"bword://go\">go</a>");
        assert_eq!(tr("<<go on>>"), "<a href=\"bword://go on\">go on</a>");
        assert_eq!(tr("[url]x.org[/url]"), "<a href=\"http://x.org\">x.org</a>");
        assert_eq!(tr("~ x"), "key x");
        assert_eq!(tr("a\\[b\\] c"), "a[b] c");
        assert_eq!(tr("{{comment}}a"), "a");
        assert_eq!(tr("[trn]elma[/trn]"), "elma");
        assert_eq!(
            tr("[s]pic.png[/s]"),
            "<img align=\"top\" src=\"pic.png\" alt=\"pic.png\" />"
        );
        assert!(tr("[s]a.wav[/s]").starts_with("<object type=\"audio/x-wav\" data=\"a.wav\""));
        // any case, more sound and picture kinds (v0.2.2)
        for f in [
            "A.WAV", "b.Mp3", "c.ogg", "d.spx", "e.opus", "f.m4a", "g.flac",
        ] {
            assert!(
                tr(&format!("[s]{f}[/s]"))
                    .starts_with(&format!("<object type=\"audio/x-wav\" data=\"{f}\"")),
                "{f}"
            );
        }
        assert!(tr("[s]p.WEBP[/s]").starts_with("<img align=\"top\" src=\"p.WEBP\""));
        assert_eq!(tr("[s]notes.txt[/s]"), "");
        assert!(Transformer::new("[b", "").transform().is_err());
    }

    #[test]
    fn titles() {
        let t = transform_title("go (to) {[']}x{[/']}\n").unwrap();
        assert_eq!(py_strip(&t.output), "go to x");
        assert_eq!(py_strip(&t.output_alt), "go  x");
        assert_eq!(py_strip(&t.title), "go to <u class=\"accent\">x</u>");
        let t = transform_title("rock & roll\n").unwrap();
        assert_eq!(py_strip(&t.output), "rock & roll");
        assert_eq!(py_strip(&t.title), "rock &amp; roll");
    }

    #[test]
    fn whole_file() {
        let text = "\u{feff}#NAME \"Tiny\"\n#INDEX_LANGUAGE \"English\"\n#CONTENTS_LANGUAGE \"Turkish\"\n\napple\n\t[trn]elma[/trn]\n\nbook\nbooks\n\t[m1]kitap[/m]\n";
        let p = parse_dsl(text);
        assert_eq!(p.name, "Tiny");
        assert_eq!(p.index_language, "English");
        assert_eq!(p.entries.len(), 2);
        assert_eq!(
            p.entries[1].0,
            vec!["book".to_string(), "books".to_string()]
        );
    }

    #[test]
    fn load_utf16_file() {
        let d = std::env::temp_dir().join(format!("dictz-dsl-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("t.dsl");
        write_dsl_utf16(&f, "#NAME \"T\"\n\napple\n\t[trn]elma[/trn]\n").unwrap();
        let mut dsl = DslDictionary::new(&f);
        dsl.load().unwrap();
        assert_eq!(dsl.metadata().name, "T");
        assert_eq!(dsl.lookup("apple", true)[0].definition, "elma");
    }

    #[test]
    fn stem_strips_dsl_dz() {
        assert_eq!(stem(Path::new("/x/En-Tr.dsl.dz")), "En-Tr");
        assert_eq!(stem(Path::new("/x/En-Tr.dsl")), "En-Tr");
    }
}
