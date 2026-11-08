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
//! forward, and KovaaK's puts a character's middle its whole height above their
//! spawn points; JSON spawn points are its middle.

use std::path::Path;

use aimadapt_sim::motion::{Blocked, Dodge, Flight, HitTurn, Jetpack, Motion, Start};
use aimadapt_sim::scenario::{Category, Scoring, Shape, Skill, Timing, Trigger, Weapon, WeaponKind};
use aimadapt_sim::world::{AccuracyMult, PlayerBlock, World, WorldBox, WorldKind, WorldMesh, WorldSpawn};
use aimadapt_sim::{Scenario, Vec3};

use crate::map::{Map, MapFormat, Placement, Solid, Teams};
use crate::scenario::{BotChoice, Sce};
use crate::settings::Section;

/// Unreal units in a meter.
const UU_PER_M: f32 = 100.0;
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

    let map = sce.map.as_ref().ok(); // a bad map is already a gap
    // Most scenarios put the player on team 1, but not all (Pasu's is team 2).
    let player_team = top.f32_or("PlayerTeam", 1.0).max(1.0) as u32;
    let other_team = if player_team == 1 { 2 } else { 1 };

    // Rules that decide the score or when a run ends, not played yet: Air Pure ends
    // when its bot runs out of lives and scores the time left.
    if top.f32_or("BotMaxLives", 0.0) > 0.0 && !top.is("InvincibleBots") {
        push_once(&mut reasons, "bots with a set number of lives (the run ends when they run out)");
    }
    if top.f32_or("ScorePerTime", 0.0) != 0.0 {
        push_once(&mut reasons, "a score for time");
    }
    for (key, what) in
        [("ScoreMultDamageEfficiency", "damage efficiency"), ("ScoreMultKillEfficiency", "kill efficiency")]
    {
        if top.is(key) {
            reasons.push(format!("a score multiplied by {what}"));
        }
    }

    // The bots. Each bot that looks or moves differently is a kind of its own; each
    // entry of `AddedBots` is a slot, filled by one of its kinds by weight, or in turn
    // from a rotation that isn't randomized (slots naming the same one share turns).
    let mut kinds: Vec<(&Section, Option<Motion>)> = Vec::new();
    let mut slots: Vec<Vec<(u32, f32)>> = Vec::new();
    let mut rotations: Vec<Option<u32>> = Vec::new();
    let mut rotation_names: Vec<String> = Vec::new();
    let mut teams = Vec::new();
    for slot in &r.bots {
        // Team 0 (some Pasu files) plays as the player's opponents.
        let team = slot.team.and_then(|t| t.trim().parse::<u32>().ok()).filter(|t| *t > 0);
        teams.push(team.unwrap_or(other_team));
        let mut choices = Vec::new();
        for c in &slot.choices {
            let Some(ch) = c.character else { continue };
            if !c.abilities.is_empty() {
                // Dashes, and the knockbacks helper bots use to push targets around
                // (Raw Control, Controlsphere): without them these play too easy.
                push_once(&mut reasons, "bots that use abilities (dashes and knockbacks)");
            }
            let m = moves(c, ch).then(|| bot_motion(c, ch));
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
    let kind = kinds.first().map(|(ch, _)| *ch);
    let motion = kinds.first().and_then(|(_, m)| m.clone());
    let moving = kinds.iter().any(|(_, m)| m.is_some());
    if r.bots.is_empty() {
        reasons.push("no bots".into());
    }
    if r.bots.len() > 50 {
        reasons.push(format!("{} bots at once (the trainer allows 50)", r.bots.len()));
    }
    let shape = kind.map(|k| hitbox(k, &mut reasons, &mut notes));
    // The other kinds' hitboxes, checked the same way.
    let others: Vec<(Shape, f32, f32)> =
        kinds.iter().skip(1).map(|(ch, _)| hitbox(ch, &mut reasons, &mut notes)).collect();
    notes.dedup();

    let (Some(map), Some(bot), Some(weapon), Some((shape, radius, height)), true) =
        (map, kind, r.weapon, shape, reasons.is_empty())
    else {
        if reasons.is_empty() {
            reasons.push("no player, weapon or bot to play with".into());
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
        height,
        player_team,
        &teams,
        &mut spawn_characters,
        &mut reasons,
        &mut notes,
    );
    if !reasons.is_empty() {
        return Err(reasons);
    }
    let Some(mut world) = world else { return Err(reasons) };
    if moving {
        notes
            .push("bots move by their dodge profiles; how closely that matches KovaaK's is still to be checked".into());
        if characters.iter().any(|ch| ch.is("HasJetpack")) {
            notes.push("bots' jetpacks thrust whenever they'd jump; how long they hold it is a guess".into());
        }
        if characters.iter().any(|ch| ch.is("CanCrouch")) && crouches(&r) {
            notes.push("bots don't crouch yet".into());
        }
    } else if all_hidden(&world, radius) {
        // Bots that stand still where they can't be seen: something in KovaaK's
        // brings them out that isn't modelled (Skeet Tracking's, behind its walls).
        return Err(vec!["every place a bot appears is behind a wall".into()]);
    }
    if kinds.len() > 1 {
        // Several kinds: each with its own hitbox, health, look and movement, on the
        // spawn points its character may use.
        let sizes = std::iter::once((shape, radius, height)).chain(others);
        for ((ch, m), (shape, radius, height)) in kinds.iter().zip(sizes) {
            let spawns: Vec<u32> = spawn_characters
                .iter()
                .enumerate()
                .filter(|(_, names)| names.is_empty() || names.iter().any(|n| n.eq_ignore_ascii_case(ch.name())))
                .map(|(i, _)| i as u32)
                .collect();
            if spawns.is_empty() {
                return Err(vec![format!("the map has no spawn point the bot \"{}\" can use", ch.name())]);
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
                motion: m.clone(),
                spawns: if all { Vec::new() } else { spawns },
            });
        }
        world.slots = slots;
        if rotations.iter().any(Option::is_some) {
            world.rotations = rotations;
        }
    } else {
        world.motion = motion;
    }
    world.source = source.into();
    world.source_hash = source_hash.into();
    world.accuracy_mult = match (top.is("ScoreMultAccuracy"), top.is("MultSqrtAcc")) {
        (true, true) => AccuracyMult::Sqrt,
        (true, false) => AccuracyMult::Linear,
        _ => AccuracyMult::None,
    };
    world.invincible = top.is("InvincibleBots");
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
    let respawn = respawn_delay(bot);
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
    t.respawn_delay_s = respawn;
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
        return Err(problems);
    }
    Ok(Imported { scenario, notes })
}

