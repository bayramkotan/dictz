//! Sound playback for DSL `[s]` media.
//!
//! With the `audio` feature the sound is decoded and played in-process
//! (WAV, MP3, Ogg Vorbis, FLAC). Otherwise — or when no audio device is
//! available — the file is handed to the system's default player.

use std::path::PathBuf;

/// Play `bytes` (named `name`) without blocking the UI.
/// `volume` 0.0–1.0; `system_player` hands the file to the OS player.
pub fn play(bytes: Vec<u8>, name: String, cache_dir: PathBuf, volume: f32, system_player: bool) {
    std::thread::spawn(move || {
        if system_player || play_inline(&bytes, volume).is_err() {
            let _ = super::system::open_media_file(&cache_dir, &name, &bytes);
        }
    });
}

#[cfg(feature = "audio")]
fn play_inline(bytes: &[u8], volume: f32) -> Result<(), String> {
    let (_stream, handle) = rodio::OutputStream::try_default().map_err(|e| e.to_string())?;
    let sink = rodio::Sink::try_new(&handle).map_err(|e| e.to_string())?;
    let source =
        rodio::Decoder::new(std::io::Cursor::new(bytes.to_vec())).map_err(|e| e.to_string())?;
    sink.set_volume(volume);
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}

#[cfg(not(feature = "audio"))]
fn play_inline(_bytes: &[u8], _volume: f32) -> Result<(), String> {
    Err("built without the audio feature".into())
}
