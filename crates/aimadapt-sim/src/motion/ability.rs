//! Movement abilities for imported bots: dashes, blinks and ability jumps. An
//! ability has charges that refill, rules for when a bot may use it, and a burst of
//! velocity (one impulse, or a speed held for a duration). When a bot tries to use
//! one was measured in KovaaK's, see [`AbilityState::decide`]. Everything is in
//! meters and seconds.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::reaction::{Pending, Reaction, Until};
use super::{BotState, Motion, between, is_zero};
use crate::rng::Rng;

mod brain;
pub use brain::Brain;

/// A bot's movement abilities, and the settings for using them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Abilities {
    /// Seconds between ticks of the bot's ability key (`AbilityUseTimer`). The key ticks
    /// at 1, 2, ... times this from the moment the bot appears. A tick is a try for an
    /// ability only when more than its `reuse_s` has passed since its last try.
    pub press_every_s: f32,
    /// The chance that a window is one in which the bot presses its key
    /// (`UseAbilityFrequency`). Every window the bot rolls it again. In a window it's
    /// willing in, it presses on every tick; in any other it doesn't press at all. The windows
    /// run on through a bot's death and its replacement's appearance, see [`Brain`].
    pub chance: f32,
    /// A window lasts a random time between these two (`UseAbilityFreqMinTime` and
    /// `UseAbilityFreqMaxTime`). With a chance of 1 they make no difference.
    pub window_s: [f32; 2],
    /// No ability until this long after the first bot appears (`BlockAbilityOnStartDuration`).
    /// It counts once: a bot that comes back after a death doesn't wait it out again (measured,
    /// see [`Brain`]). A tick exactly at its end goes through; earlier ticks aren't tries.
    /// KovaaK's waits 1 s for a character that has no such key (the importer writes that in).
    pub start_block_s: f32,
    /// A pause after any ability ends, shared by all of them (`AbilityGlobalCooldown`),
    /// counted from the end the use was scheduled for (its tick plus its duration); a
    /// try exactly at its end goes through. Not measured in KovaaK's.
    pub global_cooldown_s: f32,
    /// Counts as in combat even with no line of sight (`SeeThroughWalls`).
    pub sees_through_walls: bool,
    /// Meters to add to the height of the line from the bot's middle to the player's eye to get the
    /// bot's aim line, which a dash that can go up or down follows. KovaaK's runs that line from one
    /// body height above the bot's middle to one body height above the middle of the player's
    /// character (measured), so this is the player's body height, less the bot's, less how far the
    /// eye is above the player's middle. The importer sets it for a bot with such a dash and leaves
    /// it 0 for the others.
    #[serde(skip_serializing_if = "is_zero")]
    pub aim_lift: f32,
    /// In the order the bot tries them.
    pub list: Vec<Ability>,
}

/// One movement ability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ability {
    pub max_charges: u32,
    /// Charges it has when the bot appears.
    pub spawn_charges: u32,
    /// Seconds to regain one charge while below the maximum; 0 regains at once
    /// (`ChargeTimer`). A missing charge starts to refill at the tick of the press that
    /// spent it, whatever the ability's length, and several missing ones come back one
    /// after another, each this long after the one before. One that comes back exactly at
    /// a try isn't there for that try, except in a chain that began when the bot
    /// appeared (it spawned with fewer charges than its maximum).
    pub charge_s: f32,
    /// A use can't be followed by another use of this ability until this long after its
    /// press, and a try exactly then isn't used (`DelayAfterUse`).
    pub delay_s: f32,
    /// A tick is a try for this ability only when more than this has passed since its
    /// last try, used or not (`AIReuseTimer`, measured). It spaces the tries; it doesn't
    /// hold a use back by itself.
    pub reuse_s: f32,
    /// 0 for a single impulse. No ability starts while one is going on, and a try exactly
    /// when one ends (its tick plus this) isn't used.
    pub duration_s: f32,
    /// m/s; negative goes backwards.
    pub speed: f32,
    pub up_speed: f32,
    /// The velocity is multiplied by this when the ability ends.
    pub end_factor: f32,
    /// Goes the way the bot is moving (`MainVelocitySetToMovementKeys`); else, or
    /// when it isn't moving, the way it faces. With `vertical` it is the way the keys point
    /// over the ground (measured), pitched like the aim line.
    pub along_input: bool,
    /// The direction may point up or down (`MainVelocityCanGoVertical`). Measured: it follows
    /// the bot's aim line, see [`direction`]. Along the keys, with a flier's up and down keys
    /// playing no part; not along them, toward the player (away for a negative speed). A bot
    /// that doesn't aim goes level along the keys.
    pub vertical: bool,
    /// The direction is fixed when the ability starts.
    pub lock_direction: bool,
    /// No gravity while it lasts (`NegateGravityForDuration`).
    pub no_gravity: bool,
    /// The bot's own movement is off while it lasts (`AbilityBlocksMovement`).
    pub blocks_movement: bool,
    pub in_combat: bool,
    pub out_of_combat: bool,
    pub on_ground: bool,
    pub in_air: bool,
    /// Meters from the player; 0 for no limit on `max_distance`.
    pub min_distance: f32,
    pub max_distance: f32,
    /// The player must be within half of this of the way the bot faces
    /// (`AIMaxTargFOV`).
    pub max_angle_deg: f32,
    /// Health the bot loses each time it uses this ability (a negative `HealthRestore`
    /// in KovaaK's). A use that takes its last health makes it leave as a timeout.
    #[serde(skip_serializing_if = "is_zero")]
    pub health_cost: f32,
    /// Uses the ability when the bot is hurt (`AIDamageReaction`), see [`Reaction`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reaction: Option<Reaction>,
}

