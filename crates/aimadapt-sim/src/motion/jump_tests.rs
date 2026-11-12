//! Tests for the jump key: jumps, jetpacks and fliers.

use glam::Vec3;

use super::tests::{DT, arena, strafer};
use super::*;
use crate::collide::Collider;
use crate::rng::Rng;

/// Plays `seconds` from `start` (a ball of radius 0.5 over the arena's floor, whose
/// resting height is -1.5) with the arena's walls. `grounded` is whether it appears
/// standing. `each` sees the time, the position and the state after every frame.
fn play(
    m: &Motion,
    start: Vec3,
    grounded: bool,
    seconds: f32,
    mut each: impl FnMut(f32, Vec3, &BotState),
) -> (Vec3, BotState) {
    let walls = arena();
    let mut rng = Rng::new(7);
    let mut s = BotState::new(m, &mut rng, start, Vec3::X);
    s.grounded = grounded;
    let mut p = start;
    let mut t = 0.0;
    while t < seconds {
        s.step(m, &mut rng, &walls, &mut p, 0.5, 0.0, t, DT);
        t += DT;
        each(t, p, &s);
    }
    (p, s)
}

/// Where a ball rests on the arena's floor.
const FLOOR: Vec3 = Vec3::new(0.0, -1.5, -10.0);

/// A dodge profile that decides about the jump key every `window` seconds and never moves
/// sideways.
fn deciding(chance: f32, window: f32) -> Dodge {
    Dodge { strafe: false, jump_chance: chance, jump_s: [window, window], ..Dodge::default() }
}

/// A jumper whose jump takes 0.4 s, shorter than the windows the tests use.
fn jumper(chance: f32, window: f32) -> Motion {
    Motion { gravity: 10.0, jump_speed: [2.0, 2.0], dodges: vec![deciding(chance, window)], ..strafer() }
}

/// How many times the bot's upward speed jumped by more than a meter a second in one frame:
/// and ends above a meter a second: a jump or an air jump (gravity and a jetpack change it far
/// less than that, and a landing ends at rest).
fn jumps(m: &Motion, start: Vec3, grounded: bool, seconds: f32) -> usize {
    let (mut count, mut last) = (0, 0.0);
    play(m, start, grounded, seconds, |_, _, s| {
        if s.vel.y > 1.0 && s.vel.y > last + 1.0 {
            count += 1;
        }
        last = s.vel.y;
    });
    count
}

/// Bug: a jumper jumped at every decision to hold the key, so with a chance of 1 it hopped
/// as often as its windows came round. In KovaaK's a jump needs a press (the key going
/// down while the bot stands), and two decisions in a row to hold are one long hold: a bot
/// told to hold the key in every window hopped once in 147 s.
#[test]
fn a_held_key_is_one_jump_not_one_per_decision() {
    let m = jumper(1.0, 0.5);
    // Appearing in the air: the one jump is at the first landing.
    assert_eq!(jumps(&m, FLOOR, false, 4.0), 1);
    // Appearing standing: it jumps at once, and that is the only one.
    assert_eq!(jumps(&m, FLOOR, true, 4.0), 1);
}

/// Bug: a jetpack bot thrust only from its first decision, one window after it appeared,
/// and for one window per decision. With the key held it thrusts for as long as it is held:
/// here from the moment it appears.
#[test]
fn a_jetpack_thrusts_from_the_start_while_the_key_is_held() {
    let m = Motion {
        gravity: 10.0,
        jump_speed: [0.0, 0.0],
        jetpack: Some(Jetpack { thrust: 20.0, max_up: 3.0, air_control: 1.0, ..Jetpack::default() }),
        dodges: vec![deciding(1.0, 1.0)],
        ..strafer()
    };
    let mut at_half_a_second = f32::NAN;
    play(&m, Vec3::new(0.0, 2.0, -10.0), false, 0.6, |t, _, s| {
        if t < 0.5 + DT / 2.0 {
            at_half_a_second = s.vel.y;
        }
    });
    assert!(at_half_a_second > 2.0, "climbing at 3 m/s: {at_half_a_second}");
}

