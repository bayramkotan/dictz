//! A small HTML → styled-lines converter for dictionary articles.
//!
//! Dictionary definitions use a limited HTML subset (bold, italics, colors,
//! indented paragraphs, links, pictures, sounds). The GUI draws the result of
//! [`parse`] with native widgets; no browser engine is needed.

use crate::text::entity_codepoint;

/// RGB color.
pub type Rgb = [u8; 3];

/// Inline text style.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
    /// Struck through.
    pub strike: bool,
    /// Monospace.
    pub monospace: bool,
    /// Superscript.
    pub sup: bool,
    /// Subscript.
    pub sub: bool,
    /// Text color.
    pub color: Option<Rgb>,
    /// Link target (`bword://word`, `http://…`, `sound://…`).
    pub link: Option<String>,
    /// Relative size (1.0 = normal).
    pub scale: f32,
}

/// One inline item.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    /// A run of text.
    Text(String, Style),
    /// A picture (`<img src>`).
    Image(String),
    /// A sound (`<object data>` / `sound://` link).
    Audio(String),
}

/// One line (block) of an article.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line {
    /// Left indent in em.
    pub indent: f32,
    /// Content.
    pub items: Vec<Inline>,
}

impl Line {
    /// Plain text of the line.
    pub fn text(&self) -> String {
        self.items
            .iter()
            .map(|i| match i {
                Inline::Text(t, _) => t.as_str(),
                _ => "",
            })
            .collect()
    }
}

/// `true` when a definition should be treated as HTML (same rule as the
/// Python edition).
pub fn looks_like_html(s: &str) -> bool {
    s.contains('<') && s.contains('>')
}

/// Plain text → lines (newlines kept).
pub fn plain_to_lines(text: &str) -> Vec<Line> {
    text.split('\n')
        .map(|l| Line {
            indent: 0.0,
            items: if l.is_empty() {
                vec![]
            } else {
                vec![Inline::Text(
                    l.to_string(),
                    Style {
                        scale: 1.0,
                        ..Default::default()
                    },
                )]
            },
        })
        .collect()
}

/// Decode `&name;`, `&#NN;`, `&#xHH;`.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail[1..]
            .find(|c: char| c == ';' || c == '&' || c.is_whitespace() || c == '<')
            .map(|p| p + 1);
        let decoded = end
            .filter(|&e| tail.as_bytes().get(e) == Some(&b';'))
            .and_then(|e| {
                let name = &tail[1..e];
                let cp =
                    if let Some(h) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                        u32::from_str_radix(h, 16).ok()
                    } else if let Some(d) = name.strip_prefix('#') {
                        d.parse().ok()
                    } else {
                        entity_codepoint(name).or_else(|| entity_codepoint(&name.to_lowercase()))
                    };
                cp.and_then(char::from_u32).map(|c| (c, e + 1))
            });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &tail[len..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parse a CSS / HTML color.
pub fn parse_color(s: &str) -> Option<Rgb> {
    let s = s
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_ascii_lowercase();
    if let Some(h) = s.strip_prefix('#') {
        let v: Vec<u8> = h
            .chars()
            .filter_map(|c| c.to_digit(16).map(|d| d as u8))
            .collect();
        return match v.len() {
            3 => Some([v[0] * 17, v[1] * 17, v[2] * 17]),
            6 => Some([v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5]]),
            _ => None,
        };
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|x| x.strip_suffix(')')) {
        let p: Vec<u8> = inner
            .split(',')
            .filter_map(|x| x.trim().parse().ok())
            .collect();
        if p.len() == 3 {
            return Some([p[0], p[1], p[2]]);
        }
    }
    CSS_COLORS
        .binary_search_by(|(k, _)| (*k).cmp(s.as_str()))
        .ok()
        .map(|i| {
            let v = CSS_COLORS[i].1;
            [(v >> 16) as u8, (v >> 8) as u8, v as u8]
        })
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        while i < c.len() && (c[i].is_whitespace() || c[i] == '/') {
            i += 1;
        }
        let start = i;
        while i < c.len() && !c[i].is_whitespace() && c[i] != '=' && c[i] != '/' {
            i += 1;
        }
        if start == i {
            i += 1;
            continue;
        }
        let name: String = c[start..i].iter().collect::<String>().to_ascii_lowercase();
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < c.len() && c[i] == '=' {
            i += 1;
            while i < c.len() && c[i].is_whitespace() {
                i += 1;
            }
            if i < c.len() && (c[i] == '"' || c[i] == '\'') {
                let q = c[i];
                i += 1;
                let vs = i;
                while i < c.len() && c[i] != q {
                    i += 1;
                }
                value = c[vs..i.min(c.len())].iter().collect();
                i += 1;
            } else {
                let vs = i;
                while i < c.len() && !c[i].is_whitespace() {
                    i += 1;
                }
                value = c[vs..i].iter().collect();
            }
        }
        out.push((name, decode_entities(&value)));
    }
    out
}

