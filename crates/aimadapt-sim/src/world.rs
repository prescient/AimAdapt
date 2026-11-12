//! A KovaaK's scenario's world, for scenarios imported from `.sce` files (see the
//! `aimadapt-sce` crate). A [`Scenario`](crate::Scenario) with a world plays on the
//! world's map: targets appear at its spawn points instead of in the scenario's
//! spawn cone, and the engine draws its geometry instead of the room.
//!
//! Everything is in the sim's frame: meters, the player's eye at the origin looking
//! along -Z, +Y up and +X right. Imported scenarios aren't written by hand, so none
//! of this is in the scenario editor or SCENARIOS.md.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::motion::Motion;
use crate::rng::Rng;
use crate::scenario::Shape;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct World {
    /// Where the scenario came from, e.g. "VT 1w4ts Novice S5.sce", and a hash of
    /// the file, so history stays tied to one version of it.
    pub source: String,
    pub source_hash: String,
    /// Where targets can appear, each picked in proportion to its weight.
    pub spawns: Vec<WorldSpawn>,
    /// A random offset added to the spawn point: each axis between these two values
    /// (in either order), along the spawn point's own axes, or along `offset_axes`
    /// when the world has them.
    pub offset_min: [f32; 3],
    pub offset_max: [f32; 3],
    /// One frame for every spawn point's offset, where the offsets' x, y and z point,
    /// used in place of each point's own axes. KovaaK's never turns an offset with its
    /// spawn point, so an imported world has it when its points are turned in a way
    /// that would move the targets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_axes: Option<[[f32; 3]; 3]>,
    /// No target appears closer than this to another one, in meters.
    pub blocked_radius: f32,
    /// Inverted (KovaaK's `InvertBlockedSpawn` on the bots): a new target appears
    /// within `blocked_radius` of one already up, so they come in clusters.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cluster: bool,
    /// The score is multiplied by accuracy, or by its square root.
    pub accuracy_mult: AccuracyMult,
    /// Bots take hits, and their damage counts, but never go down (KovaaK's
    /// `InvincibleBots`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invincible: bool,
    /// The horizontal field of view is kept within this range, in degrees.
    pub fov_range: Option<[f32; 2]>,
    /// Map geometry to draw.
    pub meshes: Vec<WorldMesh>,
    /// Solid boxes (the map's `Cube` walls). A target that would appear partly
    /// inside one is pushed out to its surface, as KovaaK's does.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<WorldBox>,
    /// Where the player's profile lets targets appear, around the player and their
    /// view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_block: Option<PlayerBlock>,
    /// How the bots move; none for bots that stand still.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<Motion>,
    /// More than one kind of bot: the kinds, and for each bot slot (one per target
    /// up at a time) the kinds that can fill it, by weight. Empty when every target is
    /// the scenario's own; then `motion` is how they move.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<WorldKind>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Vec<(u32, f32)>>,
    /// For each slot, the rotation it takes its kinds from in turn (KovaaK's
    /// `Randomized=false`), or none to pick by weight. Slots that name the same
    /// rotation take turns from it together. Empty when every slot picks by weight.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rotations: Vec<Option<u32>>,
    /// Helper bots that push the targets around (KovaaK's melee abilities that do no
    /// damage). They aren't drawn, shot or scored.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pushers: Vec<Pusher>,
    /// The rate of the scenario's whole clock (KovaaK's `Timescale`): bots, the weapon,
    /// delays and the run timer all run at this many game seconds per real second, so
    /// 0.5 is slow motion and the run lasts twice as long. The countdown stays in real
    /// time.
    #[serde(skip_serializing_if = "is_one")]
    pub time_scale: f32,
    /// Points for each game second left on the clock (KovaaK's `ScorePerTime`). The
    /// score always counts the time left now, so it is 0 after a timeout and goes down
    /// as the run goes on.
    #[serde(skip_serializing_if = "is_zero")]
    pub time_score: f32,
    /// The score is multiplied by damage done over damage possible (KovaaK's
    /// `ScoreMultDamageEfficiency`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub damage_efficiency: bool,
}

/// Whether a score rate is unset, so it stays out of the file (and the fingerprint).
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// Whether a time scale is the default, so it stays out of the file (and the fingerprint).
fn is_one(v: &f32) -> bool {
    *v == 1.0
}

/// The slowest and fastest time scale a world can have.
pub const TIME_SCALE_RANGE: (f32, f32) = (0.1, 10.0);

impl Default for World {
    fn default() -> Self {
        World {
            source: String::new(),
            source_hash: String::new(),
            spawns: Vec::new(),
            offset_min: [0.0; 3],
            offset_max: [0.0; 3],
            offset_axes: None,
            blocked_radius: 0.0,
            cluster: false,
            accuracy_mult: AccuracyMult::default(),
            invincible: false,
            fov_range: None,
            meshes: Vec::new(),
            walls: Vec::new(),
            player_block: None,
            motion: None,
            kinds: Vec::new(),
            slots: Vec::new(),
            rotations: Vec::new(),
            pushers: Vec::new(),
            time_scale: 1.0,
            time_score: 0.0,
            damage_efficiency: false,
        }
    }
}

/// How a chasing pusher flies, as measured in KovaaK's. Each frame its velocity
/// loses the share `drag` times the frame time, gains `accel` times the frame time
/// along the line to its target, is cut to `max_speed`, and loses the part that
/// points into its plane. So it trails a target that moves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChaseFlight {
    /// m/s².
    pub accel: f32,
    /// m/s.
    pub max_speed: f32,
    /// Per second.
    pub drag: f32,
}

