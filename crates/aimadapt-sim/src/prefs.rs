//! Training settings that hold for every profile: the adaptive mix and the daily goal.
//! They live in `training.toml` in the data folder, beside `engine.toml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::adaptive::Mix;

/// The daily goal's allowed range, in minutes of active time.
pub const GOAL_RANGE: std::ops::RangeInclusive<f64> = 1.0..=600.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrainingPrefs {
    /// The adaptive mix for every profile. Unset until Settings › Training is first
    /// saved, which writes the mix it shows. Until then the profile's own mix (the
    /// older setting) is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix: Option<Mix>,
    /// Minutes of active time a day.
    pub daily_goal_min: f64,
}

impl Default for TrainingPrefs {
    /// The goal is written here a second time because this crate can't see
    /// `aimadapt-core`; a test in the app checks that the two agree.
    fn default() -> Self {
        TrainingPrefs { mix: None, daily_goal_min: 30.0 }
    }
}

impl TrainingPrefs {
    /// What is wrong with these settings, if anything.
    pub fn problems(&self) -> Vec<String> {
        if self.daily_goal_min.is_finite() && GOAL_RANGE.contains(&self.daily_goal_min) {
            Vec::new()
        } else {
            vec!["The daily goal must be between 1 and 600 minutes.".to_string()]
        }
    }

    /// The chosen mix, or the profile's own when none is chosen.
    pub fn mix_or(&self, profile: Mix) -> Mix {
        self.mix.unwrap_or(profile)
    }
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("training.toml")
}

/// The saved settings. A missing or damaged file gives the defaults. A goal outside
/// the allowed range is replaced by the default and the mix is kept.
pub fn load(data_dir: &Path) -> TrainingPrefs {
    let Ok(text) = std::fs::read_to_string(path(data_dir)) else { return TrainingPrefs::default() };
    let Ok(mut prefs) = toml::from_str::<TrainingPrefs>(&text) else { return TrainingPrefs::default() };
    if !prefs.problems().is_empty() {
        prefs.daily_goal_min = TrainingPrefs::default().daily_goal_min;
    }
    prefs
}

/// Saves the settings. Refuses, and writes nothing, when they have problems.
pub fn save(data_dir: &Path, prefs: &TrainingPrefs) -> Result<(), String> {
    let problems = prefs.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let text = toml::to_string_pretty(prefs).map_err(|e| e.to_string())?;
    crate::store::write_atomic(&path(data_dir), &text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_goal_and_mix_load_back() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = TrainingPrefs { mix: Some(Mix::Push), daily_goal_min: 45.0 };
        save(dir.path(), &prefs).unwrap();
        assert_eq!(load(dir.path()), prefs);
    }

    #[test]
    fn no_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = load(dir.path());
        assert_eq!(prefs.daily_goal_min, 30.0);
        assert_eq!(prefs.mix, None);
    }

    #[test]
    fn a_damaged_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(path(dir.path()), "mix = [[[ not toml").unwrap();
        assert_eq!(load(dir.path()), TrainingPrefs::default());
    }

    #[test]
    fn a_file_with_only_a_goal_has_no_mix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(path(dir.path()), "daily_goal_min = 45.0\n").unwrap();
        let prefs = load(dir.path());
        assert_eq!(prefs.mix, None);
        assert_eq!(prefs.daily_goal_min, 45.0);
        assert_eq!(prefs.mix_or(Mix::Push), Mix::Push);
    }

    #[test]
    fn a_goal_outside_the_range_is_refused_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        for goal in [0.0, -5.0, 601.0, f64::NAN, f64::INFINITY] {
            let prefs = TrainingPrefs { mix: Some(Mix::Steady), daily_goal_min: goal };
            let err = save(dir.path(), &prefs).unwrap_err();
            assert!(err.contains("between 1 and 600"), "{err}");
            assert!(!path(dir.path()).exists(), "a file was written for {goal}");
        }
    }

    #[test]
    fn a_file_with_a_bad_goal_keeps_its_mix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(path(dir.path()), "mix = \"steady\"\ndaily_goal_min = 0.0\n").unwrap();
        let prefs = load(dir.path());
        assert_eq!(prefs.mix, Some(Mix::Steady));
        assert_eq!(prefs.daily_goal_min, 30.0);
    }
}
