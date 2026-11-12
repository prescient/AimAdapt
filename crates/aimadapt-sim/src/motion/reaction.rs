//! A bot that uses a movement ability because it was hurt (KovaaK's `AIDamageReaction`). The rules are
//! written on [`BotState::react_to_hit`].

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::ability::{TIE_S, View};
use super::{BotState, Motion, between};
use crate::rng::Rng;

/// When a bot uses an ability because it was hurt (KovaaK's `AIDamageReaction`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reaction {
    /// A hit is a trigger when the damage since the last trigger reaches this. 0: every hit.
    pub threshold: f32,
    /// The damage counted so far is forgotten when no hit has landed for this long.
    pub reset_s: f32,
    /// The chance, 0 to 1, that a trigger is ignored.
    pub ignore_chance: f32,
    /// The wait between a trigger and the use, drawn evenly between the two.
    pub delay_s: [f32; 2],
}

impl Default for Reaction {
    /// The values KovaaK's editor writes for a new ability.
    fn default() -> Self {
        Reaction { threshold: 0.0, reset_s: 0.1, ignore_chance: 0.0, delay_s: [0.125, 0.25] }
    }
}

/// How far a sum of damage may fall short of the threshold and still reach it: a beam's damage per tick
/// added up in `f32` comes out a hair low.
const SUM_SLACK: f32 = 1e-3;

/// What one ability's reaction has counted and is waiting to do. Every time is on the bot's clock.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Pending {
    /// The damage taken since the last trigger.
    sum: f32,
    /// When the last hit landed.
    last_hit: Option<f32>,
    /// When the use that a trigger started is due.
    due: Option<f32>,
}

impl Pending {
    /// A hit of `damage` at `clock`. Whether it is a trigger.
    fn hit(&mut self, r: &Reaction, damage: f32, clock: f32) -> bool {
        if self.last_hit.is_some_and(|last| clock - last > r.reset_s.max(0.0) + TIE_S) {
            self.sum = 0.0;
        }
        self.last_hit = Some(clock);
        self.sum += damage;
        let trigger = self.sum + SUM_SLACK >= r.threshold;
        if trigger {
            self.sum = 0.0;
        }
        trigger
    }
}

/// Which waiting reactions to handle.
#[derive(Debug, Clone, Copy)]
pub(super) enum Until {
    /// Those due strictly before a tick of the key at this time.
    Before(f32),
    /// Those due by this time, the clock of the frame.
    By(f32),
}

impl Until {
    fn has(self, due: f32) -> bool {
        match self {
            Until::Before(at) => due + TIE_S < at,
            Until::By(clock) => due <= clock + TIE_S,
        }
    }
}

impl BotState {
    /// The bot took `damage`: counts it for each ability that has a reaction. Measured in KovaaK's
    /// 3.9.13 with a flying ball that blinks to and fro as its reaction, under a held automatic weapon and
    /// under single shots:
    ///
    /// - A hit is a trigger when the damage taken since the last trigger reaches
    ///   `AIDamageReactionThreshold`; with a threshold of 0 every hit is one. The sum starts again at the
    ///   trigger. (A threshold of 50 under shots of 10 gave a trigger at every fifth hit.)
    /// - The reset timer starts again at each hit: the sum carried across hits 0.3 s apart with a timer of
    ///   1.0 s. That the sum is cleared when the timer runs out is inferred from the name.
    /// - Hits on a bot that can't be hurt (`InvincibleBots`) count, with the weapon's damage.
    /// - Each trigger is ignored with the chance `AIDamageReactionIgnoreChance`, each by itself.
    /// - A trigger that isn't ignored starts a wait drawn evenly between `AIDamageReactionMinDelay` and
    ///   `AIDamageReactionMaxDelay`, and the ability is used when it runs out. A new trigger while one is
    ///   waiting starts the wait again (a new draw); a hit below the threshold doesn't.
    /// - When the wait is over the ability needs a charge, and `DelayAfterUse` since its last press must be
    ///   over (a tie isn't enough). A reaction that fails either is dropped; it doesn't wait for the charge.
    /// - `AIReuseTimer` and the ticks of the bot's key play no part in a reaction, and
    ///   `AIDamageReactionCooldown` did nothing in any test, so it isn't read.
    /// - With a `UseAbilityFrequency` of 0 the bot uses the ability only as a reaction. Above 0 it also
    ///   presses its key as before, and both draw on the same charges.
    /// - The direction of the blink follows the rules for that ability.
    ///
    /// Not measured, and played as follows: a reaction that comes due while an ability is going on, or at
    /// the moment one ends, is dropped (the same test as a try of the key). The ability's own rules
    /// (combat, ground or air, distance, view angle), the shared cooldown (`AbilityGlobalCooldown`) and
    /// `BlockAbilityOnStartDuration` hold a reaction back the same way: it is dropped. An ignored trigger
    /// leaves a waiting reaction alone. A reaction is no try of the key: it doesn't touch the reuse timer.
    /// A tick of the key and a reaction due at the same moment: the tick goes first. With a reset timer
    /// of 0 any gap between hits clears the sum.
    ///
    /// The numbers drawn come only from abilities that have a reaction (the ignore roll, then the wait), so
    /// a bot without one keeps its random stream. The time of a hit is the bot's own clock. The sim calls
    /// this after the bots have moved, so it is the bot's age at the end of the frame.
    pub(super) fn react_to_hit(&mut self, m: &Motion, rng: &mut Rng, damage: f32) {
        let Some(ab) = &m.abilities else { return };
        let clock = self.ab.clock;
        for (a, slot) in ab.list.iter().zip(&mut self.ab.slots) {
            let Some(r) = &a.reaction else { continue };
            if !slot.react.hit(r, damage, clock) || rng.next_f32() < r.ignore_chance {
                continue;
            }
            slot.react.due = Some(clock + between(rng, r.delay_s));
        }
    }

    /// Handles the waiting reactions that are due (see [`Until`]), in the order of their times and, for the
    /// same time, of the abilities' list. Each is tried at its own time, not the frame's: charges, the
    /// delay after a use and the end of the use all count from it. One that can't go is dropped.
    pub(super) fn fire_reactions(&mut self, m: &Motion, view: &View, pos: Vec3, seen: bool, until: Until) {
        loop {
            let next = (self.ab.slots.iter().enumerate())
                .filter_map(|(i, s)| s.react.due.filter(|d| until.has(*d)).map(|d| (i, d)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((i, due)) = next else { return };
            self.ab.slots[i].react.due = None;
            self.react_at(m, i, view, pos, seen, due);
        }
    }

    /// The reaction of the ability in slot `i`, due at `due`: used if it can be, else dropped.
    fn react_at(&mut self, m: &Motion, i: usize, view: &View, pos: Vec3, seen: bool, due: f32) {
        let Some(ab) = &m.abilities else { return };
        if due + TIE_S >= ab.start_block_s && self.can_start(m, i, view, pos, seen, due) {
            self.start_ability(m, ab, i, view, pos, due);
        }
    }
}
