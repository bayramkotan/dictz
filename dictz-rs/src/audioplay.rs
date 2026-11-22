//! Sound playback choices, shared with the Python edition (`dictz/core/audio.py`).
//!
//! `audio.player`: `auto` (the built-in player, else the first external
//! player found, else the system's), `internal` (rodio), `mpv`, `ffplay`,
//! `vlc`, `mplayer`, `system`, `command` (`audio.command`, `{file}` is the
//! sound file). `audio.speed` 0.5–1.5 slows a sound down.
//!
//! The end of a word used to be cut off: the device was closed when the data
//! ended, before its buffer had been heard. The built-in player now adds
//! [`TAIL_MS`] of silence and waits for the buffer to drain.

use std::path::{Path, PathBuf};

/// Silence added after every sound of the built-in players.
pub const TAIL_MS: u32 = 350;
/// The external players dictz knows, in the order `auto` tries them.
pub const EXTERNAL: [&str; 4] = ["mpv", "ffplay", "vlc", "mplayer"];

fn known_paths(name: &str) -> &'static [&'static str] {
    match name {
        "vlc" => &[
            r"C:\Program Files\VideoLAN\VLC\vlc.exe",
            r"C:\Program Files (x86)\VideoLAN\VLC\vlc.exe",
            "/Applications/VLC.app/Contents/MacOS/VLC",
        ],
        "mpv" => &[
            r"C:\Program Files\mpv\mpv.exe",
            "/Applications/mpv.app/Contents/MacOS/mpv",
            "/opt/homebrew/bin/mpv",
            "/usr/local/bin/mpv",
        ],
        "ffplay" => &["/opt/homebrew/bin/ffplay", "/usr/local/bin/ffplay"],
        "mplayer" => &["/opt/homebrew/bin/mplayer", "/usr/local/bin/mplayer"],
        _ => &[],
    }
}

/// A program on `PATH` (with `.exe` on Windows).
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for cand in [dir.join(name), dir.join(format!("{name}.exe"))] {
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// The path of an external player, or `None` when it is not installed.
pub fn find_program(name: &str) -> Option<PathBuf> {
    if name == "vlc" && cfg!(target_os = "linux") {
        if let Some(p) = which("cvlc") {
            return Some(p);
        }
    }
    which(name).or_else(|| {
        known_paths(name)
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
    })
}

/// Python's `f"{x:g}"` for the speeds dictz uses (at most 6 significant digits).
fn fmt_g(x: f64) -> String {
    let s = format!("{:.6}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// A command line split at spaces; `"…"` and `'…'` group (quotes removed).
/// No backslash escapes, so Windows paths stay as they are. Same as
/// `split_command` in Python.
pub fn split_command(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut have = false;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c.is_whitespace() => {
                if have {
                    out.push(std::mem::take(&mut cur));
                    have = false;
                }
            }
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                have = true;
            }
            None => {
                cur.push(c);
                have = true;
            }
        }
    }
    if have {
        out.push(cur);
    }
    out
}

