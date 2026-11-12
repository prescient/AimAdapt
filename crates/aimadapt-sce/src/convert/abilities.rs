//! A bot's abilities: movement abilities (dashes, blinks, ability jumps) become the
//! bot's [`Abilities`]; the ones the trainer can't play yet come back as reasons.
//! Velocities and distances in the file are centimeters, times are seconds.
//!
//! A dash that can go up or down (`MainVelocityCanGoVertical` with a main velocity) follows the
//! bot's aim line, which needs the player's size too (see [`aim_lift`]). It is refused when it
//! keeps gravity for its duration, when it is an impulse along the movement keys, when it isn't
//! along the keys on a bot that doesn't aim, and when the bot's or the player's hitbox is
//! neither a spheroid nor a capsule: none of those was measured.
//!
//! An ability with `AIDamageReaction=true` is one the bot also uses when it is hurt (plan K15d, see
//! [`reaction`]). That plays for abilities with a duration. One with no duration (an impulse) is
//! refused: KovaaK's sometimes showed nothing for such a reaction, and why wasn't understood.

use aimadapt_sim::motion::{Abilities, Ability, Fuse, Motion, Reaction};

use super::{UU_PER_M, camera_offset, push_once};
use crate::inventory::Gap;
use crate::scenario::{ABILITY_KINDS, BotChoice, BotSlot};
use crate::settings::Section;

pub(super) fn is_kind(a: &Section, kind: usize) -> bool {
    a.kind.eq_ignore_ascii_case(ABILITY_KINDS[kind])
}

/// A bot's movement ability profiles.
pub(super) fn movement<'a, 'b>(c: &'a BotChoice<'b>) -> impl Iterator<Item = &'b Section> + 'a {
    c.abilities.iter().copied().filter(|a| is_kind(a, 2))
}

/// Settings a movement ability may have that the trainer doesn't play yet: the key
/// and whether this value of it counts.
type Unplayed = fn(&Section) -> bool;

const UNPLAYED: &[(&str, Unplayed)] = &[
    // Healing. A negative value is a cost, which plays (see [`costs_health`]).
    ("HealthRestore", |a| a.f32_or("HealthRestore", 0.0) > 0.0),
    ("Hurtbox", |a| a.is("Hurtbox")),
    ("AIMinSelfHealth", |a| a.f32_or("AIMinSelfHealth", 0.0) > 0.0),
    ("AIMaxSelfHealth", |a| a.f32_or("AIMaxSelfHealth", 100.0) < 100.0),
    ("AIMinTargHealth", |a| a.f32_or("AIMinTargHealth", 0.0) > 0.0),
    ("AIMaxTargHealth", |a| a.f32_or("AIMaxTargHealth", 100.0) < 100.0),
    ("CancelDashOnGreaterThan45Collisions", |a| a.is("CancelDashOnGreaterThan45Collisions")),
];

/// Whether a movement ability costs the bot health each time it uses it (a negative
/// `HealthRestore`).
fn costs_health(a: &Section) -> bool {
    a.f32_or("HealthRestore", 0.0) < 0.0
}

/// Whether a movement ability can send the bot up or down and has a main velocity to do it
/// with. (With no main velocity it is an ability jump, which is level work for this module.)
fn goes_up_or_down(a: &Section) -> bool {
    a.is("MainVelocityCanGoVertical") && a.f32_or("MainVelocity", 0.0) != 0.0
}

/// The hitbox shapes a bot's aim line was measured for.
fn aim_shape(ch: &Section) -> Option<&str> {
    let shape = ch.str("MainBBType");
    ["Spheroid", "Cylindrical"].into_iter().find(|s| *s == shape)
}

/// A body height in meters: twice the radius for a spheroid (whatever its `MainBBHeight`), and
/// `MainBBHeight` for a capsule (`Cylindrical`). `None` for any other shape.
fn body_height(ch: &Section) -> Option<f32> {
    match aim_shape(ch)? {
        "Spheroid" => Some(2.0 * ch.f32_or("MainBBRadius", 0.0).max(0.0) / UU_PER_M),
        _ => Some(ch.f32_or("MainBBHeight", 0.0).max(0.0) / UU_PER_M),
    }
}

