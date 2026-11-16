//! Parsing hotkey texts ("Ctrl+Alt+F11") — the same rules as the Python
//! edition's `dictz/core/hotkeys.py`, checked by `scan_golden.json`.

use crate::settings::pyrepr;

/// A parsed hotkey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotkey {
    /// Sorted subset of "alt", "ctrl", "shift", "super".
    pub mods: Vec<&'static str>,
    /// "a".."z", "0".."9", "f1".."f24" or a named key ("space", "prior", …).
    pub key: String,
}

const NAMED: &[(&str, &str)] = &[
    ("space", "space"),
    ("tab", "tab"),
    ("enter", "return"),
    ("return", "return"),
    ("esc", "escape"),
    ("escape", "escape"),
    ("insert", "insert"),
    ("ins", "insert"),
    ("delete", "delete"),
    ("del", "delete"),
    ("home", "home"),
    ("end", "end"),
    ("pageup", "prior"),
    ("pgup", "prior"),
    ("pagedown", "next"),
    ("pgdown", "next"),
    ("up", "up"),
    ("down", "down"),
    ("left", "left"),
    ("right", "right"),
    ("pause", "pause"),
    ("print", "print"),
    ("backspace", "backspace"),
];

fn modifier(name: &str) -> Option<&'static str> {
    Some(match name {
        "ctrl" | "control" => "ctrl",
        "alt" => "alt",
        "shift" => "shift",
        "super" | "win" | "meta" | "cmd" => "super",
        _ => return None,
    })
}

fn is_function_key(k: &str) -> bool {
    k.strip_prefix('f')
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        .and_then(|n| n.parse::<u32>().ok())
        .map(|n| (1..=24).contains(&n))
        .unwrap_or(false)
}

/// `"Ctrl+Alt+F11"` → Hotkey; errors have the Python edition's wording.
pub fn parse_hotkey(text: &str) -> Result<Hotkey, String> {
    let compact: String = text.chars().filter(|c| *c != ' ').collect();
    let parts: Vec<String> = compact
        .split('+')
        .map(|p| p.trim().to_lowercase())
        .collect();
    let last = parts.last().cloned().unwrap_or_default();
    if last.is_empty() {
        return Err(format!("empty hotkey: {}", pyrepr(&text.into())));
    }
    let mut mods: Vec<&'static str> = Vec::new();
    for p in &parts[..parts.len() - 1] {
        match modifier(p) {
            Some(m) => {
                if !mods.contains(&m) {
                    mods.push(m)
                }
            }
            None => {
                return Err(format!(
                    "unknown modifier {} in {}",
                    pyrepr(&p.as_str().into()),
                    pyrepr(&text.into())
                ))
            }
        }
    }
    mods.sort();
    let single = last.chars().count() == 1 && last.chars().all(|c| c.is_ascii_alphanumeric());
    let key = if single || is_function_key(&last) {
        last.clone()
    } else if let Some((_, v)) = NAMED.iter().find(|(k, _)| *k == last) {
        v.to_string()
    } else {
        return Err(format!(
            "unknown key {} in {}",
            pyrepr(&last.as_str().into()),
            pyrepr(&text.into())
        ));
    };
    if mods.is_empty() && !key.starts_with('f') {
        return Err(format!(
            "a hotkey needs a modifier: {}",
            pyrepr(&text.into())
        ));
    }
    Ok(Hotkey { mods, key })
}

impl std::fmt::Display for Hotkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        for (m, name) in [
            ("ctrl", "Ctrl"),
            ("alt", "Alt"),
            ("shift", "Shift"),
            ("super", "Super"),
        ] {
            if self.mods.contains(&m) {
                parts.push(name.into());
            }
        }
        let k = if self.key.chars().count() == 1 || self.key.starts_with('f') {
            self.key.to_uppercase()
        } else {
            match self.key.as_str() {
                "prior" => "PgUp".into(),
                "next" => "PgDown".into(),
                "return" => "Enter".into(),
                "escape" => "Esc".into(),
                other => {
                    let mut c = other.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                }
            }
        };
        parts.push(k);
        write!(f, "{}", parts.join("+"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let h = parse_hotkey("ctrl+alt+d").unwrap();
        assert_eq!(h.mods, vec!["alt", "ctrl"]);
        assert_eq!(h.to_string(), "Ctrl+Alt+D");
        assert!(parse_hotkey("a").is_err());
    }
}
