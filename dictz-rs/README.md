# dictz — Rust edition

[![crates.io](https://img.shields.io/crates/v/dictz.svg)](https://crates.io/crates/dictz)
[![docs.rs](https://img.shields.io/docsrs/dictz)](https://docs.rs/dictz)
[![License: GPL v3+](https://img.shields.io/badge/License-GPLv3+-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

Cross-platform, multi-format **offline dictionary** — a GoldenDict-style desktop app and library written in Rust.

This is the Rust edition of [dictz](https://github.com/bayramkotan/dictz). The Python edition (`pip install dictz`) has the same features and ships from the same tag with the same version number. Both editions read dictionaries the same way: CI checks it against shared golden files. They also share one data folder, so dictionaries, history and favorites added in one edition show up in the other.

## Install the app

```bash
cargo install dictz
dictz                 # open the window
dictz En-Tr.dsl       # add a dictionary and open the window
dictz lookup apple    # command line (see below)
dictz update          # update this copy
```

On Linux, building needs the ALSA headers for in-app sound: `sudo apt install libasound2-dev pkg-config` on Debian/Ubuntu (Arch/CachyOS: `alsa-lib`, usually already installed). To build without sound, run `cargo install dictz --no-default-features --features cli,gui,update`; sounds then open in the system player.

Each [GitHub Release](https://github.com/bayramkotan/dictz/releases/latest) also has ready-made single-file builds: `dictz-rs-linux-x86_64.tar.gz`, `dictz-rs-windows-x86_64.zip` and `dictz-rs-macos-arm64.tar.gz`.

## Features

- **Formats:** StarDict (`.ifo` / `.idx[.gz]` / `.dict[.dz]`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`) and Babylon (`.bgl`). All three are parsed natively, with no Python and no PyGlossary.
- **Multi-dictionary search** in your own order. You can enable, disable and reorder dictionaries in the dictionary manager.
- **Fuzzy search:** "did you mean" suggestions. The scores are identical to RapidFuzz `fuzz.ratio`, which the Python edition uses.
- **DSL pictures and sounds** are read from `X.dsl.files/` or `X.dsl.files.zip`. Pictures show inline; sounds play with one click (WAV, MP3, Ogg, FLAC).
- **SQLite FTS5 index:** instant, diacritic-insensitive prefix suggestions.
- **Interface:**
  - tabs, back/forward, random word, favorites and history (with filters)
  - dictionary bar (one-click on/off, right-click = only this one) and a "found in" panel
  - collapsible articles per dictionary with copy / "only this dictionary" buttons
  - find in page with highlighting, save article as HTML / text
  - background loading with progress; dictionary folders scanned on start
  - light, dark and auto themes, accent color, fonts by name or file, zoom, full screen, always on top
  - drag and drop dictionary files onto the window
  - the same shortcuts as the Python edition (F1 lists them)

## Command line

The same commands, options and output as the Python edition. `dictz help` and `dictz <command> --help` list everything.

```bash
dictz lookup apple            # articles as text (-d DICT, -p prefix, -f fuzzy, --json, --html)
dictz suggest app             # headwords starting with "app" (+ ≈ fuzzy fill)
dictz fuzzy aple              # "did you mean" with scores
dictz random                  # a random article
dictz shell                   # interactive prompt (:fuzzy, :suggest, :dicts, :random, :q)

dictz add ~/sozlukler -r      # add files or whole folders
dictz dicts                   # table: position, on/off, name, format, words, path
dictz disable "Lingvo"        # names can be partial: enable / disable / remove / info
dictz move "Lingvo" top       # up / down / top / bottom / position
dictz info En-Tr.dsl          # metadata of a dictionary or a file
dictz reindex                 # rebuild the search index
dictz scan ~/Downloads --add  # find dictionary files, add the new ones
dictz dump En-Tr.dsl -F txt   # export as tsv / json / txt

dictz history  /  dictz favorites add WORD
dictz config list             # every setting with its value, range and description
dictz config set article.font_size 18
dictz config export my.json   # import / reset / path / get
dictz paths  /  dictz doctor  /  dictz selftest
dictz update --check          # crates.io or GitHub release, depending on how it was installed
eval "$(dictz completion bash)"   # also zsh, fish, powershell
```

Global options: `--data-dir DIR` (or `DICTZ_DATA_DIR`), `--no-color` (or `NO_COLOR`), `-v` / `--version`.

## Settings

54 settings in 7 pages (general, appearance, article, search, dictionaries, sound, history), stored in `settings.json` in the data folder. The Python edition reads and writes the same file. Change them in **Düzen → Tercihler** (search box, ↺ per setting, page reset, shortcuts page, advanced page) or with `dictz config`.

## Use as a library

```toml
[dependencies]
dictz = { version = "0.0.7", default-features = false }   # no GUI / audio deps
```

```rust
use dictz::query::QueryManager;

let mut qm = QueryManager::new(None);
qm.add_dictionary("En-Tr.dsl", false)?;
for e in qm.search("apple", true) {
    println!("[{}] {}: {}", e.dictionary_id, e.headword, e.definition);
}
for (word, score) in qm.fuzzy("aple", 5, 60.0) {
    println!("did you mean {word}? ({score:.0}%)");
}
```

| Module | What it does |
|---|---|
| `stardict`, `dsl`, `babylon` | format parsers (all implement `Dictionary`) |
| `query` | lookups across dictionaries, suggestions, fuzzy matching, ordering |
| `index` / `history` | SQLite FTS5 index; history and favorites (same schema as the Python edition) |
| `media` | finds pictures and sounds of an article |
| `html` | converts article HTML into styled lines (GUI) or text (CLI) |
| `settings` | the shared `settings.json` schema and `Preferences` |
| `fuzzy` | RapidFuzz-compatible `ratio`, `extract` and `rank` |

## Feature flags

| Flag | Default | Pulls in |
|---|---|---|
| `cli` | on | clap (the `dictz` command line; the binary needs it) |
| `gui` | on | eframe/egui, rfd (file dialogs), image |
| `audio` | on | rodio (needs ALSA headers on Linux) |
| `update` | on | ureq, tar (`dictz update`) |

## License

GPL-3.0-or-later © Bayram Kotan
