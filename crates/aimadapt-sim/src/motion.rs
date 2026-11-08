//! How an imported scenario's bots move. A KovaaK's dodge profile decides what the
//! bot wants to do (strafe left or right, come closer or back off to keep its
//! distance, jump, fly up or down, turn when it's hit or blocked), and Unreal-style
//! character movement turns that into velocity: acceleration, ground friction,
//! braking, gravity and air control, against the map's walls.
//!
//! The rules come from the settings' names and Unreal's character movement, which
//! KovaaK's is built on. The fine detail is still to be checked against KovaaK's
//! (BACKLOG section 9, phase 4). Everything is in meters and seconds, in the sim's
//! frame: the player's eye at the origin, +Y up.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::collide::Collider;
use crate::sim::Rng;

/// Unreal's default terminal velocity, m/s.
const TERMINAL: f32 = 40.0;
/// Unreal's default walkable floor: up to about 45 degrees.
const FLOOR_NORMAL_Y: f32 = 0.7;
/// Physics substep, seconds.
const SUBSTEP: f32 = 1.0 / 240.0;
/// Below this speed, braking stops the bot (Unreal's `BRAKE_TO_STOP_VELOCITY`).
const STOP_SPEED: f32 = 0.1;
/// A grounded bot is pressed this far into the floor each substep, so it stays on it.
const STICK: f32 = 0.002;
/// How far below its feet a floor still counts as under a bot, meters: further than
/// that is a ledge it won't walk off.
const LEDGE_DROP: f32 = 0.5;
/// No floor within this far below is a drop out of the map, meters.
const VOID: f32 = 100.0;

/// A bot's movement: its character's physics and its dodge profiles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Motion {
    /// Top speed on the ground, m/s.
    pub max_speed: f32,
    /// m/s².
    pub accel: f32,
    /// Ground friction: how fast it turns and, times the braking factor, slows down.
    pub friction: f32,
    /// Friction while in the air.
    pub air_friction: f32,
    /// Slowing down with no input, m/s², on top of friction.
    pub braking: f32,
    pub braking_friction_factor: f32,
    /// m/s², downward. 0 for bots that float.
    pub gravity: f32,
    /// How much of its acceleration it has in the air, 0–1.
    pub air_control: f32,
    /// A jump's upward speed, m/s: picked between the two.
    pub jump_speed: [f32; 2],
    pub air_jumps: u32,
    pub air_jump_speed: f32,
    /// Fastest fall, m/s; 0 for Unreal's 40.
    pub terminal_speed: f32,
    /// Speed multipliers while strafing and while backing off.
    pub strafe_mult: f32,
    pub back_mult: f32,
    /// Flies instead of walking: no gravity, and up and down on its own timer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flight: Option<Flight>,
    /// Thrusts upward while it "jumps" (Floating Heads stay up this way).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jetpack: Option<Jetpack>,
    /// Bounces off walls instead of sliding along them.
    pub bounce_off_walls: bool,
    /// Bots bump into each other.
    pub bump_bots: bool,
    /// Doesn't move until it's first hit.
    pub still_until_hurt: bool,
    /// Strafes are relative to the player (the bot turns to face them); otherwise to
    /// the way its spawn point faces.
    pub faces_player: bool,
    pub dodges: Vec<Dodge>,
    /// Seconds between changes of dodge profile, picked between the two.
    pub change_dodge_s: [f32; 2],
    /// Pick the next dodge profile at random by weight, or else take them in order.
    pub random_dodges: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Motion {
            max_speed: 0.0,
            accel: 20.0,
            friction: 8.0,
            air_friction: 0.0,
            braking: 20.0,
            braking_friction_factor: 2.0,
            gravity: 9.8,
            air_control: 0.05,
            jump_speed: [4.2, 4.2],
            air_jumps: 0,
            air_jump_speed: 0.0,
            terminal_speed: 0.0,
            strafe_mult: 1.0,
            back_mult: 1.0,
            flight: None,
            jetpack: None,
            bounce_off_walls: false,
            bump_bots: false,
            still_until_hurt: false,
            faces_player: true,
            dodges: Vec::new(),
            change_dodge_s: [0.0, 0.0],
            random_dodges: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Flight {
    pub up_speed: f32,
    pub down_speed: f32,
    pub up_accel: f32,
    pub down_accel: f32,
}

/// A jetpack: upward acceleration (m/s²) while thrusting, up to a climbing speed
/// (m/s), with this much air control.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Jetpack {
    pub thrust: f32,
    pub max_up: f32,
    pub air_control: f32,
}