impl Default for Ability {
    fn default() -> Self {
        Ability {
            max_charges: 1,
            spawn_charges: 1,
            charge_s: 0.0,
            delay_s: 0.0,
            reuse_s: 0.0,
            duration_s: 0.0,
            speed: 0.0,
            up_speed: 0.0,
            end_factor: 1.0,
            along_input: false,
            vertical: false,
            lock_direction: true,
            no_gravity: false,
            blocks_movement: false,
            in_combat: true,
            out_of_combat: true,
            on_ground: true,
            in_air: true,
            min_distance: 0.0,
            max_distance: 0.0,
            max_angle_deg: 360.0,
            health_cost: 0.0,
            reaction: None,
        }
    }
}

impl Abilities {
    /// Problems with the values, in plain words.
    pub fn problems(&self) -> Vec<String> {
        let fine = |v: f32, lo: f32, hi: f32| v.is_finite() && (lo..=hi).contains(&v);
        let mut p = Vec::new();
        let mut check = |ok: bool, what: &str| {
            if !ok {
                let msg = format!("a bot's ability has {what} out of range");
                if !p.contains(&msg) {
                    p.push(msg);
                }
            }
        };
        let timers = |v: &[f32]| v.iter().all(|t| fine(*t, 0.0, 3600.0));
        check(fine(self.chance, 0.0, 1.0), "a use chance");
        let times =
            [self.press_every_s, self.window_s[0], self.window_s[1], self.start_block_s, self.global_cooldown_s];
        check(timers(&times), "a time");
        check(fine(self.aim_lift, -100.0, 100.0), "an aim height");
        for a in &self.list {
            check(a.max_charges <= 1000 && a.spawn_charges <= 1000, "a number of charges");
            check(timers(&[a.charge_s, a.delay_s, a.reuse_s]), "a time");
            check(fine(a.duration_s, 0.0, 60.0), "a duration");
            check(fine(a.speed, -500.0, 500.0) && fine(a.up_speed, -500.0, 500.0), "a speed");
            check(fine(a.end_factor, 0.0, 10.0), "an end factor");
            check(fine(a.min_distance, 0.0, 10_000.0) && fine(a.max_distance, 0.0, 10_000.0), "a distance");
            check(fine(a.max_angle_deg, 0.0, 360.0), "a view angle");
            check(fine(a.health_cost, 0.0, 1.0e6), "a health cost");
            if let Some(r) = &a.reaction {
                check(fine(r.threshold, 0.0, 1.0e6) && fine(r.ignore_chance, 0.0, 1.0), "a damage reaction");
                check(timers(&[r.reset_s, r.delay_s[0], r.delay_s[1]]), "a time");
            }
        }
        p
    }
}

/// How far a time may be from a scheduled one and still be "exactly at" it, so that
/// rounding in the clock or in a tick's time (a count times the interval) doesn't decide
/// a tie.
pub(super) const TIE_S: f32 = 1e-3;

