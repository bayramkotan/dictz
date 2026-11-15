//! Pictures and sounds referenced by dictionary articles.
//!
//! Lookup order (same as the Python edition, `dictz/core/media.py`):
//!
//! 1. folder `<file>.files/` — e.g. `En-Tr.dsl.files/` (for `.dsl.dz` the
//!    `.dz` is dropped first), then `<stem>.files/`
//! 2. zip `<file>.files.zip`, then `<stem>.files.zip`
//! 3. StarDict `res/` folder next to the `.ifo`
//! 4. the dictionary's own folder
//!
//! Inside each place the exact name is tried first, then a case-insensitive
//! match (zips: also by base name, ignoring sub-folders).

use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Extensions shown as pictures.
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "gif", "tif", "tiff", "png", "bmp"];
/// Extensions played as sound.
pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "ogg", "oga", "flac"];

/// Kind of a media file, judged by its extension (case-insensitive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// A picture.
    Image,
    /// A sound.
    Audio,
    /// Anything else.
    Other,
}

/// Classify `name` by extension.
pub fn media_kind(name: &str) -> MediaKind {
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        MediaKind::Image
    } else if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        MediaKind::Audio
    } else {
        MediaKind::Other
    }
}

/// Finds media files for one dictionary.
#[derive(Debug, Clone)]
pub struct MediaResolver {
    dirs: Vec<PathBuf>,
    zips: Vec<PathBuf>,
}

fn append(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

/// Reject absolute paths and `..` so a dictionary cannot read arbitrary files.
fn safe_relative(name: &str) -> Option<PathBuf> {
    let name = name.trim().trim_start_matches("./").replace('\\', "/");
    if name.is_empty() {
        return None;
    }
    let p = PathBuf::from(&name);
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(p)
}

impl MediaResolver {
    /// Resolver for the dictionary file at `dict_path`.
    pub fn for_dictionary(dict_path: &Path) -> Self {
        let mut file = dict_path.to_path_buf();
        if file
            .extension()
            .map(|e| e.eq_ignore_ascii_case("dz"))
            .unwrap_or(false)
        {
            file = file.with_extension("");
        }
        let stem = file.with_extension("");
        let parent = dict_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let mut dirs = vec![
            append(&file, ".files"),
            append(&stem, ".files"),
            parent.join("res"),
            parent,
        ];
        dirs.dedup();
        let mut zips = vec![append(&file, ".files.zip"), append(&stem, ".files.zip")];
        zips.dedup();
        Self { dirs, zips }
    }

    /// Read a media file, or `None` when it cannot be found.
    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let rel = safe_relative(name)?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        // 1. folders (before zips, except the dictionary folder itself)
        let (own_dir, media_dirs) = self.dirs.split_last()?;
        for dir in media_dirs {
            if let Some(b) = read_from_dir(dir, &rel) {
                return Some(b);
            }
        }
        for z in &self.zips {
            if let Some(b) = read_from_zip(z, &rel_str) {
                return Some(b);
            }
        }
        read_from_dir(own_dir, &rel)
    }

    /// Where [`MediaResolver::read`] looks, in order (for diagnostics).
    pub fn search_locations(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.dirs[..self.dirs.len() - 1].to_vec();
        v.extend(self.zips.iter().cloned());
        v.extend(self.dirs.last().cloned());
        v
    }
}

fn read_from_dir(dir: &Path, rel: &Path) -> Option<Vec<u8>> {
    if !dir.is_dir() {
        return None;
    }
    let exact = dir.join(rel);
    if exact.is_file() {
        return std::fs::read(exact).ok();
    }
    // case-insensitive match of the file name inside its folder
    let parent = exact.parent()?;
    let wanted = rel.file_name()?.to_string_lossy().to_lowercase();
    for entry in std::fs::read_dir(parent).ok()?.flatten() {
        if entry.file_name().to_string_lossy().to_lowercase() == wanted && entry.path().is_file() {
            return std::fs::read(entry.path()).ok();
        }
    }
    None
}

fn read_from_zip(zip_path: &Path, rel: &str) -> Option<Vec<u8>> {
    if !zip_path.is_file() {
        return None;
    }
    let file = std::fs::File::open(zip_path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let names: Vec<String> = archive.file_names().map(str::to_string).collect();
    let lower = rel.to_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower).to_string();
    let pick = names
        .iter()
        .find(|n| *n == rel)
        .or_else(|| names.iter().find(|n| n.to_lowercase() == lower))
        .or_else(|| {
            names
                .iter()
                .find(|n| n.to_lowercase().rsplit('/').next() == Some(base.as_str()))
        })?
        .clone();
    let mut f = archive.by_name(&pick).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn kinds() {
        assert_eq!(media_kind("a.PNG"), MediaKind::Image);
        assert_eq!(media_kind("x/y.wav"), MediaKind::Audio);
        assert_eq!(media_kind("z.txt"), MediaKind::Other);
    }

    #[test]
    fn resolves_dir_zip_and_rejects_escape() {
        let d = std::env::temp_dir().join(format!("dictz-media-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("En.dsl.files")).unwrap();
        std::fs::write(d.join("En.dsl.files").join("Pic.PNG"), b"png").unwrap();
        let zf = std::fs::File::create(d.join("En.dsl.files.zip")).unwrap();
        let mut zw = zip::ZipWriter::new(zf);
        zw.start_file("snd/hello.wav", zip::write::SimpleFileOptions::default())
            .unwrap();
        zw.write_all(b"wav").unwrap();
        zw.finish().unwrap();
        std::fs::write(d.join("secret.txt"), b"s").unwrap();

        let r = MediaResolver::for_dictionary(&d.join("En.dsl.dz"));
        assert_eq!(r.read("pic.png").as_deref(), Some(&b"png"[..]));
        assert_eq!(r.read("Hello.WAV").as_deref(), Some(&b"wav"[..]));
        assert_eq!(r.read("secret.txt").as_deref(), Some(&b"s"[..]));
        assert!(r.read("../etc/passwd").is_none());
        assert!(r.read("/etc/passwd").is_none());
        assert!(r.read("missing.png").is_none());
    }
}