fn style_prop(style: &str, prop: &str) -> Option<String> {
    style.split(';').find_map(|decl| {
        let (k, v) = decl.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(prop)).then(|| v.trim().to_string())
    })
}

fn css_length_em(v: &str) -> Option<f32> {
    let v = v.trim();
    if let Some(n) = v.strip_suffix("em") {
        n.trim().parse().ok()
    } else if let Some(n) = v.strip_suffix("px") {
        n.trim().parse::<f32>().ok().map(|p| p / 16.0)
    } else if let Some(n) = v.strip_suffix("pt") {
        n.trim().parse::<f32>().ok().map(|p| p / 12.0)
    } else {
        v.parse::<f32>().ok().map(|p| p / 16.0)
    }
}

const BLOCK_TAGS: &[&str] = &[
    "p",
    "div",
    "li",
    "ul",
    "ol",
    "dl",
    "dd",
    "dt",
    "blockquote",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "tr",
    "table",
    "pre",
    "hr",
    "center",
    "section",
    "article",
    "header",
    "footer",
];
const SKIP_TAGS: &[&str] = &["script", "style", "head", "title"];

struct Frame {
    tag: String,
    style: Style,
    indent: f32,
    block: bool,
}

struct Builder {
    lines: Vec<Line>,
    cur: Line,
    pending_space: bool,
    pre: usize,
}

impl Builder {
    fn flush(&mut self, force: bool, indent: f32) {
        if !self.cur.items.is_empty() || force {
            let mut line = std::mem::take(&mut self.cur);
            // drop trailing spaces
            if let Some(Inline::Text(t, _)) = line.items.last_mut() {
                let trimmed = t.trim_end_matches(' ').len();
                t.truncate(trimmed);
            }
            self.lines.push(line);
        }
        self.cur.indent = indent;
        self.pending_space = false;
    }

    fn push_text(&mut self, text: &str, style: &Style) {
        let mut buf = String::new();
        if self.pre > 0 {
            let mut first = true;
            for part in text.split('\n') {
                if !first {
                    if !buf.is_empty() {
                        self.push_run(std::mem::take(&mut buf), style);
                    }
                    let ind = self.cur.indent;
                    self.flush(true, ind);
                }
                first = false;
                buf.push_str(part);
            }
            if !buf.is_empty() {
                self.push_run(buf, style);
            }
            return;
        }
        for ch in text.chars() {
            if ch.is_whitespace() && ch != '\u{a0}' {
                self.pending_space = true;
            } else {
                if self.pending_space && !buf.is_empty() {
                    buf.push(' ');
                } else if self.pending_space && !self.cur.items.is_empty() {
                    // keep the space outside links / styled runs that follow
                    let plain = Style {
                        scale: 1.0,
                        ..Default::default()
                    };
                    match self.cur.items.last_mut() {
                        Some(Inline::Text(t, s))
                            if s.link.is_none() && !s.underline && !s.strike =>
                        {
                            t.push(' ')
                        }
                        _ => self.cur.items.push(Inline::Text(" ".into(), plain)),
                    }
                }
                self.pending_space = false;
                buf.push(if ch == '\u{a0}' { ' ' } else { ch });
            }
        }
        if !buf.is_empty() {
            self.push_run(buf, style);
        }
    }

