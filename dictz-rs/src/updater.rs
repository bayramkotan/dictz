//! `dictz update` — find out how this copy was installed and update it there.
//!
//! * installed with `cargo install` → `cargo install dictz --locked --force`
//! * portable binary from GitHub Releases → download the new archive and
//!   replace the executable in place
//!
//! Nothing is moved or removed; every copy is updated where it is.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::VERSION;

/// crates.io API for this crate.
pub const CRATES_URL: &str = "https://crates.io/api/v1/crates/dictz";
/// Latest GitHub release.
pub const RELEASES_URL: &str = "https://api.github.com/repos/bayramkotan/dictz/releases/latest";

fn user_agent() -> String {
    format!(
        "dictz-updater/{} (+https://github.com/bayramkotan/dictz)",
        VERSION
    )
}

/// "0.0.10" → [0, 0, 10].
pub fn parse_version(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|p| {
            p.chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// `true` when `latest` is newer than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

fn get_json(url: &str) -> Result<serde_json::Value, String> {
    let resp = ureq::get(url)
        .set("User-Agent", &user_agent())
        .set("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| e.to_string())?;
    let mut s = String::new();
    resp.into_reader()
        .take(16 << 20)
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

/// Newest dictz on crates.io.
pub fn latest_version() -> Result<String, String> {
    let v = get_json(CRATES_URL)?;
    v["crate"]["max_stable_version"]
        .as_str()
        .or_else(|| v["crate"]["max_version"].as_str())
        .map(str::to_string)
        .ok_or_else(|| "unexpected crates.io answer".to_string())
}

/// How this copy was installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallKind {
    /// `cargo install dictz`
    Cargo,
    /// a binary from GitHub Releases (or built by hand)
    Portable,
}

/// Detected installation.
#[derive(Debug, Clone)]
pub struct Install {
    /// Kind.
    pub kind: InstallKind,
    /// Human-readable description.
    pub detail: String,
    /// The running executable.
    pub exe: PathBuf,
}

fn cargo_home() -> PathBuf {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".cargo")
        })
}

/// Detect how the running binary was installed.
pub fn detect_install() -> Install {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_default();
    let bin = cargo_home()
        .join("bin")
        .canonicalize()
        .unwrap_or_else(|_| cargo_home().join("bin"));
    if exe.starts_with(&bin) {
        Install {
            kind: InstallKind::Cargo,
            detail: "cargo install".into(),
            exe,
        }
    } else {
        Install {
            kind: InstallKind::Portable,
            detail: "portable binary".into(),
            exe,
        }
    }
}

/// Release asset for this platform.
pub fn asset_name() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("dictz-rs-linux-x86_64.tar.gz")
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        Some("dictz-rs-windows-x86_64.zip")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("dictz-rs-macos-arm64.tar.gz")
    } else {
        None
    }
}