/// The most ticks one frame tries; the rest are skipped (see [`AbilityState::skip_to`]).
const MAX_TICKS_A_FRAME: u32 = 64;

/// The most windows rolled at once to catch up with a tick.
const MAX_WINDOWS_AT_ONCE: u32 = 64;

/// One ability's state for one bot. Every time in it is on the bot's clock and comes from
/// the key's ticks and the schedule of the uses, never from the lengths of frames.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Slot {
    /// Charges it has, as of the last time the refills were settled.
    have: u32,
    /// When the refill that is going on began: the next charge is back a charge time
    /// after it, and each one after that a charge time after the one before. `None`
    /// when every charge is there.
    refill_from: Option<f32>,
    /// Whether that refill began when the bot appeared (it spawned with fewer charges
    /// than its maximum). Such a chain is used on a tie, until the slot is full again.
    from_spawn: bool,
    /// The tick at which the ability was last used.
    last_press: Option<f32>,
    /// The tick that was last a try for this ability, used or not.
    last_try: Option<f32>,
    /// What the bot's reaction to damage has counted and is waiting to do.
    pub(super) react: Pending,
}

impl Slot {
    fn new(a: &Ability) -> Slot {
        let have = a.spawn_charges.min(a.max_charges);
        let short = have < a.max_charges;
        Slot {
            have,
            refill_from: short.then_some(0.0),
            from_spawn: short,
            last_press: None,
            last_try: None,
            react: Pending::default(),
        }
    }

    /// Whether a tick at `at` is a try for this ability: strictly more than the reuse time
    /// after its last try.
    fn is_try(&self, a: &Ability, at: f32) -> bool {
        self.last_try.is_none_or(|last| at - last > a.reuse_s + TIE_S)
    }

    /// Brings the charges up to the try at `at`. A charge is there when it came back
    /// strictly before `at`; one back exactly at `at` isn't, unless the refill chain it
    /// belongs to began when the bot appeared. With a charge time of 0 a charge is back
    /// at the next try. Ends when the slot is full.
    fn settle(&mut self, a: &Ability, at: f32) {
        while self.have < a.max_charges {
            let Some(from) = self.refill_from else { break };
            let back = from + a.charge_s.max(0.0);
            let there = if a.charge_s <= 0.0 || self.from_spawn { back <= at + TIE_S } else { back + TIE_S < at };
            if !there {
                break;
            }
            self.have += 1;
            self.refill_from = Some(back);
        }
        if self.have >= a.max_charges {
            self.refill_from = None;
            self.from_spawn = false;
        }
    }

    /// A use at the tick `at`: a charge is spent, and a missing one starts to refill at
    /// once (or, if others were already coming back, queues behind them).
    fn spend(&mut self, at: f32) {
        self.have = self.have.saturating_sub(1);
        if self.refill_from.is_none() {
            self.refill_from = Some(at);
            self.from_spawn = false;
        }
        self.last_press = Some(at);
    }
}

/// The ability in use.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Active {
    idx: usize,
    left: f32,
    dir: Vec3,
}

/// All of a bot's ability state. Its clock starts when the bot appears. A bot that takes the place of
/// one that died carries part of the dead bot's state (see [`Brain`]), shifted onto its own clock: the
/// start block (`began`), the windows and each ability's last try. The rest starts again.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AbilityState {
    pub(super) clock: f32,
    /// When, on this bot's clock, the state it carries began: 0 for a bot that carries nothing, and
    /// negative for one that does (the start block counts from the first bot's appearance).
    began: f32,
    pub(super) slots: Vec<Slot>,
    active: Option<Active>,
    /// When the latest use is scheduled to end: its tick plus its duration.
    last_end: Option<f32>,
    /// The count of the next tick of the key to try: it ticks at that number times
    /// `press_every_s`, the first at 1.
    next_tick: u32,
    /// When the current window is over, on the bot's clock. Windows run back to back.
    window_end: f32,
    /// Whether the current window is one in which it presses.
    willing: bool,
    uses: u32,
    /// Health its uses have cost that the sim hasn't taken off yet.
    owed: f32,
}

impl Default for AbilityState {
    fn default() -> Self {
        AbilityState {
            clock: 0.0,
            began: 0.0,
            slots: Vec::new(),
            active: None,
            last_end: None,
            next_tick: 1,
            window_end: 0.0,
            willing: false,
            uses: 0,
            owed: 0.0,
        }
    }
}

