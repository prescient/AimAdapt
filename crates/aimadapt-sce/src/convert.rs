//! Turns a `.sce` file into a trainer [`Scenario`] with a [`World`]: the map's
//! spawn points and walls, the bots' hitbox, health and movement, the weapon's fire
//! rate and damage, and KovaaK's score rules (BACKLOG section 9, phases 2 and 3).
//! Anything the trainer can't play comes back as reasons, in plain words, so the
//! app can say why.
//!
//! Units and axes. KovaaK's (Unreal) uses centimeters with X forward, Y right and
//! Z up. The sim uses meters with -Z forward, +X right and +Y up, with the player's
//! eye at the origin facing -Z. Map positions and walls are multiplied by the
//! scenario's `MapScale`; character sizes, speeds and spawn offsets are not (for
//! character heights that's checked against KovaaK's, at map scale 1 and 4).
//!
//! Reflex maps (the older format) are in centimeters too, with X right, Y up and Z
//! forward. On both formats KovaaK's puts a character's middle its whole height
//! above the spawn point it appears at.

use std::path::Path;

use aimadapt_sim::motion::{Blocked, Dodge, Flight, HitTurn, Jetpack, Motion, Start};
use aimadapt_sim::scenario::{Category, Scoring, Shape, Skill, Timing, Trigger, Weapon, WeaponKind};
use aimadapt_sim::world::{AccuracyMult, PlayerBlock, World, WorldBox, WorldKind, WorldMesh, WorldSpawn};
use aimadapt_sim::{Scenario, Vec3};

mod abilities;
mod helpers;
mod rules;

use crate::inventory::Gap;
use crate::map::{Map, MapFormat, Placement, Solid, Teams};
use crate::scenario::{BotChoice, BotSlot, Sce};
use crate::settings::Section;

/// Unreal units in a meter.
const UU_PER_M: f32 = 100.0;
/// The fall of a character with `Gravity=1`, in m/s². Measured in KovaaK's from how
/// high and how long bots hop: 800 units/s², where Earth's would be 980.
const GRAVITY_M_S2: f32 = 8.0;
/// A JSON `Cube` brush is this many units on a side at scale 1, placed from its
/// lowest corner.
const CUBE_UU: f32 = 100.0;
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
pub fn import(path: &Path) -> Result<Imported, Vec<Gap>> {
    let bytes = std::fs::read(path)
        .map_err(|e| vec![Gap::planned("an unreadable file", format!("{}: {e}", path.display()))])?;
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

/// A Reflex map position (X right, Y up, Z forward) in meters, in the sim's axes.
fn reflex_point(v: [f32; 3], map_scale: f32) -> Vec3 {
    Vec3::new(v[0], v[1], -v[2]) * map_scale / UU_PER_M
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
        // `checked_shl`: a team above 32 has no bit, and `1 << 32` overflows.
        Teams::Mask(m) => m == 0 || (team >= 1 && 1u32.checked_shl(team - 1).is_some_and(|bit| m & bit != 0)),
        Teams::Flags { a, b } => (team == 1 && a) || (team != 1 && b),
    }
}

