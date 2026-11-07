//! Turns a `.sce` file into a trainer [`Scenario`] with a [`World`]: the map's
//! spawn points and walls, the bots' hitbox and health, the weapon's fire rate and
//! damage, and KovaaK's score rules. This is phase 2 of BACKLOG section 9: bots
//! that stand still, on JSON maps. Anything else comes back as reasons, in plain
//! words, so the app can say why a scenario can't be played yet.
//!
//! Units and axes. KovaaK's (Unreal) uses centimeters with X forward, Y right and
//! Z up. The sim uses meters with -Z forward, +X right and +Y up, with the player's
//! eye at the origin facing -Z. Map positions and walls are multiplied by the
//! scenario's `MapScale`; character sizes and spawn offsets are not. That reading
//! of `MapScale` is still to be checked against KovaaK's (phase 4), so it lives in
//! [`map_point`] alone.

use std::path::Path;

use aimadapt_sim::scenario::{Category, Scoring, Shape, Skill, Timing, Trigger, Weapon, WeaponKind};
use aimadapt_sim::world::{AccuracyMult, World, WorldMesh, WorldSpawn};
use aimadapt_sim::{Scenario, Vec3};

use crate::map::{Map, MapFormat, Placement, Solid, Teams};
use crate::scenario::Sce;
use crate::settings::Section;

/// Unreal units in a meter.
const UU_PER_M: f32 = 100.0;
/// A JSON `Cube` brush is this many units on a side at scale 1, placed from its
/// lowest corner.
const CUBE_UU: f32 = 100.0;
const PLAYER_TEAM: u32 = 1;
const WALL_COLOR: &str = "#3a3f4b";

/// A scenario read from a file, ready to play, plus notes on what plays differently
/// from KovaaK's.
#[derive(Debug, Clone)]
pub struct Imported {
    pub scenario: Scenario,
    pub notes: Vec<String>,
}

/// Reads and converts a `.sce` file. The scenario's world records the file name
/// and a hash of its bytes.
pub fn import(path: &Path) -> Result<Imported, Vec<String>> {
    let bytes = std::fs::read(path).map_err(|e| vec![format!("couldn't read {}: {e}", path.display())])?;
    let sce = Sce::parse(&crate::scenario::decode(&bytes));
    let source = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    convert(&sce, &source, &hash(&bytes))
}

/// FNV-1a of the file, as hex.
pub fn hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Unreal axes to the sim's, without scaling: (forward, right, up) → (right, up, -forward).
fn axes(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[1], v[2], -v[0])
}

/// A map position in meters, in the sim's axes, before moving to the player's eye.
fn map_point(v: [f32; 3], map_scale: f32) -> Vec3 {
    axes(v) * map_scale / UU_PER_M
}

