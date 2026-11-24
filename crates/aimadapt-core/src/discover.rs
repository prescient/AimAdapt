//! Finding the KovaaK's stats folder, including Steam libraries on other drives.
//!
//! KovaaK's is a Windows game, but on Linux Steam runs it under Proton and keeps
//! its files in the same layout under Steam's own folder, so both are looked for.

use std::path::{Path, PathBuf};

/// Folders inside a Steam library, as separate parts so they join on any system
/// (a Windows-style `steamapps\common` is one odd file name on Linux).
const STATS_SUBDIR: &[&str] = &["steamapps", "common", "FPSAimTrainer", "FPSAimTrainer", "stats"];
const SCENARIOS_SUBDIR: &[&str] =
    &["steamapps", "common", "FPSAimTrainer", "FPSAimTrainer", "Saved", "SaveGames", "Scenarios"];
const WORKSHOP_SUBDIR: &[&str] = &["steamapps", "workshop", "content", "824270"];
const VDF: &[&str] = &["steamapps", "libraryfolders.vdf"];

fn under(root: &Path, parts: &[&str]) -> PathBuf {
    parts.iter().fold(root.to_path_buf(), |path, part| path.join(part))
}

/// Default Steam install locations to look in when nothing else is configured.
fn steam_roots() -> Vec<PathBuf> {
    default_steam_roots(cfg!(windows), |name| std::env::var_os(name).map(PathBuf::from))
}

/// Where Steam installs itself: Program Files on Windows. On Linux it's
/// `$XDG_DATA_HOME/Steam`, normally `~/.local/share/Steam`, which the older
/// `~/.steam/steam` and `~/.steam/root` links point at, or the Flatpak's own folder.
fn default_steam_roots(windows: bool, var: impl Fn(&str) -> Option<PathBuf>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if windows {
        for name in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(dir) = var(name) {
                roots.push(dir.join("Steam"));
            }
        }
        roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    } else {
        let absolute = |p: &PathBuf| p.is_absolute();
        if let Some(xdg) = var("XDG_DATA_HOME").filter(absolute) {
            roots.push(xdg.join("Steam"));
        }
        if let Some(home) = var("HOME").filter(absolute) {
            roots.push(under(&home, &[".local", "share", "Steam"]));
            roots.push(under(&home, &[".steam", "steam"]));
            roots.push(under(&home, &[".steam", "root"]));
            roots.push(under(&home, &[".var", "app", "com.valvesoftware.Steam", ".local", "share", "Steam"]));
        }
    }
    roots.dedup();
    roots
}