impl AbilityState {
    /// Draws random numbers only for a bot that has abilities. The first window is
    /// rolled now. The first tick of the key comes one interval after the bot appears.
    pub(super) fn new(ab: &Abilities, rng: &mut Rng) -> AbilityState {
        let mut state = AbilityState { slots: ab.list.iter().map(Slot::new).collect(), ..AbilityState::default() };
        state.new_window(ab, rng);
        state
    }

    /// The next window starts where the last one ended.
    fn new_window(&mut self, ab: &Abilities, rng: &mut Rng) {
        self.window_end += between(rng, ab.window_s).max(0.01);
        self.willing = rng.next_f32() < ab.chance;
    }

    /// Rolls the windows up to the tick at `at`, so that the windows and the number of
    /// rolls follow the ticks and not the frames. With tiny windows and a long wait it
    /// stops after a few and starts a window at `at`.
    fn roll_windows(&mut self, ab: &Abilities, rng: &mut Rng, at: f32) {
        let mut rolled = 0;
        while at + TIE_S >= self.window_end {
            if rolled == MAX_WINDOWS_AT_ONCE {
                self.window_end = at;
            }
            self.new_window(ab, rng);
            rolled += 1;
        }
    }

    /// Whether the tick at `at` is one at which the bot tries, as measured in KovaaK's
    /// (all times are on the bot's own clock, which starts when it appears):
    ///
    /// - Nothing is used until `BlockAbilityOnStartDuration` has passed. A tick exactly at
    ///   the end of the block goes through; ticks before it aren't tries.
    /// - `AbilityUseTimer` is the time between ticks of the key: it ticks at 1, 2, ...
    ///   times that from the moment the bot appears, and the grid never shifts.
    /// - A tick is a try for an ability only when it comes strictly more than that
    ///   ability's `AIReuseTimer` after the ability's previous try (see [`Slot::is_try`]).
    ///   A try that fails, for lack of a charge, a delay that isn't over, an ability going
    ///   on or the ability's rules, still counts as a try and uses up the reuse time. So
    ///   with a tick of 0.1 s and a reuse time of 0.1 s the tries are 0.2 s apart, and
    ///   with a reuse time of 1.0 s they are 1.1 s apart.
    /// - Every window (a random time between `UseAbilityFreqMinTime` and
    ///   `UseAbilityFreqMaxTime`) the bot rolls `UseAbilityFrequency` as a chance. For
    ///   that whole window it either presses on every tick, or doesn't press at all. With
    ///   a frequency of 1 the window times make no difference.
    /// - `FullyAuto` makes no difference to a bot: a tick is one attempt to use an
    ///   ability.
    /// - Nothing else starts an ability. A bot counts as in combat with a line of sight
    ///   to the player, whether or not it shoots.
    /// - A charge starts to refill at the tick of the press that spent it, whatever the
    ///   ability's length. One that comes back exactly at a try isn't there for it, except
    ///   in a chain of refills that began when the bot appeared (see [`Slot::settle`]).
    /// - What a bot's abilities keep when it dies and another takes its place (measured in 3.9.13 with
    ///   still balls that come back where they were): the start block counts once, from the first
    ///   bot's appearance; each ability's reuse clock runs on through the death and the respawn; the
    ///   frequency windows run on, none starting at a respawn. The tick grid of the key and the
    ///   charges start again with each bot (see [`Brain`]). Only the first bot rolls a
    ///   window when it appears; a replacement draws no random number for its abilities.
    /// - `DelayAfterUse` counts from the press, and a try exactly when it is over isn't
    ///   used. No ability starts while one is going on, and a try exactly at the moment
    ///   one ends (its tick plus `AbilityDuration`) isn't used either.
    ///
    /// A bot's use of an ability because it was hurt (`AIDamageReaction`) isn't a tick of the key: its
    /// rules, measured and not, are written on [`BotState::react_to_hit`] and the ones this function
    /// states about the key (the window, the reuse time) play no part in it.
    ///
    /// Not measured, and played as follows: a tick in a window in which the bot isn't
    /// willing isn't a try. A tick is a try whether or not an ability's rules (combat,
    /// ground and air, distance, view angle) allow the use. At a tick the abilities are
    /// gone through in list order; each one for which the tick is a try has its last try
    /// set, and the first of those that is ready is used, so those after it aren't tried
    /// at that tick. The shared cooldown (`AbilityGlobalCooldown`) is a pause after the
    /// scheduled end of any ability (its tick plus its duration); a tie counts as over.
    /// Across a respawn, also not measured: `DelayAfterUse` and the shared cooldown start again with
    /// each bot, an ability going on when its bot dies ends with it, a reaction to damage that is
    /// waiting and the damage counted toward one are dropped, and a bot that first appears later in a run
    /// counts its block from when it first appears. KovaaK's also shows
    /// a dash now and then coming later than these rules say (about one in four, 5.7 s once
    /// instead of 4.2 s). The trainer doesn't copy that.
    ///
    /// Frame lengths play no part. Only scheduled times are compared; a frame just decides
    /// in which frame a tick that has come due is handled. This function answers the two
    /// questions that belong to the bot's key as a whole (the block and the window); the
    /// reuse time is [`Slot::is_try`] and whether an ability can go is
    /// [`AbilityState::ready_at`].
    fn decide(&mut self, ab: &Abilities, rng: &mut Rng, at: f32) -> bool {
        if !self.past_block(ab, at) {
            return false;
        }
        self.roll_windows(ab, rng, at);
        self.willing
    }

