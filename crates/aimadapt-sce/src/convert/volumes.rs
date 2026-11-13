//! Hurt volumes and deadly teleporters (plan K12a). A map's `Hurt` object kills or damages a body that touches it,
//! and a `Teleporter` with no delay and a target that kills at once is death for a body that touches it. The sim's
//! rules are in `aimadapt_sim::sim` (the `volumes` module), measured in KovaaK's 3.9.13 on 12 November 2026.
//! What isn't measured stays refused, each with a detail that says why (feature "map volumes"), or gets a note.
//!
//! A volume is a box of 100 units per scale step. In a map of version 0.0.7 its location is the middle of the
//! box, and in later maps (measured in 1.0.0) the lowest corner, as a `Cube` brush's is. `MapScale` multiplies a
//! volume's place and size, not the bodies.

use aimadapt_sim::Vec3;
use aimadapt_sim::motion::Motion;
use aimadapt_sim::world::WorldVolume;

use super::{CUBE_UU, UU_PER_M, heads, map_point, offsets, spawn_rise, team_can_use, transform};
use crate::map::{Map, Placement, Volume};
use crate::scenario::{Resolved, player_team};
use crate::settings::Section;

/// The map version whose volumes are placed by their middle.
const MIDDLE_PIVOT_VERSION: &str = "0.0.7";
/// How far past the player's own spawn offsets a volume still counts as touching its start, in meters.
const START_MARGIN_M: f32 = 0.1;

/// A box's corner and its three edges, in meters in the sim's axes, with `place` moving each point. The box is
/// `CUBE_UU` units a side at scale 1, from `p`'s location back by `back` units along each axis.
fn placed_box(p: &Placement, back: f32, map_scale: f32, place: &impl Fn(Vec3) -> Vec3) -> (Vec3, [Vec3; 3]) {
    let at = |c: [f32; 3]| place(map_point(transform(p, c), map_scale));
    let corner = at([back; 3]);
    let edge = |c: [f32; 3]| at(c.map(|v| v + back)) - corner;
    (corner, [edge([CUBE_UU, 0.0, 0.0]), edge([0.0, CUBE_UU, 0.0]), edge([0.0, 0.0, CUBE_UU])])
}

/// A volume's box (see the module's note on where its location is).
fn volume_box(map: &Map, p: &Placement, map_scale: f32, place: &impl Fn(Vec3) -> Vec3) -> (Vec3, [Vec3; 3]) {
    let back = if map.version.trim() == MIDDLE_PIVOT_VERSION { -CUBE_UU / 2.0 } else { 0.0 };
    placed_box(p, back, map_scale, place)
}

/// The lowest and highest point of a box given by a corner and edges.
fn bounds(corner: Vec3, edges: [Vec3; 3]) -> (Vec3, Vec3) {
    let points = (0..8).map(|i| corner + (0..3).filter(|k| i >> k & 1 == 1).map(|k| edges[k]).sum::<Vec3>());
    points.fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(p), hi.max(p)))
}

fn overlap(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    a.0.cmple(b.1).all() && b.0.cmple(a.1).all()
}

