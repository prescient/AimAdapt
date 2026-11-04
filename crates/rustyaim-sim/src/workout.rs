//! Workouts: a list of scenarios played back to back in one trainer session, each
//! for a number of runs and in standard or adaptive mode. Stored as TOML like
//! scenarios: built-ins compiled in, the player's own in a `workouts` folder.
//!
//! ```toml
//! name = "Daily Fundamentals"
//! [[items]]
//! scenario = "builtin:timed-flicks"
//! runs = 2
//! mode = "adaptive"
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::library::slug;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Workout {
    pub name: String,
    pub description: String,
    pub author: String,
    pub items: Vec<Item>,
}

impl Default for Workout {
    fn default() -> Self {
        Workout {
            name: "Untitled workout".into(),
            description: String::new(),
            author: String::new(),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// Scenario id (`builtin:…` or `user:…`).
    pub scenario: String,
    #[serde(default = "one")]
    pub runs: u32,
    #[serde(default)]
    pub mode: Mode,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Standard,
    #[default]
    Adaptive,
}

/// One run of a workout, in play order.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub scenario: String,
    pub mode: Mode,
    /// 1-based run number within its item, and that item's run count.
    pub run: u32,
    pub of: u32,
}

impl Workout {
    pub fn from_toml(text: &str) -> Result<Workout, String> {
        let w: Workout = toml::from_str(text).map_err(|e| e.to_string())?;
        let p = w.problems();
        if p.is_empty() { Ok(w) } else { Err(p.join("; ")) }
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("workout always serializes")
    }

    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.name.trim().is_empty() {
            p.push("name is empty".into());
        }
        if self.items.is_empty() || self.items.len() > 50 {
            p.push("a workout needs 1–50 scenarios".into());
        }
        for (i, item) in self.items.iter().enumerate() {
            if item.scenario.trim().is_empty() {
                p.push(format!("item {} has no scenario", i + 1));
            }
            if !(1..=20).contains(&item.runs) {
                p.push(format!("item {}: runs must be 1–20", i + 1));
            }
        }
        p
    }

    /// Every run in order.
    pub fn steps(&self) -> Vec<Step> {
        self.items
            .iter()
            .flat_map(|it| {
                (1..=it.runs).map(move |run| Step { scenario: it.scenario.clone(), mode: it.mode, run, of: it.runs })
            })
            .collect()
    }

    pub fn total_runs(&self) -> u32 {
        self.items.iter().map(|i| i.runs).sum()
    }
}

const BUILTIN: &[(&str, &str)] = &[
    ("daily-fundamentals", include_str!("../workouts/daily-fundamentals.toml")),
    ("quick-warmup", include_str!("../workouts/quick-warmup.toml")),
    ("clicking-focus", include_str!("../workouts/clicking-focus.toml")),
    ("tracking-focus", include_str!("../workouts/tracking-focus.toml")),
];

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub builtin: bool,
    pub workout: Result<Workout, String>,
}

pub fn builtins() -> Vec<Entry> {
    BUILTIN
        .iter()
        .map(|(n, t)| Entry { id: format!("builtin:{n}"), builtin: true, workout: Workout::from_toml(t) })
        .collect()
}

pub fn load_all(dir: &Path) -> Vec<Entry> {
    let mut out = builtins();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("toml")))
        .collect();
    files.sort();
    for path in files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        let workout = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Workout::from_toml(&t));
        out.push(Entry { id: format!("user:{stem}"), builtin: false, workout });
    }
    out
}

fn user_path(dir: &Path, stem: &str) -> Result<PathBuf, String> {
    if stem.is_empty() || stem.contains(['/', '\\', ':']) || stem.starts_with('.') {
        return Err(format!("bad workout id {stem}"));
    }
    Ok(dir.join(format!("{stem}.toml")))
}

pub fn resolve(id: &str, dir: &Path) -> Result<Workout, String> {
    if let Some(name) = id.strip_prefix("builtin:") {
        let (_, text) = BUILTIN.iter().find(|(n, _)| *n == name).ok_or(format!("no built-in workout {name}"))?;
        return Workout::from_toml(text);
    }
    let path = match id.strip_prefix("user:") {
        Some(stem) => user_path(dir, stem)?,
        None => PathBuf::from(id),
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Workout::from_toml(&text)
}

pub fn save_user(dir: &Path, w: &Workout, replace: Option<&str>) -> Result<String, String> {
    let p = w.problems();
    if !p.is_empty() {
        return Err(p.join("; "));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let stem = match replace.and_then(|id| id.strip_prefix("user:")) {
        Some(s) => s.to_string(),
        None => {
            let base = slug(&w.name);
            let mut stem = base.clone();
            let mut n = 2;
            while dir.join(format!("{stem}.toml")).exists() {
                stem = format!("{base}-{n}");
                n += 1;
            }
            stem
        }
    };
    std::fs::write(user_path(dir, &stem)?, w.to_toml()).map_err(|e| e.to_string())?;
    Ok(format!("user:{stem}"))
}

pub fn delete_user(dir: &Path, id: &str) -> Result<(), String> {
    let stem = id.strip_prefix("user:").ok_or("only your own workouts can be deleted")?;
    std::fs::remove_file(user_path(dir, stem)?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_workouts_are_valid_and_use_real_scenarios() {
        let scenarios: Vec<String> = crate::library::builtins().into_iter().map(|e| e.id).collect();
        for e in builtins() {
            let w = e.workout.unwrap_or_else(|err| panic!("{}: {err}", e.id));
            for item in &w.items {
                assert!(scenarios.contains(&item.scenario), "{} uses unknown {}", e.id, item.scenario);
            }
        }
    }

    #[test]
    fn steps_expand_runs_in_order() {
        let w = Workout::from_toml(
            "name = \"x\"\n[[items]]\nscenario = \"builtin:a\"\nruns = 2\n[[items]]\nscenario = \"builtin:b\"\nmode = \"standard\"",
        )
        .unwrap();
        let steps = w.steps();
        assert_eq!(steps.len(), 3);
        assert_eq!((steps[1].scenario.as_str(), steps[1].run, steps[1].of), ("builtin:a", 2, 2));
        assert_eq!(steps[2].mode, Mode::Standard);
        assert_eq!(steps[0].mode, Mode::Adaptive, "adaptive is the default");
    }

    #[test]
    fn save_and_delete_user_workouts() {
        let dir = tempfile::tempdir().unwrap();
        let w = Workout {
            name: "Morning".into(),
            items: vec![Item { scenario: "builtin:timed-flicks".into(), runs: 3, mode: Mode::Adaptive }],
            ..Default::default()
        };
        let id = save_user(dir.path(), &w, None).unwrap();
        assert_eq!(resolve(&id, dir.path()).unwrap(), w);
        assert_eq!(load_all(dir.path()).iter().filter(|e| !e.builtin).count(), 1);
        delete_user(dir.path(), &id).unwrap();
        assert!(save_user(dir.path(), &Workout::default(), None).is_err(), "empty workouts are rejected");
    }
}
