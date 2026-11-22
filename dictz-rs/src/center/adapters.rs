//! Reading each kind of source into catalog entries — the same rules as
//! the Python edition (`dictz/center/adapters.py`), checked against the same
//! answers (`tests/data/center/`) and the same expected entries
//! (`center_golden.json`). HTML listings and WebDAV answers are read with
//! the same simple patterns in both editions.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Map, Value};

use super::langs;
use super::model::{archive_of, format_of, normalize, sort_key, vint, vstr, Entry};
use super::net::{auth_headers, Fetch};
use super::sources::Source;
use super::util::{
    html_unescape, http_date, posix_name, posix_parent, quote, take, unquote, urlencode, urljoin,
    urlsplit,
};

/// Entries per source at most.
pub const MAX_ENTRIES: usize = 20000;
/// archive.org search rows.
pub const ARCHIVE_ORG_ROWS: usize = 200;

fn rx(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("pattern"))
}

fn dict_ext() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    rx(
        &R,
        r"(?i)\.(ifo|dsl|dsl\.dz|bgl|mdx|xdxf|slob|zim|tar\.gz|tgz|tar\.xz|txz|tar\.bz2|tbz2?|tar|zip|7z)$",
    )
}

/// An adapter failed.
#[derive(Debug, Clone)]
pub struct AdapterError(pub String);

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<super::net::NetError> for AdapterError {
    fn from(e: super::net::NetError) -> Self {
        AdapterError(e.message)
    }
}

type R<T> = Result<T, AdapterError>;

fn parse_json(raw: &[u8], what: &str) -> R<Value> {
    let text = String::from_utf8(raw.to_vec())
        .map_err(|e| AdapterError(format!("{}: not JSON ({})", what, e)))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| AdapterError(format!("{}: not JSON ({})", what, e)))
}

fn wanted(name: &str, flt: &str) -> bool {
    if !flt.is_empty() {
        return Regex::new(flt).map(|r| r.is_match(name)).unwrap_or(false);
    }
    dict_ext().is_match(name)
}

/// File name without its dictionary / archive extensions and a trailing
/// format word.
pub fn stem(name: &str) -> String {
    let low = name.to_lowercase();
    let mut out = None;
    for ext in [".tar.gz", ".tar.xz", ".tar.bz2", ".dsl.dz", ".dict.dz"] {
        if low.ends_with(ext) {
            out = Some(name[..name.len() - ext.len()].to_string());
            break;
        }
    }
    let out = out.unwrap_or_else(|| match name.rfind('.') {
        Some(i) => name[..i].to_string(),
        None => name.to_string(),
    });
    static T: OnceLock<Regex> = OnceLock::new();
    let trimmed = rx(&T, r"(?i)[\s._-]+(stardict|dictd|slob)$")
        .replace(&out, "")
        .to_string();
    if trimmed.is_empty() {
        out
    } else {
        trimmed
    }
}

/// `…/licenses/by-sa/4.0/` → `CC-BY-SA-4.0`; '' when not recognised.
pub fn license_from_url(url: &str) -> String {
    let u = url.to_lowercase();
    static CC: OnceLock<Regex> = OnceLock::new();
    if let Some(m) = rx(&CC, r"creativecommons\.org/licenses/([a-z-]+)/(\d\.\d)").captures(&u) {
        return super::licenses::norm(&format!("CC-{}-{}", m[1].to_uppercase(), &m[2]));
    }
    if u.contains("publicdomain/zero") {
        "CC0-1.0".into()
    } else if u.contains("publicdomain/mark") {
        "public-domain".into()
    } else if u.contains("gnu.org/licenses/gpl") || u.contains("gnu.org/copyleft/gpl") {
        "GPL-3.0-or-later".into()
    } else if u.contains("gnu.org/licenses/fdl") {
        "GFDL-1.3-or-later".into()
    } else {
        String::new()
    }
}

/// `12.3 MB` → bytes (0 when not a size).
pub fn parse_size(text: &str) -> u64 {
    static S: OnceLock<Regex> = OnceLock::new();
    let Some(m) = rx(&S, r"(?i)([\d.,]+)\s*([KMGT]?i?B)\b").captures(text) else {
        return 0;
    };
    let Ok(n) = m[1].replace(',', ".").parse::<f64>() else {
        return 0;
    };
    let unit = m[2].to_uppercase().replace('I', "");
    let mult: f64 = match unit.as_str() {
        "KB" => 1024.0,
        "MB" => 1048576.0,
        "GB" => 1073741824.0,
        "TB" => 1099511627776.0,
        _ => 1.0,
    };
    (n * mult) as u64
}