pub fn convert(sce: &Sce, source: &str, source_hash: &str) -> Result<Imported, Vec<Gap>> {
    let mut reasons: Vec<Gap> = crate::inventory::gaps(sce);
    let mut notes = Vec::new();
    let top = &sce.settings.top;
    let r = sce.resolve();

    let map = sce.map.as_ref().ok(); // a bad map is already a gap
    // Most scenarios put the player on team 1, but not all (Pasu's is team 2).
    let player_team = crate::scenario::player_team(top);
    let other_team = if player_team == 1 { 2 } else { 1 };

    rule_reasons(top, &mut reasons);

    // Bots on the player's team push the targets around; they aren't targets.
    let helpers = helpers::plan(&r, player_team, &mut reasons, &mut notes);
    let targets: Vec<&BotSlot> = r.bots.iter().filter(|s| !s.is_helper(player_team)).collect();
    let kill_limit = rules::kill_limit(top, &r.bots, player_team, &mut reasons);
    let pushed = !helpers.helpers.is_empty();
    let Bots { kinds, slots, rotations, teams, moving } = bots(&targets, other_team, pushed, &mut reasons);
    let kind = kinds.first().map(|(ch, _)| *ch);
    let motion = kinds.first().and_then(|(_, m)| m.clone());
    if targets.is_empty() {
        reasons.push(Gap::planned("no bots", ""));
    }
    if targets.len() > 50 {
        reasons.push(Gap::planned("too many bots", format!("{} at once; the trainer allows 50", targets.len())));
    }
    let shape = kind.map(|k| hitbox(k, &mut reasons));
    // The other kinds' hitboxes, checked the same way.
    let others: Vec<(Shape, f32, f32)> = kinds.iter().skip(1).map(|(ch, _)| hitbox(ch, &mut reasons)).collect();
    // Several kinds can give the same note; keep each once, in order.
    let mut seen = std::collections::HashSet::new();
    notes.retain(|n| seen.insert(n.clone()));
    if !top.f32("MapScale").is_none_or(|s| s > 0.0) {
        reasons.push(Gap::planned("a map scale of 0 or below", ""));
    }

    let (Some(map), Some(bot), Some(weapon), Some((shape, radius, height))) = (map, kind, r.weapon, shape) else {
        if reasons.is_empty() {
            reasons.push(Gap::planned("no player, weapon or bot to play with", ""));
        }
        return Err(reasons);
    };

    let map_scale = top.f32_or("MapScale", 1.0);
    let characters: Vec<&Section> = kinds.iter().map(|(ch, _)| *ch).collect();
    let mut spawn_characters = Vec::new();
    let world = world(
        map,
        map_scale,
        r.player,
        &characters,
        spawn_rise(bot),
        player_team,
        &teams,
        &helpers,
        &mut spawn_characters,
        &mut reasons,
        &mut notes,
    );
    // The map's reasons come with the earlier ones; the steps below need a clean file.
    if !reasons.is_empty() {
        return Err(reasons);
    }
    let Some(mut world) = world else { return Err(reasons) };
    if moving {
        notes
            .push("bots move by their dodge profiles; how closely that matches KovaaK's is still to be checked".into());
        if kinds.iter().any(|(_, m)| m.as_ref().is_some_and(|m| m.abilities.is_some())) {
            notes.push(
                "bots use their movement abilities (dashes and jumps), timed as measured in KovaaK's; there a \
                 dash now and then comes later than its settings say"
                    .into(),
            );
        }
        if characters.iter().any(|ch| ch.is("HasJetpack")) {
            notes.push("bots' jetpacks thrust whenever they'd jump; how long they hold it is a guess".into());
        }
        if characters.iter().any(|ch| ch.is("CanCrouch")) && crouches(&targets) {
            notes.push("bots don't crouch yet".into());
        }
    } else if all_hidden(&world, radius) {
        // Bots that stand still where they can't be seen: something in KovaaK's
        // brings them out that isn't modelled (Skeet Tracking's, behind its walls).
        return Err(vec![Gap::planned("every place a bot appears is behind a wall", "")]);
    }
    if kinds.len() > 1 {
        // Several kinds: each with its own hitbox, health, look and movement, on the
        // spawn points its character may use.
        let sizes = std::iter::once((shape, radius, height)).chain(others);
        add_kinds(&mut world, &kinds, sizes, &spawn_characters)?;
        world.slots = slots;
        if rotations.iter().any(Option::is_some) {
            world.rotations = rotations;
        }
    } else {
        world.motion = motion;
    }
    world_rules(&mut world, top, source, source_hash);
    rules::score_notes(top, &mut notes);
    if kinds.len() == 1 {
        world.invincible |= characters[0].is("InvincibleBots");
    }
    if !world.pushers.is_empty() {
        notes.push(format!(
            "helper bots push the targets around ({} of them); they aren't drawn or scored",
            world.pushers.len()
        ));
    }

    let mut scenario = Scenario { world: Some(world), ..base_scenario(sce, top, weapon) };
    scenario.timing.kill_limit = kill_limit;
    let t = &mut scenario.targets;
    t.count = targets.len() as u32;
    t.shape = shape;
    t.radius = radius;
    t.height = height;
    t.health = bot.f32_or("MaxHealth", 100.0).max(0.001);
    t.respawn_delay_s = respawn_delay(bot);
    let damage = scenario.weapon.damage;
    t.health_bar =
        !top.is("BlockHealthbars") && characters.iter().any(|ch| ch.f32_or("MaxHealth", 100.0).max(0.001) > damage);
    if let Some(c) = body_color(bot) {
        t.color = c;
    }
    // Regen counts from the last hit. Negative regen drains the bot until it goes
    // (VT Aether's last 19 s).
    t.regen_per_s = bot.f32_or("HealthRegenPerSec", 0.0);
    t.regen_delay_s = bot.f32_or("HealthRegenDelay", 0.0).clamp(0.0, 10.0);
    if weapon.f32_or("HitscanRadius", 0.0) > 1.0 {
        notes.push("the weapon's hit radius is left out".into());
    }
    let problems = scenario.problems();
    if !problems.is_empty() {
        return Err(problems.into_iter().map(|p| Gap::planned("a value the trainer doesn't allow", p)).collect());
    }
    Ok(Imported { scenario, notes })
}

/// Rules the trainer refuses: a time scale it can't run, and the kill efficiency score.
fn rule_reasons(top: &Section, reasons: &mut Vec<Gap>) {
    let (lo, hi) = aimadapt_sim::world::TIME_SCALE_RANGE;
    if let Some(v) = top.f32("Timescale").filter(|v| !(lo..=hi).contains(v)) {
        let detail = format!("Timescale is {v}; the trainer allows {lo} to {hi}");
        push_once(reasons, Gap { feature: "time scale", detail, beyond_plan: true });
    }
    if top.is("ScoreMultKillEfficiency") {
        // No KovaaK's result file has a score to check a formula against.
        let detail = "ScoreMultKillEfficiency; no result file from KovaaK's exists to check a formula against";
        reasons.push(Gap {
            feature: "a score multiplied by kill efficiency",
            detail: detail.into(),
            beyond_plan: true,
        });
    }
}

// The bots. Each bot that looks or moves differently is a kind of its own; each
// entry of `AddedBots` is a slot, filled by one of its kinds by weight, or in turn
// from a rotation that isn't randomized (slots naming the same one share turns).
struct Bots<'a> {
    kinds: Vec<(&'a Section, Option<Motion>)>,
    slots: Vec<Vec<(u32, f32)>>,
    rotations: Vec<Option<u32>>,
    teams: Vec<u32>,
    /// Some bot moves by itself (not only falls and is pushed).
    moving: bool,
}