/// Bug: every flier bobbed up and down by its toggle times. In KovaaK's only a dodge
/// profile with `AlterateJumpCrouchInput` on does that. With it off a flier with no key
/// down stays where it is.
#[test]
fn a_flier_flown_by_keys_with_no_key_down_hovers() {
    let m = Motion {
        gravity: 0.0,
        flight: Some(Flight { up_speed: 2.0, down_speed: 2.0, up_accel: 100.0, down_accel: 100.0, up_on_both: false }),
        dodges: vec![Dodge { strafe: false, up_down_s: [0.5, 0.5], fly_by_keys: true, ..Dodge::default() }],
        ..strafer()
    };
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    play(&m, Vec3::new(0.0, 2.0, -10.0), false, 3.0, |_, p, _| {
        lo = lo.min(p.y);
        hi = hi.max(p.y);
    });
    assert!(hi - lo < 0.01, "hovered at one height: {lo}..{hi}");
}

/// A bot whose jump key the test works by hand: its profile decides so rarely, and with so
/// small a chance, that it never changes the key itself.
fn manual(m: Motion) -> Motion {
    Motion { dodges: vec![deciding(1e-9, 1e6)], ..m }
}

/// A bot played frame by frame, with its key pressed and let go by the test.
struct Rig {
    m: Motion,
    walls: Collider,
    rng: Rng,
    s: BotState,
    p: Vec3,
    t: f32,
    jumps: usize,
}

impl Rig {
    /// `grounded`: the bot stands there. It is given one frame to settle first, so it
    /// has been on the ground since it appeared, as a bot is that spawns on a floor.
    fn new(m: Motion, start: Vec3, grounded: bool) -> Rig {
        let mut rng = Rng::new(7);
        let mut s = BotState::new(&m, &mut rng, start, Vec3::X);
        s.grounded = grounded;
        let mut rig = Rig { m, walls: arena(), rng, s, p: start, t: 0.0, jumps: 0 };
        if grounded {
            rig.run(DT);
        }
        rig
    }

    fn vy(&self) -> f32 {
        self.s.vel.y
    }

    /// A jump or air jump: the upward speed went up by a meter a second or more and ends
    /// above that.
    fn note(&mut self, before: f32) {
        if self.vy() > 1.0 && self.vy() > before + 1.0 {
            self.jumps += 1;
        }
    }

    /// The key goes down or up, as when the bot decides.
    fn key(&mut self, down: bool) {
        let before = self.vy();
        self.s.set_jump_key(&self.m, &mut self.rng, down);
        self.note(before);
    }

    /// Lets go and presses again.
    fn press(&mut self) {
        self.key(false);
        self.key(true);
    }

    fn run(&mut self, seconds: f32) {
        for _ in 0..(seconds / DT).round() as usize {
            let before = self.vy();
            self.s.step(&self.m, &mut self.rng, &self.walls, &mut self.p, 0.5, 0.0, self.t, DT);
            self.t += DT;
            self.note(before);
        }
    }
}

/// A character with gravity 10 whose jump is 4 m/s, so a jump lasts 0.8 s.
fn hopper() -> Motion {
    manual(Motion { gravity: 10.0, jump_speed: [4.0, 4.0], ..strafer() })
}

#[test]
fn a_press_on_the_ground_jumps_and_a_held_key_doesnt_again() {
    let mut rig = Rig::new(hopper(), FLOOR, true);
    rig.key(true);
    assert_eq!((rig.vy(), rig.s.grounded), (4.0, false));
    rig.run(1.5);
    assert!(rig.s.grounded && rig.jumps == 1, "landed with the key held and stayed: {} jumps", rig.jumps);
    // Let go and press again: another jump.
    rig.press();
    assert_eq!(rig.vy(), 4.0);
}

