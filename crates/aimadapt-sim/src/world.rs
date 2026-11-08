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
use crate::scenario::Shape;
use crate::sim::Rng;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct World {
    /// Where the scenario came from, e.g. "VT 1w4ts Novice S5.sce", and a hash of
    /// the file, so history stays tied to one version of it.
    pub source: String,
    pub source_hash: String,
    /// Where targets can appear, each picked in proportion to its weight.
    pub spawns: Vec<WorldSpawn>,
    /// A random offset added to the spawn point: each axis between these two values
    /// (in either order), along the spawn point's own axes.
    pub offset_min: [f32; 3],
    pub offset_max: [f32; 3],
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<Motion>,
    /// The spawn points it may use, as indices into the world's; empty for any.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spawns: Vec<u32>,
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
            motion: None,
            spawns: Vec::new(),
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
    /// Where the spawn point's own x, y and z axes point (it faces along -z), for
    /// the offset.
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
        p
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
        self.spawn_point_in(rng, others, radius, aim, &[])
    }

    /// [`World::spawn_point`] among the spawn points `allowed` (indices; empty for all).
    pub fn spawn_point_in(
        &self,
        rng: &mut Rng,
        others: &[Vec3],
        radius: f32,
        aim: Vec3,
        allowed: &[u32],
    ) -> (Vec3, Vec3) {
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
            for i in 0..3 {
                let (lo, hi) = (self.offset_min[i], self.offset_max[i]);
                offset += Vec3::from_array(point.axes[i]) * (lo + (hi - lo) * rng.next_f32());
            }
            pos = self.out_of_walls(Vec3::from_array(point.pos) + offset, radius);
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

    /// ww5t writes its offsets with min above max, and along each spawn point's
    /// facing: a point turned 90° moves its x offset onto the world's z.
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
}