/// A helper bot: it kicks every target in reach on a fixed schedule from the start
/// of the run (or from `start_s`), whether anything is in reach or not. Its reach is
/// a sphere of `radius` whose centre is `radius` ahead of it along `facing`, or
/// along the line to its target for a chaser with a flight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pusher {
    /// Meters, in the world's frame: where a pinned pusher is, a point on the plane
    /// a chasing one slides on, and where a chaser with a flight starts.
    pub pos: [f32; 3],
    /// A unit vector: the way a pinned pusher faces, or the plane's normal pointing
    /// toward the targets for a chasing one.
    pub facing: [f32; 3],
    /// It flies at the nearest target and is stopped by the plane it slides on.
    /// Without a `flight` it arrives at once, so it is always at the point of the
    /// plane nearest the target.
    pub chases: bool,
    /// How a chaser flies. Absent, it arrives at once (worlds written by hand, and
    /// scenarios stored before flights). Only valid on a pusher that chases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flight: Option<ChaseFlight>,
    /// Seconds from the start of the run before the first kick; a chaser flies
    /// during the wait.
    #[serde(skip_serializing_if = "is_zero")]
    pub start_s: f32,
    /// The reach's radius, in meters.
    pub radius: f32,
    /// Seconds between kicks.
    pub every_s: f32,
    /// What a kick adds to the target's speed, level and away from the pusher, in m/s.
    pub push: f32,
    /// What a kick adds to the target's upward speed in m/s; negative pushes down.
    pub lift: f32,
    /// Only kicks targets it has a clear line to past the map's solid walls.
    pub needs_sight: bool,
}

impl Default for Pusher {
    fn default() -> Self {
        Pusher {
            pos: [0.0; 3],
            facing: [0.0, 0.0, -1.0],
            chases: false,
            flight: None,
            start_s: 0.0,
            radius: 0.01,
            every_s: 0.02,
            push: 0.0,
            lift: 0.0,
            needs_sight: false,
        }
    }
}

/// The fastest a kick may add to a target's speed, in m/s.
const MAX_KICK: f32 = 1000.0;
/// The shortest time between kicks.
const MIN_EVERY_S: f32 = 0.005;
/// The most a chaser may accelerate (m/s²), the fastest it may fly (m/s) and the
/// most drag it may have (per second). Real chasers are far under all three.
const MAX_ACCEL: f32 = 1_000_000.0;
const MAX_FLIGHT_SPEED: f32 = 10_000.0;
const MAX_DRAG: f32 = 1000.0;
/// The longest a pusher may wait before its first kick, in seconds.
const MAX_START_S: f32 = 3600.0;

impl Pusher {
    /// Problems with the values, in plain words.
    fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        let flight = self.flight.as_ref().map(|f| [f.accel, f.max_speed, f.drag]).unwrap_or_default();
        let mut numbers = self
            .pos
            .iter()
            .chain(&self.facing)
            .chain([&self.radius, &self.every_s, &self.push, &self.lift, &self.start_s])
            .chain(&flight);
        if numbers.any(|v| !v.is_finite()) {
            p.push("a helper bot has a value that isn't a number".into());
            return p;
        }
        p.extend(self.timing_problems());
        if self.radius <= 0.0 {
            p.push("a helper bot's reach needs a radius above 0".into());
        }
        if self.every_s < MIN_EVERY_S {
            p.push(format!("a helper bot can't act more than every {MIN_EVERY_S} s"));
        }
        if (Vec3::from_array(self.facing).length() - 1.0).abs() > 1e-3 {
            p.push("a helper bot's facing must be a unit vector".into());
        }
        if self.push.abs() > MAX_KICK || self.lift.abs() > MAX_KICK {
            p.push(format!("a helper bot's push can't be above {MAX_KICK} m/s"));
        }
        p
    }

    /// Problems with the flight and the start delay. Numbers are already known to be
    /// finite.
    fn timing_problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if let Some(f) = &self.flight {
            if !self.chases {
                p.push("a helper bot that doesn't chase can't have a flight".into());
            }
            for (name, v, max) in [
                ("acceleration", f.accel, MAX_ACCEL),
                ("top speed", f.max_speed, MAX_FLIGHT_SPEED),
                ("drag", f.drag, MAX_DRAG),
            ] {
                if v <= 0.0 || v > max {
                    p.push(format!("a helper bot's flight needs a {name} above 0 and at most {max}"));
                }
            }
        }
        if !(0.0..=MAX_START_S).contains(&self.start_s) {
            p.push(format!("a helper bot's start delay must be 0 to {MAX_START_S} s"));
        }
        p
    }

    /// Whether a target's body, a vertical segment of half-length `half` around `mid`
    /// swollen by `radius`, overlaps the reach when the pusher is at `at`.
    pub fn reaches(&self, at: Vec3, mid: Vec3, radius: f32, half: f32) -> bool {
        self.reaches_along(at, Vec3::from_array(self.facing), mid, radius, half)
    }

    /// The same, with the reach's centre `radius` from `at` along the unit vector `dir`
    /// instead of along `facing` (a chaser with a flight reaches along the line to its
    /// target).
    pub fn reaches_along(&self, at: Vec3, dir: Vec3, mid: Vec3, radius: f32, half: f32) -> bool {
        let centre = at + dir * self.radius;
        let nearest = Vec3::new(mid.x, centre.y.clamp(mid.y - half, mid.y + half), mid.z);
        nearest.distance(centre) <= self.radius + radius
    }
}

/// One kind of bot in a scenario with several: its hitbox, health, look and
/// movement, and the spawn points it may use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldKind {
    pub shape: Shape,
    pub radius: f32,
    pub height: f32,
    pub health: f32,
    pub color: String,
    pub regen_per_s: f32,
    pub regen_delay_s: f32,
    pub respawn_delay_s: f32,
    /// This kind takes hits but never goes down (its character's `InvincibleBots`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invincible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<Motion>,
    /// The spawn points it may use, as indices into the world's; empty for any.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spawns: Vec<u32>,
    /// How much higher (or, below 0, lower) than the first kind this kind starts above
    /// its spawn point, in meters: the difference of their start heights.
    #[serde(skip_serializing_if = "is_zero")]
    pub rise: f32,
    /// This kind's own spawn offsets, `[min, max]` as the world's `offset_min` and
    /// `offset_max`, when they differ from the world's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offsets: Option<[[f32; 3]; 2]>,
}

