//! Spawn offsets. A character profile can shift where its bots appear from the spawn
//! point (`SpawnOffsetMin` and `SpawnOffsetMax`, or the older `SpawnXOffset`,
//! `SpawnYOffset` and `VerticalSpawnOffset`). Measured in KovaaK's 3.9.13: the offsets
//! run along the map's axes (X forward, Y right, Z up, in Unreal units that `MapScale`
//! doesn't scale) however the spawn point is turned, and a file saved before KovaaK's
//! 2.0.5.0 has them turned a quarter. This module reads them and decides when the world
//! needs to say which way they run.

use aimadapt_sim::Vec3;
use aimadapt_sim::world::World;

use super::{UU_PER_M, axes};
use crate::inventory::Gap;
use crate::settings::Section;

/// The older offset settings, in the order of the vector they make (X, Y, Z).
const OLDER_KEYS: [&str; 3] = ["SpawnXOffset", "SpawnYOffset", "VerticalSpawnOffset"];

/// Whether the file was saved by a KovaaK's older than 2.0.5.0, which turned the offsets
/// a quarter. A `GameVersion` that is missing or can't be read counts as a newer one.
pub(super) fn offsets_turned(top: &Section) -> bool {
    let text = top.str("GameVersion").trim();
    let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
    let Some(parts) = text.split('.').map(|p| p.trim().parse::<u32>().ok()).collect::<Option<Vec<u32>>>() else {
        return false;
    };
    let part = |i: usize| parts.get(i).copied().unwrap_or(0);
    (part(0), part(1), part(2)) < (2, 0, 5)
}

/// A character's spawn offsets `[min, max]` in meters, in the sim's axes (right, up,
/// back), for a file whose offsets are `turned` or not. Each axis is picked between the
/// two values, so their order doesn't matter (ww5t writes min above max). The older
/// settings are one fixed offset. A character with both kinds isn't read, because how
/// KovaaK's adds them wasn't measured.
pub(super) fn read(ch: &Section, turned: bool) -> Result<[[f32; 3]; 2], Gap> {
    let newer = ["SpawnOffsetMin", "SpawnOffsetMax"].map(|k| ch.vec3(k).unwrap_or_default());
    // A millimeter can't be seen, and reading it would change the stored scenario of
    // the files that carry such a leftover.
    let older = OLDER_KEYS.map(|k| ch.f32_or(k, 0.0)).map(|v| if v.abs() < 0.1 { 0.0 } else { v });
    let mut offsets = newer;
    if older.iter().any(|v| *v != 0.0) {
        if newer.iter().flatten().any(|v| *v != 0.0) {
            return Err(Gap::planned(
                "spawn offsets",
                format!(
                    "{}: SpawnXOffset, SpawnYOffset or VerticalSpawnOffset together with SpawnOffsetMin or \
                     SpawnOffsetMax; how KovaaK's adds them wasn't measured",
                    ch.name()
                ),
            ));
        }
        offsets = [older; 2];
    }
    // A zero vector is left alone: turning it would flip the sign of a zero, and that
    // is stored (and so is in the scenario's fingerprint).
    Ok(offsets.map(|[x, y, z]| {
        let v = if turned && (x != 0.0 || y != 0.0) { [-y, x, z] } else { [x, y, z] };
        (axes(v) / UU_PER_M).to_array()
    }))
}

/// Whether two ways of running the offsets `[lo, hi]` fill the same box of positions:
/// the centres are within a millimeter, and every edge that has a length matches an edge
/// of the other box, the same way or the opposite way. A spawn point turned half round
/// with offsets that mirror each other keeps its box.
pub(super) fn same_box(a: [Vec3; 3], b: [Vec3; 3], lo: [f32; 3], hi: [f32; 3]) -> bool {
    const SAME: f32 = 0.001;
    const SHORTEST: f32 = 0.000_001;
    let centre = |f: [Vec3; 3]| (0..3).map(|i| f[i] * (lo[i] + hi[i]) / 2.0).sum::<Vec3>();
    let edges = |f: [Vec3; 3]| -> Vec<Vec3> {
        (0..3).map(|i| f[i] * (hi[i] - lo[i]).abs() / 2.0).filter(|e| e.length() > SHORTEST).collect()
    };
    let matched = |from: &[Vec3], to: &[Vec3]| {
        from.iter().all(|e| to.iter().any(|g| (*e - *g).length() <= SAME || (*e + *g).length() <= SAME))
    };
    let (ea, eb) = (edges(a), edges(b));
    (centre(a) - centre(b)).length() <= SAME && matched(&ea, &eb) && matched(&eb, &ea)
}

/// Settles `world.offset_axes`, which the world builder sets to the map's frame: it
/// stays only when running the offsets along it, not along each spawn point's own axes,
/// moves some target. Otherwise it goes, and the scenario keeps the settings (and the
/// fingerprint) it had without it.
pub(super) fn keep_offset_axes(world: &mut World) {
    let Some(map) = world.offset_axes.take() else { return };
    let frame = map.map(Vec3::from_array);
    let uses = |offsets: [[f32; 3]; 2], spawns: &[u32]| {
        let all = spawns.is_empty();
        world
            .spawns
            .iter()
            .enumerate()
            .filter(|(i, _)| all || spawns.contains(&(*i as u32)))
            .any(|(_, s)| !same_box(s.axes.map(Vec3::from_array), frame, offsets[0], offsets[1]))
    };
    let own = [world.offset_min, world.offset_max];
    let moved = if world.kinds.is_empty() {
        uses(own, &[])
    } else {
        world.kinds.iter().any(|k| uses(k.offsets.unwrap_or(own), &k.spawns))
    };
    world.offset_axes = moved.then_some(map);
}

#[cfg(test)]
mod tests;