fn languages(src: &Source, name: &str) -> (String, Vec<String>) {
    if !src.name_pattern.is_empty() {
        if let Ok(re) = Regex::new(&src.name_pattern) {
            if let Some(c) = re.captures(name) {
                let s = langs::norm(c.name("src").map(|m| m.as_str()).unwrap_or(""));
                let d = langs::norm(c.name("dst").map(|m| m.as_str()).unwrap_or(""));
                if !s.is_empty() {
                    let dl = if !d.is_empty() {
                        vec![d]
                    } else if !src.target_lang.is_empty() {
                        vec![langs::norm(&src.target_lang)]
                    } else {
                        vec![]
                    };
                    return (s, dl);
                }
            }
        }
    }
    if let Some((a, b)) = crate::dictident::languages_from_name(name) {
        return (langs::norm(&a), vec![langs::norm(&b)]);
    }
    let s = langs::norm(&src.source_lang);
    let d = langs::norm(&src.target_lang);
    (s, if d.is_empty() { vec![] } else { vec![d] })
}

fn file_entry(src: &Source, rel: &str, url: &str, size: u64, date: &str) -> Map<String, Value> {
    let name = posix_name(rel);
    let title = stem(&name);
    let (s, d) = languages(src, &title);
    let mut m = Map::new();
    m.insert("id".into(), json!(rel.to_lowercase()));
    m.insert("name".into(), json!(title));
    m.insert(
        "files".into(),
        json!([{"name": name, "url": url, "size": size}]),
    );
    m.insert("source_lang".into(), json!(s));
    m.insert("target_lang".into(), json!(d));
    m.insert("date".into(), json!(take(date, 10)));
    m
}

/// A listing's file: (relative path, url, size, date).
pub type Listed = (String, String, u64, String);

/// A listing → entries: a StarDict set or an MDict pair in one folder is one
/// entry; every other dictionary file or archive is its own.
pub fn group_sets(src: &Source, files: &[Listed]) -> Vec<Value> {
    let mut by_dir: BTreeMap<String, Vec<&Listed>> = BTreeMap::new();
    for f in files {
        by_dir.entry(posix_parent(&f.0)).or_default().push(f);
    }
    static PART: OnceLock<Regex> = OnceLock::new();
    static PART2: OnceLock<Regex> = OnceLock::new();
    let part = rx(&PART, r"\.(idx|idx\.gz|dict|dict\.dz|syn|syn\.dz|mdd)$");
    let part2 = rx(&PART2, r"\.(idx|idx\.gz|dict|dict\.dz|syn|mdd)$");
    let mut out = Vec::new();
    for items in by_dir.values() {
        let mut names: BTreeMap<String, &Listed> = BTreeMap::new();
        for it in items {
            names.insert(posix_name(&it.0).to_lowercase(), it);
        }
        let mut used: HashSet<String> = HashSet::new();
        for (low, (rel, _u, _s, date)) in names.iter().map(|(k, v)| (k, *v)) {
            if low.ends_with(".ifo") || low.ends_with(".mdx") {
                let base = &low[..low.len() - 4];
                let mut sib: Vec<&String> = names
                    .keys()
                    .filter(|n| {
                        *n != low
                            && n.starts_with(&format!("{}.", base))
                            && !used.contains(*n)
                            && part.is_match(n)
                    })
                    .collect();
                sib.sort();
                let mut group = vec![names[low]];
                group.extend(sib.iter().map(|n| names[*n]));
                used.insert(low.clone());
                for n in &sib {
                    used.insert((*n).clone());
                }
                let fname = posix_name(rel);
                let title = fname[..fname.len() - 4].to_string();
                let (s, dl) = languages(src, &title);
                let fl: Vec<Value> = group
                    .iter()
                    .map(|(r, u, z, _)| json!({"name": posix_name(r), "url": u, "size": z}))
                    .collect();
                out.push(json!({"id": rel.to_lowercase(), "name": title, "source_lang": s, "target_lang": dl,
                                "date": take(date, 10), "files": fl}));
            }
        }
        for (low, (rel, url, size, date)) in names.iter().map(|(k, v)| (k, *v)) {
            if used.contains(low) || !wanted(&posix_name(rel), &src.filter) {
                continue;
            }
            if part2.is_match(low) {
                continue;
            }
            out.push(Value::Object(file_entry(src, rel, url, *size, date)));
        }
    }
    out
}

