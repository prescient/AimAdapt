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
//! - [`shapes`]: triangles for the map editor's primitives (cylinders, ramps…).

pub mod convert;
pub mod inventory;
pub mod map;
pub mod scenario;
pub mod settings;
pub mod shapes;

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

/// The file of a scenario among these files: the first whose name without `.sce` is the scenario's,
/// ignoring case and spaces at the ends. KovaaK's names its stats after the file, so this is how a
/// name in your history or a workout finds its file.
pub fn file_named(name: &str, files: &[PathBuf]) -> Option<PathBuf> {
    let want = name.trim().to_lowercase();
    files.iter().find(|f| f.file_stem().is_some_and(|s| s.to_string_lossy().trim().to_lowercase() == want)).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scenarios_file_is_found_by_its_name() {
        let files = vec![PathBuf::from("C:/S/VT 1w4ts Novice S5.sce"), PathBuf::from("C:/S/vt ww5t novice s5.sce")];
        assert_eq!(file_named("VT 1w4ts Novice S5", &files), Some(files[0].clone()));
        assert_eq!(file_named(" VT ww5t Novice S5", &files), Some(files[1].clone()));
        assert_eq!(file_named("VT 1w4ts Novice S5 Hard", &files), None);
    }
}
