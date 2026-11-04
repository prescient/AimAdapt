//! Settings profiles: named sets of player settings, usually one per game, so the
//! trainer can feel like Overwatch one day and Counter-Strike the next. A profile
//! can take its sensitivity in a game's own units (e.g. Valorant 0.35 at 800 DPI);
//! the engine always works in cm/360.
//!
//! Stored in `engine.toml`:
//!
//! ```toml
//! active = "Overwatch"
//!
//! [[profiles]]
//! name = "Overwatch"
//! game = "overwatch"
//! game_sens = 5.3
//!
//! [profiles.settings]
//! dpi = 400
//! fov = 103.0
//! ```

use serde::{Deserialize, Serialize};

use crate::settings::PlayerSettings;

/// A game whose sensitivity can be typed in its own units.
#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub id: &'static str,
    pub name: &'static str,
    /// Degrees turned per mouse count at sensitivity 1. 0 for "custom".
    pub yaw: f32,
    /// The game's usual FOV, in this trainer's units (horizontal at 16:9).
    pub fov: Option<f32>,
    /// How to read the game's FOV setting as ours.
    pub fov_note: &'static str,
}

/// Games with known sensitivity scales. "custom" means cm/360 typed directly.
pub const GAMES: &[Game] = &[
    Game {
        id: "overwatch",
        name: "Overwatch 2",
        yaw: 0.0066,
        fov: Some(103.0),
        fov_note: "Overwatch's FOV setting is horizontal at 16:9, the same as here.",
    },
    Game { id: "valorant", name: "Valorant", yaw: 0.07, fov: Some(103.0), fov_note: "Valorant's FOV is fixed at 103." },
    Game {
        id: "cs2",
        name: "Counter-Strike 2",
        yaw: 0.022,
        fov: Some(106.26),
        fov_note: "CS2's FOV is 90 measured at 4:3, which is 106.3 here.",
    },
    Game {
        id: "apex",
        name: "Apex Legends",
        yaw: 0.022,
        fov: Some(121.28),
        fov_note: "Apex's FOV slider is measured at 4:3: 90 is 106.3 here and 110 is 121.3.",
    },
    Game {
        id: "cod",
        name: "Call of Duty",
        yaw: 0.0066,
        fov: None,
        fov_note: "Enter your FOV as its horizontal value on a 16:9 screen.",
    },
    Game {
        id: "custom",
        name: "Custom (cm/360)",
        yaw: 0.0,
        fov: None,
        fov_note: "FOV is horizontal on a 16:9 screen.",
    },
];

pub fn game(id: &str) -> Option<&'static Game> {
    GAMES.iter().find(|g| g.id == id)
}

/// The game a KovaaK's `Sens Scale` name matches, if any.
pub fn game_for_kovaaks_scale(scale: &str) -> Option<&'static Game> {
    let id = match scale {
        "Overwatch" => "overwatch",
        "Valorant" => "valorant",
        "CS:GO" | "CS2" | "Counter-Strike 2" | "Source" | "Quake/Source" => "cs2",
        "Apex Legends" | "Apex" => "apex",
        _ => return None,
    };
    game(id)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    /// A [`GAMES`] id. With "custom", `settings.cm_per_360` is used as typed.
    pub game: String,
    /// Sensitivity in the game's own units (ignored for "custom").
    pub game_sens: f32,
    pub settings: PlayerSettings,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { name: "Default".into(), game: "custom".into(), game_sens: 1.0, settings: PlayerSettings::default() }
    }
}

impl Profile {
    /// cm/360 for this profile: converted from the game's units, or as typed.
    pub fn cm_per_360(&self) -> f32 {
        match game(&self.game) {
            Some(g) if g.yaw > 0.0 && self.game_sens > 0.0 => {
                let counts = 360.0 / (g.yaw * self.game_sens);
                counts / self.settings.dpi.max(1) as f32 * 2.54
            }
            _ => self.settings.cm_per_360,
        }
    }

