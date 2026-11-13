//! Props that block shots and bodies (plan K13a). A map's `prop` object is a ready-made model placed with a
//! location, rotation and scale. Which props are solid, and where, was measured in KovaaK's 3.9.13 on 12 and
//! 13 November 2026 with test maps (map versions 0.0.7 and 1.0.0). Boxes are in the prop's own units before its
//! scale (x forward, y right, z up, the origin at the prop's pivot):
//!
//! - `Window`: one box, x from -198 to 198, y from -4.25 to 3.55, z from 0 to 302. The whole rectangle blocks,
//!   glass and frame alike.
//! - `Tree`: three boxes on the pivot's vertical, each square in x and y: half-width 37.5 from z 0 to 130,
//!   half-width 22.5 from z 130 to 190, half-width 12.5 from z 190 to 222.
//! - `Banner` and `BannerB`: never solid, whatever the flag says. Shots and bodies pass through.
//!
//! The solid follows the prop's location, rotation and scale exactly as a `Cube` brush does, including a
//! `MapScale` other than 1 and a scale that differs per axis. A prop whose `EnableCollision` is false is drawn
//! in KovaaK's but isn't solid. A ball that spawned overlapping a Window's solid behaved as one overlapping a
//! `Cube` brush slab did in every case tried, so a box here is treated as a `Cube` brush wall everywhere.
//!
//! Read by inference, not measured: a Tree with `EnableCollision` written out as true or false (its default was
//! tried), and a Tree's and a Banner's behaviour at every rotation. Names are matched exactly, with their case.
//! A colliding prop of any other kind stays refused (feature "solid props").
//!
//! The props are drawn as plain boxes the size of what blocks, in a flat colour. A Window's glass can be seen
//! through in KovaaK's and its box here can't.

use aimadapt_sim::Vec3;
use aimadapt_sim::world::{WorldBox, WorldMesh};

use super::{box_mesh, box_wall};
use crate::map::{Map, Prop};

/// A box in a prop's own units, before the prop's scale.
pub(crate) struct LocalBox {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

/// What KovaaK's showed for a kind of prop.
pub(crate) enum Measured {
    /// Solid in these boxes, drawn here in a flat colour.
    Boxes { boxes: &'static [LocalBox], color: &'static str },
    /// Never solid, whatever `EnableCollision` says.
    NotSolid,
}

const WINDOW: [LocalBox; 1] = [LocalBox { lo: [-198.0, -4.25, 0.0], hi: [198.0, 3.55, 302.0] }];
const TREE: [LocalBox; 3] = [
    LocalBox { lo: [-37.5, -37.5, 0.0], hi: [37.5, 37.5, 130.0] },
    LocalBox { lo: [-22.5, -22.5, 130.0], hi: [22.5, 22.5, 190.0] },
    LocalBox { lo: [-12.5, -12.5, 190.0], hi: [12.5, 12.5, 222.0] },
];

/// What was measured for the prop kind `name`, if anything.
pub(crate) fn measured(name: &str) -> Option<Measured> {
    match name {
        "Window" => Some(Measured::Boxes { boxes: &WINDOW, color: "#46505f" }),
        "Tree" => Some(Measured::Boxes { boxes: &TREE, color: "#2f4a3a" }),
        "Banner" | "BannerB" => Some(Measured::NotSolid),
        _ => None,
    }
}

/// The names of the props that collide and have no measured entry, in file order.
pub(crate) fn unmeasured(map: &Map) -> Vec<&str> {
    map.props.iter().filter(|p| p.collides && measured(&p.name).is_none()).map(|p| p.name.as_str()).collect()
}

/// The boxes a prop blocks with and their colour: none when it doesn't collide, is never solid or has no size.
fn solid_boxes(prop: &Prop) -> Option<(&'static [LocalBox], &'static str)> {
    if !prop.collides || prop.place.scale.iter().any(|s| s.abs() < 1e-6) {
        return None;
    }
    match measured(&prop.name)? {
        Measured::Boxes { boxes, color } => Some((boxes, color)),
        Measured::NotSolid => None,
    }
}

/// The meshes and wall boxes of every solid prop, where they stand in the map (the world builder moves them
/// to the player's frame with the rest).
pub(super) fn build(map: &Map, map_scale: f32) -> (Vec<WorldMesh>, Vec<WorldBox>) {
    let same = |p: Vec3| p;
    let (mut meshes, mut walls) = (Vec::new(), Vec::new());
    for prop in &map.props {
        let Some((boxes, color)) = solid_boxes(prop) else { continue };
        for b in boxes {
            let mesh = box_mesh(&prop.place, b.lo, b.hi, map_scale, &same);
            meshes.push(WorldMesh { color: color.into(), decor: false, clip: false, ..mesh });
            walls.push(box_wall(&prop.place, b.lo, b.hi, map_scale, &same));
        }
    }
    (meshes, walls)
}

/// Notes on what the importer did with the map's props: the kinds drawn as boxes, and the kinds left out.
pub(super) fn notes(map: &Map, notes: &mut Vec<String>) {
    let kinds = |keep: &dyn Fn(&Prop) -> bool| -> Vec<&str> {
        let set: std::collections::BTreeSet<&str> =
            map.props.iter().filter(|p| keep(p)).map(|p| p.name.as_str()).collect();
        set.into_iter().collect()
    };
    let built = kinds(&|p| solid_boxes(p).is_some());
    if !built.is_empty() {
        notes.push(format!(
            "some props are drawn as plain boxes the size of what blocks shots in KovaaK's ({})",
            built.join(", ")
        ));
    }
    // A colliding kind that wasn't measured refuses the file, so it has no note.
    let left_out = kinds(&|p| solid_boxes(p).is_none() && !(p.collides && measured(&p.name).is_none()));
    if !left_out.is_empty() {
        notes.push(format!(
            "some props aren't drawn ({}); nothing collides with them in KovaaK's either",
            left_out.join(", ")
        ));
    }
}

#[cfg(test)]
mod tests;
