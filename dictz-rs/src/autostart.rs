//! Start dictz when the user logs in (setting `general.autostart`).
//!
//! Same places and rules as the Python edition (`dictz/core/autostart.py`):
//!
//! * Linux / BSD: `$XDG_CONFIG_HOME/autostart/dictz.desktop` (default `~/.config`)
//! * Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `dictz`
//! * macOS: `~/Library/LaunchAgents/com.bayramkotan.dictz.plist` (`RunAtLoad`)
//!
//! The command is the program that starts the window plus `--autostart`:
//! dictz then starts hidden in the system tray and, when dictz is already
//! running, does nothing.

use std::path::{Path, PathBuf};

/// Argument added to the log-in command.
pub const FLAG: &str = "--autostart";
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "dictz";
const PLIST_LABEL: &str = "com.bayramkotan.dictz";

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The autostart file (`None` on Windows: the registry is used).
pub fn entry_path() -> Option<PathBuf> {
    if cfg!(windows) {
        None
    } else if cfg!(target_os = "macos") {
        Some(
            home()
                .join("Library/LaunchAgents")
                .join(format!("{}.plist", PLIST_LABEL)),
        )
    } else {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".config"));
        Some(base.join("autostart").join("dictz.desktop"))
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn reg(args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut c = std::process::Command::new("reg");
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c.output()
}

/// `true` when the log-in start is set up.
pub fn is_enabled() -> bool {
    if cfg!(windows) {
        return reg(&["query", RUN_KEY, "/v", VALUE])
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
    entry_path().map(|p| p.is_file()).unwrap_or(false)
}

/// Write the log-in start for the program running now (replaced every time,
/// so it always points at the current copy).
pub fn enable(data_dir: Option<&Path>) -> std::io::Result<()> {
    let program = crate::desktop::launch_program();
    if cfg!(windows) {
        let cmd = format!("\"{}\" {}", program.display(), FLAG);
        let o = reg(&[
            "add", RUN_KEY, "/v", VALUE, "/t", "REG_SZ", "/d", &cmd, "/f",
        ])?;
        if !o.status.success() {
            return Err(std::io::Error::other(
                String::from_utf8_lossy(&o.stderr).trim().to_string(),
            ));
        }
        return Ok(());
    }
    let p = entry_path().ok_or_else(|| std::io::Error::other("no autostart folder"))?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if cfg!(target_os = "macos") {
        let text = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  \
             <key>Label</key>\n  <string>{}</string>\n  <key>ProgramArguments</key>\n  <array>\n    \
             <string>{}</string>\n    <string>{}</string>\n  </array>\n  <key>RunAtLoad</key>\n  <true/>\n</dict>\n</plist>\n",
            PLIST_LABEL,
            xml_escape(&program.to_string_lossy()),
            FLAG
        );
        return std::fs::write(&p, text);
    }
    let icon = data_dir
        .map(|d| d.join("dictz.png"))
        .unwrap_or_else(|| p.with_file_name("dictz.png"));
    if !icon.is_file() {
        if let Some(dir) = icon.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&icon, crate::desktop::png_bytes(64));
    }
    let text = crate::desktop::desktop_entry(&program, &icon)
        .replace(" %F\n", &format!(" {}\n", FLAG))
        + "X-GNOME-Autostart-enabled=true\nX-KDE-autostart-after=panel\n";
    std::fs::write(&p, text)
}

/// Remove the log-in start.
pub fn disable() -> std::io::Result<()> {
    if cfg!(windows) {
        let _ = reg(&["delete", RUN_KEY, "/v", VALUE, "/f"])?;
        return Ok(());
    }
    if let Some(p) = entry_path() {
        if p.is_file() {
            std::fs::remove_file(p)?;
        }
    }
    Ok(())
}

/// Make the system match the setting; `Err` carries the reason.
pub fn sync(on: bool, data_dir: Option<&Path>) -> Result<(), String> {
    let r = if on {
        enable(data_dir)
    } else if is_enabled() {
        disable()
    } else {
        Ok(())
    };
    r.map_err(|e| e.to_string())
}

#[cfg(all(test, not(windows), not(target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn linux_entry_is_written_and_removed() {
        let d = std::env::temp_dir().join(format!("dictz-autostart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::env::set_var("XDG_CONFIG_HOME", &d);
        sync(true, Some(&d)).unwrap();
        let text = std::fs::read_to_string(d.join("autostart/dictz.desktop")).unwrap();
        assert!(text.contains("--autostart") && text.contains("X-GNOME-Autostart-enabled=true"));
        assert!(is_enabled());
        sync(false, None).unwrap();
        assert!(!is_enabled());
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&d);
    }
}
