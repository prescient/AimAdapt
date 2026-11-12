//! What a bot's abilities keep when it dies and another bot takes its place.
//!
//! Measured in KovaaK's 3.9.13 on maps of still balls that come back where they were: a bot has something
//! like a brain that outlives it. From one life of a bot (its spawn to its death) to the next, three things
//! carry over: the start block (`BlockAbilityOnStartDuration`) counts once, from the first bot's appearance;
//! each ability's reuse clock (`AIReuseTimer`) runs on through the death and the respawn; and the frequency
//! windows run on (no window starts at a respawn). Two things start again: the tick grid of the key
//! (`AbilityUseTimer`, the first tick comes one interval after the bot appears) and the charges.
//!
//! Not measured, and played as follows: `DelayAfterUse` and the shared cooldown start again with each life,
//! an ability going on when its bot dies ends with it, a reaction to damage that is waiting and the damage
//! counted toward one are dropped, and a bot that first appears later in a run has its block counted from
//! when it first appears.
//!
//! Every time the brain carries is shifted onto the new bot's clock, which starts at 0 when it appears, by
//! `gone`: the old bot's clock when it left plus the time it was away.

use super::{Abilities, AbilityState, Slot};
use crate::motion::{BotState, Motion};

/// What a bot's abilities keep from one life to the next. The sim saves it when a bot leaves and hands it
/// to the bot that takes the place (see [`BotState::reborn`]). It is opaque; what it holds can grow (the
/// next package adds a weapon's cooldown).
#[derive(Debug, Clone, PartialEq)]
pub struct Brain {
    /// The leaving bot's state. [`AbilityState::reborn`] picks from it what carries over.
    ab: AbilityState,
}

impl Brain {
    /// Whether it belongs to a bot with these abilities: the same number of them.
    pub(in crate::motion) fn fits(&self, ab: &Abilities) -> bool {
        self.ab.slots.len() == ab.list.len()
    }

    /// The ability state of a bot that takes the place of the one this came from, `away_s` seconds after
    /// that one left. Draws no random numbers.
    pub(in crate::motion) fn reborn(&self, ab: &Abilities, away_s: f32) -> AbilityState {
        self.ab.reborn(ab, away_s)
    }
}

impl AbilityState {
    /// What carries over to a new bot (see the module's notes), on the new bot's clock; the rest is as
    /// [`Slot::new`] and [`AbilityState::default`] give it.
    fn reborn(&self, ab: &Abilities, away_s: f32) -> AbilityState {
        let gone = self.clock + away_s;
        let slots = self.slots.iter().zip(&ab.list);
        AbilityState {
            began: self.began - gone,
            slots: slots.map(|(old, a)| Slot { last_try: old.last_try.map(|t| t - gone), ..Slot::new(a) }).collect(),
            window_end: self.window_end - gone,
            willing: self.willing,
            ..AbilityState::default()
        }
    }
}

impl BotState {
    /// Takes what this bot's abilities keep, for the bot that replaces it (see [`Brain`]). `None` for a bot
    /// whose motion has no abilities.
    pub fn take_brain(&mut self, m: &Motion) -> Option<Brain> {
        m.abilities.as_ref()?;
        Some(Brain { ab: std::mem::take(&mut self.ab) })
    }
}
