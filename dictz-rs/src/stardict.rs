//! StarDict (`.ifo` + `.idx[.gz]` + `.dict[.dz]`) parser.
//!
//! Same rules as `dictz/parsers/stardict.py`.

use std::path::{Path, PathBuf};

use crate::{read_maybe_gzip, Dictionary, DictionaryMetadata, DictzError, WordTable};

/// A StarDict dictionary.
pub struct StarDictDictionary {
    path: PathBuf,
    base: PathBuf,
    meta: DictionaryMetadata,
    words: WordTable,
    spans: Vec<(usize, usize)>,
    data: Vec<u8>,
    offset_bits: u32,
    loaded: bool,
}

fn with_ext(base: &Path, ext: &str) -> PathBuf {
    let mut s = base.as_os_str().to_os_string();
    s.push(ext);
    PathBuf::from(s)
}

impl StarDictDictionary {
    /// Open `path` (any of the `.ifo` / `.idx` / `.dict` files).
    pub fn new(path: impl AsRef<Path>) -> Result<Self, DictzError> {
        let mut path = path.as_ref().to_path_buf();
        let is_ifo = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("ifo"))
            .unwrap_or(false);
        if !is_ifo {
            let candidate = path.with_extension("ifo");
            if candidate.exists() {
                path = candidate;
            } else {
                return Err(DictzError::NotFound(format!(
                    "No .ifo file found alongside {}",
                    path.display()
                )));
            }
        }
        let base = path.with_extension("");
        let meta = DictionaryMetadata {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            file_path: path.clone(),
            format: "stardict".into(),
            ..Default::default()
        };
        Ok(Self {
            path,
            base,
            meta,
            words: WordTable::new(),
            spans: Vec::new(),
            data: Vec::new(),
            offset_bits: 32,
            loaded: false,
        })
    }

    /// Path of the `.ifo` file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn parse_ifo(&mut self) -> Result<(), DictzError> {
        let ifo = with_ext(&self.base, ".ifo");
        let raw = std::fs::read(&ifo)?;
        let text = String::from_utf8_lossy(&raw);
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.starts_with("StarDict's dict ifo file") => {}
            _ => {
                return Err(DictzError::Malformed(format!(
                    "Not a valid StarDict .ifo: {}",
                    ifo.display()
                )))
            }
        }
        for line in lines {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "bookname" => self.meta.name = value.to_string(),
                "description" => self.meta.description = value.to_string(),
                "author" => self.meta.author = value.to_string(),
                "idxoffsetbits" => self.offset_bits = value.parse().unwrap_or(32),
                _ => {}
            }
        }
        if self.offset_bits != 32 && self.offset_bits != 64 {
            return Err(DictzError::Malformed(format!(
                "Unsupported idxoffsetbits: {}",
                self.offset_bits
            )));
        }
        Ok(())
    }

    fn parse_idx(&mut self) -> Result<(), DictzError> {
        let idx = with_ext(&self.base, ".idx");
        let gz = with_ext(&self.base, ".idx.gz");
        let data = if idx.exists() {
            std::fs::read(&idx)?
        } else if gz.exists() {
            read_maybe_gzip(&gz)?
        } else {
            return Err(DictzError::NotFound(format!(
                "Missing .idx file: {}",
                idx.display()
            )));
        };
        let off_size = (self.offset_bits / 8) as usize;
        let tail = off_size + 4;
        let mut pos = 0usize;
        while pos < data.len() {
            let Some(nul) = data[pos..].iter().position(|&b| b == 0).map(|p| p + pos) else {
                break;
            };
            if nul + tail > data.len() {
                break;
            }
            let word = String::from_utf8_lossy(&data[pos..nul]).into_owned();
            let t = nul + 1;
            let offset = if off_size == 4 {
                u32::from_be_bytes(data[t..t + 4].try_into().unwrap()) as usize
            } else {
                u64::from_be_bytes(data[t..t + 8].try_into().unwrap()) as usize
            };
            let size =
                u32::from_be_bytes(data[t + off_size..t + tail].try_into().unwrap()) as usize;
            if let Some(slot) = self.words.insert(&word) {
                debug_assert_eq!(slot, self.spans.len());
                self.spans.push((offset, size));
            }
            pos = t + tail;
        }
        Ok(())
    }

    fn load_dict(&mut self) -> Result<(), DictzError> {
        let plain = with_ext(&self.base, ".dict");
        let dz = with_ext(&self.base, ".dict.dz");
        self.data = if plain.exists() {
            std::fs::read(&plain)?
        } else if dz.exists() {
            read_maybe_gzip(&dz)?
        } else {
            return Err(DictzError::NotFound(format!(
                "Missing .dict or .dict.dz file near {}",
                self.base.display()
            )));
        };
        Ok(())
    }
}

