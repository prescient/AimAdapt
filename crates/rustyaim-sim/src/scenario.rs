//! The scenario file format. A scenario is a TOML file; every field is optional and
//! falls back to the default shown here, so a file only needs the values it changes.
//! [`crate::spec`] describes each field for the editor and the docs.

use serde::{Deserialize, Serialize};

use crate::adaptive::Adaptive;

/// Current scenario file format version.
pub const SCHEMA_VERSION: u32 = 1;

/// A `[min, max]` pair. Each spawn or direction change picks a value in between.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Range(pub f32, pub f32);

impl Range {
    pub const fn fixed(v: f32) -> Self {
        Range(v, v)
    }

    pub fn min(self) -> f32 {
        self.0
    }

    pub fn max(self) -> f32 {
        self.1
    }

    pub fn contains(self, v: f32) -> bool {
        v >= self.0 && v <= self.1
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scenario {
    pub schema: u32,
    pub name: String,
    pub description: String,
    pub author: String,
    pub category: Category,
    /// The sub-skill it trains, for skill profiles and programs.
    pub skill: Skill,
    pub tags: Vec<String>,
    pub timing: Timing,
    pub arena: Arena,
    pub weapon: Weapon,
    pub targets: Targets,
    pub scoring: Scoring,
    /// How adaptive mode changes the difficulty.
    pub adaptive: Adaptive,
}

impl Default for Scenario {
    fn default() -> Self {
        Scenario {
            schema: SCHEMA_VERSION,
            name: "Untitled".into(),
            description: String::new(),
            author: String::new(),
            category: Category::Clicking,
            skill: Skill::None,
            tags: Vec::new(),
            timing: Timing::default(),
            arena: Arena::default(),
            weapon: Weapon::default(),
            targets: Targets::default(),
            scoring: Scoring::default(),
            adaptive: Adaptive::default(),
        }
    }
}

/// The skill a scenario trains. Used for grouping and, later, for building plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Clicking,
    Tracking,
    Switching,
}

/// Nine sub-skills in three groups, the split the Voltaic benchmarks use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skill {
    #[default]
    None,
    /// Flicking to still targets quickly and accurately.
    StaticClicking,
    /// Clicking on targets that change direction.
    DynamicClicking,
    /// Clicking on targets moving in straight lines.
    LinearClicking,
    /// Staying on small, slow, smooth targets.
    PreciseTracking,
    /// Following sudden direction changes.
    ReactiveTracking,
    /// Smooth, controlled tracking of predictable paths.
    ControlTracking,
    /// Moving between targets that die quickly.
    SpeedSwitching,
    /// Switching between dodging targets.
    EvasiveSwitching,
    /// Staying on each target long enough to finish it.
    StabilitySwitching,
}