/// Library paths listed in Steam's `libraryfolders.vdf`, e.g. `"path"  "D:\\SteamLibrary"`.
pub fn library_paths_from_vdf(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|line| {
            let mut quoted = line.split('"').skip(1).step_by(2);
            match (quoted.next(), quoted.next()) {
                (Some("path"), Some(value)) => Some(PathBuf::from(value.replace(r"\\", r"\"))),
                _ => None,
            }
        })
        .collect()
}

/// Every Steam library on this machine: the install roots plus any extra libraries.
pub fn steam_libraries() -> Vec<PathBuf> {
    libraries_from(steam_roots())
}

/// The libraries `roots` and their `libraryfolders.vdf` files list, each once.
/// Paths that lead to the same folder (Linux's `~/.steam/steam` link to
/// `~/.local/share/Steam`, or a different case on Windows) count once, or every
/// KovaaK's file would be listed twice.
fn libraries_from(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut libs = Vec::new();
    for root in roots {
        if let Ok(text) = std::fs::read_to_string(under(&root, VDF)) {
            libs.extend(library_paths_from_vdf(&text));
        }
        libs.push(root);
    }
    let mut seen = std::collections::HashSet::new();
    libs.retain(|p| {
        let same = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
        let key = same.to_string_lossy().into_owned();
        seen.insert(if cfg!(windows) { key.to_lowercase() } else { key })
    });
    libs
}

pub fn stats_dir_in(library: &Path) -> PathBuf {
    under(library, STATS_SUBDIR)
}

/// The first KovaaK's stats folder found in any Steam library.
pub fn find_stats_dir() -> Option<PathBuf> {
    steam_libraries().into_iter().map(|lib| stats_dir_in(&lib)).find(|p| p.is_dir())
}

/// Where a library keeps KovaaK's scenario files: your own (and downloaded) ones,
/// and Steam Workshop subscriptions.
pub fn scenario_dirs_in(library: &Path) -> [PathBuf; 2] {
    [under(library, SCENARIOS_SUBDIR), under(library, WORKSHOP_SUBDIR)]
}

/// Every KovaaK's scenario folder that exists, across all Steam libraries.
pub fn find_scenario_dirs() -> Vec<PathBuf> {
    steam_libraries().iter().flat_map(|lib| scenario_dirs_in(lib)).filter(|p| p.is_dir()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_library_paths() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
        assert_eq!(
            library_paths_from_vdf(vdf),
            vec![PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"D:\SteamLibrary")]
        );
    }

    /// A Steam library laid out as KovaaK's leaves it, in a temporary folder.
    fn library_with_kovaaks() -> tempfile::TempDir {
        let lib = tempfile::tempdir().unwrap();
        for parts in [STATS_SUBDIR, SCENARIOS_SUBDIR, WORKSHOP_SUBDIR] {
            let dir = parts.iter().fold(lib.path().to_path_buf(), |path, part| path.join(part));
            std::fs::create_dir_all(dir).unwrap();
        }
        lib
    }

    // Bug (Nov 24, 2026): the folders inside a library were joined as Windows paths
    // (`steamapps\common\...`), which on Linux is one file name with backslashes in
    // it, so KovaaK's stats and scenarios were never found there.
    #[test]
    fn kovaaks_folders_inside_a_library_are_found_on_any_system() {
        let lib = library_with_kovaaks();
        assert!(stats_dir_in(lib.path()).is_dir(), "{}", stats_dir_in(lib.path()).display());
        for dir in scenario_dirs_in(lib.path()) {
            assert!(dir.is_dir(), "{}", dir.display());
        }
        assert!(stats_dir_in(lib.path()).ends_with(Path::new("FPSAimTrainer").join("stats")));
    }

    // Same bug: Steam was only looked for in Program Files.
    #[test]
    fn steam_is_looked_for_where_linux_installs_it() {
        // Absolute on whichever system runs the test (`/home` has no drive on Windows).
        let base = std::env::temp_dir();
        let (home, data) = (base.join("home"), base.join("data"));
        let var = |name: &str| match name {
            "HOME" => Some(home.clone()),
            "XDG_DATA_HOME" => Some(data.clone()),
            _ => None,
        };
        let roots = default_steam_roots(false, var);
        for root in [
            under(&data, &["Steam"]),
            under(&home, &[".local", "share", "Steam"]),
            under(&home, &[".steam", "steam"]),
            under(&home, &[".var", "app", "com.valvesoftware.Steam", ".local", "share", "Steam"]),
        ] {
            assert!(roots.contains(&root), "{} missing from {roots:?}", root.display());
        }
        assert!(!roots.iter().any(|r| r.to_string_lossy().contains("Program Files")), "{roots:?}");

        let relative = |name: &str| (name == "XDG_DATA_HOME").then(|| PathBuf::from("data"));
        assert!(default_steam_roots(false, relative).is_empty(), "a relative XDG_DATA_HOME is ignored");

        let windows =
            default_steam_roots(true, |name| (name == "ProgramFiles(x86)").then(|| PathBuf::from(r"D:\Apps")));
        assert_eq!(
            windows,
            vec![PathBuf::from(r"D:\Apps").join("Steam"), PathBuf::from(r"C:\Program Files (x86)\Steam")]
        );
    }

    // On Linux `~/.steam/steam` is a link to `~/.local/share/Steam`; read through both,
    // every KovaaK's scenario would be listed twice.
    #[cfg(unix)]
    #[test]
    fn a_library_reached_through_a_link_counts_once() {
        let lib = library_with_kovaaks();
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("steam");
        std::os::unix::fs::symlink(lib.path(), &link).unwrap();
        let libs = libraries_from(vec![lib.path().to_path_buf(), link, links.path().join("missing")]);
        assert_eq!(libs, vec![lib.path().to_path_buf(), links.path().join("missing")]);
    }

    #[test]
    fn libraries_listed_in_the_vdf_are_read() {
        let root = library_with_kovaaks();
        let other = library_with_kovaaks();
        let vdf = format!(
            "\"libraryfolders\"\n{{\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n",
            other.path().display()
        );
        std::fs::write(root.path().join("steamapps").join("libraryfolders.vdf"), vdf.replace('\\', r"\\")).unwrap();
        let libs = libraries_from(vec![root.path().to_path_buf()]);
        assert_eq!(libs, vec![other.path().to_path_buf(), root.path().to_path_buf()]);
    }
}