/// Normalised, de-duplicated (first wins), sorted entries of a source.
pub fn finish(src: &Source, raw: Vec<Value>) -> Vec<Entry> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in raw {
        let e = normalize(&r, &src.id, &src.license);
        if !seen.insert(e.id.clone()) {
            continue;
        }
        out.push(e);
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out.sort_by_key(sort_key);
    out
}

// ------------------------------------------------------------------ types

fn read_dictz_catalog(src: &Source, http: &dyn Fetch, depth: u8) -> R<Vec<Value>> {
    let url = &src.url;
    let data = parse_json(&http.get(url, &[])?, url)?;
    let (head, rows) = match &data {
        Value::Array(a) => (Map::new(), a.clone()),
        Value::Object(o) => (
            o.get("catalog")
                .and_then(|c| c.as_object())
                .cloned()
                .unwrap_or_default(),
            o.get("dictionaries")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default(),
        ),
        _ => return Err(AdapterError(format!("{}: not a dictz catalog", url))),
    };
    let base = head
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(url)
        .to_string();
    let mut out = Vec::new();
    for r in rows {
        let Value::Object(mut r) = r else { continue };
        let files: Vec<Value> = r
            .get("files")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|f| {
                let mut f = match f {
                    Value::String(s) => json!({"url": s}),
                    o => o,
                };
                let u = vstr(f.get("url"));
                if let Some(o) = f.as_object_mut() {
                    o.insert("url".into(), json!(urljoin(&base, &u)));
                }
                f
            })
            .collect();
        r.insert("files".into(), Value::Array(files));
        if vstr(r.get("license")).is_empty() && !vstr(head.get("license")).is_empty() {
            r.insert("license".into(), head["license"].clone());
        }
        out.push(Value::Object(r));
    }
    if depth == 0 {
        for inc in head
            .get("include")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
        {
            let mut sub = src.clone();
            sub.url = urljoin(url, &vstr(Some(&inc)));
            if let Ok(mut more) = read_dictz_catalog(&sub, http, 1) {
                out.append(&mut more);
            }
        }
    }
    Ok(out)
}

fn read_freedict(src: &Source, http: &dyn Fetch) -> R<Vec<Value>> {
    let data = parse_json(&http.get(&src.url, &[])?, &src.url)?;
    let Value::Array(list) = data else {
        return Err(AdapterError("FreeDict: not a list".into()));
    };
    let mut out = Vec::new();
    for d in list {
        let Some(d) = d.as_object() else { continue };
        if vstr(d.get("name")).is_empty() || !d.get("releases").is_some_and(|r| r.is_array()) {
            continue;
        }
        let mut rels: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
        for r in d["releases"].as_array().unwrap() {
            if let Some(o) = r.as_object() {
                if !vstr(o.get("URL")).is_empty() {
                    rels.insert(vstr(o.get("platform")), o.clone());
                }
            }
        }
        let Some(plat) = ["stardict", "slob", "dictd", "src"]
            .iter()
            .find(|p| rels.contains_key(**p))
        else {
            continue;
        };
        let rel = &rels[*plat];
        let pair = vstr(d.get("name"));
        let (a, b) = pair.split_once('-').unwrap_or((pair.as_str(), ""));
        let (s, t) = (langs::norm(a), langs::norm(b));
        let sn = langs::name(&s);
        let tn = langs::name(&t);
        let title = format!(
            "FreeDict {} → {}",
            if sn.is_empty() { a } else { &sn },
            if tn.is_empty() { b } else { &tn }
        );
        let fmt = match *plat {
            "stardict" => "stardict",
            "slob" => "slob",
            "dictd" => "dictd",
            _ => "",
        };
        let or = |k: &str, alt: &str| {
            let v = vstr(d.get(k));
            if v.is_empty() {
                vstr(rel.get(alt))
            } else {
                v
            }
        };
        out.push(json!({
            "id": pair, "name": title, "source_lang": s, "target_lang": if t.is_empty() { vec![] } else { vec![t.clone()] },
            "headwords": d.get("headwords").cloned().unwrap_or(Value::Null), "version": or("edition", "version"),
            "date": or("date", "date"), "author": vstr(d.get("maintainerName")),
            "homepage": "https://freedict.org/", "page": vstr(d.get("sourceURL")), "format": fmt,
            "tags": rels.keys().cloned().collect::<Vec<_>>(), "description": vstr(d.get("status")),
            "files": [{"url": vstr(rel.get("URL")), "size": rel.get("size").cloned().unwrap_or(Value::Null),
                       "sha512": vstr(rel.get("checksum"))}],
        }));
    }
    Ok(out)
}

