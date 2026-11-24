//! Pictures and sounds referenced by dictionary articles.
//!
//! Lookup order (same as the Python edition, `dictz/core/media.py`; the
//! same names GoldenDict tries):
//!
//! 1. folders `<name>.files/` for every base name of the file — for
//!    `En-Tr.dsl.dz`: `En-Tr.dsl.dz.files/`, `En-Tr.dsl.files/`,
//!    `En-Tr.files/` (a plain `.dsl` tries the same three) — then
//!    StarDict's `res/` next to the `.ifo`
//! 2. zips `<name>.files.zip` for the same base names
//! 3. the dictionary's own folder
//!
//! Folder and zip names match without regard to case. Inside each place the
//! exact name is tried first, then a case-insensitive match (zips: also by
//! base name, ignoring sub-folders). A zip's list of names is read once and
//! kept (big media zips hold 100 000+ files).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Extensions shown as pictures (compared in lower case; same list as
/// `IMAGE_EXTENSIONS` in Python).
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "gif", "tif", "tiff", "png", "bmp", "webp"];
/// Extensions played as sound (compared in lower case). Lingvo / GoldenDict
/// sound packs use `.wav` (often ADPCM), `.ogg`, `.spx` (Speex), `.opus` …;
/// what the built-in player cannot decode goes to an external player.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "ogg", "oga", "opus", "spx", "flac", "m4a", "aac", "wma",
];

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

struct ZipIndex {
    mtime: std::time::SystemTime,
    archive: zip::ZipArchive<std::fs::File>,
    lower: HashMap<String, String>, // lower-case name → name
    base: HashMap<String, String>,  // lower-case base name → name
}

impl std::fmt::Debug for ZipIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZipIndex({} names)", self.lower.len())
    }
}

/// Finds media files for one dictionary.
#[derive(Debug, Clone)]
pub struct MediaResolver {
    dirs: Vec<PathBuf>,
    zips: Vec<PathBuf>,
    index: Arc<Mutex<HashMap<PathBuf, ZipIndex>>>,
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

/// `X.dsl.dz` → `X.dsl.dz`, `X.dsl`, `X`; `X.dsl` → the same three;
/// `X.ifo` → `X.ifo`, `X`.
fn base_names(path: &Path) -> Vec<PathBuf> {
    let mut out = vec![path.to_path_buf()];
    let mut p = path.to_path_buf();
    if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dz")) {
        p = p.with_extension("");
        out.push(p.clone());
    } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dsl")) {
        // GoldenDict tries X.dsl.dz.files(.zip) for a plain .dsl too
        out.insert(0, append(&p, ".dz"));
    }
    out.push(p.with_extension(""));
    out.dedup();
    out
}

/// `path` itself, or the entry of its folder with the same name in another
/// case (`X.DSL.FILES.ZIP`).
fn sibling(path: PathBuf, listing: &HashMap<String, PathBuf>) -> PathBuf {
    if path.exists() {
        return path;
    }
    let key = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    listing.get(&key).cloned().unwrap_or(path)
}

fn push_unique(v: &mut Vec<PathBuf>, p: PathBuf) {
    if !v.contains(&p) {
        v.push(p);
    }
}

impl MediaResolver {
    /// Resolver for the dictionary file at `dict_path`.
    pub fn for_dictionary(dict_path: &Path) -> Self {
        let parent = dict_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let listing: HashMap<String, PathBuf> = std::fs::read_dir(&parent)
            .map(|rd| {
                rd.flatten()
                    .map(|e| (e.file_name().to_string_lossy().to_lowercase(), e.path()))
                    .collect()
            })
            .unwrap_or_default();
        let bases = base_names(dict_path);
        let mut dirs = Vec::new();
        for b in &bases {
            push_unique(&mut dirs, sibling(append(b, ".files"), &listing));
        }
        push_unique(&mut dirs, sibling(parent.join("res"), &listing));
        dirs.push(parent);
        let mut zips = Vec::new();
        for b in &bases {
            push_unique(&mut zips, sibling(append(b, ".files.zip"), &listing));
        }
        Self {
            dirs,
            zips,
            index: Arc::new(Mutex::new(HashMap::new())),
        }
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
            if let Some(b) = self.read_from_zip(z, &rel_str) {
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

    fn read_from_zip(&self, zip_path: &Path, rel: &str) -> Option<Vec<u8>> {
        let mtime = std::fs::metadata(zip_path)
            .and_then(|m| m.modified())
            .ok()?;
        // the zip stays open with its list of names (opened once, not per picture)
        let mut idx = self.index.lock().unwrap_or_else(|e| e.into_inner());
        if !idx.get(zip_path).is_some_and(|e| e.mtime == mtime) {
            let file = std::fs::File::open(zip_path).ok()?;
            let archive = zip::ZipArchive::new(file).ok()?;
            let mut lower = HashMap::new();
            let mut base = HashMap::new();
            for n in archive.file_names() {
                let l = n.to_lowercase();
                let b = l.rsplit('/').next().unwrap_or(&l).to_string();
                base.entry(b).or_insert_with(|| n.to_string());
                lower.entry(l).or_insert_with(|| n.to_string());
            }
            idx.insert(
                zip_path.to_path_buf(),
                ZipIndex {
                    mtime,
                    archive,
                    lower,
                    base,
                },
            );
        }
        let z = idx.get_mut(zip_path)?;
        let pick = if z.archive.index_for_name(rel).is_some() {
            rel.to_string() // the exact name wins over another case
        } else {
            let want = rel.to_lowercase();
            let b = want.rsplit('/').next().unwrap_or(&want);
            z.lower.get(&want).or_else(|| z.base.get(b))?.clone()
        };
        let mut f = z.archive.by_name(&pick).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        Some(buf)
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

    #[test]
    fn finds_goldendict_zip_names() {
        // X.dsl.dz with X.dsl.dz.files.zip (the usual GoldenDict packs, e.g. OALD8)
        for (i, zname) in [
            "OALD.dsl.dz.files.zip",
            "OALD.DSL.DZ.FILES.ZIP",
            "OALD.files.zip",
        ]
        .iter()
        .enumerate()
        {
            let d =
                std::env::temp_dir().join(format!("dictz-media-gd-{}-{}", std::process::id(), i));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("OALD.dsl.dz"), b"").unwrap();
            let zf = std::fs::File::create(d.join(zname)).unwrap();
            let mut zw = zip::ZipWriter::new(zf);
            zw.start_file(
                "z_house__gb_2.wav",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zw.write_all(b"wav").unwrap();
            zw.start_file(
                "pics/x_verb_forms_house.jpg",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zw.write_all(b"jpg").unwrap();
            zw.finish().unwrap();
            let r = MediaResolver::for_dictionary(&d.join("OALD.dsl.dz"));
            assert_eq!(
                r.read("z_house__gb_2.wav").as_deref(),
                Some(&b"wav"[..]),
                "{zname}"
            );
            assert_eq!(
                r.read("X_VERB_FORMS_HOUSE.JPG").as_deref(),
                Some(&b"jpg"[..])
            );
            let _ = std::fs::remove_dir_all(&d);
        }
    }
}
