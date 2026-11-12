//! Bots that blow themselves up, "the fuse" (plan K8a). Some scenarios give each target a time limit of its
//! own: the bot carries a weapon ability whose weapon hurts the bot itself, so a target the player doesn't kill
//! in time blows itself up and a new one comes. This module decides which of those the trainer plays and why it
//! refuses the others; the rules of the play itself are written on [`aimadapt_sim::motion::Fuse`].
//!
//! The shape that plays is the simplest and most common one. A bot that stays where it spawned (no speed, no
//! gravity, nothing pushes it) and may use weapons, with no weapon of its own and exactly one ability, a
//! weapon ability. That ability's weapon is a projectile that crawls out of the bot, goes off after a short
//! delay while still inside the bot's body and takes health off the bot. Every other weapon ability is refused
//! with the first reason that fails, so [`refusal`] is the one place that says what plays. The inventory
//! ([`crate::inventory::gaps`]) and the converter ([`super::convert`]) both ask it, so they always agree.
//!
//! All sizes and speeds in the file are in KovaaK's units (centimeters), times in seconds. The distances here are
//! only compared with each other, so they stay in units.

use aimadapt_sim::motion::{Abilities, Motion};

use super::{abilities, falls, moves, still_motion};
use crate::inventory::armed;
use crate::scenario::BotChoice;
use crate::settings::{Section, Settings};

/// What the file says beyond the bot itself that decides whether a bot plays as a fuse.
pub(crate) struct Scene<'a> {
    /// The file's profiles, to find the ability's weapon.
    pub settings: &'a Settings,
    /// The scenario's own `InvincibleBots`.
    pub invincible: bool,
    /// Helper bots push the targets around.
    pub pushed: bool,
}

/// How a key and its value read in a reason: `Key=value`, or `no Key` when the file doesn't have it.
fn shown(s: &Section, key: &str) -> String {
    match s.get(key) {
        Some(v) => format!("{key}={}", v.trim()),
        None => format!("no {key}"),
    }
}

/// Why a bot with this weapon ability `a` doesn't play as a fuse, in plain words, or `None` when it does.
pub(crate) fn refusal(scene: &Scene, c: &BotChoice, a: &Section) -> Option<String> {
    let Some(ch) = c.character else { return Some("its bot has no character profile".into()) };
    bot_refusal(scene, c, ch).or_else(|| ability_refusal(a)).or_else(|| weapon_refusal(scene, a, ch))
}

/// The weapon ability of a bot that plays as a fuse.
pub(crate) fn weapon_ability<'a>(scene: &Scene, c: &BotChoice<'a>) -> Option<&'a Section> {
    c.abilities.iter().copied().find(|a| super::abilities::is_kind(a, 0) && refusal(scene, c, a).is_none())
}

/// The first thing about the bot that keeps it from being a fuse.
fn bot_refusal(scene: &Scene, c: &BotChoice, ch: &Section) -> Option<String> {
    if !c.bot.is("UseWeapons") {
        return Some(match c.bot.get("UseWeapons") {
            Some(v) => format!("its bot has UseWeapons={}", v.trim()),
            None => "its bot doesn't set UseWeapons=true".into(),
        });
    }
    // A fuse on a bot that doesn't aim wasn't measured.
    if c.bot.is("NoAiming") {
        return Some("its bot has NoAiming=true".into());
    }
    if armed(c) {
        return Some("its bot carries a weapon of its own".into());
    }
    if c.abilities.len() != 1 {
        return Some(format!("its character has {} abilities; a fuse bot has this one alone", c.abilities.len()));
    }
    if moves(c, ch) {
        return Some("its bot moves by itself".into());
    }
    if scene.pushed {
        return Some("helper bots push the targets".into());
    }
    if falls(ch) {
        return Some(format!("its character has {}", shown(ch, "Gravity")));
    }
    if scene.invincible || ch.is("InvincibleBots") {
        return Some("its bot can't die (InvincibleBots=true)".into());
    }
    if ch.is("BlockSelfDamage") {
        return Some("its character has BlockSelfDamage=true".into());
    }
    // A blast on a bot that heals or drains wasn't measured.
    if ch.f32_or("HealthRegenPerSec", 0.0) != 0.0 {
        return Some(format!("its character has {}", shown(ch, "HealthRegenPerSec")));
    }
    None
}

/// The first thing about the ability's own settings that keeps its bot from being a fuse.
fn ability_refusal(a: &Section) -> Option<String> {
    if a.is("AIDamageReaction") {
        return Some("AIDamageReaction=true".into());
    }
    // A bot that stays put has no ground state, and what counts as combat for a weapon ability wasn't measured.
    for key in ["AIUseInCombat", "AIUseOutOfCombat", "AIUseOnGround", "AIUseInAir"] {
        if a.bool(key) == Some(false) {
            return Some(format!("{key}=false"));
        }
    }
    // A health gate that is closed some of the time wasn't measured.
    for key in ["AIMinSelfHealth", "AIMinTargHealth"] {
        if a.f32_or(key, 0.0) > 0.0 {
            return Some(shown(a, key));
        }
    }
    for key in ["AIMaxSelfHealth", "AIMaxTargHealth"] {
        if a.f32_or(key, 100.0) < 100.0 {
            return Some(shown(a, key));
        }
    }
    None
}