/// Which way a strafe or an advance starts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Start {
    /// Right, or forward (toward the player).
    #[default]
    Plus,
    /// Left, or back.
    Minus,
    Random,
}

/// One KovaaK's dodge profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dodge {
    pub weight: f32,
    /// Strafes left and right (`ToggleLeftRight`), switching every so often.
    pub strafe: bool,
    pub strafe_s: [f32; 2],
    /// Strafe times are multiplied by these going left and right.
    pub left_mult: f32,
    pub right_mult: f32,
    /// A stop between strafes, seconds.
    pub strafe_pause_s: [f32; 2],
    pub start_side: Start,
    /// Moves toward and away from the player (`ToggleForwardBack`).
    pub advance: bool,
    pub advance_s: [f32; 2],
    pub forward_mult: f32,
    pub back_mult: f32,
    pub start_forward: Start,
    /// Keeps between these distances from the player, meters; 0 for no limit.
    pub min_distance: f32,
    pub max_distance: f32,
    /// Every so often (`jump_s`) it jumps with this chance.
    pub jump_chance: f32,
    pub jump_s: [f32; 2],
    /// Fliers switch between up and down this often, with a pause between.
    pub up_down_s: [f32; 2],
    pub up_down_pause_s: [f32; 2],
    /// Changes strafe direction when hit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hit_turn: Option<HitTurn>,
    /// Turns around when a wall stops it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Blocked>,
}

impl Default for Dodge {
    fn default() -> Self {
        Dodge {
            weight: 1.0,
            strafe: true,
            strafe_s: [0.5, 1.0],
            left_mult: 1.0,
            right_mult: 1.0,
            strafe_pause_s: [0.0, 0.0],
            start_side: Start::Plus,
            advance: false,
            advance_s: [0.5, 1.0],
            forward_mult: 1.0,
            back_mult: 1.0,
            start_forward: Start::Plus,
            min_distance: 0.0,
            max_distance: 0.0,
            jump_chance: 0.0,
            jump_s: [0.5, 1.0],
            up_down_s: [0.5, 1.0],
            up_down_pause_s: [0.0, 0.0],
            hit_turn: None,
            blocked: None,
        }
    }
}

/// KovaaK's damage reaction: after taking `threshold` damage (counted since the
/// last pause of `reset_s` without hits), with this `chance`, it changes direction
/// after a delay, then not again for `cooldown_s`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HitTurn {
    pub chance: f32,
    pub delay_s: [f32; 2],
    pub cooldown_s: f32,
    pub threshold: f32,
    pub reset_s: f32,
}

/// Moving slower than `fraction` of its speed for the reaction time turns it around.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Blocked {
    pub fraction: f32,
    pub reaction_s: [f32; 2],
    /// Turns around straight away on touching a wall (`CounterStrafeOnCollision`).
    pub on_touch: bool,
}

/// One bot's movement as it plays.
#[derive(Debug, Clone, PartialEq)]
pub struct BotState {
    pub vel: Vec3,
    pub grounded: bool,
    dodge: usize,
    next_dodge: f32,
    /// +1 right, -1 left, 0 pausing (then `resume` is the side it'll go next).
    side: f32,
    resume: f32,
    next_side: f32,
    fore: f32,
    next_fore: f32,
    next_jump: f32,
    air_jumps_left: u32,
    up: f32,
    up_resume: f32,
    next_up: f32,
    hit_damage: f32,
    last_hit: f32,
    turn_at: Option<f32>,
    turn_again_at: f32,
    blocked_for: f32,
    blocked_limit: f32,
    hurt: bool,
    /// Seconds of jetpack thrust left.
    thrust_left: f32,
    /// The spawn point's right, for bots that don't face the player.
    spawn_right: Vec3,
    /// Where it appeared, to go back to if it falls out of the map.
    home: Vec3,
}

fn between(rng: &mut Rng, r: [f32; 2]) -> f32 {
    let (lo, hi) = (r[0].min(r[1]), r[0].max(r[1]));
    lo + (hi - lo) * rng.next_f32()
}

