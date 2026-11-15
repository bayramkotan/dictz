//! Opening files and links with the operating system.

use std::path::Path;
use std::process::Command;

/// Open a URL or file with the default application.
pub fn open_external(target: &str) -> std::io::Result<()> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]).arg(target);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(target);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(target);
        c
    };
    cmd.spawn().map(|_| ())
}

/// Write `bytes` to the media cache and open it with the default player.
pub fn open_media_file(cache_dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(cache_dir)?;
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
    let path = cache_dir.join(safe);
    std::fs::write(&path, bytes)?;
    open_external(&path.to_string_lossy())
}