    fn push_run(&mut self, text: String, style: &Style) {
        if let Some(Inline::Text(t, s)) = self.cur.items.last_mut() {
            if s == style {
                t.push_str(&text);
                return;
            }
        }
        self.cur.items.push(Inline::Text(text, style.clone()));
    }

    fn push_item(&mut self, item: Inline) {
        if self.pending_space && !self.cur.items.is_empty() {
            self.cur.items.push(Inline::Text(
                " ".into(),
                Style {
                    scale: 1.0,
                    ..Default::default()
                },
            ));
        }
        self.pending_space = false;
        self.cur.items.push(item);
    }
}

/// Parse dictionary HTML into lines.
pub fn parse(html: &str) -> Vec<Line> {
    let base = Style {
        scale: 1.0,
        ..Default::default()
    };
    let mut stack: Vec<Frame> = vec![Frame {
        tag: String::new(),
        style: base,
        indent: 0.0,
        block: true,
    }];
    let mut b = Builder {
        lines: Vec::new(),
        cur: Line::default(),
        pending_space: false,
        pre: 0,
    };
    let mut skip_depth: Option<String> = None;
    let mut i = 0usize;
    let bytes = html.as_bytes();
    while i < html.len() {
        if bytes[i] == b'<' {
            if html[i..].starts_with("<!--") {
                i = html[i + 4..]
                    .find("-->")
                    .map(|p| i + 4 + p + 3)
                    .unwrap_or(html.len());
                continue;
            }
            let Some(close) = html[i..].find('>') else {
                // stray '<' — treat as text
                if skip_depth.is_none() {
                    let st = stack.last().unwrap().style.clone();
                    b.push_text("<", &st);
                }
                i += 1;
                continue;
            };
            let raw = &html[i + 1..i + close];
            i += close + 1;
            let raw_trim = raw.trim();
            if raw_trim.starts_with('!') || raw_trim.starts_with('?') {
                continue;
            }
            let closing = raw_trim.starts_with('/');
            let body = raw_trim.trim_start_matches('/');
            let name_end = body
                .find(|c: char| c.is_whitespace() || c == '/')
                .unwrap_or(body.len());
            let name = body[..name_end].to_ascii_lowercase();
            if name.is_empty() || !name.chars().next().unwrap().is_ascii_alphabetic() {
                if skip_depth.is_none() {
                    let st = stack.last().unwrap().style.clone();
                    b.push_text(&format!("<{}>", raw), &st);
                }
                continue;
            }
            if let Some(sk) = &skip_depth {
                if closing && *sk == name {
                    skip_depth = None;
                }
                continue;
            }
            if SKIP_TAGS.contains(&name.as_str()) && !closing {
                if !raw_trim.ends_with('/') {
                    skip_depth = Some(name);
                }
                continue;
            }
            let attrs = parse_attrs(&body[name_end..]);
            let attr = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
            let cur_indent = stack.last().unwrap().indent;
            if closing {
                if let Some(pos) = stack.iter().rposition(|f| f.tag == name) {
                    if pos > 0 {
                        let was_block = stack[pos].block;
                        stack.truncate(pos);
                        if name == "pre" {
                            b.pre = b.pre.saturating_sub(1);
                        }
                        if was_block {
                            let ind = stack.last().unwrap().indent;
                            b.flush(false, ind);
                        }
                    }
                }
                continue;
            }
            match name.as_str() {
                "br" => {
                    b.flush(true, cur_indent);
                    continue;
                }
                "hr" => {
                    b.flush(false, cur_indent);
                    b.cur.items.push(Inline::Text(
                        "────────".into(),
                        Style {
                            scale: 1.0,
                            color: Some([150, 150, 150]),
                            ..Default::default()
                        },
                    ));
                    b.flush(false, cur_indent);
                    continue;
                }
                "img" => {
                    if let Some(src) = attr("src").filter(|s| !s.is_empty()) {
                        b.push_item(Inline::Image(src));
                    }
                    continue;
                }
                "object" | "embed" | "audio" | "source" => {
                    if let Some(src) = attr("data")
                        .or_else(|| attr("src"))
                        .filter(|s| !s.is_empty())
                    {
                        b.push_item(Inline::Audio(src));
                    }
                    if name == "object" || name == "audio" {
                        skip_depth = Some(name);
                    }
                    continue;
                }
                _ => {}
            }
            let self_closing = raw_trim.ends_with('/');
            let mut st = stack.last().unwrap().style.clone();
            let mut indent = cur_indent;
            let block = BLOCK_TAGS.contains(&name.as_str());
            match name.as_str() {
                "b" | "strong" | "th" => st.bold = true,
                "i" | "em" | "cite" | "var" | "dfn" => st.italic = true,
                "u" | "ins" => st.underline = true,
                "s" | "strike" | "del" => st.strike = true,
                "code" | "tt" | "kbd" | "samp" | "pre" => st.monospace = true,
                "sup" => {
                    st.sup = true;
                    st.scale *= 0.75;
                }
                "sub" => {
                    st.sub = true;
                    st.scale *= 0.75;
                }
                "small" => st.scale *= 0.85,
                "big" => st.scale *= 1.2,
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    st.bold = true;
                    st.scale *= match name.as_str() {
                        "h1" => 1.6,
                        "h2" => 1.4,
                        "h3" => 1.2,
                        _ => 1.05,
                    };
                }
                "a" => {
                    if let Some(h) = attr("href") {
                        st.link = Some(h);
                    }
                }
                "blockquote" | "dd" => indent += 2.0,
                "ul" | "ol" => indent += 1.5,
                _ => {}
            }
            if let Some(c) = attr("color").and_then(|c| parse_color(&c)) {
                st.color = Some(c);
            }
            if let Some(style) = attr("style") {
                if let Some(c) = style_prop(&style, "color").and_then(|c| parse_color(&c)) {
                    st.color = Some(c);
                }
                if style_prop(&style, "font-weight")
                    .map(|w| w == "bold" || w.parse::<u32>().map(|n| n >= 600).unwrap_or(false))
                    == Some(true)
                {
                    st.bold = true;
                }
                if style_prop(&style, "font-style").as_deref() == Some("italic") {
                    st.italic = true;
                }
                for prop in ["padding-left", "margin-left", "text-indent"] {
                    if let Some(em) = style_prop(&style, prop).and_then(|v| css_length_em(&v)) {
                        if prop != "text-indent" {
                            indent += em;
                        }
                    }
                }
            }
            if block {
                if name == "p" {
                    // an open <p> is closed by a new block
                    if let Some(pos) = stack.iter().rposition(|f| f.tag == "p") {
                        if pos > 0 {
                            stack.truncate(pos);
                            let base_indent = stack.last().unwrap().indent;
                            let extra = indent - cur_indent;
                            indent = base_indent + extra;
                        }
                    }
                }
                b.flush(false, indent);
                if name == "pre" {
                    b.pre += 1;
                }
                if name == "li" {
                    let bullet_style = Style {
                        scale: 1.0,
                        ..Default::default()
                    };
                    b.cur.items.push(Inline::Text("• ".into(), bullet_style));
                }
            }
            if name == "a" {
                if let Some(h) = st.link.as_deref() {
                    if h.starts_with("sound://") {
                        b.push_item(Inline::Audio(h.trim_start_matches("sound://").to_string()));
                    }
                }
            }
            if !self_closing {
                stack.push(Frame {
                    tag: name,
                    style: st,
                    indent,
                    block,
                });
            }
        } else {
            let next = html[i..].find('<').map(|p| i + p).unwrap_or(html.len());
            if skip_depth.is_none() {
                let text = decode_entities(&html[i..next]);
                let st = stack.last().unwrap().style.clone();
                b.push_text(&text, &st);
            }
            i = next;
        }
    }
    let ind = b.cur.indent;
    b.flush(false, ind);
    // drop leading/trailing empty lines and collapse runs of empty lines
    let mut out: Vec<Line> = Vec::new();
    for l in b.lines {
        if l.items.is_empty()
            && out
                .last()
                .map(|p: &Line| p.items.is_empty())
                .unwrap_or(true)
        {
            continue;
        }
        out.push(l);
    }
    while out.last().map(|l| l.items.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out
}

