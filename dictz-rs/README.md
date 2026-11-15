# dictz (Rust)

[![Crates.io](https://img.shields.io/crates/v/dictz.svg)](https://crates.io/crates/dictz)
[![docs.rs](https://docs.rs/dictz/badge.svg)](https://docs.rs/dictz)
[![License: GPL v3+](https://img.shields.io/badge/License-GPLv3+-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

Rust port of **[bayramkotan/dictz](https://github.com/bayramkotan/dictz)** — a cross-platform multi-format offline dictionary.

> The shipping, feature-complete edition is Python + PySide6 (`pip install dictz`).
> This crate is the Rust implementation, currently at scaffold stage, growing alongside the Python edition.

## Status

| v0.0.5 | Scaffold: `Dictionary` trait, `DictionaryEntry`, `DictionaryMetadata`, `DictzError` |
| v0.1.x | StarDict parser (native, zero-alloc hot path) |
| v0.2.x | DSL + Babylon parsers |
| v0.3.x | SQLite FTS5 index + fuzzy matching |
| v1.0   | Feature parity with the Python edition |

## Install

```toml
[dependencies]
dictz = "0.0.5"
```

## Why a Rust port?

- **Single-static-binary** distribution — no Python runtime required
- **Memory-safe** (`#![forbid(unsafe_code)]`) end-to-end parsing
- **Benchmark target** for the Python edition
- **Learning** — concurrent parsers, `mmap`-backed index access

## License

GPL-3.0 or later — same as the Python edition.
