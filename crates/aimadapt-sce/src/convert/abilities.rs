//! A bot's abilities: movement abilities (dashes, blinks, ability jumps) become the
//! bot's [`Abilities`]; the ones the trainer can't play yet come back as reasons.
//! Velocities and distances in the file are centimeters, times are seconds.

use aimadapt_sim::motion::{Abilities, Ability};

use super::{UU_PER_M, push_once};
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
    // In KovaaK's a dash that can go up or down with a main velocity launches the bot high
    // into the air; the trainer can't play that. Ability jumps (a velocity of 0 and an
    // up velocity) are fine.
    ("MainVelocityCanGoVertical", |a| a.is("MainVelocityCanGoVertical") && a.f32_or("MainVelocity", 0.0) != 0.0),
    // Not measured in KovaaK's.
    ("AIDamageReaction", |a| a.is("AIDamageReaction")),
];

/// Whether a movement ability costs the bot health each time it uses it (a negative
/// `HealthRestore`).
fn costs_health(a: &Section) -> bool {
    a.f32_or("HealthRestore", 0.0) < 0.0
}

/// Reasons a target's abilities can't be played yet. A target's melee ability (a
/// knockback) isn't played: only helper bots push, see [`super::helpers`]. Weapon
/// abilities are reported with the file's other gaps. `ch` is the bot's character and
/// `top_invincible` whether the scenario's `InvincibleBots` is on.
pub(super) fn reasons(c: &BotChoice, ch: &Section, top_invincible: bool, reasons: &mut Vec<Gap>) {
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

/// The longest time and distance the trainer allows. More than an hour or 10 km is
/// never or everywhere in any run, so larger values in a file are cut to these.
const LONGEST_S: f32 = 3600.0;
const FARTHEST_M: f32 = 10_000.0;

/// The bot's movement abilities and the settings for using them, or none if it has
/// no movement ability. `ch` is its character profile.
pub(super) fn abilities(c: &BotChoice, ch: &Section) -> Option<Abilities> {
    let list: Vec<Ability> = movement(c).map(ability).collect();
    if list.is_empty() {
        return None;
    }
    let secs = |s: &Section, key: &str| s.f32_or(key, 0.0).clamp(0.0, LONGEST_S);
    Some(Abilities {
        press_every_s: secs(c.bot, "AbilityUseTimer"),
        chance: c.bot.f32_or("UseAbilityFrequency", 0.0).clamp(0.0, 1.0),
        window_s: [secs(c.bot, "UseAbilityFreqMinTime"), secs(c.bot, "UseAbilityFreqMaxTime")],
        start_block_s: secs(ch, "BlockAbilityOnStartDuration"),
        global_cooldown_s: secs(ch, "AbilityGlobalCooldown"),
        sees_through_walls: c.bot.is("SeeThroughWalls"),
        list,
    })
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
    }
}
