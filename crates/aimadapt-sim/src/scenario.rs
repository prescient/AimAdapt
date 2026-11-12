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

impl Default for Range {
    fn default() -> Self {
        Range(0.0, 0.0)
    }
}

thread_local! {
    static FINGERPRINTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Fields added after fingerprints were first stored are left out of the
/// fingerprint while they're at their "off" value, so adding a field doesn't change
/// the fingerprint of scenarios that don't use it (which would reset their
/// ratings). Their defaults must stay at `Default::default()`.
fn off_in_fingerprint<T: PartialEq + Default>(v: &T) -> bool {
    FINGERPRINTING.with(std::cell::Cell::get) && *v == T::default()
}

/// FNV-1a: stable across Rust versions, unlike `DefaultHasher`.
fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

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
    /// Set on scenarios imported from KovaaK's: they play on the scenario's own map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world: Option<crate::world::World>,
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
            world: None,
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
    /// Health restored per second: with a beam while the crosshair is off the
    /// target, with hitscan once `regen_delay_s` has passed since the last hit.
    /// Negative (imported KovaaK's scenarios) drains it once the delay has passed,
    /// until the target goes as a timeout. An imported scenario (one with a world) heals with
    /// hitscan in steps of a tenth of a second's regen, as KovaaK's does; the drain is smooth.
    pub regen_per_s: f32,
    /// Seconds after the last hit before regen starts. 0 heals straight away. With the steps of an
    /// imported scenario, each hit starts the wait again and a delay of 0 is never paused.
    #[serde(skip_serializing_if = "off_in_fingerprint")]
    pub regen_delay_s: f32,
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
            regen_delay_s: 0.0,
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
    /// An upright box. `radius` is half its width and half its depth, `height` its full
    /// height, and its faces lie along the sim's axes (X, Y up, Z).
    Box,
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
    /// How quickly a target changes velocity, in m/s². 0 turns and reverses
    /// instantly; a value makes it slow down and speed back up, and turns curve.
    #[serde(skip_serializing_if = "off_in_fingerprint")]
    pub accel: f32,
    /// Speed toward or away from the player, in m/s, within the spawn distance
    /// range. `[0, 0]` keeps each target at one distance.
    #[serde(skip_serializing_if = "off_in_fingerprint")]
    pub approach_speed: Range,
    /// Seconds between switching from coming closer to backing away. `[0, 0]`
    /// uses `change_every_s`.
    #[serde(skip_serializing_if = "off_in_fingerprint")]
    pub approach_every_s: Range,
    /// Seconds on the ground between jumps. `[0, 0]` never jumps.
    #[serde(skip_serializing_if = "off_in_fingerprint")]
    pub jump_every_s: Range,
    /// Height of a jump in meters, and seconds in the air. Together they set
    /// gravity: 8 × height ÷ airtime².
    #[serde(skip_serializing_if = "off_in_fingerprint")]
    pub jump_height: f32,
    #[serde(skip_serializing_if = "off_in_fingerprint")]
    pub jump_air_s: f32,
    /// `curve` movement: how fast the heading turns, in degrees per second. Each
    /// direction change picks a new turn rate from this range, turning left or right.
    #[serde(skip_serializing_if = "off_in_fingerprint")]
    pub turn_deg_s: Range,
}

impl Default for Movement {
    fn default() -> Self {
        Movement {
            kind: MovementKind::Static,
            speed: Range(3.0, 5.0),
            change_every_s: Range(0.4, 1.2),
            vertical: 0.5,
            accel: 0.0,
            approach_speed: Range(0.0, 0.0),
            approach_every_s: Range(0.0, 0.0),
            jump_every_s: Range(0.0, 0.0),
            jump_height: 0.0,
            jump_air_s: 0.0,
            turn_deg_s: Range(0.0, 0.0),
        }
    }
}

impl Movement {
    pub fn approaches(&self) -> bool {
        self.approach_speed.1 > 0.0
    }

