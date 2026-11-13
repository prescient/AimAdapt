//! Bots with heads and the weapon's headshot damage (plan K9). A character's head (`MainBBHasHead`) is a ball on
//! its body with a radius and an offset of its own; a weapon that can headshot (`HeadshotCapable`) does its
//! `HeadshotMultiplier` times the damage to it, and a character that is `HeadshotOnly` takes nothing on its body.
//! The sim's rules are in `aimadapt_sim::sim` (the `geometry` module), measured in KovaaK's 3.9.13 on 12 November
//! 2026 for capsule (`Cylindrical`) and ball (`Spheroid`) bodies. What isn't measured stays refused, each with a
//! detail that says why, or gets a note.

use aimadapt_sim::motion::Motion;
use aimadapt_sim::scenario::Head;

use super::{UU_PER_M, hex_color};
use crate::scenario::BotSlot;
use crate::settings::Section;

/// A character's head as the importer plays it: none unless the character has one (`MainBBHasHead`), so two
/// headless characters whose unused head settings differ play the same. The radius and offset are in meters, the
/// colour is `EnemyHeadColor` unless that is unset (black), when the head keeps the default colour.
pub(super) fn head(ch: &Section) -> Option<Head> {
    ch.is("MainBBHasHead").then(|| Head {
        radius: ch.f32_or("MainBBHeadRadius", 0.0) / UU_PER_M,
        offset: ch.f32_or("MainBBHeadOffset", 0.0) / UU_PER_M,
        color: ch
            .vec3("EnemyHeadColor")
            .filter(|c| c.iter().any(|v| *v > 0.0))
            .map_or_else(|| Head::default().color, hex_color),
        headshots_only: ch.is("HeadshotOnly"),
    })
}

/// Whether a shootable bot has a head (helpers can't be shot, so theirs is never read).
pub(crate) fn any_head<'a, 'b: 'a>(mut shootable: impl Iterator<Item = &'a BotSlot<'b>>) -> bool {
    shootable.any(|s| s.choices.iter().filter_map(|c| c.character).any(|ch| ch.is("MainBBHasHead")))
}

/// The weapon's headshot multiplier when the file sets it for the sim: the weapon can headshot
/// (`HeadshotCapable=true`), the multiplier isn't 1, and either a shootable bot has a head or the scenario scores
/// by damage efficiency (whose possible damage includes the multiplier whether or not there is a head to hit).
/// Otherwise it stays at 1, which leaves every file that played before exactly as it was: a weapon that can headshot
/// with nothing to headshot and no efficiency score changes no score.
pub(crate) fn multiplier(weapon: &Section, any_head: bool, efficiency: bool) -> Option<f32> {
    let m = weapon.f32_or("HeadshotMultiplier", 1.0);
    (weapon.is("HeadshotCapable") && m != 1.0 && (any_head || efficiency)).then_some(m)
}

/// What keeps a shootable character's head and headshot-only setting from being played, as features and details
/// for the inventory's gaps: the first reason for each of "heads" and "headshots only". `weapon` is the player's.
pub(crate) fn refusals(ch: &Section, weapon: Option<&Section>) -> Vec<(&'static str, String)> {
    let name = ch.name();
    let mut out = Vec::new();
    if ch.is("MainBBHasHead") {
        let radius = ch.f32_or("MainBBHeadRadius", 0.0);
        if ch.str("MainBBType") == "Cuboid" {
            out.push(("heads", format!("{name}: a head on a cuboid body wasn't measured")));
        } else if radius <= 0.0 {
            out.push(("heads", format!("{name}: MainBBHeadRadius is {radius}, a head needs a radius above 0")));
        }
    }
    if ch.is("HeadshotOnly") {
        if !ch.is("MainBBHasHead") {
            out.push(("headshots only", format!("{name}: HeadshotOnly=true on a character with no head")));
        } else if !weapon.is_some_and(|w| w.is("HeadshotCapable")) {
            let why = "HeadshotOnly=true, and the player's weapon can't headshot (HeadshotCapable isn't true)";
            out.push(("headshots only", format!("{name}: {why}")));
        }
    }
    out
}

/// The refusal for a weapon whose multiplier the file would set to something the sim can't use.
pub(crate) fn multiplier_refusal(weapon: &Section, any_head: bool, efficiency: bool) -> Option<String> {
    multiplier(weapon, any_head, efficiency)
        .filter(|m| *m <= 0.0)
        .map(|m| format!("{}: HeadshotMultiplier is {m}, it must be above 0", weapon.name()))
}

/// Notes on heads that play by a rule nobody checked in KovaaK's, each once, in order.
pub(super) fn notes(kinds: &[(&Section, Option<Motion>)], notes: &mut Vec<String>) {
    let heads: Vec<(&Section, Head)> = kinds.iter().filter_map(|(ch, _)| head(ch).map(|h| (*ch, h))).collect();
    let mut note = |text: &str| {
        if !notes.iter().any(|n| n == text) {
            notes.push(text.to_string());
        }
    };
    if heads.iter().any(|(_, h)| h.offset < -2.0 * h.radius) {
        note(
            "a head sunk more than twice its radius into its body is placed by the same rule as a shallower one; \
             that wasn't checked against KovaaK's",
        );
    }
    if heads.iter().any(|(ch, h)| h.radius > ch.f32_or("MainBBRadius", 0.0) / UU_PER_M) {
        note(
            "a head wider than its body: the bot bumps into walls and other bots at its body's width; that wasn't \
             checked against KovaaK's",
        );
    }
}

#[cfg(test)]
mod tests;
