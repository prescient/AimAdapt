//! Rules that decide when a run ends and how it is scored: bot lives, and notes on
//! score rules that were only checked on their own.

use super::push_once;
use crate::inventory::Gap;
use crate::scenario::BotSlot;
use crate::settings::Section;

const LIVES: &str = "bots with a set number of lives";

/// A lives rule KovaaK's was never measured on.
fn unmeasured(detail: String) -> Gap {
    Gap { feature: LIVES, detail: format!("{detail}; that hasn't been measured in KovaaK's"), beyond_plan: true }
}

/// The kills that end the run (0 for no limit), from `BotMaxLives`: a list with one number
/// per bot slot, in the order of `AddedBots`, where 0 means no limit. Measured in KovaaK's
/// on files with one slot: `BotMaxLives=N` gave exactly N kills, each of the next bot in the
/// slot's rotation, and the run ended on the last kill. Anything else is refused.
///
/// Bots that can't die have no lives to use up: the scenario's `InvincibleBots`, or a slot
/// whose characters all have it.
pub(super) fn kill_limit(top: &Section, slots: &[BotSlot], player_team: u32, reasons: &mut Vec<Gap>) -> u32 {
    if top.is("InvincibleBots") {
        return 0;
    }
    let lives: Vec<u32> = top
        .list("BotMaxLives")
        .iter()
        .map(|v| v.parse::<f32>().ok().filter(|n| n.is_finite() && *n > 0.0).map_or(0, |n| n.round().max(1.0) as u32))
        .collect();
    if lives.iter().all(|n| *n == 0) {
        return 0;
    }
    if lives.len() != slots.len() {
        let detail = format!("BotMaxLives has {} number(s) for {} bot slot(s)", lives.len(), slots.len());
        push_once(reasons, unmeasured(detail));
        return 0;
    }
    let is_target = |i: &usize| !slots[*i].is_helper(player_team);
    if (0..slots.len()).any(|i| lives[i] > 0 && !is_target(&i)) {
        push_once(reasons, unmeasured("BotMaxLives is set on a helper bot".into()));
        return 0;
    }
    let targets: Vec<usize> = (0..slots.len()).filter(is_target).collect();
    if targets.len() > 1 {
        let all = targets.iter().all(|i| lives[*i] > 0);
        let detail =
            if all { "BotMaxLives is set on several bots" } else { "BotMaxLives is set on some bots and not others" };
        push_once(reasons, unmeasured(detail.into()));
        return 0;
    }
    let Some(&only) = targets.first() else { return 0 };
    let invincible: Vec<bool> =
        slots[only].choices.iter().filter_map(|c| c.character).map(|ch| ch.is("InvincibleBots")).collect();
    if invincible.iter().all(|i| *i) {
        return 0;
    }
    if invincible.iter().any(|i| *i) {
        push_once(reasons, unmeasured("BotMaxLives is set on a bot that is only sometimes invincible".into()));
        return 0;
    }
    lives[only]
}

/// Notes on score rules whose combination was never checked against KovaaK's result files:
/// the time score alone, and damage efficiency scoring by damage only, were.
pub(super) fn score_notes(top: &Section, notes: &mut Vec<String>) {
    let accuracy = top.is("ScoreMultAccuracy");
    let efficiency = top.is("ScoreMultDamageEfficiency");
    let time = top.f32_or("ScorePerTime", 0.0) != 0.0;
    if time && (accuracy || efficiency) {
        notes.push(
            "the score multiplier applies to the points for time left too; that hasn't been checked against KovaaK's"
                .into(),
        );
    }
    if accuracy && efficiency {
        notes.push(
            "the score is multiplied by both accuracy and damage efficiency; that hasn't been checked against KovaaK's"
                .into(),
        );
    }
    let others = ["ScorePerKill", "ScorePerHit", "ScoreLossPerMiss"].iter().any(|k| top.f32_or(k, 0.0) != 0.0);
    if efficiency && others {
        notes.push(
            "damage efficiency multiplies the points for kills, hits and misses too; that hasn't been checked \
             against KovaaK's"
                .into(),
        );
    }
}