fn github(url: &str) -> R<(String, String, String, String)> {
    let (_s, _n, path, _q, _f) = urlsplit(url);
    let parts: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();
    if parts.len() < 2 {
        return Err(AdapterError(format!("not a GitHub repository: {}", url)));
    }
    let owner = parts[0].to_string();
    let repo = parts[1]
        .strip_suffix(".git")
        .unwrap_or(parts[1])
        .to_string();
    let (mut r, mut p) = (String::new(), String::new());
    if parts.len() > 3 && (parts[2] == "tree" || parts[2] == "blob") {
        r = parts[3].to_string();
        p = parts[4..].join("/");
    }
    Ok((owner, repo, r, p))
}

fn gh_headers() -> Vec<(String, String)> {
    vec![("Accept".into(), "application/vnd.github+json".into())]
}

fn read_github_releases(src: &Source, http: &dyn Fetch) -> R<Vec<Value>> {
    let (owner, repo, _r, _p) = github(&src.url)?;
    let api = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=30",
        owner, repo
    );
    let rels = parse_json(&http.get(&api, &gh_headers())?, &api)?;
    let Value::Array(rels) = rels else {
        return Err(AdapterError(format!("{}: unexpected answer", api)));
    };
    let rels: Vec<Value> = rels
        .into_iter()
        .filter(|r| r.is_object() && !r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false))
        .collect();
    let depth = src.depth;
    let n = if depth > 2 {
        (depth - 1).max(1) as usize
    } else {
        1
    };
    let keep: Vec<&Value> = rels.iter().take(n).collect();
    let mut out = Vec::new();
    for r in &keep {
        let tag = vstr(r.get("tag_name"));
        for a in r
            .get("assets")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default()
        {
            let name = vstr(a.get("name"));
            if name.is_empty() || !wanted(&name, &src.filter) {
                continue;
            }
            let mut e = file_entry(
                src,
                &name,
                &vstr(a.get("browser_download_url")),
                vint(a.get("size")),
                &vstr(r.get("published_at")),
            );
            e.insert("version".into(), json!(tag));
            e.insert("page".into(), json!(vstr(r.get("html_url"))));
            e.insert("homepage".into(), json!(src.homepage));
            if keep.len() > 1 {
                e.insert(
                    "id".into(),
                    json!(format!("{}/{}", tag, name).to_lowercase()),
                );
                let nm = vstr(e.get("name"));
                e.insert("name".into(), json!(format!("{} ({})", nm, tag)));
            }
            out.push(Value::Object(e));
        }
    }
    Ok(out)
}

fn read_github_folder(src: &Source, http: &dyn Fetch) -> R<Vec<Value>> {
    let (owner, repo, r, p) = github(&src.url)?;
    let mut files: Vec<Listed> = Vec::new();
    fn walk(
        http: &dyn Fetch,
        owner: &str,
        repo: &str,
        r: &str,
        p: &str,
        depth: i64,
        files: &mut Vec<Listed>,
    ) -> R<()> {
        let mut api = format!("https://api.github.com/repos/{}/{}/contents", owner, repo);
        if !p.is_empty() {
            api.push('/');
            api.push_str(&quote(p));
        }
        if !r.is_empty() {
            api.push_str("?ref=");
            api.push_str(&quote(r));
        }
        let items = parse_json(&http.get(&api, &gh_headers())?, &api)?;
        let Value::Array(mut items) = items else {
            return Err(AdapterError(format!("{}: not a folder", api)));
        };
        items.sort_by_key(|x| vstr(x.get("path")));
        for it in items {
            let kind = vstr(it.get("type"));
            let ipath = vstr(it.get("path"));
            if kind == "dir" && depth > 1 {
                walk(http, owner, repo, r, &ipath, depth - 1, files)?;
            } else if kind == "file" && !vstr(it.get("download_url")).is_empty() {
                files.push((
                    ipath,
                    vstr(it.get("download_url")),
                    vint(it.get("size")),
                    String::new(),
                ));
            }
        }
        Ok(())
    }
    walk(http, &owner, &repo, &r, &p, src.depth.max(1), &mut files)?;
    let mut out = group_sets(src, &files);
    for e in out.iter_mut() {
        if let Some(o) = e.as_object_mut() {
            o.insert("homepage".into(), json!(src.homepage));
        }
    }
    Ok(out)
}

/// `…/x/index.html` → `…/x/` (a folder's own page); other addresses unchanged.
fn folder_of(url: &str) -> String {
    let low = url.to_ascii_lowercase();
    if low.ends_with("/index.html") || low.ends_with("/index.htm") {
        url[..url.rfind('/').map(|i| i + 1).unwrap_or(0)].to_string()
    } else {
        url.to_string()
    }
}

