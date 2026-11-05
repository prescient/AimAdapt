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

/// Saved profiles, or a single one matched to KovaaK's if there are none yet.
pub fn load_profiles(data_dir: &Path, db: Option<&Db>) -> Profiles {
    if let Ok(text) = std::fs::read_to_string(settings_path(data_dir))
        && let Ok(p) = Profiles::from_toml(&text)
    {
        return p;
    }
    let profile = db.and_then(kovaaks_profile).unwrap_or_default();
    Profiles { active: profile.name.clone(), profiles: vec![profile] }
}

pub fn save_profiles(data_dir: &Path, profiles: &Profiles) -> Result<(), String> {
    let problems = profiles.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(data_dir), profiles.to_toml()).map_err(|e| e.to_string())
}

/// Settings to play with: the named profile, or the active one.
pub fn load(data_dir: &Path, db: Option<&Db>, profile: Option<&str>) -> Result<PlayerSettings, String> {
    let ps = load_profiles(data_dir, db);
    match profile {
        Some(name) => ps.get(name).map(Profile::resolved).ok_or(format!("no profile called {name}")),
        None => Ok(ps.active().resolved()),
    }
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

    #[test]
    fn missing_file_gives_one_default_profile() {
        let dir = tempfile::tempdir().unwrap();
        let ps = load_profiles(dir.path(), None);
        assert_eq!(ps.profiles.len(), 1);
        assert_eq!(ps.active().name, ps.active);
    }
}