impl Dictionary for StarDictDictionary {
    fn load(&mut self) -> Result<(), DictzError> {
        if self.loaded {
            return Ok(());
        }
        self.parse_ifo()?;
        self.parse_idx()?;
        self.load_dict()?;
        self.words.finish();
        self.meta.word_count = self.words.len();
        self.loaded = true;
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }

    fn metadata(&self) -> &DictionaryMetadata {
        &self.meta
    }

    fn metadata_mut(&mut self) -> &mut DictionaryMetadata {
        &mut self.meta
    }

    fn words(&self) -> &WordTable {
        &self.words
    }

    fn definition(&self, headword: &str) -> Option<String> {
        let (offset, size) = *self.spans.get(self.words.slot(headword)?)?;
        let end = offset.saturating_add(size).min(self.data.len());
        let start = offset.min(end);
        Some(String::from_utf8_lossy(&self.data[start..end]).into_owned())
    }

    fn close(&mut self) {
        self.data = Vec::new();
        self.spans.clear();
        self.words.clear();
        self.loaded = false;
    }
}

/// Write a small uncompressed StarDict dictionary (used by tests and `--selftest`).
pub fn write_stardict(
    base: &Path,
    name: &str,
    entries: &[(&str, &str)],
) -> std::io::Result<PathBuf> {
    let mut body = Vec::new();
    let mut idx = Vec::new();
    for (word, defi) in entries {
        idx.extend_from_slice(word.as_bytes());
        idx.push(0);
        idx.extend_from_slice(&(body.len() as u32).to_be_bytes());
        idx.extend_from_slice(&(defi.len() as u32).to_be_bytes());
        body.extend_from_slice(defi.as_bytes());
    }
    std::fs::write(with_ext(base, ".dict"), &body)?;
    std::fs::write(with_ext(base, ".idx"), &idx)?;
    let ifo = with_ext(base, ".ifo");
    std::fs::write(
        &ifo,
        format!(
            "StarDict's dict ifo file\nversion=2.4.2\nbookname={}\nwordcount={}\nidxfilesize={}\n",
            name,
            entries.len(),
            idx.len()
        ),
    )?;
    Ok(ifo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dictz-sd-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn load_and_lookup() {
        let d = tmpdir("basic");
        let ifo = write_stardict(
            &d.join("t"),
            "Tiny",
            &[("apple", "elma"), ("Book", "kitap"), ("cat", "kedi")],
        )
        .unwrap();
        let mut sd = StarDictDictionary::new(&ifo).unwrap();
        sd.load().unwrap();
        assert_eq!(sd.metadata().name, "Tiny");
        assert_eq!(sd.metadata().word_count, 3);
        assert_eq!(sd.lookup("apple", true)[0].definition, "elma");
        assert_eq!(sd.lookup("book", true)[0].headword, "Book");
        assert!(sd.lookup("ca", true).is_empty());
        assert_eq!(sd.lookup("ca", false)[0].headword, "cat");
        assert_eq!(sd.entries().len(), 3);
    }

    #[test]
    fn opens_from_idx_path_and_gz() {
        let d = tmpdir("gz");
        write_stardict(&d.join("t"), "Gz", &[("a", "b")]).unwrap();
        // gzip the .dict into .dict.dz
        let raw = std::fs::read(d.join("t.dict")).unwrap();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut enc, &raw).unwrap();
        std::fs::write(d.join("t.dict.dz"), enc.finish().unwrap()).unwrap();
        std::fs::remove_file(d.join("t.dict")).unwrap();
        let mut sd = StarDictDictionary::new(d.join("t.idx")).unwrap();
        sd.load().unwrap();
        assert_eq!(sd.lookup("a", true)[0].definition, "b");
    }

    #[test]
    fn rejects_bad_ifo() {
        let d = tmpdir("bad");
        std::fs::write(d.join("x.ifo"), "nope\n").unwrap();
        let mut sd = StarDictDictionary::new(d.join("x.ifo")).unwrap();
        assert!(matches!(sd.load(), Err(DictzError::Malformed(_))));
    }
}
