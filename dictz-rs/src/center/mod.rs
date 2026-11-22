//! The Dictionary Center: sources (servers), catalogs, downloads, installs,
//! import / export. Same files and rules as the Python edition
//! (`dictz/center/`):
//!
//! * `DATA/center/sources.json`      the servers / sources (built-in + the user's)
//! * `DATA/center/credentials.json`  user names, passwords, tokens (never exported)
//! * `DATA/center/cache/<id>.json`   each source's last catalog
//! * `DATA/center/installed.json`    dictionaries installed from a catalog
//! * `DATA/center/downloads.json`    download history
//! * `DATA/dictionaries/<source>/<entry>/`  where downloads are installed
//!
//! The catalog format (`dictz-catalog.json`) is described in `docs/CATALOG.md`.

pub mod adapters;
pub mod catalog;
pub mod install;
pub mod langs;
pub mod licenses;
pub mod model;
pub mod net;
pub mod sources;
pub mod store;
pub mod transfer;
pub mod util;

/// Folder of the center's files inside the data folder.
pub const CENTER_DIR: &str = "center";
/// The catalog schema this dictz reads.
pub const CATALOG_SCHEMA: i64 = 1;