impl Skill {
    pub const ALL: [Skill; 9] = [
        Skill::StaticClicking,
        Skill::DynamicClicking,
        Skill::LinearClicking,
        Skill::PreciseTracking,
        Skill::ReactiveTracking,
        Skill::ControlTracking,
        Skill::SpeedSwitching,
        Skill::EvasiveSwitching,
        Skill::StabilitySwitching,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Skill::None => "none",
            Skill::StaticClicking => "static_clicking",
            Skill::DynamicClicking => "dynamic_clicking",
            Skill::LinearClicking => "linear_clicking",
            Skill::PreciseTracking => "precise_tracking",
            Skill::ReactiveTracking => "reactive_tracking",
            Skill::ControlTracking => "control_tracking",
            Skill::SpeedSwitching => "speed_switching",
            Skill::EvasiveSwitching => "evasive_switching",
            Skill::StabilitySwitching => "stability_switching",
        }
    }

    pub fn from_id(id: &str) -> Option<Skill> {
        Skill::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Skill::None => "None",
            Skill::StaticClicking => "Static clicking",
            Skill::DynamicClicking => "Dynamic clicking",
            Skill::LinearClicking => "Linear clicking",
            Skill::PreciseTracking => "Precise tracking",
            Skill::ReactiveTracking => "Reactive tracking",
            Skill::ControlTracking => "Control tracking",
            Skill::SpeedSwitching => "Speed switching",
            Skill::EvasiveSwitching => "Evasive switching",
            Skill::StabilitySwitching => "Stability switching",
        }
    }

    pub fn group(self) -> Option<Category> {
        Some(match self {
            Skill::None => return None,
            Skill::StaticClicking | Skill::DynamicClicking | Skill::LinearClicking => Category::Clicking,
            Skill::PreciseTracking | Skill::ReactiveTracking | Skill::ControlTracking => Category::Tracking,
            Skill::SpeedSwitching | Skill::EvasiveSwitching | Skill::StabilitySwitching => Category::Switching,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Timing {
    /// Length of the run in seconds.
    pub duration_s: f32,
    /// Seconds of countdown before the clock starts. Targets show but can't be hit.
    pub countdown_s: f32,
    /// End the run early once this many targets are destroyed. 0 means no limit.
    pub kill_limit: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Timing { duration_s: 60.0, countdown_s: 3.0, kill_limit: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Arena {
    /// Room width, height and depth in meters. The player stands in the middle of the
    /// floor, facing the far wall.
    pub size: [f32; 3],
    /// Height of the player's eyes above the floor, in meters.
    pub eye_height: f32,
    pub wall_color: String,
    pub floor_color: String,
}

impl Default for Arena {
    fn default() -> Self {
        Arena { size: [60.0, 24.0, 60.0], eye_height: 1.7, wall_color: "#3a3f4b".into(), floor_color: "#2a2e37".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Weapon {
    pub kind: WeaponKind,
    pub trigger: Trigger,
    /// Hitscan: most shots per second (holding fires at this rate). 0 means no cap
    /// for click trigger.
    pub fire_rate: f32,
    /// Hitscan: damage per hit.
    pub damage: f32,
    /// Beam: damage per second while the crosshair is on a target.
    pub damage_per_s: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Weapon {
            kind: WeaponKind::Hitscan,
            trigger: Trigger::Click,
            fire_rate: 0.0,
            damage: 100.0,
            damage_per_s: 100.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponKind {
    /// Instant shots along the crosshair.
    Hitscan,
    /// A continuous beam that damages whatever the crosshair is on, sampled 60 times a second.
    Beam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// One shot per click.
    Click,
    /// Fires while the button is held.
    Hold,
    /// Always firing; for tracking where only aim matters.
    Always,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Targets {
    /// Targets alive at once.
    pub count: u32,
    pub shape: Shape,
    /// Radius in meters.
    pub radius: f32,
    /// Capsule height in meters (including the rounded ends).
    pub height: f32,
    /// Damage needed to destroy a target.
    pub health: f32,
    /// Restore health when the crosshair leaves the target (beam only).
    pub regen_per_s: f32,
    pub color: String,
    /// Seconds before a destroyed target is replaced.
    pub respawn_delay_s: f32,
    /// Seconds a target lives before it disappears as a timeout. 0 means forever.
    pub lifetime_s: f32,
    /// Size at the end of its lifetime, as a fraction of `radius` (needs `lifetime_s`).
    pub shrink_to: f32,
    /// Show a health bar above each target.
    pub health_bar: bool,
    /// Show the time left above targets that time out.
    pub timer_bar: bool,
    /// Targets that time out alternate between `color` and `alt_color`, so a new
    /// target looks different from the one before it.
    pub alternate_colors: bool,
    pub alt_color: String,
    pub spawn: Spawn,
    pub movement: Movement,
}

impl Default for Targets {
    fn default() -> Self {
        Targets {
            count: 1,
            shape: Shape::Sphere,
            radius: 0.3,
            height: 1.8,
            health: 100.0,
            regen_per_s: 0.0,
            color: "#ff5a4e".into(),
            respawn_delay_s: 0.0,
            lifetime_s: 0.0,
            shrink_to: 1.0,
            health_bar: false,
            timer_bar: true,
            alternate_colors: true,
            alt_color: "#4da3ff".into(),
            spawn: Spawn::default(),
            movement: Movement::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Sphere,
    /// Upright pill shape, roughly a body.
    Capsule,
}

/// Where targets appear, as angles from straight ahead and distance from the player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Spawn {
    /// Meters from the player.
    pub distance: Range,
    /// Degrees left (-) or right (+) of straight ahead.
    pub yaw: Range,
    /// Degrees below (-) or above (+) eye level.
    pub pitch: Range,
    /// Smallest angle in degrees between a new target and any live target.
    pub min_separation: f32,
    /// Largest angle in degrees from the last destroyed target. 0 means no limit.
    /// Small values make micro-flicks; large `min_separation` makes wide flicks.
    pub max_from_last: f32,
}

impl Default for Spawn {
    fn default() -> Self {
        Spawn {
            distance: Range::fixed(10.0),
            yaw: Range(-25.0, 25.0),
            pitch: Range(-5.0, 15.0),
            min_separation: 4.0,
            max_from_last: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Movement {
    pub kind: MovementKind,
    /// Meters per second.
    pub speed: Range,
    /// Seconds between direction changes.
    pub change_every_s: Range,
    /// Share of speed that can go up and down for `random` movement (0 = flat, 1 = any direction).
    pub vertical: f32,
}

impl Default for Movement {
    fn default() -> Self {
        Movement { kind: MovementKind::Static, speed: Range(3.0, 5.0), change_every_s: Range(0.4, 1.2), vertical: 0.5 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementKind {
    Static,
    /// Side to side, reversing at random intervals.
    Strafe,
    /// A new random direction at each change.
    Random,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scoring {
    pub per_kill: f32,
    pub per_hit: f32,
    /// Usually zero or negative.
    pub per_miss: f32,
    /// Points per point of damage dealt (beam scoring).
    pub per_damage: f32,
    /// Usually zero or negative.
    pub per_timeout: f32,
}

impl Default for Scoring {
    fn default() -> Self {
        Scoring { per_kill: 100.0, per_hit: 0.0, per_miss: 0.0, per_damage: 0.0, per_timeout: 0.0 }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScenarioError {
    #[error("{0}")]
    Toml(#[from] toml::de::Error),
    #[error("{}", .0.join("; "))]
    Invalid(Vec<String>),
}

impl Scenario {
    /// Parses and validates a scenario file.
    pub fn from_toml(text: &str) -> Result<Scenario, ScenarioError> {
        let s: Scenario = toml::from_str(text)?;
        let problems = s.problems();
        if problems.is_empty() { Ok(s) } else { Err(ScenarioError::Invalid(problems)) }
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("scenario always serializes")
    }

    /// A short fingerprint of everything that affects play, so harder or easier
    /// variants of a scenario with the same name are told apart in history.
    pub fn fingerprint(&self) -> String {
        let mut s = self.clone();
        s.description.clear();
        s.author.clear();
        s.tags.clear();
        // FNV-1a: stable across Rust versions, unlike `DefaultHasher`.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in s.to_toml().bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }

    /// Everything wrong with the values, in plain words. Empty when valid.
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        let mut check = |ok: bool, msg: &str| {
            if !ok {
                p.push(msg.to_string());
            }
        };
        let finite_range = |r: Range| r.0.is_finite() && r.1.is_finite() && r.0 <= r.1;
        let t = &self.targets;
        let sp = &t.spawn;
        let m = &t.movement;

        check(self.schema <= SCHEMA_VERSION, "made for a newer version of RustyAim");
        check(!self.name.trim().is_empty(), "name is empty");
        check(
            self.skill.group().is_none_or(|g| g == self.category),
            "the skill doesn't belong to the category (e.g. a tracking skill on a clicking scenario)",
        );
        check(self.timing.duration_s > 0.0 && self.timing.duration_s <= 3600.0, "duration must be 0–3600 seconds");
        check(self.timing.countdown_s >= 0.0 && self.timing.countdown_s <= 30.0, "countdown must be 0–30 seconds");
        check(self.arena.size.iter().all(|v| *v >= 2.0 && *v <= 1000.0), "arena size must be 2–1000 m per side");
        check(
            self.arena.eye_height > 0.0 && self.arena.eye_height < self.arena.size[1],
            "eye height must be inside the room",
        );
        check(self.weapon.fire_rate >= 0.0 && self.weapon.fire_rate <= 100.0, "fire rate must be 0–100 shots/s");
        check(
            self.weapon.kind != WeaponKind::Hitscan
                || self.weapon.trigger != Trigger::Hold
                || self.weapon.fire_rate > 0.0,
            "hold-to-fire hitscan needs a fire rate",
        );
        check(self.weapon.damage > 0.0, "damage must be above 0");
        check(self.weapon.damage_per_s > 0.0, "beam damage must be above 0");
        check(t.count >= 1 && t.count <= 50, "target count must be 1–50");
        check(t.radius >= 0.01 && t.radius <= 10.0, "target radius must be 0.01–10 m");
        check(t.shape != Shape::Capsule || t.height >= 2.0 * t.radius, "capsule height must be at least 2× radius");
        check(t.health > 0.0, "target health must be above 0");
        check(t.regen_per_s >= 0.0, "regen can't be negative");
        check(t.respawn_delay_s >= 0.0, "respawn delay can't be negative");
        check(t.lifetime_s >= 0.0, "lifetime can't be negative");
        check(t.shrink_to > 0.0 && t.shrink_to <= 1.0, "shrink-to must be above 0 and at most 1");
        check(parse_color(&t.color).is_some(), "target color must look like #ff5a4e");
        check(parse_color(&t.alt_color).is_some(), "second target color must look like #4da3ff");
        check(parse_color(&self.arena.wall_color).is_some(), "wall color must look like #3a3f4b");
        check(parse_color(&self.arena.floor_color).is_some(), "floor color must look like #2a2e37");
        check(finite_range(sp.distance) && sp.distance.0 >= 1.0, "spawn distance must be at least 1 m, min ≤ max");
        check(
            finite_range(sp.yaw) && sp.yaw.0 >= -180.0 && sp.yaw.1 <= 180.0,
            "spawn yaw must be within ±180°, min ≤ max",
        );
        check(
            finite_range(sp.pitch) && sp.pitch.0 >= -85.0 && sp.pitch.1 <= 85.0,
            "spawn pitch must be within ±85°, min ≤ max",
        );
        check(sp.min_separation >= 0.0 && sp.min_separation < 180.0, "min separation must be 0–180°");
        check(sp.max_from_last >= 0.0 && sp.max_from_last <= 360.0, "max from last must be 0–360°");
        check(
            sp.max_from_last == 0.0 || sp.max_from_last >= sp.min_separation,
            "max from last must be at least min separation",
        );
        check(finite_range(m.speed) && m.speed.0 >= 0.0 && m.speed.1 <= 100.0, "speed must be 0–100 m/s, min ≤ max");
        check(
            finite_range(m.change_every_s) && m.change_every_s.0 >= 0.05,
            "direction change must be at least 0.05 s, min ≤ max",
        );
        check((0.0..=1.0).contains(&m.vertical), "vertical share must be 0–1");
        // Targets must fit in the room.
        let half_w = self.arena.size[0] / 2.0;
        check(sp.distance.1 + t.radius < half_w.max(self.arena.size[2] / 2.0), "spawn distance reaches past the walls");
        p.extend(self.adaptive.problems());
        p
    }
}

/// Parses `#rrggbb` into linear-ish 0–1 sRGB components.
pub fn parse_color(s: &str) -> Option<[f32; 3]> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok().map(|v| f32::from(v) / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_is_the_default_scenario() {
        let s = Scenario::from_toml("name = \"x\"").unwrap();
        assert_eq!(s.targets, Targets::default());
        assert_eq!(s.name, "x");
    }

    #[test]
    fn round_trips_through_toml() {
        let s = Scenario { name: "Round trip".into(), ..Default::default() };
        assert_eq!(Scenario::from_toml(&s.to_toml()).unwrap(), s);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err = Scenario::from_toml("name = \"x\"\n[targets]\nradiuss = 1.0").unwrap_err();
        assert!(err.to_string().contains("radiuss"), "{err}");
    }

    #[test]
    fn bad_values_are_explained() {
        let mut s = Scenario::default();
        s.targets.radius = 0.0;
        s.targets.spawn.yaw = Range(10.0, -10.0);
        let p = s.problems();
        assert!(p.iter().any(|m| m.contains("radius")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("yaw")), "{p:?}");
    }

    #[test]
    fn fingerprint_ignores_description_but_not_difficulty() {
        let a = Scenario::default();
        let b = Scenario { description: "notes".into(), ..a.clone() };
        let mut c = a.clone();
        c.targets.radius = 0.25;
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_ne!(a.fingerprint(), c.fingerprint());
    }

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#ff0000"), Some([1.0, 0.0, 0.0]));
        assert_eq!(parse_color("ff0000"), None);
        assert_eq!(parse_color("#ff00"), None);
    }
}
