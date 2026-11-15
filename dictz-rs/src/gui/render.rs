//! Drawing parsed article HTML with egui widgets.

use eframe::egui::{self, Align, Color32, CursorIcon, FontFamily, FontId, RichText, Sense};

use crate::html::{Inline, Line, Rgb};

use super::fonts::BOLD;

/// What the user clicked inside an article.
#[derive(Debug, Clone)]
pub enum Click {
    /// A link (`bword://…`, `http…`).
    Link(String),
    /// A sound file.
    Audio(String),
}

/// Media lookup used while drawing: returns image bytes for a file name.
pub trait MediaSource {
    /// Bytes of the picture `name`, if it can be found.
    fn image(&mut self, name: &str) -> Option<std::sync::Arc<[u8]>>;
}

/// Drawing options (from the preferences).
pub struct Opts {
    /// Base font size in points (zoom applied).
    pub size: f32,
    /// Extra space between wrapped rows, in points.
    pub row_gap: f32,
    /// Regular text font family.
    pub family: FontFamily,
    /// Draw pictures (otherwise `[name]`).
    pub show_images: bool,
    /// Picture height limit in points.
    pub max_image_height: f32,
    /// Find-in-page text, lowercased chars (empty = no highlighting).
    pub find: Vec<char>,
    /// Highlight background.
    pub highlight: Color32,
}

/// Find-in-page state shared across one frame's drawing.
#[derive(Default)]
pub struct Find {
    /// Matches drawn so far in this frame.
    pub counter: usize,
    /// Index of the current match.
    pub current: usize,
    /// Scroll the current match into view this frame.
    pub scroll: bool,
}

fn adjust(c: Rgb, dark: bool) -> Color32 {
    let [r, g, b] = c;
    let lum = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0;
    if dark && lum < 0.45 {
        // lift dark colors so they stay readable on a dark background
        let mix = |v: u8| (v as f32 + (255.0 - v as f32) * 0.55) as u8;
        Color32::from_rgb(mix(r), mix(g), mix(b))
    } else if !dark && lum > 0.85 {
        let mix = |v: u8| (v as f32 * 0.6) as u8;
        Color32::from_rgb(mix(r), mix(g), mix(b))
    } else {
        Color32::from_rgb(r, g, b)
    }
}

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Lowercased chars of `s` (one char per char, so indices line up).
pub fn lower_chars(s: &str) -> Vec<char> {
    s.chars().map(lower).collect()
}