/// Hex color from a KovaaK's color vector, written as 0–1 or 0–255.
fn hex_color(c: [f32; 3]) -> String {
    let scale = if c.iter().any(|v| *v > 1.0) { 1.0 } else { 255.0 };
    let b = |v: f32| (v * scale).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

/// Whether a spawn lets this team use it. `TeamMask` is one bit per team.
fn team_can_use(teams: Teams, team: u32) -> bool {
    match teams {
        Teams::Mask(m) => m == 0 || (team >= 1 && m & (1 << (team - 1)) != 0),
        Teams::Flags { a, b } => (team == 1 && a) || (team != 1 && b),
    }
}

pub fn convert(sce: &Sce, source: &str, source_hash: &str) -> Result<Imported, Vec<String>> {
    let mut reasons: Vec<String> = crate::inventory::gaps(sce)
        .into_iter()
        .map(|g| if g.detail.is_empty() { g.feature.to_string() } else { format!("{} ({})", g.feature, g.detail) })
        .collect();
    let mut notes = Vec::new();
    let top = &sce.settings.top;
    let r = sce.resolve();

    let map = match &sce.map {
        Ok(m) if m.format == MapFormat::Json => Some(m),
        Ok(_) => {
            reasons.push("Reflex maps (the older map format) aren't playable yet".into());
            None
        }
        Err(_) => None, // already a gap
    };

    // The bots: one kind for now, standing still.
    let mut kind: Option<&Section> = None;
    let mut teams = Vec::new();
    for slot in &r.bots {
        teams.push(slot.team.and_then(|t| t.trim().parse::<u32>().ok()).unwrap_or(2));
        for c in &slot.choices {
            let Some(ch) = c.character else { continue };
            if ch.f32_or("MaxSpeed", 0.0) > 0.0 && !c.bot.is("NoDodging") {
                push_once(&mut reasons, "moving bots (coming with bot movement)");
            }
            match kind {
                None => kind = Some(ch),
                Some(k) if k.name() != ch.name() && !same_hitbox(k, ch) => {
                    push_once(&mut reasons, "more than one kind of bot");
                }
                _ => {}
            }
        }
    }
    if r.bots.is_empty() {
        reasons.push("no bots".into());
    }
    if r.bots.len() > 50 {
        reasons.push(format!("{} bots at once (the trainer allows 50)", r.bots.len()));
    }
    let shape = kind.map(|k| hitbox(k, &mut reasons, &mut notes));

    let (Some(map), Some(bot), Some(weapon), Some((shape, radius, height)), true) =
        (map, kind, r.weapon, shape, reasons.is_empty())
    else {
        if reasons.is_empty() {
            reasons.push("no player, weapon or bot to play with".into());
        }
        return Err(reasons);
    };

    let map_scale = top.f32_or("MapScale", 1.0);
    let world = world(map, map_scale, r.player, bot, &teams, &mut reasons, &mut notes);
    if !reasons.is_empty() {
        return Err(reasons);
    }
    let Some(mut world) = world else { return Err(reasons) };
    world.source = source.into();
    world.source_hash = source_hash.into();
    world.accuracy_mult = match (top.is("ScoreMultAccuracy"), top.is("MultSqrtAcc")) {
        (true, true) => AccuracyMult::Sqrt,
        (true, false) => AccuracyMult::Linear,
        _ => AccuracyMult::None,
    };
    if top.is("LockFOVRange") {
        let lo = top.f32_or("LockedFOVMin", 0.0);
        let hi = top.f32_or("LockedFOVMax", 180.0);
        if lo > 0.0 && hi >= lo {
            world.fov_range = Some([lo, hi]);
        }
    }

    let interval = weapon.f32_or("TimeBetweenShots", 0.0);
    let category = match top.str("AimTypeTag") {
        "Tracking" => Category::Tracking,
        "Target Switching" => Category::Switching,
        _ => Category::Clicking,
    };
    let respawn = (bot.f32_or("MinRespawnDelay", 0.0) + bot.f32_or("MaxRespawnDelay", 0.0)) / 2.0;
    let mut scenario = Scenario {
        name: sce.name().trim().to_string(),
        description: top.str("Description").replace("[nl]", "\n"),
        author: top.str("Author").to_string(),
        category,
        // Left unset so imported runs don't count toward the program's skills.
        skill: Skill::None,
        tags: vec!["kovaaks".into()],
        timing: Timing { duration_s: top.f32_or("Timelimit", 60.0), ..Timing::default() },
        weapon: Weapon {
            kind: WeaponKind::Hitscan,
            trigger: if weapon.str("Category") == "FullyAuto" { Trigger::Hold } else { Trigger::Click },
            fire_rate: if interval > 0.0 { (1.0 / interval).min(100.0) } else { 0.0 },
            damage: weapon.f32_or("DamagePerShot", 1.0).max(0.001),
            ..Weapon::default()
        },
        scoring: Scoring {
            per_kill: top.f32_or("ScorePerKill", 0.0),
            per_hit: top.f32_or("ScorePerHit", 0.0),
            per_damage: top.f32_or("ScorePerDamage", 0.0),
            per_miss: -top.f32_or("ScoreLossPerMiss", 0.0),
            per_timeout: 0.0,
        },
        world: Some(world),
        ..Scenario::default()
    };
    let t = &mut scenario.targets;
    t.count = r.bots.len() as u32;
    t.shape = shape;
    t.radius = radius;
    t.height = height;
    t.health = bot.f32_or("MaxHealth", 100.0).max(0.001);
    t.respawn_delay_s = respawn.max(0.0);
    t.health_bar = !top.is("BlockHealthbars") && t.health > scenario.weapon.damage;
    if let Some(c) = bot.vec3("EnemyBodyColor").filter(|c| c.iter().any(|v| *v > 0.0)) {
        t.color = hex_color(c);
    }
    if bot.f32_or("HealthRegenPerSec", 0.0) > 0.0 {
        notes.push("bots don't regenerate health yet".into());
    }
    if weapon.f32_or("HitscanRadius", 0.0) > 1.0 {
        notes.push("the weapon's hit radius is left out".into());
    }
    let problems = scenario.problems();
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Imported { scenario, notes })
}

fn push_once(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.into());
    }
}

fn same_hitbox(a: &Section, b: &Section) -> bool {
    ["MainBBType", "MainBBRadius", "MainBBHeight", "MaxHealth"].iter().all(|k| a.str(k) == b.str(k))
}

