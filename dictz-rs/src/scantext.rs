//! Turning copied / selected text into a word to look up (scan popup) — the
//! same rules as the Python edition's `dictz/core/scantext.py`, checked by
//! `tests/data/parity/scan_golden.json`.

/// Characters stripped from both ends of a selection.
pub const TRIM: &str =
    " \u{9}\u{d}\u{a}\u{a0}.,;:!?¡¿\u{22}'`()[]{}<>«»“”„‘’‚‹›…-–—/\u{5c}|*#@&%^~_=+";

/// The word or phrase to look up, or `None` when the text is not one.
///
/// Whitespace runs become one space, punctuation around the text is
/// removed, and text longer than `max_length` characters (after cleaning) or
/// with more than one line is ignored.
pub fn clean_scan_text(text: &str, max_length: usize) -> Option<String> {
    let t = text.trim_matches(|c: char| TRIM.contains(c));
    if t.is_empty() || t.contains('\n') || t.contains('\r') {
        return None;
    }
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() || t.chars().count() > max_length {
        return None;
    }
    if !t.chars().any(char::is_alphabetic) {
        return None;
    }
    Some(t)
}

/// Ctrl+C+C: the same text copied twice within `interval_ms`.
#[derive(Debug, Clone)]
pub struct DoubleCopy {
    /// Allowed gap between the two copies.
    pub interval_ms: f64,
    last_text: Option<String>,
    last_ms: f64,
}

impl DoubleCopy {
    /// New detector.
    pub fn new(interval_ms: f64) -> Self {
        Self {
            interval_ms,
            last_text: None,
            last_ms: -1e18,
        }
    }

    /// Feed one copy; `true` when it completes a Ctrl+C+C.
    pub fn feed(&mut self, text: &str, now_ms: f64) -> bool {
        let gap = now_ms - self.last_ms;
        let hit =
            self.last_text.as_deref() == Some(text) && (0.0..=self.interval_ms).contains(&gap);
        if hit {
            // a third copy does not trigger again
            self.last_text = None;
            self.last_ms = -1e18;
        } else {
            self.last_text = Some(text.to_string());
            self.last_ms = now_ms;
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean() {
        assert_eq!(clean_scan_text("  Hello, ", 80).as_deref(), Some("Hello"));
        assert_eq!(clean_scan_text("a\nb", 80), None);
        assert_eq!(clean_scan_text("123", 80), None);
    }

    #[test]
    fn double() {
        let mut d = DoubleCopy::new(500.0);
        assert!(!d.feed("a", 0.0));
        assert!(d.feed("a", 300.0));
        assert!(!d.feed("a", 400.0));
    }
}
