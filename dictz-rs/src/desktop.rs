//! Desktop and menu shortcuts (`dictz shortcut`, Dosya → Masaüstü kısayolu
//! oluştur) — the same files and places as the Python edition's
//! `dictz/core/desktop.py`:
//!
//! * Linux: `~/.local/share/applications/dictz.desktop` and a copy on the
//!   desktop (`xdg-user-dir DESKTOP`), icon `dictz.png` in the data folder
//! * Windows: `dictz.lnk` on the desktop and in the Start menu (PowerShell /
//!   WScript.Shell), icon `dictz.ico`
//! * macOS: `~/Applications/dictz.app` (launcher bundle) and a link to it on
//!   the desktop
//!
//! Existing shortcuts are replaced.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::icon::app_icon_rgba;

fn crc32(data: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for b in data {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// The dictz icon as a PNG file.
pub fn png_bytes(n: usize) -> Vec<u8> {
    let rgba = app_icon_rgba(n);
    let mut raw = Vec::with_capacity(n * (n * 4 + 1));
    for y in 0..n {
        raw.push(0u8);
        raw.extend_from_slice(&rgba[y * n * 4..(y + 1) * n * 4]);
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = z.write_all(&raw);
    let idat = z.finish().unwrap_or_default();
    let chunk = |tag: &[u8], data: &[u8]| {
        let mut out = Vec::new();
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut td = tag.to_vec();
        td.extend_from_slice(data);
        out.extend_from_slice(&td);
        out.extend_from_slice(&crc32(&td).to_be_bytes());
        out
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(n as u32).to_be_bytes());
    ihdr.extend_from_slice(&(n as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    out.extend(chunk(b"IHDR", &ihdr));
    out.extend(chunk(b"IDAT", &idat));
    out.extend(chunk(b"IEND", b""));
    out
}

/// Windows .ico holding one PNG image.
pub fn ico_bytes(n: usize) -> Vec<u8> {
    let png = png_bytes(n);
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    out.extend_from_slice(&[n as u8, n as u8, 0, 0, 1, 0, 32, 0]);
    out.extend_from_slice(&(png.len() as u32).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes());
    out.extend(png);
    out
}

/// macOS .icns holding one PNG image (`icp6` = 64×64).
pub fn icns_bytes(n: usize) -> Vec<u8> {
    let png = png_bytes(n);
    let mut entry = b"icp6".to_vec();
    entry.extend_from_slice(&((png.len() + 8) as u32).to_be_bytes());
    entry.extend(png);
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&((entry.len() + 8) as u32).to_be_bytes());
    out.extend(entry);
    out
}

/// The program that starts the window (an AppImage when run from one).
pub fn launch_program() -> PathBuf {
    if let Some(a) = std::env::var_os("APPIMAGE") {
        return PathBuf::from(a);
    }
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("dictz"))
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn desktop_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(out) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
            let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
            if out.status.success() && !p.as_os_str().is_empty() && p != home() && p.is_dir() {
                return Some(p);
            }
        }
    }
    ["Desktop", "Masaüstü"]
        .iter()
        .map(|n| home().join(n))
        .find(|p| p.is_dir())
}

fn exec_quote(s: &str) -> String {
    if s.chars()
        .any(|c| matches!(c, ' ' | '\t' | '"' | '\\' | '$' | '`'))
    {
        let e = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`");
        format!("\"{}\"", e)
    } else {
        s.to_string()
    }
}

/// Contents of the Linux `dictz.desktop` file.
pub fn desktop_entry(program: &Path, icon: &Path) -> String {
    [
        "[Desktop Entry]".to_string(),
        "Type=Application".into(),
        "Version=1.0".into(),
        "Name=dictz".into(),
        "GenericName=Dictionary".into(),
        "GenericName[tr]=Sözlük".into(),
        "Comment=Offline dictionary (StarDict, DSL, Babylon)".into(),
        "Comment[tr]=Çevrimdışı sözlük (StarDict, DSL, Babylon)".into(),
        format!("Exec={} %F", exec_quote(&program.to_string_lossy())),
        format!("Icon={}", icon.display()),
        "Terminal=false".into(),
        "Categories=Office;Dictionary;Education;".into(),
        "Keywords=dictionary;sözlük;stardict;dsl;babylon;goldendict;".into(),
        "StartupWMClass=dictz".into(),
        String::new(),
    ]
    .join("\n")
}

fn remove_any(p: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(p),
        Ok(_) => std::fs::remove_file(p),
        Err(_) => Ok(()),
    }
}

fn write(p: &Path, data: &[u8], executable: bool) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    remove_any(p)?;
    std::fs::write(p, data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if executable { 0o755 } else { 0o644 };
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode))?;
    }
    let _ = executable;
    Ok(())
}

#[cfg(windows)]
fn powershell(script: &str, envs: &[(&str, String)]) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    for (k, v) in envs {
        c.env(k, v);
    }
    let out = c.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(windows)]
fn windows_targets(desktop: bool, menu: bool) -> Vec<PathBuf> {
    let mut kinds = Vec::new();
    if desktop {
        kinds.push("[Environment]::GetFolderPath('Desktop')");
    }
    if menu {
        kinds.push("[Environment]::GetFolderPath('Programs')");
    }
    if kinds.is_empty() {
        return Vec::new();
    }
    powershell(&kinds.join("; "), &[])
        .map(|o| {
            o.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| PathBuf::from(l.trim()).join("dictz.lnk"))
                .collect()
        })
        .unwrap_or_default()
}

/// Where the shortcuts go on this system.
pub fn shortcut_paths(desktop: bool, menu: bool) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        windows_targets(desktop, menu)
    }
    #[cfg(target_os = "macos")]
    {
        let mut v = Vec::new();
        if menu {
            v.push(home().join("Applications").join("dictz.app"));
        }
        if desktop {
            if let Some(d) = desktop_dir() {
                v.push(d.join("dictz.app"));
            }
        }
        v
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let mut v = Vec::new();
        if menu {
            let data = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".local").join("share"));
            v.push(data.join("applications").join("dictz.desktop"));
        }
        if desktop {
            if let Some(d) = desktop_dir() {
                v.push(d.join("dictz.desktop"));
            }
        }
        v
    }
}

/// Create (replace) the shortcuts; returns the files written.
pub fn create_shortcuts(
    data_dir: &Path,
    desktop: bool,
    menu: bool,
) -> Result<Vec<PathBuf>, String> {
    let program = launch_program();
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let e = |x: std::io::Error| x.to_string();
    #[cfg(windows)]
    {
        let ico = data_dir.join("dictz.ico");
        write(&ico, &ico_bytes(64), false).map_err(e)?;
        let targets = windows_targets(desktop, menu);
        if targets.is_empty() {
            return Err("Masaüstü / Başlat menüsü klasörü bulunamadı (PowerShell)".into());
        }
        let files: Vec<String> = targets.iter().map(|t| t.display().to_string()).collect();
        let script = "$w = New-Object -ComObject WScript.Shell; \
            foreach ($f in $env:DICTZ_SC_FILES.Split('|')) { \
            if (Test-Path $f) { Remove-Item -Force $f }; \
            $s = $w.CreateShortcut($f); $s.TargetPath = $env:DICTZ_SC_TARGET; \
            $s.Arguments = ''; $s.IconLocation = $env:DICTZ_SC_ICON; \
            $s.WorkingDirectory = $env:DICTZ_SC_HOME; \
            $s.Description = 'dictz - offline dictionary'; $s.Save() }";
        powershell(
            script,
            &[
                ("DICTZ_SC_TARGET", program.display().to_string()),
                ("DICTZ_SC_ICON", ico.display().to_string()),
                ("DICTZ_SC_FILES", files.join("|")),
                ("DICTZ_SC_HOME", home().display().to_string()),
            ],
        )?;
        Ok(targets.into_iter().filter(|t| t.exists()).collect())
    }
    #[cfg(target_os = "macos")]
    {
        let bundle = home().join("Applications").join("dictz.app");
        let script = format!(
            "#!/bin/sh\nexec {} \"$@\"\n",
            exec_quote(&program.to_string_lossy())
        );
        write(
            &bundle.join("Contents/MacOS/dictz"),
            script.as_bytes(),
            true,
        )
        .map_err(e)?;
        write(
            &bundle.join("Contents/Resources/dictz.icns"),
            &icns_bytes(64),
            false,
        )
        .map_err(e)?;
        write(
            &bundle.join("Contents/Info.plist"),
            info_plist().as_bytes(),
            false,
        )
        .map_err(e)?;
        let mut made = vec![bundle.clone()];
        for p in shortcut_paths(desktop, menu) {
            if p != bundle {
                remove_any(&p).map_err(e)?;
                std::os::unix::fs::symlink(&bundle, &p).map_err(e)?;
                made.push(p);
            }
        }
        Ok(made)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let icon = data_dir.join("dictz.png");
        write(&icon, &png_bytes(64), false).map_err(e)?;
        let text = desktop_entry(&program, &icon);
        let mut made = Vec::new();
        for p in shortcut_paths(desktop, menu) {
            write(&p, text.as_bytes(), true).map_err(e)?;
            let _ = Command::new("gio")
                .arg("set")
                .arg(&p)
                .args(["metadata::trusted", "true"])
                .output();
            made.push(p);
        }
        if let Some(apps) = made
            .iter()
            .filter_map(|p| p.parent())
            .find(|d| d.ends_with("applications"))
        {
            let _ = Command::new("update-desktop-database").arg(apps).output();
        }
        Ok(made)
    }
}

#[cfg(target_os = "macos")]
fn info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>dictz</string>
  <key>CFBundleDisplayName</key><string>dictz</string>
  <key>CFBundleIdentifier</key><string>com.bayramkotan.dictz</string>
  <key>CFBundleExecutable</key><string>dictz</string>
  <key>CFBundleIconFile</key><string>dictz.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{}</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.reference</string>
</dict>
</plist>
"#,
        crate::VERSION
    )
}

/// Remove the shortcuts; returns what was deleted.
pub fn remove_shortcuts(desktop: bool, menu: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in shortcut_paths(desktop, menu) {
        if std::fs::symlink_metadata(&p).is_ok() && remove_any(&p).is_ok() {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_files() {
        let png = png_bytes(64);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(&ico_bytes(64)[..4], &[0, 0, 1, 0]);
        assert_eq!(&icns_bytes(64)[..4], b"icns");
        let e = desktop_entry(Path::new("/opt/my apps/dictz"), Path::new("/x/dictz.png"));
        assert!(e.contains("Exec=\"/opt/my apps/dictz\" %F"));
    }
}
