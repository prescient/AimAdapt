//! The player's own settings: sensitivity, field of view and crosshair. These stay
//! the same across scenarios, so they live in their own file
//! (`%APPDATA%\RustyAim\engine.toml`) instead of in each scenario.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlayerSettings {
    /// Centimeters of mouse movement for a full turn.
    pub cm_per_360: f32,
    /// Mouse counts per inch.
    pub dpi: u32,
    /// Horizontal field of view in degrees, measured on a 16:9 screen (like
    /// Overwatch and Valorant). Wider screens see more at the sides.
    pub fov: f32,
    pub fullscreen: bool,
    pub show_fps: bool,
    pub crosshair: Crosshair,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        PlayerSettings {
            cm_per_360: 35.0,
            dpi: 800,
            fov: 103.0,
            fullscreen: true,
            show_fps: false,
            crosshair: Crosshair::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Crosshair {
    pub color: String,
    /// Length of each arm in pixels. 0 for a dot only.
    pub size: f32,
    pub thickness: f32,
    /// Gap between the center and each arm in pixels.
    pub gap: f32,
    pub dot: bool,
}

impl Default for Crosshair {
    fn default() -> Self {
        Crosshair { color: "#3cff8f".into(), size: 6.0, thickness: 2.0, gap: 3.0, dot: true }
    }
}

impl PlayerSettings {
    /// Degrees of turn per mouse count.
    pub fn degrees_per_count(&self) -> f32 {
        let counts_per_turn = self.cm_per_360 / 2.54 * self.dpi.max(1) as f32;
        360.0 / counts_per_turn
    }

    /// Vertical field of view in radians. Fixed at the 16:9 value so the view
    /// gets wider, not shorter, on wider screens.
    pub fn vertical_fov(&self) -> f32 {
        let h = self.fov.clamp(30.0, 150.0).to_radians();
        2.0 * ((h / 2.0).tan() * 9.0 / 16.0).atan()
    }

    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if !(1.0..=500.0).contains(&self.cm_per_360) {
            p.push("cm/360 must be 1–500".into());
        }
        if !(50..=50_000).contains(&self.dpi) {
            p.push("DPI must be 50–50000".into());
        }
        if !(30.0..=150.0).contains(&self.fov) {
            p.push("FOV must be 30–150°".into());
        }
        if crate::scenario::parse_color(&self.crosshair.color).is_none() {
            p.push("crosshair color must look like #3cff8f".into());
        }
        p
    }

    pub fn from_toml(text: &str) -> Result<PlayerSettings, String> {
        let s: PlayerSettings = toml::from_str(text).map_err(|e| e.to_string())?;
        let p = s.problems();
        if p.is_empty() { Ok(s) } else { Err(p.join("; ")) }
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("settings always serialize")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitivity_conversion() {
        // 400 DPI at 65.3 cm/360 (Overwatch 5.3) turns 0.03498° per count.
        let s = PlayerSettings { cm_per_360: 65.3, dpi: 400, ..Default::default() };
        assert!((s.degrees_per_count() - 0.035).abs() < 1e-4, "{}", s.degrees_per_count());
    }

    #[test]
    fn fov_103_is_about_70_vertical() {
        let v = PlayerSettings::default().vertical_fov().to_degrees();
        assert!((v - 70.5).abs() < 0.5, "{v}");
    }
}
