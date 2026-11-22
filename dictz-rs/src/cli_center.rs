//! Command line of the Dictionary Center — the same commands, options and
//! output as the Python edition's `dictz/cli_center.py`:
//!
//! ```text
//! dct sources [list|add|edit|remove|enable|disable|default|up|down|refresh|export|import|reset]
//! dct catalog [search TEXT|show ID|languages|updates|build FOLDER|check FILE|URL]
//! dct install ID|NAME|URL...        dct uninstall ID|NAME...
//! dct downloads [list|clear]        dct licenses
//! dct setup export|import|show FILE
//! ```

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde_json::{json, Map, Value};

use super::{die, group, repr, table, Ctx};
use crate::center::catalog::{Cached, Catalog};
use crate::center::install::Installer;
use crate::center::model::{human_size, pair_text, Entry, Filter};
use crate::center::net::{Fetch, Http};
use crate::center::sources::{guess_type, Sources};
use crate::center::{adapters, langs, licenses, store, transfer, util};
use crate::registry::norm_path;

const SOURCE_ACTIONS: &[&str] = &[
    "list", "add", "edit", "remove", "enable", "disable", "default", "up", "down", "refresh",
    "export", "import", "reset",
];
const CATALOG_ACTIONS: &[&str] = &["search", "show", "languages", "updates", "build", "check"];
const OFFLINE: &str = "offline: center.online is off (dct config set center.online true)";

#[derive(Args)]
pub struct SourceOpts {
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
    /// dictz-catalog, freedict, github-releases, github-folder, http-index, webdav, nextcloud-share, archive-org, local, telegram
    #[arg(long = "type", value_name = "TYPE")]
    kind: Option<String>,
    /// (edit) a new address
    #[arg(long, value_name = "URL")]
    url: Option<String>,
    /// file names to list (archive.org: the search query)
    #[arg(long, value_name = "REGEX")]
    filter: Option<String>,
    /// languages from names: (?P<src>…) (?P<dst>…)
    #[arg(long, value_name = "REGEX")]
    name_pattern: Option<String>,
    /// source language when the source does not say
    #[arg(long = "from", value_name = "LANG")]
    src: Option<String>,
    /// target language when the source does not say
    #[arg(long = "to", value_name = "LANG")]
    dst: Option<String>,
    /// licence when the source does not say
    #[arg(long, value_name = "ID")]
    license: Option<String>,
    /// folders deep (listings) / pages (Telegram)
    #[arg(long, value_name = "N")]
    depth: Option<i64>,
    #[arg(long, value_parser = ["none", "basic", "bearer", "header"])]
    auth: Option<String>,
    /// user name (basic) or header name (header)
    #[arg(long, value_name = "USER")]
    user: Option<String>,
    /// kept in credentials.json, never exported
    #[arg(long, value_name = "PASSWORD")]
    password: Option<String>,
    /// kept in credentials.json, never exported
    #[arg(long, value_name = "TOKEN")]
    token: Option<String>,
    #[arg(long, value_name = "TEXT")]
    description: Option<String>,
    /// (add) add switched off
    #[arg(long)]
    disabled: bool,
}

