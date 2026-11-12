//! Movement abilities for imported bots: dashes, blinks and ability jumps. An
//! ability has charges that refill, rules for when a bot may use it, and a burst of
//! velocity (one impulse, or a speed held for a duration). When a bot tries to use
//! one was measured in KovaaK's, see [`AbilityState::decide`]. Everything is in
//! meters and seconds.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::{BotState, Motion, between, is_zero};
use crate::rng::Rng;

/// A bot's movement abilities, and the settings for using them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Abilities {
    /// Seconds between presses of the bot's ability key (`AbilityUseTimer`).
    pub press_every_s: f32,
    /// The chance that a window is one in which the bot presses its key
    /// (`UseAbilityFrequency`). Every window the bot rolls it again. In a window it's
    /// willing in, it presses on every press; in any other it doesn't press at all.
    pub chance: f32,
    /// A window lasts a random time between these two (`UseAbilityFreqMinTime` and
    /// `UseAbilityFreqMaxTime`). With a chance of 1 they make no difference.
    pub window_s: [f32; 2],
    /// No ability for this long after the bot appears (`BlockAbilityOnStartDuration`).
    /// Not measured in KovaaK's.
    pub start_block_s: f32,
    /// A pause after any ability ends, shared by all of them (`AbilityGlobalCooldown`).
    /// Not measured in KovaaK's.
    pub global_cooldown_s: f32,
    /// Counts as in combat even with no line of sight (`SeeThroughWalls`).
    pub sees_through_walls: bool,
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
    /// (`ChargeTimer`). A charge doesn't refill while the ability is going on, so it
    /// starts refilling when the ability ends.
    pub charge_s: f32,
    /// After a use ends it can't be used again for the larger of these two
    /// (`DelayAfterUse`, not measured, and `AIReuseTimer`, measured).
    pub delay_s: f32,
    pub reuse_s: f32,
    /// 0 for a single impulse.
    pub duration_s: f32,
    /// m/s; negative goes backwards.
    pub speed: f32,
    pub up_speed: f32,
    /// The velocity is multiplied by this when the ability ends.
    pub end_factor: f32,
    /// Goes the way the bot is moving (`MainVelocitySetToMovementKeys`); else, or
    /// when it isn't moving, the way it faces.
    pub along_input: bool,
    /// The direction may point up or down (`MainVelocityCanGoVertical`).
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
        for a in &self.list {
            check(a.max_charges <= 1000 && a.spawn_charges <= 1000, "a number of charges");
            check(timers(&[a.charge_s, a.delay_s, a.reuse_s]), "a time");
            check(fine(a.duration_s, 0.0, 60.0), "a duration");
            check(fine(a.speed, -500.0, 500.0) && fine(a.up_speed, -500.0, 500.0), "a speed");
            check(fine(a.end_factor, 0.0, 10.0), "an end factor");
            check(fine(a.min_distance, 0.0, 10_000.0) && fine(a.max_distance, 0.0, 10_000.0), "a distance");
            check(fine(a.max_angle_deg, 0.0, 360.0), "a view angle");
            check(fine(a.health_cost, 0.0, 1.0e6), "a health cost");
        }
        p
    }
}

/// One ability's state for one bot.
#[derive(Debug, Clone, PartialEq)]
struct Slot {
    have: u32,
    /// Seconds toward the next charge.
    regen: f32,
    /// The bot's clock when its delay is over.
    ready_at: f32,
    /// The bot's clock when its last charge came back.
    back_at: f32,
}

impl Slot {
    fn new(a: &Ability) -> Slot {
        Slot {
            have: a.spawn_charges.min(a.max_charges),
            regen: 0.0,
            ready_at: f32::NEG_INFINITY,
            back_at: f32::NEG_INFINITY,
        }
    }

    /// `now` is the bot's clock at the end of the `dt` that is added.
    fn recharge(&mut self, a: &Ability, dt: f32, now: f32) {
        if self.have >= a.max_charges {
            self.regen = 0.0;
            return;
        }
        self.regen += dt;
        while self.have < a.max_charges && self.regen >= a.charge_s {
            self.regen -= a.charge_s;
            self.have += 1;
            self.back_at = now;
        }
        if self.have >= a.max_charges {
            self.regen = 0.0;
        }
    }
}

/// How far the bot's clock may be short of a press time and still press, so that
/// rounding in the clock doesn't make a press a frame late.
const TIE_S: f32 = 1e-3;

/// The ability in use.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Active {
    idx: usize,
    left: f32,
    dir: Vec3,
}