/// The bot's hitbox as the sim's shape, radius and height, in meters.
fn hitbox(ch: &Section, reasons: &mut Vec<String>, notes: &mut Vec<String>) -> (Shape, f32, f32) {
    let radius = ch.f32_or("MainBBRadius", 0.0) / UU_PER_M;
    let height = ch.f32_or("MainBBHeight", 0.0) / UU_PER_M;
    if !(0.01..=10.0).contains(&radius) {
        reasons.push(format!("a bot radius of {:.2} m (the trainer allows 0.01–10 m)", radius));
    }
    match ch.str("MainBBType") {
        "Spheroid" => {
            if (height - 2.0 * radius).abs() > 0.05 * radius.max(1e-6) * 2.0 {
                notes.push("bots are stretched spheres in KovaaK's; here they're round".into());
            }
            (Shape::Sphere, radius, 2.0 * radius)
        }
        "Cylindrical" => {
            notes.push("cylinder bots are played as capsules (rounded ends)".into());
            if height >= 2.0 * radius {
                (Shape::Capsule, radius, height)
            } else {
                (Shape::Sphere, radius, 2.0 * radius)
            }
        }
        other => {
            reasons.push(format!("{other} hitboxes"));
            (Shape::Sphere, radius, 2.0 * radius)
        }
    }
}