impl Default for WorldKind {
    fn default() -> Self {
        WorldKind {
            shape: Shape::Sphere,
            radius: 0.5,
            height: 1.0,
            health: 1.0,
            color: "#e05050".into(),
            regen_per_s: 0.0,
            regen_delay_s: 0.0,
            respawn_delay_s: 0.0,
            invincible: false,
            motion: None,
            spawns: Vec::new(),
            rise: 0.0,
            offsets: None,
        }
    }
}

/// A box: one corner and its three edges from there, in meters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldBox {
    pub corner: [f32; 3],
    pub edges: [[f32; 3]; 3],
}

/// KovaaK's `BlockedSpawnRadius`, `BlockSpawnFOV`, `BlockSpawnDistance` and
/// `InvertBlockedSpawn` on the player's profile: no target appears within `radius`
/// of the player, or within `fov_deg` (the whole cone) of where they're looking and
/// `distance` away (0 for any distance). Inverted, targets appear only there.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlayerBlock {
    pub radius: f32,
    pub fov_deg: f32,
    pub distance: f32,
    pub invert: bool,
}

impl PlayerBlock {
    /// Whether a target may appear at `pos` while the player looks along `aim`.
    pub fn allows(&self, pos: Vec3, aim: Vec3) -> bool {
        let d = pos.length();
        let in_radius = self.radius > 0.0 && d < self.radius;
        let in_view = self.fov_deg > 0.0
            && (self.distance <= 0.0 || d <= self.distance)
            && aim.angle_between(pos).to_degrees() < self.fov_deg / 2.0;
        (in_radius || in_view) == self.invert
    }
}

impl WorldBox {
    /// `pos` moved out of the box so a sphere of `radius` there no longer overlaps
    /// it: to the nearest face if its center is inside, else straight away from the
    /// box. Unchanged if they don't touch.
    pub fn push_out(&self, pos: Vec3, radius: f32) -> Vec3 {
        let corner = Vec3::from_array(self.corner);
        let edges = self.edges.map(Vec3::from_array);
        let lens = edges.map(Vec3::length);
        if lens.iter().any(|l| *l < 1e-6) {
            return pos;
        }
        let axes = [0, 1, 2].map(|k| edges[k] / lens[k]);
        let local = [0, 1, 2].map(|k| (pos - corner).dot(axes[k]));
        let inside = (0..3).all(|k| (0.0..=lens[k]).contains(&local[k]));
        if inside {
            // The face it's least deep behind: (depth, axis, outward sign).
            let mut best = (f32::MAX, 0, 1.0);
            for k in 0..3 {
                if local[k] < best.0 {
                    best = (local[k], k, -1.0);
                }
                if lens[k] - local[k] < best.0 {
                    best = (lens[k] - local[k], k, 1.0);
                }
            }
            let (depth, k, sign) = best;
            return pos + axes[k] * sign * (depth + radius);
        }
        let closest = corner + (0..3).map(|k| axes[k] * local[k].clamp(0.0, lens[k])).sum::<Vec3>();
        let away = pos - closest;
        let d = away.length();
        if d >= radius || d < 1e-6 { pos } else { closest + away / d * radius }
    }

