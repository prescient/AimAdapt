//! Helper bots: bots on the player's own team that push the targets around with a
//! melee ability that does no damage (Raw Control, Controlsphere). They can't be shot
//! and aren't scored, so they aren't targets; each becomes a [`Pusher`] in the world.
//!
//! Two kinds were measured in KovaaK's. A pinned one stays where it spawns and faces
//! the way its spawn point does. A chasing one flies at its target and is stopped by
//! an invisible slab the map puts between them. It has a drag and an acceleration, so
//! it trails a target that moves and takes time to cross to one that appears
//! elsewhere; its character's `Acceleration`, `MaxSpeed` and `DragCoefficient` become
//! the pusher's flight. Any helper may wait before its first hit (`BlockAbilityOnStartDuration`).

use std::collections::HashMap;

use aimadapt_sim::Vec3;
use aimadapt_sim::collide::Collider;
use aimadapt_sim::world::{ChaseFlight, Pusher, WorldMesh, WorldSpawn};

use super::abilities::is_kind;
use super::{UU_PER_M, axes, push_once, rise};
use crate::inventory::{Gap, armed};
use crate::map::{Map, Spawn};
use crate::scenario::{BotChoice, Resolved};
use crate::settings::Section;

/// The longest start delay the trainer allows, in seconds.
const LONGEST_START_S: f32 = 3600.0;
/// The longest swing the trainer models: a longer one hits once per entry.
const LONGEST_SWING_S: f32 = 0.05;
/// The shortest time between hits the trainer allows.
const SHORTEST_PERIOD_S: f32 = 0.005;

/// A helper bot, read from the file: what it does, and the spawn point it takes.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Helper {
    /// Its character's name: spawn points name the characters that may use them.
    character: String,
    /// Which of the helper slots with this character it is, counting from 0.
    nth: usize,
    chases: bool,
    /// How a chaser flies, in meters, seconds and m/s.
    flight: Option<ChaseFlight>,
    /// Seconds from the start of the run before its first hit.
    start_s: f32,
    /// Meters, m/s, m/s and seconds.
    radius: f32,
    push: f32,
    lift: f32,
    every_s: f32,
    needs_sight: bool,
    /// Its height in meters, and the middle of its spawn offset in the sim's axes
    /// (right, up, back).
    height: f32,
    offset: Vec3,
}

/// The helpers a file has.
#[derive(Debug, Default)]
pub(super) struct Plan {
    /// The ones that push.
    pub helpers: Vec<Helper>,
    /// The characters of every helper slot, whether it pushes or is left out: their
    /// spawn points are theirs, not the player's.
    pub characters: Vec<String>,
}

fn refuse(reasons: &mut Vec<Gap>, detail: String) {
    push_once(reasons, Gap { feature: "helper bots", detail, beyond_plan: true });
}

/// The seconds between a helper's hits. Its ability can be used again once its reuse
/// timer, its charge and its delay are over, and the bot presses the button every
/// `tick` seconds, so a use comes on the first press that is late enough (a tie
/// counts) and never before the first. With no press timer it uses it as soon as it can.
fn period(tick: f32, reuse: f32, duration: f32, charge: f32, delay: f32) -> f32 {
    let (tick, wait) =
        (f64::from(tick), f64::from(reuse).max(f64::from(duration + charge)).max(f64::from(duration + delay)));
    let every = if tick > 0.0 { (wait / tick - 1e-4).ceil().max(1.0) * tick } else { wait };
    every as f32
}

