//! The Dictionary Center against the Python edition: the same answers
//! (`tests/data/center/`) give the same entries (`center_golden.json`), the
//! same licence table, built-in sources and language codes; and the steps
//! (sources file, install, setup archive, catalog build) work end to end.
#![cfg(feature = "center")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dictz::center::adapters::{self, FileFetch};
use dictz::center::model::{normalize, Entry};
use dictz::center::sources::{builtin, clean, Sources};
use dictz::center::{install, langs, licenses, transfer};
use serde_json::{json, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("data")
}

fn fixtures() -> (Vec<dictz::center::sources::Source>, FileFetch) {
    let fx: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("center/fixtures.json")).unwrap(),
    )
    .unwrap();
    let sources = fx["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(clean)
        .collect();
    let mut map = HashMap::new();
    for (k, v) in fx["responses"].as_object().unwrap() {
        map.insert(k.clone(), root().join("center").join(v.as_str().unwrap()));
    }
    (sources, FileFetch { map })
}

struct Tmp(PathBuf);
impl Tmp {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir().join(format!("dictz-center-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Self(d)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn adapters_match_the_python_golden_file() {
    let (sources, http) = fixtures();
    let want: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("parity/center_golden.json")).unwrap(),
    )
    .unwrap();
    for s in &sources {
        let got: Vec<Value> = adapters::read(s, &http, "", "")
            .unwrap()
            .iter()
            .map(Entry::to_value)
            .collect();
        assert_eq!(Value::Array(got), want[&s.id], "source {}", s.id);
    }
}

#[test]
fn licences_sources_languages_match_python() {
    let want: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("parity/licenses_golden.json")).unwrap(),
    )
    .unwrap();
    let got: Vec<Value> = licenses::all()
        .iter()
        .map(|l| json!({"id": l.id, "name": l.name, "url": l.url, "status": l.status, "attribution": l.attribution,
                        "share_alike": l.share_alike, "commercial": l.commercial}))
        .collect();
    assert_eq!(Value::Array(got), want);
    let want: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("parity/sources_golden.json")).unwrap(),
    )
    .unwrap();
    let got: Vec<Value> = builtin().iter().map(|s| s.to_value()).collect();
    assert_eq!(Value::Array(got), want);
    let want: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("parity/langs_golden.json")).unwrap(),
    )
    .unwrap();
    for (k, v) in want.as_object().unwrap() {
        assert_eq!(langs::norm(k), v.as_str().unwrap(), "{}", k);
    }
}

#[test]
fn archive_org_items_resolve() {
    let (_s, http) = fixtures();
    let files = adapters::resolve_archive_org("stardict-en-ru", &http).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[0]["url"],
        "https://archive.org/download/stardict-en-ru/stardict-en-ru/Mueller.tar.bz2"
    );
}

#[test]
fn sources_file() {
    let t = Tmp::new("src");
    let s = Sources::new(&t.0);
    assert_eq!(s.all().len(), builtin().len());
    let mine = s.add(
        &json!({"name": "My server", "type": "dictz-catalog", "url": "https://me.example/c.json"}),
    );
    assert_eq!(mine.id, "my-server");
    assert_eq!(
        s.add(&json!({"name": "My server", "url": "x"})).id,
        "my-server-2"
    );
    s.update(
        "freedict",
        &json!({"url": "https://mirror/fd.json", "enabled": false}),
    );
    assert!(!s.get("freedict").unwrap().enabled);
    s.remove("huzheng");
    assert!(Sources::new(&t.0).get("huzheng").is_none());
    s.set_default("my-server");
    assert_eq!(s.default_id(), "my-server");
    assert_eq!(s.all()[0].id, "my-server");
    s.set_secret("my-server", "pw", "");
    assert_eq!(s.secret("my-server"), ("pw".to_string(), String::new()));
    let out = t.0.join("list.json");
    assert_eq!(s.export(&out, false).unwrap(), 2);
    assert!(!std::fs::read_to_string(&out).unwrap().contains("pw\""));
    s.reset();
    assert!(s.get("huzheng").is_some() && s.get("my-server").is_some());
}

fn tiny_tar_xz() -> Vec<u8> {
    // a one-word StarDict set, packed as .tar then .xz, plus an entry that
    // tries to leave the folder
    let mut idx = b"hello\0".to_vec();
    idx.extend_from_slice(&0u32.to_be_bytes());
    idx.extend_from_slice(&5u32.to_be_bytes());
    let ifo = format!(
        "StarDict's dict ifo file\nversion=2.4.2\nbookname=Tiny\nwordcount=1\nidxfilesize={}\nsametypesequence=m\n",
        idx.len()
    );
    let mut tar = tar::Builder::new(Vec::new());
    for (name, data) in [
        ("tiny/tiny.ifo", ifo.as_bytes()),
        ("tiny/tiny.idx", &idx[..]),
        ("tiny/tiny.dict", b"world"),
    ] {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        tar.append_data(&mut h, name, data).unwrap();
    }
    let raw = tar.into_inner().unwrap();
    let mut out = Vec::new();
    lzma_rs::xz_compress(&mut std::io::Cursor::new(raw), &mut out).unwrap();
    out
}

