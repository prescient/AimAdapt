//! Several pellets per shot (plan K11a). A weapon's `ShotsPerClick` is the number of pellets a click fires, and
//! `DamagePerShot` is one pellet's damage. The sim's rules are in `aimadapt_sim::sim` (`fire_shot`), measured in
//! KovaaK's 3.9.13 on 12 November 2026: a click is one shot and, if it lands, one hit, all its pellets go on the
//! first body the aim line meets, and the damage counted is pellets times damage. A weapon that spreads its pellets
//! is refused, because the spread law wasn't measured: the only thing known is that a weapon whose `SpreadSSH`
//! pattern (standing still, not aiming down sights) ends in 0 doesn't spread, whatever the other seven patterns say,
//! and one whose pattern ends above 0 blooms under held fire and misses.

use crate::settings::Section;

/// The most pellets a click can fire in the sim.
const MOST: f32 = 1000.0;

/// How many pellets the weapon fires a click: its `ShotsPerClick` when that is a whole number from 2 to 1000, and
/// otherwise 1 (a count of 1 or below, or none, is the plain weapon; a count that is refused is never played).
pub(crate) fn count(weapon: &Section) -> u32 {
    let n = weapon.f32_or("ShotsPerClick", 1.0);
    if n > 1.0 && n <= MOST && n.fract() == 0.0 { n as u32 } else { 1 }
}

/// What keeps the weapon's pellets from being played, as features and details for the inventory's gaps. The
/// pattern's fourth number is read whatever the pellet count is, so a one-pellet weapon that spreads is refused
/// too. A weapon with no `SpreadSSH` line plays with no spread. The other seven patterns, `UsePerBulletSpread`,
/// `SpreadStationaryVelocity` and `SpreadDecayDelay` are not read: the player can't crouch or move in an imported
/// scenario today and aiming down sights is refused on its own.
pub(crate) fn refusals(weapon: &Section) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let n = weapon.f32_or("ShotsPerClick", 1.0);
    if n > 1.0 {
        let shown = weapon.str("ShotsPerClick");
        if n.fract() != 0.0 {
            out.push(("several pellets per shot", format!("ShotsPerClick={shown} isn't a whole number")));
        } else if n > MOST {
            out.push(("several pellets per shot", format!("ShotsPerClick={shown} is above {MOST}")));
        }
    }
    if let Some(value) = weapon.get("SpreadSSH") {
        match last_of_four(value) {
            None => out.push(("weapon spread", format!("SpreadSSH={value} can't be read as four numbers"))),
            Some(fourth) if fourth != 0.0 => out.push((
                "weapon spread",
                format!(
                    "SpreadSSH={value} has a fourth number other than 0, so shots spread when the player stands still"
                ),
            )),
            Some(_) => {}
        }
    }
    out
}

/// The fourth of exactly four comma-separated numbers, spaces allowed around them.
fn last_of_four(value: &str) -> Option<f32> {
    let parts: Vec<f32> =
        value.split(',').map(|p| p.trim().parse::<f32>().ok().filter(|v| v.is_finite())).collect::<Option<_>>()?;
    (parts.len() == 4).then(|| parts[3])
}

/// The note for a weapon that fires several pellets when a shootable bot has a head, once: pellets against a head
/// were not measured.
pub(super) fn notes(weapon: &Section, any_head: bool, notes: &mut Vec<String>) {
    let n = count(weapon);
    if n > 1 && any_head {
        notes.push(format!(
            "the weapon fires {n} pellets a click, and all of them count on the part the aim line meets first; \
             pellets against a head weren't checked in KovaaK's"
        ));
    }
}

#[cfg(test)]
mod tests;