/// How a helper is held: pinned, or chasing its target (flying at it, whatever its
/// speed: every chaser follows the same law). `None` for neither.
fn chases(c: &BotChoice, ch: &Section) -> Option<bool> {
    let no_dodging = c.bot.is("NoDodging");
    let speed = ch.f32_or("MaxSpeed", 0.0);
    if c.bot.is("NoAiming") {
        return (no_dodging || speed <= 0.0).then_some(false);
    }
    // Always moves forward at its target: a dodge profile that starts forward and
    // never goes back.
    let forward = |d: &Section| {
        d.str("InitialForwardMovementState").trim().eq_ignore_ascii_case("Forward")
            && (!d.is("ToggleForwardBack") || d.f32_or("BackTimeMult", 1.0) <= 0.01)
    };
    let chaser = !no_dodging && ch.is("IsFlyer") && !c.dodges.is_empty() && c.dodges.iter().all(|(d, _)| forward(d));
    chaser.then_some(true)
}

/// A chaser's flight, from its character: `Acceleration` and `MaxSpeed` (units/s² and
/// units/s) in meters, and `DragCoefficient` as it is. One that is missing or 0 is
/// refused, naming the helper and the setting.
fn flight(who: &str, ch: &Section, reasons: &mut Vec<Gap>) -> Option<ChaseFlight> {
    let mut get = |key: &str| {
        let v = ch.f32_or(key, 0.0);
        if v > 0.0 {
            return v;
        }
        let was = if ch.str(key).trim().is_empty() { "missing".to_string() } else { ch.str(key).trim().to_string() };
        refuse(reasons, format!("{who}: a chaser needs {key} above 0 on its character ({key} is {was})"));
        0.0
    };
    let (accel, max_speed, drag) = (get("Acceleration"), get("MaxSpeed"), get("DragCoefficient"));
    (accel > 0.0 && max_speed > 0.0 && drag > 0.0).then(|| ChaseFlight {
        accel: accel / UU_PER_M,
        max_speed: max_speed / UU_PER_M,
        drag,
    })
}

/// A helper's start delay: its character's `BlockAbilityOnStartDuration`, 0 without
/// one. Refused above an hour.
fn start_delay(who: &str, ch: &Section, reasons: &mut Vec<Gap>) -> f32 {
    let s = ch.f32_or("BlockAbilityOnStartDuration", 0.0).max(0.0);
    if s > LONGEST_START_S {
        refuse(reasons, format!("{who}: BlockAbilityOnStartDuration={}", ch.str("BlockAbilityOnStartDuration").trim()));
    }
    s
}

/// Reads the file's helper slots: the helpers the trainer plays, and the reasons it
/// can't play the others. A helper with nothing to push with is left out, with a note.
pub(super) fn plan(r: &Resolved, player_team: u32, reasons: &mut Vec<Gap>, notes: &mut Vec<String>) -> Plan {
    let mut plan = Plan::default();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for slot in r.bots.iter().filter(|s| s.is_helper(player_team)) {
        if slot.choices.len() > 1 {
            refuse(reasons, format!("{}: the slot has {} bots to choose from", slot.reference, slot.choices.len()));
            continue;
        }
        let Some(c) = slot.choices.first() else { continue };
        let Some(ch) = c.character else { continue };
        let name = ch.name().to_string();
        let key = name.to_ascii_lowercase();
        let nth = counts.entry(key).or_default();
        *nth += 1;
        plan.characters.push(name.clone());
        let Some(helper) = read(c, ch, name, *nth - 1, reasons, notes) else { continue };
        plan.helpers.push(helper);
    }
    plan
}