/// The target slots' bots. With `pushed` (the file has helper bots) every bot gets a
/// motion, since a bot has to have one to fall or be pushed.
fn bots<'a>(target_slots: &[&BotSlot<'a>], other_team: u32, pushed: bool, reasons: &mut Vec<Gap>) -> Bots<'a> {
    let mut kinds: Vec<(&Section, Option<Motion>)> = Vec::new();
    let mut slots: Vec<Vec<(u32, f32)>> = Vec::new();
    let mut rotations: Vec<Option<u32>> = Vec::new();
    let mut rotation_names: Vec<String> = Vec::new();
    let mut teams = Vec::new();
    let mut moving = false;
    for slot in target_slots {
        // Team 0 (some Pasu files) plays as the player's opponents.
        let team = slot.team.and_then(|t| t.trim().parse::<u32>().ok()).filter(|t| *t > 0);
        teams.push(team.unwrap_or(other_team));
        let mut choices = Vec::new();
        for c in &slot.choices {
            let Some(ch) = c.character else { continue };
            // Movement abilities play. A target's melee ability doesn't: the knockbacks
            // that matter are the helper bots'.
            abilities::reasons(c, reasons);
            let moves = moves(c, ch);
            moving |= moves;
            if pushed && !moves && ch.is("IsFlyer") {
                // Not measured in KovaaK's: whether such a bot hovers or falls when it is pushed.
                let detail = format!("a flying target that doesn't move by itself: {}", ch.name());
                push_once(reasons, Gap { feature: "helper bots", detail, beyond_plan: true });
            }
            let m = if moves { Some(bot_motion(c, ch)) } else { pushed.then(|| still_motion(c, ch)) };
            let k = match kinds.iter().position(|(k, km)| same_kind(k, ch) && *km == m) {
                Some(k) => k,
                None => {
                    kinds.push((ch, m));
                    kinds.len() - 1
                }
            };
            choices.push((k as u32, c.weight.max(0.0)));
        }
        if !choices.is_empty() {
            let in_turn = (slot.in_order && choices.len() > 1).then(|| {
                let name = slot.reference.trim().to_ascii_lowercase();
                let at = rotation_names.iter().position(|n| *n == name).unwrap_or_else(|| {
                    rotation_names.push(name);
                    rotation_names.len() - 1
                });
                at as u32
            });
            rotations.push(in_turn);
            slots.push(choices);
        }
    }
    Bots { kinds, slots, rotations, teams, moving }
}

/// The kinds of bot, each on the spawn points its character may use (all of them
/// when the map doesn't say).
fn add_kinds(
    world: &mut World,
    kinds: &[(&Section, Option<Motion>)],
    sizes: impl Iterator<Item = (Shape, f32, f32)>,
    spawn_characters: &[Vec<String>],
) -> Result<(), Vec<Gap>> {
    for ((ch, m), (shape, radius, height)) in kinds.iter().zip(sizes) {
        let spawns: Vec<u32> = spawn_characters
            .iter()
            .enumerate()
            .filter(|(_, names)| names.is_empty() || names.iter().any(|n| n.eq_ignore_ascii_case(ch.name())))
            .map(|(i, _)| i as u32)
            .collect();
        if spawns.is_empty() {
            return Err(vec![Gap::planned("the map has no spawn point a bot can use", ch.name())]);
        }
        let all = spawns.len() == world.spawns.len();
        world.kinds.push(WorldKind {
            shape,
            radius,
            height,
            health: ch.f32_or("MaxHealth", 100.0).max(0.001),
            color: body_color(ch).unwrap_or_else(|| WorldKind::default().color),
            regen_per_s: ch.f32_or("HealthRegenPerSec", 0.0),
            regen_delay_s: ch.f32_or("HealthRegenDelay", 0.0).clamp(0.0, 10.0),
            respawn_delay_s: respawn_delay(ch),
            invincible: ch.is("InvincibleBots"),
            motion: m.clone(),
            spawns: if all { Vec::new() } else { spawns },
        });
    }
    Ok(())
}

/// The scenario-wide rules the world keeps: where it came from, accuracy scoring,
/// invincible bots, the time scale, the score for time left, damage efficiency and a locked
/// field of view.
fn world_rules(world: &mut World, top: &Section, source: &str, source_hash: &str) {
    world.source = source.into();
    world.source_hash = source_hash.into();
    world.accuracy_mult = match (top.is("ScoreMultAccuracy"), top.is("MultSqrtAcc")) {
        (true, true) => AccuracyMult::Sqrt,
        (true, false) => AccuracyMult::Linear,
        _ => AccuracyMult::None,
    };
    world.invincible = top.is("InvincibleBots");
    // Out-of-range values are refused in `rule_reasons`.
    world.time_scale = top.f32("Timescale").unwrap_or(1.0);
    world.time_score = top.f32_or("ScorePerTime", 0.0);
    world.damage_efficiency = top.is("ScoreMultDamageEfficiency");
    if top.is("LockFOVRange") {
        let lo = top.f32_or("LockedFOVMin", 0.0);
        let hi = top.f32_or("LockedFOVMax", 180.0);
        if lo > 0.0 && hi >= lo {
            world.fov_range = Some([lo, hi]);
        }
    }
}

/// The scenario without its world or targets: name, timing, weapon and scoring.
fn base_scenario(sce: &Sce, top: &Section, weapon: &Section) -> Scenario {
    let interval = weapon.f32_or("TimeBetweenShots", 0.0);
    let category = match top.str("AimTypeTag") {
        "Tracking" => Category::Tracking,
        "Target Switching" => Category::Switching,
        _ => Category::Clicking,
    };
    Scenario {
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
        ..Scenario::default()
    }
}

/// How far above the point it appears at KovaaK's puts a character's middle: its
/// whole height, on both map formats. Measured with test maps for players 2 to 300
/// units tall, radius 1 to 55, map scale 1 and 4, standing or floating, and still
/// bots on spawn points and volumes. So its feet are half its height above the point.
fn rise(height: f32) -> Vec3 {
    Vec3::Y * height
}

