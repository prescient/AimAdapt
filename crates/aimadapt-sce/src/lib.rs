//! Reading KovaaK's scenario files (`.sce`): their settings, the profiles those
//! settings point at, and the map embedded at the end of each file. This crate only
//! reads; it never writes or changes a KovaaK's file. The game rules are in
//! `aimadapt-sim`; [`convert`] turns a file into one of its scenarios.
//!
//! - [`settings`]: the `key=value` sections.
//! - [`map`]: the `[Map Data]` part, in Reflex or JSON format.
//! - [`scenario`]: a whole file, with its player, weapon and bots resolved.
//! - [`inventory`]: which features a scenario uses that the trainer doesn't model yet.
//! - [`convert`]: a file as a trainer scenario, for the scenarios it can play.

pub mod convert;
pub mod inventory;
pub mod map;
pub mod scenario;
pub mod settings;

pub use convert::{Imported, import};
pub use inventory::{Gap, gaps};
pub use scenario::Sce;

use std::path::{Path, PathBuf};

/// Every `.sce` file under these folders (searched recursively), sorted by path.
pub fn find_files(dirs: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("sce")) {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    for d in dirs {
        walk(d, &mut out);
    }
    out.sort();
    out.dedup();
    out
}