/// An Apache / nginx / lighttpd folder listing, `depth` folders deep; also a
/// static site whose folders are `…/x/index.html` pages (like the huzheng
/// mirror at stardict.uber.space).
fn read_http_index(src: &Source, http: &dyn Fetch) -> R<Vec<Value>> {
    let root = if src.url.ends_with('/') {
        src.url.clone()
    } else {
        format!("{}/", src.url)
    };
    let host = urlsplit(&root).1;
    let mut files: Vec<Listed> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    static HREF: OnceLock<Regex> = OnceLock::new();
    let href_re = rx(&HREF, r#"(?i)<a\s[^>]*?href\s*=\s*["']([^"'#]+)["']"#);
    #[allow(clippy::too_many_arguments)]
    fn walk(
        http: &dyn Fetch,
        re: &Regex,
        root: &str,
        host: &str,
        url: &str,
        depth: i64,
        seen: &mut HashSet<String>,
        files: &mut Vec<Listed>,
    ) -> R<()> {
        let here = folder_of(url);
        if !seen.insert(here.clone()) {
            return Ok(());
        }
        let page = match http.get(url, &[]) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => {
                if here == root {
                    return Err(e.into());
                }
                return Ok(());
            }
        };
        for c in re.captures_iter(&page) {
            let href = html_unescape(&c[1]);
            if href.starts_with('?')
                || href.starts_with("mailto:")
                || href.starts_with("javascript:")
                || href == "../"
                || href == "./"
                || href == "/"
            {
                continue;
            }
            let full = urljoin(url, &href);
            let folder = folder_of(&full);
            if urlsplit(&full).1 != host || !folder.starts_with(root) || folder == here {
                continue;
            }
            if folder.ends_with('/') {
                if depth > 1 {
                    walk(http, re, root, host, &full, depth - 1, seen, files)?;
                }
            } else if !full.contains('?') {
                let rel = unquote(&full[root.len()..]);
                files.push((rel, full, 0, String::new()));
            }
        }
        Ok(())
    }
    walk(
        http,
        href_re,
        &root,
        &host,
        &root,
        src.depth.max(1),
        &mut seen,
        &mut files,
    )?;
    Ok(group_sets(src, &files))
}

/// The PROPFIND body.
pub const PROPFIND: &[u8] = b"<?xml version=\"1.0\" encoding=\"utf-8\"?><d:propfind xmlns:d=\"DAV:\"><d:prop><d:resourcetype/><d:getcontentlength/><d:getlastmodified/></d:prop></d:propfind>";

fn read_dav(
    src: &Source,
    http: &dyn Fetch,
    root: &str,
    headers: &[(String, String)],
) -> R<Vec<Value>> {
    let root = if root.ends_with('/') {
        root.to_string()
    } else {
        format!("{}/", root)
    };
    let (bs, bn, _, _, _) = urlsplit(&root);
    static RESP: OnceLock<Regex> = OnceLock::new();
    static HREF: OnceLock<Regex> = OnceLock::new();
    static LEN: OnceLock<Regex> = OnceLock::new();
    static COLL: OnceLock<Regex> = OnceLock::new();
    static DATE: OnceLock<Regex> = OnceLock::new();
    let resp = rx(
        &RESP,
        r"(?is)<(?:[\w-]+:)?response\b.*?</(?:[\w-]+:)?response>",
    );
    let href = rx(
        &HREF,
        r"(?is)<(?:[\w-]+:)?href>\s*(.*?)\s*</(?:[\w-]+:)?href>",
    );
    let len = rx(&LEN, r"(?i)<(?:[\w-]+:)?getcontentlength>\s*(\d+)\s*<");
    let coll = rx(&COLL, r"(?i)<(?:[\w-]+:)?collection\s*/?>");
    let date = rx(&DATE, r"(?i)<(?:[\w-]+:)?getlastmodified>\s*(.*?)\s*<");
    let mut files: Vec<Listed> = Vec::new();
    let host_root = format!("{}://{}/", bs, bn);
    // depth-first in listing order, like the Python edition's recursion
    fn walk(
        http: &dyn Fetch,
        url: &str,
        depth: i64,
        ctx: (&str, &str, &[(String, String)]),
        res: (&Regex, &Regex, &Regex, &Regex, &Regex),
        files: &mut Vec<Listed>,
    ) -> R<()> {
        let (root, host_root, headers) = ctx;
        let (resp, href, len, coll, date) = res;
        let mut h = headers.to_vec();
        h.push(("Depth".into(), "1".into()));
        h.push(("Content-Type".into(), "application/xml".into()));
        let body = http.request("PROPFIND", url, Some(PROPFIND), &h)?;
        let text = String::from_utf8_lossy(&body).into_owned();
        for block in resp.find_iter(&text) {
            let block = block.as_str();
            let Some(m) = href.captures(block) else {
                continue;
            };
            let hr = html_unescape(&m[1]);
            let full = if hr.starts_with('/') {
                urljoin(host_root, &hr)
            } else {
                urljoin(url, &hr)
            };
            if full.trim_end_matches('/') == url.trim_end_matches('/') {
                continue;
            }
            let rel = match full.strip_prefix(root) {
                Some(r) => unquote(r),
                None => unquote(hr.rsplit('/').next().unwrap_or("")),
            };
            if coll.is_match(block) {
                if depth > 1 {
                    let next = if full.ends_with('/') {
                        full.clone()
                    } else {
                        format!("{}/", full)
                    };
                    walk(http, &next, depth - 1, ctx, res, files)?;
                }
                continue;
            }
            let n = len
                .captures(block)
                .and_then(|c| c[1].parse::<u64>().ok())
                .unwrap_or(0);
            let d = date
                .captures(block)
                .map(|c| http_date(&c[1]))
                .unwrap_or_default();
            files.push((rel, full, n, d));
        }
        Ok(())
    }
    walk(
        http,
        &root,
        src.depth.max(1),
        (&root, &host_root, headers),
        (resp, href, len, coll, date),
        &mut files,
    )?;
    Ok(group_sets(src, &files))
}

