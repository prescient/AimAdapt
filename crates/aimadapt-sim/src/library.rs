//! Finding scenarios: the built-in set compiled into the program, plus the player's
//! own TOML files in a folder (`%APPDATA%\AimAdapt\scenarios` in the app).
//!
//! Ids are `builtin:<name>` or `user:<file stem>`.

use std::path::{Path, PathBuf};

use crate::scenario::Scenario;
use crate::store;

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
    ("hop-clicks", include_str!("../scenarios/hop-clicks.toml")),
    ("angled-glides", include_str!("../scenarios/angled-glides.toml")),
    ("depth-drift", include_str!("../scenarios/depth-drift.toml")),
    ("hop-strafe-track", include_str!("../scenarios/hop-strafe-track.toml")),
    ("unbroken-strafes", include_str!("../scenarios/unbroken-strafes.toml")),
    ("arc-track", include_str!("../scenarios/arc-track.toml")),
    ("unbroken-float", include_str!("../scenarios/unbroken-float.toml")),
    ("wide-speed-switch", include_str!("../scenarios/wide-speed-switch.toml")),
    ("hop-switch", include_str!("../scenarios/hop-switch.toml")),
    ("depth-stability-switch", include_str!("../scenarios/depth-stability-switch.toml")),
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
    for (stem, path) in store::user_files(user_dir) {
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
    let (path, text) = store::read_user(user_dir, id, "scenario")?;
    Scenario::from_toml(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes a scenario to the user folder and returns its id. `replace` is the id of
/// the user scenario being edited; otherwise a new file named after the scenario is
/// created (with a number added if the name is taken).
pub fn save_user(dir: &Path, scenario: &Scenario, replace: Option<&str>) -> Result<String, String> {
    let problems = scenario.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    store::save_user(dir, &scenario.name, &scenario.to_toml(), replace, "scenario")
}

pub fn delete_user(dir: &Path, id: &str) -> Result<(), String> {
    store::delete_user(dir, id, "scenario")
}

pub use crate::store::slug;

#[cfg(test)]
mod tests {
    use super::*;

    /// Built-ins are listed by hand, so a file left out of the table, or listed
    /// under another file's id, would go unnoticed. Names must differ too: history
    /// is kept by name.
    #[test]
    fn every_built_in_scenario_file_is_listed_once() {
        crate::store::check_builtins("scenarios", BUILTIN);
        let mut names: Vec<String> = builtins().into_iter().map(|e| e.scenario.unwrap().name).collect();
        names.sort();
        let n = names.len();
        names.dedup();
        assert_eq!(names.len(), n, "two built-in scenarios share a name");
    }

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