    fn contains(&self, p: Vec3) -> bool {
        let corner = Vec3::from_array(self.corner);
        self.edges.iter().all(|e| {
            let e = Vec3::from_array(*e);
            let t = (p - corner).dot(e) / e.length_squared().max(1e-12);
            (0.0..=1.0).contains(&t)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldSpawn {
    pub pos: [f32; 3],
    pub weight: f32,
    /// Where the spawn point's own x, y and z axes point (it faces along -z). The
    /// offset runs along them unless the world has `offset_axes`; a bot that doesn't
    /// face the player strafes along the first one either way.
    pub axes: [[f32; 3]; 3],
    /// A spawn volume's three edges from `pos`, one of its corners, in meters: a
    /// target appears anywhere in the box they span. All zero for a spawn point.
    #[serde(skip_serializing_if = "is_point")]
    pub edges: [[f32; 3]; 3],
}

fn is_point(edges: &[[f32; 3]; 3]) -> bool {
    edges.iter().flatten().all(|v| *v == 0.0)
}

impl Default for WorldSpawn {
    fn default() -> Self {
        WorldSpawn {
            pos: [0.0; 3],
            weight: 1.0,
            axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            edges: [[0.0; 3]; 3],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccuracyMult {
    #[default]
    None,
    Linear,
    Sqrt,
}

impl AccuracyMult {
    /// The factor a score is multiplied by at this accuracy (0–1). No shots counts
    /// as full accuracy.
    pub fn factor(self, accuracy: Option<f32>) -> f32 {
        let a = accuracy.unwrap_or(1.0).clamp(0.0, 1.0);
        match self {
            AccuracyMult::None => 1.0,
            AccuracyMult::Linear => a,
            AccuracyMult::Sqrt => a.sqrt(),
        }
    }
}

/// Triangles with one color, e.g. a wall. Walls are solid: shots stop at them and
/// moving bots bump into them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldMesh {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub color: String,
    /// Drawn only: shots and bots pass through (a shape whose exact form is a guess).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub decor: bool,
    /// An invisible wall that keeps characters in (a Reflex clip brush): not drawn,
    /// and shots pass through, but bots bump into it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub clip: bool,
}

impl World {
    /// How each kind of bot moves, for the kinds that do.
    pub fn motions(&self) -> impl Iterator<Item = &Motion> {
        self.motion.iter().chain(self.kinds.iter().filter_map(|k| k.motion.as_ref()))
    }

    /// Problems with the values, in plain words.
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.spawns.is_empty() {
            p.push("the map has no spawn points for targets".into());
        }
        if self.spawns.iter().all(|s| s.weight <= 0.0) {
            p.push("no spawn point has a weight above 0".into());
        }
        if self.blocked_radius.is_nan() || self.blocked_radius < 0.0 {
            p.push("blocked spawn radius can't be negative".into());
        }
        for m in &self.meshes {
            let n = m.positions.len() as u32;
            if m.triangles.iter().flatten().any(|i| *i >= n) {
                p.push("a map mesh refers to a vertex it doesn't have".into());
                break;
            }
        }
        let kinds = self.kinds.len() as u32;
        if self.slots.iter().flatten().any(|(k, _)| *k >= kinds) || self.slots.iter().any(Vec::is_empty) {
            p.push("a bot slot refers to a kind of bot the world doesn't have".into());
        }
        if !self.rotations.is_empty() && self.rotations.len() != self.slots.len() {
            p.push("bot slots and their rotations don't match up".into());
        }
        // Each rotation id indexes a counter, so a huge one would allocate gigabytes.
        if self.rotations.iter().flatten().any(|r| *r as usize >= self.slots.len()) {
            p.push("a bot slot names a rotation the world doesn't have".into());
        }
        if let Some(axes) = &self.offset_axes {
            if axes.iter().flatten().any(|v| !v.is_finite()) {
                p.push("the offset frame has a value that isn't a number".into());
            } else if axes.iter().any(|a| Vec3::from_array(*a).length_squared() <= 1e-12) {
                p.push("the offset frame has an axis with no length".into());
            }
        }
        let motions = self.motion.iter().chain(self.kinds.iter().filter_map(|k| k.motion.as_ref()));
        for problem in motions.filter_map(|m| m.abilities.as_ref()).flat_map(|a| a.problems()) {
            if !p.contains(&problem) {
                p.push(problem);
            }
        }
        let spawns = self.spawns.len() as u32;
        for k in &self.kinds {
            if k.radius.is_nan() || k.radius <= 0.0 || k.health.is_nan() || k.health <= 0.0 {
                p.push("a kind of bot needs a size and health above 0".into());
                break;
            }
            if k.spawns.iter().any(|s| *s >= spawns) {
                p.push("a kind of bot refers to a spawn point the map doesn't have".into());
                break;
            }
        }
        for problem in self.pushers.iter().flat_map(Pusher::problems) {
            if !p.contains(&problem) {
                p.push(problem);
            }
        }
        let (lo, hi) = TIME_SCALE_RANGE;
        if !(lo..=hi).contains(&self.time_scale) {
            p.push(format!("time scale must be {lo}–{hi}"));
        }
        if !self.time_score.is_finite() {
            p.push("the score for time left must be a number".into());
        }
        if !self.pushers.is_empty() && !self.bots_move() {
            p.push("helper bots push nothing unless the bots can move".into());
        }
        p
    }

    /// Whether every kind of bot has a motion, so it can be pushed.
    fn bots_move(&self) -> bool {
        if self.kinds.is_empty() { self.motion.is_some() } else { self.kinds.iter().all(|k| k.motion.is_some()) }
    }

    /// The kind to fill bot slot `slot` with: the next in turn when the slot takes
    /// them in order (`turns` counts each rotation's picks so far), otherwise picked
    /// by weight; none when the world has one kind of bot.
    pub fn pick_kind(&self, rng: &mut Rng, slot: usize, turns: &mut Vec<u32>) -> Option<u32> {
        let choices = self.slots.get(slot)?;
        if let Some(r) = self.rotations.get(slot).copied().flatten().filter(|_| !choices.is_empty()) {
            let r = r as usize;
            if turns.len() <= r {
                turns.resize(r + 1, 0);
            }
            let k = choices[turns[r] as usize % choices.len()].0;
            turns[r] += 1;
            return Some(k);
        }
        let total: f32 = choices.iter().map(|(_, w)| w.max(0.0)).sum();
        let mut x = rng.next_f32() * total;
        for (k, w) in choices {
            if x < w.max(0.0) {
                return Some(*k);
            }
            x -= w.max(0.0);
        }
        choices.last().map(|(k, _)| *k)
    }

    /// How a target of `kind` (none for the world's one kind) moves, if it does.
    pub fn motion_of(&self, kind: Option<u32>) -> Option<&Motion> {
        match kind {
            Some(k) => self.kinds.get(k as usize).and_then(|k| k.motion.as_ref()),
            None => self.motion.as_ref(),
        }
    }

    /// Whether any of the bots bump into each other.
    pub fn bots_bump(&self) -> bool {
        self.motion.iter().chain(self.kinds.iter().filter_map(|k| k.motion.as_ref())).any(|m| m.bump_bots)
    }

    /// A spawn position for a target of `radius`: a weighted spawn point (or a
    /// random point in a spawn volume) plus a random offset, pushed out of walls,
    /// away from `others` and where the player's profile allows while they look
    /// along `aim`. After 40 tries it takes the first one that was apart from the
    /// others, or else the last one, as a crowded map would.
    pub fn spawn_pos(&self, rng: &mut Rng, others: &[Vec3], radius: f32, aim: Vec3) -> Vec3 {
        self.spawn_point(rng, others, radius, aim).0
    }

    /// [`World::spawn_pos`], with the right-hand direction of the spawn point it
    /// picked (where a bot that doesn't face the player strafes).
    pub fn spawn_point(&self, rng: &mut Rng, others: &[Vec3], radius: f32, aim: Vec3) -> (Vec3, Vec3) {
        self.spawn_point_in(rng, others, radius, aim, None)
    }

    /// [`World::spawn_point`] for a bot of `kind` (none for the world's one kind): among
    /// the spawn points the kind may use, with its own offsets and rise if it has them.
    pub fn spawn_point_in(
        &self,
        rng: &mut Rng,
        others: &[Vec3],
        radius: f32,
        aim: Vec3,
        kind: Option<&WorldKind>,
    ) -> (Vec3, Vec3) {
        let allowed = kind.map_or(&[][..], |k| &k.spawns[..]);
        let [offset_min, offset_max] = kind.and_then(|k| k.offsets).unwrap_or([self.offset_min, self.offset_max]);
        let rise = Vec3::Y * kind.map_or(0.0, |k| k.rise);
        let usable: Vec<&WorldSpawn> = if allowed.is_empty() {
            self.spawns.iter().collect()
        } else {
            allowed.iter().filter_map(|i| self.spawns.get(*i as usize)).collect()
        };
        let total: f32 = usable.iter().map(|s| s.weight.max(0.0)).sum();
        let mut pos = Vec3::NEG_Z * 10.0;
        let mut right = Vec3::X;
        if usable.is_empty() {
            return (pos, right);
        }
        let mut apart_only = None;
        for _ in 0..40 {
            let mut pick = rng.next_f32() * total;
            let mut point = usable[0];
            for s in &usable {
                let w = s.weight.max(0.0);
                if pick < w {
                    point = s;
                    break;
                }
                pick -= w;
            }
            let mut offset = Vec3::ZERO;
            // Points draw no extra numbers, so their runs keep their seeds' layouts.
            if !is_point(&point.edges) {
                for e in point.edges {
                    offset += Vec3::from_array(e) * rng.next_f32();
                }
            }
            let offset_axes = self.offset_axes.unwrap_or(point.axes);
            for i in 0..3 {
                let (lo, hi) = (offset_min[i], offset_max[i]);
                offset += Vec3::from_array(offset_axes[i]) * (lo + (hi - lo) * rng.next_f32());
            }
            pos = self.out_of_walls(Vec3::from_array(point.pos) + offset + rise, radius);
            right = Vec3::from_array(point.axes[0]);
            let apart = if self.cluster {
                // Near one that's up (the first can go anywhere), without overlapping.
                (others.is_empty() || others.iter().any(|o| o.distance(pos) < self.blocked_radius))
                    && others.iter().all(|o| o.distance(pos) >= 2.0 * radius)
            } else {
                others.iter().all(|o| o.distance(pos) >= self.blocked_radius)
            };
            if apart && self.player_block.as_ref().is_none_or(|b| b.allows(pos, aim)) {
                return (pos, right);
            }
            if apart {
                apart_only.get_or_insert((pos, right));
            }
        }
        apart_only.unwrap_or((pos, right))
    }

    /// `pos` pushed out of every wall it overlaps, except walls around the player
    /// (a box the player stands in is the room, not a wall).
    fn out_of_walls(&self, mut pos: Vec3, radius: f32) -> Vec3 {
        for _ in 0..4 {
            let before = pos;
            for w in self.walls.iter().filter(|w| !w.contains(Vec3::ZERO)) {
                pos = w.push_out(pos, radius);
            }
            if pos == before {
                break;
            }
        }
        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bug: rotation ids weren't checked, and the sim sized a counter list by the
    /// largest one, so `4000000000` tried to allocate 16 GB.
    #[test]
    fn rotation_ids_must_name_a_slot() {
        let mut w = World {
            spawns: vec![WorldSpawn { weight: 1.0, ..Default::default() }],
            kinds: vec![WorldKind { radius: 0.3, health: 1.0, ..Default::default() }],
            slots: vec![vec![(0, 1.0)]],
            rotations: vec![Some(4_000_000_000)],
            ..Default::default()
        };
        assert!(w.problems().iter().any(|p| p.contains("rotation")), "{:?}", w.problems());
        w.rotations = vec![Some(0)];
        assert!(w.problems().is_empty(), "{:?}", w.problems());
    }

    fn world() -> World {
        World {
            spawns: vec![
                WorldSpawn { pos: [-2.0, 0.0, -20.0], ..WorldSpawn::default() },
                WorldSpawn { pos: [2.0, 0.0, -20.0], ..WorldSpawn::default() },
                WorldSpawn { pos: [0.0, 50.0, 0.0], weight: 0.0, ..WorldSpawn::default() },
            ],
            offset_min: [-0.5, -0.5, 0.0],
            offset_max: [0.5, 0.5, 0.0],
            blocked_radius: 1.5,
            ..World::default()
        }
    }

    #[test]
    fn spawns_land_near_a_weighted_point_and_apart() {
        let w = world();
        let mut rng = Rng::new(7);
        let mut placed: Vec<Vec3> = Vec::new();
        for _ in 0..2 {
            let p = w.spawn_pos(&mut rng, &placed, 0.0, Vec3::NEG_Z);
            // Never the zero-weight point.
            assert!(p.y.abs() <= 0.5 && p.z == -20.0, "{p}");
            assert!((p.x.abs() - 2.0).abs() <= 0.5, "{p}");
            placed.push(p);
        }
        // Two points 4 m apart with offsets of ±0.5 m: one each side.
        assert!(placed[0].distance(placed[1]) >= 1.5);
    }

    /// ww5t writes its offsets with min above max. The sim's own rule is that an
    /// offset runs along its spawn point's axes (a point turned 90° moves its x offset
    /// onto the world's z); KovaaK's doesn't do that, and the importer sets
    /// `offset_axes` where it matters.
    #[test]
    fn offsets_follow_the_spawn_points_axes_in_either_order() {
        let w = World {
            spawns: vec![WorldSpawn {
                pos: [0.0, 0.0, -10.0],
                axes: [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]],
                ..WorldSpawn::default()
            }],
            offset_min: [1.0, 0.5, 0.0],
            offset_max: [1.0, -0.5, 0.0],
            ..World::default()
        };
        let mut rng = Rng::new(3);
        for _ in 0..20 {
            let p = w.spawn_pos(&mut rng, &[], 0.0, Vec3::NEG_Z);
            assert!((p.z + 9.0).abs() < 1e-5 && p.x.abs() < 1e-5 && p.y.abs() <= 0.5, "{p}");
        }
    }

    /// A world with a spawn point turned 90° (its own x axis along the world's z) and
    /// an offset of 1 m along x, with a frame for the offsets or without.
    fn turned_world(offset_axes: Option<[[f32; 3]; 3]>) -> World {
        World {
            spawns: vec![WorldSpawn {
                pos: [0.0, 0.0, -10.0],
                axes: [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]],
                ..WorldSpawn::default()
            }],
            offset_min: [1.0, 0.0, 0.0],
            offset_max: [1.0, 0.0, 0.0],
            offset_axes,
            ..World::default()
        }
    }

    /// With `offset_axes` the offset runs along that frame in place of the spawn point's
    /// own axes, and the right-hand direction a bot strafes along stays the point's.
    #[test]
    fn offsets_run_along_the_worlds_frame_when_it_has_one() {
        let map = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mut rng = Rng::new(3);
        let (pos, right) = turned_world(Some(map)).spawn_point(&mut rng, &[], 0.0, Vec3::NEG_Z);
        assert!(pos.distance(Vec3::new(1.0, 0.0, -10.0)) < 1e-5, "{pos}");
        assert!(right.distance(Vec3::Z) < 1e-5, "{right}");
        // Without it: along the point's own axes, as before.
        let (pos, right) = turned_world(None).spawn_point(&mut Rng::new(3), &[], 0.0, Vec3::NEG_Z);
        assert!(pos.distance(Vec3::new(0.0, 0.0, -9.0)) < 1e-5, "{pos}");
        assert!(right.distance(Vec3::Z) < 1e-5, "{right}");
    }

    /// A world without a frame for its offsets stores no key for it (so scenarios stored
    /// before it keep their fingerprints); with one it is kept and read back.
    #[test]
    fn the_offset_frame_is_left_out_when_unset_and_survives_a_round_trip() {
        let text = toml::to_string(&turned_world(None)).unwrap();
        assert!(!text.contains("offset_axes"), "{text}");
        let w = turned_world(Some([[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]));
        let text = toml::to_string(&w).unwrap();
        assert!(text.contains("offset_axes"), "{text}");
        assert_eq!(toml::from_str::<World>(&text).unwrap(), w);
    }

    /// A frame for the offsets with a value that isn't a number, or an axis with no
    /// length, is a problem.
    #[test]
    fn a_bad_offset_frame_is_a_problem() {
        let good = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert!(World { offset_axes: Some(good), ..world() }.problems().is_empty());
        for bad in [
            [[f32::NAN, 0.0, 0.0], good[1], good[2]],
            [good[0], [0.0, f32::INFINITY, 0.0], good[2]],
            [good[0], good[1], [0.0, 0.0, 0.0]],
        ] {
            let problems = World { offset_axes: Some(bad), ..world() }.problems();
            assert!(problems.iter().any(|p| p.contains("offset frame")), "{bad:?}: {problems:?}");
        }
    }

    /// A 1 m thick wall 10 m ahead, 4 m wide and tall, its front face at z = -10.
    fn wall() -> WorldBox {
        WorldBox { corner: [-2.0, -2.0, -11.0], edges: [[4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 1.0]] }
    }

    /// Bug: on "Amare Microcluster Horizontal Easy" the spawn volume reaches into
    /// the wall behind it, and bots appeared buried in it. KovaaK's pushes a bot out
    /// of a wall it would overlap.
    #[test]
    fn targets_are_pushed_out_of_walls() {
        let w = wall();
        // Center inside, near the front face: out the front, clear by the radius.
        assert!((w.push_out(Vec3::new(0.5, 0.0, -10.3), 0.2) - Vec3::new(0.5, 0.0, -9.8)).length() < 1e-5);
        // Center in front but touching: moved to just touch.
        assert!((w.push_out(Vec3::new(0.0, 1.0, -9.9), 0.2) - Vec3::new(0.0, 1.0, -9.8)).length() < 1e-5);
        // Clear of the wall: unchanged.
        assert_eq!(w.push_out(Vec3::new(0.0, 0.0, -9.0), 0.2), Vec3::new(0.0, 0.0, -9.0));

        // A spawn volume from 0.4 m inside the wall to 0.4 m in front of it, and a
        // room around the player, which is left alone.
        let room = WorldBox { corner: [-50.0; 3], edges: [[100.0, 0.0, 0.0], [0.0, 100.0, 0.0], [0.0, 0.0, 100.0]] };
        let world = World {
            spawns: vec![WorldSpawn {
                pos: [-1.0, -1.0, -10.4],
                edges: [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 0.8]],
                ..WorldSpawn::default()
            }],
            walls: vec![wall(), room],
            ..World::default()
        };
        let mut rng = Rng::new(2);
        for _ in 0..100 {
            let p = world.spawn_pos(&mut rng, &[], 0.2, Vec3::NEG_Z);
            assert!((-9.8 - 1e-4..=-9.6 + 1e-4).contains(&p.z), "{p}");
        }
    }

    /// Bug: the player's `BlockSpawnFOV` was ignored, so on 1w4ts a new target could
    /// appear under the crosshair, and on ww5t (inverted) anywhere on the wall
    /// instead of near where you look.
    #[test]
    fn the_players_view_limits_where_targets_appear() {
        let ahead = Vec3::new(0.0, 0.0, -20.0);
        let side = Vec3::new(10.0, 0.0, -20.0); // about 26.6 degrees right
        let block = PlayerBlock { fov_deg: 9.0, ..PlayerBlock::default() };
        assert!(!block.allows(ahead, Vec3::NEG_Z) && block.allows(side, Vec3::NEG_Z));
        // Looking at the side point instead.
        assert!(block.allows(ahead, side.normalize()) && !block.allows(side, side.normalize()));
        let only = PlayerBlock { fov_deg: 40.0, invert: true, ..PlayerBlock::default() };
        assert!(only.allows(ahead, Vec3::NEG_Z) && !only.allows(side, Vec3::NEG_Z));
        // Beyond the distance the view doesn't count.
        assert!(PlayerBlock { fov_deg: 9.0, distance: 15.0, ..PlayerBlock::default() }.allows(ahead, Vec3::NEG_Z));
        assert!(!PlayerBlock { radius: 25.0, ..PlayerBlock::default() }.allows(ahead, Vec3::NEG_Z));

        let w = World {
            spawns: vec![
                WorldSpawn { pos: ahead.to_array(), ..WorldSpawn::default() },
                WorldSpawn { pos: side.to_array(), ..WorldSpawn::default() },
            ],
            player_block: Some(block),
            ..World::default()
        };
        let mut rng = Rng::new(4);
        assert!((0..50).all(|_| w.spawn_pos(&mut rng, &[], 0.1, Vec3::NEG_Z) == side));
        assert!((0..50).all(|_| w.spawn_pos(&mut rng, &[], 0.1, side.normalize()) == ahead));
        // With the only allowed spot taken, staying apart wins over the view
        // (ww5t Clusters fits 12 targets in its 40 degree view only by overlapping).
        let crowded = World { blocked_radius: 2.0, ..w };
        assert!((0..50).all(|_| crowded.spawn_pos(&mut rng, &[side], 0.1, Vec3::NEG_Z) == ahead));
    }

    #[test]
    fn clustered_targets_appear_near_one_already_up() {
        // Spawn points spread along 20 m; clusters keep within 2 m of a target.
        let spawns =
            (0..21).map(|i| WorldSpawn { pos: [i as f32 - 10.0, 0.0, -10.0], ..WorldSpawn::default() }).collect();
        let w = World { spawns, blocked_radius: 2.0, cluster: true, ..World::default() };
        let mut rng = Rng::new(5);
        let mut up = vec![w.spawn_pos(&mut rng, &[], 0.3, Vec3::NEG_Z)];
        for _ in 0..5 {
            let p = w.spawn_pos(&mut rng, &up, 0.3, Vec3::NEG_Z);
            assert!(up.iter().any(|o| o.distance(p) < 2.0), "{p} isn't near {up:?}");
            assert!(up.iter().all(|o| o.distance(p) >= 0.6), "{p} overlaps one of {up:?}");
            up.push(p);
        }
    }

    #[test]
    fn accuracy_multipliers() {
        assert_eq!(AccuracyMult::None.factor(Some(0.25)), 1.0);
        assert_eq!(AccuracyMult::Linear.factor(Some(0.25)), 0.25);
        assert_eq!(AccuracyMult::Sqrt.factor(Some(0.25)), 0.5);
        assert_eq!(AccuracyMult::Sqrt.factor(None), 1.0);
    }

    #[test]
    fn a_world_without_spawns_is_a_problem() {
        let w = World { spawns: vec![], ..world() };
        assert!(!w.problems().is_empty());
        assert!(world().problems().is_empty());
    }

    /// A bot's ability numbers are checked, whether the bots share one motion or each
    /// kind has its own.
    #[test]
    fn ability_values_out_of_range_are_problems() {
        use crate::motion::{Abilities, Ability, Motion};
        let motion = |speed: f32| Motion {
            abilities: Some(Abilities { list: vec![Ability { speed, ..Default::default() }], ..Default::default() }),
            ..Default::default()
        };
        let w = World { motion: Some(motion(10.0)), ..world() };
        assert!(w.problems().is_empty(), "{:?}", w.problems());
        let w = World { motion: Some(motion(9000.0)), ..world() };
        assert!(w.problems().iter().any(|p| p.contains("speed")), "{:?}", w.problems());
        let kind = WorldKind { motion: Some(motion(f32::NAN)), ..Default::default() };
        let w = World { kinds: vec![kind], slots: vec![vec![(0, 1.0)]], ..world() };
        assert!(w.problems().iter().any(|p| p.contains("speed")), "{:?}", w.problems());
    }

    fn moving() -> Option<crate::motion::Motion> {
        Some(crate::motion::Motion::default())
    }

    /// A helper bot's numbers are checked, and a world with helpers needs bots that can
    /// be pushed.
    #[test]
    fn helper_bots_with_bad_values_are_problems() {
        let good =
            Pusher { pos: [0.0, 0.0, -5.0], radius: 0.5, every_s: 0.1, push: 3.0, lift: -2.0, ..Pusher::default() };
        let with = |p: Pusher| World { pushers: vec![p], motion: moving(), ..world() };
        assert!(with(good.clone()).problems().is_empty(), "{:?}", with(good.clone()).problems());
        let bad = [
            ("a number", Pusher { push: f32::NAN, ..good.clone() }),
            ("a number", Pusher { pos: [f32::INFINITY, 0.0, 0.0], ..good.clone() }),
            ("radius", Pusher { radius: 0.0, ..good.clone() }),
            ("radius", Pusher { radius: -1.0, ..good.clone() }),
            ("every", Pusher { every_s: 0.004, ..good.clone() }),
            ("unit vector", Pusher { facing: [0.0, 0.0, 2.0], ..good.clone() }),
            ("unit vector", Pusher { facing: [0.0; 3], ..good.clone() }),
            ("push", Pusher { push: 1000.5, ..good.clone() }),
            ("push", Pusher { lift: -1001.0, ..good.clone() }),
        ];
        for (word, p) in bad {
            let problems = with(p.clone()).problems();
            assert!(problems.iter().any(|m| m.contains(word)), "{p:?}: {problems:?}");
        }
        // The edges are fine.
        let edge = Pusher { every_s: 0.005, push: 1000.0, lift: -1000.0, facing: [0.0, 1.0, 0.0001], ..good.clone() };
        assert!(with(edge).problems().is_empty());
        // Nothing to push: no bot can move, or one of several kinds can't.
        let w = World { pushers: vec![good.clone()], ..world() };
        assert!(w.problems().iter().any(|m| m.contains("can move")), "{:?}", w.problems());
        let kinds = |second: Option<crate::motion::Motion>| World {
            pushers: vec![good.clone()],
            kinds: vec![
                WorldKind { motion: moving(), ..Default::default() },
                WorldKind { motion: second, ..Default::default() },
            ],
            slots: vec![vec![(0, 1.0)], vec![(1, 1.0)]],
            ..world()
        };
        assert!(kinds(moving()).problems().is_empty(), "{:?}", kinds(moving()).problems());
        assert!(kinds(None).problems().iter().any(|m| m.contains("can move")));
    }

    /// Imported scenarios saved before helper bots have no `pushers` and no
    /// `invincible` on their kinds, and their fingerprints must stay what they were,
    /// so these are left out of the file at their defaults.
    #[test]
    fn new_world_fields_are_left_out_at_their_defaults() {
        let w = World { kinds: vec![WorldKind::default()], slots: vec![vec![(0, 1.0)]], ..world() };
        let text = toml::to_string(&w).unwrap();
        assert!(!text.contains("pushers") && !text.contains("invincible"), "{text}");
        let back: World = toml::from_str(&text).unwrap();
        assert_eq!(back, w);
        // Set, they are kept.
        let w = World {
            pushers: vec![Pusher { push: 1.5, ..Pusher::default() }],
            kinds: vec![WorldKind { invincible: true, ..Default::default() }],
            slots: vec![vec![(0, 1.0)]],
            motion: moving(),
            ..world()
        };
        let back: World = toml::from_str(&toml::to_string(&w).unwrap()).unwrap();
        assert_eq!(back, w);
        assert!(toml::from_str::<World>("[[pushers]]\nspeed = 1.0").is_err(), "unknown fields are refused");
    }

    /// A pusher with no flight and no start delay stores neither (so scenarios stored
    /// before them keep their fingerprints); set, both are kept and read back.
    #[test]
    fn a_flight_and_a_start_delay_are_left_out_at_their_defaults() {
        let with = |p: Pusher| World { pushers: vec![p], motion: moving(), ..world() };
        let plain = with(Pusher { chases: true, push: 1.5, ..Pusher::default() });
        let text = toml::to_string(&plain).unwrap();
        assert!(!text.contains("flight") && !text.contains("start_s"), "{text}");
        assert_eq!(toml::from_str::<World>(&text).unwrap(), plain);
        let flying = with(Pusher {
            chases: true,
            flight: Some(ChaseFlight { accel: 300.0, max_speed: 40.0, drag: 6.0 }),
            start_s: 1.5,
            ..Pusher::default()
        });
        let text = toml::to_string(&flying).unwrap();
        assert!(text.contains("flight") && text.contains("start_s"), "{text}");
        assert_eq!(toml::from_str::<World>(&text).unwrap(), flying);
        assert!(flying.problems().is_empty(), "{:?}", flying.problems());
    }

    /// A flight is only valid on a chaser, with three numbers above 0 and not absurd,
    /// and a start delay is 0 to an hour.
    #[test]
    fn a_flight_and_a_start_delay_with_bad_values_are_problems() {
        let flight = |accel: f32, max_speed: f32, drag: f32| Some(ChaseFlight { accel, max_speed, drag });
        let good = Pusher { chases: true, flight: flight(300.0, 40.0, 6.0), ..Pusher::default() };
        let with = |p: Pusher| World { pushers: vec![p], motion: moving(), ..world() };
        assert!(with(good.clone()).problems().is_empty());
        let bad = [
            ("can't have a flight", Pusher { chases: false, ..good.clone() }),
            ("acceleration", Pusher { flight: flight(0.0, 40.0, 6.0), ..good.clone() }),
            ("top speed", Pusher { flight: flight(300.0, -1.0, 6.0), ..good.clone() }),
            ("drag", Pusher { flight: flight(300.0, 40.0, 0.0), ..good.clone() }),
            ("acceleration", Pusher { flight: flight(1_000_001.0, 40.0, 6.0), ..good.clone() }),
            ("top speed", Pusher { flight: flight(300.0, 10_001.0, 6.0), ..good.clone() }),
            ("drag", Pusher { flight: flight(300.0, 40.0, 1000.5), ..good.clone() }),
            ("a number", Pusher { flight: flight(f32::NAN, 40.0, 6.0), ..good.clone() }),
            ("a number", Pusher { flight: flight(300.0, f32::INFINITY, 6.0), ..good.clone() }),
            ("start delay", Pusher { start_s: -0.5, ..good.clone() }),
            ("start delay", Pusher { start_s: 3600.5, ..good.clone() }),
            ("a number", Pusher { start_s: f32::NAN, ..good.clone() }),
        ];
        for (word, p) in bad {
            let problems = with(p.clone()).problems();
            assert!(problems.iter().any(|m| m.contains(word)), "{p:?}: {problems:?}");
        }
        // The edges are fine, for a pinned pusher too (a start delay).
        let edge = Pusher { flight: flight(1_000_000.0, 10_000.0, 1000.0), start_s: 3600.0, ..good.clone() };
        assert!(with(edge).problems().is_empty());
        let pinned = Pusher { start_s: 2.0, ..Pusher::default() };
        assert!(with(pinned).problems().is_empty());
    }

    /// `reaches` is the reach along the facing, and `reaches_along` takes the direction.
    #[test]
    fn the_reach_can_point_along_any_direction() {
        let p = Pusher { radius: 0.5, facing: [0.0, 1.0, 0.0], ..Pusher::default() };
        let (at, ball) = (Vec3::ZERO, Vec3::new(1.3, 0.5, 0.0));
        assert!(!p.reaches(at, ball, 0.5, 0.0));
        assert!(!p.reaches_along(at, Vec3::Y, ball, 0.5, 0.0));
        assert!(p.reaches_along(at, ball.normalize(), ball, 0.5, 0.0));
    }
}