#[derive(Subcommand)]
pub enum CenterCmd {
    /// dictionary servers / sources: list, add, edit, remove, refresh, export, import
    Sources {
        #[arg(default_value = "list", value_parser = clap::builder::PossibleValuesParser::new(SOURCE_ACTIONS))]
        action: String,
        #[arg(value_name = "ID|URL|FILE")]
        args: Vec<String>,
        #[command(flatten)]
        opts: SourceOpts,
        /// (import) drop your own sources first
        #[arg(long)]
        replace: bool,
        /// (export) only your own sources
        #[arg(long)]
        mine: bool,
        #[arg(long)]
        json: bool,
    },
    /// search the catalog of downloadable dictionaries; build or check a catalog
    Catalog(CatalogArgs),
    /// download and install dictionaries from the catalog or a link
    Install {
        #[arg(required = true, value_name = "ID|NAME|URL")]
        targets: Vec<String>,
        /// skip checksums
        #[arg(long)]
        no_verify: bool,
        /// keep the downloaded archive
        #[arg(long)]
        keep_archive: bool,
        /// install only, do not add to the list
        #[arg(long)]
        no_add: bool,
        /// read the sources first
        #[arg(long)]
        refresh: bool,
    },
    /// remove dictionaries installed from the catalog
    Uninstall {
        #[arg(required = true, value_name = "ID|NAME")]
        targets: Vec<String>,
        /// remove from dictz, keep the folder
        #[arg(long)]
        keep_files: bool,
    },
    /// download history
    Downloads {
        #[arg(default_value = "list", value_parser = ["list", "clear"])]
        action: String,
        #[arg(short = 'n', long, default_value_t = 50, value_name = "N")]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// licences dictz knows and what they allow
    Licenses {
        #[arg(long)]
        json: bool,
    },
    /// export / import the whole setup (list, settings, sources, history, files)
    Setup {
        #[arg(value_parser = ["export", "import", "show"])]
        action: String,
        #[arg(value_name = "FILE")]
        file: PathBuf,
        /// (export) put the dictionary files in too
        #[arg(long)]
        with_files: bool,
        #[arg(long)]
        no_settings: bool,
        #[arg(long)]
        no_sources: bool,
        #[arg(long)]
        no_history: bool,
        /// leave out the dictionary list
        #[arg(long)]
        no_list: bool,
        /// (import) replace instead of merging
        #[arg(long)]
        replace: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args)]
pub struct CatalogArgs {
    /// search (default) | show | languages | updates | build | check
    #[arg(default_value = "search", value_name = "ACTION")]
    action: String,
    #[arg(value_name = "TEXT|ID|FOLDER|FILE")]
    args: Vec<String>,
    /// source language (en, eng, English …)
    #[arg(long = "from", value_name = "LANG")]
    src: Option<String>,
    /// target language
    #[arg(long = "to", value_name = "LANG")]
    dst: Option<String>,
    /// only this source
    #[arg(long, value_name = "ID")]
    source: Option<String>,
    /// open = open licence, free = open or free to use
    #[arg(long, default_value = "all", value_parser = ["all", "open", "free", "unknown", "nonfree"])]
    license: String,
    /// stardict, dsl, babylon, mdict, dictd …
    #[arg(long = "format", value_name = "FORMAT")]
    fmt: Option<String>,
    /// only what dictz opens today
    #[arg(long)]
    supported: bool,
    /// read the sources first
    #[arg(long)]
    refresh: bool,
    #[arg(short = 'n', long, default_value_t = 0, value_name = "N")]
    limit: usize,
    /// (build) where the files will be served
    #[arg(long, value_name = "URL")]
    base_url: Option<String>,
    /// (build) default: FOLDER/catalog.json
    #[arg(short = 'o', long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// (build) the catalog's name
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
    /// (build) licence when a dictionary has none
    #[arg(long, value_name = "ID")]
    default_license: Option<String>,
    /// (build) do not open the dictionaries
    #[arg(long)]
    no_read: bool,
    #[arg(long)]
    json: bool,
}

// ------------------------------------------------------------------ helpers

/// JSON as the Python edition prints it (keys sorted, indent 2).
fn print_json(v: &Value) {
    super::print_json(&store::sorted(v));
}

fn http(ctx: &mut Ctx) -> Http {
    let p = ctx.prefs();
    Http::new(p.float("center.timeout"), &p.str("center.proxy"))
}

fn installer(ctx: &mut Ctx, verify: bool, keep: bool) -> Installer {
    let h = http(ctx);
    let data_dir = ctx.data_dir.clone();
    let p = ctx.prefs();
    let root = p.str("center.install_folder").trim().to_string();
    Installer::new(
        &data_dir,
        h,
        &root,
        verify && p.bool("center.verify_checksums"),
        keep || p.bool("center.keep_archives"),
    )
}

fn online(ctx: &mut Ctx) -> bool {
    ctx.prefs().bool("center.online")
}

fn refresh(ctx: &mut Ctx, cat: &Catalog, ids: Option<&[String]>, quiet: bool) {
    if !online(ctx) {
        eprintln!("{}", OFFLINE);
        return;
    }
    let h = http(ctx);
    if !quiet {
        eprintln!("reading sources…");
    }
    let prog = |sid: &str, r: &Cached| {
        if !quiet {
            let state = if r.ok {
                format!("{} dictionaries", r.entries.len())
            } else {
                format!("error: {}", r.error)
            };
            eprintln!("  {}: {}", sid, state);
        }
    };
    cat.refresh(ids, 0.0, &h, &prog, 4);
}

fn never_read(cat: &Catalog) -> bool {
    !cat.sources
        .enabled()
        .iter()
        .any(|s| !cat.cached(&s.id).fetched.is_empty())
}

fn pair(e: &Entry) -> String {
    let p = pair_text(e);
    if p.is_empty() {
        "-".into()
    } else {
        p
    }
}

fn or_dash(s: &str) -> String {
    if s.is_empty() {
        "-".into()
    } else {
        s.to_string()
    }
}

fn take(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn source_id(src: &Sources, key: &str) -> String {
    match src.find(key) {
        Some(s) => s.id,
        None => die(format!(
            "error: no source {} (see `dct sources`)",
            repr(key)
        )),
    }
}

fn source_fields(o: &SourceOpts) -> Map<String, Value> {
    let mut m = Map::new();
    let pairs = [
        ("name", &o.name),
        ("type", &o.kind),
        ("filter", &o.filter),
        ("name_pattern", &o.name_pattern),
        ("source_lang", &o.src),
        ("target_lang", &o.dst),
        ("license", &o.license),
        ("auth", &o.auth),
        ("user", &o.user),
        ("description", &o.description),
    ];
    for (k, v) in pairs {
        if let Some(v) = v {
            m.insert(k.into(), Value::from(v.as_str()));
        }
    }
    if let Some(d) = o.depth {
        m.insert("depth".into(), Value::from(d));
    }
    m
}

fn yn(b: bool) -> String {
    if b { "yes" } else { "no" }.into()
}

/// Run one center command.
pub fn run(ctx: &mut Ctx, cmd: CenterCmd) -> i32 {
    match cmd {
        CenterCmd::Sources {
            action,
            args,
            opts,
            replace,
            mine,
            json,
        } => cmd_sources(ctx, &action, &args, &opts, replace, mine, json),
        CenterCmd::Catalog(a) => cmd_catalog(ctx, a),
        CenterCmd::Install {
            targets,
            no_verify,
            keep_archive,
            no_add,
            refresh,
        } => cmd_install(ctx, &targets, no_verify, keep_archive, no_add, refresh),
        CenterCmd::Uninstall {
            targets,
            keep_files,
        } => cmd_uninstall(ctx, &targets, keep_files),
        CenterCmd::Downloads {
            action,
            limit,
            json,
        } => cmd_downloads(ctx, &action, limit, json),
        CenterCmd::Licenses { json } => cmd_licenses(json),
        CenterCmd::Setup {
            action,
            file,
            with_files,
            no_settings,
            no_sources,
            no_history,
            no_list,
            replace,
            json,
        } => {
            let parts: Vec<&str> = transfer::PARTS
                .iter()
                .copied()
                .filter(|p| {
                    !((*p == "dictionaries" && no_list)
                        || (*p == "settings" && no_settings)
                        || (*p == "sources" && no_sources)
                        || (*p == "history" && no_history)
                        || (*p == "files" && action == "export" && !with_files))
                })
                .collect();
            cmd_setup(ctx, &action, &file, &parts, replace, json)
        }
    }
}

// ------------------------------------------------------------------ sources

#[allow(clippy::too_many_arguments)]
fn cmd_sources(
    ctx: &mut Ctx,
    a: &str,
    args: &[String],
    o: &SourceOpts,
    replace: bool,
    mine: bool,
    as_json: bool,
) -> i32 {
    let src = Sources::new(&ctx.data_dir);
    match a {
        "list" => {
            let cat = Catalog::new(&ctx.data_dir);
            let def = src.default_id();
            let rows: Vec<Value> = src
                .all()
                .iter()
                .map(|s| {
                    let c = cat.cached(&s.id);
                    let mut v = s.to_value();
                    let m = v.as_object_mut().unwrap();
                    m.insert("default".into(), Value::from(s.id == def));
                    m.insert("entries".into(), Value::from(c.entries.len()));
                    m.insert("checked".into(), Value::from(take(&c.fetched, 10)));
                    m.insert("ok".into(), Value::from(c.ok));
                    m.insert("error".into(), Value::from(c.error.clone()));
                    v
                })
                .collect();
            if as_json {
                print_json(&Value::Array(rows));
                return 0;
            }
            let t: Vec<Vec<String>> = rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let s = |k: &str| r[k].as_str().unwrap_or("").to_string();
                    let state = if r["ok"].as_bool().unwrap_or(false) {
                        "ok"
                    } else if !s("error").is_empty() {
                        "error"
                    } else {
                        "-"
                    };
                    vec![
                        (i + 1).to_string(),
                        yn(r["enabled"].as_bool().unwrap_or(false)),
                        format!(
                            "{}{}",
                            s("id"),
                            if r["default"].as_bool().unwrap_or(false) {
                                " *"
                            } else {
                                ""
                            }
                        ),
                        s("type"),
                        r["entries"].to_string(),
                        or_dash(&s("checked")),
                        state.into(),
                        s("url"),
                    ]
                })
                .collect();
            println!(
                "{}",
                table(
                    &t,
                    &["#", "on", "id", "type", "entries", "checked", "state", "url"]
                )
            );
            println!("\n* = default server · dct sources refresh · dct sources add URL");
            0
        }
        "add" => {
            let Some(url) = args.first() else {
                die("error: dct sources add URL [--name NAME] [--type TYPE] …")
            };
            let mut f = source_fields(o);
            f.entry("type")
                .or_insert_with(|| Value::from(guess_type(url)));
            let has_name = f
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|n| !n.is_empty());
            if !has_name {
                let tail = url.trim_end_matches('/').rsplit('/').next().unwrap_or("");
                f.insert(
                    "name".into(),
                    Value::from(if tail.is_empty() { url.as_str() } else { tail }),
                );
            }
            f.insert("url".into(), Value::from(url.as_str()));
            f.insert("enabled".into(), Value::from(!o.disabled));
            let s = src.add(&Value::Object(f));
            let pw = o.password.clone().unwrap_or_default();
            let tok = o.token.clone().unwrap_or_default();
            if !pw.is_empty() || !tok.is_empty() {
                src.set_secret(&s.id, &pw, &tok);
            }
            println!("added: {} ({}) {}", s.id, s.kind, s.url);
            0
        }
        "reset" => {
            src.reset();
            println!("built-in sources reset");
            0
        }
        "refresh" => {
            let ids: Vec<String> = args.iter().map(|x| source_id(&src, x)).collect();
            if !online(ctx) {
                die(format!("error: {}", OFFLINE));
            }
            let h = http(ctx);
            let cat = Catalog::new(&ctx.data_dir);
            let res = cat.refresh(
                if ids.is_empty() { None } else { Some(&ids) },
                0.0,
                &h,
                &|_, _| {},
                4,
            );
            let mut rc = 0;
            for (sid, r) in res {
                if r.ok {
                    println!("{}: {} dictionaries", sid, r.entries.len());
                } else {
                    println!("{}: error: {}", sid, r.error);
                    rc = 1;
                }
            }
            rc
        }
        "export" => {
            let Some(file) = args.first() else {
                die("error: dct sources export FILE")
            };
            match src.export(Path::new(file), !mine) {
                Ok(n) => {
                    println!("exported {} sources → {}", n, file);
                    0
                }
                Err(e) => die(format!("error: {}", e)),
            }
        }
        "import" => {
            let Some(file) = args.first() else {
                die("error: dct sources import FILE")
            };
            match src.import(Path::new(file), replace) {
                Ok(n) => {
                    println!("imported {} sources", n);
                    0
                }
                Err(e) => die(format!("error: {}", e)),
            }
        }
        _ => {
            if args.is_empty() {
                die(format!("error: dct sources {} ID", a));
            }
            for key in args {
                let sid = source_id(&src, key);
                match a {
                    "edit" => {
                        let mut f = source_fields(o);
                        if let Some(u) = &o.url {
                            f.insert("url".into(), Value::from(u.as_str()));
                        }
                        src.update(&sid, &Value::Object(f));
                        if o.password.is_some() || o.token.is_some() {
                            let (old_pw, old_tok) = src.secret(&sid);
                            src.set_secret(
                                &sid,
                                o.password.as_deref().unwrap_or(&old_pw),
                                o.token.as_deref().unwrap_or(&old_tok),
                            );
                        }
                        println!("changed: {}", sid);
                    }
                    "remove" => {
                        src.remove(&sid);
                        println!("removed: {}", sid);
                    }
                    "enable" | "disable" => {
                        src.set_enabled(&sid, a == "enable");
                        println!("{}d: {}", a, sid);
                    }
                    "default" => {
                        src.set_default(&sid);
                        println!("default server: {}", sid);
                    }
                    _ => {
                        src.move_by(&sid, if a == "up" { -1 } else { 1 });
                        println!("moved {}: {}", a, sid);
                    }
                }
            }
            0
        }
    }
}