/// A bot that appears in the air with the key held jumps when it first lands, with no
/// air jump on the way down, and a key that stays held then makes no air jump either.
#[test]
fn the_first_landing_jumps_but_a_held_key_makes_no_air_jump() {
    let m = Motion { air_jumps: 1, air_jump_speed: 6.0, ..jumper(1.0, 100.0) };
    assert_eq!(jumps(&m, Vec3::new(0.0, 1.0, -10.0), false, 3.0), 1, "from the air: the one jump at the landing");
    assert_eq!(jumps(&m, FLOOR, true, 3.0), 1, "from the ground: the one jump at once");
}

#[test]
fn a_pogo_bot_jumps_at_every_landing_with_the_key_held() {
    let mut rig = Rig::new(Motion { pogo: true, ..hopper() }, FLOOR, true);
    rig.key(true);
    rig.run(3.0);
    assert!(rig.jumps >= 4, "a jump about every 0.8 s: {}", rig.jumps);
}

/// A press in the air with no air jump left is remembered when the character buffers
/// jumps: it jumps at the landing if the key is held then. Letting go forgets it.
#[test]
fn a_buffered_press_jumps_at_the_landing() {
    for (buffered, jetpack, jumps) in [(true, None, 2), (false, None, 1), (true, Some(Jetpack::default()), 2)] {
        let mut rig = Rig::new(Motion { buffered_jumps: buffered, jetpack, ..hopper() }, FLOOR, true);
        rig.key(true);
        rig.run(0.1);
        rig.key(false);
        rig.run(0.1);
        rig.key(true);
        assert_eq!(rig.s.keys.buffered, buffered);
        rig.run(1.2);
        assert_eq!(rig.jumps, jumps, "buffered {buffered}, jetpack {}", rig.m.jetpack.is_some());
    }
    let mut rig = Rig::new(Motion { buffered_jumps: true, ..hopper() }, FLOOR, true);
    rig.key(true);
    rig.run(0.1);
    rig.press();
    assert!(rig.s.keys.buffered);
    rig.key(false);
    assert!(!rig.s.keys.buffered, "letting go forgets the press");
}

/// The air jumps: one per press in the air up to `air_jumps`, at `air_jump_speed`, and a
/// landing refills them.
#[test]
fn air_jumps_run_out_and_a_landing_refills_them() {
    let mut rig = Rig::new(Motion { air_jumps: 2, air_jump_speed: 6.0, ..hopper() }, FLOOR, true);
    rig.key(true);
    for left in [2, 1, 0] {
        rig.run(0.1);
        rig.press();
        let made = rig.vy() == 6.0;
        assert_eq!(made, left > 0, "{left} air jumps left, upward speed {}", rig.vy());
        rig.run(0.2);
    }
    assert_eq!(rig.jumps, 3, "a jump and two air jumps");
    rig.key(false);
    rig.run(3.0);
    assert!(rig.s.grounded);
    rig.run(0.1);
    rig.key(true);
    rig.run(0.1);
    rig.press();
    assert_eq!(rig.vy(), 6.0, "refilled by the landing");
}

/// The jetpacks. A bot in the air that presses the key, or one on the ground; thrust 30
/// against gravity 10, with no cap on the climb unless a test sets one.
fn jetpack(delay_s: f32, fuel_s: f32, refill_per_s: f32) -> Motion {
    manual(Motion {
        gravity: 10.0,
        jump_speed: [0.0, 0.0],
        jetpack: Some(Jetpack {
            thrust: 30.0,
            max_up: 100.0,
            air_control: 1.0,
            delay_s,
            fuel_s,
            refill_per_s,
            refill_in_air: true,
        }),
        ..strafer()
    })
}

const HIGH: Vec3 = Vec3::new(0.0, 6.0, -10.0);

#[test]
fn a_jetpack_waits_out_its_delay_on_the_ground_but_not_in_the_air() {
    let mut rig = Rig::new(jetpack(0.5, 0.0, 0.0), FLOOR, true);
    rig.key(true);
    rig.run(0.4);
    assert!(rig.vy() == 0.0 && rig.s.grounded, "no thrust yet: {}", rig.vy());
    rig.run(0.2);
    assert!(rig.vy() > 0.5, "thrusting since 0.5 s: {}", rig.vy());
    let mut rig = Rig::new(jetpack(0.5, 0.0, 0.0), HIGH, false);
    rig.key(true);
    rig.run(0.05);
    assert!(rig.vy() > 0.5, "thrusts at once in the air: {}", rig.vy());
}

