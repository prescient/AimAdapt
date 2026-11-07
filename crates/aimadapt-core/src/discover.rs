//! Finding the KovaaK's stats folder, including Steam libraries on other drives.

use std::path::{Path, PathBuf};

const STATS_SUBDIR: &str = r"steamapps\common\FPSAimTrainer\FPSAimTrainer\stats";

/// Default Steam install locations to look in when nothing else is configured.
fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(dir) = std::env::var_os(var) {
            roots.push(PathBuf::from(dir).join("Steam"));
        }
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
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
    let mut libs = Vec::new();
    for root in steam_roots() {
        let vdf = root.join(r"steamapps\libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            libs.extend(library_paths_from_vdf(&text));
        }
        libs.push(root);
    }
    let mut seen = std::collections::HashSet::new();
    libs.retain(|p| seen.insert(p.to_string_lossy().to_lowercase()));
    libs
}

pub fn stats_dir_in(library: &Path) -> PathBuf {
    library.join(STATS_SUBDIR)
}

/// The first KovaaK's stats folder found in any Steam library.
pub fn find_stats_dir() -> Option<PathBuf> {
    steam_libraries().into_iter().map(|lib| stats_dir_in(&lib)).find(|p| p.is_dir())
}

/// Where a library keeps KovaaK's scenario files: your own (and downloaded) ones,
/// and Steam Workshop subscriptions.
pub fn scenario_dirs_in(library: &Path) -> [PathBuf; 2] {
    [library.join(SCENARIOS_SUBDIR), library.join(WORKSHOP_SUBDIR)]
}

const SCENARIOS_SUBDIR: &str = r"steamapps\common\FPSAimTrainer\FPSAimTrainer\Saved\SaveGames\Scenarios";
const WORKSHOP_SUBDIR: &str = r"steamapps\workshop\content\824270";

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
}