fn read_webdav(src: &Source, http: &dyn Fetch, headers: &[(String, String)]) -> R<Vec<Value>> {
    let url = if let Some(r) = src.url.strip_prefix("webdavs://") {
        format!("https://{}", r)
    } else if let Some(r) = src.url.strip_prefix("webdav://") {
        format!("http://{}", r)
    } else {
        src.url.clone()
    };
    read_dav(src, http, &url, headers)
}

/// `https://host/index.php/s/TOKEN` → (WebDAV root, token).
pub fn nextcloud_endpoint(share: &str) -> R<(String, String)> {
    let (s, n, path, _, _) = urlsplit(share);
    static S: OnceLock<Regex> = OnceLock::new();
    let m = rx(&S, r"/s/([A-Za-z0-9]+)")
        .captures(&path)
        .ok_or_else(|| AdapterError(format!("not a share link: {}", share)))?;
    let prefix = match path.find("/index.php/s/") {
        Some(i) => &path[..i],
        None => &path[..m.get(0).map(|x| x.start()).unwrap_or(0)],
    };
    Ok((
        format!("{}://{}{}/public.php/webdav/", s, n, prefix),
        m[1].to_string(),
    ))
}

fn read_nextcloud(src: &Source, http: &dyn Fetch, password: &str) -> R<Vec<Value>> {
    let (root, token) = nextcloud_endpoint(&src.url)?;
    let auth = vec![(
        "Authorization".into(),
        format!(
            "Basic {}",
            super::util::base64(format!("{}:{}", token, password).as_bytes())
        ),
    )];
    read_dav(src, http, &root, &auth)
}