/// How much higher than the line from a bot's middle to the player's eye the bot's aim line is.
/// KovaaK's runs it from one body height above the bot's middle to one body height above the
/// middle of the player's character (measured; `CameraOffset` plays no part at either end), so
/// it is the player's body height, less the bot's, less how far the eye is above the player's
/// middle. The level part of the camera offset is left out. A player with a head follows the
/// same rule, which wasn't measured. `None` when either hitbox is of a shape that wasn't
/// measured, or there is no player character.
fn aim_lift(ch: &Section, player: Option<&Section>) -> Option<f32> {
    let player = player?;
    Some(body_height(player)? - body_height(ch)? - camera_offset(player).y)
}

/// Reasons a target's abilities can't be played yet. A target's melee ability (a
/// knockback) isn't played: only helper bots push, see [`super::helpers`]. Weapon
/// abilities are reported with the file's other gaps, the ones that play being the fuse
/// (see [`super::fuse`]). `ch` is the bot's character,
/// `top_invincible` whether the scenario's `InvincibleBots` is on and `player` the
/// player's character.
pub(super) fn reasons(
    c: &BotChoice,
    ch: &Section,
    top_invincible: bool,
    player: Option<&Section>,
    reasons: &mut Vec<Gap>,
) {
    for a in movement(c).filter(|a| costs_health(a)) {
        // A bot that can't die was not measured paying for an ability. Nor was a cost on an
        // ability that doesn't move the bot: the bot only gets abilities for those that do.
        let why = if top_invincible || ch.is("InvincibleBots") {
            Some("on a bot that can't die")
        } else if !launches(a) {
            Some("on an ability that doesn't move the bot")
        } else {
            None
        };
        if let Some(why) = why {
            let detail = format!("{}: HealthRestore={} {why}", a.name(), a.str("HealthRestore").trim());
            push_once(reasons, Gap::planned("a movement ability the trainer can't play yet", detail));
        }
    }
    // Whether a bot that is told not to dodge still dashes wasn't measured in KovaaK's.
    if c.bot.is("NoDodging") {
        for a in movement(c).filter(|a| launches(a)) {
            let detail = format!("{}: on a bot with NoDodging", a.name());
            push_once(reasons, Gap::planned("a movement ability the trainer can't play yet", detail));
        }
    }
    for a in movement(c).filter(|a| goes_up_or_down(a)) {
        if let Some(why) = vertical_refusal(a, c, ch, player) {
            let detail = format!("{}: MainVelocityCanGoVertical=true {why}", a.name());
            push_once(reasons, Gap::planned("a movement ability the trainer can't play yet", detail));
        }
    }
    for a in movement(c).filter(|a| reacts(a) && !lasts(a)) {
        let detail = format!("{}: AIDamageReaction=true with no AbilityDuration", a.name());
        push_once(reasons, Gap::planned("a movement ability the trainer can't play yet", detail));
    }
    for a in &c.abilities {
        if is_kind(a, 1) {
            let detail = format!("a target's melee ability: {}", a.name());
            push_once(reasons, Gap::planned("bots that use abilities", detail));
        } else if is_kind(a, 2) {
            for (key, unplayed) in UNPLAYED {
                if unplayed(a) {
                    let detail = format!("{}: {key}={}", a.name(), a.str(key).trim());
                    push_once(reasons, Gap::planned("a movement ability the trainer can't play yet", detail));
                }
            }
        }
    }
}

