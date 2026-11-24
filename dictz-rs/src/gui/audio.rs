//! Sound playback for DSL `[s]` media (`audio.player`, `audio.speed`,
//! `audio.volume`; choices and command lines in [`crate::audioplay`]).
//!
//! The built-in player (rodio, the `audio` feature) decodes WAV, MP3, Ogg
//! Vorbis and FLAC in-process. It used to cut the last syllable: the output
//! stream was dropped as soon as the data was used up, before the device's
//! buffer had been heard. Now [`crate::audioplay::TAIL_MS`] of silence is
//! played after the sound and the stream is kept a moment longer.
//! External players run as their own process; one sound at a time. The
//! status bar is told ([`take_note`]) when the chosen player is not installed
//! or an external player gives up at once (bad codec, wrong option).

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::audioplay;
use crate::i18n::{tr, tr_args};

/// What to play and how.
#[derive(Clone, Debug)]
pub struct Request {
    pub bytes: Vec<u8>,
    pub name: String,
    pub cache_dir: PathBuf,
    /// 0–100.
    pub volume: i64,
    /// 0.5–1.5.
    pub speed: f64,
    pub player: String,
    pub command: String,
}

/// Bumped by every new sound and by [`stop`]: a built-in playback whose
/// number is no longer current stops.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static CHILD: Mutex<Option<Child>> = Mutex::new(None);
static LAST: Mutex<Option<Request>> = Mutex::new(None);
/// A message for the status bar from the playing thread.
static NOTE: Mutex<Option<String>> = Mutex::new(None);

fn note(msg: String) {
    if let Ok(mut n) = NOTE.lock() {
        *n = Some(msg);
    }
}

/// The status-bar message the player left, if any (taken once).
pub fn take_note() -> Option<String> {
    NOTE.lock().ok().and_then(|mut n| n.take())
}

/// Stop what is playing.
pub fn stop() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut c) = CHILD.lock() {
        if let Some(mut child) = c.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Play the last sound again.
pub fn replay() {
    let last = LAST.lock().ok().and_then(|l| l.clone());
    if let Some(r) = last {
        play(r);
    }
}

/// `true` once a sound was played (the status bar shows replay / stop).
pub fn has_last() -> bool {
    LAST.lock().map(|l| l.is_some()).unwrap_or(false)
}

/// Play without blocking the window. Returns at once.
pub fn play(req: Request) {
    stop();
    if let Ok(mut l) = LAST.lock() {
        *l = Some(req.clone());
    }
    let gen = GENERATION.load(Ordering::SeqCst);
    std::thread::spawn(move || {
        let prefer = audioplay::prefers_external();
        let used = audioplay::resolve(&req.player, cfg!(feature = "audio"), &req.command, prefer);
        if audioplay::EXTERNAL.contains(&req.player.as_str()) && used != req.player {
            note(tr_args(
                "{player} kurulu değil, {used} kullanılıyor",
                &[
                    ("player", &tr(audioplay::label(&req.player))),
                    ("used", &tr(audioplay::label(&used))),
                ],
            ));
        }
        if used == "internal" && play_inline(&req, gen).is_ok() {
            return;
        }
        let used = if used == "internal" {
            audioplay::resolve("auto", false, "", prefer)
        } else {
            used
        };
        let Ok(file) = audioplay::cache_file(&req.cache_dir, &req.name, &req.bytes) else {
            return;
        };
        if used != "system" {
            let program = if used == "command" {
                String::new()
            } else {
                audioplay::find_program(&used)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            };
            let cmd = audioplay::player_command(
                &used,
                &program,
                &file.to_string_lossy(),
                req.volume,
                req.speed,
                &req.command,
            );
            if let Some((exe, args)) = cmd.split_first() {
                let mut c = Command::new(exe);
                c.args(args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
                }
                if let Ok(child) = c.spawn() {
                    let pid = child.id();
                    if let Ok(mut slot) = CHILD.lock() {
                        *slot = Some(child);
                    }
                    let name = if used == "command" {
                        exe.clone()
                    } else {
                        tr(audioplay::label(&used))
                    };
                    watch(pid, &name);
                    return;
                }
            }
        }
        let _ = super::system::open_external(&file.to_string_lossy());
    });
}

/// Say so when the external player (`pid`) stops within 1.5 s with an error.
fn watch(pid: u32, name: &str) {
    for _ in 0..15 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let Ok(mut slot) = CHILD.lock() else { return };
        let Some(child) = slot.as_mut() else { return };
        if child.id() != pid {
            return; // another sound took its place
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let code = status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".into());
                    note(tr_args(
                        "{player} sesi çalamadı (çıkış kodu {code})",
                        &[("player", &name), ("code", &code)],
                    ));
                }
                slot.take();
                return;
            }
            Ok(None) => {}
            Err(_) => return,
        }
    }
}

#[cfg(feature = "audio")]
fn play_inline(req: &Request, gen: u64) -> Result<(), String> {
    use rodio::Source;
    use std::time::Duration;
    let (_stream, handle) = rodio::OutputStream::try_default().map_err(|e| e.to_string())?;
    let sink = rodio::Sink::try_new(&handle).map_err(|e| e.to_string())?;
    let source =
        rodio::Decoder::new(std::io::Cursor::new(req.bytes.clone())).map_err(|e| e.to_string())?;
    let (channels, rate) = (source.channels(), source.sample_rate());
    sink.set_volume(req.volume.clamp(0, 100) as f32 / 100.0);
    sink.set_speed(req.speed.clamp(0.5, 1.5) as f32);
    sink.append(source);
    // silence after the sound: what the device still buffers is heard
    sink.append(
        rodio::source::Zero::<i16>::new(channels, rate)
            .take_duration(Duration::from_millis(audioplay::TAIL_MS as u64)),
    );
    while !sink.empty() {
        if GENERATION.load(Ordering::SeqCst) != gen {
            sink.stop();
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // keep the stream open until the device has played its buffer
    std::thread::sleep(Duration::from_millis(150));
    Ok(())
}

#[cfg(not(feature = "audio"))]
fn play_inline(_req: &Request, _gen: u64) -> Result<(), String> {
    Err("built without the audio feature".into())
}
