//! `dct` — short name of `dictz` (the same program, like `vs` for venvstudio).

#![forbid(unsafe_code)]
#![cfg_attr(all(windows, feature = "gui"), windows_subsystem = "windows")]

fn main() {
    dictz::entry::main();
}