/// A map's solids as the world's meshes and wall boxes, with the shapes it had to
/// leave out or guess at.
struct MapWalls {
    meshes: Vec<WorldMesh>,
    walls: Vec<WorldBox>,
    skipped: std::collections::BTreeSet<String>,
    guessed: std::collections::BTreeSet<String>,
}

fn map_walls(map: &Map, map_scale: f32) -> MapWalls {
    let mut meshes = Vec::new();
    let mut walls = Vec::new();
    let mut skipped = std::collections::BTreeSet::new();
    let mut guessed = std::collections::BTreeSet::new();
    let same = |p: Vec3| p;
    for solid in &map.solids {
        match solid {
            Solid::Primitive { mesh, place: p, name } if mesh.eq_ignore_ascii_case("Cube") => {
                let Some((decor, clip)) = brush_kind(name) else { continue };
                meshes.push(WorldMesh { decor, clip, ..cube(p, map_scale, &same) });
                if !decor {
                    walls.push(cube_box(p, map_scale, &same));
                }
            }
            Solid::Primitive { mesh, place: p, name } => {
                let Some((decor, clip)) = brush_kind(name) else { continue };
                let Some(shape) = crate::shapes::primitive(mesh) else {
                    if !clip {
                        skipped.insert(mesh.clone());
                    }
                    continue;
                };
                if !shape.solid && !decor && !clip {
                    guessed.insert(mesh.clone());
                }
                let positions =
                    shape.vertices.iter().map(|v| map_point(transform(p, *v), map_scale).to_array()).collect();
                meshes.push(WorldMesh {
                    positions,
                    triangles: shape.triangles,
                    color: WALL_COLOR.into(),
                    decor: decor || !shape.solid,
                    clip,
                });
            }
            Solid::Mesh { place: p, vertices, triangles, name } => {
                let Some((decor, clip)) = brush_kind(name) else { continue };
                // The editor saves a brush's own triangles divided by the map scale
                // (a cube's run 0 to 100 / MapScale), so they're scaled back first.
                let positions = vertices
                    .iter()
                    .map(|v| map_point(transform(p, v.map(|c| c * map_scale)), map_scale).to_array())
                    .collect();
                meshes.push(WorldMesh {
                    positions,
                    triangles: triangles.clone(),
                    color: WALL_COLOR.into(),
                    decor,
                    clip,
                });
            }
            Solid::Polyhedron { vertices, faces, materials } => {
                // A convex brush: each face is a fan of triangles. Clip brushes are
                // invisible walls that keep characters in; weapon clips only stop
                // shots, which these scenarios don't need, so they're left out.
                let all = |f: &dyn Fn(&str) -> bool| !materials.is_empty() && materials.iter().all(|m| f(m));
                if all(&|m| m.ends_with("editor_weaponclip")) {
                    continue;
                }
                let clip = all(&|m| m.contains("editor_") && m.ends_with("clip"));
                let positions = vertices.iter().map(|v| reflex_point(*v, map_scale).to_array()).collect();
                let triangles = faces
                    .iter()
                    .flat_map(|f| (1..f.len().saturating_sub(1)).map(|k| [f[0], f[k], f[k + 1]].map(|i| i as u32)))
                    .collect();
                meshes.push(WorldMesh { positions, triangles, color: WALL_COLOR.into(), decor: false, clip });
            }
        }
    }
    MapWalls { meshes, walls, skipped, guessed }
}

// Where the player stands: lifted onto a floor it overlaps, or pulled down onto
// one within its step-up height, as Unreal's walking movement does (both measured
// in KovaaK's). The eye is the camera offset above its middle, or with a head, above
// the top of its body (and half the head's offset higher: one measurement).
fn player_eye(player: Option<&Section>, start: Vec3, meshes: &[WorldMesh]) -> Vec3 {
    let mut body = start;
    let mut camera = Vec3::ZERO;
    if let Some(p) = player {
        let lo = axes(p.vec3("SpawnOffsetMin").unwrap_or_default());
        let hi = axes(p.vec3("SpawnOffsetMax").unwrap_or_default());
        let height = p.f32_or("MainBBHeight", 0.0).max(0.0) / UU_PER_M;
        let head = p.is("MainBBHasHead");
        // The capsule Unreal stands on floors: a head reaches its radius further each way.
        let head_radius = if head { p.f32_or("MainBBHeadRadius", 0.0).max(0.0) / UU_PER_M } else { 0.0 };
        let half = height / 2.0 + head_radius;
        let radius = (p.f32_or("MainBBRadius", 0.0).max(0.0) / UU_PER_M).min(half);
        let step_up = p.f32_or("StepUpHeight", 0.0).max(0.0) / UU_PER_M;
        body += (lo + hi) / 2.0 / UU_PER_M + rise(height);
        if radius > 0.0 {
            body = aimadapt_sim::collide::Collider::for_bots(meshes).stand(body, half - radius, radius, step_up);
        }
        camera = axes(p.vec3("CameraOffset").unwrap_or_default()) / UU_PER_M;
        if head {
            camera.y += height / 2.0 + p.f32_or("MainBBHeadOffset", 0.0) / 2.0 / UU_PER_M;
        }
    }
    body + camera
}