/// Why a dash that can go up or down isn't played, if it isn't: what was measured is a held
/// dash that negates gravity (along the keys or not) and an impulse not along the keys, with
/// the aim line of a spheroid or a capsule.
fn vertical_refusal(a: &Section, c: &BotChoice, ch: &Section, player: Option<&Section>) -> Option<String> {
    let held = a.f32_or("AbilityDuration", 0.0) > 0.0;
    let along_keys = a.is("MainVelocitySetToMovementKeys");
    if held && !a.is("NegateGravityForDuration") {
        let seconds = a.str("AbilityDuration").trim();
        return Some(format!("with AbilityDuration={seconds} and no NegateGravityForDuration"));
    }
    if !held && along_keys {
        return Some("with MainVelocitySetToMovementKeys=true and no AbilityDuration".into());
    }
    if !along_keys && c.bot.is("NoAiming") {
        return Some("on a bot with NoAiming".into());
    }
    let Some(player) = player else { return Some("and the file has no player character".into()) };
    if aim_shape(ch).is_none() || aim_shape(player).is_none() {
        let name = |s: &Section| match s.str("MainBBType").trim() {
            "" => "none".to_string(),
            t => t.to_string(),
        };
        return Some(format!("with a bot MainBBType of {} and a player MainBBType of {}", name(ch), name(player)));
    }
    None
}

/// Whether a movement ability has a duration, so it is a dash or a blink and not an impulse.
fn lasts(a: &Section) -> bool {
    a.f32_or("AbilityDuration", 0.0) > 0.0
}

/// Whether the bot also uses a movement ability when it is hurt (`AIDamageReaction`).
fn reacts(a: &Section) -> bool {
    a.is("AIDamageReaction")
}

/// Whether a movement ability changes the bot's speed or sends it up.
fn launches(a: &Section) -> bool {
    a.f32_or("MainVelocity", 0.0) != 0.0 || a.f32_or("UpVelocity", 0.0) != 0.0
}

/// Whether a bot has a movement ability that changes its speed or sends it up.
pub(super) fn moves(c: &BotChoice) -> bool {
    movement(c).any(launches)
}

/// Notes for combinations of a health cost that nobody checked in KovaaK's. `slots` are
/// the target slots and `kill_limit` is the run's limit from bot lives, 0 for none.
pub(super) fn cost_notes(slots: &[&BotSlot], kill_limit: u32, notes: &mut Vec<String>) {
    let mut note = |text: &str| {
        if !notes.iter().any(|n| n == text) {
            notes.push(text.into());
        }
    };
    for c in slots.iter().flat_map(|s| &s.choices) {
        if !movement(c).any(costs_health) {
            continue;
        }
        if c.character.is_some_and(|ch| ch.f32_or("HealthRegenPerSec", 0.0) != 0.0) {
            note(
                "a bot that pays health for an ability and also regenerates or drains health: the cost is played \
                 as not restarting the wait before that; that hasn't been checked against KovaaK's",
            );
        }
        if kill_limit > 0 {
            note(
                "a bot that pays health for an ability, with bot lives: a bot that dies of the cost is played as a \
                 timeout that uses up no life; that hasn't been checked against KovaaK's",
            );
        }
    }
}

/// Notes for bots that blow themselves up (see [`super::fuse`]), for the scenarios that have one. `motions` are
/// those of the kinds of bot and `kill_limit` is the run's limit from bot lives, 0 for none. Unlike
/// [`cost_notes`] this runs for every scenario, since a fuse bot isn't a moving one.
pub(super) fn fuse_notes(motions: &[&Motion], kill_limit: u32, notes: &mut Vec<String>) {
    if !motions.iter().any(|m| m.stays_put) {
        return;
    }
    notes.push(
        "a bot that blows itself up: a press of its ability's key that the weapon doesn't take is played as still \
         using up the ability's charge, and a blast still waiting when its bot is killed is dropped; neither has \
         been checked against KovaaK's"
            .into(),
    );
    if kill_limit > 0 {
        notes.push(
            "a bot that blows itself up, with bot lives: a bot that dies of its own blast is played as a timeout \
             that uses up no life; that hasn't been checked against KovaaK's"
                .into(),
        );
    }
}

/// The longest time and distance the trainer allows. More than an hour or 10 km is
/// never or everywhere in any run, so larger values in a file are cut to these.
const LONGEST_S: f32 = 3600.0;
const FARTHEST_M: f32 = 10_000.0;