    /// Whether the start block is over at `at`: it counts from `began`, a tie being over.
    pub(super) fn past_block(&self, ab: &Abilities, at: f32) -> bool {
        at - self.began + TIE_S >= ab.start_block_s
    }

    /// Whether the ability in slot `i` is ready at the try at `at`: it has a charge, its
    /// delay after the press that last used it is over, and nothing is going on. The
    /// tick, the reuse time, the shared cooldown and the ability's rules aren't part of
    /// it, so that a use that isn't the bot's key (a reaction to damage) can ask the same
    /// question.
    pub(super) fn ready_at(&mut self, a: &Ability, i: usize, at: f32) -> bool {
        let busy = self.active.is_some() || self.last_end.is_some_and(|end| at <= end + TIE_S);
        let Some(s) = self.slots.get_mut(i) else { return false };
        if busy {
            return false;
        }
        s.settle(a, at);
        s.have > 0 && s.last_press.is_none_or(|press| at > press + a.delay_s + TIE_S)
    }

    /// Moves `next_tick` on to the first tick of the grid that is after `limit`, so the
    /// ticks in between aren't tried.
    fn skip_to(&mut self, press_every_s: f32, limit: f32) {
        if press_every_s.is_nan() || press_every_s <= 0.0 {
            return;
        }
        let at = |k: u32| k as f32 * press_every_s;
        let mut k = ((limit / press_every_s).floor().max(0.0) as u32).saturating_add(1);
        while k > self.next_tick && at(k - 1) > limit {
            k -= 1;
        }
        while k < u32::MAX && at(k) <= limit {
            k += 1;
        }
        self.next_tick = self.next_tick.max(k);
    }
}

/// What a bot's abilities need to know about its movement this frame.
pub(super) struct View {
    /// Its movement input as a direction, or zero.
    pub wish: Vec3,
    pub spawn_forward: Vec3,
}

/// The way an ability sends the bot from `pos`.
///
/// A level ability goes along the bot's movement input (when it is set to the keys and a key is
/// down) or the way it faces. One that can go up or down follows the bot's aim line, which
/// KovaaK's runs from one body height above the bot's middle to one body height above the middle
/// of the player's character: the line from the middle to the eye, `aim_lift` higher (see
/// [`Abilities::aim_lift`]). Measured: set to the keys, it goes where the keys point over the
/// ground and takes the pitch of the aim line whichever key is down, at the whole speed along
/// the slanted line (a flier's up and down keys play no part); a bot that doesn't aim goes level.
/// Not set to the keys, it goes along the aim line, toward the player for a positive speed.
pub(super) fn direction(m: &Motion, ab: &Abilities, a: &Ability, v: &View, pos: Vec3) -> Vec3 {
    let to_eye = -pos;
    let flat = Vec3::new(to_eye.x, 0.0, to_eye.z).normalize_or(v.spawn_forward);
    let facing = if m.faces_player { flat } else { v.spawn_forward };
    let level = |d: Vec3| Vec3::new(d.x, 0.0, d.z).normalize_or(facing);
    let heading = if a.along_input && v.wish != Vec3::ZERO { level(v.wish) } else { facing };
    if !a.vertical {
        return heading;
    }
    let aim = to_eye + Vec3::Y * ab.aim_lift;
    let len = aim.length();
    if !m.faces_player || len * len <= 1e-6 {
        return heading;
    }
    // The heading, pitched like the aim line. Not set to the keys the heading is the facing, which is
    // the way the aim line runs over the ground, so this is the aim line itself.
    let run = Vec3::new(aim.x, 0.0, aim.z).length();
    heading * (run / len) + Vec3::Y * (aim.y / len)
}

