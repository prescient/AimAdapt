//! Finding scenarios: the built-in set compiled into the program, plus the player's
//! own TOML files in a folder (`%APPDATA%\RustyAim\scenarios` in the app).
//!
//! Ids are `builtin:<name>` or `user:<file stem>`.

use std::path::{Path, PathBuf};

use crate::scenario::Scenario;

const BUILTIN: &[(&str, &str)] = &[
    ("static-grid", include_str!("../scenarios/static-grid.toml")),
    ("wide-flicks", include_str!("../scenarios/wide-flicks.toml")),
    ("micro-flicks", include_str!("../scenarios/micro-flicks.toml")),
    ("shrink-reflex", include_str!("../scenarios/shrink-reflex.toml")),
    ("strafe-clicks", include_str!("../scenarios/strafe-clicks.toml")),
    ("strafe-track", include_str!("../scenarios/strafe-track.toml")),
    ("float-track", include_str!("../scenarios/float-track.toml")),
    ("switch-track", include_str!("../scenarios/switch-track.toml")),
    ("timed-flicks", include_str!("../scenarios/timed-flicks.toml")),
    ("timed-wide-flicks", include_str!("../scenarios/timed-wide-flicks.toml")),
    ("timed-micro-flicks", include_str!("../scenarios/timed-micro-flicks.toml")),
    ("timed-grid", include_str!("../scenarios/timed-grid.toml")),
    ("timed-strafe-clicks", include_str!("../scenarios/timed-strafe-clicks.toml")),
    ("linear-clicks", include_str!("../scenarios/linear-clicks.toml")),
    ("close-strafe-track", include_str!("../scenarios/close-strafe-track.toml")),
    ("precise-track", include_str!("../scenarios/precise-track.toml")),
    ("control-track", include_str!("../scenarios/control-track.toml")),
    ("speed-switch", include_str!("../scenarios/speed-switch.toml")),
    ("stability-switch", include_str!("../scenarios/stability-switch.toml")),
];

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub builtin: bool,
    pub path: Option<PathBuf>,
    /// `Err` holds why the file couldn't be used.
    pub scenario: Result<Scenario, String>,
}

pub fn builtins() -> Vec<Entry> {
    BUILTIN
        .iter()
        .map(|(name, text)| Entry {
            id: format!("builtin:{name}"),
            builtin: true,
            path: None,
            scenario: Scenario::from_toml(text).map_err(|e| e.to_string()),
        })
        .collect()
}

/// Built-in scenarios followed by every `.toml` file in `user_dir`, by file name.
pub fn load_all(user_dir: &Path) -> Vec<Entry> {
    let mut entries = builtins();
    let mut files: Vec<PathBuf> = std::fs::read_dir(user_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("toml")))
        .collect();
    files.sort();
    for path in files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        let scenario = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| Scenario::from_toml(&t).map_err(|e| e.to_string()));
        entries.push(Entry { id: format!("user:{stem}"), builtin: false, path: Some(path.clone()), scenario });
    }
    entries
}

/// Loads a scenario by id, or by path to a `.toml` file.
pub fn resolve(id: &str, user_dir: &Path) -> Result<Scenario, String> {
    if let Some(name) = id.strip_prefix("builtin:") {
        let (_, text) = BUILTIN.iter().find(|(n, _)| *n == name).ok_or(format!("no built-in scenario {name}"))?;
        return Scenario::from_toml(text).map_err(|e| e.to_string());
    }
    let path = match id.strip_prefix("user:") {
        Some(stem) => user_path(user_dir, stem)?,
        None => PathBuf::from(id),
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Scenario::from_toml(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn user_path(dir: &Path, stem: &str) -> Result<PathBuf, String> {
    if stem.is_empty() || stem.contains(['/', '\\', ':']) || stem.starts_with('.') {
        return Err(format!("bad scenario id {stem}"));
    }
    Ok(dir.join(format!("{stem}.toml")))
}

/// Writes a scenario to the user folder and returns its id. `replace` is the id of
/// the user scenario being edited; otherwise a new file named after the scenario is
/// created (with a number added if the name is taken).
pub fn save_user(dir: &Path, scenario: &Scenario, replace: Option<&str>) -> Result<String, String> {
    let problems = scenario.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let stem = match replace.and_then(|id| id.strip_prefix("user:")) {
        Some(stem) => stem.to_string(),
        None => {
            let base = slug(&scenario.name);
            let mut stem = base.clone();
            let mut n = 2;
            while dir.join(format!("{stem}.toml")).exists() {
                stem = format!("{base}-{n}");
                n += 1;
            }
            stem
        }
    };
    let path = user_path(dir, &stem)?;
    std::fs::write(&path, scenario.to_toml()).map_err(|e| e.to_string())?;
    Ok(format!("user:{stem}"))
}

pub fn delete_user(dir: &Path, id: &str) -> Result<(), String> {
    let stem = id.strip_prefix("user:").ok_or("only your own scenarios can be deleted")?;
    std::fs::remove_file(user_path(dir, stem)?).map_err(|e| e.to_string())
}

/// Lowercase words joined by dashes, safe as a file name.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "scenario".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_valid() {
        for e in builtins() {
            assert!(e.scenario.is_ok(), "{}: {:?}", e.id, e.scenario.err());
        }
    }

    #[test]
    fn save_load_and_delete_user_scenarios() {
        let dir = tempfile::tempdir().unwrap();
        let s = Scenario { name: "My Flicks!".into(), ..Default::default() };
        let id = save_user(dir.path(), &s, None).unwrap();
        assert_eq!(id, "user:my-flicks");
        assert_eq!(save_user(dir.path(), &s, None).unwrap(), "user:my-flicks-2");
        assert_eq!(resolve(&id, dir.path()).unwrap(), s);

        let mut edited = s.clone();
        edited.targets.radius = 0.2;
        assert_eq!(save_user(dir.path(), &edited, Some(&id)).unwrap(), id);
        assert_eq!(resolve(&id, dir.path()).unwrap().targets.radius, 0.2);

        std::fs::write(dir.path().join("broken.toml"), "nope = ").unwrap();
        let all = load_all(dir.path());
        assert_eq!(all.iter().filter(|e| !e.builtin).count(), 3);
        assert!(all.iter().find(|e| e.id == "user:broken").unwrap().scenario.is_err());

        delete_user(dir.path(), &id).unwrap();
        assert!(resolve(&id, dir.path()).is_err());
        assert!(delete_user(dir.path(), "builtin:static-grid").is_err());
        assert!(resolve("user:../x", dir.path()).is_err());
    }
}
