//! Loading and saving the player's settings profiles (`engine.toml` in the data
//! folder). On first use there's one profile matched to the most recent KovaaK's
//! run, so the trainer feels the same straight away.

use std::path::{Path, PathBuf};

use aimadapt_core::Db;
use aimadapt_core::progress::cm_per_360;
use aimadapt_sim::PlayerSettings;
use aimadapt_sim::profiles::{Profile, Profiles, game_for_kovaaks_scale};

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("engine.toml")
}

pub fn scenarios_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("scenarios")
}

pub fn workouts_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("workouts")
}

/// What's in `engine.toml`.
pub enum SavedProfiles {
    /// No file yet.
    None,
    Saved(Profiles),
    /// A file that can't be read or used, and why.
    Unusable(String),
}

pub fn read_profiles(data_dir: &Path) -> SavedProfiles {
    match std::fs::read_to_string(settings_path(data_dir)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => SavedProfiles::None,
        Err(e) => SavedProfiles::Unusable(e.to_string()),
        Ok(text) => match Profiles::from_toml(&text) {
            Ok(p) => SavedProfiles::Saved(p),
            Err(e) => SavedProfiles::Unusable(e),
        },
    }
}

/// Saved profiles, or a single one matched to KovaaK's if there are none yet or
/// the file can't be used.
pub fn load_profiles(data_dir: &Path, db: Option<&Db>) -> Profiles {
    if let SavedProfiles::Saved(p) = read_profiles(data_dir) {
        return p;
    }
    let profile = db.and_then(kovaaks_profile).unwrap_or_default();
    Profiles { active: profile.name.clone(), profiles: vec![profile] }
}

/// For the app to show when `engine.toml` can't be used: why, and what saving does.
pub fn profiles_note(data_dir: &Path) -> Option<String> {
    let SavedProfiles::Unusable(why) = read_profiles(data_dir) else { return None };
    Some(format!(
        "Your settings file couldn't be used ({why}), so these are the defaults. Saving keeps the old file beside the new one as {}.",
        UNUSABLE_NAME
    ))
}

const UNUSABLE_NAME: &str = "engine (unusable).toml";

/// Saves the profiles. A settings file that can't be used is renamed first (to
/// `engine (unusable).toml`, numbered if that's taken), so the player's own
/// settings are never overwritten with defaults.
pub fn save_profiles(data_dir: &Path, profiles: &Profiles) -> Result<(), String> {
    let problems = profiles.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = settings_path(data_dir);
    if path.exists() && matches!(read_profiles(data_dir), SavedProfiles::Unusable(_)) {
        let aside = (1..)
            .map(|n| match n {
                1 => data_dir.join(UNUSABLE_NAME),
                n => data_dir.join(format!("engine (unusable {n}).toml")),
            })
            .find(|p| !p.exists())
            .expect("a free name");
        std::fs::rename(&path, &aside).map_err(|e| format!("couldn't keep the old settings file: {e}"))?;
    }
    aimadapt_sim::store::write_atomic(&path, &profiles.to_toml()).map_err(|e| e.to_string())
}

/// Settings to play with: the named profile, or the active one.
pub fn load(data_dir: &Path, db: Option<&Db>, profile: Option<&str>) -> Result<PlayerSettings, String> {
    let ps = load_profiles(data_dir, db);
    let mut settings = match profile {
        Some(name) => ps.get(name).map(Profile::resolved).ok_or(format!("no profile called {name}"))?,
        None => ps.active().resolved(),
    };
    // One mix holds for every profile once it is chosen on Settings › Training.
    settings.challenge = aimadapt_sim::prefs::load(data_dir).mix_or(settings.challenge);
    Ok(settings)
}

/// Settings matching the latest KovaaK's run, when its sensitivity scale is known.
pub fn from_kovaaks(db: &Db) -> Option<PlayerSettings> {
    let k = db.latest_kovaaks_settings().ok()??;
    let cm = cm_per_360(&k.sens_scale, k.horiz_sens, k.dpi)?;
    let mut s = PlayerSettings { cm_per_360: cm as f32, dpi: u32::try_from(k.dpi).ok()?, ..Default::default() };
    // KovaaK's FOV is horizontal for the game-style scales we read, which matches ours.
    if let Some(fov) = k.fov.filter(|f| (30.0..=150.0).contains(f)) {
        s.fov = fov as f32;
    }
    s.problems().is_empty().then_some(s)
}

