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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldSpawn {
    pub pos: [f32; 3],
    pub weight: f32,
    /// Where the spawn point's own x, y and z axes point (it faces along -z), for
    /// the offset.
    pub axes: [[f32; 3]; 3],
}

impl Default for WorldSpawn {
    fn default() -> Self {
        WorldSpawn { pos: [0.0; 3], weight: 1.0, axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] }
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

    /// A spawn position away from `others`: a weighted spawn point plus a random
    /// offset. After 40 tries it takes the last one, as a crowded map would.
    pub fn spawn_pos(&self, rng: &mut Rng, others: &[Vec3]) -> Vec3 {
        let total: f32 = self.spawns.iter().map(|s| s.weight.max(0.0)).sum();
        let mut pos = Vec3::NEG_Z * 10.0;
        if self.spawns.is_empty() {
            return pos;
        }
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
            for i in 0..3 {
                let (lo, hi) = (self.offset_min[i], self.offset_max[i]);
                offset += Vec3::from_array(point.axes[i]) * (lo + (hi - lo) * rng.next_f32());
            }
            pos = Vec3::from_array(point.pos) + offset;
            if others.iter().all(|o| o.distance(pos) >= self.blocked_radius) {
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
            let p = w.spawn_pos(&mut rng, &placed);
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
            let p = w.spawn_pos(&mut rng, &[]);
            assert!((p.z + 9.0).abs() < 1e-5 && p.x.abs() < 1e-5 && p.y.abs() <= 0.5, "{p}");
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