/// A bot that lands with the key held keeps thrusting: the delay doesn't start again.
#[test]
fn landing_with_the_key_held_starts_no_new_delay() {
    let mut weak = jetpack(0.5, 0.0, 0.0);
    weak.jetpack.as_mut().unwrap().thrust = 8.0; // less than gravity: it comes down
    let mut rig = Rig::new(weak, HIGH, false);
    rig.key(true);
    rig.run(4.0);
    assert!(rig.s.grounded && rig.s.keys.delay_left == 0.0, "{}", rig.s.keys.delay_left);
}

/// A kind with a jump speed jumps at a press on the ground, and the thrust follows after
/// the delay.
#[test]
fn a_press_on_the_ground_jumps_and_the_thrust_follows_the_delay() {
    let m = Motion { jump_speed: [4.0, 4.0], ..jetpack(0.3, 0.0, 0.0) };
    let mut rig = Rig::new(m, FLOOR, true);
    rig.key(true);
    assert_eq!(rig.vy(), 4.0);
    rig.run(0.2);
    assert!((rig.vy() - 2.0).abs() < 0.2, "gravity alone so far: {}", rig.vy());
    rig.run(0.3);
    assert!(rig.vy() > 2.0, "thrust since 0.3 s: {}", rig.vy());
}

#[test]
fn a_jetpack_thrusts_while_the_key_is_held_and_stops_when_it_is_let_go() {
    let mut rig = Rig::new(jetpack(0.0, 0.0, 0.0), HIGH, false);
    rig.key(true);
    rig.run(4.0);
    assert!(rig.vy() > 60.0, "an endless tank keeps thrusting: {}", rig.vy());
    let before = rig.vy();
    rig.key(false);
    rig.run(0.3);
    assert!(rig.vy() < before - 2.0, "{} then {}", before, rig.vy());
}

#[test]
fn the_tank_drains_while_the_key_is_held_even_before_the_thrust() {
    let mut rig = Rig::new(jetpack(0.0, 1.0, 0.0), HIGH, false);
    rig.key(true);
    rig.run(0.5);
    assert!((rig.s.keys.tank - 0.5).abs() < 0.02, "{}", rig.s.keys.tank);
    rig.run(0.6);
    assert!(rig.s.keys.tank == 0.0 && rig.s.keys.wait > 0.0, "dry: {}", rig.s.keys.tank);
    // With no refill and the key still held it stays off.
    rig.run(0.5);
    let at = rig.vy();
    rig.run(1.0);
    assert!(rig.vy() < at - 8.0, "falling freely: {at} then {}", rig.vy());
    // The delay drains it too.
    let mut rig = Rig::new(jetpack(0.5, 1.0, 0.0), FLOOR, true);
    rig.key(true);
    rig.run(0.3);
    assert!((rig.s.keys.tank - 0.7).abs() < 0.02, "{}", rig.s.keys.tank);
}

#[test]
fn the_tank_refills_while_the_jetpack_isnt_thrusting() {
    let mut rig = Rig::new(jetpack(0.0, 1.0, 1.0), HIGH, false);
    rig.key(true);
    rig.run(0.5);
    rig.key(false);
    rig.run(0.25);
    assert!((rig.s.keys.tank - 0.75).abs() < 0.03, "{}", rig.s.keys.tank);
    rig.run(1.0);
    assert!((rig.s.keys.tank - 1.0).abs() < 1e-3, "a full tank stays full: {}", rig.s.keys.tank);
}