/// Why a file's volumes can't be played, as the details of the gaps "map volumes", each once, in the order found.
/// The file's volumes are `map.volumes`; `top` is its settings and `r` what they point at.
pub(crate) fn refusals(map: &Map, top: &Section, r: &Resolved) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if map.volumes.is_empty() {
        return out;
    }
    let mut say = |text: &str| {
        if !out.iter().any(|t| t == text) {
            out.push(text.to_string());
        }
    };
    let map_scale = top.f32_or("MapScale", 1.0);
    let team = player_team(top);
    let targets: Vec<&Section> = r
        .bots
        .iter()
        .filter(|s| !s.is_helper(team))
        .flat_map(|s| s.choices.iter().filter_map(|c| c.character))
        .collect();
    // The health a hit that is due every frame has to reach to kill at once.
    let max_health = targets.iter().map(|ch| ch.f32_or("MaxHealth", 100.0).max(0.001)).fold(0.0, f32::max);
    let bounds_of = |p: &Placement| {
        let (corner, edges) = volume_box(map, p, map_scale, &|v| v);
        bounds(corner, edges)
    };
    let deadly: Vec<(Vec3, Vec3)> = map
        .volumes
        .iter()
        .filter_map(|v| match v {
            Volume::Hurt { place, kill, damage, cooldown_s }
                if *kill || (*cooldown_s <= 0.0 && *damage >= max_health) =>
            {
                Some(bounds_of(place))
            }
            _ => None,
        })
        .collect();
    let invincible = top.is("InvincibleBots") || targets.iter().any(|ch| ch.is("InvincibleBots"));
    for v in &map.volumes {
        if v.place().rotation != [0.0; 3] {
            say("a turned volume");
        }
        match v {
            Volume::Teleporter { target, delay_s, .. } => {
                if *delay_s > 0.0 {
                    say("a teleporter with a delay");
                }
                match map.waypoints.iter().find(|w| w.name == *target) {
                    None => say("a teleporter whose target is missing"),
                    Some(w) => {
                        // Where each kind of target bot's middle ends up: its spawn rise above the waypoint.
                        let lands = |ch: &Section| map_point(w.position, map_scale) + Vec3::Y * spawn_rise(ch);
                        let inside = |p: Vec3| deadly.iter().any(|(lo, hi)| lo.cmple(p).all() && p.cmple(*hi).all());
                        if !targets.iter().all(|ch| inside(lands(ch))) {
                            say("a teleporter that leads into the open");
                        }
                    }
                }
            }
            Volume::Hurt { kill, damage, cooldown_s, .. } => {
                if *damage < 0.0 || *cooldown_s < 0.0 {
                    say("a volume with a negative number");
                }
                if !kill && *cooldown_s <= 0.0 && *damage != 0.0 && *damage < max_health {
                    say("damage every frame that doesn't kill at once");
                }
                if invincible && (*kill || *damage > 0.0) {
                    say("bots that can't die");
                }
            }
        }
    }
    if r.bots.iter().any(|s| s.is_helper(team)) {
        say("a bot on the player's team");
    }
    if let Some(p) = r.player {
        // The player's body: upright, from the spawn point up to twice its rise, its radius wide, and each side
        // grown by the most its spawn offsets can move it, with a margin.
        let reach = offsets::largest(p) + START_MARGIN_M;
        let radius = p.f32_or("MainBBRadius", 0.0) / UU_PER_M + reach;
        let (below, above) = (Vec3::new(radius, reach, radius), Vec3::new(radius, 2.0 * spawn_rise(p) + reach, radius));
        let starts_inside = map.spawns.iter().filter(|s| team_can_use(s.teams, team)).any(|s| {
            let at = map_point(s.position, map_scale);
            let (lo, hi) = match &s.volume {
                Some(v) => {
                    let (corner, edges) = placed_box(v, 0.0, map_scale, &|p| p);
                    bounds(corner, edges)
                }
                None => (at, at),
            };
            let body = (lo - below, hi + above);
            map.volumes.iter().any(|v| overlap(body, bounds_of(v.place())))
        });
        if starts_inside {
            say("the player starts inside one");
        }
    }
    out
}

/// The map's volumes as the world's, moved by `place` into the sim's frame. A hurt volume is drawn. A teleporter
/// is not, and since only one whose target kills at once is played (see [`refusals`]), touching it is death.
pub(super) fn build(map: &Map, map_scale: f32, place: &impl Fn(Vec3) -> Vec3) -> Vec<WorldVolume> {
    map.volumes
        .iter()
        .map(|v| {
            let (corner, edges) = volume_box(map, v.place(), map_scale, place);
            let (kill, damage, cooldown_s, shown) = match v {
                Volume::Hurt { kill, damage, cooldown_s, .. } => (*kill, *damage, *cooldown_s, true),
                Volume::Teleporter { .. } => (true, 0.0, 0.0, false),
            };
            WorldVolume {
                corner: corner.to_array(),
                edges: edges.map(|e| e.to_array()),
                kill,
                damage,
                cooldown_s,
                shown,
            }
        })
        .collect()
}

/// Notes on what plays by a rule nobody checked in KovaaK's, pushed when the file has volumes.
pub(super) fn notes(map: &Map, kinds: &[(&Section, Option<Motion>)], notes: &mut Vec<String>) {
    if map.volumes.is_empty() {
        return;
    }
    notes.push(
        "a bot touches a hurt volume by its bounding box, which wasn't checked against KovaaK's for shapes other \
         than balls"
            .into(),
    );
    notes.push(
        "a hit from a volume doesn't set off a bot's damage reactions; that wasn't checked against KovaaK's".into(),
    );
    if kinds.iter().any(|(ch, _)| heads::head(ch).is_some()) {
        notes.push(
            "a bot with a head touches a volume as its body and head together do, as one upright box; that wasn't \
             checked against KovaaK's"
                .into(),
        );
    }
}

#[cfg(test)]
mod tests;