/// The command line that plays `file` with `player` (found at `program`) at
/// `volume` (0–100) and `speed` (0.5–1.5). Same as `player_command` in Python.
pub fn player_command(
    player: &str,
    program: &str,
    file: &str,
    volume: i64,
    speed: f64,
    custom: &str,
) -> Vec<String> {
    let v = volume.clamp(0, 100);
    let s = fmt_g(speed.clamp(0.5, 1.5));
    let own = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    match player {
        "mpv" => own(&[
            program,
            "--no-video",
            "--really-quiet",
            &format!("--volume={v}"),
            &format!("--speed={s}"),
            file,
        ]),
        "ffplay" => own(&[
            program,
            "-nodisp",
            "-autoexit",
            "-loglevel",
            "quiet",
            "-volume",
            &v.to_string(),
            "-af",
            &format!("atempo={s},apad=pad_dur={}", fmt_g(TAIL_MS as f64 / 1000.0)),
            file,
        ]),
        "vlc" => own(&[
            program,
            "-I",
            "dummy",
            "--play-and-exit",
            "--no-video",
            &format!("--rate={s}"),
            &format!("--gain={:.2}", v as f64 / 100.0),
            file,
        ]),
        "mplayer" => own(&[
            program,
            "-really-quiet",
            "-novideo",
            "-volume",
            &v.to_string(),
            "-af",
            "scaletempo",
            "-speed",
            &s,
            file,
        ]),
        "command" => {
            let mut parts = split_command(custom);
            if parts.is_empty() {
                return Vec::new();
            }
            if !parts.iter().any(|p| p.contains("{file}")) {
                parts.push("{file}".into());
            }
            parts
                .into_iter()
                .map(|p| {
                    p.replace("{file}", file)
                        .replace("{volume}", &v.to_string())
                        .replace("{speed}", &s)
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// `auto` → internal, else the first external player installed, else
/// `system`; an external player that is not installed → `system`.
pub fn resolve(player: &str, internal_ok: bool) -> String {
    match player {
        "auto" => {
            if internal_ok {
                "internal".into()
            } else {
                EXTERNAL
                    .iter()
                    .find(|p| find_program(p).is_some())
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "system".into())
            }
        }
        "internal" if !internal_ok => resolve("auto", false),
        p if EXTERNAL.contains(&p) && find_program(p).is_none() => "system".into(),
        p => p.to_string(),
    }
}

/// A PCM WAV with `ms` of silence added at the end (`None` when `data` is
/// not a plain PCM WAV). Same bytes as Python's `pad_wav`.
pub fn pad_wav(data: &[u8], ms: u32) -> Option<Vec<u8>> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }
    // (format tag, channels, rate, bits per sample)
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut frames: Option<&[u8]> = None;
    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body = &data[pos + 8..(pos + 8 + len).min(data.len())];
        if id == b"fmt " && body.len() >= 16 {
            let tag = u16::from_le_bytes([body[0], body[1]]);
            let ch = u16::from_le_bytes([body[2], body[3]]);
            let rate = u32::from_le_bytes(body[4..8].try_into().ok()?);
            let bits = u16::from_le_bytes([body[14], body[15]]);
            if tag != 1 {
                return None; // only PCM, like Python's wave module
            }
            fmt = Some((tag, ch, rate, bits));
        } else if id == b"data" {
            frames = Some(body);
            break;
        }
        pos += 8 + len + (len & 1);
    }
    let (_, ch, rate, bits) = fmt?;
    let frames = frames?;
    let width = (bits as usize).div_ceil(8);
    let frame = width * ch as usize;
    let frames = &frames[..frames.len() - frames.len() % frame.max(1)];
    let n = (rate as u64 * ms as u64 / 1000) as usize;
    let silence = if width == 1 { 0x80u8 } else { 0 };
    let mut body = frames.to_vec();
    body.extend(std::iter::repeat_n(silence, n * frame));
    Some(wav(&body, rate, ch, width as u16))
}

/// Samples as a WAV file (the header Python's `wave` module writes).
pub fn wav(pcm: &[u8], rate: u32, channels: u16, width: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * width as u32).to_le_bytes());
    out.extend_from_slice(&(channels * width).to_le_bytes());
    out.extend_from_slice(&(width * 8).to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(pcm);
    out // like Python's wave module: no pad byte
}

/// The sound file in the media cache (written once).
pub fn cache_file(cache: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(cache)?;
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = cache.join(safe);
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) != bytes.len() as u64 {
        std::fs::write(&path, bytes)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_keeps_windows_paths() {
        assert_eq!(
            split_command(r#""C:\Program Files\mpv\mpv.exe" --title 'a b' {file}"#),
            vec![r"C:\Program Files\mpv\mpv.exe", "--title", "a b", "{file}"]
        );
        assert!(split_command("   ").is_empty());
    }

    #[test]
    fn fmt_like_python_g() {
        assert_eq!(fmt_g(1.0), "1");
        assert_eq!(fmt_g(0.75), "0.75");
        assert_eq!(fmt_g(0.35), "0.35");
    }
}