/// A profile matching the latest KovaaK's run, in that game's units when KovaaK's
/// names one we know (e.g. Overwatch 5.3), otherwise in cm/360.
pub fn kovaaks_profile(db: &Db) -> Option<Profile> {
    let settings = from_kovaaks(db)?;
    let k = db.latest_kovaaks_settings().ok()??;
    Some(match game_for_kovaaks_scale(&k.sens_scale) {
        Some(g) => Profile { name: g.name.into(), game: g.id.into(), game_sens: k.horiz_sens as f32, settings },
        None => Profile { name: "From KovaaK's".into(), game: "custom".into(), game_sens: 1.0, settings },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aimadapt_sim::adaptive::Mix;
    use aimadapt_sim::prefs::TrainingPrefs;
    use aimadapt_sim::profiles::Profile;

    #[test]
    fn profiles_save_and_load_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let ow = Profile {
            name: "Overwatch 2".into(),
            game: "overwatch".into(),
            game_sens: 5.3,
            settings: PlayerSettings { dpi: 400, ..Default::default() },
        };
        let val = Profile {
            name: "Valorant".into(),
            game: "valorant".into(),
            game_sens: 0.35,
            settings: PlayerSettings { dpi: 800, ..Default::default() },
        };
        let ps = Profiles { active: "Overwatch 2".into(), profiles: vec![ow, val] };
        save_profiles(dir.path(), &ps).unwrap();
        let active = load(dir.path(), None, None).unwrap();
        assert!((active.cm_per_360 - 65.35).abs() < 0.05, "{}", active.cm_per_360);
        let v = load(dir.path(), None, Some("Valorant")).unwrap();
        assert!((v.cm_per_360 - 46.65).abs() < 0.05, "{}", v.cm_per_360);
        assert!(load(dir.path(), None, Some("Apex")).is_err());

        let bad = Profiles { active: "Nope".into(), ..ps };
        assert!(save_profiles(dir.path(), &bad).is_err(), "invalid profiles aren't written");
    }

    /// Bug: an engine.toml that failed to load gave way to defaults, and the next
    /// save overwrote it, losing every profile in it.
    #[test]
    fn saving_keeps_an_unusable_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let broken = "active = \"Mine\"\n[[profiles]]\nname = \"Mine\"\ncolour = 1\n";
        std::fs::write(settings_path(dir.path()), broken).unwrap();
        assert!(profiles_note(dir.path()).is_some_and(|n| n.contains("couldn't be used")));
        let ps = load_profiles(dir.path(), None);
        save_profiles(dir.path(), &ps).unwrap();
        let kept: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .collect();
        assert!(kept.iter().any(|t| t == broken), "the old file is gone");
        assert!(profiles_note(dir.path()).is_none(), "the new file loads");
    }

    fn two_profiles(dir: &Path) {
        let with = |name: &str, challenge| Profile {
            name: name.into(),
            settings: PlayerSettings { challenge, ..Default::default() },
            ..Default::default()
        };
        let ps = Profiles { active: "A".into(), profiles: vec![with("A", Mix::Steady), with("B", Mix::Balanced)] };
        save_profiles(dir, &ps).unwrap();
    }

    #[test]
    fn a_chosen_mix_is_used_for_every_profile() {
        let dir = tempfile::tempdir().unwrap();
        two_profiles(dir.path());
        let prefs = TrainingPrefs { mix: Some(Mix::Push), ..Default::default() };
        aimadapt_sim::prefs::save(dir.path(), &prefs).unwrap();
        assert_eq!(load(dir.path(), None, Some("A")).unwrap().challenge, Mix::Push);
        assert_eq!(load(dir.path(), None, Some("B")).unwrap().challenge, Mix::Push);
        assert_eq!(load(dir.path(), None, None).unwrap().challenge, Mix::Push);
    }

    #[test]
    fn without_a_chosen_mix_the_profiles_own_is_used() {
        let dir = tempfile::tempdir().unwrap();
        two_profiles(dir.path());
        assert_eq!(load(dir.path(), None, Some("A")).unwrap().challenge, Mix::Steady);
        assert_eq!(load(dir.path(), None, Some("B")).unwrap().challenge, Mix::Balanced);
        // A training file that only sets a goal leaves the profiles' mixes alone.
        std::fs::write(aimadapt_sim::prefs::path(dir.path()), "daily_goal_min = 45.0\n").unwrap();
        assert_eq!(load(dir.path(), None, Some("A")).unwrap().challenge, Mix::Steady);
        assert_eq!(load(dir.path(), None, Some("B")).unwrap().challenge, Mix::Balanced);
    }

    #[test]
    fn missing_file_gives_one_default_profile() {
        let dir = tempfile::tempdir().unwrap();
        let ps = load_profiles(dir.path(), None);
        assert_eq!(ps.profiles.len(), 1);
        assert_eq!(ps.active().name, ps.active);
    }
}