impl BotState {
    /// How many times it has used an ability.
    pub fn ability_uses(&self) -> u32 {
        self.ab.uses
    }

    /// The health its abilities have cost since this was last asked; asking sets it
    /// back to 0. The sim takes it off the bot's health (see `Ability::health_cost`).
    pub fn take_owed_health(&mut self) -> f32 {
        std::mem::take(&mut self.ab.owed)
    }

    /// Whether an ability is going on right now.
    pub fn using_ability(&self) -> bool {
        self.ab.active.is_some()
    }

    /// Whether an ability that blocks the bot's own movement is going on.
    pub(super) fn ability_blocks_input(&self, m: &Motion) -> bool {
        let list = m.abilities.as_ref().map(|ab| &ab.list);
        self.ab.active.is_some_and(|act| list.and_then(|l| l.get(act.idx)).is_some_and(|a| a.blocks_movement))
    }

    /// Once a frame: every tick of the bot's key that has come due (see
    /// [`AbilityState::decide`]) is tried in order, and one that finds an ability ready
    /// starts it. `seen` is whether the bot has a line of sight to the player. A key that
    /// ticks every 0 s ticks once a frame, at the frame's clock. At most 64 ticks are
    /// tried in a frame; if more came due, the rest are skipped, not tried. The bot's
    /// reactions to damage that have come due are handled here too, each in its place
    /// among the ticks by its time (see [`BotState::react_to_hit`]).
    pub(super) fn use_abilities(&mut self, m: &Motion, rng: &mut Rng, view: &View, pos: Vec3, seen: bool) {
        let Some(ab) = &m.abilities else { return };
        let clock = self.ab.clock;
        if ab.press_every_s <= 0.0 {
            self.fire_reactions(m, view, pos, seen, Until::Before(clock));
            self.try_at(m, rng, view, pos, seen, clock);
        } else {
            let mut tried = 0;
            loop {
                let at = self.ab.next_tick as f32 * ab.press_every_s;
                if at.is_nan() || at > clock + TIE_S {
                    break;
                }
                if tried == MAX_TICKS_A_FRAME {
                    self.ab.skip_to(ab.press_every_s, clock + TIE_S);
                    break;
                }
                self.ab.next_tick = self.ab.next_tick.saturating_add(1);
                tried += 1;
                // Reactions to damage that came due before this tick go first (see
                // `BotState::react_to_hit`); one due together with it comes after.
                self.fire_reactions(m, view, pos, seen, Until::Before(at));
                self.try_at(m, rng, view, pos, seen, at);
            }
        }
        self.fire_reactions(m, view, pos, seen, Until::By(clock));
    }

    /// The tick at `at`: for each ability it is a try for, in list order, the first that
    /// is ready is used and the rest aren't tried.
    fn try_at(&mut self, m: &Motion, rng: &mut Rng, view: &View, pos: Vec3, seen: bool, at: f32) {
        let Some(ab) = &m.abilities else { return };
        if !self.ab.decide(ab, rng, at) {
            return;
        }
        for (i, a) in ab.list.iter().enumerate() {
            let Some(slot) = self.ab.slots.get_mut(i) else { return };
            if !slot.is_try(a, at) {
                continue;
            }
            slot.last_try = Some(at);
            if self.can_start(m, i, view, pos, seen, at) {
                self.start_ability(m, ab, i, view, pos, at);
                return;
            }
        }
    }

    /// Whether the ability in slot `i` can start at `at`, a tick of the key or a reaction to damage:
    /// it is ready (see [`AbilityState::ready_at`]), the shared cooldown is over (a tie counts as over)
    /// and its own rules allow it.
    pub(super) fn can_start(&mut self, m: &Motion, i: usize, view: &View, pos: Vec3, seen: bool, at: f32) -> bool {
        let Some(ab) = &m.abilities else { return false };
        let Some(a) = ab.list.get(i) else { return false };
        self.ab.ready_at(a, i, at)
            && self.ab.last_end.is_none_or(|end| at - end + TIE_S >= ab.global_cooldown_s)
            && self.rules_allow(m, a, view, pos, seen || ab.sees_through_walls)
    }