/// The highest damage threshold of a reaction the trainer allows. A file's higher one is cut to it: no
/// bot takes that much, so it never reacts either way.
const HIGHEST_THRESHOLD: f32 = 1.0e6;

/// The time between ticks of a bot's ability key when its profile has no `AbilityUseTimer` (measured).
const DEFAULT_TICK_S: f32 = 0.1;

/// The bot's movement abilities and the settings for using them, or none if it has
/// no movement ability. `ch` is its character profile and `player` the player's.
pub(super) fn abilities(c: &BotChoice, ch: &Section, player: Option<&Section>) -> Option<Abilities> {
    let list: Vec<Ability> = movement(c).map(ability).collect();
    if list.is_empty() {
        return None;
    }
    // Only a bot with a dash that goes up or down has an aim line to follow.
    let aim_lift = if movement(c).any(goes_up_or_down) { aim_lift(ch, player).unwrap_or(0.0) } else { 0.0 };
    Some(wrap(c, ch, list, aim_lift))
}

/// The abilities of a bot that blows itself up (see [`super::fuse`]): its weapon ability `a`, whose weapon is
/// `w`, with the same settings for using it as a movement ability has.
pub(super) fn fuse_abilities(c: &BotChoice, ch: &Section, a: &Section, w: &Section) -> Abilities {
    wrap(c, ch, vec![fuse_ability(a, w)], 0.0)
}

/// The settings of a bot for using its abilities, around the `list` of them.
fn wrap(c: &BotChoice, ch: &Section, list: Vec<Ability>, aim_lift: f32) -> Abilities {
    let secs = |s: &Section, key: &str| s.f32_or(key, 0.0).clamp(0.0, LONGEST_S);
    Abilities {
        // Measured in KovaaK's: a bot profile with no `AbilityUseTimer` key ticks every 0.1 s, and one with no
        // `UseAbilityFrequency` key always presses (a chance of 1). An explicit 0 stays 0 for both. The window
        // keys read as 0 when absent.
        press_every_s: c.bot.f32_or("AbilityUseTimer", DEFAULT_TICK_S).clamp(0.0, LONGEST_S),
        chance: c.bot.f32_or("UseAbilityFrequency", 1.0).clamp(0.0, 1.0),
        window_s: [secs(c.bot, "UseAbilityFreqMinTime"), secs(c.bot, "UseAbilityFreqMaxTime")],
        // KovaaK's waits 1 s for a character with no such key; an explicit 0 is no wait.
        start_block_s: ch.f32_or("BlockAbilityOnStartDuration", 1.0).clamp(0.0, LONGEST_S),
        global_cooldown_s: secs(ch, "AbilityGlobalCooldown"),
        sees_through_walls: c.bot.is("SeeThroughWalls"),
        aim_lift,
        list,
    }
}

/// The highest damage a blast does the bot in the trainer. A file's higher one is cut to it.
const HIGHEST_DAMAGE: f32 = 1.0e6;

/// A weapon ability `a` with the weapon `w` that blows its own bot up, as an [`Ability`]: its charges, timers
/// and gates are read as a movement ability's are, and it has no speed, duration, cost or reaction. Its fuse is
/// the weapon's time between shots and delay after the shot, and a blast's damage times `SelfDamageMultiplier`
/// (missing reads as 1), times cut to [`LONGEST_S`] like the others. (`super::fuse::refusal` has checked the
/// weapon.)
fn fuse_ability(a: &Section, w: &Section) -> Ability {
    let secs = |key: &str| w.f32_or(key, 0.0).clamp(0.0, LONGEST_S);
    let damage = w.f32_or("DamageAtCenter", 0.0) * w.f32_or("SelfDamageMultiplier", 1.0);
    Ability {
        duration_s: 0.0,
        speed: 0.0,
        up_speed: 0.0,
        end_factor: 1.0,
        along_input: false,
        vertical: false,
        no_gravity: false,
        blocks_movement: false,
        health_cost: 0.0,
        reaction: None,
        fuse: Some(Fuse {
            shot_every_s: secs("TimeBetweenShots"),
            burn_s: secs("DelayAfterSpawn"),
            damage: damage.clamp(0.0, HIGHEST_DAMAGE),
        }),
        ..ability(a)
    }
}

