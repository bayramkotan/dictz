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
dictz --selftest      # headless check, exit code 0 = OK
dictz --version
dictz En-Tr.dsl       # add a dictionary and open the window
```

On Linux, building needs the ALSA headers for in-app sound: `sudo apt install libasound2-dev pkg-config` on Debian/Ubuntu (Arch/CachyOS: `alsa-lib`, usually already installed). To build without sound, run `cargo install dictz --no-default-features --features gui`; sounds then open in the system player.

Each [GitHub Release](https://github.com/bayramkotan/dictz/releases/latest) also has ready-made single-file builds: `dictz-rs-linux-x86_64.tar.gz`, `dictz-rs-windows-x86_64.zip` and `dictz-rs-macos-arm64.tar.gz`.

## Features

- **Formats:** StarDict (`.ifo` / `.idx[.gz]` / `.dict[.dz]`), ABBYY Lingvo DSL (`.dsl`, `.dsl.dz`) and Babylon (`.bgl`). All three are parsed natively, with no Python and no PyGlossary.
- **Multi-dictionary search** in your own order. You can enable, disable and reorder dictionaries in the dictionary manager.
- **Fuzzy search:** "did you mean" suggestions. The scores are identical to RapidFuzz `fuzz.ratio`, which the Python edition uses.
- **DSL pictures and sounds** are read from `X.dsl.files/` or `X.dsl.files.zip`. Pictures show inline; sounds play with one click (WAV, MP3, Ogg, FLAC).
- **SQLite FTS5 index:** instant, diacritic-insensitive prefix suggestions.
- **Interface:**
  - tabs, back/forward, favorites and history
  - light, dark and auto themes, zoom, and your own font file
  - drag and drop dictionary files onto the window
  - the same shortcuts as the Python edition (Ctrl+F, Ctrl+T, Ctrl+W, Ctrl+D, Ctrl+B, Ctrl+±, Alt+←/→, Ctrl+O, Ctrl+M)

## Use as a library

```toml
[dependencies]
dictz = { version = "0.0.6", default-features = false }   # no GUI / audio deps
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
| `html` | converts article HTML into styled lines (used by the GUI) |
| `fuzzy` | RapidFuzz-compatible `ratio`, `extract` and `rank` |

## Feature flags

| Flag | Default | Pulls in |
|---|---|---|
| `gui` | on | eframe/egui, rfd (file dialogs), image |
| `audio` | on | rodio (needs ALSA headers on Linux) |

## License

GPL-3.0-or-later © Bayram Kotan