/// Whether a bot moves, unless `NoDodging` keeps it still: it runs (`MaxSpeed`),
/// flies up and down (`IsFlyer` with a flight speed: its dodge profile's jumps and
/// crouches move it), jumps (a jump speed and a dodge profile that jumps), or has a
/// movement ability that dashes or launches it.
fn moves(c: &BotChoice, ch: &Section) -> bool {
    if c.bot.is("NoDodging") {
        return false;
    }
    let flies =
        ch.is("IsFlyer") && (ch.f32_or("FlightVelocityUp", 0.0) > 0.0 || ch.f32_or("FlightVelocityDown", 0.0) > 0.0);
    let jumps =
        ch.f32_or("JumpVelocityMax", 0.0) > 0.0 && c.dodges.iter().any(|(d, _)| d.f32_or("JumpFrequency", 0.0) > 0.0);
    ch.f32_or("MaxSpeed", 0.0) > 0.0 || flies || jumps || abilities::moves(c)
}

/// Whether any bot's dodge profile crouches.
fn crouches(slots: &[&BotSlot]) -> bool {
    slots
        .iter()
        .flat_map(|s| &s.choices)
        .flat_map(|c| &c.dodges)
        .any(|(d, _)| d.f32_or("CrouchOnGroundFrequency", 0.0) > 0.0 || d.f32_or("CrouchInAirFrequency", 0.0) > 0.0)
}

/// A moving bot's character physics and dodge profiles, in meters and seconds.
fn bot_motion(c: &BotChoice, ch: &Section) -> Motion {
    let cm = |k: &str, default: f32| ch.f32_or(k, default).max(0.0) / UU_PER_M;
    let (lo, hi) = (ch.f32_or("JumpVelocityMin", 0.0), ch.f32_or("JumpVelocityMax", 0.0));
    let jump = if hi > 0.0 { [lo.clamp(0.0, hi), hi] } else { [ch.f32_or("JumpVelocity", 420.0).max(0.0); 2] };
    let flies =
        ch.is("IsFlyer") && (ch.f32_or("FlightVelocityUp", 0.0) > 0.0 || ch.f32_or("FlightVelocityDown", 0.0) > 0.0);
    // A terminal speed of a few centimeters a second isn't a real limit.
    let terminal = ch.f32_or("TerminalVelocity", 0.0);
    let total: f32 = c.dodges.iter().map(|(_, w)| w.max(0.0)).sum();
    Motion {
        max_speed: cm("MaxSpeed", 0.0),
        accel: cm("Acceleration", 2048.0),
        friction: ch.f32_or("Friction", 8.0).max(0.0),
        air_friction: ch.f32_or("AerialFriction", 0.0).max(0.0),
        braking: cm("BrakingDeceleration", 2048.0),
        braking_friction_factor: ch.f32_or("BrakingFrictionFactor", 2.0).max(0.0),
        gravity: ch.f32_or("Gravity", 1.0).max(0.0) * GRAVITY_M_S2,
        air_control: ch.f32_or("AirControl", 0.05).clamp(0.0, 1.0),
        jump_speed: jump.map(|v| v / UU_PER_M),
        air_jumps: ch.f32_or("AirJumpCount", 0.0).clamp(0.0, 100.0) as u32,
        air_jump_speed: cm("AirJumpVelocity", 0.0),
        terminal_speed: if terminal >= 10.0 { terminal / UU_PER_M } else { 0.0 },
        strafe_mult: ch.f32_or("StrafeSpeedMult", 1.0).max(0.0),
        back_mult: ch.f32_or("BackSpeedMult", 1.0).max(0.0),
        jetpack: ch.is("HasJetpack").then(|| Jetpack {
            thrust: cm("JetpackThrust", 6000.0),
            max_up: cm("JetpackMaxZVelocity", 400.0),
            air_control: ch.f32_or("JetpackAirControlWithThrust", 0.25).clamp(0.0, 1.0),
        }),
        flight: flies.then(|| Flight {
            up_speed: cm("FlightVelocityUp", 0.0),
            down_speed: cm("FlightVelocityDown", 0.0),
            up_accel: cm("FlightAccelUp", 800.0),
            down_accel: cm("FlightAccelDown", 800.0),
        }),
        bounce_off_walls: ch.is("BounceOffWalls"),
        bump_bots: !ch.is("DisableCharacterCollision"),
        still_until_hurt: c.bot.is("StandStillUntilHurt"),
        faces_player: !c.bot.is("NoAiming"),
        dodges: c.dodges.iter().map(|(d, w)| dodge(d, if total > 0.0 { *w } else { 1.0 })).collect(),
        change_dodge_s: [
            c.bot.f32_or("DodgeProfileMinChangeTime", 0.0).max(0.0),
            c.bot.f32_or("DodgeProfileMaxChangeTime", 0.0).max(0.0),
        ],
        random_dodges: c.bot.bool("RandomizeDodgeProfiles").unwrap_or(true),
        abilities: abilities::abilities(c, ch),
    }
}

/// The physics of a bot that does nothing by itself and is only pushed: it falls and
/// slides to a stop. It has no dodge profile, ability, jetpack or flight, and nothing
/// that makes it jump or wait until it is hurt. A bot gets this in a scenario with
/// helper bots when it wouldn't move on its own (a flier that wouldn't is refused).
fn still_motion(c: &BotChoice, ch: &Section) -> Motion {
    Motion {
        max_speed: 0.0,
        dodges: Vec::new(),
        abilities: None,
        jetpack: None,
        flight: None,
        still_until_hurt: false,
        ..bot_motion(c, ch)
    }
}