/// One movement ability profile.
fn ability(a: &Section) -> Ability {
    let secs = |key: &str| a.f32_or(key, 0.0).clamp(0.0, LONGEST_S);
    let cm = |key: &str| a.f32_or(key, 0.0) / UU_PER_M;
    let far = |key: &str| cm(key).clamp(0.0, FARTHEST_M);
    let flag = |key: &str| a.bool(key).unwrap_or(true);
    let count = |key: &str| a.f32_or(key, 1.0).clamp(0.0, 1000.0) as u32;
    Ability {
        max_charges: count("MaxCharges"),
        spawn_charges: count("ChargesOnSpawn"),
        charge_s: secs("ChargeTimer"),
        delay_s: secs("DelayAfterUse"),
        reuse_s: secs("AIReuseTimer"),
        // `FullyAuto` makes no difference for a bot, so it isn't read.
        // Longer than a minute is refused as a value the trainer doesn't allow.
        duration_s: a.f32_or("AbilityDuration", 0.0).max(0.0),
        speed: cm("MainVelocity"),
        up_speed: cm("UpVelocity"),
        end_factor: a.f32_or("EndVelocityFactor", 1.0).max(0.0),
        along_input: a.is("MainVelocitySetToMovementKeys"),
        vertical: a.is("MainVelocityCanGoVertical"),
        lock_direction: a.is("LockDirectionForDuration"),
        no_gravity: a.is("NegateGravityForDuration"),
        blocks_movement: a.is("AbilityBlocksMovement"),
        in_combat: flag("AIUseInCombat"),
        out_of_combat: flag("AIUseOutOfCombat"),
        on_ground: flag("AIUseOnGround"),
        in_air: flag("AIUseInAir"),
        min_distance: far("AIMinTargDist"),
        // No limit unless the file sets one.
        max_distance: far("AIMaxTargDist"),
        max_angle_deg: a.f32_or("AIMaxTargFOV", 360.0).clamp(0.0, 360.0),
        // In health points, like `MaxHealth`: a cost of 40 takes 40 off a bot of 100.
        health_cost: (-a.f32_or("HealthRestore", 0.0)).max(0.0),
        reaction: reaction(a),
        fuse: None,
    }
}

/// How a bot uses an ability when it is hurt: the settings of `AIDamageReaction`, read as KovaaK's 3.9.13
/// was measured to play them (the rules themselves are written on `BotState::react_to_hit` in the sim).
/// A key that is missing is read as the value KovaaK's editor writes for a new ability (a threshold of 0,
/// a reset timer of 0.1 s, no chance to ignore a trigger and a wait of 0.125 to 0.25 s); that a missing
/// key means that value is inference, not measured. The two ends of the wait are put in order. Times are
/// cut to [`LONGEST_S`]. `AIDamageReactionCooldown` isn't read: it did nothing in any test.
fn reaction(a: &Section) -> Option<Reaction> {
    if !reacts(a) {
        return None;
    }
    let secs = |key: &str, default: f32| a.f32_or(key, default).clamp(0.0, LONGEST_S);
    let (min, max) = (secs("AIDamageReactionMinDelay", 0.125), secs("AIDamageReactionMaxDelay", 0.25));
    Some(Reaction {
        threshold: a.f32_or("AIDamageReactionThreshold", 0.0).clamp(0.0, HIGHEST_THRESHOLD),
        reset_s: secs("AIDamageReactionResetTimer", 0.1),
        ignore_chance: a.f32_or("AIDamageReactionIgnoreChance", 0.0).clamp(0.0, 1.0),
        delay_s: [min.min(max), min.max(max)],
    })
}
