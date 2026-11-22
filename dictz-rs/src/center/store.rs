//! A JSON file changed by several writers — same rules as the Python
//! edition (`dictz/center/store.py`): `<file>.lock` (O_EXCL, stale after
//! 10 s), read fresh inside the lock, temp file + rename, `<file>.bak`.
//! Written with sorted keys, one-space indent, UTF-8 — byte for byte what
//! the Python edition writes.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Map, Value};

/// `value` with every object's keys sorted.
pub fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                out.insert(k.clone(), sorted(&m[k]));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        o => o.clone(),
    }
}

/// JSON text: sorted keys, `indent` spaces, trailing newline.
pub fn dumps_indent(v: &Value, indent: usize) -> String {
    let ind = vec![b' '; indent];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&ind);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    let _ = serde::Serialize::serialize(&sorted(v), &mut ser);
    let mut s = String::from_utf8(buf).unwrap_or_default();
    s.push('\n');
    s
}

/// The files' format (one-space indent).
pub fn dumps(v: &Value) -> String {
    dumps_indent(v, 1)
}

/// One JSON file.
pub struct JsonFile {
    /// Its path.
    pub path: PathBuf,
    default: fn() -> Value,
}

impl JsonFile {
    /// `default()` when the file is missing or broken (and its backup too).
    pub fn new(path: PathBuf, default: fn() -> Value) -> Self {
        Self { path, default }
    }

    fn backup(&self) -> PathBuf {
        let mut s = self.path.as_os_str().to_os_string();
        s.push(".bak");
        PathBuf::from(s)
    }

    fn load(p: &Path) -> Option<Value> {
        let t = fs::read_to_string(p).ok()?;
        serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()
    }

    /// The data.
    pub fn read(&self) -> Value {
        let mut d = Self::load(&self.path);
        if d.is_none() && self.path.exists() {
            d = Self::load(&self.backup());
        }
        d.unwrap_or_else(self.default)
    }

    /// Write (keeping the previous file as `.bak`).
    pub fn write(&self, data: &Value) -> std::io::Result<()> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir)?;
        if self.path.is_file() && Self::load(&self.path).is_some() {
            let _ = fs::copy(&self.path, self.backup());
        }
        let stem = self
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tmp = dir.join(format!(".{}-{}.json", stem, std::process::id()));
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(dumps(data).as_bytes())?;
        }
        let mut last = Ok(());
        for _ in 0..10 {
            last = fs::rename(&tmp, &self.path);
            if last.is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = fs::remove_file(&tmp);
        last
    }

    /// Read fresh, change, write — under the lock.
    pub fn update(&self, change: impl FnOnce(&mut Value)) -> std::io::Result<Value> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir)?;
        let mut ls = self.path.as_os_str().to_os_string();
        ls.push(".lock");
        let lock = PathBuf::from(ls);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut held = false;
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock)
            {
                Ok(_) => {
                    held = true;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&lock)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| SystemTime::now().duration_since(t).ok())
                        .is_some_and(|age| age > Duration::from_secs(10));
                    if stale {
                        let _ = fs::remove_file(&lock);
                        continue;
                    }
                    if Instant::now() > deadline {
                        break; // do the change anyway rather than lose it
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        let mut data = self.read();
        change(&mut data);
        let res = self.write(&data);
        if held {
            let _ = fs::remove_file(&lock);
        }
        res.map(|_| data)
    }
}