/// All of a bot's ability state. Its clock starts when the bot appears.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AbilityState {
    clock: f32,
    slots: Vec<Slot>,
    active: Option<Active>,
    /// The ability that ended this frame. Its charge starts to refill next frame.
    ended: Option<usize>,
    last_end: Option<f32>,
    /// The bot's clock when its next press is due.
    press_at: f32,
    /// The bot's clock when the current window is over.
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
            slots: Vec::new(),
            active: None,
            ended: None,
            last_end: None,
            press_at: 0.0,
            window_end: 0.0,
            willing: false,
            uses: 0,
            owed: 0.0,
        }
    }
}

impl AbilityState {
    /// Draws random numbers only for a bot that has abilities. The first window is
    /// rolled now. The first press comes one press interval after the bot appears;
    /// KovaaK's was not measured for that.
    pub(super) fn new(ab: &Abilities, rng: &mut Rng) -> AbilityState {
        let mut state = AbilityState {
            slots: ab.list.iter().map(Slot::new).collect(),
            press_at: ab.press_every_s,
            ..AbilityState::default()
        };
        state.new_window(ab, rng);
        state
    }

    fn new_window(&mut self, ab: &Abilities, rng: &mut Rng) {
        self.window_end = self.clock + between(rng, ab.window_s).max(0.01);
        self.willing = rng.next_f32() < ab.chance;
    }

    /// Whether the bot presses its ability key this frame, as measured in KovaaK's:
    ///
    /// - `AbilityUseTimer` is the time between presses of the key.
    /// - Every window (a random time between `UseAbilityFreqMinTime` and
    ///   `UseAbilityFreqMaxTime`) the bot rolls `UseAbilityFrequency` as a chance. For
    ///   that whole window it either presses on every press, or doesn't press at all.
    ///   The window timer and the press timer are separate. With a frequency of 1 the
    ///   window times make no difference.
    /// - `FullyAuto` makes no difference to a bot: a press is one attempt to use an
    ///   ability, so the first ready ability in the list goes.
    /// - Nothing else starts an ability. A bot counts as in combat with a line of
    ///   sight to the player, whether or not it shoots.
    ///
    /// Not measured, and guessed: the pause after the bot appears
    /// (`BlockAbilityOnStartDuration`), the shared cooldown (`AbilityGlobalCooldown`),
    /// the first press (one interval after the bot appears) and `DelayAfterUse`.
    /// KovaaK's also shows a dash now and then coming later than these rules say
    /// (about one in four, 5.7 s once instead of 4.2 s). The trainer doesn't copy that.
    fn decide(&mut self, ab: &Abilities, rng: &mut Rng) -> bool {
        if self.clock >= self.window_end {
            self.new_window(ab, rng);
        }
        if self.clock + TIE_S < self.press_at {
            return false;
        }
        // At most one press a frame; a press interval shorter than a frame means a
        // press every frame.
        self.press_at = (self.press_at + ab.press_every_s).max(self.clock);
        self.willing
    }

    /// Time moves on by `dt`: charges refill, except one whose own ability is going on
    /// or ended in this frame (measured: a charge starts to refill when its ability
    /// has ended).
    pub(super) fn advance(&mut self, ab: &Abilities, dt: f32) {
        self.clock += dt;
        let busy = [self.active.map(|a| a.idx), self.ended.take()];
        for (i, (s, a)) in self.slots.iter_mut().zip(&ab.list).enumerate() {
            if !busy.contains(&Some(i)) {
                s.recharge(a, dt, self.clock);
            }
        }
    }
}

/// What a bot's abilities need to know about its movement this frame.
pub(super) struct View {
    /// Its movement input as a direction, or zero.
    pub wish: Vec3,
    /// A flier's up (+1) or down (-1) intent.
    pub up: f32,
    pub spawn_forward: Vec3,
}

