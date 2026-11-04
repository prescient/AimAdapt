//! Loading and saving the player's engine settings (`engine.toml` in the data
//! folder). On first use they start from the sensitivity and FOV of the most recent
//! KovaaK's run, so the engine feels the same straight away.

use std::path::{Path, PathBuf};

use rustyaim_core::Db;
use rustyaim_core::progress::cm_per_360;
use rustyaim_sim::PlayerSettings;

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("engine.toml")
}

pub fn scenarios_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("scenarios")
}

pub fn workouts_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("workouts")
}

/// Saved settings, or ones matched to KovaaK's if there are none yet.
pub fn load(data_dir: &Path, db: Option<&Db>) -> PlayerSettings {
    if let Ok(text) = std::fs::read_to_string(settings_path(data_dir))
        && let Ok(s) = PlayerSettings::from_toml(&text)
    {
        return s;
    }
    db.and_then(from_kovaaks).unwrap_or_default()
}

pub fn save(data_dir: &Path, settings: &PlayerSettings) -> Result<(), String> {
    let problems = settings.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(data_dir), settings.to_toml()).map_err(|e| e.to_string())
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