    pub fn jumps(&self) -> bool {
        self.jump_height > 0.0 && self.jump_air_s > 0.0
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
    /// Steady speed along curved paths: the heading keeps turning, and each change
    /// picks a new turn rate and side.
    Curve,
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

    /// How long a run takes in real seconds, countdown included: the duration is in game
    /// seconds, which a time-scaled import plays at `time_scale` per real second.
    pub fn real_seconds(&self) -> f32 {
        let scale = self.world.as_ref().map_or(1.0, |w| w.time_scale);
        self.timing.duration_s / scale + self.timing.countdown_s
    }

    /// A short fingerprint of everything that affects play, so harder or easier
    /// variants of a scenario with the same name are told apart in history: its
    /// settings and, for a scenario that uses a rule the sim has changed since, a mark
    /// for the change (see `rule_marks`).
    pub fn fingerprint(&self) -> String {
        fnv(&(self.settings_text() + &self.rule_marks()))
    }

    /// Marks for the rules the sim has changed since scenarios were first stored. A rule
    /// change leaves a scenario's settings as they were, so without a mark its scores from
    /// before would count as the same version. Only a scenario that uses the rule gets the
    /// mark, and the rest keep their fingerprints.
    fn rule_marks(&self) -> String {
        let mut marks = String::new();
        // Plan K6b: jumps, jetpacks and fliers are played by a jump key.
        let keys = self.world.as_ref().is_some_and(|w| w.motions().any(crate::motion::Motion::works_keys));
        if keys {
            marks += "\n# rules: jump key";
        }
        // Imported bots with a hitscan weapon heal in steps of a tenth of a second.
        let heals = self.weapon.kind == WeaponKind::Hitscan
            && self
                .world
                .as_ref()
                .is_some_and(|w| self.targets.regen_per_s > 0.0 || w.kinds.iter().any(|k| k.regen_per_s > 0.0));
        if heals {
            marks += "\n# rules: heals in steps";
        }
        // Imported bots use their abilities at the ticks and tries measured in KovaaK's.
        let abilities = self
            .world
            .as_ref()
            .is_some_and(|w| w.motions().any(|m| m.abilities.as_ref().is_some_and(|a| !a.list.is_empty())));
        if abilities {
            marks += "\n# rules: ability tries";
            // The start block, the reuse clocks and the frequency windows run on from one life of a bot to
            // the next instead of starting again with each bot (see `motion::Brain`).
            marks += "\n# rules: abilities over respawns";
        }
        marks
    }

    /// The settings that go into the fingerprint, as text.
    fn settings_text(&self) -> String {
        let mut s = self.clone();
        s.description.clear();
        s.author.clear();
        s.tags.clear();
        FINGERPRINTING.with(|f| f.set(true));
        let text = s.to_toml();
        FINGERPRINTING.with(|f| f.set(false));
        text
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

        check(self.schema <= SCHEMA_VERSION, "made for a newer version of AimAdapt");
        if let Some(w) = &self.world {
            for m in w.problems() {
                check(false, &m);
            }
        }
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
        check(
            t.shape == Shape::Sphere || t.height >= 2.0 * t.radius,
            "capsule and box height must be at least 2× radius",
        );
        check(t.health > 0.0, "target health must be above 0");
        check(t.regen_per_s.is_finite(), "regen must be a number");
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
        check(t.regen_delay_s.is_finite() && (0.0..=10.0).contains(&t.regen_delay_s), "regen delay must be 0–10 s");
        check(m.accel.is_finite() && (0.0..=500.0).contains(&m.accel), "acceleration must be 0–500 m/s²");
        check(
            finite_range(m.approach_speed) && m.approach_speed.0 >= 0.0 && m.approach_speed.1 <= 30.0,
            "forward/back speed must be 0–30 m/s, min ≤ max",
        );
        check(
            !m.approaches() || sp.distance.0 < sp.distance.1,
            "forward/back movement needs a spawn distance range to move in (min < max)",
        );
        check(finite_range(m.approach_every_s) && m.approach_every_s.0 >= 0.0, "forward/back timing can't be negative");
        check(finite_range(m.jump_every_s) && m.jump_every_s.0 >= 0.0, "time between jumps can't be negative");
        check(m.jump_height.is_finite() && (0.0..=10.0).contains(&m.jump_height), "jump height must be 0–10 m");
        check(m.jump_height == 0.0 || (0.1..=5.0).contains(&m.jump_air_s), "a jump needs an airtime of 0.1–5 s");
        // Targets must fit in the room.
        let half_w = self.arena.size[0] / 2.0;
        check(sp.distance.1 + t.radius < half_w.max(self.arena.size[2] / 2.0), "spawn distance reaches past the walls");
        check(
            m.kind != MovementKind::Curve
                || (finite_range(m.turn_deg_s)
                    && m.turn_deg_s.0 >= 0.0
                    && m.turn_deg_s.1 > 0.0
                    && m.turn_deg_s.1 <= 720.0),
            "curve movement needs a turn rate of 0–720 °/s",
        );
        // Targets must fit between floor and ceiling too.
        let half_height = if t.shape != Shape::Sphere { t.radius.max(t.height / 2.0) } else { t.radius };
        check(
            2.0 * half_height < self.arena.size[1],
            "targets must fit between floor and ceiling (the room is lower than a target is tall)",
        );
        let resolved = self.adaptive.resolve(self);
        p.extend(self.adaptive.problems(&resolved));
        if resolved.knob == crate::adaptive::Knob::TargetSpeed && m.kind == MovementKind::Static {
            p.push("adaptive target speed needs moving targets".into());
        }
        // Every number must be a real number. TOML reads `inf` and `nan`, and not every
        // check above catches them (scoring has no limits at all).
        if let Ok(doc) = toml::Value::try_from(self) {
            let mut bad = Vec::new();
            non_finite(&doc, "", &mut bad);
            let more = bad.len().saturating_sub(5);
            for path in bad.into_iter().take(5) {
                p.push(format!("{path} must be a number, not inf or nan"));
            }
            if more > 0 {
                p.push(format!("and {more} more values that aren't numbers"));
            }
        }
        let knob = self.adaptive.knob;
        if self.weapon.kind == WeaponKind::Hitscan
            && matches!(knob, crate::adaptive::Knob::TargetSpeed | crate::adaptive::Knob::Radius)
            && self.targets.lifetime_s <= 0.0
        {
            p.push(format!(
                "with a hitscan weapon, adaptive {} needs a target lifetime: a target counts as a success when it's killed before it times out",
                knob.label().to_lowercase()
            ));
        }
        p
    }
}

/// Collects the dotted paths of every `inf` or `nan` in a TOML document.
fn non_finite(v: &toml::Value, path: &str, out: &mut Vec<String>) {
    match v {
        toml::Value::Float(f) if !f.is_finite() => out.push(path.to_string()),
        toml::Value::Table(t) => {
            for (k, v) in t {
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                non_finite(v, &p, out);
            }
        }
        toml::Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                non_finite(v, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
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

    /// Hitscan with an adaptive target-speed knob needs a lifetime: without one a
    /// careful player never fails, so the rating would climb forever.
    #[test]
    fn hitscan_target_speed_needs_a_lifetime() {
        let mut s = Scenario::default();
        s.targets.movement.kind = MovementKind::Strafe;
        s.adaptive.knob = crate::adaptive::Knob::TargetSpeed;
        assert!(s.problems().iter().any(|p| p.contains("lifetime")), "{:?}", s.problems());
        s.targets.lifetime_s = 1.5;
        assert!(s.problems().is_empty(), "{:?}", s.problems());
    }

    /// Same for target size: a careful player who takes their time never misses.
    #[test]
    fn hitscan_target_size_needs_a_lifetime() {
        let mut s = Scenario::default();
        s.adaptive.knob = crate::adaptive::Knob::Radius;
        assert!(s.problems().iter().any(|p| p.contains("target size needs a target lifetime")), "{:?}", s.problems());
        s.targets.lifetime_s = 1.5;
        assert!(s.problems().is_empty(), "{:?}", s.problems());
    }

    #[test]
    fn curve_movement_needs_a_turn_rate() {
        let mut s = Scenario::default();
        s.targets.movement.kind = MovementKind::Curve;
        assert!(s.problems().iter().any(|p| p.contains("turn rate")), "{:?}", s.problems());
        s.targets.movement.turn_deg_s = Range(20.0, 60.0);
        assert!(s.problems().is_empty(), "{:?}", s.problems());
    }

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

    /// Bug (plan K6b): the fingerprint covered only a scenario's stored settings. When the
    /// sim began to play jumps, jetpacks and fliers by a jump key, a scenario with such
    /// bots played differently but was stored the same, so scores from before the change
    /// counted as the same version. A scenario whose bots fly or press the jump key now
    /// has a mark for the rule in its fingerprint, and every other scenario keeps the
    /// fingerprint its settings give.
    #[test]
    fn a_rule_change_restarts_only_the_scenarios_that_use_the_rule() {
        use crate::motion::{Dodge, Flight, Motion};
        use crate::world::{World, WorldKind};
        let with = |world: World| Scenario { world: Some(world), ..Scenario::default() };
        let of_settings = |s: &Scenario| fnv(&s.settings_text());
        let jumper = Motion { dodges: vec![Dodge { jump_chance: 0.5, ..Dodge::default() }], ..Motion::default() };
        let flier = Motion { flight: Some(Flight::default()), ..Motion::default() };
        let walker = Motion { max_speed: 3.0, dodges: vec![Dodge::default()], ..Motion::default() };
        for m in [jumper.clone(), flier] {
            let s = with(World { motion: Some(m), ..World::default() });
            assert_ne!(s.fingerprint(), of_settings(&s));
        }
        // A second kind that jumps counts too.
        let second = WorldKind { motion: Some(jumper), ..WorldKind::default() };
        let s = with(World { motion: Some(walker.clone()), kinds: vec![second], ..World::default() });
        assert_ne!(s.fingerprint(), of_settings(&s));
        // Bots that walk or stand still, and scenarios without a world, are untouched.
        for s in [with(World { motion: Some(walker), ..World::default() }), with(World::default()), Scenario::default()]
        {
            assert_eq!(s.fingerprint(), of_settings(&s));
        }
    }

    /// Bug: imported bots healed continuously between shots; they now heal in steps, and scenarios
    /// whose bots regenerate under a hitscan weapon restart their bests with a second mark.
    #[test]
    fn healing_in_steps_marks_only_imported_hitscan_scenarios_with_regen() {
        use crate::motion::{Dodge, Motion};
        use crate::world::{World, WorldKind};
        let of_settings = |s: &Scenario| fnv(&s.settings_text());
        let world = |w: World, regen: f32| {
            let mut s = Scenario { world: Some(w), ..Scenario::default() };
            s.targets.regen_per_s = regen;
            s
        };
        let regen_kind = |r: f32| WorldKind { regen_per_s: r, ..WorldKind::default() };
        for s in [
            world(World::default(), 10.0),
            world(World { kinds: vec![regen_kind(0.0), regen_kind(5.0)], ..World::default() }, 0.0),
        ] {
            assert_ne!(s.fingerprint(), of_settings(&s));
        }
        let mut beam = world(World::default(), 10.0);
        beam.weapon.kind = WeaponKind::Beam;
        for s in [
            world(World::default(), 0.0),
            world(World::default(), -10.0),
            Scenario { world: None, ..world(World::default(), 10.0) },
            beam,
        ] {
            assert_eq!(s.fingerprint(), of_settings(&s));
        }
        let jumper = Motion { dodges: vec![Dodge { jump_chance: 0.5, ..Dodge::default() }], ..Motion::default() };
        let jumps = world(World { motion: Some(jumper.clone()), ..World::default() }, 0.0);
        assert_eq!(jumps.fingerprint(), fnv(&(jumps.settings_text() + "\n# rules: jump key")));
        let both = world(World { motion: Some(jumper), ..World::default() }, 10.0);
        assert_eq!(both.fingerprint(), fnv(&(both.settings_text() + "\n# rules: jump key\n# rules: heals in steps")));
    }

    /// Bug: imported bots used their abilities at the wrong times (every tick was a try, charges
    /// and delays counted from the end of the ability). Scenarios whose world has a bot with an
    /// ability restart their bests with a third mark; bots without abilities keep their fingerprints.
    #[test]
    fn ability_tries_mark_only_scenarios_with_a_bot_that_has_an_ability() {
        use crate::motion::{Abilities, Ability, Dodge, Motion};
        use crate::world::{World, WorldKind};
        let of_settings = |s: &Scenario| fnv(&s.settings_text());
        let with_world = |w: World| Scenario { world: Some(w), ..Scenario::default() };
        let ability = Abilities { list: vec![Ability::default()], ..Abilities::default() };
        let dasher = Motion { abilities: Some(ability), ..Motion::default() };
        let s = with_world(World { motion: Some(dasher.clone()), ..World::default() });
        assert_eq!(
            s.fingerprint(),
            fnv(&(s.settings_text() + "\n# rules: ability tries\n# rules: abilities over respawns"))
        );
        // A second kind with an ability counts too.
        let second = WorldKind { motion: Some(dasher.clone()), ..WorldKind::default() };
        let s = with_world(World { kinds: vec![second], ..World::default() });
        assert_eq!(
            s.fingerprint(),
            fnv(&(s.settings_text() + "\n# rules: ability tries\n# rules: abilities over respawns"))
        );
        // No ability: an empty list, none at all, a bot that only walks, no world.
        let empty = Motion { abilities: Some(Abilities::default()), ..Motion::default() };
        let walker = Motion { max_speed: 3.0, dodges: vec![Dodge::default()], ..Motion::default() };
        for s in [
            with_world(World { motion: Some(empty), ..World::default() }),
            with_world(World { motion: Some(walker), ..World::default() }),
            with_world(World::default()),
            Scenario::default(),
        ] {
            assert_eq!(s.fingerprint(), of_settings(&s));
        }
        // Next to the other marks, after them.
        let mut both = with_world(World { motion: Some(dasher), ..World::default() });
        both.targets.regen_per_s = 10.0;
        assert_eq!(
            both.fingerprint(),
            fnv(&(both.settings_text()
                + "\n# rules: heals in steps\n# rules: ability tries\n# rules: abilities over respawns"))
        );
    }

    /// Bug: easy and hard were only compared when both were written, so a file that
    /// set one of them (or set both under the automatic knob) passed, and the rating
    /// panicked in `clamp` once the defaults filled in the other.
    #[test]
    fn adaptive_limits_are_checked_after_defaults_fill_in() {
        use crate::adaptive::Knob;
        let mut s = Scenario::default();
        // Automatic resolves to target lifetime here: 0.3 s is harder than 2 s.
        s.adaptive.easy = 0.3;
        s.adaptive.hard = 2.0;
        assert!(s.problems().iter().any(|p| p.contains("easier than hard")), "{:?}", s.problems());
        // Only easy written; the default hard is 0.25 s.
        let mut s = Scenario::default();
        s.adaptive.knob = Knob::Lifetime;
        s.adaptive.easy = 0.2;
        assert!(s.problems().iter().any(|p| p.contains("easier than hard")), "{:?}", s.problems());
        // Only hard written on a beam scenario; the default easy is 3 °/s.
        let mut s = Scenario::default();
        s.weapon.kind = WeaponKind::Beam;
        s.targets.movement.kind = MovementKind::Strafe;
        s.adaptive.knob = Knob::TargetSpeed;
        s.adaptive.hard = 2.0;
        assert!(s.problems().iter().any(|p| p.contains("easier than hard")), "{:?}", s.problems());
    }

    /// Bug: only the horizontal fit was checked, so a target taller than the room
    /// passed and the sim panicked placing it.
    #[test]
    fn targets_must_fit_between_floor_and_ceiling() {
        let mut s = Scenario::default();
        s.arena.size = [40.0, 2.0, 40.0];
        s.arena.eye_height = 1.0;
        s.targets.radius = 1.5;
        assert!(s.problems().iter().any(|p| p.contains("floor and ceiling")), "{:?}", s.problems());
        s.targets.radius = 0.5;
        assert!(!s.problems().iter().any(|p| p.contains("floor and ceiling")), "{:?}", s.problems());
    }

    /// Bug: `inf` passed the checks for damage, health, delays and scoring, and NaN
    /// passed scoring, which made every score NaN.
    #[test]
    fn non_finite_numbers_are_refused() {
        type Set = fn(&mut Scenario);
        let cases: [(&str, Set); 6] = [
            ("damage", |s| s.weapon.damage = f32::INFINITY),
            ("health", |s| s.targets.health = f32::INFINITY),
            ("respawn", |s| s.targets.respawn_delay_s = f32::INFINITY),
            ("lifetime", |s| s.targets.lifetime_s = f32::INFINITY),
            ("per kill", |s| s.scoring.per_kill = f32::NAN),
            ("per miss", |s| s.scoring.per_miss = f32::NEG_INFINITY),
        ];
        for (what, set) in cases {
            let mut s = Scenario::default();
            set(&mut s);
            assert!(!s.problems().is_empty(), "{what} went through");
        }
    }

    /// Every number field the editor offers, as `(path, min, max)`.
    fn editor_limits() -> Vec<(&'static str, f64, f64)> {
        use crate::spec::{FIELDS, Kind};
        FIELDS
            .iter()
            .filter_map(|f| match f.kind {
                Kind::Number { min, max, .. } | Kind::Range { min, max, .. } | Kind::Size3 { min, max, .. } => {
                    Some((f.path, f64::from(min), f64::from(max)))
                }
                Kind::Integer { min, max } => Some((f.path, f64::from(min), f64::from(max))),
                _ => None,
            })
            .collect()
    }

    fn numbers_at(s: &Scenario, path: &str) -> Vec<f64> {
        let doc = toml::Value::try_from(s).unwrap();
        let v = path.split('.').try_fold(&doc, |v, k| v.get(k)).unwrap_or_else(|| panic!("{path}"));
        let num = |v: &toml::Value| v.as_float().or(v.as_integer().map(|i| i as f64)).unwrap();
        match v {
            toml::Value::Array(a) => a.iter().map(num).collect(),
            v => vec![num(v)],
        }
    }

    fn with_numbers(s: &Scenario, path: &str, n: &[f64]) -> Scenario {
        let mut doc = toml::Value::try_from(s).unwrap();
        let (parent, last) = path.rsplit_once('.').unwrap_or(("", path));
        let table = parent.split('.').filter(|k| !k.is_empty()).fold(&mut doc, |v, k| v.get_mut(k).unwrap());
        let is_int = table[last].is_integer();
        let val = |x: f64| if is_int { toml::Value::Integer(x as i64) } else { toml::Value::Float(x) };
        table[last] =
            if table[last].is_array() { toml::Value::Array(n.iter().map(|&x| val(x)).collect()) } else { val(n[0]) };
        doc.try_into().unwrap()
    }

    /// Bug: the editor's limits and the engine's were written separately and
    /// disagreed. Any value the editor offers must load. (Adaptive easy, hard and
    /// start, room size and eye height are left out: whether they fit depends on
    /// the other values.)
    #[test]
    fn the_engine_accepts_every_value_the_editor_offers() {
        let mut base = Scenario::default();
        base.arena.size = [1000.0, 100.0, 1000.0];
        // Room to move forward and back, and an airtime for any jump.
        base.targets.movement.jump_air_s = 0.5;
        base.targets.spawn.distance = Range(5.0, 20.0);
        assert!(base.problems().is_empty(), "{:?}", base.problems());
        for (path, min, max) in editor_limits() {
            if matches!(path, "adaptive.easy" | "adaptive.hard" | "adaptive.start" | "arena.size" | "arena.eye_height")
            {
                continue;
            }
            let n = numbers_at(&base, path).len();
            for v in [min, max] {
                let s = with_numbers(&base, path, &vec![v; n]);
                assert!(s.problems().is_empty(), "{path} = {v}: {:?}", s.problems());
            }
        }
    }

    /// Bug: eight built-in tracking scenarios set a health of 1,000,000, above the
    /// editor's 100,000, so editing a copy showed a value the editor called invalid.
    #[test]
    fn built_in_scenarios_stay_inside_the_editor_limits() {
        for e in crate::library::builtins() {
            let s = e.scenario.unwrap();
            for (path, min, max) in editor_limits() {
                for v in numbers_at(&s, path) {
                    assert!((min..=max).contains(&v), "{}: {path} = {v} is outside {min}–{max}", e.id);
                }
            }
        }
    }

    /// Adaptive target speed scales how fast targets move, so still targets would
    /// never change.
    #[test]
    fn adaptive_target_speed_needs_moving_targets() {
        let mut s = Scenario::default();
        s.weapon.kind = WeaponKind::Beam;
        s.adaptive.knob = crate::adaptive::Knob::TargetSpeed;
        assert!(s.problems().iter().any(|p| p.contains("moving targets")), "{:?}", s.problems());
        s.targets.movement.kind = MovementKind::Strafe;
        assert!(s.problems().is_empty(), "{:?}", s.problems());
    }

    /// Ratings are keyed on the fingerprint, so a change to it restarts every
    /// rating. This pins the default scenario's value: if it changes, make sure
    /// that's intended and say so in the guide.
    #[test]
    fn fingerprint_is_stable() {
        assert_eq!(Scenario::default().fingerprint(), Scenario::default().fingerprint());
        assert_eq!(Scenario::default().fingerprint(), PINNED_FINGERPRINT);
    }
    const PINNED_FINGERPRINT: &str = "2aab4223302f7e95";

    /// A scenario whose one kind of bot has this motion.
    fn with_motion(motion: crate::motion::Motion) -> Scenario {
        let spawns = vec![crate::world::WorldSpawn { pos: [0.0, 0.0, -10.0], ..Default::default() }];
        let world = crate::world::World { spawns, motion: Some(motion), ..Default::default() };
        Scenario { world: Some(world), ..Scenario::default() }
    }

    fn a_movement_ability() -> crate::motion::Ability {
        crate::motion::Ability { speed: 8.0, duration_s: 0.2, ..Default::default() }
    }

    fn abilities_of(list: Vec<crate::motion::Ability>) -> crate::motion::Abilities {
        crate::motion::Abilities { press_every_s: 0.1, chance: 1.0, window_s: [1.0, 1.0], list, ..Default::default() }
    }

    /// Plan K8a: the fuse's fields (`fuse` on an ability, `stays_put` on a motion) are left out of the stored
    /// settings when unused, so no scenario that existed before has a new fingerprint. A scenario that uses
    /// them has them in its settings and a fingerprint of its own.
    #[test]
    fn a_scenario_without_a_fuse_stores_no_fuse_fields() {
        use crate::motion::{Fuse, Motion};
        let plain =
            with_motion(Motion { abilities: Some(abilities_of(vec![a_movement_ability()])), ..Motion::default() });
        let text = plain.settings_text();
        assert!(text.contains("duration_s"), "the ability is stored: {text}");
        assert!(!text.contains("fuse") && !text.contains("stays_put"), "{text}");
        let bare = toml::to_string_pretty(&Motion::default()).unwrap();
        assert!(!bare.contains("fuse") && !bare.contains("stays_put"), "{bare}");

        let fuse = Fuse { shot_every_s: 0.7, burn_s: 0.04, damage: 20.0 };
        let ability = crate::motion::Ability { fuse: Some(fuse), ..Default::default() };
        let used =
            with_motion(Motion { stays_put: true, abilities: Some(abilities_of(vec![ability])), ..Motion::default() });
        let text = used.settings_text();
        assert!(text.contains("stays_put") && text.contains("shot_every_s") && text.contains("burn_s"), "{text}");
        assert_ne!(used.fingerprint(), plain.fingerprint());
    }

    /// Plan K8a: a fuse whose time or damage is zero, negative, infinite or not a number is a problem, and a
    /// sound one is not.
    #[test]
    fn a_fuse_with_a_bad_time_or_damage_is_a_problem() {
        use crate::motion::{Fuse, Motion};
        let make = |fuse: Fuse| {
            let ability = crate::motion::Ability { fuse: Some(fuse), ..Default::default() };
            with_motion(Motion { stays_put: true, abilities: Some(abilities_of(vec![ability])), ..Motion::default() })
        };
        let sound = Fuse { shot_every_s: 0.7, burn_s: 0.04, damage: 20.0 };
        assert!(make(sound.clone()).problems().is_empty(), "{:?}", make(sound.clone()).problems());
        let bad = f32::NAN;
        for (fuse, what) in [
            (Fuse { shot_every_s: 0.0, ..sound.clone() }, "a fuse time"),
            (Fuse { shot_every_s: -1.0, ..sound.clone() }, "a fuse time"),
            (Fuse { burn_s: 0.0, ..sound.clone() }, "a fuse time"),
            (Fuse { burn_s: -0.1, ..sound.clone() }, "a fuse time"),
            (Fuse { burn_s: bad, ..sound.clone() }, "a fuse time"),
            (Fuse { shot_every_s: f32::INFINITY, ..sound.clone() }, "a fuse time"),
            (Fuse { shot_every_s: 7200.0, ..sound.clone() }, "a fuse time"),
            (Fuse { damage: 0.0, ..sound.clone() }, "a fuse damage"),
            (Fuse { damage: -5.0, ..sound.clone() }, "a fuse damage"),
            (Fuse { damage: bad, ..sound.clone() }, "a fuse damage"),
        ] {
            let problems = make(fuse.clone()).problems();
            assert!(problems.iter().any(|p| p.contains(what)), "{fuse:?}: {problems:?}");
        }
    }

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#ff0000"), Some([1.0, 0.0, 0.0]));
        assert_eq!(parse_color("ff0000"), None);
        assert_eq!(parse_color("#ff00"), None);
    }
}