/// One dodge profile: when the bot strafes, advances, jumps and turns.
fn dodge(d: &Section, weight: f32) -> Dodge {
    let pair = |a: &str, b: &str, da: f32, db: f32| [d.f32_or(a, da).max(0.0), d.f32_or(b, db).max(0.0)];
    let start = |key: &str, minus: &str| {
        let v = d.str(key);
        if v.eq_ignore_ascii_case("Random") {
            Start::Random
        } else if v.eq_ignore_ascii_case(minus) {
            Start::Minus
        } else {
            Start::Plus
        }
    };
    let blocked = d.f32_or("BlockedMovementPercent", 0.0).clamp(0.0, 1.0);
    Dodge {
        weight: weight.max(0.0),
        strafe: d.bool("ToggleLeftRight").unwrap_or(true),
        strafe_s: pair("MinLRTimeChange", "MaxLRTimeChange", 0.5, 1.0),
        left_mult: d.f32_or("LeftStrafeTimeMult", 1.0).max(0.0),
        right_mult: d.f32_or("RightStrafeTimeMult", 1.0).max(0.0),
        strafe_pause_s: pair("StrafeSwapMinPause", "StrafeSwapMaxPause", 0.0, 0.0),
        start_side: start("InitialRightMovementState", "Left"),
        advance: d.is("ToggleForwardBack"),
        advance_s: pair("MinFBTimeChange", "MaxFBTimeChange", 0.5, 1.0),
        forward_mult: d.f32_or("ForwardTimeMult", 1.0).max(0.0),
        back_mult: d.f32_or("BackTimeMult", 1.0).max(0.0),
        start_forward: start("InitialForwardMovementState", "Back"),
        min_distance: d.f32_or("MinTargetDistance", 0.0).max(0.0) / UU_PER_M,
        max_distance: d.f32_or("MaxTargetDistance", 0.0).max(0.0) / UU_PER_M,
        jump_chance: d.f32_or("JumpFrequency", 0.0).clamp(0.0, 1.0),
        jump_s: pair("MinJumpTime", "MaxJumpTime", 0.3, 0.6),
        up_down_s: pair("ToggleUpDownMinTime", "ToggleUpDownMaxTime", 0.2, 0.5),
        up_down_pause_s: pair("UpDownSwapPauseMinTime", "UpDownSwapPauseMaxTime", 0.0, 0.0),
        hit_turn: d.is("DamageReactionChangesDirection").then(|| HitTurn {
            chance: 1.0 - d.f32_or("DamageReactionChanceToIgnore", 0.0).clamp(0.0, 1.0),
            delay_s: pair("DamageReactionMinimumDelay", "DamageReactionMaximumDelay", 0.125, 0.25),
            cooldown_s: d.f32_or("DamageReactionCooldown", 1.0).max(0.0),
            threshold: d.f32_or("DamageReactionThreshold", 0.0).max(0.0),
            reset_s: d.f32_or("DamageReactionResetTimer", 0.5).max(0.0),
        }),
        blocked: (blocked > 0.0 || d.is("CounterStrafeOnCollision")).then(|| Blocked {
            fraction: blocked,
            reaction_s: pair("BlockedMovementReactionMin", "BlockedMovementReactionMax", 0.0, 0.0),
            on_touch: d.is("CounterStrafeOnCollision"),
        }),
    }
}

fn push_once(v: &mut Vec<Gap>, g: Gap) {
    if !v.contains(&g) {
        v.push(g);
    }
}

fn same_hitbox(a: &Section, b: &Section) -> bool {
    ["MainBBType", "MainBBRadius", "MainBBHeight", "MaxHealth"].iter().all(|k| a.str(k) == b.str(k))
}

/// Whether two characters play the same: the same one, or alike in hitbox, health,
/// regen, color and respawn time.
fn same_kind(a: &Section, b: &Section) -> bool {
    a.name() == b.name()
        || same_hitbox(a, b)
            && [
                "EnemyBodyColor",
                "HealthRegenPerSec",
                "HealthRegenDelay",
                "MinRespawnDelay",
                "MaxRespawnDelay",
                "InvincibleBots",
            ]
            .iter()
            .all(|k| a.str(k) == b.str(k))
}

/// A character's body color, unless it's unset (black).
fn body_color(ch: &Section) -> Option<String> {
    ch.vec3("EnemyBodyColor").filter(|c| c.iter().any(|v| *v > 0.0)).map(hex_color)
}

/// Seconds before a character comes back: halfway between its least and most.
fn respawn_delay(ch: &Section) -> f32 {
    ((ch.f32_or("MinRespawnDelay", 0.0) + ch.f32_or("MaxRespawnDelay", 0.0)) / 2.0).max(0.0)
}

/// The bot's hitbox as the sim's shape, radius and height, in meters.
fn hitbox(ch: &Section, reasons: &mut Vec<Gap>) -> (Shape, f32, f32) {
    let radius = ch.f32_or("MainBBRadius", 0.0) / UU_PER_M;
    let height = ch.f32_or("MainBBHeight", 0.0) / UU_PER_M;
    if !(0.01..=10.0).contains(&radius) {
        reasons.push(Gap::planned("bot size", format!("a radius of {radius:.2} m; the trainer allows 0.01–10 m")));
    }
    match ch.str("MainBBType") {
        // A ball of radius R whatever the height is.
        "Spheroid" => (Shape::Sphere, radius, 2.0 * radius),
        kind @ ("Cylindrical" | "Cuboid") => {
            if height < 2.0 * radius {
                reasons.push(flat_hitbox(kind, radius, height));
            }
            (if kind == "Cuboid" { Shape::Box } else { Shape::Capsule }, radius, height)
        }
        other => {
            reasons.push(Gap::planned("hitbox shape", other));
            (Shape::Sphere, radius, 2.0 * radius)
        }
    }
}

