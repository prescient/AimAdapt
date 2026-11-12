//! The jump key. A KovaaK's dodge profile decides, when the bot appears and then every
//! jump window, whether the key is held until the next decision. What the key does depends
//! on the bot: a jumper jumps when it is pressed, a jetpack thrusts while it is held and
//! has fuel, and a flier flown by keys goes up while it is down. Two decisions in a row to
//! hold are one long hold.

use super::{BotState, Dodge, Flight, Motion, TERMINAL, between};
use crate::rng::Rng;

/// How long a jetpack can't start again after its tank runs dry, whatever the key does.
/// KovaaK's measured about 0.26 s with the key held throughout.
const JETPACK_WAIT_S: f32 = 0.25;

/// The state of a bot's keys: the jump key, the jetpack's tank and, for a flier flown by
/// keys, the crouch key.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Keys {
    /// The jump key is down.
    pub(super) held: bool,
    /// It went down when the bot appeared, before anything is known about where it
    /// stands: the first step acts on it.
    pending: bool,
    started: bool,
    /// The bot hasn't landed since it appeared.
    pub(super) fresh: bool,
    /// A press in the air with no air jump left, remembered for the landing.
    pub(super) buffered: bool,
    /// Seconds before a jetpack pressed on the ground starts to thrust.
    pub(super) delay_left: f32,
    /// Seconds of fuel in the tank.
    pub(super) tank: f32,
    /// Seconds before a jetpack that ran dry can start again.
    pub(super) wait: f32,
    /// The crouch key is down (a flier flown by keys only).
    pub(super) crouch: bool,
    next_crouch: f32,
    /// Which way the flier was going last substep, to start from rest at a new input.
    last_fly: f32,
}

impl Keys {
    pub(super) fn new(m: &Motion) -> Keys {
        Keys { tank: m.jetpack.as_ref().map_or(0.0, |j| j.fuel_s.max(0.0)), ..Keys::default() }
    }
}

/// The fastest a bot can fall.
fn terminal(m: &Motion) -> f32 {
    if m.terminal_speed > 0.0 { m.terminal_speed } else { TERMINAL }
}

impl BotState {
    /// The first decisions, when the bot appears. Draws nothing for a dodge with no jump
    /// chance (and none for a flier that alternates), so such bots keep their layouts.
    pub(super) fn decide_at_birth(&mut self, m: &Motion, d: &Dodge, rng: &mut Rng) {
        if m.flight.is_some() {
            if d.fly_by_keys {
                if d.jump_chance > 0.0 {
                    self.keys.held = rng.next_f32() < d.jump_chance;
                }
                if d.crouch_chance > 0.0 {
                    self.keys.crouch = rng.next_f32() < d.crouch_chance;
                }
                self.up = self.key_flight(m);
            }
        } else if d.jump_chance > 0.0 && rng.next_f32() < d.jump_chance {
            self.keys.held = true;
            self.keys.pending = true;
        }
    }

    /// A new dodge profile of a flier flown by keys starts its crouch timer.
    pub(super) fn begin_crouch(&mut self, m: &Motion, d: &Dodge, rng: &mut Rng) {
        if m.flight.is_some() && d.fly_by_keys && d.crouch_chance > 0.0 {
            self.keys.next_crouch = between(rng, d.crouch_s).max(0.01);
        }
    }

    /// One frame of the keys: the timers that decide, then what the keys make the bot do.
    pub(super) fn drive(&mut self, m: &Motion, d: &Dodge, rng: &mut Rng, dt: f32) {
        match &m.flight {
            Some(_) if d.fly_by_keys => self.fly_by_keys(m, d, rng, dt),
            Some(_) => self.fly_alternating(d, rng, dt),
            None => self.jump_keys(m, d, rng, dt),
        }
    }

