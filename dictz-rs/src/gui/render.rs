//! Drawing parsed article HTML with egui widgets.

use eframe::egui::{self, Color32, CursorIcon, FontFamily, FontId, RichText, Sense};

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

/// Draw `lines`. `size` is the base font size in points.
pub fn lines(
    ui: &mut egui::Ui,
    lines: &[Line],
    size: f32,
    dict_key: &str,
    media: &mut dyn MediaSource,
    clicks: &mut Vec<Click>,
) {
    let dark = ui.visuals().dark_mode;
    let link_color = ui.visuals().hyperlink_color;
    for (li, line) in lines.iter().enumerate() {
        if line.items.is_empty() {
            ui.add_space(size * 0.5);
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
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
                            FontFamily::Proportional
                        };
                        let mut rt = RichText::new(text).font(FontId::new(fsize, family));
                        if st.bold {
                            rt = rt.strong();
                        }
                        if st.italic {
                            rt = rt.italics();
                        }
                        if st.underline {
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
                        }
                        if let Some(href) = &st.link {
                            if !href.starts_with("sound://") {
                                if st.color.is_none() {
                                    rt = rt.color(link_color);
                                }
                                rt = rt.underline();
                                let resp = ui
                                    .add(egui::Label::new(rt).sense(Sense::click()))
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(href.as_str());
                                if resp.clicked() {
                                    clicks.push(Click::Link(href.clone()));
                                }
                                continue;
                            }
                        }
                        ui.add(egui::Label::new(rt));
                    }
                    Inline::Image(src) => match media.image(src) {
                        Some(bytes) => {
                            let uri = format!("bytes://dictz/{}/{}", dict_key, src);
                            ui.add(
                                egui::Image::from_bytes(uri, egui::load::Bytes::Shared(bytes))
                                    .max_height(size * 14.0)
                                    .max_width(ui.available_width().max(64.0)),
                            )
                            .on_hover_text(src.as_str());
                        }
                        None => {
                            ui.label(RichText::new(format!("[{}]", src)).weak().size(size * 0.85))
                                .on_hover_text("resim bulunamadı");
                        }
                    },
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

fn short(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}