/// Spawn points, offsets and walls, moved so the player's eye is at the origin
/// facing -Z.
fn world(
    map: &Map,
    map_scale: f32,
    player: Option<&Section>,
    bot: &Section,
    teams: &[u32],
    reasons: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> Option<World> {
    let Some(start) = map.spawns.iter().find(|s| team_can_use(s.teams, PLAYER_TEAM)) else {
        reasons.push("the map has no spawn point for the player".into());
        return None;
    };
    let player_offset = player.map_or(Vec3::ZERO, |p| {
        let lo = axes(p.vec3("SpawnOffsetMin").unwrap_or_default());
        let hi = axes(p.vec3("SpawnOffsetMax").unwrap_or_default());
        (lo + hi) / 2.0 / UU_PER_M + axes(p.vec3("CameraOffset").unwrap_or_default()) / UU_PER_M
    });
    let eye = map_point(start.position, map_scale) + player_offset;
    // JSON rotations are written roll, pitch, yaw; turn the map so the player's
    // starting yaw looks along -Z.
    let yaw = start.angles.map_or(0.0, |a| a[2]).to_radians();
    let turn = glam::Quat::from_rotation_y(yaw);
    let place = |p: Vec3| turn * (p - eye);

    let bot_teams: Vec<u32> = if teams.is_empty() { vec![2] } else { teams.to_vec() };
    let mut spawns = Vec::new();
    let mut volumes = 0;
    for s in &map.spawns {
        let usable = bot_teams.iter().any(|t| team_can_use(s.teams, *t))
            && (s.characters.is_empty() || s.characters.iter().any(|c| c.eq_ignore_ascii_case(bot.name())));
        if !usable || s.weight <= 0.0 {
            continue;
        }
        if s.volume.is_some() {
            volumes += 1;
        }
        // The offset runs along the spawn point's own axes: Unreal right, up and back.
        let q = ue_rotation(s.angles.unwrap_or_default());
        let axis = |v: [f32; 3]| (turn * axes((q * glam::Vec3::from_array(v)).to_array())).to_array();
        spawns.push(WorldSpawn {
            pos: place(map_point(s.position, map_scale)).to_array(),
            weight: s.weight,
            axes: [axis([0.0, 1.0, 0.0]), axis([0.0, 0.0, 1.0]), axis([-1.0, 0.0, 0.0])],
        });
    }
    if spawns.is_empty() {
        reasons.push("the map has no spawn point the bots can use".into());
        return None;
    }
    if volumes > 0 {
        notes.push("spawn volumes are played as their center point".into());
    }
    // Each axis of the offset is picked between the two values, so their order
    // doesn't matter (ww5t writes min above max).
    let a = axes(bot.vec3("SpawnOffsetMin").unwrap_or_default()) / UU_PER_M;
    let b = axes(bot.vec3("SpawnOffsetMax").unwrap_or_default()) / UU_PER_M;
    let mut meshes = Vec::new();
    let mut skipped = std::collections::BTreeSet::new();
    for solid in &map.solids {
        match solid {
            Solid::Primitive { mesh, place: p } if mesh.eq_ignore_ascii_case("Cube") => {
                meshes.push(cube(p, map_scale, &place));
            }
            Solid::Primitive { mesh, .. } => {
                skipped.insert(mesh.clone());
            }
            Solid::Mesh { place: p, vertices, triangles } => {
                let positions =
                    vertices.iter().map(|v| place(map_point(transform(p, *v), map_scale)).to_array()).collect();
                meshes.push(WorldMesh { positions, triangles: triangles.clone(), color: WALL_COLOR.into() });
            }
            Solid::Polyhedron { .. } => {}
        }
    }
    if !skipped.is_empty() {
        notes.push(format!(
            "some walls aren't drawn yet ({}); they don't change where bots appear",
            skipped.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    Some(World {
        spawns,
        offset_min: a.to_array(),
        offset_max: b.to_array(),
        blocked_radius: bot.f32_or("BlockedSpawnRadius", 0.0).max(0.0) / UU_PER_M,
        meshes,
        ..World::default()
    })
}

/// A JSON rotation (roll, pitch, yaw in degrees, as written) as a rotation of
/// Unreal coordinates: yaw turns forward toward right, pitch raises it toward up.
fn ue_rotation(r: [f32; 3]) -> glam::Quat {
    let [roll, pitch, yaw] = r.map(f32::to_radians);
    glam::Quat::from_rotation_z(yaw) * glam::Quat::from_rotation_y(-pitch) * glam::Quat::from_rotation_x(roll)
}

/// A local point of a placed brush, in Unreal map units: scaled, rotated, then
/// moved to its location.
fn transform(p: &Placement, v: [f32; 3]) -> [f32; 3] {
    let scaled = glam::Vec3::new(v[0] * p.scale[0], v[1] * p.scale[1], v[2] * p.scale[2]);
    (ue_rotation(p.rotation) * scaled + glam::Vec3::from_array(p.location)).to_array()
}

/// A `Cube` brush: 100 units per scale step from its lowest corner.
fn cube(p: &Placement, map_scale: f32, place: &impl Fn(Vec3) -> Vec3) -> WorldMesh {
    let corners: Vec<[f32; 3]> = (0..8)
        .map(|i| {
            let c = [(i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32].map(|v| v * CUBE_UU);
            place(map_point(transform(p, c), map_scale)).to_array()
        })
        .collect();
    // Two triangles per face; the engine draws both sides, so winding doesn't matter.
    let faces = [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]];
    let triangles = faces.iter().flat_map(|f| [[f[0], f[1], f[2]], [f[0], f[2], f[3]]]).collect();
    WorldMesh { positions: corners, triangles, color: WALL_COLOR.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreal_axes_become_the_sims() {
        // Forward, right, up.
        assert_eq!(axes([1.0, 0.0, 0.0]), Vec3::NEG_Z);
        assert_eq!(axes([0.0, 1.0, 0.0]), Vec3::X);
        assert_eq!(axes([0.0, 0.0, 1.0]), Vec3::Y);
        assert_eq!(map_point([200.0, 0.0, 0.0], 4.0), Vec3::new(0.0, 0.0, -8.0));
    }

    #[test]
    fn rotations_turn_forward_right_and_up() {
        let fwd = glam::Vec3::X;
        // Yaw 90: facing right. Pitch -15: looking down.
        assert!((ue_rotation([0.0, 0.0, 90.0]) * fwd - glam::Vec3::Y).length() < 1e-5);
        assert!((ue_rotation([0.0, -15.0, 0.0]) * fwd).z < 0.0);
    }

    #[test]
    fn team_masks_are_one_bit_per_team() {
        assert!(team_can_use(Teams::Mask(1), 1));
        assert!(!team_can_use(Teams::Mask(1), 2));
        assert!(team_can_use(Teams::Mask(2), 2));
        assert!(team_can_use(Teams::Mask(3), 2));
        assert!(team_can_use(Teams::Flags { a: false, b: true }, 2));
    }

    #[test]
    fn colors_in_either_scale() {
        assert_eq!(hex_color([1.0, 0.0, 0.0]), "#ff0000");
        assert_eq!(hex_color([255.0, 128.0, 0.0]), "#ff8000");
    }

    #[test]
    fn a_cube_spans_100_units_per_scale_step_from_its_corner() {
        let p = Placement { location: [100.0, -50.0, 0.0], rotation: [0.0; 3], scale: [2.0, 1.0, 1.0] };
        let m = cube(&p, 1.0, &|v| v);
        let xs: Vec<f32> = m.positions.iter().map(|q| q[2]).collect();
        // Unreal X 100..300 is sim Z -1..-3 m.
        assert!(xs.iter().all(|z| (*z + 1.0).abs() < 1e-5 || (*z + 3.0).abs() < 1e-5), "{xs:?}");
        assert_eq!(m.triangles.len(), 12);
    }
}