    /// A walker's jump key: a decision every window.
    fn jump_keys(&mut self, m: &Motion, d: &Dodge, rng: &mut Rng, dt: f32) {
        if !self.keys.started {
            self.keys.started = true;
            self.keys.fresh = !self.grounded;
            if self.keys.pending {
                self.keys.pending = false;
                self.press(m, rng, true);
            }
        }
        if d.jump_chance <= 0.0 {
            self.let_go();
            return;
        }
        self.next_jump -= dt;
        if self.next_jump <= 0.0 {
            self.next_jump = between(rng, d.jump_s);
            let hold = rng.next_f32() < d.jump_chance;
            self.set_jump_key(m, rng, hold);
        }
    }

    /// The jump key is decided to be down or up. A key already down stays down, with no
    /// new press.
    pub(super) fn set_jump_key(&mut self, m: &Motion, rng: &mut Rng, down: bool) {
        if !down {
            self.let_go();
        } else if !self.keys.held {
            self.keys.held = true;
            self.press(m, rng, false);
        }
    }

    /// The key comes up, which also forgets a remembered press.
    fn let_go(&mut self) {
        self.keys.held = false;
        self.keys.buffered = false;
        self.keys.delay_left = 0.0;
    }

    /// The jump key went down (`birth`: it was down when the bot appeared). A press on
    /// the ground jumps. A press in the air makes an air jump if one is left, and
    /// otherwise is remembered for the landing when the character buffers jumps. The
    /// press at birth isn't one in the air: a bot that comes down with the key held
    /// jumps when it first lands.
    fn press(&mut self, m: &Motion, rng: &mut Rng, birth: bool) {
        if m.flight.is_some() {
            return;
        }
        if let Some(j) = &m.jetpack {
            // A press in the air thrusts at once; one on the ground waits.
            self.keys.delay_left = if self.grounded { j.delay_s.max(0.0) } else { 0.0 };
        }
        if self.grounded {
            self.ground_jump(m, rng);
        } else if !birth && !self.air_jump(m) && m.buffered_jumps && m.gravity > 0.0 {
            self.keys.buffered = true;
        }
    }

    fn ground_jump(&mut self, m: &Motion, rng: &mut Rng) {
        // With nothing to bring it down a jump would never end.
        if m.gravity <= 0.0 || m.jump_speed[0].max(m.jump_speed[1]) <= 0.0 {
            return;
        }
        self.vel.y = between(rng, m.jump_speed);
        self.grounded = false;
        self.air_jumps_left = m.air_jumps;
    }

    /// Whether an air jump was made.
    fn air_jump(&mut self, m: &Motion) -> bool {
        if m.gravity <= 0.0 || self.air_jumps_left == 0 || m.air_jump_speed <= 0.0 {
            return false;
        }
        self.vel.y = m.air_jump_speed;
        self.air_jumps_left -= 1;
        true
    }

    /// The bot touched down. A landing refills the air jumps; with the key held it jumps
    /// again for a pogo jumper, for a remembered press and for the first landing since it
    /// appeared.
    pub(super) fn land(&mut self, m: &Motion, rng: &mut Rng) {
        self.air_jumps_left = m.air_jumps;
        if self.vel.y > 0.01 || m.flight.is_some() {
            return; // still rising: grazing the floor on the way up
        }
        let first = std::mem::take(&mut self.keys.fresh);
        let again = self.keys.held && (m.pogo || self.keys.buffered || first);
        self.keys.buffered = false;
        if again {
            self.ground_jump(m, rng);
        }
    }

    /// One substep of the jetpack: whether it thrusts. It thrusts while the key is held,
    /// the delay has passed, the tank isn't empty and it isn't waiting after running dry.
    /// The tank drains while the key is held (the delay included) and refills whenever the
    /// jetpack isn't thrusting. A tank of 0 or less is endless.
    pub(super) fn jetpack_thrusts(&mut self, m: &Motion, h: f32) -> bool {
        let Some(j) = m.jetpack.as_ref().filter(|_| m.flight.is_none()) else { return false };
        let on_ground = self.grounded;
        let k = &mut self.keys;
        k.wait = (k.wait - h).max(0.0);
        if k.held {
            k.delay_left = (k.delay_left - h).max(0.0);
        }
        let endless = j.fuel_s <= 0.0;
        let thrusting = k.held && k.delay_left <= 0.0 && k.wait <= 0.0 && (endless || k.tank > 0.0);
        if !endless {
            if k.held {
                k.tank -= h;
            }
            if !thrusting && (j.refill_in_air || (on_ground && !k.held)) {
                k.tank += j.refill_per_s.max(0.0) * h;
            }
            k.tank = k.tank.clamp(0.0, j.fuel_s);
            if thrusting && k.tank <= 0.0 {
                k.wait = JETPACK_WAIT_S;
            }
        }
        thrusting
    }