/// A vector's length, in units, or `None` when the key is missing or isn't a vector.
fn length(w: &Section, key: &str) -> Option<f32> {
    w.vec3(key).map(|[x, y, z]| (x * x + y * y + z * z).sqrt())
}

/// The first thing about the ability's weapon that keeps its bot from being a fuse.
fn weapon_refusal(scene: &Scene, a: &Section, ch: &Section) -> Option<String> {
    let name = a.str("WeaponProfile").trim();
    let Some(w) = scene.settings.find("Weapon Profile", name) else {
        return Some(match name {
            "" => "it has no WeaponProfile".to_string(),
            _ => format!("its WeaponProfile={name} is no weapon profile in the file"),
        });
    };
    let text = |key: &str, want: &str| w.str(key).trim().eq_ignore_ascii_case(want);
    let has = |bad: String| Some(format!("its weapon has {bad}"));
    if !text("Type", "Projectile") {
        return has(shown(w, "Type"));
    }
    if !w.is("Explosive") {
        return has(shown(w, "Explosive"));
    }
    if !text("Category", "SemiAuto") {
        return has(shown(w, "Category"));
    }
    for key in ["IsBurstWeapon", "ExplodesOnContactWithEnemy", "ExplodesOnContactWithWorld", "ExplodesOnNextAttack"] {
        if w.is(key) {
            return has(format!("{key}=true"));
        }
    }
    for (key, ok) in [("ShotsPerClick", 1.0), ("DelayBeforeShot", 0.0), ("MagazineMax", 0.0)] {
        if w.f32_or(key, ok) != ok {
            return has(shown(w, key));
        }
    }
    if w.f32("GravityScale") != Some(0.0) {
        return has(format!("{}; it must be 0", shown(w, "GravityScale")));
    }
    if w.f32_or("TimeBetweenShots", 0.0) <= 0.0 {
        return has(shown(w, "TimeBetweenShots"));
    }
    let (delay, travel) = (w.f32_or("DelayAfterSpawn", 0.0), w.f32_or("MaxTravelTime", 0.0));
    if delay <= 0.0 || delay >= travel {
        let both = format!("{} and {}", shown(w, "DelayAfterSpawn"), shown(w, "MaxTravelTime"));
        return has(format!("{both}; the delay must be above 0 and below the travel time"));
    }
    let (center, edge) = (w.f32_or("DamageAtCenter", 0.0), w.f32_or("DamageAtEdge", 0.0));
    if center != edge {
        return has(format!("{} and {}, which differ", shown(w, "DamageAtCenter"), shown(w, "DamageAtEdge")));
    }
    if center * w.f32_or("SelfDamageMultiplier", 1.0) <= 0.0 {
        let both = format!("{} and {}", shown(w, "DamageAtCenter"), shown(w, "SelfDamageMultiplier"));
        return has(format!("{both}, so a blast does its bot no damage"));
    }
    reach_refusal(w, ch, delay)
}

/// A blast that could reach outside the bot's body could hurt something else, which wasn't measured. The
/// farthest the projectile gets before it goes off (the longer muzzle speed times the delay, plus how far
/// from the bot's middle it starts) plus the blast's radius must stay inside the body's nearest side.
fn reach_refusal(w: &Section, ch: &Section, delay: f32) -> Option<String> {
    let unknown = |key: &str| format!("its weapon has {}; the blast's reach can't be worked out", shown(w, key));
    let vector = |key: &str| length(w, key).ok_or_else(|| unknown(key));
    let reach = || -> Result<f32, String> {
        let speed = vector("MuzzleVelocityMin")?.max(vector("MuzzleVelocityMax")?);
        let radius = w.f32("Radius").ok_or_else(|| unknown("Radius"))?;
        Ok(speed * delay + vector("OriginOffset")? + radius)
    };
    let reach = match reach() {
        Ok(reach) => reach,
        Err(why) => return Some(why),
    };
    let across = ch.f32_or("MainBBRadius", 0.0);
    let body =
        if ch.str("MainBBType") == "Spheroid" { across } else { across.min(ch.f32_or("MainBBHeight", 0.0) / 2.0) };
    (reach > body).then(|| {
        format!("its weapon's blast reaches {reach:.1} units from the bot's middle, outside its body ({body:.1} units)")
    })
}

/// A fuse bot's motion: it stays where it appeared and has this one ability, the weapon `a`, read as a movement
/// ability's settings are (see `abilities::fuse_abilities`).
pub(super) fn motion(scene: &Scene, c: &BotChoice, ch: &Section, player: Option<&Section>, a: &Section) -> Motion {
    let weapon = scene.settings.find("Weapon Profile", a.str("WeaponProfile").trim());
    let abilities: Option<Abilities> = weapon.map(|w| abilities::fuse_abilities(c, ch, a, w));
    Motion { abilities, stays_put: true, ..still_motion(c, ch, player) }
}

#[cfg(test)]
mod tests;