fn read_archive_org(src: &Source, http: &dyn Fetch) -> R<Vec<Value>> {
    let q = if src.filter.is_empty() {
        "stardict"
    } else {
        &src.filter
    };
    let rows = ARCHIVE_ORG_ROWS.to_string();
    let params: Vec<(&str, &str)> = vec![
        ("q", q),
        ("fl[]", "identifier"),
        ("fl[]", "title"),
        ("fl[]", "description"),
        ("fl[]", "licenseurl"),
        ("fl[]", "date"),
        ("fl[]", "item_size"),
        ("fl[]", "language"),
        ("sort[]", "downloads desc"),
        ("rows", &rows),
        ("page", "1"),
        ("output", "json"),
    ];
    let url = format!(
        "https://archive.org/advancedsearch.php?{}",
        urlencode(&params)
    );
    let data = parse_json(&http.get(&url, &[])?, &url)?;
    let docs = data
        .get("response")
        .and_then(|r| r.get("docs"))
        .and_then(|d| d.as_array())
        .cloned()
        .ok_or_else(|| AdapterError("archive.org: unexpected answer".into()))?;
    static TAG: OnceLock<Regex> = OnceLock::new();
    static WS: OnceLock<Regex> = OnceLock::new();
    let tag = rx(&TAG, r"<[^>]+>");
    let ws = rx(&WS, r"\s+");
    let mut out = Vec::new();
    for d in docs {
        let ident = vstr(d.get("identifier"));
        if ident.is_empty() {
            continue;
        }
        let title = match d.get("title") {
            Some(Value::Array(a)) => vstr(a.first()),
            Some(v) if !vstr(Some(v)).is_empty() => vstr(Some(v)),
            _ => ident.clone(),
        };
        let desc = match d.get("description") {
            Some(Value::Array(a)) => a
                .iter()
                .map(|x| vstr(Some(x)))
                .collect::<Vec<_>>()
                .join(" "),
            v => vstr(v),
        };
        let desc = tag.replace_all(&desc, " ").to_string();
        let desc = take(ws.replace_all(&html_unescape(&desc), " ").trim(), 400);
        let (mut s, dl) = languages(src, &title);
        if s.is_empty() {
            let first = match d.get("language") {
                Some(Value::Array(a)) => vstr(a.first()),
                v => vstr(v),
            };
            if !first.is_empty() {
                s = langs::norm(&first);
            }
        }
        out.push(json!({
            "id": ident, "name": title, "description": desc, "source_lang": s, "target_lang": dl,
            "license": license_from_url(&vstr(d.get("licenseurl"))), "date": take(&vstr(d.get("date")), 10),
            "size": d.get("item_size").cloned().unwrap_or(json!(0)),
            "page": format!("https://archive.org/details/{}", ident), "format": "archive",
            "resolve": format!("archive-org:{}", ident),
        }));
    }
    Ok(out)
}

/// An archive.org item's dictionary files (for installing).
pub fn resolve_archive_org(ident: &str, http: &dyn Fetch) -> R<Vec<Value>> {
    let url = format!("https://archive.org/metadata/{}", quote(ident));
    let data = parse_json(&http.get(&url, &[])?, &url)?;
    let mut out = Vec::new();
    for f in data
        .get("files")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default()
    {
        let name = vstr(f.get("name"));
        if name.is_empty() || vstr(f.get("source")) == "metadata" || !dict_ext().is_match(&name) {
            continue;
        }
        out.push(json!({"name": posix_name(&name),
                        "url": format!("https://archive.org/download/{}/{}", quote(ident), quote(&name)),
                        "size": f.get("size").cloned().unwrap_or(json!(0)), "md5": vstr(f.get("md5"))}));
    }
    Ok(out)
}

fn file_uri(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = if s.starts_with('/') {
        s
    } else {
        format!("/{}", s)
    };
    format!("file://{}", quote(&s))
}