// ------------------------------------------------------------------ catalog

fn cmd_catalog(ctx: &mut Ctx, a: CatalogArgs) -> i32 {
    let (action, rest): (String, Vec<String>) = if CATALOG_ACTIONS.contains(&a.action.as_str()) {
        (a.action.clone(), a.args.clone())
    } else {
        let mut r = vec![a.action.clone()];
        r.extend(a.args.iter().cloned());
        ("search".into(), r)
    };
    if action == "build" {
        return catalog_build(&a, &rest);
    }
    if action == "check" {
        return catalog_check(ctx, &rest);
    }
    let cat = Catalog::new(&ctx.data_dir);
    if a.refresh || never_read(&cat) {
        refresh(ctx, &cat, None, false);
    }
    match action.as_str() {
        "show" => {
            if rest.is_empty() {
                die("error: dct catalog show ID");
            }
            let key = rest.join(" ");
            let hits = cat.find(&key);
            let Some(e) = hits.first() else {
                die(format!("error: no catalog entry {}", repr(&key)))
            };
            if a.json {
                print_json(&e.to_value());
                return 0;
            }
            let lic = licenses::get(&e.license);
            let lines: Vec<(&str, String)> = vec![
                ("id", e.id.clone()),
                ("name", e.name.clone()),
                ("languages", pair(e)),
                ("kind", e.kind.clone()),
                (
                    "format",
                    format!(
                        "{}{}",
                        e.format,
                        if e.supported == Some(false) {
                            " (not opened by dictz yet)"
                        } else {
                            ""
                        }
                    ),
                ),
                (
                    "headwords",
                    if e.headwords > 0 {
                        group(e.headwords as i64)
                    } else {
                        "-".into()
                    },
                ),
                ("size", human_size(e.size)),
                ("version", or_dash(&e.version)),
                ("date", or_dash(&e.date)),
                (
                    "license",
                    format!(
                        "{} [{}]{}",
                        lic.name,
                        lic.status,
                        if e.license_url.is_empty() {
                            String::new()
                        } else {
                            format!(" {}", e.license_url)
                        }
                    ),
                ),
                ("author", or_dash(&e.author)),
                ("source", e.source.clone()),
                ("homepage", or_dash(&e.homepage)),
                ("page", or_dash(&e.page)),
                ("description", or_dash(&e.description)),
            ];
            for (k, v) in lines {
                println!("{:<13}{}", format!("{}:", k), v);
            }
            for f in &e.files {
                println!("{:<13}{} ({})", "file:", f.url, human_size(f.size));
            }
            0
        }
        "languages" => {
            let (s, t) = cat.languages();
            if a.json {
                print_json(&json!({"from": s, "to": t}));
                return 0;
            }
            let lab = |v: &[String]| {
                v.iter()
                    .map(|c| langs::label(c))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!("from: {}", lab(&s));
            println!("to:   {}", lab(&t));
            0
        }
        "updates" => {
            let ups = installer(ctx, true, false).updates(&cat.entries(true));
            if ups.is_empty() {
                println!("all installed dictionaries are up to date");
                return 0;
            }
            for (rec, e) in ups {
                let s = |k: &str| rec.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                let old = [s("version"), s("date")]
                    .into_iter()
                    .find(|x| !x.is_empty())
                    .unwrap_or_else(|| "?".into());
                let new = if e.version.is_empty() {
                    e.date.clone()
                } else {
                    e.version.clone()
                };
                println!("{}: {} → {}", e.id, old, new);
            }
            0
        }
        _ => {
            let f = Filter {
                text: rest.join(" "),
                src: a.src.as_deref().map(langs::norm).unwrap_or_default(),
                dst: a.dst.as_deref().map(langs::norm).unwrap_or_default(),
                source: a.source.clone().unwrap_or_default(),
                license: a.license.clone(),
                fmt: a.fmt.clone().unwrap_or_default().to_lowercase(),
                supported_only: a.supported,
            };
            let mut hits = cat.search(&f);
            if a.limit > 0 {
                hits.truncate(a.limit);
            }
            if a.json {
                print_json(&Value::Array(hits.iter().map(Entry::to_value).collect()));
                return 0;
            }
            if hits.is_empty() {
                println!("nothing found (dct sources refresh to read the sources again)");
                return 0;
            }
            let inst = installer(ctx, true, false).installed();
            let t: Vec<Vec<String>> = hits
                .iter()
                .map(|e| {
                    vec![
                        e.id.clone(),
                        take(&e.name, 48),
                        pair(e),
                        e.format.clone(),
                        human_size(e.size),
                        licenses::get(&e.license).id,
                        if inst.contains_key(&e.id) { "yes" } else { "" }.into(),
                    ]
                })
                .collect();
            println!(
                "{}",
                table(
                    &t,
                    &[
                        "id",
                        "name",
                        "languages",
                        "format",
                        "size",
                        "license",
                        "installed"
                    ]
                )
            );
            println!(
                "\n{} dictionaries · dct install ID · dct catalog show ID",
                hits.len()
            );
            0
        }
    }
}

fn catalog_build(a: &CatalogArgs, rest: &[String]) -> i32 {
    let (Some(first), Some(base)) = (rest.first(), a.base_url.as_deref()) else {
        die("error: dct catalog build FOLDER --base-url URL [-o FILE]")
    };
    let folder = util::expand_user(first);
    if !folder.is_dir() {
        die(format!("error: no such folder: {}", folder.display()));
    }
    let cat = transfer::build_catalog(
        &folder,
        base,
        a.name.as_deref().unwrap_or(""),
        a.default_license.as_deref().unwrap_or(""),
        !a.no_read,
    );
    let out = a
        .output
        .clone()
        .unwrap_or_else(|| folder.join("catalog.json"));
    if let Err(e) = std::fs::write(&out, store::dumps(&cat)) {
        die(format!("error: {}: {}", out.display(), e));
    }
    let n = cat["dictionaries"].as_array().map_or(0, Vec::len);
    println!("{} dictionaries → {}", n, out.display());
    0
}

fn catalog_check(ctx: &mut Ctx, rest: &[String]) -> i32 {
    let Some(target) = rest.first() else {
        die("error: dct catalog check FILE|URL")
    };
    let raw: Result<Vec<u8>, String> =
        if target.starts_with("http://") || target.starts_with("https://") {
            http(ctx).get(target, &[]).map_err(|e| e.to_string())
        } else {
            std::fs::read(target).map_err(|e| e.to_string())
        };
    let data: Result<Value, String> = raw.and_then(|b| {
        let b = b.strip_prefix(b"\xef\xbb\xbf".as_slice()).unwrap_or(&b);
        serde_json::from_slice(b).map_err(|e| e.to_string())
    });
    let data = match data {
        Ok(d) => d,
        Err(e) => {
            println!("error: {}: {}", target, e);
            return 1;
        }
    };
    let (n, problems) = transfer::check_catalog(&data);
    for p in &problems {
        println!("{}", p);
    }
    let errors = problems.iter().filter(|p| p.starts_with("error")).count();
    println!(
        "{} dictionaries · {} errors · {} warnings",
        n,
        errors,
        problems.len() - errors
    );
    i32::from(errors > 0)
}

// ------------------------------------------------------------------ install

fn cmd_install(
    ctx: &mut Ctx,
    targets: &[String],
    no_verify: bool,
    keep: bool,
    no_add: bool,
    do_refresh: bool,
) -> i32 {
    let cat = Catalog::new(&ctx.data_dir);
    if do_refresh {
        refresh(ctx, &cat, None, false);
    }
    let inst = installer(ctx, !no_verify, keep);
    if !online(ctx) && targets.iter().any(|t| !t.starts_with("file:")) {
        die(format!("error: {}", OFFLINE));
    }
    let mut rc = 0;
    for t in targets {
        let entry =
            if t.starts_with("http://") || t.starts_with("https://") || t.starts_with("file:") {
                adapters::entry_from_url(t)
            } else {
                let mut hits = cat.find(t);
                if hits.is_empty() && never_read(&cat) {
                    refresh(ctx, &cat, None, false);
                    hits = cat.find(t);
                }
                if hits.is_empty() {
                    eprintln!("error: no catalog entry {} (dct catalog {})", repr(t), t);
                    rc = 1;
                    continue;
                }
                if hits.len() > 1 && hits[0].id != *t {
                    let ids: Vec<&str> = hits.iter().take(8).map(|h| h.id.as_str()).collect();
                    eprintln!(
                        "error: {} matches {} entries: {}",
                        repr(t),
                        hits.len(),
                        ids.join(", ")
                    );
                    rc = 1;
                    continue;
                }
                hits.remove(0)
            };
        let mut last: i64 = -1;
        let name = entry.name.clone();
        let mut prog = |stage: &str, done: u64, total: u64| {
            if stage == "download" && total > 0 {
                let pct = (done * 100 / total) as i64;
                if pct != last && pct % 10 == 0 {
                    last = pct;
                    eprintln!("  {}: {}%", name, pct);
                }
            }
        };
        eprintln!("downloading: {}", entry.name);
        let rec = match inst.install(&entry, &mut prog, &|| false) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {}: {}", entry.name, e.message);
                rc = 1;
                continue;
            }
        };
        let files: Vec<String> = rec["files"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "installed: {} → {} ({} dictionaries)",
            entry.name,
            rec["folder"].as_str().unwrap_or(""),
            files.len()
        );
        if files.is_empty() {
            println!("  dictz cannot open this format yet; the files are kept in the folder above");
        }
        if !no_add {
            for f in files {
                let mut qm = ctx.qm(None, false);
                let res = qm.add_dictionary(Path::new(&f), false);
                let info = res.as_ref().ok().and_then(|n| {
                    qm.dictionary(n)
                        .map(|d| (d.metadata().format.clone(), d.metadata().word_count))
                });
                ctx.index = qm.index.take();
                match res {
                    Ok(n) => {
                        let (fmt, words) = info.unwrap_or_default();
                        println!("added: {} ({}, {} words)", n, fmt, group(words as i64));
                    }
                    Err(e) => {
                        eprintln!("error: {}: {}", f, e);
                        rc = 1;
                    }
                }
            }
        }
    }
    rc
}