/// With a refill of 2 a bot that keeps the key held is on and off for a quarter second
/// each: it runs dry, waits 0.25 s whatever the key does, and then has a quarter second
/// of fuel.
#[test]
fn a_jetpack_that_ran_dry_waits_a_quarter_second() {
    let mut rig = Rig::new(jetpack(0.0, 1.0, 2.0), HIGH, false);
    rig.key(true);
    rig.run(1.2);
    let mut runs: Vec<(bool, usize)> = Vec::new();
    for _ in 0..(2.0 / DT) as usize {
        let before = rig.vy();
        rig.run(DT);
        let on = rig.vy() > before;
        match runs.last_mut() {
            Some((was, n)) if *was == on => *n += 1,
            _ => runs.push((on, 1)),
        }
    }
    // The whole runs in the middle, not the cut ones at the ends.
    let whole: Vec<usize> = runs[1..runs.len() - 1].iter().map(|(_, n)| *n).collect();
    assert!(whole.len() >= 4 && whole.iter().all(|n| (27..=33).contains(n)), "frames on and off: {whole:?}");

    // Letting go and pressing again during the wait doesn't start the thrust early.
    let mut rig = Rig::new(jetpack(0.0, 0.5, 4.0), HIGH, false);
    rig.key(true);
    rig.run(0.55);
    assert!(rig.s.keys.wait > 0.0);
    rig.key(false);
    rig.run(0.05);
    rig.key(true);
    let at = rig.vy();
    rig.run(0.05);
    assert!(rig.vy() < at, "still waiting, with fuel: {at} then {}", rig.vy());
    rig.run(0.15);
    let at = rig.vy();
    rig.run(0.05);
    assert!(rig.vy() > at + 0.5, "thrusting again: {at} then {}", rig.vy());
}

/// With `refill_in_air` off the tank refills only on the ground with the key up.
#[test]
fn the_tank_refills_in_the_air_only_when_it_may() {
    let tank_after = |in_air: bool, start: Vec3, grounded: bool| {
        let mut m = jetpack(0.0, 1.0, 1.0);
        m.jetpack.as_mut().unwrap().refill_in_air = in_air;
        let mut rig = Rig::new(m, start, grounded);
        rig.s.keys.tank = 0.5;
        rig.run(0.2);
        rig.s.keys.tank
    };
    assert!((tank_after(true, HIGH, false) - 0.7).abs() < 0.02);
    assert_eq!(tank_after(false, HIGH, false), 0.5, "not in the air");
    assert!((tank_after(false, FLOOR, true) - 0.7).abs() < 0.02, "on the ground");
}

/// A bot that has the jump settings but never moves (the importer's still bot) has no
/// dodge profile to decide with, so it never presses the key: it doesn't jump, thrust or fly.
#[test]
fn a_bot_with_no_dodge_profile_never_jumps_or_thrusts() {
    let m = Motion {
        gravity: 10.0,
        jump_speed: [4.0, 4.0],
        pogo: true,
        jetpack: Some(Jetpack { thrust: 30.0, max_up: 100.0, air_control: 1.0, ..Jetpack::default() }),
        ..Motion::default()
    };
    let (mut top, mut jumps) = (f32::MIN, 0);
    let mut last = 0.0;
    play(&m, FLOOR, true, 3.0, |_, p, s| {
        top = top.max(p.y);
        jumps += usize::from(s.vel.y > last + 1.0);
        last = s.vel.y;
    });
    assert!(top < -1.49 && jumps == 0, "stayed on the floor: top {top}, {jumps} jumps");
}

/// A flier flown by keys, gravity 0 unless a test says: up 2 m/s gaining 50 m/s², down 3
/// m/s gaining 100 m/s². The test works its keys by hand.
fn flier(gravity: f32, up_on_both: bool) -> Motion {
    let hold = Dodge {
        strafe: false,
        fly_by_keys: true,
        jump_chance: 1e-9,
        jump_s: [1e6, 1e6],
        crouch_chance: 1e-9,
        crouch_s: [1e6, 1e6],
        ..Dodge::default()
    };
    Motion {
        gravity,
        jump_speed: [4.0, 4.0],
        jetpack: Some(Jetpack { thrust: 30.0, max_up: 100.0, air_control: 1.0, ..Jetpack::default() }),
        flight: Some(Flight { up_speed: 2.0, down_speed: 3.0, up_accel: 50.0, down_accel: 100.0, up_on_both }),
        dodges: vec![hold],
        ..strafer()
    }
}