fn read_local(src: &Source) -> R<Vec<Value>> {
    let root = super::util::expand_user(&src.url);
    if !root.is_dir() {
        return Err(AdapterError(format!("no such folder: {}", root.display())));
    }
    let depth = src.depth.max(1) as usize;
    let mut found: Vec<(Vec<String>, std::path::PathBuf)> = Vec::new();
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(Vec<String>, std::path::PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else if p.is_file() {
                let rel: Vec<String> = p
                    .strip_prefix(root)
                    .map(|r| {
                        r.components()
                            .map(|c| c.as_os_str().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                out.push((rel, p));
            }
        }
    }
    walk(&root, &root, &mut found);
    found.sort();
    let files: Vec<Listed> = found
        .into_iter()
        .filter(|(rel, _)| rel.len() <= depth)
        .map(|(rel, p)| {
            let abs = std::fs::canonicalize(&p).unwrap_or(p.clone());
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            (rel.join("/"), file_uri(&abs), size, String::new())
        })
        .collect();
    Ok(group_sets(src, &files))
}

fn read_telegram(src: &Source, http: &dyn Fetch) -> R<Vec<Value>> {
    static POST: OnceLock<Regex> = OnceLock::new();
    static DOC: OnceLock<Regex> = OnceLock::new();
    static EXTRA: OnceLock<Regex> = OnceLock::new();
    static TIME: OnceLock<Regex> = OnceLock::new();
    static TEXT: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    let post_re = rx(&POST, r#"data-post="([^"]+)""#);
    let doc_re = rx(
        &DOC,
        r"(?s)tgme_widget_message_document_title[^>]*>(.*?)</div>",
    );
    let extra_re = rx(
        &EXTRA,
        r"(?s)tgme_widget_message_document_extra[^>]*>(.*?)</div>",
    );
    let time_re = rx(&TIME, r#"<time[^>]*datetime="([^"]+)""#);
    let text_re = rx(&TEXT, r"(?s)tgme_widget_message_text[^>]*>(.*?)</div>");
    let tag = rx(&TAG, r"<[^>]+>");
    let mut out = Vec::new();
    let mut before = String::new();
    for _ in 0..src.depth.max(1) {
        let url = if before.is_empty() {
            src.url.clone()
        } else {
            format!("{}?before={}", src.url, before)
        };
        let page = String::from_utf8_lossy(&http.get(&url, &[])?).into_owned();
        let mut posts: Vec<String> = Vec::new();
        for block in page.split("<div class=\"tgme_widget_message_wrap").skip(1) {
            let Some(post) = post_re.captures(block) else {
                continue;
            };
            let post = post[1].to_string();
            posts.push(post.clone());
            let Some(doc) = doc_re.captures(block) else {
                continue;
            };
            let title = html_unescape(&tag.replace_all(&doc[1], ""))
                .trim()
                .to_string();
            let desc = text_re
                .captures(block)
                .map(|t| html_unescape(&tag.replace_all(&t[1], " ")))
                .unwrap_or_default();
            let desc = take(&desc.split_whitespace().collect::<Vec<_>>().join(" "), 400);
            let (s, dl) = languages(src, &title);
            out.push(json!({
                "id": post.to_lowercase(), "name": stem(&title), "description": desc,
                "size": extra_re.captures(block).map(|x| parse_size(&x[1])).unwrap_or(0),
                "source_lang": s, "target_lang": dl,
                "date": take(&time_re.captures(block).map(|x| x[1].to_string()).unwrap_or_default(), 10),
                "page": format!("https://t.me/{}", post), "format": format_of(&title), "tags": ["telegram"],
            }));
        }
        let nums: Vec<u64> = posts
            .iter()
            .filter_map(|p| p.rsplit('/').next().and_then(|x| x.parse().ok()))
            .collect();
        match nums.iter().min() {
            Some(m) => before = m.to_string(),
            None => break,
        }
    }
    Ok(out)
}

/// Every entry of `src` (normalised).
pub fn read(src: &Source, http: &dyn Fetch, password: &str, token: &str) -> R<Vec<Entry>> {
    let raw = match src.kind.as_str() {
        "dictz-catalog" => read_dictz_catalog(src, http, 0)?,
        "freedict" => read_freedict(src, http)?,
        "github-releases" => read_github_releases(src, http)?,
        "github-folder" => read_github_folder(src, http)?,
        "http-index" => read_http_index(src, http)?,
        "webdav" => read_webdav(src, http, &auth_headers(src, password, token))?,
        "nextcloud-share" => read_nextcloud(src, http, password)?,
        "archive-org" => read_archive_org(src, http)?,
        "local" => read_local(src)?,
        "telegram" => read_telegram(src, http)?,
        t => return Err(AdapterError(format!("unknown source type: {}", t))),
    };
    Ok(finish(src, raw))
}

/// "Add from link…": one entry for any address of a file or archive.
pub fn entry_from_url(url: &str) -> Entry {
    let path = urlsplit(url).2;
    let mut name = unquote(path.trim_end_matches('/').rsplit('/').next().unwrap_or(""));
    if name.is_empty() {
        name = url.to_string();
    }
    let src = Source {
        id: "link".into(),
        license: "unknown".into(),
        ..Default::default()
    };
    let mut e = file_entry(&src, &name, url, 0, "");
    static NON: OnceLock<Regex> = OnceLock::new();
    let mut id = rx(&NON, "[^a-z0-9._-]+")
        .replace_all(&name.to_lowercase(), "-")
        .trim_matches('-')
        .to_string();
    if id.is_empty() {
        id = "download".into();
    }
    e.insert("id".into(), json!(id));
    e.insert("page".into(), json!(url));
    if archive_of(&name).is_empty() && format_of(&name) == "unknown" {
        e.insert("format".into(), json!("archive"));
    }
    normalize(&Value::Object(e), "link", "unknown")
}

/// For the golden tests: a fake server answering from files.
pub struct FileFetch {
    /// `"METHOD url"` → file.
    pub map: HashMap<String, std::path::PathBuf>,
}

impl Fetch for FileFetch {
    fn request(
        &self,
        method: &str,
        url: &str,
        _body: Option<&[u8]>,
        _headers: &[(String, String)],
    ) -> Result<Vec<u8>, super::net::NetError> {
        let key = format!("{} {}", method, url);
        match self.map.get(&key) {
            Some(p) => std::fs::read(p).map_err(|e| super::net::NetError {
                message: e.to_string(),
                status: 0,
            }),
            None => Err(super::net::NetError {
                message: format!("HTTP 404: {}", url),
                status: 404,
            }),
        }
    }
}