fn cmd_uninstall(ctx: &mut Ctx, targets: &[String], keep_files: bool) -> i32 {
    let inst = installer(ctx, true, false);
    let recs = inst.installed();
    let mut rc = 0;
    for t in targets {
        let rid = if recs.contains_key(t) {
            Some(t.clone())
        } else {
            recs.iter()
                .find(|(_, r)| r["name"].as_str().unwrap_or("").to_lowercase() == t.to_lowercase())
                .map(|(k, _)| k.clone())
        };
        let Some(rid) = rid else {
            eprintln!(
                "error: not installed from the catalog: {} (dct downloads)",
                repr(t)
            );
            rc = 1;
            continue;
        };
        let rec = &recs[&rid];
        let files: std::collections::HashSet<String> = rec["files"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|f| norm_path(Path::new(f)))
                    .collect()
            })
            .unwrap_or_default();
        for row in ctx.rows() {
            if files.contains(&norm_path(&row.path)) {
                let _ = ctx.index().remove_dictionary(&row.name);
                println!("removed: {}", row.name);
            }
        }
        inst.uninstall(&rid, !keep_files);
        let name = rec["name"].as_str().unwrap_or("");
        if keep_files {
            println!("uninstalled: {}", name);
        } else {
            println!(
                "uninstalled: {} (deleted {})",
                name,
                rec["folder"].as_str().unwrap_or("")
            );
        }
    }
    rc
}