/// The reason for a cylinder or cuboid bot flatter than it is wide: how KovaaK's draws and hits one
/// isn't known (one test showed a shape about as wide as it was tall).
fn flat_hitbox(kind: &str, radius: f32, height: f32) -> Gap {
    let name = if kind == "Cuboid" { "cuboid" } else { "cylinder" };
    Gap::planned(
        "hitbox shape",
        format!("a {name} bot flatter than it is wide (height {height:.2} m, radius {radius:.2} m)"),
    )
}

/// How far above its spawn point a bot's middle starts, in meters: its height, except a
/// spheroid, which starts the larger of its height and twice its radius (a flat one rests as a
/// ball).
fn spawn_rise(ch: &Section) -> f32 {
    let height = ch.f32_or("MainBBHeight", 0.0) / UU_PER_M;
    if ch.str("MainBBType") == "Spheroid" {
        height.max(2.0 * ch.f32_or("MainBBRadius", 0.0) / UU_PER_M)
    } else {
        height
    }
}

/// A box bot stays lined up with the map's axes, so it lines up with the sim's only when the
/// map's turn is a multiple of 90 degrees.
fn box_alignment(bots: &[&Section], yaw_deg: f32, reasons: &mut Vec<Gap>) {
    let off = yaw_deg.rem_euclid(90.0);
    let off = off.min(90.0 - off);
    if off > 0.5 && bots.iter().any(|b| b.str("MainBBType") == "Cuboid") {
        reasons.push(Gap::planned(
            "hitbox shape",
            format!(
                "box bots on a map where the player starts turned {:.0}° from the map's axes",
                yaw_deg.rem_euclid(360.0)
            ),
        ));
    }
}