/// Remove a `dictz.old` left behind by a previous update (Windows cannot
/// delete a running executable, so it is renamed first).
pub fn cleanup_old_binary() {
    if let Ok(exe) = std::env::current_exe() {
        let old = exe.with_extension("old");
        if old.exists() {
            let _ = std::fs::remove_file(old);
        }
    }
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", &user_agent())
        .timeout(std::time::Duration::from_secs(300))
        .call()
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(512 << 20)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

fn extract_binary(archive: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let exe_name = if cfg!(windows) { "dictz.exe" } else { "dictz" };
    if name.ends_with(".zip") {
        let mut z =
            zip::ZipArchive::new(std::io::Cursor::new(archive)).map_err(|e| e.to_string())?;
        for i in 0..z.len() {
            let mut f = z.by_index(i).map_err(|e| e.to_string())?;
            if f.name().rsplit('/').next() == Some(exe_name) {
                let mut out = Vec::new();
                f.read_to_end(&mut out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
        }
    } else {
        let gz = flate2::read::GzDecoder::new(archive);
        let mut t = tar::Archive::new(gz);
        for entry in t.entries().map_err(|e| e.to_string())? {
            let mut e = entry.map_err(|e| e.to_string())?;
            let path = e.path().map_err(|e| e.to_string())?.to_path_buf();
            if path.file_name().map(|f| f == exe_name).unwrap_or(false) {
                let mut out = Vec::new();
                e.read_to_end(&mut out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
        }
    }
    Err(format!("{} not found in {}", exe_name, name))
}

fn replace_executable(exe: &Path, bytes: &[u8]) -> Result<(), String> {
    let old = exe.with_extension("old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).map_err(|e| format!("cannot move {}: {}", exe.display(), e))?;
    if let Err(e) = std::fs::write(exe, bytes) {
        let _ = std::fs::rename(&old, exe);
        return Err(format!("cannot write {}: {}", exe.display(), e));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(exe, std::fs::Permissions::from_mode(0o755));
    }
    if !cfg!(windows) {
        let _ = std::fs::remove_file(&old);
    }
    Ok(())
}

fn confirm(question: &str) -> bool {
    use std::io::{IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        return false;
    }
    print!("{} [y/N] ", question);
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    matches!(
        line.trim().to_lowercase().as_str(),
        "y" | "yes" | "e" | "evet"
    )
}

/// `dictz update`. Returns the process exit code.
pub fn run_update(check_only: bool, assume_yes: bool) -> i32 {
    let inst = detect_install();
    println!("dictz {} (Rust edition) — {}", VERSION, inst.detail);
    println!("  location: {}", inst.exe.display());
    let latest = match latest_version() {
        Ok(v) => v,
        Err(e) => {
            println!("Could not reach crates.io: {}", e);
            return 2;
        }
    };
    println!("  latest:   {}", latest);
    if !is_newer(&latest, VERSION) {
        println!("Already up to date.");
        return 0;
    }
    let cmd = "cargo install dictz --locked --force";
    if inst.kind == InstallKind::Cargo {
        println!("Update command: {}", cmd);
    }
    if check_only {
        println!("Update available: {} -> {}", VERSION, latest);
        return 0;
    }
    if !assume_yes && !confirm(&format!("Update dictz {} -> {}?", VERSION, latest)) {
        println!("Cancelled (use --yes when not running in a terminal).");
        return 1;
    }
    match inst.kind {
        InstallKind::Cargo => {
            if Command::new("cargo").arg("--version").output().is_err() {
                println!("cargo is not on PATH.");
                return 1;
            }
            // Windows cannot overwrite a running executable: move it aside first.
            let old = inst.exe.with_extension("old");
            let moved = cfg!(windows) && std::fs::rename(&inst.exe, &old).is_ok();
            let status = Command::new("cargo")
                .args(["install", "dictz", "--locked", "--force"])
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("Updated.");
                    0
                }
                other => {
                    if moved && !inst.exe.exists() {
                        let _ = std::fs::rename(&old, &inst.exe);
                    }
                    println!("Update failed: {:?}", other.map(|s| s.code()));
                    1
                }
            }
        }
        InstallKind::Portable => {
            let Some(asset) = asset_name() else {
                println!("No prebuilt binary for this platform; use: {}", cmd);
                return 1;
            };
            let rel = match get_json(RELEASES_URL) {
                Ok(v) => v,
                Err(e) => {
                    println!("Could not reach GitHub: {}", e);
                    return 2;
                }
            };
            let url = rel["assets"]
                .as_array()
                .and_then(|a| a.iter().find(|x| x["name"].as_str() == Some(asset)))
                .and_then(|x| x["browser_download_url"].as_str())
                .map(str::to_string);
            let Some(url) = url else {
                println!("Release {} has no {}.", latest, asset);
                return 1;
            };
            println!("Downloading {} …", asset);
            let result = download(&url)
                .and_then(|a| extract_binary(&a, asset))
                .and_then(|bin| {
                    replace_executable(&inst.exe, &bin)?;
                    // `dictz` and `dct` are the same program: update the sibling too
                    for name in ["dictz", "dct"] {
                        let sib = inst.exe.with_file_name(format!(
                            "{}{}",
                            name,
                            std::env::consts::EXE_SUFFIX
                        ));
                        if sib != inst.exe && sib.is_file() && !sib.is_symlink() {
                            replace_executable(&sib, &bin)?;
                        }
                    }
                    Ok(())
                });
            match result {
                Ok(()) => {
                    println!("Updated {}. Restart dictz.", inst.exe.display());
                    0
                }
                Err(e) => {
                    println!("Update failed: {}", e);
                    1
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("0.0.10", "0.0.9"));
        assert!(!is_newer("0.0.7", "0.0.7"));
        assert!(is_newer("v0.1.0", "0.0.99"));
    }
}