fn cmd_downloads(ctx: &mut Ctx, action: &str, limit: usize, as_json: bool) -> i32 {
    let inst = installer(ctx, true, false);
    if action == "clear" {
        inst.clear_history();
        println!("download history cleared");
        return 0;
    }
    let rows: Vec<Value> = inst.history().into_iter().take(limit).collect();
    let recs = inst.installed();
    if as_json {
        print_json(&json!({"installed": recs, "history": rows}));
        return 0;
    }
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    if !recs.is_empty() {
        let mut list: Vec<&Value> = recs.values().collect();
        list.sort_by_key(|r| s(r, "name").to_lowercase());
        let t: Vec<Vec<String>> = list
            .iter()
            .map(|r| {
                vec![
                    s(r, "id"),
                    take(&s(r, "name"), 40),
                    or_dash(&s(r, "version")),
                    take(&s(r, "installed"), 10),
                    s(r, "license"),
                ]
            })
            .collect();
        println!(
            "{}",
            table(&t, &["installed", "name", "version", "date", "license"])
        );
        println!();
    }
    if rows.is_empty() {
        println!("no downloads yet");
        return 0;
    }
    let t: Vec<Vec<String>> = rows
        .iter()
        .map(|h| {
            vec![
                take(&s(h, "finished"), 16).replace('T', " "),
                s(h, "status"),
                take(&s(h, "name"), 40),
                human_size(h.get("size").and_then(Value::as_u64).unwrap_or(0)),
                take(&s(h, "error"), 60),
            ]
        })
        .collect();
    println!(
        "{}",
        table(&t, &["when", "status", "name", "size", "error"])
    );
    0
}