/// The way an ability sends the bot from `pos`.
fn direction(m: &Motion, a: &Ability, v: &View, pos: Vec3) -> Vec3 {
    let to_eye = -pos;
    let flat = Vec3::new(to_eye.x, 0.0, to_eye.z).normalize_or(v.spawn_forward);
    let facing = if m.faces_player { flat } else { v.spawn_forward };
    let climb = if a.vertical { v.up } else { 0.0 };
    let mut d = if a.along_input && (v.wish != Vec3::ZERO || climb != 0.0) {
        v.wish + Vec3::Y * climb
    } else if a.vertical && m.faces_player && to_eye.length_squared() > 1e-6 {
        to_eye
    } else {
        facing
    };
    if !a.vertical {
        d.y = 0.0;
    }
    d.normalize_or(facing)
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

    pub(super) fn ability_view(&self, m: &Motion, wish: Vec3, spawn_forward: Vec3) -> View {
        View { wish, up: if m.flight.is_some() { self.up } else { 0.0 }, spawn_forward }
    }

    /// Whether an ability that blocks the bot's own movement is going on.
    pub(super) fn ability_blocks_input(&self, m: &Motion) -> bool {
        let list = m.abilities.as_ref().map(|ab| &ab.list);
        self.ab.active.is_some_and(|act| list.and_then(|l| l.get(act.idx)).is_some_and(|a| a.blocks_movement))
    }

    /// Once a frame: when the bot presses its key, it starts the first ability that is
    /// ready. `seen` is whether the bot has a line of sight to the player.
    pub(super) fn use_abilities(&mut self, m: &Motion, rng: &mut Rng, view: &View, pos: Vec3, seen: bool) {
        let Some(ab) = &m.abilities else { return };
        if !self.ab.decide(ab, rng) || self.ab.active.is_some() {
            return;
        }
        for i in 0..ab.list.len() {
            if self.ready(m, ab, i, view, pos, seen) {
                self.start_ability(m, ab, i, view, pos);
                return;
            }
        }
    }

    fn ready(&self, m: &Motion, ab: &Abilities, i: usize, view: &View, pos: Vec3, seen: bool) -> bool {
        let (Some(a), Some(s)) = (ab.list.get(i), self.ab.slots.get(i)) else { return false };
        let clock = self.ab.clock;
        // Measured: a charge that comes back at the very moment of a press isn't there
        // for that press. A 4 s charge after a 0.1 s dash, pressed every 0.1 s, went
        // every 4.2 s and never every 4.1 s.
        let just_back = u32::from(clock <= s.back_at);
        self.ab.active.is_none()
            && s.have > just_back
            && clock >= s.ready_at
            && clock >= ab.start_block_s
            && self.ab.last_end.is_none_or(|end| clock - end >= ab.global_cooldown_s)
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

    /// Spends a charge and begins the ability: an impulse ends at once, a longer one
    /// is held by [`BotState::hold_ability`].
    fn start_ability(&mut self, m: &Motion, ab: &Abilities, i: usize, view: &View, pos: Vec3) {
        let a = &ab.list[i];
        self.ab.slots[i].have -= 1;
        self.ab.uses += 1;
        self.ab.owed += a.health_cost;
        let dir = direction(m, a, view, pos);
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
            self.finish_ability(ab, i, self.ab.clock);
        }
        if self.vel.y > 0.0 {
            self.grounded = false;
        }
    }

    /// Ends an ability at `at`: the velocity is scaled and the delays start.
    fn finish_ability(&mut self, ab: &Abilities, i: usize, at: f32) {
        let a = &ab.list[i];
        self.vel *= a.end_factor;
        self.ab.slots[i].ready_at = at + a.delay_s.max(a.reuse_s).max(0.0);
        self.ab.last_end = Some(at);
        self.ab.active = None;
        self.ab.ended = Some(i);
    }

    /// One physics substep of `h` seconds, ending at the bot's time `at`: an ability
    /// that lasts holds its speed.
    pub(super) fn hold_ability(&mut self, m: &Motion, view: &View, pos: Vec3, h: f32, at: f32) {
        let (Some(ab), Some(act)) = (&m.abilities, self.ab.active) else { return };
        let Some(a) = ab.list.get(act.idx) else { return };
        let dir = if a.lock_direction { act.dir } else { direction(m, a, view, pos) };
        if a.speed != 0.0 {
            self.vel.x = dir.x * a.speed;
            self.vel.z = dir.z * a.speed;
        }
        if a.no_gravity {
            self.vel.y = a.up_speed + dir.y * a.speed;
        }
        if act.left - h <= 0.0 {
            self.finish_ability(ab, act.idx, at);
        } else if let Some(now) = &mut self.ab.active {
            now.left = act.left - h;
        }
    }

    /// The bot's ability clock: its time since it appeared, at the start of the frame.
    pub(super) fn ability_clock(&self) -> f32 {
        self.ab.clock
    }

    /// A frame is over: charges refill and the clock moves on.
    pub(super) fn ability_tick(&mut self, m: &Motion, dt: f32) {
        if let Some(ab) = &m.abilities {
            self.ab.advance(ab, dt);
        }
    }
}
