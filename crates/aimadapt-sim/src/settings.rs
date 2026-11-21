//! The player's own settings: sensitivity, field of view, crosshair and sound. These stay
//! the same across scenarios, so they live in their own file
//! (`%APPDATA%\AimAdapt\engine.toml`) instead of in each scenario.

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
    /// The adaptive mix as it was set per profile. The trainer now plays with the mix
    /// in `training.toml` (see `prefs`) once one is chosen there. This one is still
    /// read, so a mix picked before that carries over, and still written.
    pub challenge: crate::adaptive::Mix,
    pub show_fps: bool,
    pub crosshair: Crosshair,
    pub sound: Sound,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        PlayerSettings {
            cm_per_360: 35.0,
            dpi: 800,
            fov: 103.0,
            display: Display::default(),
            challenge: crate::adaptive::Mix::default(),
            show_fps: false,
            crosshair: Crosshair::default(),
            sound: Sound::default(),
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
    /// Draw the game in an area of this many pixels with black around it, e.g. to
    /// play at 24" size on a larger monitor. Unset: the whole screen.
    pub play_area: Option<[u32; 2]>,
    /// Where the play area sits: the share of the black space that goes to its
    /// left and above it, 0–1 each. Unset: centered (`[0.5, 0.5]`). `[0.5, 0.0]`
    /// puts it at the top of the screen.
    pub play_area_position: Option<[f32; 2]>,
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
    /// The play area actually used inside a screen of `size` pixels: its top-left
    /// corner and size, at its position and no bigger than the screen. `None` means
    /// the whole screen.
    pub fn play_area_in(&self, size: [u32; 2]) -> Option<([u32; 2], [u32; 2])> {
        let [w, h] = self.play_area?;
        let (w, h) = (w.min(size[0]), h.min(size[1]));
        if w == size[0] && h == size[1] {
            return None;
        }
        let [fx, fy] = self.play_area_position.unwrap_or([0.5, 0.5]).map(|f| f.clamp(0.0, 1.0));
        let at = |spare: u32, f: f32| (spare as f32 * f).round() as u32;
        Some(([at(size[0] - w, fx), at(size[1] - h, fy)], [w, h]))
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

/// The trainer's sound: a pop on every shot, which it makes itself (there are no sound files).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "SavedSound")]
pub struct Sound {
    /// How loud the pop is, 0 (off) to 100.
    pub volume: u32,
}

impl Default for Sound {
    fn default() -> Self {
        Sound { volume: 50 }
    }
}

/// `sound` as it may be saved. From 19 to 21 November 2026 it also had two switches: `shots` for the pop and
/// `hits` for a tick and a chime on hits. They went with the tick and the chime, and files that still have them
/// load: a pop that was switched off is kept off, at volume 0.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SavedSound {
    volume: u32,
    shots: bool,
    #[serde(rename = "hits")]
    _hits: Option<serde::de::IgnoredAny>,
}

impl Default for SavedSound {
    fn default() -> Self {
        SavedSound { volume: Sound::default().volume, shots: true, _hits: None }
    }
}

impl From<SavedSound> for Sound {
    fn from(s: SavedSound) -> Sound {
        Sound { volume: if s.shots { s.volume } else { 0 } }
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
        if self.display.play_area_position.is_some_and(|v| v.iter().any(|f| !(0.0..=1.0).contains(f))) {
            p.push("play area position must be 0–100% each way".into());
        }
        if self.display.refresh_hz.is_some_and(|r| !(24..=1000).contains(&r)) {
            p.push("refresh rate must be 24–1000 Hz".into());
        }
        if crate::scenario::parse_color(&self.crosshair.color).is_none() {
            p.push("crosshair color must look like #3cff8f".into());
        }
        let c = &self.crosshair;
        if !(0.0..=40.0).contains(&c.size) {
            p.push("crosshair arm length must be 0–40 px".into());
        }
        if !(1.0..=10.0).contains(&c.thickness) {
            p.push("crosshair thickness must be 1–10 px".into());
        }
        if !(0.0..=30.0).contains(&c.gap) {
            p.push("crosshair gap must be 0–30 px".into());
        }
        if self.sound.volume > 100 {
            p.push("sound volume must be 0–100".into());
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

    /// Bug: crosshair numbers weren't checked, so `size = nan` or `gap = inf` in
    /// engine.toml loaded and drew nothing (or covered the screen).
    #[test]
    fn crosshair_numbers_are_checked() {
        let cases: [fn(&mut Crosshair); 4] =
            [|c| c.size = f32::NAN, |c| c.gap = f32::INFINITY, |c| c.thickness = 0.0, |c| c.size = 500.0];
        for set in cases {
            let mut s = PlayerSettings::default();
            set(&mut s.crosshair);
            assert!(!s.problems().is_empty(), "{:?}", s.crosshair);
        }
        assert!(PlayerSettings::default().problems().is_empty());
    }

    /// Sound was added after engine.toml files were written. A file without it plays the
    /// default pop, and a volume past 100 is refused.
    #[test]
    fn sound_settings_default_and_are_checked() {
        let old = "cm_per_360 = 40.0\ndpi = 800\nfov = 103.0\n";
        let s = PlayerSettings::from_toml(old).unwrap();
        assert_eq!(s.sound, Sound { volume: 50 });
        let quiet = PlayerSettings { sound: Sound { volume: 0 }, ..s.clone() };
        assert_eq!(PlayerSettings::from_toml(&quiet.to_toml()).unwrap(), quiet);
        assert!(!quiet.to_toml().contains("shots"), "{}", quiet.to_toml());
        let loud = PlayerSettings { sound: Sound { volume: 101 }, ..s };
        assert!(loud.problems().iter().any(|p| p.contains("sound volume")), "{:?}", loud.problems());
        let typo = format!("{old}[sound]\nvolumee = 40\n");
        assert!(PlayerSettings::from_toml(&typo).is_err(), "an unknown sound key is still refused");
    }

    /// Bug guard (21 Nov 2026): the tick, the chime and their two switches went, and profiles saved with the
    /// switches (19-21 Nov) have to keep loading instead of being set aside as unusable.
    #[test]
    fn sound_saved_with_the_old_switches_still_loads() {
        let base = "cm_per_360 = 40.0\ndpi = 800\nfov = 103.0\n";
        let on = format!("{base}[sound]\nvolume = 70\nshots = true\nhits = false\n");
        assert_eq!(PlayerSettings::from_toml(&on).unwrap().sound, Sound { volume: 70 });
        let off = format!("{base}[sound]\nvolume = 70\nshots = false\nhits = true\n");
        assert_eq!(PlayerSettings::from_toml(&off).unwrap().sound, Sound { volume: 0 }, "a pop switched off stays off");
    }

    #[test]
    fn sensitivity_conversion() {
        // 400 DPI at 65.3 cm/360 (Overwatch 5.3) turns 0.03498° per count.
        let s = PlayerSettings { cm_per_360: 65.3, dpi: 400, ..Default::default() };
        assert!((s.degrees_per_count() - 0.035).abs() < 1e-4, "{}", s.degrees_per_count());
    }

    #[test]
    /// Playtest: the play area was always centered, but the player keeps their
    /// window near the top of the screen. It can sit anywhere now.
    fn play_area_can_sit_at_the_top_or_anywhere() {
        let d = Display { play_area: Some([1920, 1080]), play_area_position: Some([0.5, 0.0]), ..Default::default() };
        assert_eq!(d.play_area_in([3840, 2160]), Some(([960, 0], [1920, 1080])), "top center");
        let d = Display { play_area_position: Some([0.0, 1.0]), ..d };
        assert_eq!(d.play_area_in([3840, 2160]), Some(([0, 1080], [1920, 1080])), "bottom left");
        let d = Display { play_area_position: Some([0.25, 0.1]), ..d };
        assert_eq!(d.play_area_in([3840, 2160]), Some(([480, 108], [1920, 1080])));
        // Out-of-range positions are rejected, and clamped if they get through.
        let p = PlayerSettings {
            display: Display { play_area_position: Some([1.5, 0.0]), ..d.clone() },
            ..Default::default()
        };
        assert!(p.problems().iter().any(|x| x.contains("position")), "{:?}", p.problems());
        assert_eq!(p.display.play_area_in([3840, 2160]), Some(([1920, 0], [1920, 1080])));
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
                play_area_position: None,
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