fn cmd_licenses(as_json: bool) -> i32 {
    let all = licenses::all();
    if as_json {
        print_json(&Value::Array(
            all.iter()
                .map(|x| {
                    json!({"id": x.id, "name": x.name, "url": x.url, "status": x.status,
                           "attribution": x.attribution, "share_alike": x.share_alike,
                           "commercial": x.commercial})
                })
                .collect(),
        ));
        return 0;
    }
    let t: Vec<Vec<String>> = all
        .iter()
        .map(|x| {
            vec![
                x.id.clone(),
                x.status.clone(),
                yn(x.attribution),
                yn(x.share_alike),
                yn(x.commercial),
                x.name.clone(),
            ]
        })
        .collect();
    println!(
        "{}",
        table(
            &t,
            &[
                "id",
                "status",
                "credit",
                "share-alike",
                "commercial",
                "name"
            ]
        )
    );
    println!("\nopen = open licence · free = free to use, terms not a licence · unknown = the source does not say");
    0
}

fn cmd_setup(
    ctx: &mut Ctx,
    action: &str,
    file: &Path,
    parts: &[&str],
    replace: bool,
    as_json: bool,
) -> i32 {
    let path = util::expand_user(&file.to_string_lossy());
    match action {
        "show" => {
            let m = transfer::read_manifest(&path).unwrap_or_else(|e| die(format!("error: {}", e)));
            if as_json {
                print_json(&m);
                return 0;
            }
            let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let mparts: Vec<String> = m["parts"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let dicts = m["dictionaries"].as_array().cloned().unwrap_or_default();
            println!("created: {} by {}", s("created"), s("app"));
            println!("parts:   {}", mparts.join(", "));
            let inside = if mparts.iter().any(|p| p == "files") {
                let n: usize = dicts
                    .iter()
                    .map(|d| d["files"].as_array().map_or(0, Vec::len))
                    .sum();
                format!(" ({} files inside)", n)
            } else {
                String::new()
            };
            println!("dictionaries: {}{}", dicts.len(), inside);
            0
        }
        "export" => {
            let m = transfer::export_setup(&path, &ctx.data_dir, parts)
                .unwrap_or_else(|e| die(format!("error: {}", e)));
            let mparts: Vec<&str> = m["parts"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            println!(
                "exported {} dictionaries ({}) → {}",
                m["dictionaries"].as_array().map_or(0, Vec::len),
                mparts.join(", "),
                path.display()
            );
            0
        }
        _ => {
            let res = transfer::import_setup(&path, &ctx.data_dir, replace, parts)
                .unwrap_or_else(|e| die(format!("error: {}", e)));
            println!(
                "imported: {} dictionaries, {} files, {} sources, {} history, {} favorites{}",
                res.added.len(),
                res.files,
                res.sources,
                res.history,
                res.favorites,
                if res.settings { ", settings" } else { "" }
            );
            for m in &res.missing {
                println!("missing: {}", m);
            }
            if !res.added.is_empty() {
                println!("the new dictionaries are opened at the next start (or: dct reindex)");
            }
            0
        }
    }
}

/// `dct dicts --export FILE [--format csv|json|txt]`.
pub fn export_list(ctx: &Ctx, file: &str, fmt: &str) -> i32 {
    match transfer::export_list(&util::expand_user(file), &ctx.data_dir, fmt) {
        Ok(n) => {
            println!("exported {} dictionaries → {}", n, file);
            0
        }
        Err(e) => die(format!("error: {}", e)),
    }
}