    /// The settings the engine plays with.
    pub fn resolved(&self) -> PlayerSettings {
        PlayerSettings { cm_per_360: self.cm_per_360(), ..self.settings.clone() }
    }

    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.name.trim().is_empty() {
            p.push("profile name is empty".into());
        }
        match game(&self.game) {
            None => p.push(format!("unknown game {}", self.game)),
            Some(g) if g.yaw > 0.0 && !(self.game_sens > 0.0 && self.game_sens.is_finite()) => {
                p.push(format!("{}: sensitivity must be above 0", self.name))
            }
            _ => {}
        }
        p.extend(self.resolved().problems().into_iter().map(|m| format!("{}: {m}", self.name)));
        p
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profiles {
    /// Name of the profile the trainer uses.
    pub active: String,
    pub profiles: Vec<Profile>,
}

impl Default for Profiles {
    fn default() -> Self {
        Profiles { active: "Default".into(), profiles: vec![Profile::default()] }
    }
}

impl Profiles {
    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    /// The active profile, or the first one if the active name is missing.
    pub fn active(&self) -> &Profile {
        static FALLBACK: std::sync::OnceLock<Profile> = std::sync::OnceLock::new();
        self.get(&self.active).or(self.profiles.first()).unwrap_or_else(|| FALLBACK.get_or_init(Profile::default))
    }

    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.profiles.is_empty() {
            p.push("keep at least one profile".into());
        }
        if self.get(&self.active).is_none() {
            p.push(format!("the active profile {} doesn't exist", self.active));
        }
        let mut names: Vec<String> = Vec::new();
        for prof in &self.profiles {
            let key = prof.name.trim().to_lowercase();
            if names.contains(&key) {
                p.push(format!("two profiles are called {}", prof.name));
            }
            names.push(key);
            p.extend(prof.problems());
        }
        p
    }

    /// Reads `engine.toml`. Files from before profiles existed (a bare
    /// [`PlayerSettings`]) become a single "Default" profile.
    pub fn from_toml(text: &str) -> Result<Profiles, String> {
        let parsed = match toml::from_str::<Profiles>(text) {
            Ok(p) => p,
            Err(e) => match PlayerSettings::from_toml(text) {
                Ok(settings) => Profiles {
                    active: "Default".into(),
                    profiles: vec![Profile { name: "Default".into(), game: "custom".into(), game_sens: 1.0, settings }],
                },
                Err(_) => return Err(e.to_string()),
            },
        };
        let p = parsed.problems();
        if p.is_empty() { Ok(parsed) } else { Err(p.join("; ")) }
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("profiles always serialize")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, game: &str, sens: f32, dpi: u32) -> Profile {
        Profile {
            name: name.into(),
            game: game.into(),
            game_sens: sens,
            settings: PlayerSettings { dpi, ..Default::default() },
        }
    }

    #[test]
    fn game_sensitivity_converts_to_cm_per_360() {
        // Overwatch 5.3 at 400 DPI is about 65.35 cm/360.
        assert!((profile("a", "overwatch", 5.3, 400).cm_per_360() - 65.35).abs() < 0.05);
        // Valorant 0.35 at 800 DPI is about 46.65 cm/360.
        assert!((profile("b", "valorant", 0.35, 800).cm_per_360() - 46.65).abs() < 0.05);
        // CS2 1.0 at 800 DPI is about 51.95 cm/360.
        assert!((profile("c", "cs2", 1.0, 800).cm_per_360() - 51.95).abs() < 0.05);
        // Custom uses the typed cm/360.
        let mut c = profile("d", "custom", 0.0, 800);
        c.settings.cm_per_360 = 40.0;
        assert_eq!(c.cm_per_360(), 40.0);
        assert_eq!(c.resolved().cm_per_360, 40.0);
    }

    #[test]
    fn profiles_round_trip_and_pick_the_active_one() {
        let ps = Profiles {
            active: "Valorant".into(),
            profiles: vec![profile("Overwatch", "overwatch", 5.3, 400), profile("Valorant", "valorant", 0.35, 800)],
        };
        let back = Profiles::from_toml(&ps.to_toml()).unwrap();
        assert_eq!(back, ps);
        assert_eq!(back.active().name, "Valorant");
    }

    #[test]
    fn bad_profiles_are_explained() {
        let ps = Profiles {
            active: "Missing".into(),
            profiles: vec![profile("A", "overwatch", 0.0, 400), profile("a", "nope", 1.0, 400)],
        };
        let p = ps.problems().join(" | ");
        assert!(
            p.contains("Missing")
                && p.contains("sensitivity")
                && p.contains("two profiles")
                && p.contains("unknown game"),
            "{p}"
        );
    }

    #[test]
    fn old_settings_files_become_a_default_profile() {
        let old = PlayerSettings { cm_per_360: 42.0, ..Default::default() }.to_toml();
        let ps = Profiles::from_toml(&old).unwrap();
        assert_eq!(ps.active().name, "Default");
        assert_eq!(ps.active().resolved().cm_per_360, 42.0);
    }

    #[test]
    fn every_game_is_unique_and_kovaaks_scales_map() {
        for (i, g) in GAMES.iter().enumerate() {
            assert!(GAMES.iter().skip(i + 1).all(|h| h.id != g.id), "{}", g.id);
        }
        assert_eq!(game_for_kovaaks_scale("Overwatch").map(|g| g.id), Some("overwatch"));
        assert!(game_for_kovaaks_scale("Fortnite").is_none());
    }
}