/// CSS named colors.
static CSS_COLORS: &[(&str, u32)] = &[
    ("aliceblue", 0xF0F8FF),
    ("antiquewhite", 0xFAEBD7),
    ("aqua", 0x00FFFF),
    ("aquamarine", 0x7FFFD4),
    ("azure", 0xF0FFFF),
    ("beige", 0xF5F5DC),
    ("bisque", 0xFFE4C4),
    ("black", 0x000000),
    ("blanchedalmond", 0xFFEBCD),
    ("blue", 0x0000FF),
    ("blueviolet", 0x8A2BE2),
    ("brown", 0xA52A2A),
    ("burlywood", 0xDEB887),
    ("cadetblue", 0x5F9EA0),
    ("chartreuse", 0x7FFF00),
    ("chocolate", 0xD2691E),
    ("coral", 0xFF7F50),
    ("cornflowerblue", 0x6495ED),
    ("cornsilk", 0xFFF8DC),
    ("crimson", 0xDC143C),
    ("cyan", 0x00FFFF),
    ("darkblue", 0x00008B),
    ("darkcyan", 0x008B8B),
    ("darkgoldenrod", 0xB8860B),
    ("darkgray", 0xA9A9A9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xA9A9A9),
    ("darkkhaki", 0xBDB76B),
    ("darkmagenta", 0x8B008B),
    ("darkolivegreen", 0x556B2F),
    ("darkorange", 0xFF8C00),
    ("darkorchid", 0x9932CC),
    ("darkred", 0x8B0000),
    ("darksalmon", 0xE9967A),
    ("darkseagreen", 0x8FBC8F),
    ("darkslateblue", 0x483D8B),
    ("darkslategray", 0x2F4F4F),
    ("darkslategrey", 0x2F4F4F),
    ("darkturquoise", 0x00CED1),
    ("darkviolet", 0x9400D3),
    ("deeppink", 0xFF1493),
    ("deepskyblue", 0x00BFFF),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1E90FF),
    ("firebrick", 0xB22222),
    ("floralwhite", 0xFFFAF0),
    ("forestgreen", 0x228B22),
    ("fuchsia", 0xFF00FF),
    ("gainsboro", 0xDCDCDC),
    ("ghostwhite", 0xF8F8FF),
    ("gold", 0xFFD700),
    ("goldenrod", 0xDAA520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xADFF2F),
    ("grey", 0x808080),
    ("honeydew", 0xF0FFF0),
    ("hotpink", 0xFF69B4),
    ("indianred", 0xCD5C5C),
    ("indigo", 0x4B0082),
    ("ivory", 0xFFFFF0),
    ("khaki", 0xF0E68C),
    ("lavender", 0xE6E6FA),
    ("lavenderblush", 0xFFF0F5),
    ("lawngreen", 0x7CFC00),
    ("lemonchiffon", 0xFFFACD),
    ("lightblue", 0xADD8E6),
    ("lightcoral", 0xF08080),
    ("lightcyan", 0xE0FFFF),
    ("lightgoldenrodyellow", 0xFAFAD2),
    ("lightgray", 0xD3D3D3),
    ("lightgreen", 0x90EE90),
    ("lightgrey", 0xD3D3D3),
    ("lightpink", 0xFFB6C1),
    ("lightsalmon", 0xFFA07A),
    ("lightseagreen", 0x20B2AA),
    ("lightskyblue", 0x87CEFA),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xB0C4DE),
    ("lightyellow", 0xFFFFE0),
    ("lime", 0x00FF00),
    ("limegreen", 0x32CD32),
    ("linen", 0xFAF0E6),
    ("magenta", 0xFF00FF),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66CDAA),
    ("mediumblue", 0x0000CD),
    ("mediumorchid", 0xBA55D3),
    ("mediumpurple", 0x9370DB),
    ("mediumseagreen", 0x3CB371),
    ("mediumslateblue", 0x7B68EE),
    ("mediumspringgreen", 0x00FA9A),
    ("mediumturquoise", 0x48D1CC),
    ("mediumvioletred", 0xC71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xF5FFFA),
    ("mistyrose", 0xFFE4E1),
    ("moccasin", 0xFFE4B5),
    ("navajowhite", 0xFFDEAD),
    ("navy", 0x000080),
    ("oldlace", 0xFDF5E6),
    ("olive", 0x808000),
    ("olivedrab", 0x6B8E23),
    ("orange", 0xFFA500),
    ("orangered", 0xFF4500),
    ("orchid", 0xDA70D6),
    ("palegoldenrod", 0xEEE8AA),
    ("palegreen", 0x98FB98),
    ("paleturquoise", 0xAFEEEE),
    ("palevioletred", 0xDB7093),
    ("papayawhip", 0xFFEFD5),
    ("peachpuff", 0xFFDAB9),
    ("peru", 0xCD853F),
    ("pink", 0xFFC0CB),
    ("plum", 0xDDA0DD),
    ("powderblue", 0xB0E0E6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xFF0000),
    ("rosybrown", 0xBC8F8F),
    ("royalblue", 0x4169E1),
    ("saddlebrown", 0x8B4513),
    ("salmon", 0xFA8072),
    ("sandybrown", 0xF4A460),
    ("seagreen", 0x2E8B57),
    ("seashell", 0xFFF5EE),
    ("sienna", 0xA0522D),
    ("silver", 0xC0C0C0),
    ("skyblue", 0x87CEEB),
    ("slateblue", 0x6A5ACD),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xFFFAFA),
    ("springgreen", 0x00FF7F),
    ("steelblue", 0x4682B4),
    ("tan", 0xD2B48C),
    ("teal", 0x008080),
    ("thistle", 0xD8BFD8),
    ("tomato", 0xFF6347),
    ("turquoise", 0x40E0D0),
    ("violet", 0xEE82EE),
    ("wheat", 0xF5DEB3),
    ("white", 0xFFFFFF),
    ("whitesmoke", 0xF5F5F5),
    ("yellow", 0xFFFF00),
    ("yellowgreen", 0x9ACD32),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(parse_color("#f00"), Some([255, 0, 0]));
        assert_eq!(parse_color("#00ff80"), Some([0, 255, 128]));
        assert_eq!(parse_color("SteelBlue"), Some([0x46, 0x82, 0xB4]));
        assert_eq!(parse_color("\"blue\""), Some([0, 0, 255]));
        assert!(CSS_COLORS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn entities() {
        assert_eq!(
            decode_entities("a &amp; b &lt; &#65;&#x42; &nbsp;x &foo; &"),
            "a & b < AB \u{a0}x &foo; &"
        );
    }

    #[test]
    fn basic_structure() {
        let lines = parse(
            "<b>apple</b> <i>n.</i><br/>elma<p style=\"padding-left:2em;margin:0\">meyve</p>",
        );
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text(), "apple n.");
        assert!(matches!(&lines[0].items[0], Inline::Text(t, s) if t.trim() == "apple" && s.bold));
        assert_eq!(lines[1].text(), "elma");
        assert_eq!(lines[2].indent, 2.0);
    }

    #[test]
    fn links_media_colors() {
        let l = parse("<a href=\"bword://go\">go</a> <font color=\"red\">x</font><img src=\"p.png\"/><object type=\"audio/x-wav\" data=\"s.wav\"><param name=\"autoplay\"/></object>");
        let items = &l[0].items;
        assert!(
            matches!(&items[0], Inline::Text(t, s) if t == "go" && s.link.as_deref() == Some("bword://go"))
        );
        assert!(items
            .iter()
            .any(|i| matches!(i, Inline::Text(t, s) if t == "x" && s.color == Some([255, 0, 0]))));
        assert!(items.contains(&Inline::Image("p.png".into())));
        assert!(items.contains(&Inline::Audio("s.wav".into())));
    }

    #[test]
    fn unclosed_paragraphs_and_skip() {
        let l = parse("<style>b{}</style><p>a<p>b</p><script>x</script>");
        assert_eq!(l.iter().map(Line::text).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn plain() {
        let l = plain_to_lines("a\n\nb");
        assert_eq!(l.len(), 3);
        assert!(!looks_like_html("a < b"));
        assert!(looks_like_html("<b>x</b>"));
    }
}