/// Whether a bot moves, unless `NoDodging` keeps it still: it runs (`MaxSpeed`),
/// flies up and down (`IsFlyer` with a flight speed: its dodge profile's jumps and
/// crouches move it), or jumps (a jump speed and a dodge profile that jumps).
fn moves(c: &BotChoice, ch: &Section) -> bool {
    if c.bot.is("NoDodging") {
        return false;
    }
    let flies =
        ch.is("IsFlyer") && (ch.f32_or("FlightVelocityUp", 0.0) > 0.0 || ch.f32_or("FlightVelocityDown", 0.0) > 0.0);
    let jumps =
        ch.f32_or("JumpVelocityMax", 0.0) > 0.0 && c.dodges.iter().any(|(d, _)| d.f32_or("JumpFrequency", 0.0) > 0.0);
    ch.f32_or("MaxSpeed", 0.0) > 0.0 || flies || jumps
}

/// Whether any bot's dodge profile crouches.
fn crouches(r: &crate::scenario::Resolved) -> bool {
    r.bots
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
        gravity: ch.f32_or("Gravity", 1.0).max(0.0) * 9.8,
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

fn push_once(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.into());
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
            && ["EnemyBodyColor", "HealthRegenPerSec", "HealthRegenDelay", "MinRespawnDelay", "MaxRespawnDelay"]
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
#[allow(clippy::too_many_arguments)]
fn world(
    map: &Map,
    map_scale: f32,
    player: Option<&Section>,
    bots: &[&Section],
    bot_height: f32,
    player_team: u32,
    teams: &[u32],
    spawn_characters: &mut Vec<Vec<String>>,
    reasons: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> Option<World> {
    let reflex = map.format == MapFormat::Reflex;
    // The player's own start comes first if the map says which one it is.
    let mut starts: Vec<_> = map.spawns.iter().filter(|s| team_can_use(s.teams, player_team)).collect();
    starts.sort_by_key(|s| !s.initial);
    let Some(start) = starts.first().copied() else {
        reasons.push("the map has no spawn point for the player".into());
        return None;
    };
    let point = |v: [f32; 3]| if reflex { reflex_point(v, map_scale) } else { map_point(v, map_scale) };
    // KovaaK's puts a character's middle its whole height above the point it appears
    // at, on both map formats: measured with test maps for players 2 to 300 units
    // tall, radius 1 to 55, map scale 1 and 4, standing or floating, and still bots
    // on spawn points and volumes. So its feet are half its height above the point.
    let rise = |height: f32| Vec3::Y * height;

    // The walls, first where the map has them: where the player stands depends on them.
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

    // Where the player stands: lifted onto a floor it overlaps, or pulled down onto
    // one within its step-up height, as Unreal's walking movement does (both measured
    // in KovaaK's). The eye is the camera offset above its middle, or with a head, above
    // the top of its body (and half the head's offset higher: one measurement).
    let mut body = point(start.position);
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
            body = aimadapt_sim::collide::Collider::for_bots(&meshes).stand(body, half - radius, radius, step_up);
        }
        camera = axes(p.vec3("CameraOffset").unwrap_or_default()) / UU_PER_M;
        if head {
            camera.y += height / 2.0 + p.f32_or("MainBBHeadOffset", 0.0) / 2.0 / UU_PER_M;
        }
    }
    let eye = body + camera;
    // JSON rotations are written roll, pitch, yaw, and Reflex ones yaw first; turn
    // the map so the player's starting yaw looks along -Z.
    let yaw_of = |a: [f32; 3]| if reflex { a[0] } else { a[2] };
    let yaw = start.angles.map_or(0.0, yaw_of).to_radians();
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
    let mut spawns = Vec::new();
    let bot = bots[0];
    for s in &map.spawns {
        let usable = bot_teams.iter().any(|t| team_can_use(s.teams, *t))
            && (s.characters.is_empty()
                || s.characters.iter().any(|c| bots.iter().any(|b| c.eq_ignore_ascii_case(b.name()))));
        if !usable || s.weight <= 0.0 {
            continue;
        }
        // The offset runs along the spawn point's own axes: Unreal right, up and back.
        // A Reflex yaw turns the same way as an Unreal one, so both use its rotation.
        let q = ue_rotation(match s.angles {
            Some(a) if reflex => [0.0, 0.0, a[0]],
            a => a.unwrap_or_default(),
        });
        let axis = |v: [f32; 3]| (turn * axes((q * glam::Vec3::from_array(v)).to_array())).to_array();
        let at = place(point(s.position));
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
            axes: [axis([0.0, 1.0, 0.0]), axis([0.0, 0.0, 1.0]), axis([-1.0, 0.0, 0.0])],
            edges,
        });
    }
    if spawns.is_empty() {
        reasons.push("the map has no spawn point the bots can use".into());
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
    Some(World {
        spawns,
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

    /// Bug: roll turned the wrong way, so rolled walls stood in the wrong place: VT
    /// Pulseshot's and horizontal map's rooms were all below the player, and DriftTS's
    /// view-limiting ramps crossed the view. A rotation is Unreal's `FRotator`, whose
    /// matrix has these rows (where forward, right and up end up).
    #[test]
    fn rotations_follow_unreals_rotator() {
        for [roll, pitch, yaw] in [[30.0, 0.0, 0.0], [-45.0, -90.0, 0.0], [144.5, 90.0, 9.5], [20.0, 35.0, -70.0]] {
            let [r, p, y] = [roll, pitch, yaw].map(|a: f32| a.to_radians());
            let (cr, sr, cp, sp, cy, sy) = (r.cos(), r.sin(), p.cos(), p.sin(), y.cos(), y.sin());
            let rows = [
                glam::Vec3::new(cp * cy, cp * sy, sp),
                glam::Vec3::new(sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp),
                glam::Vec3::new(-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp),
            ];
            let q = ue_rotation([roll, pitch, yaw]);
            for (axis, row) in [glam::Vec3::X, glam::Vec3::Y, glam::Vec3::Z].into_iter().zip(rows) {
                assert!(
                    (q * axis - row).length() < 1e-4,
                    "{roll} {pitch} {yaw}: {axis} went to {}, not {row}",
                    q * axis
                );
            }
        }
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

    /// Made up for this test: three still bots on a map whose only bot spawn is a
    /// `SpawnVolume`, 1 m deep, 8 m wide and 2 m tall, 10 m ahead.
    const VOLUME_SCE: &str = r#"Name=AimAdapt Test - Box
AimTypeTag=Clicking
PlayerProfile=Gunner
AddedBots=Dot.bot;Dot.bot;Dot.bot
BotTeams=2;2;2
ScorePerKill=1.0
MapScale=1.0
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
WeaponProfileNames=Tapper
[Weapon Profile]
Name=Tapper
Type=Hitscan
Category=SemiAuto
DamagePerShot=1.0
TimeBetweenShots=0.2
[Bot Profile]
Name=Dot
CharacterProfile=Dot Ball
[Character Profile]
Name=Dot Ball
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=20.0
MainBBHeight=40.0
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}]},
 {"type": "gameObject", "name": "SpawnVolume", "location": "1000, -400, -100", "rotation": "0, 0, 0", "scale": "1, 8, 2", "properties": [{"name": "TeamMask", "value": 0}]}
]}
"#;

    /// Bug: every bot on a spawn volume appeared at one spot, the volume's corner,
    /// so on "4BK - Micro Horizontal Easy Reflex" the balls sat on top of each other
    /// and came back in the same place. A volume is a box placed from its lowest
    /// corner, 100 units per scale step like a `Cube`, and bots appear anywhere in it.
    #[test]
    fn bots_appear_anywhere_in_a_spawn_volume() {
        let imported = convert(&Sce::parse(VOLUME_SCE), "box.sce", "0").unwrap();
        let world = imported.scenario.world.unwrap();
        let mut rng = aimadapt_sim::sim::Rng::new(5);
        let points: Vec<Vec3> = (0..200).map(|_| world.spawn_pos(&mut rng, &[], 0.2, Vec3::NEG_Z)).collect();
        // Unreal x 1000–1100, y −400–400, z −100–100 is sim z −10 to −11, x −4 to 4, y −1
        // to 1, and the 0.4 m balls' middles are their height above that: y −0.6 to 1.4.
        for p in &points {
            assert!(
                (-4.0..=4.0).contains(&p.x) && (-0.6..=1.4).contains(&p.y) && (-11.0..=-10.0).contains(&p.z),
                "{p}"
            );
        }
        let spread = |f: fn(&Vec3) -> f32| {
            let v: Vec<f32> = points.iter().map(f).collect();
            v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!(spread(|p| p.x) > 7.0 && spread(|p| p.y) > 1.5 && spread(|p| p.z) > 0.8, "{:?}", &points[..5]);
        assert!(imported.notes.is_empty(), "{:?}", imported.notes);
    }

    /// Bug: a brush saved as its own triangles (KovaaK's editor does this for some
    /// cubes) was drawn at 1 / MapScale of its size: the editor divides those
    /// triangles by the map scale (4BK - SYW Truly FIXED's floor was 8 times too small).
    #[test]
    fn a_brushs_own_triangles_are_scaled_like_a_cube() {
        let tri = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
            let v = |p: [f32; 3]| format!("{{\"location\": \"{}, {}, {}\"}}", p[0], p[1], p[2]);
            format!("{{\"indices\": [0, 1, 2], \"vertices\": [{}, {}, {}]}}", v(a), v(b), v(c))
        };
        // One face of a cube saved at MapScale 4: 0 to 25 instead of 0 to 100.
        let part = tri([0.0, 0.0, 0.0], [25.0, 0.0, 0.0], [25.0, 25.0, 25.0]);
        let text = VOLUME_SCE.replace("MapScale=1.0", "MapScale=4.0").replace(
            "
]}",
            &format!(",
 {{\"type\": \"brush\", \"name\": \"Default\", \"mesh\": \"Cube\", \"location\": \"500, 0, 0\", \"rotation\": \"0, 0, 0\", \"scale\": \"2, 1, 1\", \"procedural\": [{part}]}}
]}}"),
        );
        let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
        let m = w.meshes.last().unwrap();
        let p: Vec<Vec3> = m.positions.iter().map(|p| Vec3::from_array(*p)).collect();
        // Unreal x 500 to 700 (scale 2), times MapScale 4, is 8 m deep.
        assert!(((p[1] - p[0]).length() - 8.0).abs() < 1e-3, "{p:?}");
        assert!(((p[2] - p[1]).length() - 4.0 * 2f32.sqrt()).abs() < 1e-3, "{p:?}");
    }

    fn reasons(text: &str) -> Vec<String> {
        convert(&Sce::parse(text), "box.sce", "0").err().unwrap_or_default()
    }

    /// Bug: "4BK - SYV Easy Slow Regen Edit" played as a still ball. Its bot has no
    /// `MaxSpeed` but flies up and down; flying and jumping bots move.
    #[test]
    fn flying_and_jumping_bots_move() {
        let motion = |text: &str| convert(&Sce::parse(text), "box.sce", "0").unwrap().scenario.world.unwrap().motion;
        let flyer =
            VOLUME_SCE.replace("MainBBType=Spheroid", "MainBBType=Spheroid\nIsFlyer=true\nFlightVelocityUp=800.0");
        let f = motion(&flyer).expect("a flier moves").flight.expect("and flies");
        assert_eq!(f.up_speed, 8.0);
        let jumper = VOLUME_SCE
            .replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nDodgeProfileNames=Hop")
            .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nJumpVelocityMax=256.0\nJumpVelocityMin=200.0")
            .replace(
                "[Map Data]",
                "[Dodge Profile]\nName=Hop\nJumpFrequency=1.0\nMinJumpTime=0.5\nMaxJumpTime=0.7\n[Map Data]",
            );
        let m = motion(&jumper).expect("a jumper moves");
        assert_eq!((m.jump_speed, m.dodges[0].jump_chance, m.dodges[0].jump_s), ([2.0, 2.56], 1.0, [0.5, 0.7]));
        // NoDodging keeps both still.
        let still = flyer.replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nNoDodging=true");
        assert_eq!(motion(&still), None);
    }

    #[test]
    fn a_dodge_profile_and_character_physics_come_through_in_meters() {
        let text = VOLUME_SCE
            .replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nDodgeProfileNames=Weave\nNoAiming=true")
            .replace(
                "MaxSpeed=0.0\nMainBBType",
                "MaxSpeed=600.0\nAcceleration=3000.0\nGravity=2.0\nFriction=4.0\nDisableCharacterCollision=true\nMainBBType",
            )
            .replace(
                "[Map Data]",
                "[Dodge Profile]\nName=Weave\nMinLRTimeChange=0.4\nMaxLRTimeChange=0.9\nMinTargetDistance=500\nMaxTargetDistance=2000\n\
                 InitialRightMovementState=Left\nToggleForwardBack=true\nDamageReactionChangesDirection=true\n\
                 DamageReactionChanceToIgnore=0.25\nBlockedMovementPercent=0.5\n[Map Data]",
            );
        let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
        let m = w.motion.unwrap();
        assert_eq!((m.max_speed, m.accel, m.friction), (6.0, 30.0, 4.0));
        assert!((m.gravity - 19.6).abs() < 1e-4);
        assert!(!m.bump_bots && !m.faces_player);
        let d = &m.dodges[0];
        assert_eq!((d.strafe_s, d.min_distance, d.max_distance), ([0.4, 0.9], 5.0, 20.0));
        assert_eq!((d.start_side, d.advance), (aimadapt_sim::motion::Start::Minus, true));
        assert_eq!(d.hit_turn.as_ref().unwrap().chance, 0.75);
        assert_eq!(d.blocked.as_ref().unwrap().fraction, 0.5);
    }

    /// Bots of several kinds: each keeps its own size, health, color and movement,
    /// and appears only where its character may.
    #[test]
    fn several_kinds_of_bot_play_side_by_side() {
        let text = VOLUME_SCE
            .replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Dot.bot;Big.bot")
            .replace("BotTeams=2;2;2", "BotTeams=2;2")
            .replace(
                "[Map Data]",
                "[Bot Profile]\nName=Big\nCharacterProfile=Big Ball\nDodgeProfileNames=Weave\n\
                 [Character Profile]\nName=Big Ball\nMaxHealth=50.0\nMaxSpeed=500.0\nMainBBType=Spheroid\n\
                 MainBBRadius=60.0\nMainBBHeight=120.0\nEnemyBodyColor=X=0.000 Y=0.000 Z=1.000\n\
                 [Dodge Profile]\nName=Weave\n[Map Data]",
            )
            .replace(
                "\"properties\": [{\"name\": \"TeamMask\", \"value\": 0}]}",
                "\"properties\": [{\"name\": \"TeamMask\", \"value\": 0}]},\n {\"type\": \"gameObject\", \"name\": \"SpawnPoint\", \"location\": \"1500, 0, 0\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 1, 1\", \"properties\": [{\"name\": \"TeamMask\", \"value\": 2}, {\"name\": \"PermittedCharacterProfiles\", \"value\": \"Big Ball\"}]}",
            );
        let s = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario;
        let w = s.world.as_ref().unwrap();
        assert_eq!(w.kinds.len(), 2);
        assert_eq!(w.slots, vec![vec![(0, 1.0)], vec![(1, 1.0)]]);
        let (dot, big) = (&w.kinds[0], &w.kinds[1]);
        assert_eq!((dot.radius, dot.health, dot.motion.is_some()), (0.2, 1.0, false));
        assert_eq!((big.radius, big.health, big.color.as_str()), (0.6, 50.0, "#0000ff"));
        assert!(big.motion.is_some() && w.motion.is_none());
        // The volume is for anyone; the far point only for the big ball.
        assert_eq!((dot.spawns.clone(), big.spawns.clone()), (vec![0], vec![]));
        // In play: one of each.
        let sim = aimadapt_sim::Sim::new(s.clone(), 4);
        let mut kinds: Vec<Option<u32>> = sim.targets().iter().map(|t| t.kind()).collect();
        kinds.sort();
        assert_eq!(kinds, vec![Some(0), Some(1)]);
        let big_t = sim.targets().iter().find(|t| t.kind() == Some(1)).unwrap();
        assert_eq!((big_t.radius, big_t.health), (0.6, 50.0));
    }

    /// Bots whose `InvertBlockedSpawn` radius puts them in clusters.
    #[test]
    fn inverted_bot_spawn_radius_makes_clusters() {
        let text = VOLUME_SCE
            .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nBlockedSpawnRadius=100.0\nInvertBlockedSpawn=true");
        let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
        assert!(w.cluster && (w.blocked_radius - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bots_with_dashes_or_knockbacks_cant_play_yet() {
        let text = VOLUME_SCE
            .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nAbilityProfileNames=Shove.abilmelee;;")
            .replace("[Map Data]", "[Melee Ability Profile]\nName=Shove\nHurtboxRadius=80.0\n[Map Data]");
        assert!(reasons(&text).iter().any(|r| r.starts_with("bots that use abilities")), "{:?}", reasons(&text));
    }

    /// Bug: Pasu's player is on team 2, so the player and the bots swapped spawn
    /// points: the bots appeared around the player.
    #[test]
    fn the_player_can_be_on_any_team_and_team_0_bots_are_their_opponents() {
        // Teams swapped: the player starts at the point TeamMask 2 allows, 10 m from
        // the volume the bots (team 0, "the other side") use.
        let text = VOLUME_SCE
            .replace("ScorePerKill=1.0", "ScorePerKill=1.0\nPlayerTeam=2")
            .replace("BotTeams=2;2;2", "BotTeams=0;0;0")
            .replace("{\"name\": \"TeamMask\", \"value\": 1}", "{\"name\": \"TeamMask\", \"value\": 2}")
            .replace("{\"name\": \"TeamMask\", \"value\": 0}", "{\"name\": \"TeamMask\", \"value\": 1}");
        let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
        assert_eq!(w.spawns.len(), 1);
        assert!((w.spawns[0].pos[2] - -10.0).abs() < 1e-4, "{:?}", w.spawns[0].pos);
    }

    /// Bug: JSON maps' invisible walls (`Clip`, `FullClip`) were drawn and stopped
    /// shots, and a player who spawns sunk into the floor saw from under it
    /// (Midrange Long Strafes): characters are lifted onto the floor, as in Unreal.
    #[test]
    fn clip_brushes_are_invisible_and_the_player_stands_on_the_floor() {
        let brush = |name: &str, loc: &str, scale: &str| {
            format!(
                ",\n {{\"type\": \"brush\", \"name\": \"{name}\", \"mesh\": \"Cube\", \"location\": \"{loc}\", \"rotation\": \"0, 0, 0\", \"scale\": \"{scale}\"}}"
            )
        };
        let walls = [
            brush("Default", "-1000, -1000, -50", "20, 20, 0.5"), // a floor whose top is at z = 0
            brush("Clip", "500, -1000, 0", "0.1, 20, 10"),
            brush("DefaultNoCollision", "600, -1000, 0", "0.1, 20, 10"),
            brush("WeaponClip", "700, -1000, 0", "0.1, 20, 10"),
        ]
        .concat();
        let text = VOLUME_SCE
            .replace("MaxSpeed=0.0\nWeaponProfileNames", "MaxSpeed=0.0\nMainBBRadius=30.0\nMainBBHeight=200.0\nCameraOffset=X=0.000 Y=0.000 Z=60.000\nSpawnOffsetMin=X=0.000 Y=0.000 Z=-100.000\nSpawnOffsetMax=X=0.000 Y=0.000 Z=-100.000\nWeaponProfileNames")
            .replace("\n]}", &format!("{walls}\n]}}"));
        let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
        assert_eq!(w.meshes.len(), 3, "the weapon clip is left out");
        assert!(w.meshes[1].clip && !w.meshes[1].decor);
        assert!(w.meshes[2].decor && !w.meshes[2].clip);
        // The floor's top is 1.62 m below the eye: the 2 m player stands 2 cm above it
        // (its middle 1.02 m up), and the camera is 0.6 m above its middle.
        let floor_top = w.meshes[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!((floor_top - -1.62).abs() < 0.005, "{floor_top}");
        // Shots pass the clip wall but not the floor.
        let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
        assert_eq!(walls.ray(Vec3::ZERO, Vec3::NEG_Z, 20.0), None);
        assert!(walls.ray(Vec3::ZERO, Vec3::NEG_Y, 20.0).is_some());
    }

    /// [`VOLUME_SCE`] with a 2.3 m player (plus `player` settings) appearing `spawn_z`
    /// units above a floor whose top is at z = 0, and how far below the eye that floor
    /// and the bots' spawn volume's lowest corner (100 units under the floor) are.
    fn standing(player: &str, spawn_z: f32) -> (f32, f32) {
        let text = VOLUME_SCE
            .replace(
                "MaxSpeed=0.0\nWeaponProfileNames",
                &format!("MaxSpeed=0.0\nMainBBRadius=55.0\nMainBBHeight=230.0\n{player}WeaponProfileNames"),
            )
            .replace("\"location\": \"0, 0, 0\"", &format!("\"location\": \"0, 0, {spawn_z}\""))
            .replace("\n]}", &format!("{}\n]}}", brush_json("Cube", "-2000, -2000, -16", "0, 0, 0", "40, 40, 0.16")));
        let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
        let floor = w.meshes[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        (-floor, -w.spawns[0].pos[1])
    }

    /// Bug: characters stood lower than in KovaaK's, by up to their whole height.
    /// Measured there with test maps (`AimAdapt - Eye Test …`, 23 of them): on both map
    /// formats a character's middle starts its whole height (`MainBBHeight`) above
    /// the point it appears at, players, bots and spawn volumes alike, and the eye is
    /// the camera offset above that. With a head (`MainBBHasHead`) the eye is at the
    /// top of the body instead (and half the head's offset higher), and the capsule
    /// Unreal stands on floors reaches the head's radius further each way. A
    /// character whose feet are within `StepUpHeight` (and 2.4 units) above a floor is
    /// pulled down onto it, and one on a floor stands 2 units above it.
    #[test]
    fn characters_start_their_height_above_where_they_appear() {
        let near = |got: f32, want: f32| assert!((got - want).abs() < 0.005, "{got} != {want}");
        // Floating 5 m up: the eye is 2.3 m above the spawn point (KovaaK's: 729.7
        // units over a floor 500 below), and the 0.4 m ball's middle 0.4 m above its
        // volume's corner, 1 m below the floor.
        let (floor, volume) = standing("", 500.0);
        near(floor, 7.3);
        near(volume, 7.3 + 1.0 - 0.4);
        // A camera offset adds to it.
        near(standing("CameraOffset=X=0.000 Y=0.000 Z=50.000\n", 500.0).0, 7.8);
        // With a head the eye is the top of the body: 1.15 m higher (844.9 units).
        let head = "MainBBHasHead=true\nMainBBHeadRadius=45.0\nMainBBHeadOffset=0.0\n";
        near(standing(head, 500.0).0, 8.45);
        // Standing on the floor with a 75-unit step-up: the head's capsule (1.6 m
        // each way from the middle) has its feet 0.7 m up, within the step, so it's
        // pulled down to 2 cm above the floor and the eye is at 1.62 + 1.15 m (276.6
        // units in KovaaK's). With a 30-unit step it stays up: 3.45 m (344.7).
        near(standing(&format!("{head}StepUpHeight=75.0\n"), 0.0).0, 2.77);
        near(standing(&format!("{head}StepUpHeight=30.0\n"), 0.0).0, 3.45);
        // No head and a 120-unit step: the feet are 1.15 m up, so it's pulled down
        // too, and sees from 1.17 m (116.5 units).
        near(standing("StepUpHeight=120.0\n", 0.0).0, 1.17);
        near(standing("StepUpHeight=75.0\n", 0.0).0, 2.3);
    }

    /// A made-up brush for [`VOLUME_SCE`]'s map, which has `MapScale=1`: 100 units a meter.
    fn brush_json(mesh: &str, loc: &str, rot: &str, scale: &str) -> String {
        format!(
            ",\n {{\"type\": \"brush\", \"name\": \"Default\", \"mesh\": \"{mesh}\", \"location\": \"{loc}\", \"rotation\": \"{rot}\", \"scale\": \"{scale}\"}}"
        )
    }

    fn world_with(brushes: &[String]) -> World {
        let text = VOLUME_SCE.replace("\n]}", &format!("{}\n]}}", brushes.concat()));
        convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap()
    }

    /// A level ray from the eye, `deg` degrees right of straight ahead.
    fn level(deg: f32) -> Vec3 {
        let a = deg.to_radians();
        Vec3::new(a.sin(), 0.0, -a.cos())
    }

    /// Bug: DriftTS's two ramps that limit the view to 90° were drawn as slanted planes
    /// across it, and passed shots through. A `Ramp` is placed from its corner like a
    /// `Cube`, its slope runs across it (Y to Z), and roll turns as in Unreal: rolled
    /// -45° and pitched -90°, this one stands with its side on the line 45° left of
    /// straight ahead, its near end 6 m out.
    #[test]
    fn a_ramp_turned_to_limit_the_view_has_its_side_on_the_limit() {
        let w = world_with(&[brush_json("Ramp", "282.843, -565.685, 200", "-45, -90, 0", "4, 2, 6")]);
        assert!(!w.meshes[0].decor, "a ramp is solid");
        let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
        // Just past the limit, the ramp's near end stops the shot about 6 m out.
        let hit = walls.ray(Vec3::ZERO, level(-50.0), 20.0).expect("the ramp stops a shot past the limit");
        assert!((hit - 6.0 / 5f32.to_radians().cos()).abs() < 0.02, "{hit}");
        // Inside the limit, nothing does.
        assert_eq!(walls.ray(Vec3::ZERO, level(-40.0), 20.0), None);
    }

    /// Bug: the curved wall of VT ww5t's map (and Aether's, PGT's, Snake Track's) was
    /// drawn as four skate ramps off its corners, and passed shots. A `Concave` is a
    /// block placed from its corner with a quarter circle cut from the far corner, seen
    /// from above: four of them, turned to a square room's corners, make it round.
    #[test]
    fn four_concave_corners_make_a_round_room() {
        // 15 m across, around the bots' spawn volume 10 m ahead.
        let w = world_with(&[
            brush_json("Concave", "-1500, -1500, -300", "0, 0, 0", "15, 15, 6"),
            brush_json("Concave", "1500, -1500, -300", "0, 0, 90", "15, 15, 6"),
            brush_json("Concave", "1500, 1500, -300", "0, 0, 180", "15, 15, 6"),
            brush_json("Concave", "-1500, 1500, -300", "0, 0, -90", "15, 15, 6"),
        ]);
        let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
        for deg in [0.0, 20.0, 45.0, 70.0, 90.0, 135.0, 180.0, -100.0, -45.0] {
            let hit = walls.ray(Vec3::ZERO, level(deg), 40.0).unwrap_or(f32::MAX);
            assert!((hit - 15.0).abs() < 0.05, "{deg}°: {hit}");
        }
    }

    /// Bug: VT ControlTS's floor, a `WedgeB`, was drawn as a narrow ramp to one side and
    /// passed shots. Like a `Ramp`, its slope runs across it from its high corner: turned
    /// 90°, this floor is 1 m higher at the far end, as ControlTS's is in KovaaK's.
    #[test]
    fn a_wedge_floor_rises_away_from_its_low_end() {
        let w = world_with(&[brush_json("WedgeB", "800, -500, -300", "0, 0, 90", "10, 8, 1")]);
        let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
        // Straight down from 1 m and 7 m ahead: the top is 3 m - x / 8 below the eye.
        for x in [1.0, 7.0] {
            let drop = walls.ray(Vec3::new(0.0, 0.0, -x), Vec3::NEG_Y, 10.0).expect("the wedge is solid");
            assert!((drop - (3.0 - x / 8.0)).abs() < 0.01, "{x} m ahead: {drop}");
        }
    }

    /// Bug: VT Aether's three targets (a rotation with `Randomized=false`, each losing
    /// 100 health a second) were picked at random and never drained. Made up here: a
    /// rotation of two bots that differ in health, taken in turn, both draining.
    #[test]
    fn rotations_in_order_and_draining_health_come_through() {
        let text = VOLUME_SCE
            .replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Turns.rot")
            .replace("BotTeams=2;2;2", "BotTeams=2")
            .replace(
                "[Bot Profile]",
                "[Bot Rotation Profile]\nName=Turns\nProfileNames=Dot;Big Dot\nProfileWeights=1.0;1.0\nRandomized=false\n\
                 [Bot Profile]\nName=Big Dot\nCharacterProfile=Big Ball\n\
                 [Character Profile]\nName=Big Ball\nMaxHealth=50.0\nHealthRegenPerSec=-10.0\nMaxSpeed=0.0\nMainBBType=Spheroid\nMainBBRadius=20.0\nMainBBHeight=40.0\n\
                 [Bot Profile]",
            )
            .replace("MaxHealth=1.0\nMaxSpeed=0.0\nMainBBType", "MaxHealth=1.0\nHealthRegenPerSec=-10.0\nMaxSpeed=0.0\nMainBBType");
        let i = convert(&Sce::parse(&text), "box.sce", "0").unwrap();
        let w = i.scenario.world.unwrap();
        assert_eq!(w.kinds.len(), 2);
        assert_eq!(w.slots, vec![vec![(0, 1.0), (1, 1.0)]]);
        assert_eq!(w.rotations, vec![Some(0)]);
        assert!(w.kinds.iter().all(|k| k.regen_per_s == -10.0), "{:?}", w.kinds);
        assert!(!i.notes.iter().any(|n| n.contains("negative")), "{:?}", i.notes);
        // Picked by weight when the rotation is randomized.
        let random = text.replace("Randomized=false", "Randomized=true");
        let w = convert(&Sce::parse(&random), "box.sce", "0").unwrap().scenario.world.unwrap();
        assert!(w.rotations.is_empty(), "{:?}", w.rotations);
    }

    /// Bug: Air Pure and Air CELESTIAL end when the bot runs out of lives and score
    /// the time left, which isn't played: they played 1000 s scoring nothing. Rules
    /// that decide the score or when a run ends say they can't be played yet.
    #[test]
    fn bot_lives_and_unplayed_score_rules_cant_play_yet() {
        let lives = VOLUME_SCE.replace("ScorePerKill=1.0", "ScorePerKill=0.0\nBotMaxLives=5\nScorePerTime=1.0");
        let r = reasons(&lives);
        assert!(r.iter().any(|x| x.starts_with("bots with a set number of lives")), "{r:?}");
        assert!(r.iter().any(|x| x.starts_with("a score for time")), "{r:?}");
        // Bots that can't die never run out of lives.
        let invincible = VOLUME_SCE.replace("ScorePerKill=1.0", "ScorePerKill=1.0\nBotMaxLives=1\nInvincibleBots=true");
        assert!(!reasons(&invincible).iter().any(|x| x.contains("lives")), "{:?}", reasons(&invincible));
        // Bug: `InvincibleBots` was ignored (VT Snake Track), so its bot died.
        assert!(convert(&Sce::parse(&invincible), "box.sce", "0").unwrap().scenario.world.unwrap().invincible);
        let eff = VOLUME_SCE.replace("ScorePerKill=1.0", "ScorePerKill=1.0\nScoreMultDamageEfficiency=true");
        assert!(
            reasons(&eff).iter().any(|x| x.starts_with("a score multiplied by damage efficiency")),
            "{:?}",
            reasons(&eff)
        );
    }

    /// Skeet Tracking's targets stand behind its walls until something launches them,
    /// which isn't played: still targets that are all out of sight can't be played.
    #[test]
    fn still_bots_that_all_start_behind_walls_cant_be_played() {
        let wall = ",\n {\"type\": \"brush\", \"name\": \"Default\", \"mesh\": \"Cube\", \"location\": \"500, -2000, -1000\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 40, 20\"}";
        let text = VOLUME_SCE.replace("\n]}", &format!("{wall}\n]}}"));
        assert_eq!(reasons(&text), vec!["every place a bot appears is behind a wall".to_string()]);
    }

    const REFLEX_SCE: &str = "Name=AimAdapt Test - Old Box\nPlayerProfile=Gunner\nAddedBots=Dot.bot\nBotTeams=2\nMapScale=2.0\n\
        [Character Profile]\nName=Gunner\nMaxSpeed=0.0\nMainBBRadius=10.0\nMainBBHeight=100.0\nWeaponProfileNames=Tapper\n\
        [Weapon Profile]\nName=Tapper\nType=Hitscan\nCategory=SemiAuto\n\
        [Bot Profile]\nName=Dot\nCharacterProfile=Dot Ball\n\
        [Character Profile]\nName=Dot Ball\nMaxSpeed=0.0\nMainBBType=Spheroid\nMainBBRadius=20.0\nMainBBHeight=40.0\n\
        [Map Data]\nreflex map version 8\nglobal\n\
        \tentity\n\t\ttype PlayerSpawn\n\t\tVector3 position 0.000000 0.000000 0.000000\n\t\tVector3 angles 90.000000 0.000000 0.000000\n\t\tBool8 teamB 0\n\
        \tentity\n\t\ttype PlayerSpawn\n\t\tVector3 position 500.000000 0.000000 0.000000\n\t\tVector3 angles 270.000000 0.000000 0.000000\n\t\tBool8 teamA 0\n\
        \tbrush\n\t\tvertices\n\
        \t\t\t-1000.000000 -16.000000 -1000.000000\n\t\t\t1000.000000 -16.000000 -1000.000000\n\t\t\t1000.000000 -16.000000 1000.000000\n\t\t\t-1000.000000 -16.000000 1000.000000\n\
        \t\t\t-1000.000000 0.000000 -1000.000000\n\t\t\t1000.000000 0.000000 -1000.000000\n\t\t\t1000.000000 0.000000 1000.000000\n\t\t\t-1000.000000 0.000000 1000.000000\n\
        \t\tfaces\n\
        \t\t\t0.000000 0.000000 1.000000 1.000000 0.000000 4 5 6 7 0x00000000\n\
        \t\t\t0.000000 0.000000 1.000000 1.000000 0.000000 0 3 2 1 0x00000000\n\
        \tbrush\n\t\tvertices\n\
        \t\t\t200.000000 0.000000 -100.000000\n\t\t\t210.000000 0.000000 -100.000000\n\t\t\t210.000000 300.000000 -100.000000\n\t\t\t200.000000 300.000000 100.000000\n\
        \t\tfaces\n\
        \t\t\t0.000000 0.000000 1.000000 1.000000 0.000000 0 1 2 3 0x00000000 internal/editor/textures/editor_clip\n";

    /// Reflex maps: centimeters with X right, Y up and Z forward, and yaw first.
    ///
    /// Bug: characters stood half their height too low on these maps, so tamTS's
    /// 300-unit player saw from inside the pit KovaaK's has them look down into.
    /// Measured in KovaaK's with test maps, a character's middle is its whole
    /// height (`MainBBHeight`) above its spawn point, whatever its radius or the
    /// map scale, standing or floating: the eye is the spawn plus the height plus
    /// the camera offset, and a still 200-unit ball's middle is 200 units up.
    #[test]
    fn reflex_maps_play() {
        let floor_top = |w: &World| w.meshes[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        let imported = convert(&Sce::parse(REFLEX_SCE), "old.sce", "0").unwrap();
        let w = imported.scenario.world.unwrap();
        // The player faces +X at yaw 90, and the bot's spawn is 5 m that way, times
        // MapScale 2: 10 m straight ahead. Sizes aren't scaled: the 1 m player's eye
        // is 1 m above the floor, and the 0.4 m ball's middle 0.4 m above it.
        let bot = Vec3::from_array(w.spawns[0].pos);
        assert!((bot - Vec3::new(0.0, -0.6, -10.0)).length() < 1e-3, "{bot}");
        // The floor's top face, and the clip wall, invisible.
        assert_eq!(w.meshes.len(), 2);
        assert!((floor_top(&w) - -1.0).abs() < 1e-3, "{}", floor_top(&w));
        assert_eq!(w.meshes[0].triangles.len(), 4, "two quads, two triangles each");
        assert!(w.meshes[1].clip);
        assert!(!imported.notes.iter().any(|n| n.contains("Reflex")), "{:?}", imported.notes);
        // A camera offset adds to the height: 3 m tall with the camera 0.8 m up sees
        // from 3.8 m (the test map in KovaaK's: 379.8 units).
        let tall = REFLEX_SCE.replace(
            "MainBBHeight=100.0\nWeaponProfileNames",
            "MainBBHeight=300.0\nCameraOffset=X=0.000 Y=0.000 Z=80.000\nWeaponProfileNames",
        );
        let imported = convert(&Sce::parse(&tall), "old.sce", "0").unwrap();
        let w = imported.scenario.world.unwrap();
        assert!((floor_top(&w) - -3.8).abs() < 1e-3, "{}", floor_top(&w));
        assert!(!imported.notes.iter().any(|n| n.contains("Reflex")), "{:?}", imported.notes);
    }

    /// Bugs: health regen was left out (Pokeball Small heals 4 hp/s), the player's
    /// spawn rules were ignored (1w4ts, ww5t), and walls didn't stop bots appearing
    /// inside them (Amare Microcluster).
    #[test]
    fn regen_the_players_spawn_rules_and_walls_come_through() {
        let text = VOLUME_SCE
            .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nHealthRegenPerSec=4.0\nHealthRegenDelay=0.25")
            .replace("WeaponProfileNames=Tapper", "WeaponProfileNames=Tapper\nBlockSpawnFOV=40.0\nBlockSpawnDistance=9999.0\nInvertBlockedSpawn=true")
            .replace(
                "\n]}",
                ",\n {\"type\": \"brush\", \"name\": \"Default\", \"mesh\": \"Cube\", \"location\": \"1050, -500, -200\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 10, 4\"}\n]}",
            );
        let s = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario;
        assert_eq!((s.targets.regen_per_s, s.targets.regen_delay_s), (4.0, 0.25));
        let w = s.world.unwrap();
        let b = w.player_block.clone().unwrap();
        assert_eq!((b.fov_deg, b.invert), (40.0, true));
        assert!((b.distance - 99.99).abs() < 1e-3);
        // The wall: Unreal x 1050–1150 is sim z -10.5 to -11.5, its front face inside
        // the spawn volume (z -10 to -11), so every bot ends up in front of it.
        assert_eq!(w.walls.len(), 1);
        let mut rng = aimadapt_sim::sim::Rng::new(9);
        for _ in 0..100 {
            let p = w.spawn_pos(&mut rng, &[], 0.2, Vec3::NEG_Z);
            assert!(p.z >= -10.5 + 0.2 - 1e-4, "{p}");
        }
    }
}