/// One helper: `None` when it is refused (with its reasons added) or left out (with a
/// note).
fn read(
    c: &BotChoice,
    ch: &Section,
    character: String,
    nth: usize,
    reasons: &mut Vec<Gap>,
    notes: &mut Vec<String>,
) -> Option<Helper> {
    let who = c.bot.name();
    let before = reasons.len();
    let (melee, other): (Vec<&Section>, Vec<&Section>) = c.abilities.iter().partition(|a| is_kind(a, 1));
    for a in other {
        refuse(reasons, format!("{who}: {} is not a melee ability", a.name()));
    }
    if melee.len() > 1 {
        refuse(reasons, format!("{who}: {} melee abilities; the trainer plays one", melee.len()));
    }
    let Some(a) = melee.first().copied() else {
        if reasons.len() == before {
            notes.push(format!("the helper bot {who} has no ability and is left out"));
        }
        return None;
    };
    let name = a.name();
    let mut check = |bad: bool, key: &str, from: &Section| {
        if bad {
            refuse(reasons, format!("{name}: {key}={}", from.str(key).trim()));
        }
    };
    check(a.f32_or("HurtboxDamage", 0.0) != 0.0, "HurtboxDamage", a);
    let horizontal = a.f32_or("FlatKnockbackHorizontal", 0.0);
    let vertical = a.f32_or("FlatKnockbackVertical", 0.0);
    // A push that varies between a smallest and a largest value isn't played.
    check(a.f32("FlatKnockbackHorizontalMin").is_some_and(|v| v != horizontal), "FlatKnockbackHorizontalMin", a);
    check(a.f32("FlatKnockbackVerticalMin").is_some_and(|v| v != vertical), "FlatKnockbackVerticalMin", a);
    // A long swing hits once for each target that enters it, not on a schedule.
    check(a.f32_or("AbilityDuration", 0.0) > LONGEST_SWING_S, "AbilityDuration", a);
    check(a.f32_or("MaxCharges", 1.0) != 1.0, "MaxCharges", a);
    check(a.f32_or("AIMaxTargFOV", 360.0) < 360.0, "AIMaxTargFOV", a);
    check(a.f32_or("HurtboxRadius", 0.0) <= 0.0, "HurtboxRadius", a);
    let radius = a.f32_or("HurtboxRadius", 0.0);
    check(a.f32("AIMaxTargDist").is_some_and(|v| v < 2.0 * radius), "AIMaxTargDist", a);
    check(!a.bool("AIUseInCombat").unwrap_or(true), "AIUseInCombat", a);
    check(c.bot.f32_or("UseAbilityFrequency", 0.0) < 1.0, "UseAbilityFrequency", c.bot);
    // A helper with a weapon wasn't measured, aiming or not: a helper's `NoAiming` means
    // something else here (it pins the helper).
    if armed(c) {
        refuse(reasons, format!("{who}: UseWeapons=true with a weapon"));
    }
    let every = period(
        c.bot.f32_or("AbilityUseTimer", 0.0).max(0.0),
        a.f32_or("AIReuseTimer", 0.0).max(0.0),
        a.f32_or("AbilityDuration", 0.0).max(0.0),
        a.f32_or("ChargeTimer", 0.0).max(0.0),
        a.f32_or("DelayAfterUse", 0.0).max(0.0),
    );
    if every < SHORTEST_PERIOD_S {
        refuse(
            reasons,
            format!("{name}: it hits every {every} s; the trainer allows {SHORTEST_PERIOD_S} s at the least"),
        );
    }
    let held = chases(c, ch);
    if held.is_none() {
        let detail = format!(
            "{who}: neither pinned (NoAiming with NoDodging or no speed) nor chasing (a flier that always goes forward); \
             NoAiming={}, NoDodging={}, MaxSpeed={}",
            c.bot.str("NoAiming").trim(),
            c.bot.str("NoDodging").trim(),
            ch.str("MaxSpeed").trim()
        );
        refuse(reasons, detail);
    }
    let flight = if held == Some(true) { flight(who, ch, reasons) } else { None };
    let start_s = start_delay(who, ch, reasons);
    if reasons.len() > before {
        return None;
    }
    if horizontal == 0.0 && vertical == 0.0 {
        notes.push(format!("the helper bot {who} pushes by nothing and is left out"));
        return None;
    }
    if start_s > 0.0 {
        // One note for each delay, however many helpers have it.
        let note = format!(
            "helper bots with BlockAbilityOnStartDuration={start_s} are played as making no hits for the first \
             {start_s} s of the run; what KovaaK's does with it for a helper wasn't measured"
        );
        if !notes.contains(&note) {
            notes.push(note);
        }
    }
    let offset = |key: &str| axes(ch.vec3(key).unwrap_or_default()) / UU_PER_M;
    Some(Helper {
        character,
        nth,
        chases: held == Some(true),
        flight,
        start_s,
        radius: radius / UU_PER_M,
        push: horizontal / UU_PER_M,
        lift: vertical / UU_PER_M,
        every_s: every,
        needs_sight: !c.bot.is("SeeThroughWalls"),
        height: ch.f32_or("MainBBHeight", 0.0).max(0.0) / UU_PER_M,
        offset: (offset("SpawnOffsetMin") + offset("SpawnOffsetMax")) / 2.0,
    })
}

