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
    /// The score is multiplied by accuracy, or by its square root.
    pub accuracy_mult: AccuracyMult,
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

/// Triangles with one color, e.g. a wall.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldMesh {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub color: String,
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
        p
    }

    /// A spawn position for a target of `radius`: a weighted spawn point (or a
    /// random point in a spawn volume) plus a random offset, pushed out of walls,
    /// away from `others` and where the player's profile allows while they look
    /// along `aim`. After 40 tries it takes the first one that was apart from the
    /// others, or else the last one, as a crowded map would.
    pub fn spawn_pos(&self, rng: &mut Rng, others: &[Vec3], radius: f32, aim: Vec3) -> Vec3 {
        let total: f32 = self.spawns.iter().map(|s| s.weight.max(0.0)).sum();
        let mut pos = Vec3::NEG_Z * 10.0;
        if self.spawns.is_empty() {
            return pos;
        }
        let mut apart_only = None;
        for _ in 0..40 {
            let mut pick = rng.next_f32() * total;
            let mut point = &self.spawns[0];
            for s in &self.spawns {
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
            let apart = others.iter().all(|o| o.distance(pos) >= self.blocked_radius);
            if apart && self.player_block.as_ref().is_none_or(|b| b.allows(pos, aim)) {
                return pos;
            }
            if apart {
                apart_only.get_or_insert(pos);
            }
        }
        apart_only.unwrap_or(pos)
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
