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
    pub display: Display,
    pub show_fps: bool,
    pub crosshair: Crosshair,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        PlayerSettings {
            cm_per_360: 35.0,
            dpi: 800,
            fov: 103.0,
            display: Display::default(),
            show_fps: false,
            crosshair: Crosshair::default(),
        }
    }
}

/// How the trainer window uses the screen.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Display {
    pub mode: DisplayMode,
    /// Screen resolution in pixels for exclusive full screen (e.g. 1280x960 for
    /// stretched 4:3), or the window size when windowed. Unset: the desktop
    /// resolution, or 1600x900 for a window.
    pub resolution: Option<[u32; 2]>,
    /// Exclusive full screen only. Unset: the highest rate at that resolution.
    pub refresh_hz: Option<u32>,
    /// Draw the game in a centered area of this many pixels with black around it,
    /// e.g. to play at 24" size on a larger monitor. Unset: the whole screen.
    pub play_area: Option<[u32; 2]>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    /// Full screen without changing the desktop resolution. Fast alt-tab.
    #[default]
    Borderless,
    /// True full screen at `resolution`, which can differ from the desktop's.
    Exclusive,
    Windowed,
}

impl Display {
    /// The play area actually used inside a screen of `size` pixels: centered, no
    /// bigger than the screen. `None` means the whole screen.
    pub fn play_area_in(&self, size: [u32; 2]) -> Option<([u32; 2], [u32; 2])> {
        let [w, h] = self.play_area?;
        let (w, h) = (w.min(size[0]), h.min(size[1]));
        if w == size[0] && h == size[1] {
            return None;
        }
        Some(([(size[0] - w) / 2, (size[1] - h) / 2], [w, h]))
    }

    /// The play area for a `diagonal_in`-inch screen inside a monitor of
    /// `monitor_px` pixels and `monitor_in` inches, keeping the monitor's shape.
    pub fn play_area_for_size(monitor_px: [u32; 2], monitor_in: f32, diagonal_in: f32) -> [u32; 2] {
        let k = (diagonal_in / monitor_in).clamp(0.1, 1.0);
        [(monitor_px[0] as f32 * k).round() as u32, (monitor_px[1] as f32 * k).round() as u32]
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
    /// gets wider, not shorter, on wider screens or play areas (and narrower on 4:3,
    /// as in most shooters).
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
        let px_ok =
            |r: Option<[u32; 2]>| r.is_none_or(|[w, h]| (320..=16384).contains(&w) && (240..=16384).contains(&h));
        if !px_ok(self.display.resolution) {
            p.push("resolution must be at least 320x240".into());
        }
        if !px_ok(self.display.play_area) {
            p.push("play area must be at least 320x240".into());
        }
        if self.display.refresh_hz.is_some_and(|r| !(24..=1000).contains(&r)) {
            p.push("refresh rate must be 24–1000 Hz".into());
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
    fn play_area_is_centered_and_clamped() {
        let d = Display { play_area: Some([1920, 1080]), ..Default::default() };
        assert_eq!(d.play_area_in([3840, 2160]), Some(([960, 540], [1920, 1080])));
        assert_eq!(d.play_area_in([1920, 1080]), None, "fills the screen");
        assert_eq!(d.play_area_in([1600, 900]), None, "never bigger than the screen");
        // A 24" area on a 42" 4K screen.
        assert_eq!(Display::play_area_for_size([3840, 2160], 42.0, 24.0), [2194, 1234]);
    }

    #[test]
    fn display_settings_round_trip() {
        let s = PlayerSettings {
            display: Display {
                mode: DisplayMode::Exclusive,
                resolution: Some([1280, 960]),
                refresh_hz: Some(240),
                play_area: None,
            },
            ..Default::default()
        };
        assert_eq!(PlayerSettings::from_toml(&s.to_toml()).unwrap(), s);
        let bad = PlayerSettings { display: Display { resolution: Some([100, 100]), ..Default::default() }, ..s };
        assert!(!bad.problems().is_empty());
    }

    #[test]
    fn fov_103_is_about_70_vertical() {
        let v = PlayerSettings::default().vertical_fov().to_degrees();
        assert!((v - 70.5).abs() < 0.5, "{v}");
    }
}