/// Case-insensitive, non-overlapping matches of `needle` in `text` as
/// (start, end) char indices.
pub fn find_ranges(text: &str, needle: &[char]) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    let hay = lower_chars(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == *needle {
            out.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Number of matches of `needle` in `lines`.
pub fn count_matches(lines: &[Line], needle: &[char]) -> usize {
    lines
        .iter()
        .flat_map(|l| l.items.iter())
        .map(|i| match i {
            Inline::Text(t, _) => find_ranges(t, needle).len(),
            _ => 0,
        })
        .sum()
}

/// Split `text` into (segment, is_match) pieces.
fn segments(text: &str, needle: &[char]) -> Vec<(String, bool)> {
    let ranges = find_ranges(text, needle);
    if ranges.is_empty() {
        return vec![(text.to_string(), false)];
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut pos = 0;
    for (s, e) in ranges {
        if s > pos {
            out.push((chars[pos..s].iter().collect(), false));
        }
        out.push((chars[s..e].iter().collect(), true));
        pos = e;
    }
    if pos < chars.len() {
        out.push((chars[pos..].iter().collect(), false));
    }
    out
}

/// Draw `text` (one style) with find highlighting; returns the last response.
pub fn highlighted(
    ui: &mut egui::Ui,
    text: &str,
    style: impl Fn(RichText) -> RichText,
    opts: &Opts,
    find: &mut Find,
    sense: Option<Sense>,
) -> Option<egui::Response> {
    let mut last: Option<egui::Response> = None;
    for (seg, hit) in segments(text, &opts.find) {
        let mut rt = style(RichText::new(seg));
        let mut is_current = false;
        if hit {
            is_current = find.counter == find.current;
            let bg = if is_current {
                opts.highlight
            } else {
                opts.highlight.gamma_multiply(0.45)
            };
            rt = rt.background_color(bg).color(Color32::BLACK);
            find.counter += 1;
        }
        let mut label = egui::Label::new(rt);
        if let Some(s) = sense {
            label = label.sense(s);
        }
        let resp = ui.add(label);
        if is_current && find.scroll {
            resp.scroll_to_me(Some(Align::Center));
            find.scroll = false;
        }
        last = Some(match &last {
            Some(prev) => resp.union(prev.clone()),
            None => resp,
        });
    }
    last
}

/// Draw `lines`.
pub fn lines(
    ui: &mut egui::Ui,
    lines: &[Line],
    opts: &Opts,
    dict_key: &str,
    media: &mut dyn MediaSource,
    find: &mut Find,
    clicks: &mut Vec<Click>,
) {
    let size = opts.size;
    let dark = ui.visuals().dark_mode;
    let link_color = ui.visuals().hyperlink_color;
    for (li, line) in lines.iter().enumerate() {
        if line.items.is_empty() {
            ui.add_space(size * 0.5);
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, opts.row_gap);
            if line.indent > 0.0 {
                ui.add_space(line.indent * size);
            }
            for (ii, item) in line.items.iter().enumerate() {
                match item {
                    Inline::Text(text, st) => {
                        let fsize = size * st.scale;
                        let family = if st.monospace {
                            FontFamily::Monospace
                        } else if st.bold {
                            FontFamily::Name(BOLD.into())
                        } else {
                            opts.family.clone()
                        };
                        let link = st.link.as_ref().filter(|h| !h.starts_with("sound://"));
                        let style = |mut rt: RichText| {
                            rt = rt.font(FontId::new(fsize, family.clone()));
                            if st.bold {
                                rt = rt.strong();
                            }
                            if st.italic {
                                rt = rt.italics();
                            }
                            if st.underline || link.is_some() {
                                rt = rt.underline();
                            }
                            if st.strike {
                                rt = rt.strikethrough();
                            }
                            if st.sup {
                                rt = rt.raised();
                            }
                            if let Some(c) = st.color {
                                rt = rt.color(adjust(c, dark));
                            } else if link.is_some() {
                                rt = rt.color(link_color);
                            }
                            rt
                        };
                        match link {
                            Some(href) => {
                                if let Some(resp) =
                                    highlighted(ui, text, style, opts, find, Some(Sense::click()))
                                {
                                    let resp = resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(href.as_str());
                                    if resp.clicked() {
                                        clicks.push(Click::Link(href.clone()));
                                    }
                                }
                            }
                            None => {
                                highlighted(ui, text, style, opts, find, None);
                            }
                        }
                    }
                    Inline::Image(src) => {
                        let bytes = if opts.show_images {
                            media.image(src)
                        } else {
                            None
                        };
                        match bytes {
                            Some(bytes) => {
                                let uri = format!("bytes://dictz/{}/{}", dict_key, src);
                                ui.add(
                                    egui::Image::from_bytes(uri, egui::load::Bytes::Shared(bytes))
                                        .fit_to_original_size(1.0)
                                        .max_height(opts.max_image_height)
                                        .max_width(ui.available_width().max(64.0)),
                                )
                                .on_hover_text(src.as_str());
                            }
                            None => {
                                ui.label(
                                    RichText::new(format!("[{}]", src)).weak().size(size * 0.85),
                                )
                                .on_hover_text(
                                    if opts.show_images {
                                        "resim bulunamadı"
                                    } else {
                                        "resimler kapalı (Tercihler → Makale)"
                                    },
                                );
                            }
                        }
                    }
                    Inline::Audio(src) => {
                        let id = egui::Id::new(("audio", dict_key, li, ii, src));
                        let b = egui::Button::new(
                            RichText::new(format!("🔊 {}", short(src))).size(size * 0.85),
                        )
                        .small();
                        if ui
                            .push_id(id, |ui| ui.add(b))
                            .inner
                            .on_hover_text("Sesi çal")
                            .clicked()
                        {
                            clicks.push(Click::Audio(src.clone()));
                        }
                    }
                }
            }
        });
    }
}

/// First sound referenced in `lines`.
pub fn first_audio(lines: &[Line]) -> Option<&str> {
    lines
        .iter()
        .flat_map(|l| l.items.iter())
        .find_map(|i| match i {
            Inline::Audio(s) => Some(s.as_str()),
            Inline::Text(_, st) => st.link.as_deref().and_then(|h| h.strip_prefix("sound://")),
            _ => None,
        })
}

fn short(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        let n = lower_chars("an");
        assert_eq!(find_ranges("Banana", &n), vec![(1, 3), (3, 5)]);
        assert_eq!(find_ranges("İstanbul", &lower_chars("ist")).len(), 1);
        assert!(find_ranges("abc", &[]).is_empty());
        let s = segments("xAnx", &n);
        assert_eq!(
            s,
            vec![
                ("x".into(), false),
                ("An".into(), true),
                ("x".into(), false)
            ]
        );
    }
}