fn keys(rig: &mut Rig, jump: bool, crouch: bool) {
    rig.s.keys.held = jump;
    rig.s.keys.crouch = crouch;
}

/// The jump key flies a flier up and a crouch key down, both together cancel unless it
/// rises on both, and with no key it stops. It never jumps or thrusts, whatever its jump
/// speed or jetpack.
#[test]
fn a_flier_flown_by_keys_goes_up_down_or_stays() {
    let mut rig = Rig::new(flier(0.0, false), HIGH, false);
    let mut fastest: f32 = 0.0;
    for (jump, crouch, speed) in [(true, false, 2.0), (false, false, 0.0), (false, true, -3.0), (true, true, 0.0)] {
        keys(&mut rig, jump, crouch);
        for _ in 0..120 {
            rig.run(DT);
            fastest = fastest.max(rig.vy().abs());
        }
        assert!((rig.vy() - speed).abs() < 0.01, "jump {jump}, crouch {crouch}: {}", rig.vy());
    }
    assert!(fastest <= 3.001 && rig.jumps == 0, "no jump or thrust: {fastest}, {} jumps", rig.jumps);
    let mut rig = Rig::new(flier(0.0, true), HIGH, false);
    keys(&mut rig, true, true);
    rig.run(1.0);
    assert!((rig.vy() - 2.0).abs() < 0.01, "both keys rise with up_on_both: {}", rig.vy());
}

/// At a new input the vertical speed starts from zero, and gains speed at that
/// direction's own acceleration.
#[test]
fn a_fliers_speed_starts_from_zero_at_a_new_input() {
    let mut rig = Rig::new(flier(0.0, false), HIGH, false);
    keys(&mut rig, true, false);
    rig.run(0.03);
    assert!((rig.vy() - 50.0 * rig.t).abs() < 0.05, "{} after {}", rig.vy(), rig.t);
    rig.run(1.0);
    assert!((rig.vy() - 2.0).abs() < 0.01);
    keys(&mut rig, false, true);
    let t = rig.t;
    rig.run(0.02);
    assert!((rig.vy() + 100.0 * (rig.t - t)).abs() < 0.05, "turned at once, from zero: {}", rig.vy());
}

/// A flier with gravity stands on a floor and falls with no key down, keeps its speed as
/// it falls, and with a key down gravity doesn't act.
#[test]
fn a_flier_with_gravity_falls_with_no_key_and_stands_on_the_floor() {
    let mut rig = Rig::new(flier(10.0, false), Vec3::new(0.0, 2.0, -10.0), false);
    rig.run(3.0);
    assert!(rig.s.grounded && (rig.p.y - -1.5).abs() < 0.01, "{}", rig.p.y);
    keys(&mut rig, true, false);
    rig.run(1.0);
    assert!((rig.vy() - 2.0).abs() < 0.01 && rig.p.y > -0.5, "up against gravity: {} at {}", rig.vy(), rig.p.y);
    keys(&mut rig, false, false);
    rig.run(DT);
    assert!(rig.vy() > 1.5, "keeps its speed and falls from there: {}", rig.vy());
    rig.run(3.0);
    assert!(rig.s.grounded && (rig.p.y - -1.5).abs() < 0.01);
    // Never jumps, even standing with no key and a jump speed.
    assert_eq!(rig.jumps, 0);
}

/// A flier that alternates, with a pause between: still with no gravity, falling with it.
#[test]
fn an_alternating_flier_pauses_and_a_flier_with_gravity_falls_in_the_pause() {
    let pausing = |gravity: f32| Motion {
        gravity,
        flight: Some(Flight { up_speed: 2.0, down_speed: 2.0, up_accel: 100.0, down_accel: 100.0, up_on_both: false }),
        dodges: vec![Dodge { strafe: false, up_down_s: [1.0, 1.0], up_down_pause_s: [0.5, 0.5], ..Dodge::default() }],
        ..strafer()
    };
    for (gravity, still) in [(0.0, true), (10.0, false)] {
        let mut rig = Rig::new(pausing(gravity), HIGH, false);
        rig.run(0.9);
        assert!((rig.vy() - 2.0).abs() < 0.01, "up for the first toggle time: {}", rig.vy());
        rig.run(0.55);
        assert_eq!(rig.vy() == 0.0, still, "gravity {gravity}: {}", rig.vy());
        assert!(still || rig.vy() < 0.0, "falling in the pause: {}", rig.vy());
    }
}