/// Spawn points, offsets and walls, moved so the player's eye is at the origin
/// facing -Z.
#[allow(clippy::too_many_arguments)]
fn world(
    map: &Map,
    map_scale: f32,
    player: Option<&Section>,
    bots: &[&Section],
    bot_height: f32,
    player_team: u32,
    teams: &[u32],
    helpers: &helpers::Plan,
    spawn_characters: &mut Vec<Vec<String>>,
    reasons: &mut Vec<Gap>,
    notes: &mut Vec<String>,
) -> Option<World> {
    let reflex = map.format == MapFormat::Reflex;
    // The player's own start comes first if the map says which one it is. A helper
    // bot's spawn point isn't a start.
    let mut starts: Vec<_> = map
        .spawns
        .iter()
        .filter(|s| team_can_use(s.teams, player_team) && !helpers::is_reserved(s, helpers, player))
        .collect();
    starts.sort_by_key(|s| !s.initial);
    let Some(start) = starts.first().copied() else {
        reasons.push(Gap::planned("the map has no spawn point for the player", ""));
        return None;
    };
    let point = |v: [f32; 3]| if reflex { reflex_point(v, map_scale) } else { map_point(v, map_scale) };

    // The walls, first where the map has them: where the player stands depends on them.
    let MapWalls { mut meshes, mut walls, skipped, guessed } = map_walls(map, map_scale);

    let eye = player_eye(player, point(start.position), &meshes);
    // JSON rotations are written roll, pitch, yaw, and Reflex ones yaw first; turn
    // the map so the player's starting yaw looks along -Z.
    let yaw_of = |a: [f32; 3]| if reflex { a[0] } else { a[2] };
    let yaw = start.angles.map_or(0.0, yaw_of).to_radians();
    box_alignment(bots, yaw.to_degrees(), reasons);
    let turn = glam::Quat::from_rotation_y(yaw);
    let place = |p: Vec3| turn * (p - eye);
    for m in &mut meshes {
        for v in &mut m.positions {
            *v = place(Vec3::from_array(*v)).to_array();
        }
    }
    for w in &mut walls {
        w.corner = place(Vec3::from_array(w.corner)).to_array();
        w.edges = w.edges.map(|e| (turn * Vec3::from_array(e)).to_array());
    }

    let bot_teams: Vec<u32> =
        if teams.is_empty() { vec![if player_team == 1 { 2 } else { 1 }] } else { teams.to_vec() };
    // Where a spawn point is and which way it faces: its own right, up and back axes.
    // The offset runs along them. A Reflex yaw turns the same way as an Unreal one, so
    // both use its rotation.
    let frame = |s: &crate::map::Spawn| {
        let q = ue_rotation(match s.angles {
            Some(a) if reflex => [0.0, 0.0, a[0]],
            a => a.unwrap_or_default(),
        });
        let axis = |v: [f32; 3]| turn * axes((q * glam::Vec3::from_array(v)).to_array());
        helpers::Frame {
            at: place(point(s.position)),
            axes: [axis([0.0, 1.0, 0.0]), axis([0.0, 0.0, 1.0]), axis([-1.0, 0.0, 0.0])],
        }
    };
    let taken = helpers::assign(map, helpers, |s| team_can_use(s.teams, player_team), reasons)?;
    let mut spawns = Vec::new();
    let bot = bots[0];
    for (i, s) in map.spawns.iter().enumerate() {
        let usable = bot_teams.iter().any(|t| team_can_use(s.teams, *t))
            && (s.characters.is_empty()
                || s.characters.iter().any(|c| bots.iter().any(|b| c.eq_ignore_ascii_case(b.name()))));
        // A helper's spawn point isn't a target's.
        if !usable || s.weight <= 0.0 || taken.contains(&i) {
            continue;
        }
        let helpers::Frame { at, axes: [right, up, back] } = frame(s);
        let pos = at + rise(bot_height);
        // A volume is a box placed like a `Cube`: from its lowest corner, 100 units
        // per scale step, turned by its rotation. Bots rise from wherever in it they appear.
        let edges = s.volume.as_ref().map_or([[0.0; 3]; 3], |v| {
            let corner = |c: [f32; 3]| place(map_point(transform(v, c), map_scale)) - at;
            [corner([CUBE_UU, 0.0, 0.0]), corner([0.0, CUBE_UU, 0.0]), corner([0.0, 0.0, CUBE_UU])]
                .map(|e| e.to_array())
        });
        spawn_characters.push(s.characters.clone());
        spawns.push(WorldSpawn {
            pos: pos.to_array(),
            weight: s.weight,
            axes: [right, up, back].map(|a| a.to_array()),
            edges,
        });
    }
    if spawns.is_empty() {
        reasons.push(Gap::planned("the map has no spawn point the bots can use", ""));
        return None;
    }
    // Each axis of the offset is picked between the two values, so their order
    // doesn't matter (ww5t writes min above max).
    let a = axes(bot.vec3("SpawnOffsetMin").unwrap_or_default()) / UU_PER_M;
    let b = axes(bot.vec3("SpawnOffsetMax").unwrap_or_default()) / UU_PER_M;
    if !skipped.is_empty() {
        notes.push(format!(
            "some walls aren't drawn yet ({}); they don't change where bots appear",
            skipped.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if !guessed.is_empty() {
        notes.push(format!(
            "some walls are drawn from their names and may look different in KovaaK's ({}); shots and bots pass through them",
            guessed.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let frames: Vec<helpers::Frame> = taken.iter().map(|i| frame(&map.spawns[*i])).collect();
    let pushers = helpers::pushers(helpers, &frames, &meshes, &spawns, reasons);
    Some(World {
        spawns,
        pushers,
        offset_min: a.to_array(),
        offset_max: b.to_array(),
        blocked_radius: bot.f32_or("BlockedSpawnRadius", 0.0).max(0.0) / UU_PER_M,
        cluster: bot.is("InvertBlockedSpawn") && bot.f32_or("BlockedSpawnRadius", 0.0) > 0.0,
        meshes,
        walls,
        player_block: player.and_then(player_block),
        ..World::default()
    })
}

/// What a named JSON brush is: `None` to leave it out (a weapon clip only stops
/// shots, which these scenarios don't need), else whether it's drawn only
/// (`DefaultNoCollision`) and whether it's an invisible wall (`Clip`, `FullClip`).
fn brush_kind(name: &str) -> Option<(bool, bool)> {
    match name {
        "WeaponClip" => None,
        "DefaultNoCollision" => Some((true, false)),
        "Clip" | "FullClip" => Some((false, true)),
        _ => Some((false, false)),
    }
}

/// Whether every place a bot can appear is out of sight behind a wall.
fn all_hidden(w: &World, radius: f32) -> bool {
    let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
    !walls.is_empty()
        && w.spawns.iter().all(|s| {
            let middle = Vec3::from_array(s.pos) + s.edges.iter().map(|e| Vec3::from_array(*e) / 2.0).sum::<Vec3>();
            let d = middle.length();
            d > radius && walls.ray(Vec3::ZERO, middle / d, d - radius).is_some()
        })
}

/// The player profile's rules for where bots may appear, if it has any.
fn player_block(p: &Section) -> Option<PlayerBlock> {
    let b = PlayerBlock {
        radius: p.f32_or("BlockedSpawnRadius", 0.0).max(0.0) / UU_PER_M,
        fov_deg: p.f32_or("BlockSpawnFOV", 0.0).clamp(0.0, 360.0),
        distance: p.f32_or("BlockSpawnDistance", 0.0).max(0.0) / UU_PER_M,
        invert: p.is("InvertBlockedSpawn"),
    };
    (b.radius > 0.0 || b.fov_deg > 0.0).then_some(b)
}

/// A JSON rotation (roll, pitch, yaw in degrees, as written) as a rotation of
/// Unreal coordinates, as Unreal's `FRotator` turns them: yaw turns forward toward
/// right, pitch raises it toward up, and roll lowers right toward down.
fn ue_rotation(r: [f32; 3]) -> glam::Quat {
    let [roll, pitch, yaw] = r.map(f32::to_radians);
    glam::Quat::from_rotation_z(yaw) * glam::Quat::from_rotation_y(-pitch) * glam::Quat::from_rotation_x(-roll)
}

/// A local point of a placed brush, in Unreal map units: scaled, rotated, then
/// moved to its location.
fn transform(p: &Placement, v: [f32; 3]) -> [f32; 3] {
    let scaled = glam::Vec3::new(v[0] * p.scale[0], v[1] * p.scale[1], v[2] * p.scale[2]);
    (ue_rotation(p.rotation) * scaled + glam::Vec3::from_array(p.location)).to_array()
}

/// A `Cube` brush as a solid box: its lowest corner and three edges.
fn cube_box(p: &Placement, map_scale: f32, place: &impl Fn(Vec3) -> Vec3) -> WorldBox {
    let at = |c: [f32; 3]| place(map_point(transform(p, c), map_scale));
    let corner = at([0.0; 3]);
    let edges = [[CUBE_UU, 0.0, 0.0], [0.0, CUBE_UU, 0.0], [0.0, 0.0, CUBE_UU]].map(|c| (at(c) - corner).to_array());
    WorldBox { corner: corner.to_array(), edges }
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
    WorldMesh { positions: corners, triangles, color: WALL_COLOR.into(), ..WorldMesh::default() }
}

#[cfg(test)]
mod tests;