#[test]
fn install_from_a_local_file_and_uninstall() {
    let t = Tmp::new("inst");
    let data = tiny_tar_xz();
    std::fs::create_dir_all(t.0.join("srv")).unwrap();
    let f = t.0.join("srv/tiny.stardict.tar.xz");
    std::fs::write(&f, &data).unwrap();
    let url = format!("file://{}", f.display());
    let e = normalize(
        &json!({"id": "tiny", "name": "Tiny", "license": "CC0-1.0", "version": "1",
                              "files": [{"url": url, "size": data.len()}]}),
        "test",
        "",
    );
    let inst = install::Installer::new(
        &t.0.join("data"),
        dictz::center::net::Http::new(10.0, ""),
        "",
        true,
        false,
    );
    let mut stages = Vec::new();
    let rec = inst
        .install(&e, &mut |s, _, _| stages.push(s.to_string()), &|| false)
        .unwrap();
    assert!(stages.contains(&"unpack".to_string()));
    let files: Vec<String> = rec["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(files.len(), 1);
    assert!(files[0].ends_with("tiny.ifo"));
    assert!(inst.installed().contains_key("test:tiny"));
    assert_eq!(inst.history()[0]["status"], "done");
    assert!(inst.record_for_path(Path::new(&files[0])).is_some());
    let newer = Entry {
        version: "2".into(),
        ..e.clone()
    };
    assert_eq!(inst.updates(&[newer]).len(), 1);
    assert!(inst.updates(std::slice::from_ref(&e)).is_empty());
    let folder = rec["folder"].as_str().unwrap().to_string();
    assert!(inst.uninstall("test:tiny", true).is_some());
    assert!(!Path::new(&folder).exists());
}

#[test]
fn unpack_skips_escapes() {
    let t = Tmp::new("zip");
    let z = t.0.join("a.zip");
    {
        let mut w = zip::ZipWriter::new(std::fs::File::create(&z).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        for n in ["ok/x.dsl", "../bad.dsl", "/abs.dsl"] {
            w.start_file(n, o).unwrap();
            std::io::Write::write_all(&mut w, b"x").unwrap();
        }
        w.finish().unwrap();
    }
    let out = install::unpack(&z, "zip", &t.0.join("out")).unwrap();
    assert_eq!(out.len(), 1);
    assert!(out[0].ends_with("ok/x.dsl"));
}

#[test]
fn catalog_check_finds_problems() {
    let (n, p) = transfer::check_catalog(&json!({"dictionaries": [{"id": "a"}, {"id": "a"}]}));
    assert_eq!(n, 2);
    assert!(p.iter().any(|x| x.contains("twice")));
    assert!(p.iter().any(|x| x.contains("no files")));
}

#[test]
fn setup_archive_round_trip() {
    let t = Tmp::new("setup");
    let a = t.0.join("a");
    std::fs::create_dir_all(&a).unwrap();
    Sources::new(&a).add(&json!({"name": "Mine", "url": "https://me.example/c.json"}));
    Sources::new(&a).set_secret("mine", "secret-pw", "");
    let out = t.0.join("s.zip");
    let m = transfer::export_setup(
        &out,
        &a,
        &[
            "dictionaries",
            "settings",
            "sources",
            "installed",
            "history",
        ],
    )
    .unwrap();
    assert_eq!(m["dictz-setup"], 1);
    assert!(!String::from_utf8_lossy(&std::fs::read(&out).unwrap()).contains("secret-pw"));
    let b = t.0.join("b");
    let res = transfer::import_setup(&out, &b, false, transfer::PARTS).unwrap();
    assert!(res.sources >= 1);
    assert!(Sources::new(&b).get("mine").is_some());
    // a Python-made archive reads the same way
    assert!(transfer::read_manifest(&out).is_ok());
}

#[test]
fn download_queue_installs_in_the_background() {
    use dictz::center::install::{DownloadQueue, QueueEvent};
    use std::sync::{mpsc, Arc, Mutex};
    let t = Tmp::new("queue");
    let data = tiny_tar_xz();
    std::fs::create_dir_all(t.0.join("srv")).unwrap();
    let f = t.0.join("srv/q.stardict.tar.xz");
    std::fs::write(&f, &data).unwrap();
    let e = normalize(
        &json!({"id": "q", "name": "Q", "license": "CC0-1.0",
                              "files": [format!("file://{}", f.display())]}),
        "test",
        "",
    );
    let inst = Arc::new(install::Installer::new(
        &t.0.join("data"),
        dictz::center::net::Http::new(10.0, ""),
        "",
        true,
        false,
    ));
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let q = DownloadQueue::new(
        Arc::clone(&inst),
        2,
        Arc::new(move |ev| {
            let _ = tx.lock().unwrap().send(ev);
        }),
    );
    q.add(e.clone());
    q.add(e); // queued once
    let mut installed = None;
    while let Ok(ev) = rx.recv_timeout(std::time::Duration::from_secs(20)) {
        if let QueueEvent::Installed(rec, _) = ev {
            installed = Some(rec);
            break;
        }
    }
    let rec = installed.expect("installed");
    assert_eq!(rec["files"].as_array().unwrap().len(), 1);
    assert_eq!(q.snapshot().len(), 1);
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(q.snapshot()[0].state, "done");
    assert_eq!(q.active(), 0);
}