fn names(s: &Spawn, character: &str) -> bool {
    s.characters.iter().any(|c| c.eq_ignore_ascii_case(character))
}

/// Whether a spawn point is a helper's: it names a helper's character and not the
/// player's, so the player doesn't start there.
pub(super) fn is_reserved(s: &Spawn, plan: &Plan, player: Option<&Section>) -> bool {
    plan.characters.iter().any(|c| names(s, c)) && !player.is_some_and(|p| names(s, p.name()))
}

/// The spawn point each helper takes, as indices into the map's: one the player's
/// team can use whose list names the helper's character. The n-th helper with a
/// character takes the n-th such point in file order.
pub(super) fn assign(
    map: &Map,
    plan: &Plan,
    usable: impl Fn(&Spawn) -> bool,
    reasons: &mut Vec<Gap>,
) -> Option<Vec<usize>> {
    let mut at = Vec::new();
    for h in &plan.helpers {
        let point = (0..map.spawns.len())
            .filter(|i| usable(&map.spawns[*i]) && names(&map.spawns[*i], &h.character))
            .nth(h.nth);
        match point {
            Some(i) => at.push(i),
            None => {
                let detail = format!("{}: helper {} has no spawn point", h.character, h.nth + 1);
                reasons.push(Gap::planned("the map has no spawn point a helper bot can use", detail));
                return None;
            }
        }
    }
    Some(at)
}

/// Where a helper's spawn point is, placed like a target's, and its three axes in
/// the sim's frame: right, up and back.
pub(super) struct Frame {
    pub at: Vec3,
    pub axes: [Vec3; 3],
}

/// The helpers as pushers, each at its spawn point's `frame`. A chasing one needs a
/// wall bots can't pass between it and every place a target appears; without one it
/// would fly to its target.
pub(super) fn pushers(
    plan: &Plan,
    frames: &[Frame],
    meshes: &[WorldMesh],
    spawns: &[WorldSpawn],
    reasons: &mut Vec<Gap>,
) -> Vec<Pusher> {
    let walls = Collider::for_bots(meshes);
    let mut out = Vec::new();
    for (h, f) in plan.helpers.iter().zip(frames) {
        let offset: Vec3 = f.axes.iter().zip(h.offset.to_array()).map(|(a, o)| *a * o).sum();
        let pos = f.at + rise(h.height) + offset;
        if h.chases {
            let open = spawns.iter().any(|s| {
                let to =
                    Vec3::from_array(s.pos) + s.edges.iter().map(|e| Vec3::from_array(*e) / 2.0).sum::<Vec3>() - pos;
                let d = to.length();
                d < 1e-6 || walls.ray(pos, to / d, d).is_none()
            });
            if open {
                let detail = format!("{}: it chases its target with no wall between them", h.character);
                refuse(reasons, detail);
                continue;
            }
        }
        out.push(Pusher {
            pos: pos.to_array(),
            facing: (-f.axes[2]).normalize_or(Vec3::NEG_Z).to_array(),
            chases: h.chases,
            flight: h.flight.clone(),
            start_s: h.start_s,
            radius: h.radius,
            every_s: h.every_s,
            push: h.push,
            lift: h.lift,
            needs_sight: h.needs_sight,
        });
    }
    out
}

#[cfg(test)]
mod tests;