fn start(rng: &mut Rng, s: Start) -> f32 {
    match s {
        Start::Plus => 1.0,
        Start::Minus => -1.0,
        Start::Random => {
            if rng.next_f32() < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

impl BotState {
    /// A bot that just appeared at `home`; `spawn_right` is its spawn point's right.
    pub fn new(m: &Motion, rng: &mut Rng, home: Vec3, spawn_right: Vec3) -> BotState {
        let mut s = BotState {
            vel: Vec3::ZERO,
            grounded: false,
            dodge: 0,
            next_dodge: 0.0,
            side: 0.0,
            resume: 1.0,
            next_side: 0.0,
            fore: 0.0,
            next_fore: 0.0,
            next_jump: 0.0,
            air_jumps_left: m.air_jumps,
            up: 1.0,
            up_resume: 1.0,
            next_up: 0.0,
            hit_damage: 0.0,
            last_hit: f32::NEG_INFINITY,
            turn_at: None,
            turn_again_at: f32::NEG_INFINITY,
            blocked_for: 0.0,
            blocked_limit: 0.0,
            hurt: false,
            thrust_left: 0.0,
            spawn_right: spawn_right.normalize_or(Vec3::X),
            home,
        };
        s.dodge = if m.random_dodges { pick(rng, &m.dodges) } else { 0 };
        s.next_dodge = between(rng, m.change_dodge_s);
        s.begin_dodge(m, rng);
        s
    }

    fn begin_dodge(&mut self, m: &Motion, rng: &mut Rng) {
        let Some(d) = m.dodges.get(self.dodge) else { return };
        self.side = if d.strafe { start(rng, d.start_side) } else { 0.0 };
        self.resume = if self.side != 0.0 { self.side } else { 1.0 };
        self.next_side = self.strafe_time(d, rng, self.side);
        self.fore = if d.advance { start(rng, d.start_forward) } else { 0.0 };
        self.next_fore = self.advance_time(d, rng, self.fore);
        self.next_jump = between(rng, d.jump_s);
        self.next_up = between(rng, d.up_down_s);
        if let Some(b) = &d.blocked {
            self.blocked_limit = between(rng, b.reaction_s);
        }
    }

    fn strafe_time(&self, d: &Dodge, rng: &mut Rng, side: f32) -> f32 {
        between(rng, d.strafe_s) * if side < 0.0 { d.left_mult } else { d.right_mult }
    }

    fn advance_time(&self, d: &Dodge, rng: &mut Rng, fore: f32) -> f32 {
        between(rng, d.advance_s) * if fore < 0.0 { d.back_mult } else { d.forward_mult }
    }

    /// The bot took `damage` at `now` (seconds into the run).
    pub fn on_hit(&mut self, m: &Motion, rng: &mut Rng, damage: f32, now: f32) {
        self.hurt = true;
        let Some(h) = m.dodges.get(self.dodge).and_then(|d| d.hit_turn.as_ref()) else { return };
        if now - self.last_hit > h.reset_s.max(0.0) {
            self.hit_damage = 0.0;
        }
        self.last_hit = now;
        self.hit_damage += damage;
        if self.turn_at.is_none()
            && now >= self.turn_again_at
            && self.hit_damage >= h.threshold
            && rng.next_f32() < h.chance
        {
            self.turn_at = Some(now + between(rng, h.delay_s));
            self.hit_damage = 0.0;
        }
    }

    fn turn_around(&mut self) {
        if self.side != 0.0 {
            self.side = -self.side;
        } else {
            self.resume = -self.resume;
        }
    }

    /// What the bot wants to do this frame: updates its timers.
    fn think(&mut self, m: &Motion, rng: &mut Rng, pos: Vec3, now: f32, dt: f32) {
        if m.dodges.len() > 1 && m.change_dodge_s[1] > 0.0 {
            self.next_dodge -= dt;
            if self.next_dodge <= 0.0 {
                self.dodge = if m.random_dodges { pick(rng, &m.dodges) } else { (self.dodge + 1) % m.dodges.len() };
                self.next_dodge = between(rng, m.change_dodge_s).max(0.05);
                self.begin_dodge(m, rng);
            }
        }
        let Some(d) = m.dodges.get(self.dodge).cloned() else { return };
        if d.strafe {
            self.next_side -= dt;
            if self.next_side <= 0.0 {
                let pause = between(rng, d.strafe_pause_s);
                if self.side != 0.0 && pause > 0.0 {
                    self.resume = -self.side;
                    self.side = 0.0;
                    self.next_side = pause;
                } else {
                    self.side = if self.side == 0.0 { self.resume } else { -self.side };
                    self.next_side = self.strafe_time(&d, rng, self.side).max(0.01);
                }
            }
        } else {
            self.side = 0.0;
        }
        if let Some(at) = self.turn_at
            && now >= at
        {
            self.turn_around();
            self.turn_at = None;
            self.turn_again_at = now + m.dodges[self.dodge].hit_turn.as_ref().map_or(0.0, |h| h.cooldown_s);
        }
        let dist = Vec3::new(pos.x, 0.0, pos.z).length();
        if d.advance {
            self.next_fore -= dt;
            if self.next_fore <= 0.0 {
                self.fore = if self.fore == 0.0 { 1.0 } else { -self.fore };
                self.next_fore = self.advance_time(&d, rng, self.fore).max(0.01);
            }
        }
        let wants = if d.max_distance > 0.0 && dist > d.max_distance {
            1.0
        } else if d.min_distance > 0.0 && dist < d.min_distance {
            -1.0
        } else if d.advance {
            self.fore
        } else {
            0.0
        };
        if d.advance && wants != self.fore {
            // Pushed back into range: carry on that way.
            self.fore = wants;
        }
        if !d.advance {
            self.fore = wants;
        }
        if m.flight.is_some() {
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
        } else if d.jump_chance > 0.0 {
            self.next_jump -= dt;
            if self.next_jump <= 0.0 {
                self.next_jump = between(rng, d.jump_s);
                if rng.next_f32() < d.jump_chance {
                    self.jump(m, rng);
                    // A jetpack thrusts until the next time it decides.
                    if m.jetpack.is_some() {
                        self.thrust_left = self.next_jump.max(0.05);
                    }
                }
            }
        }
    }

    fn jump(&mut self, m: &Motion, rng: &mut Rng) {
        if m.gravity <= 0.0 {
            return; // with nothing to bring it down, a jump would never end
        }
        if self.grounded {
            self.vel.y = between(rng, m.jump_speed);
            self.grounded = false;
            self.air_jumps_left = m.air_jumps;
        } else if self.air_jumps_left > 0 && m.air_jump_speed > 0.0 {
            self.vel.y = m.air_jump_speed;
            self.air_jumps_left -= 1;
        }
    }

    /// Moves the bot for `dt` seconds: a capsule of `radius` with a middle segment of
    /// half-length `half` (0 for a ball) at `pos`, among `walls`.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        m: &Motion,
        rng: &mut Rng,
        walls: &Collider,
        pos: &mut Vec3,
        radius: f32,
        half: f32,
        now: f32,
        dt: f32,
    ) {
        if m.still_until_hurt && !self.hurt {
            return;
        }
        self.think(m, rng, *pos, now, dt);
        // Coming closer and backing off are toward and away from the player; strafes
        // are across the way the bot faces (the player, or its spawn's forward).
        let flat = Vec3::new(-pos.x, 0.0, -pos.z);
        let spawn_forward = Vec3::Y.cross(self.spawn_right).normalize_or(Vec3::Z);
        let toward = if flat.length_squared() > 1e-6 { flat.normalize() } else { spawn_forward };
        let facing = if m.faces_player { toward } else { spawn_forward };
        let right = facing.cross(Vec3::Y);
        let back = if self.fore < 0.0 { m.back_mult } else { 1.0 };
        let wish = right * self.side * m.strafe_mult + toward * self.fore * back;
        let cap = m.max_speed * wish.length();
        let dir = wish.normalize_or_zero();
        let before = Vec3::new(self.vel.x, 0.0, self.vel.z).length();

        let steps = (dt / SUBSTEP).ceil().clamp(1.0, 64.0) as usize;
        let h = dt / steps as f32;
        let mut touched_wall = false;
        for _ in 0..steps {
            self.physics(m, dir, cap, h);
            let was = *pos;
            let on_floor = self.grounded;
            *pos += self.vel * h;
            if self.grounded && self.vel.y <= 0.0 {
                // Keep in touch with the floor (and follow it down small steps).
                pos.y -= STICK;
            }
            let c = walls.push_out(*pos, half, radius);
            *pos = c.pos;
            let over_ledge = m.gravity > 0.0
                && if on_floor {
                    // Bots don't walk off ledges (nor get nudged off by a floor's edge).
                    walls.ray(*pos, Vec3::NEG_Y, half + radius + LEDGE_DROP).is_none()
                } else {
                    // Nor jump out over nothing at all: they may drop to a lower floor,
                    // not out of the map.
                    walls.ray(*pos, Vec3::NEG_Y, VOID).is_none() && walls.ray(was, Vec3::NEG_Y, VOID).is_some()
                };
            if over_ledge {
                // It stops as it would at a wall.
                pos.x = was.x;
                pos.z = was.z;
                self.vel.x = 0.0;
                self.vel.z = 0.0;
                touched_wall = true;
            }
            let mut floor = false;
            for n in &c.normals {
                let into = self.vel.dot(*n);
                if into < 0.0 {
                    if m.bounce_off_walls && n.y.abs() < FLOOR_NORMAL_Y {
                        self.vel -= *n * (2.0 * into);
                    } else {
                        self.vel -= *n * into;
                    }
                }
                if n.y > FLOOR_NORMAL_Y {
                    floor = true;
                } else if n.y.abs() < FLOOR_NORMAL_Y {
                    touched_wall = true;
                }
            }
            if floor && !self.grounded {
                self.air_jumps_left = m.air_jumps;
            }
            self.grounded = floor && self.vel.y <= 0.01;
            if self.grounded {
                self.vel.y = self.vel.y.min(0.0);
            }
        }
        // A wall in the way: turn around, at once or after the reaction time.
        if let Some(b) = m.dodges.get(self.dodge).and_then(|d| d.blocked.as_ref())
            && self.side != 0.0
        {
            if b.on_touch && touched_wall {
                self.side = -self.side;
            } else if b.fraction > 0.0 && cap > 0.0 {
                let speed = Vec3::new(self.vel.x, 0.0, self.vel.z).length().max(before.min(0.0));
                if speed < b.fraction * cap {
                    self.blocked_for += dt;
                    if self.blocked_for >= self.blocked_limit {
                        self.side = -self.side;
                        self.blocked_for = 0.0;
                        self.blocked_limit = between(rng, b.reaction_s);
                    }
                } else {
                    self.blocked_for = 0.0;
                }
            }
        }
        if pos.y < -200.0 {
            // Fell out of the map: back to where it appeared.
            *pos = self.home;
            self.vel = Vec3::ZERO;
        }
    }

    /// One substep of Unreal's `CalcVelocity`: friction and braking, then input
    /// acceleration up to the speed cap; gravity or flight vertically.
    fn physics(&mut self, m: &Motion, dir: Vec3, cap: f32, h: f32) {
        let flying = m.flight.is_some();
        let on_ground = self.grounded || flying;
        let mut v = Vec3::new(self.vel.x, 0.0, self.vel.z);
        let thrusting = self.thrust_left > 0.0 && m.jetpack.is_some();
        let air_control = match &m.jetpack {
            Some(j) if thrusting => j.air_control,
            _ => m.air_control,
        };
        let (friction, accel, braking) =
            if on_ground { (m.friction, m.accel, m.braking) } else { (m.air_friction, m.accel * air_control, 0.0) };
        let has_input = dir != Vec3::ZERO && cap > 0.0;
        let over = v.length() > cap * 1.001 + 1e-3;
        if !has_input || over {
            v = brake(v, friction * m.braking_friction_factor.max(0.0), braking, h);
        } else {
            let speed = v.length();
            v -= (v - dir * speed) * (h * friction).min(1.0);
        }
        if has_input {
            let limit = if v.length() > cap { v.length() } else { cap };
            v += dir * accel * h;
            v = v.clamp_length_max(limit);
        }
        self.vel.x = v.x;
        self.vel.z = v.z;
        if let Some(f) = &m.flight {
            let (goal, a) = if self.up > 0.0 {
                (f.up_speed, f.up_accel)
            } else if self.up < 0.0 {
                (-f.down_speed, f.down_accel)
            } else {
                (0.0, f.up_accel.max(f.down_accel))
            };
            let step = a.max(0.0) * h;
            self.vel.y += (goal - self.vel.y).clamp(-step, step);
        } else {
            self.vel.y -= m.gravity * h;
            if let Some(j) = &m.jetpack
                && self.thrust_left > 0.0
            {
                self.thrust_left -= h;
                if self.vel.y < j.max_up {
                    self.vel.y = (self.vel.y + j.thrust * h).min(j.max_up.max(self.vel.y));
                }
            }
            let terminal = if m.terminal_speed > 0.0 { m.terminal_speed } else { TERMINAL };
            self.vel.y = self.vel.y.max(-terminal);
        }
    }
}

/// Unreal's `ApplyVelocityBraking`: friction and a constant deceleration, never
/// reversing.
fn brake(v: Vec3, friction: f32, decel: f32, h: f32) -> Vec3 {
    if v == Vec3::ZERO || (friction <= 0.0 && decel <= 0.0) {
        return v;
    }
    let old = v;
    let rev = -v.normalize() * decel.max(0.0);
    let next = v + (-friction.max(0.0) * v + rev) * h;
    if next.dot(old) <= 0.0 || (decel > 0.0 && next.length() < STOP_SPEED) { Vec3::ZERO } else { next }
}

/// A dodge profile picked by weight.
fn pick(rng: &mut Rng, dodges: &[Dodge]) -> usize {
    let total: f32 = dodges.iter().map(|d| d.weight.max(0.0)).sum();
    if total <= 0.0 {
        return 0;
    }
    let mut x = rng.next_f32() * total;
    for (i, d) in dodges.iter().enumerate() {
        let w = d.weight.max(0.0);
        if x < w {
            return i;
        }
        x -= w;
    }
    dodges.len() - 1
}

/// Bots that bump into each other are pushed apart sideways, half each.
pub fn separate(bots: &mut [(Vec3, f32, f32)]) {
    for i in 0..bots.len() {
        for j in i + 1..bots.len() {
            let (a, ra, ha) = bots[i];
            let (b, rb, hb) = bots[j];
            if (a.y - b.y).abs() > ha + hb + ra + rb {
                continue;
            }
            let mut d = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
            let len = d.length();
            let need = ra + rb;
            if len >= need {
                continue;
            }
            if len < 1e-5 {
                d = Vec3::X;
            } else {
                d /= len;
            }
            let push = d * ((need - len) / 2.0);
            bots[i].0 -= push;
            bots[j].0 += push;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldMesh;

    const DT: f32 = 1.0 / 120.0;

    /// A floor at y = -2 and walls at x = ±6, with the bot 10 m in front.
    fn arena() -> Collider {
        let quad = |p: [[f32; 3]; 4]| WorldMesh {
            positions: p.to_vec(),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..WorldMesh::default()
        };
        Collider::new(&[
            quad([[-50.0, -2.0, -50.0], [50.0, -2.0, -50.0], [50.0, -2.0, 50.0], [-50.0, -2.0, 50.0]]),
            quad([[6.0, -2.0, -50.0], [6.0, 8.0, -50.0], [6.0, 8.0, 50.0], [6.0, -2.0, 50.0]]),
            quad([[-6.0, -2.0, -50.0], [-6.0, 8.0, -50.0], [-6.0, 8.0, 50.0], [-6.0, -2.0, 50.0]]),
        ])
    }

    fn strafer() -> Motion {
        Motion {
            max_speed: 4.0,
            accel: 40.0,
            dodges: vec![Dodge { strafe_s: [1.0, 1.0], ..Dodge::default() }],
            ..Motion::default()
        }
    }

    fn run(m: &Motion, pos: Vec3, seconds: f32, mut each: impl FnMut(f32, Vec3, &BotState)) -> (Vec3, BotState) {
        let walls = arena();
        let mut rng = Rng::new(7);
        let mut s = BotState::new(m, &mut rng, pos, Vec3::X);
        let mut p = pos;
        let mut t = 0.0;
        while t < seconds {
            s.step(m, &mut rng, &walls, &mut p, 0.5, 0.0, t, DT);
            t += DT;
            each(t, p, &s);
        }
        (p, s)
    }

    #[test]
    fn a_bot_falls_to_the_floor_and_rests_on_it() {
        let m = Motion { dodges: vec![], ..strafer() };
        let (p, s) = run(&m, Vec3::new(0.0, 3.0, -10.0), 3.0, |_, _, _| {});
        assert!((p.y - -1.5).abs() < 0.01, "a 0.5 m ball rests with its center 0.5 m above the floor: {p}");
        assert!(s.grounded);
    }

    #[test]
    fn strafes_switch_sides_on_the_timer_at_top_speed() {
        let m = strafer();
        let mut xs = Vec::new();
        let mut top: f32 = 0.0;
        run(&m, Vec3::new(0.0, -1.5, -10.0), 4.0, |t, p, s| {
            xs.push((t, p.x));
            top = top.max(Vec3::new(s.vel.x, 0.0, s.vel.z).length());
        });
        assert!((top - 4.0).abs() < 0.05, "never faster than its top speed: {top}");
        // Facing the player, the bot's right is the player's left (-X); a second
        // later it turns and comes back.
        let at = |t: f32| xs.iter().find(|(s, _)| *s >= t).unwrap().1;
        assert!(at(0.95) < -3.0, "{}", at(0.95));
        assert!(at(1.95) > at(0.95) + 3.0, "{} {}", at(0.95), at(1.95));
    }

    #[test]
    fn walls_stop_a_strafe_and_blocked_bots_turn_around() {
        // Long strafes into a wall 6 m away.
        let long = Dodge { strafe_s: [10.0, 10.0], ..Dodge::default() };
        let m = Motion { dodges: vec![long.clone()], ..strafer() };
        let (p, _) = run(&m, Vec3::new(0.0, -1.5, -10.0), 3.0, |_, _, _| {});
        assert!((p.x - -5.5).abs() < 0.02, "stopped at the wall: {p}");
        let turning =
            Dodge { blocked: Some(Blocked { fraction: 0.7, reaction_s: [0.2, 0.2], on_touch: false }), ..long };
        // Facing the way its spawn faces, the bot runs straight at the wall (strafing
        // around the player, it would slide along it instead).
        let m = Motion { dodges: vec![turning], faces_player: false, ..strafer() };
        let (mut touched, mut back) = (false, false);
        run(&m, Vec3::new(0.0, -1.5, -10.0), 4.0, |_, p, _| {
            touched |= p.x > 5.45;
            back |= touched && p.x < 0.0;
        });
        assert!(touched && back, "turned around at the wall and headed back");
    }

    /// Bug: on maps whose floor ends before the walls, bots strafed off the edge and
    /// fell (4BK - SYW Truly FIXED, Predictable Strafes).
    #[test]
    fn bots_turn_at_ledges_instead_of_walking_off() {
        let quad = |p: [[f32; 3]; 4]| WorldMesh {
            positions: p.to_vec(),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..WorldMesh::default()
        };
        // A platform 6 m wide under the bot, nothing else.
        let walls =
            Collider::new(&[quad([[-3.0, -2.0, -20.0], [3.0, -2.0, -20.0], [3.0, -2.0, 0.0], [-3.0, -2.0, 0.0]])]);
        let long = Dodge {
            strafe_s: [10.0, 10.0],
            blocked: Some(Blocked { fraction: 0.5, reaction_s: [0.2, 0.2], on_touch: false }),
            ..Dodge::default()
        };
        let m = Motion { dodges: vec![long], faces_player: false, ..strafer() };
        let mut rng = Rng::new(3);
        let mut s = BotState::new(&m, &mut rng, Vec3::ZERO, Vec3::X);
        let mut p = Vec3::new(0.0, -1.5, -10.0);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..(6.0 / DT) as usize {
            s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
            lo = lo.min(p.x);
            hi = hi.max(p.x);
        }
        assert!(p.y > -1.6, "still on the platform: {p}");
        assert!(hi > 2.9 && lo < -2.9 && hi < 3.05 && lo > -3.05, "went edge to edge: {lo}..{hi}");
        // One that jumps as it goes doesn't jump off into nothing either.
        let hopping =
            Motion { dodges: vec![Dodge { jump_chance: 1.0, jump_s: [0.3, 0.3], ..m.dodges[0].clone() }], ..m.clone() };
        let mut s = BotState::new(&hopping, &mut rng, Vec3::ZERO, Vec3::X);
        let mut p = Vec3::new(0.0, -1.5, -10.0);
        let mut lowest = f32::MAX;
        for i in 0..(6.0 / DT) as usize {
            s.step(&hopping, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
            lowest = lowest.min(p.y);
        }
        assert!(lowest > -1.6 && p.x.abs() < 3.6, "{p} lowest {lowest}");
    }

    /// Bug: a bot that doesn't face the player and was too far "came closer" along its
    /// spawn's forward, away from the player, and ran off the map.
    #[test]
    fn bots_that_dont_face_the_player_still_come_closer_to_them() {
        let keep = Dodge { strafe: false, min_distance: 6.0, max_distance: 8.0, ..Dodge::default() };
        // The spawn's right is +X, so it faces -Z: away from the player.
        let m = Motion { dodges: vec![keep], faces_player: false, ..strafer() };
        let walls = arena();
        let mut rng = Rng::new(7);
        let mut s = BotState::new(&m, &mut rng, Vec3::ZERO, Vec3::X);
        let mut p = Vec3::new(0.0, -1.5, -20.0);
        for i in 0..(6.0 / DT) as usize {
            s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
        }
        assert!((7.5..=8.5).contains(&Vec3::new(p.x, 0.0, p.z).length()), "{p}");
    }

    #[test]
    fn a_jetpack_lifts_against_gravity_while_it_thrusts() {
        // Thrust 2 m/s² against gravity 1 m/s²: always thrusting, it climbs at 1 m/s²
        // up to its top climbing speed of 0.5 m/s.
        let hop = Dodge { strafe: false, jump_chance: 1.0, jump_s: [0.5, 0.5], ..Dodge::default() };
        let m = Motion {
            gravity: 1.0,
            jump_speed: [0.0, 0.0],
            jetpack: Some(Jetpack { thrust: 2.0, max_up: 0.5, air_control: 1.0 }),
            dodges: vec![hop],
            ..strafer()
        };
        let (p, s) = run(&m, Vec3::new(0.0, 0.0, -10.0), 4.0, |_, _, _| {});
        assert!((s.vel.y - 0.5).abs() < 0.02, "{}", s.vel.y);
        assert!(p.y > 1.0, "{p}");
    }

    #[test]
    fn jumps_rise_as_high_as_their_speed_allows() {
        let hop = Dodge { strafe: false, jump_chance: 1.0, jump_s: [0.5, 0.5], ..Dodge::default() };
        let m = Motion { jump_speed: [5.0, 5.0], dodges: vec![hop], ..strafer() };
        let mut peak = f32::MIN;
        run(&m, Vec3::new(0.0, -1.5, -10.0), 3.0, |_, p, _| peak = peak.max(p.y));
        let rise = 5.0f32 * 5.0 / (2.0 * 9.8);
        assert!((peak - (-1.5 + rise)).abs() < 0.05, "peak {peak}, expected {}", -1.5 + rise);
    }

    #[test]
    fn bots_come_closer_or_back_off_to_keep_their_distance() {
        let keep = Dodge { strafe: false, min_distance: 6.0, max_distance: 8.0, ..Dodge::default() };
        let m = Motion { dodges: vec![keep], ..strafer() };
        let (far, _) = run(&m, Vec3::new(0.0, -1.5, -20.0), 6.0, |_, _, _| {});
        assert!((7.5..=8.5).contains(&far.length()), "came in from 20 m: {}", far.length());
        let (near, _) = run(&m, Vec3::new(0.0, -1.5, -3.0), 6.0, |_, _, _| {});
        assert!((5.5..=6.5).contains(&Vec3::new(near.x, 0.0, near.z).length()), "backed off from 3 m: {near}");
    }

    #[test]
    fn fliers_go_up_and_down_on_their_own_timer() {
        let bob = Dodge { strafe: false, up_down_s: [0.5, 0.5], ..Dodge::default() };
        let m = Motion {
            flight: Some(Flight { up_speed: 2.0, down_speed: 2.0, up_accel: 100.0, down_accel: 100.0 }),
            dodges: vec![bob],
            ..strafer()
        };
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (p, _) = run(&m, Vec3::new(0.0, 2.0, -10.0), 4.0, |_, p, _| {
            lo = lo.min(p.y);
            hi = hi.max(p.y);
        });
        assert!((hi - lo) > 0.8 && (hi - lo) < 1.2, "about a meter of travel: {lo}..{hi}");
        assert!((p.y - 2.0).abs() < 1.2, "{p}");
    }

    #[test]
    fn a_hit_can_make_the_bot_change_direction() {
        let turn = HitTurn { chance: 1.0, delay_s: [0.1, 0.1], cooldown_s: 1.0, threshold: 0.0, reset_s: 1.0 };
        let d = Dodge { strafe_s: [10.0, 10.0], hit_turn: Some(turn), ..Dodge::default() };
        let m = Motion { dodges: vec![d], ..strafer() };
        let walls = arena();
        let mut rng = Rng::new(1);
        let mut s = BotState::new(&m, &mut rng, Vec3::ZERO, Vec3::X);
        let mut p = Vec3::new(0.0, -1.5, -10.0);
        for i in 0..60 {
            s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
        }
        let x = p.x;
        s.on_hit(&m, &mut rng, 1.0, 0.5);
        for i in 60..240 {
            s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
        }
        assert!(x < -1.0 && p.x > x + 1.0, "went left, then turned back after the hit: {x} → {}", p.x);
    }

    #[test]
    fn still_bots_wait_until_hurt_and_bots_push_apart() {
        let m = Motion { still_until_hurt: true, ..strafer() };
        let (p, _) = run(&m, Vec3::new(0.0, 0.0, -10.0), 1.0, |_, _, _| {});
        assert_eq!(p, Vec3::new(0.0, 0.0, -10.0));
        let mut two = [(Vec3::new(0.0, 0.0, -5.0), 0.5, 0.0), (Vec3::new(0.2, 0.0, -5.0), 0.5, 0.0)];
        separate(&mut two);
        assert!((two[1].0.x - two[0].0.x - 1.0).abs() < 1e-5);
    }
}