    /// A flier that alternates: up for one toggle time, down for the next, still for the
    /// pause between.
    fn fly_alternating(&mut self, d: &Dodge, rng: &mut Rng, dt: f32) {
        self.next_up -= dt;
        if self.next_up <= 0.0 {
            let pause = between(rng, d.up_down_pause_s);
            if self.up != 0.0 && pause > 0.0 {
                self.up_resume = -self.up;
                self.up = 0.0;
                self.next_up = pause;
            } else {
                self.up = if self.up == 0.0 { self.up_resume } else { -self.up };
                self.next_up = between(rng, d.up_down_s).max(0.01);
            }
        }
    }

    /// A flier flown by keys: the jump key and the crouch key each decide on their own
    /// windows.
    fn fly_by_keys(&mut self, m: &Motion, d: &Dodge, rng: &mut Rng, dt: f32) {
        if d.jump_chance > 0.0 {
            self.next_jump -= dt;
            if self.next_jump <= 0.0 {
                self.next_jump = between(rng, d.jump_s);
                self.keys.held = rng.next_f32() < d.jump_chance;
            }
        } else {
            self.keys.held = false;
        }
        if d.crouch_chance > 0.0 {
            self.keys.next_crouch -= dt;
            if self.keys.next_crouch <= 0.0 {
                self.keys.next_crouch = between(rng, d.crouch_s).max(0.01);
                self.keys.crouch = rng.next_f32() < d.crouch_chance;
            }
        } else {
            self.keys.crouch = false;
        }
        self.up = self.key_flight(m);
    }

    /// Which way the keys fly the bot: up, down or neither. Both keys cancel unless the
    /// character rises on both.
    fn key_flight(&self, m: &Motion) -> f32 {
        let up_on_both = m.flight.as_ref().is_some_and(|f| f.up_on_both);
        match (self.keys.held, self.keys.crouch) {
            (true, false) => 1.0,
            (false, true) => -1.0,
            (true, true) if up_on_both => 1.0,
            _ => 0.0,
        }
    }

    /// One substep of a flier's vertical speed. Going up or down it gains speed from rest
    /// at that direction's acceleration, up to its flight speed, and gravity doesn't act.
    /// With no input one without gravity stops at once, and one with gravity keeps its
    /// speed and falls.
    pub(super) fn fly_vertically(&mut self, m: &Motion, f: &Flight, h: f32) {
        let input = self.up;
        if input != self.keys.last_fly {
            if input != 0.0 {
                self.vel.y = 0.0;
            }
            self.keys.last_fly = input;
        }
        let (goal, accel) = if input > 0.0 {
            (f.up_speed, f.up_accel)
        } else if input < 0.0 {
            (-f.down_speed, f.down_accel)
        } else if m.gravity > 0.0 {
            self.vel.y = (self.vel.y - m.gravity * h).max(-terminal(m));
            return;
        } else {
            self.vel.y = 0.0;
            return;
        };
        let step = accel.max(0.0) * h;
        self.vel.y += (goal - self.vel.y).clamp(-step, step);
    }

    /// One substep of a walker's vertical speed: gravity, a jetpack's thrust and the
    /// fall limit.
    pub(super) fn fall_or_thrust(&mut self, m: &Motion, thrusting: bool, h: f32) {
        self.vel.y -= m.gravity * h;
        if let Some(j) = m.jetpack.as_ref().filter(|_| thrusting)
            && self.vel.y < j.max_up
        {
            self.vel.y = (self.vel.y + j.thrust * h).min(j.max_up.max(self.vel.y));
        }
        self.vel.y = self.vel.y.max(-terminal(m));
    }
}