    /// The ability's own rules: combat, ground or air, distance, view angle.
    fn rules_allow(&self, m: &Motion, a: &Ability, view: &View, pos: Vec3, in_combat: bool) -> bool {
        if !(if in_combat { a.in_combat } else { a.out_of_combat }) {
            return false;
        }
        if !(if self.grounded { a.on_ground } else { a.in_air }) {
            return false;
        }
        let dist = pos.length();
        if (a.min_distance > 0.0 && dist < a.min_distance) || (a.max_distance > 0.0 && dist > a.max_distance) {
            return false;
        }
        // A bot that faces the player always has them in view.
        if !m.faces_player && a.max_angle_deg < 360.0 && dist > 1e-3 {
            let angle = view.spawn_forward.angle_between(-pos).to_degrees();
            if angle > a.max_angle_deg / 2.0 {
                return false;
            }
        }
        true
    }

    /// Spends a charge and begins the ability at the tick `at`: an impulse ends at once, a
    /// longer one is held by [`BotState::hold_ability`]. It is scheduled to end at `at`
    /// plus its duration, whenever the frames end it.
    pub(super) fn start_ability(&mut self, m: &Motion, ab: &Abilities, i: usize, view: &View, pos: Vec3, at: f32) {
        let a = &ab.list[i];
        self.ab.slots[i].spend(at);
        self.ab.last_end = Some(at + a.duration_s.max(0.0));
        self.ab.uses += 1;
        self.ab.owed += a.health_cost;
        let dir = direction(m, ab, a, view, pos);
        if a.duration_s > 0.0 {
            // Held: the horizontal speed every substep. Without `no_gravity` the climb
            // happens once, here, and gravity acts as usual.
            self.ab.active = Some(Active { idx: i, left: a.duration_s, dir });
            let rise = a.up_speed + dir.y * a.speed;
            if !a.no_gravity && rise != 0.0 {
                self.vel.y = rise;
            }
        } else {
            if a.speed != 0.0 {
                self.vel.x = dir.x * a.speed;
                self.vel.z = dir.z * a.speed;
                if a.vertical {
                    self.vel.y = dir.y * a.speed;
                }
            }
            if a.up_speed != 0.0 {
                self.vel.y = a.up_speed;
            }
            self.finish_ability(ab, i);
        }
        if self.vel.y > 0.0 {
            self.grounded = false;
        }
    }

    /// Ends an ability: the velocity is scaled and nothing is going on.
    fn finish_ability(&mut self, ab: &Abilities, i: usize) {
        self.vel *= ab.list[i].end_factor;
        self.ab.active = None;
    }

    /// One physics substep of `h` seconds: an ability that lasts holds its speed.
    pub(super) fn hold_ability(&mut self, m: &Motion, view: &View, pos: Vec3, h: f32) {
        let (Some(ab), Some(act)) = (&m.abilities, self.ab.active) else { return };
        let Some(a) = ab.list.get(act.idx) else { return };
        let dir = if a.lock_direction { act.dir } else { direction(m, ab, a, view, pos) };
        if a.speed != 0.0 {
            self.vel.x = dir.x * a.speed;
            self.vel.z = dir.z * a.speed;
        }
        if a.no_gravity {
            self.vel.y = a.up_speed + dir.y * a.speed;
        }
        if act.left - h <= 0.0 {
            self.finish_ability(ab, act.idx);
        } else if let Some(now) = &mut self.ab.active {
            now.left = act.left - h;
        }
    }

    /// A frame is over: the bot's clock moves on.
    pub(super) fn ability_tick(&mut self, m: &Motion, dt: f32) {
        if m.abilities.is_some() {
            self.ab.clock += dt;
        }
    }

    /// A frame of a bot that waits to be hurt: its clock moves on, and the ticks that
    /// passed meanwhile are skipped, not tried, so it doesn't make up for them when it
    /// wakes. The grid stays where it is.
    pub(super) fn ability_wait(&mut self, m: &Motion, dt: f32) {
        let Some(ab) = &m.abilities else { return };
        self.ab.clock += dt;
        self.ab.skip_to(ab.press_every_s, self.ab.clock - TIE_S);
    }
}