/// The keys' own decisions fly the flier: a crouch with a chance of 1 flies it down,
/// and both keys together cancel.
#[test]
fn a_fliers_crouch_key_decides_on_its_own_windows() {
    let crouching = |jump_chance: f32, up_on_both: bool| {
        let mut m = flier(0.0, up_on_both);
        m.dodges = vec![Dodge {
            strafe: false,
            fly_by_keys: true,
            jump_chance,
            jump_s: [0.5, 0.5],
            crouch_chance: 1.0,
            crouch_s: [0.5, 0.5],
            ..Dodge::default()
        }];
        let mut rig = Rig::new(m, HIGH, false);
        rig.run(2.0);
        rig.vy()
    };
    assert!((crouching(0.0, false) + 3.0).abs() < 0.01, "the crouch key alone");
    assert_eq!(crouching(1.0, false), 0.0, "both keys");
    assert!((crouching(1.0, true) - 2.0).abs() < 0.01, "both keys with up_on_both");
}

/// A dodge profile with no jump chance (and a flier that alternates) draws the same
/// random numbers as before the jump key existed. The positions and the stream's place are
/// what the code played just before it, for the same seed.
#[test]
fn a_dodge_with_no_jump_chance_draws_what_it_always_drew() {
    let trace = |m: &Motion, start: Vec3| {
        let walls = arena();
        let mut rng = Rng::new(7);
        let mut s = BotState::new(m, &mut rng, start, Vec3::X);
        let mut p = start;
        let mut out = Vec::new();
        for i in 0..(5.0 / DT) as usize {
            s.step(m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
            if (i + 1) % 120 == 0 {
                out.push(p);
            }
        }
        (out, rng.next_u64())
    };
    let walker = Motion {
        dodges: vec![
            Dodge {
                strafe_s: [0.3, 0.8],
                strafe_pause_s: [0.0, 0.2],
                start_side: Start::Random,
                advance: true,
                advance_s: [0.4, 1.2],
                blocked: Some(Blocked { fraction: 0.5, reaction_s: [0.1, 0.3], on_touch: false }),
                ..Dodge::default()
            },
            Dodge { strafe_s: [0.5, 1.0], ..Dodge::default() },
        ],
        change_dodge_s: [1.0, 2.0],
        ..strafer()
    };
    let (at, next) = trace(&walker, Vec3::new(0.0, -1.5, -10.0));
    let pinned =
        [(0.5472234, -7.6509156), (-2.1024978, -7.7887573), (-0.21922867, -7.9428377), (-0.8611942, -6.6430774)];
    for (p, (x, z)) in at.iter().zip(pinned) {
        assert!((p.x - x).abs() < 1e-4 && (p.z - z).abs() < 1e-4 && p.y == -1.5, "{p}");
    }
    assert_eq!(next, 12369902262100240266);
    // A flier that alternates draws nothing for a jump chance it has no use for.
    let flyer = Motion {
        gravity: 0.0,
        flight: Some(Flight { up_speed: 2.0, down_speed: 3.0, up_accel: 20.0, down_accel: 30.0, up_on_both: false }),
        dodges: vec![Dodge {
            up_down_s: [0.3, 0.6],
            up_down_pause_s: [0.0, 0.2],
            jump_chance: 0.7,
            jump_s: [0.2, 0.4],
            ..Dodge::default()
        }],
        ..strafer()
    };
    assert_eq!(trace(&flyer, Vec3::new(0.0, 2.0, -10.0)).1, 5894705141355004150);
}
