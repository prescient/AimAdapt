//! Tests for the run simulation.

use super::*;
use crate::scenario::Scenario;
use std::collections::HashMap;

const FRAME: f32 = 1.0 / 240.0;

fn idle() -> Input {
    Input { aim: Vec3::NEG_Z, fire_pressed: false, fire_held: false }
}

fn shoot(at: Vec3) -> Input {
    Input { aim: at.normalize(), fire_pressed: true, fire_held: true }
}

fn no_countdown(mut s: Scenario) -> Scenario {
    s.timing.countdown_s = 0.0;
    s
}

#[test]
fn angles_round_trip() {
    for (y, p) in [(0.0, 0.0), (30.0, 10.0), (-120.0, -40.0)] {
        let (y2, p2) = angles(from_angles(y, p));
        assert!((y - y2).abs() < 1e-3 && (p - p2).abs() < 1e-3, "{y},{p} -> {y2},{p2}");
    }
    assert!(from_angles(0.0, 0.0).abs_diff_eq(Vec3::NEG_Z, 1e-6));
    assert!(from_angles(90.0, 0.0).abs_diff_eq(Vec3::X, 1e-6));
}

#[test]
fn sphere_and_capsule_hits() {
    let c = Vec3::new(0.0, 0.0, -10.0);
    assert!((ray_hit(Vec3::NEG_Z, c, 0.5, Shape::Sphere, 0.0).unwrap() - 9.5).abs() < 1e-4);
    assert!(ray_hit(from_angles(3.0, 0.0), c, 0.5, Shape::Sphere, 0.0).is_none());
    assert!(ray_hit(Vec3::Z, c, 0.5, Shape::Sphere, 0.0).is_none(), "behind the player");
    // A 1.8 m capsule is hit near its top but not above it.
    assert!(ray_hit(from_angles(0.0, 4.5), c, 0.3, Shape::Capsule, 1.8).is_some());
    assert!(ray_hit(from_angles(0.0, 6.0), c, 0.3, Shape::Capsule, 1.8).is_none());
}

#[test]
fn box_hits_its_faces() {
    // A box 0.7 m wide and deep, 1.8 m tall, centred 10 m ahead.
    let c = Vec3::new(0.0, 0.0, -10.0);
    let hit = |dir: Vec3| ray_hit(dir, c, 0.35, Shape::Box, 1.8);
    assert!((hit(Vec3::NEG_Z).unwrap() - 9.65).abs() < 1e-4, "front face");
    // Side faces: just inside and just outside (the front face edge is at x = 0.35).
    let at = |x: f32, y: f32| Vec3::new(x, y, -9.65).normalize();
    assert!(hit(at(0.34, 0.0)).is_some() && hit(at(-0.34, 0.0)).is_some());
    assert!(hit(at(0.36, 0.0)).is_none() && hit(at(-0.36, 0.0)).is_none());
    // Top and bottom faces (half height 0.9).
    assert!(hit(at(0.0, 0.89)).is_some() && hit(at(0.0, -0.89)).is_some());
    assert!(hit(at(0.0, 0.91)).is_none() && hit(at(0.0, -0.91)).is_none());
    // A corner: this ray passes the capsule's rounded top but clips the box's edge.
    let corner = at(0.3, 0.85);
    assert!(hit(corner).is_some(), "box takes the corner");
    assert!(ray_hit(corner, c, 0.35, Shape::Capsule, 1.8).is_none(), "capsule of the same size misses it");
    // Inside the box: 0. Behind the player: none.
    assert_eq!(ray_hit(Vec3::NEG_Z, Vec3::new(0.0, 0.0, -0.1), 0.35, Shape::Box, 1.8), Some(0.0));
    assert!(ray_hit(Vec3::Z, c, 0.35, Shape::Box, 1.8).is_none(), "behind the player");
}

#[test]
fn box_off_to_one_side_is_entered_through_its_side_face() {
    // Centre 5 m right and 4 m ahead, half width 1: its left face is the plane x = 4.
    let c = Vec3::new(5.0, 0.0, -4.0);
    let dir = Vec3::new(1.0, 0.0, -0.8).normalize();
    // x = 4 is reached at t = 4 / dir.x; z there is -3.2, inside the depth range -5..-3.
    let d = ray_hit(dir, c, 1.0, Shape::Box, 4.0).unwrap();
    assert!((d - 4.0 / dir.x).abs() < 1e-4, "{d}");
    // Aimed at the same face but too far forward, it passes in front of the box.
    assert!(ray_hit(Vec3::new(1.0, 0.0, -0.2).normalize(), c, 1.0, Shape::Box, 4.0).is_none());
}

#[test]
fn boxes_must_be_tall_enough_and_fit_the_room() {
    let mut flat = Scenario::default();
    flat.targets.shape = Shape::Box;
    flat.targets.radius = 0.5;
    flat.targets.height = 0.8;
    assert!(flat.problems().iter().any(|p| p.contains("at least 2")), "{:?}", flat.problems());
    let mut tall = Scenario::default();
    tall.targets.shape = Shape::Box;
    tall.targets.height = tall.arena.size[1] + 1.0;
    assert!(tall.problems().iter().any(|p| p.contains("fit between floor and ceiling")), "{:?}", tall.problems());
}

#[test]
fn countdown_blocks_shots() {
    let mut sim = Sim::new(Scenario::default(), 1);
    let at = sim.targets()[0].pos;
    sim.step(FRAME, shoot(at));
    assert_eq!(sim.stats().shots, 0);
    assert_eq!(sim.phase(), Phase::Countdown);
}

#[test]
fn clicking_a_target_kills_and_replaces_it() {
    let mut sim = Sim::new(no_countdown(Scenario::default()), 7);
    let first = sim.targets()[0].clone();
    let events = sim.step(FRAME, shoot(first.pos));
    assert!(events.contains(&Event::Killed { id: first.id }), "{events:?}");
    assert_eq!(sim.targets().len(), 1, "replacement spawned");
    assert_ne!(sim.targets()[0].id, first.id);
    assert_eq!(sim.stats().kills, 1);
    assert_eq!(sim.score(), 100.0);
}

#[test]
fn misses_count_and_holding_click_fires_once() {
    let mut s = no_countdown(Scenario::default());
    s.scoring.per_miss = -10.0;
    let mut sim = Sim::new(s, 3);
    let away = -sim.targets()[0].pos;
    sim.step(FRAME, shoot(away));
    for _ in 0..10 {
        sim.step(FRAME, Input { aim: away.normalize(), fire_pressed: false, fire_held: true });
    }
    assert_eq!(sim.stats().shots, 1);
    assert_eq!(sim.stats().misses, 1);
    assert_eq!(sim.score(), -10.0);
    assert_eq!(sim.stats().accuracy(), Some(0.0));
}

#[test]
fn hold_trigger_respects_fire_rate() {
    let mut s = no_countdown(Scenario::default());
    s.weapon.trigger = Trigger::Hold;
    s.weapon.fire_rate = 10.0;
    let mut sim = Sim::new(s, 3);
    let away = -sim.targets()[0].pos;
    for _ in 0..240 {
        sim.step(FRAME, Input { aim: away.normalize(), fire_pressed: false, fire_held: true });
    }
    assert!((9..=11).contains(&sim.stats().shots), "{} shots in 1 s at 10/s", sim.stats().shots);
}

/// Bug: the cooldown dropped the time left over in each frame and allowed one
/// shot per step, so the fire rate depended on frame rate: 9/s fired 8.57/s at
/// 60 fps, and 100/s fired 60/s.
#[test]
fn hold_fire_rate_is_the_same_at_any_frame_rate() {
    for (rate, fps) in [(9.0, 60.0), (100.0, 60.0), (13.0, 144.0), (20.0, 30.0)] {
        let mut s = no_countdown(Scenario::default());
        s.timing.duration_s = 100.0;
        s.weapon.trigger = Trigger::Hold;
        s.weapon.fire_rate = rate;
        let mut sim = Sim::new(s, 3);
        let away = (-sim.targets()[0].pos).normalize();
        let held = Input { aim: away, fire_pressed: false, fire_held: true };
        for _ in 0..(10.0 * fps) as usize {
            sim.step(1.0 / fps, held);
        }
        let want = rate * 10.0;
        let got = sim.stats().shots as f32;
        assert!((got - want).abs() <= 1.0, "{got} shots in 10 s at {rate}/s and {fps} fps");
        // Letting go doesn't bank shots for a burst on the next press.
        for _ in 0..fps as usize {
            sim.step(1.0 / fps, idle());
        }
        let before = sim.stats().shots;
        sim.step(1.0 / fps, held);
        assert_eq!(sim.stats().shots - before, 1, "at {rate}/s and {fps} fps");
    }
}

/// Bug: a target taller than the room (or grown by adaptive size) panicked in
/// `clamp` when it was placed.
#[test]
fn a_target_too_tall_for_the_room_is_placed_without_panicking() {
    let mut s = no_countdown(Scenario::default());
    s.arena.size = [40.0, 2.0, 40.0];
    s.arena.eye_height = 1.0;
    s.targets.radius = 1.5;
    let mut sim = Sim::new(s, 1);
    sim.step(FRAME, idle());
    assert!(sim.targets().iter().all(|t| t.pos.is_finite()));
}

#[test]
fn run_ends_on_time_and_kill_limit() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 1.0;
    let mut sim = Sim::new(s.clone(), 1);
    for _ in 0..300 {
        sim.step(FRAME, idle());
    }
    assert_eq!(sim.phase(), Phase::Finished);
    assert!((sim.result().duration_s - 1.0).abs() < 1e-3);

    s.timing.duration_s = 60.0;
    s.timing.kill_limit = 3;
    let mut sim = Sim::new(s, 1);
    while sim.phase() != Phase::Finished {
        let at = sim.targets()[0].pos;
        sim.step(FRAME, shoot(at));
        sim.step(FRAME, idle());
    }
    assert_eq!(sim.stats().kills, 3);
    assert_eq!(sim.result().kills.len(), 3);
}

#[test]
fn beam_on_target_is_full_accuracy() {
    let mut s = no_countdown(Scenario::default());
    s.weapon.kind = WeaponKind::Beam;
    s.weapon.trigger = Trigger::Always;
    s.targets.health = 1e9;
    s.scoring = crate::scenario::Scoring { per_kill: 0.0, per_damage: 1.0, ..Default::default() };
    let mut sim = Sim::new(s, 1);
    let at = sim.targets()[0].pos;
    for _ in 0..240 {
        sim.step(FRAME, Input { aim: at.normalize(), fire_pressed: false, fire_held: false });
    }
    let st = sim.stats();
    assert!((59..=61).contains(&st.shots), "{} ticks in 1 s", st.shots);
    assert_eq!(st.misses, 0);
    assert!((sim.score() - 100.0).abs() < 2.0, "1 s at 100 dps, got {}", sim.score());
}

/// Regression: on Float Track the ball sank through the floor. Its spawn area
/// reaches 10° below eye level at 10 m, which puts a 0.5 m ball's bottom under
/// the floor, and movement only bounced off the spawn area, not the room.
#[test]
fn no_target_ever_leaves_the_room() {
    for e in crate::library::builtins() {
        let mut s = e.scenario.unwrap();
        s.timing.countdown_s = 0.0;
        let a = s.arena.clone();
        let (half_w, half_d) = (a.size[0] / 2.0, a.size[2] / 2.0);
        let (floor, ceiling) = (-a.eye_height, a.size[1] - a.eye_height);
        for adaptive in [false, true] {
            let mut sim = if adaptive {
                Sim::adaptive(s.clone(), 5, None, crate::adaptive::Mix::Balanced)
            } else {
                Sim::new(s.clone(), 5)
            };
            while sim.phase() != Phase::Finished {
                sim.step(FRAME, idle());
                for t in sim.targets() {
                    let half_h = match s.targets.shape {
                        Shape::Sphere => t.radius,
                        Shape::Capsule | Shape::Box => {
                            (t.height * t.radius / t.base_radius.max(1e-6) / 2.0).max(t.radius)
                        }
                    };
                    let p = t.pos;
                    let ok = p.y - half_h >= floor - 1e-3
                        && p.y + half_h <= ceiling + 1e-3
                        && p.x.abs() + t.radius <= half_w + 1e-3
                        && p.z.abs() + t.radius <= half_d + 1e-3;
                    assert!(ok, "{}: target at {p:?} (r {}) left the room at {:.2} s", e.id, t.radius, sim.elapsed());
                }
            }
        }
    }
}

#[test]
fn moving_targets_stay_in_their_area() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 600.0;
    s.targets.count = 3;
    s.targets.movement = Movement { kind: MovementKind::Random, speed: Range(4.0, 8.0), ..Default::default() };
    let mut sim = Sim::new(s.clone(), 9);
    let start: Vec<f32> = sim.targets().iter().map(|t| t.pos.length()).collect();
    for _ in 0..(240 * 30) {
        sim.step(FRAME, idle());
    }
    for (t, d) in sim.targets().iter().zip(start) {
        let (yaw, pitch) = angles(t.pos);
        assert!(s.targets.spawn.yaw.contains(yaw) && s.targets.spawn.pitch.contains(pitch), "{yaw},{pitch}");
        assert!((t.pos.length() - d).abs() < 1e-3, "keeps its distance");
    }
}

#[test]
fn lifetime_times_out_and_shrinks() {
    let mut s = no_countdown(Scenario::default());
    s.targets.lifetime_s = 1.0;
    s.targets.shrink_to = 0.5;
    s.scoring.per_timeout = -50.0;
    let mut sim = Sim::new(s, 2);
    for _ in 0..120 {
        sim.step(FRAME, idle());
    }
    assert!((sim.targets()[0].radius - 0.225).abs() < 0.01, "half-way: {}", sim.targets()[0].radius);
    for _ in 0..121 {
        sim.step(FRAME, idle());
    }
    assert_eq!(sim.stats().timeouts, 1);
    assert_eq!(sim.score(), -50.0);
}

#[test]
fn spawns_keep_their_spacing() {
    let mut s = no_countdown(Scenario::default());
    s.targets.count = 5;
    s.targets.spawn.min_separation = 8.0;
    let sim = Sim::new(s, 11);
    let t = sim.targets();
    for a in 0..t.len() {
        for b in a + 1..t.len() {
            let angle = t[a].pos.normalize().angle_between(t[b].pos.normalize()).to_degrees();
            assert!(angle >= 8.0 - 1e-3, "{angle}");
        }
    }
}

#[test]
fn same_seed_same_run() {
    let run = |seed| {
        let mut sim = Sim::new(no_countdown(Scenario::default()), seed);
        let mut aim = Vec3::NEG_Z;
        for i in 0..2000 {
            if i % 50 == 0 {
                aim = sim.targets()[0].pos.normalize();
            }
            sim.step(FRAME, Input { aim, fire_pressed: i % 50 == 0, fire_held: false });
        }
        sim.result()
    };
    assert_eq!(run(5), run(5));
}

#[test]
fn adaptive_rating_speeds_up_for_hits_and_slows_for_timeouts() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 30.0;
    // A perfect player: kills each target the frame it appears.
    let mut sim = Sim::adaptive(s.clone(), 1, None, Mix::Balanced);
    let start = sim.rating().unwrap().1;
    while sim.phase() != Phase::Finished {
        let at = sim.targets()[0].pos;
        sim.step(FRAME, shoot(at));
        sim.step(FRAME, idle());
    }
    let r = sim.result();
    let a = r.adaptive.clone().unwrap();
    assert_eq!(a.knob, Knob::Lifetime);
    assert!(a.value < start * 0.5, "perfect play gets fast: {start} -> {}", a.value);
    assert_eq!(r.score, a.value, "the result is the rating");
    assert!(a.calibration() && a.pick == Pick::Calibrate, "no earlier rating: a calibration run");
    assert_eq!(a.fastest, None, "a calibration run sets no best");
    // Targets got their lifetimes from the rating, and record them.
    let t = &r.telemetry.targets[3];
    assert_eq!(t.value, Some(t.lifetime_s));
    assert!(t.chance.is_some() && t.pick.is_some());

    // A player who never shoots only sees timeouts and the rating slows down.
    let mut sim = Sim::adaptive(s, 1, None, Mix::Balanced);
    while sim.phase() != Phase::Finished {
        sim.step(FRAME, idle());
    }
    let a = sim.result().adaptive.unwrap();
    assert!(a.value > start && a.success_rate == Some(0.0), "{a:?}");
}

fn hitscan_strafer() -> Scenario {
    let mut s = no_countdown(Scenario::default());
    s.targets.lifetime_s = 1.5;
    s.targets.spawn.yaw = Range(-30.0, 30.0);
    s.targets.movement = Movement { kind: MovementKind::Strafe, ..Default::default() };
    s.adaptive.knob = Knob::TargetSpeed;
    s.timing.duration_s = 20.0;
    s
}

/// Found in the scoring review: an adaptive hitscan scenario set to target
/// speed recorded no outcomes (only beam ticks fed tracking segments), so its
/// rating never moved. Each target now gets its own speed, and a kill before
/// it times out is a success.
#[test]
fn hitscan_target_speed_is_rated_per_target() {
    let s = hitscan_strafer();
    assert!(s.problems().is_empty(), "{:?}", s.problems());
    let mut sim = Sim::adaptive(s.clone(), 1, None, Mix::Balanced);
    let start = sim.rating().unwrap().1;
    while sim.phase() != Phase::Finished {
        let at = sim.targets()[0].pos;
        sim.step(FRAME, shoot(at));
        sim.step(FRAME, idle());
    }
    let r = sim.result();
    let a = r.adaptive.unwrap();
    assert!(a.outcomes > 20, "one outcome per target: {}", a.outcomes);
    assert!(a.value > start, "perfect play rates faster: {start} -> {}", a.value);
    // Targets move at the speed they were given, and keep it.
    let t = r.telemetry.targets.iter().rev().find(|t| t.value.is_some()).unwrap();
    assert!(t.value.unwrap() > start);

    // Never shooting: every target times out and the rating slows.
    let mut sim = Sim::adaptive(s, 1, None, Mix::Balanced);
    while sim.phase() != Phase::Finished {
        sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
    }
    let a = sim.result().adaptive.unwrap();
    assert!(a.outcomes > 5 && a.value < start, "{a:?}");
}

#[test]
fn hitscan_target_speed_keeps_each_targets_speed() {
    let s = hitscan_strafer();
    let mut sim = Sim::adaptive(s, 2, None, Mix::Balanced);
    // Each target's position every quarter second, by spawn number.
    let mut path: HashMap<u32, Vec<Vec3>> = HashMap::new();
    let mut frame = 0;
    while sim.phase() != Phase::Finished {
        sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
        frame += 1;
        if frame % 60 == 0 {
            for t in sim.targets() {
                path.entry(t.no()).or_default().push(t.pos.normalize());
            }
        }
    }
    let r = sim.result();
    let mut checked = 0;
    for rec in &r.telemetry.targets {
        let (Some(value), Some(p)) = (rec.value, path.get(&rec.no)) else { continue };
        if p.len() < 4 {
            continue;
        }
        // Fastest quarter second: a window without a reversal moves at full speed.
        let peak = p.windows(2).map(|w| (w[1] - w[0]).length().to_degrees() * 4.0).fold(0.0f32, f32::max);
        assert!(peak > value * 0.75 && peak < value * 1.1, "target {} given {value:.1}, moved {peak:.1} deg/s", rec.no);
        checked += 1;
    }
    assert!(checked >= 5, "{checked} targets checked");
}

fn mover(mv: Movement) -> Scenario {
    let mut s = no_countdown(Scenario::default());
    s.weapon.kind = WeaponKind::Beam;
    s.weapon.trigger = Trigger::Always;
    s.targets.health = 1e9;
    s.targets.spawn.yaw = Range(-40.0, 40.0);
    s.targets.movement = mv;
    s.timing.duration_s = 30.0;
    s
}

fn look_away() -> Input {
    Input { aim: Vec3::Y, fire_pressed: false, fire_held: false }
}

/// Screen velocity in degrees per second.
fn screen(t: &Target) -> (f32, f32) {
    (t.rate[0] * t.pitch.to_radians().cos(), t.rate[1])
}

#[test]
fn acceleration_slows_down_and_speeds_back_up() {
    let accel = 20.0;
    let mv = Movement { kind: MovementKind::Strafe, speed: Range(5.0, 5.0), accel, ..Default::default() };
    let mut sim = Sim::new(mover(mv), 1);
    let d = sim.targets()[0].distance;
    let max_step = (accel / d).to_degrees() * FRAME * 1.05;
    let mut prev = screen(&sim.targets()[0]);
    let (mut slow, mut reversals) = (false, 0);
    for _ in 0..(240 * 10) {
        sim.step(FRAME, look_away());
        let t = &sim.targets()[0];
        let now = screen(t);
        let dv = ((now.0 - prev.0).powi(2) + (now.1 - prev.1).powi(2)).sqrt();
        // A bounce off the area's edge flips instantly; everything else is gradual.
        if now.0.signum() == prev.0.signum() || now.0.abs() < 1.0 {
            assert!(dv <= max_step, "velocity jumped {dv:.3} deg/s in a frame (limit {max_step:.3})");
        }
        if now.0.abs() < 0.5 {
            slow = true;
        }
        if now.0.signum() != prev.0.signum() && now.0 != 0.0 {
            reversals += 1;
        }
        prev = now;
    }
    assert!(slow && reversals >= 5, "slowed through reversals: {reversals}");
    // Full speed again between reversals.
    let full = (5.0 / d).to_degrees();
    assert!(prev.0.abs() <= full * 1.01);
}

#[test]
fn forward_back_movement_stays_in_range_and_keeps_speed() {
    let mut s = mover(Movement {
        kind: MovementKind::Strafe,
        speed: Range(4.0, 4.0),
        change_every_s: Range(3.0, 3.0),
        approach_speed: Range(3.0, 3.0),
        approach_every_s: Range(0.7, 1.2),
        ..Default::default()
    });
    s.targets.spawn.distance = Range(6.0, 14.0);
    assert!(s.problems().is_empty(), "{:?}", s.problems());
    let mut sim = Sim::new(s.clone(), 2);
    let (mut lo, mut hi) = (f32::MAX, 0.0f32);
    for _ in 0..(240 * 20) {
        sim.step(FRAME, look_away());
        let t = &sim.targets()[0];
        assert!(t.distance >= 6.0 - 1e-3 && t.distance <= 14.0 + 1e-3, "{}", t.distance);
        lo = lo.min(t.distance);
        hi = hi.max(t.distance);
        // Sideways speed in m/s stays put while the distance changes.
        let (x, _) = screen(t);
        let mps = x.abs().to_radians() * t.distance;
        assert!(mps < 4.0 * 1.02, "{mps:.2} m/s at {:.1} m", t.distance);
    }
    assert!(hi - lo > 3.0, "it moved forward and back: {lo:.1}..{hi:.1} m");
    // Without a distance range there's nowhere to move.
    s.targets.spawn.distance = Range(10.0, 10.0);
    assert!(s.problems().iter().any(|p| p.contains("distance range")));
}

#[test]
fn jumps_go_up_and_come_down_under_gravity() {
    let mut s = mover(Movement {
        kind: MovementKind::Strafe,
        jump_every_s: Range(0.5, 0.5),
        jump_height: 1.2,
        jump_air_s: 0.7,
        ..Default::default()
    });
    s.targets.spawn.pitch = Range(-8.0, -8.0);
    let mut sim = Sim::new(s, 3);
    let (mut peak, mut airborne, mut landings, mut air_frames) = (0.0f32, false, 0, 0);
    let mut airtimes = Vec::new();
    for _ in 0..(240 * 8) {
        sim.step(FRAME, look_away());
        let t = &sim.targets()[0];
        peak = peak.max(t.lift);
        if t.lift > 0.0 {
            airborne = true;
            air_frames += 1;
        } else if airborne {
            airborne = false;
            landings += 1;
            airtimes.push(air_frames as f32 * FRAME);
            air_frames = 0;
        }
    }
    assert!((peak - 1.2).abs() < 0.05, "peak {peak:.2} m");
    assert!(landings >= 5, "{landings} jumps");
    for a in airtimes {
        assert!((a - 0.7).abs() < 0.03, "airtime {a:.3} s");
    }
}

/// Bug: regen only worked with a beam, so imported hitscan scenarios with
/// `HealthRegenPerSec` (Pokeball Small) never healed.
#[test]
fn hitscan_targets_heal_after_the_delay() {
    let mut s = builtin("builtin:static-grid");
    s.targets.count = 1;
    s.targets.health = 3.0;
    s.weapon.damage = 1.0;
    s.targets.regen_per_s = 2.0;
    s.targets.regen_delay_s = 0.2;
    let mut sim = Sim::new(s, 1);
    let at = sim.targets()[0].pos.normalize();
    while sim.phase() == Phase::Countdown {
        sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
    }
    sim.step(FRAME, Input { aim: at, fire_pressed: true, fire_held: true });
    assert_eq!(sim.targets()[0].health, 2.0);
    let idle = Input { aim: Vec3::Y, fire_pressed: false, fire_held: false };
    for _ in 0..24 {
        sim.step(FRAME, idle);
    }
    assert_eq!(sim.targets()[0].health, 2.0, "no healing in the first 0.1 s");
    for _ in 0..240 {
        sim.step(FRAME, idle);
    }
    assert_eq!(sim.targets()[0].health, 3.0, "healed back to full");
}

/// An imported scenario: one target 10 m ahead on a floor 1.5 m below the eye,
/// with `walls` added.
fn on_a_floor(walls: Vec<crate::world::WorldMesh>, motion: Option<crate::motion::Motion>) -> Scenario {
    let quad = |p: [[f32; 3]; 4]| crate::world::WorldMesh {
        positions: p.to_vec(),
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        ..Default::default()
    };
    let mut meshes = vec![quad([[-30.0, -1.5, -30.0], [30.0, -1.5, -30.0], [30.0, -1.5, 5.0], [-30.0, -1.5, 5.0]])];
    meshes.extend(walls);
    let mut s = no_countdown(builtin("builtin:static-grid"));
    s.targets.count = 1;
    s.targets.radius = 0.5;
    s.targets.height = 1.0;
    s.world = Some(crate::world::World {
        spawns: vec![crate::world::WorldSpawn { pos: [0.0, -1.0, -10.0], ..Default::default() }],
        meshes,
        motion,
        ..Default::default()
    });
    s
}

#[test]
fn walls_stop_shots_but_clip_walls_dont() {
    let wall = |clip: bool| crate::world::WorldMesh {
        positions: vec![[-5.0, -2.0, -5.0], [5.0, -2.0, -5.0], [5.0, 5.0, -5.0], [-5.0, 5.0, -5.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        clip,
        ..Default::default()
    };
    for (clip, kills) in [(false, 0), (true, 1)] {
        let mut sim = Sim::new(on_a_floor(vec![wall(clip)], None), 1);
        let at = sim.targets()[0].pos.normalize();
        sim.step(FRAME, Input { aim: at, fire_pressed: true, fire_held: true });
        assert_eq!(sim.stats().kills, kills, "clip {clip}");
    }
}

/// A bot with a movement ability for "in combat" only sees the player past a clip
/// wall (shots go through it) but not past a solid one.
#[test]
fn a_solid_wall_blocks_a_bots_sight_but_a_clip_wall_doesnt() {
    use crate::motion::{Abilities, Ability, Motion};
    let wall = |clip: bool| crate::world::WorldMesh {
        positions: vec![[-5.0, -2.0, -5.0], [5.0, -2.0, -5.0], [5.0, 5.0, -5.0], [-5.0, 5.0, -5.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        clip,
        ..Default::default()
    };
    let dash = Ability { charge_s: 0.2, speed: 3.0, out_of_combat: false, ..Ability::default() };
    let m = Motion {
        abilities: Some(Abilities { press_every_s: 0.05, chance: 1.0, list: vec![dash], ..Default::default() }),
        ..Default::default()
    };
    for (clip, uses) in [(false, false), (true, true)] {
        let mut sim = Sim::new(on_a_floor(vec![wall(clip)], Some(m.clone())), 1);
        for _ in 0..480 {
            sim.step(FRAME, idle());
        }
        let used = sim.targets[0].bot.as_ref().unwrap().ability_uses() > 0;
        assert_eq!(used, uses, "clip wall {clip}");
    }
}

#[test]
fn imported_bots_move_on_the_map() {
    let strafe = crate::motion::Dodge { strafe_s: [0.5, 0.5], ..Default::default() };
    let m = crate::motion::Motion { max_speed: 4.0, accel: 40.0, dodges: vec![strafe], ..Default::default() };
    let mut sim = Sim::new(on_a_floor(Vec::new(), Some(m)), 1);
    let idle = Input { aim: Vec3::Y, fire_pressed: false, fire_held: false };
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for _ in 0..480 {
        sim.step(FRAME, idle);
        let p = sim.targets()[0].pos;
        lo = lo.min(p.x);
        hi = hi.max(p.x);
        assert!((p.y - -1.0).abs() < 0.02, "it stays on the floor: {p}");
    }
    assert!(hi - lo > 1.0, "it strafed: {lo}..{hi}");
}

/// Bug: VT Aether's and PGT's targets lose 100 health a second, so each goes
/// after 19 s and the next comes; without the drain the first stayed all run.
/// Negative regen drains a target until it goes, as a timeout (not a kill), and
/// it comes back after its respawn delay.
#[test]
fn negative_regen_drains_a_target_until_it_goes() {
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.health = 100.0;
    s.targets.regen_per_s = -50.0;
    s.targets.respawn_delay_s = 0.5;
    let mut sim = Sim::new(s, 1);
    let first = sim.targets()[0].id;
    // 1.9 s in: 5 health left.
    for _ in 0..456 {
        sim.step(FRAME, idle());
    }
    assert!((sim.targets()[0].health - 5.0).abs() < 1.0, "draining: {}", sim.targets()[0].health);
    for _ in 0..48 {
        sim.step(FRAME, idle());
    }
    assert!(sim.targets().is_empty(), "gone once drained");
    assert_eq!((sim.stats().timeouts, sim.stats().kills), (1, 0));
    for _ in 0..144 {
        sim.step(FRAME, idle());
    }
    assert!(sim.targets().iter().any(|t| t.id != first), "a new one after the respawn delay");
}

/// Bug: KovaaK's `InvincibleBots` (VT Snake Track, PGTI Voltaic) was ignored, so
/// the bot died and came back elsewhere. Invincible bots take hits, and their
/// damage counts for the score, but they never go down.
#[test]
fn invincible_bots_take_hits_but_never_die() {
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.health = 10.0;
    s.weapon.damage = 4.0;
    s.world.as_mut().unwrap().invincible = true;
    let mut sim = Sim::new(s, 1);
    let (first, at) = (sim.targets()[0].id, sim.targets()[0].pos.normalize());
    for _ in 0..20 {
        sim.step(FRAME, Input { aim: at, fire_pressed: true, fire_held: false });
        sim.step(FRAME, idle());
    }
    let st = sim.stats();
    assert!(st.hits >= 5 && st.kills == 0, "{st:?}");
    assert_eq!(st.damage_done, 4.0 * st.hits as f32);
    assert_eq!(sim.targets()[0].id, first);
    assert_eq!(sim.targets()[0].health, 10.0);
}

/// Bug: KovaaK's rotations with `Randomized=false` (VT Aether's three targets,
/// B180's) fill a slot with their bots in turn; they were picked at random.
/// Slots that name the same rotation take turns from it together (FlyTS).
#[test]
fn rotations_in_order_take_turns() {
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.count = 1;
    s.targets.regen_per_s = -1000.0;
    s.targets.respawn_delay_s = 0.0;
    let w = s.world.as_mut().unwrap();
    w.kinds =
        (0..3).map(|_| crate::world::WorldKind { health: 1.0, regen_per_s: -1000.0, ..Default::default() }).collect();
    w.slots = vec![vec![(0, 1.0), (1, 5.0), (2, 1.0)]];
    w.rotations = vec![Some(0)];
    let mut sim = Sim::new(s, 7);
    let mut seen = Vec::new();
    for _ in 0..200 {
        let kind = sim.targets().first().and_then(|t| t.kind());
        if kind.is_some() && seen.last() != Some(&kind) {
            seen.push(kind);
        }
        sim.step(FRAME, idle());
    }
    let seen: Vec<u32> = seen.into_iter().flatten().take(5).collect();
    assert_eq!(seen, vec![0, 1, 2, 0, 1]);
    // Two slots sharing one rotation start on its first two entries.
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.count = 2;
    let w = s.world.as_mut().unwrap();
    w.kinds = (0..2).map(|_| crate::world::WorldKind { health: 1.0, ..Default::default() }).collect();
    w.slots = vec![vec![(0, 1.0), (1, 1.0)], vec![(0, 1.0), (1, 1.0)]];
    w.rotations = vec![Some(0), Some(0)];
    let sim = Sim::new(s, 7);
    let mut kinds: Vec<Option<u32>> = sim.targets().iter().map(|t| t.kind()).collect();
    kinds.sort();
    assert_eq!(kinds, vec![Some(0), Some(1)]);
}

/// Bug: an adaptive run replaces its opening targets, but the rotations kept
/// the turns those first targets took, so the run started on the second bot.
#[test]
fn adaptive_runs_start_rotations_on_their_first_bot() {
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.count = 1;
    s.targets.lifetime_s = 1.0;
    let w = s.world.as_mut().unwrap();
    w.kinds = (0..3).map(|_| crate::world::WorldKind { health: 1.0, ..Default::default() }).collect();
    w.slots = vec![vec![(0, 1.0), (1, 1.0), (2, 1.0)]];
    w.rotations = vec![Some(0)];
    let prior = crate::adaptive::Prior {
        theta: 0.0,
        slope: 0.12,
        sd: 0.05,
        slope_sd: None,
        days: 0.0,
        fastest: None,
        last_run: None,
    };
    let sim = Sim::adaptive(s, 7, Some(prior), crate::adaptive::Mix::default());
    assert_eq!(sim.targets()[0].kind(), Some(0));
}

#[test]
fn regen_waits_for_its_delay() {
    let mut s = builtin("builtin:stability-switch");
    s.targets.count = 1;
    s.targets.movement.kind = MovementKind::Static;
    s.targets.regen_per_s = 100.0;
    s.targets.regen_delay_s = 0.5;
    let mut sim = Sim::new(s, 1);
    let at = sim.targets()[0].pos.normalize();
    let held = |aim| Input { aim, fire_pressed: false, fire_held: true };
    for _ in 0..120 {
        sim.step(FRAME, held(at));
    }
    let damaged = sim.targets()[0].health;
    for _ in 0..108 {
        sim.step(FRAME, held(Vec3::Y));
    }
    assert_eq!(sim.targets()[0].health, damaged, "no healing in the first 0.45 s");
    for _ in 0..60 {
        sim.step(FRAME, held(Vec3::Y));
    }
    assert!(sim.targets()[0].health > damaged, "healing after the delay");
}

/// Regen smoothness: with a beam, target lifetimes and the target-speed knob,
/// each target is a kill to finish in time. Fast regen after a short delay means
/// slipping off loses the progress, so unbroken tracking is what succeeds.
fn unbroken(slip_every: Option<f32>) -> AdaptiveSummary {
    let mut s = no_countdown(Scenario::default());
    s.weapon.kind = WeaponKind::Beam;
    s.weapon.trigger = Trigger::Hold;
    s.weapon.damage_per_s = 100.0;
    s.targets.health = 150.0;
    s.targets.regen_per_s = 400.0;
    s.targets.regen_delay_s = 0.1;
    s.targets.lifetime_s = 4.0;
    s.targets.movement = Movement { kind: MovementKind::Strafe, accel: 20.0, ..Default::default() };
    s.adaptive.knob = Knob::TargetSpeed;
    s.timing.duration_s = 30.0;
    assert!(s.problems().is_empty(), "{:?}", s.problems());
    let mut sim = Sim::adaptive(s, 5, None, Mix::Balanced);
    let mut t = 0.0;
    while sim.phase() != Phase::Finished {
        t += FRAME;
        let slipping = slip_every.is_some_and(|e| t % e > e - 0.3);
        let aim = if slipping { Vec3::Y } else { sim.targets()[0].pos.normalize() };
        sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: true });
    }
    sim.result().adaptive.unwrap()
}

#[test]
fn regen_smoothness_rewards_unbroken_tracking() {
    let steady = unbroken(None);
    assert!(steady.outcomes >= 10, "one outcome per target: {}", steady.outcomes);
    assert_eq!(steady.success_rate, Some(1.0));
    // Slipping off for 0.3 s every 1.2 s: the target heals about as fast as the beam hurts it, so no kills.
    let slips = unbroken(Some(1.2));
    assert_eq!(slips.success_rate, Some(0.0), "{slips:?}");
    assert!(slips.value < steady.value);
}

#[test]
fn new_movement_fields_dont_change_old_fingerprints() {
    let a = Scenario::default();
    let mut b = a.clone();
    b.targets.movement.accel = 10.0;
    assert_ne!(a.fingerprint(), b.fingerprint(), "using a new field changes it");
    assert!(a.to_toml().contains("accel"), "files still list every field");
}

#[test]
fn spraying_clicks_does_not_count_as_success() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 10.0;
    let mut sim = Sim::adaptive(s, 2, None, Mix::Balanced);
    while sim.phase() != Phase::Finished {
        let at = sim.targets()[0].pos;
        // Two misses, then the kill.
        sim.step(FRAME, shoot(-at));
        sim.step(FRAME, idle());
        sim.step(FRAME, shoot(-at));
        sim.step(FRAME, idle());
        sim.step(FRAME, shoot(at));
        sim.step(FRAME, idle());
    }
    let a = sim.result().adaptive.unwrap();
    assert_eq!(a.success_rate, Some(0.0));
}

#[test]
fn adaptive_tracking_rates_target_speed() {
    let mut s = no_countdown(Scenario::default());
    s.weapon.kind = WeaponKind::Beam;
    s.weapon.trigger = Trigger::Always;
    s.targets.health = 1e9;
    s.targets.spawn.yaw = Range(-40.0, 40.0);
    s.targets.movement = Movement { kind: MovementKind::Strafe, ..Default::default() };
    s.timing.duration_s = 20.0;
    let mut sim = Sim::adaptive(s, 3, None, Mix::Balanced);
    let start = sim.rating().unwrap().1;
    while sim.phase() != Phase::Finished {
        let at = sim.targets()[0].pos;
        sim.step(FRAME, Input { aim: at.normalize(), fire_pressed: false, fire_held: false });
    }
    let r = sim.result();
    let a = r.adaptive.unwrap();
    assert_eq!(a.knob, Knob::TargetSpeed);
    assert_eq!(a.success_rate, Some(1.0));
    assert!(a.value > start, "perfect tracking rates faster: {start} -> {}", a.value);
    assert!((9..=10).contains(&a.outcomes), "one outcome per 2 s segment: {}", a.outcomes);
    // Samples show the target really moving at the chosen speeds.
    let late: Vec<f32> = r.telemetry.samples.iter().filter(|x| x.t > 15.0).map(|x| x.target_speed_dps).collect();
    let mean = late.iter().sum::<f32>() / late.len() as f32;
    assert!(mean > start, "targets sped up: {mean} vs {start}");
}

fn prior_at(knob: Knob, value: f32, fastest: Option<f32>) -> Prior {
    Prior {
        theta: knob.h(value) + 0.12 * crate::adaptive::logit_with_slips(0.85),
        slope: 0.12,
        sd: 0.08,
        slope_sd: None,
        days: 1.0,
        fastest,
        last_run: Some((Pick::Near, 0.85)),
    }
}

/// Bug: a run stored the posterior mean of theta, so its stored curve (theta and slope,
/// which the next run and the Insights chart start from) did not pass through the rating.
#[test]
fn a_run_keeps_a_curve_through_its_rating() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 20.0;
    let mut sim = Sim::adaptive(s, 4, Some(prior_at(Knob::Lifetime, 0.8, None)), Mix::Balanced);
    let mut i = 0;
    while sim.phase() != Phase::Finished {
        // Kill every other target; let the rest time out.
        if let Some(t) = sim.targets().first()
            && i % 40 == 0
        {
            let at = t.pos;
            sim.step(FRAME, shoot(at));
        } else {
            sim.step(FRAME, idle());
        }
        i += 1;
    }
    let a = sim.result().adaptive.unwrap();
    let through = a.theta - a.slope * crate::adaptive::logit_with_slips(a.goal);
    assert!((a.knob.h(a.value) - through).abs() < 1e-4, "{} vs {through}", a.knob.h(a.value));
}

/// Planned order: five runs of one drill on Balanced play at level, at level,
/// stretch, at level, easy, not a random draw each. A block run with no prior is
/// still a calibration run.
#[test]
fn block_runs_follow_the_planned_order() {
    let s = no_countdown(Scenario::default());
    let prior = Prior { last_run: None, ..prior_at(Knob::Lifetime, 0.8, None) };
    let kinds: Vec<Pick> = (1..=5)
        .map(|rep| {
            Sim::adaptive_in_block(s.clone(), 4 + u64::from(rep), Some(prior), Mix::Balanced, rep, 5)
                .run_pick()
                .unwrap()
                .1
        })
        .collect();
    assert_eq!(kinds, [Pick::Near, Pick::Near, Pick::Stretch, Pick::Near, Pick::Easy]);
    for rep in 1..=5 {
        let sim = Sim::adaptive_in_block(s.clone(), 4, None, Mix::Balanced, rep, 5);
        assert_eq!(sim.run_pick(), None, "rep {rep}: no prior means calibration");
    }
    // A prior that needs calibration (a long break) is calibration too.
    let stale = Prior { days: 400.0, ..prior };
    assert!(stale.needs_calibration());
    assert_eq!(Sim::adaptive_in_block(s, 4, Some(stale), Mix::Balanced, 3, 5).run_pick(), None);
}

/// Playtest: adapting target by target inside a run felt chaotic. After the
/// calibration run, every target in a run gets the same lifetime.
#[test]
fn later_runs_play_at_one_difficulty() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 20.0;
    let mut sim = Sim::adaptive(s, 4, Some(prior_at(Knob::Lifetime, 0.8, None)), Mix::Balanced);
    let (value, pick) = sim.run_pick().expect("a fixed-difficulty run");
    assert_ne!(pick, Pick::Calibrate);
    let start = sim.rating().unwrap().1;
    let mut i = 0;
    while sim.phase() != Phase::Finished {
        // Kill every other target; let the rest time out.
        if let Some(t) = sim.targets().first()
            && i % 40 == 0
        {
            let at = t.pos;
            sim.step(FRAME, shoot(at));
        } else {
            sim.step(FRAME, idle());
        }
        i += 1;
    }
    let r = sim.result();
    let targets = &r.telemetry.targets;
    assert!(targets.len() > 10);
    for t in targets {
        assert!((t.lifetime_s - value).abs() < 1e-6, "every target at {value}: {}", t.lifetime_s);
        assert_eq!(t.value, Some(value));
        assert_eq!(t.pick, Some(pick.as_str()));
    }
    let a = r.adaptive.unwrap();
    assert_eq!(a.run_value, Some(value));
    assert!(!a.calibration());
    assert!(a.outcomes >= 10, "the rating took in the run: {}", a.outcomes);
    assert_ne!(a.value, start, "and moved");
}

/// The summary carries the run's evidence in target-equivalents, so the results
/// screen judges a run against its plan with the same measure the rating uses.
#[test]
fn the_summary_reports_evidence_like_the_rating() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 20.0;
    let mut sim = Sim::adaptive(s, 4, Some(prior_at(Knob::Lifetime, 0.8, None)), Mix::Balanced);
    let mut i = 0;
    while sim.phase() != Phase::Finished {
        if let Some(t) = sim.targets().first()
            && i % 40 == 0
        {
            let at = t.pos;
            sim.step(FRAME, shoot(at));
        } else {
            sim.step(FRAME, idle());
        }
        i += 1;
    }
    let a = sim.result().adaptive.unwrap();
    assert!(a.outcomes >= 10);
    assert_eq!(a.evidence, a.outcomes as f32, "a clicking target is one target-equivalent");
}

/// After a long break the old rating is a poor guess, so the next run is a
/// calibration run again: it starts from the old rating and adapts target by
/// target, and the result says how long the break was.
#[test]
fn a_run_after_a_long_break_is_a_calibration_run() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 20.0;
    let old = 0.8;
    let prior = Prior { days: 10.0, ..prior_at(Knob::Lifetime, old, Some(3.0)) };
    let mut sim = Sim::adaptive(s.clone(), 4, Some(prior), Mix::Balanced);
    assert_eq!(sim.run_pick(), None, "each target gets its own value");
    assert!((sim.break_days().unwrap() - 10.0).abs() < 1e-6);
    let start = sim.rating().unwrap().1;
    assert!((start / old - 1.0).abs() < 0.1, "starts from the old rating: {start}");
    let first = sim.result().telemetry.targets[0].lifetime_s;
    assert!(first / old > 0.5 && first / old < 2.0, "first target near the old rating: {first}");
    let mut i = 0;
    while sim.phase() != Phase::Finished {
        if let Some(t) = sim.targets().first()
            && i % 40 == 0
        {
            let at = t.pos;
            sim.step(FRAME, shoot(at));
        } else {
            sim.step(FRAME, idle());
        }
        i += 1;
    }
    let a = sim.result().adaptive.unwrap();
    assert!(a.calibration());
    assert_eq!(a.pick, Pick::Calibrate);
    assert!((a.break_days.unwrap() - 10.0).abs() < 1e-6);
    assert!(!a.breakthrough);

    // A day away from the same rating still plays at one value, with no break.
    let sim = Sim::adaptive(s, 4, Some(Prior { days: 1.0, ..prior }), Mix::Balanced);
    assert!(sim.run_pick().is_some());
    assert_eq!(sim.break_days(), None);
    assert_eq!(sim.result().adaptive.unwrap().break_days, None);
}

#[test]
fn later_tracking_runs_keep_one_speed() {
    let mut s = no_countdown(Scenario::default());
    s.weapon.kind = WeaponKind::Beam;
    s.weapon.trigger = Trigger::Always;
    s.targets.health = 1e9;
    s.targets.spawn.yaw = Range(-40.0, 40.0);
    s.targets.movement = Movement { kind: MovementKind::Strafe, ..Default::default() };
    s.timing.duration_s = 20.0;
    let mut sim = Sim::adaptive(s, 3, Some(prior_at(Knob::TargetSpeed, 30.0, None)), Mix::Balanced);
    let (value, _) = sim.run_pick().unwrap();
    while sim.phase() != Phase::Finished {
        let at = sim.targets()[0].pos;
        sim.step(FRAME, Input { aim: at.normalize(), fire_pressed: false, fire_held: false });
    }
    let r = sim.result();
    let a = r.adaptive.unwrap();
    assert!((9..=10).contains(&a.outcomes), "still one outcome per segment: {}", a.outcomes);
    let speeds: Vec<f32> = r.telemetry.samples.iter().map(|x| x.target_speed_dps).filter(|v| *v > 0.5).collect();
    let mean = speeds.iter().sum::<f32>() / speeds.len() as f32;
    assert!((mean / value - 1.0).abs() < 0.2, "targets move at the run's {value} deg/s: {mean}");
    assert!(a.value > 30.0, "perfect tracking raises the rating");
}

/// A breakthrough is holding the goal for a whole run at a value faster than
/// your best, and is announced when the run ends.
#[test]
fn breakthroughs_come_at_the_end_of_a_run() {
    let mut s = no_countdown(Scenario::default());
    s.timing.duration_s = 20.0;
    let mut sim = Sim::adaptive(s, 6, Some(prior_at(Knob::Lifetime, 0.8, Some(3.0))), Mix::Balanced);
    let (value, _) = sim.run_pick().unwrap();
    let mut events = Vec::new();
    while sim.phase() != Phase::Finished {
        let at = sim.targets()[0].pos;
        events.push(sim.step(FRAME, shoot(at)));
        events.push(sim.step(FRAME, idle()));
    }
    let at: Vec<usize> =
        (0..events.len()).filter(|&i| events[i].iter().any(|e| matches!(e, Event::Breakthrough { .. }))).collect();
    assert_eq!(at.len(), 1, "one breakthrough");
    assert!(events[at[0]].contains(&Event::Finished), "on the last step");
    let a = sim.result().adaptive.unwrap();
    assert!(a.breakthrough);
    assert_eq!(a.fastest, Some(value));
}

fn speed_switch() -> Scenario {
    let mut s = builtin("builtin:speed-switch");
    s.timing.duration_s = 30.0;
    s
}

/// Plays a player who kills one target every `gap` seconds: looks away (up),
/// then beams the first target until it dies. Returns the result.
fn kill_every(mut sim: Sim, gap: f32) -> RunResult {
    let mut last = 0.0;
    while sim.phase() != Phase::Finished {
        let kills = sim.stats().kills;
        let input = if sim.elapsed() - last >= gap - 0.15 {
            let at = sim.targets()[0].pos;
            Input { aim: at.normalize(), fire_pressed: false, fire_held: true }
        } else {
            Input { aim: Vec3::Y, fire_pressed: false, fire_held: false }
        };
        sim.step(FRAME, input);
        if sim.stats().kills > kills {
            last = sim.elapsed();
        }
    }
    sim.result()
}

/// Playtest: Speed Switch's rating was mostly each target's own timer, which
/// runs while you're busy with the others. It's now rated on the pace between
/// kills, and its targets never time out.
#[test]
fn speed_switch_is_rated_on_kill_pace() {
    let s = speed_switch();
    assert_eq!(s.targets.lifetime_s, 0.0, "no per-target timers");
    assert_eq!(s.adaptive.resolve(&s).knob, Knob::KillPace);
    let prior = prior_at(Knob::KillPace, 1.0, None);
    let mut sim = Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced);
    let (value, _) = sim.run_pick().unwrap();
    assert!(sim.pace().is_some());
    sim.step(FRAME, idle());
    assert!(sim.pace().unwrap().1 > 0.0);

    // Comfortably inside the pace: every kill is a success.
    let fast = kill_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 0.6);
    let a = fast.adaptive.unwrap();
    assert_eq!(a.success_rate, Some(1.0), "{a:?}");
    assert!(fast.stats.kills as usize >= a.outcomes && a.outcomes > 20, "one outcome per kill: {}", a.outcomes);
    assert_eq!(fast.stats.timeouts, 0);

    // Slower than the pace: every window runs out, and the late kill isn't a
    // success either.
    let slow = kill_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 1.7);
    let a = slow.adaptive.unwrap();
    assert_eq!(a.success_rate, Some(0.0), "{a:?}");
    // Kills 1.7 paces apart: each gap holds one full empty window.
    let windows = (30.0 / (value * 1.7)) as usize;
    assert!(a.outcomes.abs_diff(windows) <= 2, "one failure per empty window: {} vs {windows}", a.outcomes);
    assert!(a.value > 1.0, "the rating slows down: {}", a.value);
}

/// Playtest: a workout showed a 1320 ms time to kill at 132 kills a minute.
/// With several targets up, each one was timed from when it appeared, so it
/// counted the time spent killing the others. TTK starts when you could start
/// on the target: when it appears or at the previous kill, whichever is later.
#[test]
fn time_to_kill_doesnt_count_time_spent_on_other_targets() {
    let s = speed_switch();
    assert!(s.targets.count >= 3);
    let r = kill_every(Sim::new(s, 2), 0.45);
    let per_min = r.stats.kills as f32 * 2.0;
    assert!(per_min > 110.0, "{per_min} kills a minute");
    let ttk = r.avg_ttk_s().unwrap();
    assert!(ttk < 0.5, "a kill every 0.45 s can't average {ttk:.2} s to kill");
    // One target at a time: unchanged, from spawn to kill.
    let mut one = speed_switch();
    one.targets.count = 1;
    let r = kill_every(Sim::new(one, 2), 0.45);
    assert!((r.avg_ttk_s().unwrap() - 0.45).abs() < 0.05, "{:?}", r.avg_ttk_s());
}

#[test]
fn kill_pace_calibration_finds_the_players_pace() {
    // A player who kills every 0.7 s, against a first guess of 1.5 s.
    let r = kill_every(Sim::adaptive(speed_switch(), 3, None, Mix::Balanced), 0.7);
    let a = r.adaptive.unwrap();
    assert!(a.calibration());
    assert!(a.value < 1.0 && a.value > 0.5, "rated near 0.7 s: {}", a.value);
}

/// Plays a clicker who shoots the oldest target (the first to time out)
/// `gap` seconds after the last kill or timeout, after `misses` deliberate misses.
fn click_every(mut sim: Sim, gap: f32, misses: u32) -> RunResult {
    let mut last = 0.0;
    while sim.phase() != Phase::Finished {
        let (kills, timeouts) = (sim.stats().kills, sim.stats().timeouts);
        let oldest = |sim: &Sim| sim.targets().iter().max_by(|a, b| a.age.total_cmp(&b.age)).map(|t| t.pos);
        if sim.elapsed() - last >= gap
            && let Some(at) = oldest(&sim)
        {
            for _ in 0..misses {
                sim.step(FRAME, shoot(-at));
                sim.step(FRAME, idle());
            }
            let at = oldest(&sim).unwrap_or(at);
            sim.step(FRAME, shoot(at));
        } else {
            sim.step(FRAME, idle());
        }
        if sim.stats().kills > kills || sim.stats().timeouts > timeouts {
            last = sim.elapsed();
        }
    }
    sim.result()
}

fn short(id: &str) -> Scenario {
    let mut s = builtin(id);
    s.timing.duration_s = 30.0;
    s
}

/// Scoring review, findings 1 and 2: in adaptive mode the untimed clicking
/// drills got per-target timers and became copies of their timed versions, and
/// with several targets up a lifetime rating measured the queue. They're rated
/// on the pace between kills now.
#[test]
fn untimed_and_grid_clicking_is_rated_on_kill_pace() {
    for id in ["builtin:static-grid", "builtin:timed-grid", "builtin:wide-flicks"] {
        let s = short(id);
        assert_eq!(s.adaptive.resolve(&s).knob, Knob::KillPace, "{id}");
        let prior = prior_at(Knob::KillPace, 1.0, None);
        let value = Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced).run_pick().unwrap().0;
        let fast = click_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 0.6, 0);
        let a = fast.adaptive.unwrap();
        assert_eq!(a.success_rate, Some(1.0), "{id}: {a:?}");
        assert!(a.outcomes > 15, "{id}: one outcome per kill: {}", a.outcomes);
        let slow = click_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 1.7, 0);
        assert_eq!(slow.adaptive.unwrap().success_rate, Some(0.0), "{id}");
    }
    // Untimed stays untimed, and Timed Grid keeps its own 2 s timers.
    assert_eq!(short("builtin:static-grid").targets.lifetime_s, 0.0);
    let tg = short("builtin:timed-grid");
    let sim = Sim::adaptive(tg.clone(), 2, Some(prior_at(Knob::KillPace, 1.0, None)), Mix::Balanced);
    assert_eq!(sim.current().targets.lifetime_s, tg.targets.lifetime_s);
}

/// Kill pace on a clicking drill: a kill after a spray of misses isn't a
/// success, as with the lifetime knob.
#[test]
fn spraying_clicks_doesnt_count_toward_kill_pace() {
    let s = short("builtin:static-grid");
    let prior = prior_at(Knob::KillPace, 1.0, None);
    let value = Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced).run_pick().unwrap().0;
    let one = click_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 0.5, 1);
    assert_eq!(one.adaptive.unwrap().success_rate, Some(1.0), "one miss is allowed");
    let spray = click_every(Sim::adaptive(s, 2, Some(prior), Mix::Balanced), value * 0.5, 2);
    assert_eq!(spray.adaptive.unwrap().success_rate, Some(0.0));
}

/// Scoring review, findings 3 and 4: Micro Flicks' lifetime rating was mostly
/// reaction time, and Precise Track's speed rating turned it into reactive
/// tracking as you improved. Both are rated on target size now, with time and
/// speed held fixed.
#[test]
fn precision_drills_are_rated_on_size() {
    let micro = short("builtin:micro-flicks");
    assert_eq!(micro.adaptive.resolve(&micro).knob, Knob::Radius);
    let prior = prior_at(Knob::Radius, 0.1, None);
    let sim = Sim::adaptive(micro.clone(), 3, Some(prior), Mix::Balanced);
    let value = sim.run_pick().unwrap().0;
    assert!(sim.targets().iter().all(|t| (t.radius - value).abs() < 1e-6), "targets at the run's size");
    assert!(sim.targets().iter().all(|t| t.lifetime == 1.5), "a fixed, generous lifetime");
    // Hit in time: success. Too slow: each timeout is a failure.
    let quick = click_every(Sim::adaptive(micro.clone(), 3, Some(prior), Mix::Balanced), 0.4, 0);
    let a = quick.adaptive.unwrap();
    assert_eq!(a.success_rate, Some(1.0), "{a:?}");
    assert!(a.outcomes > 30);
    let late = click_every(Sim::adaptive(micro, 3, Some(prior), Mix::Balanced), 2.0, 0);
    assert_eq!(late.adaptive.unwrap().success_rate, Some(0.0));

    let precise = short("builtin:precise-track");
    assert_eq!(precise.adaptive.resolve(&precise).knob, Knob::Radius);
    let mut sim = Sim::adaptive(precise.clone(), 3, Some(prior_at(Knob::Radius, 0.15, None)), Mix::Balanced);
    let value = sim.run_pick().unwrap().0;
    assert_eq!(sim.current().targets.movement.speed, precise.targets.movement.speed, "speed stays slow");
    while sim.phase() != Phase::Finished {
        assert!((sim.targets()[0].radius - value).abs() < 1e-6);
        let aim = sim.targets()[0].pos.normalize();
        sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: false });
    }
    let a = sim.result().adaptive.unwrap();
    assert_eq!(a.success_rate, Some(1.0));
    assert!(a.outcomes >= 14, "one outcome per 2 s segment: {}", a.outcomes);
    // Calibration shrinks the ball segment by segment for a steady tracker.
    let mut cal = Sim::adaptive(precise.clone(), 3, None, Mix::Balanced);
    let mut sizes = Vec::new();
    while cal.phase() != Phase::Finished {
        sizes.push(cal.targets()[0].radius);
        let aim = cal.targets()[0].pos.normalize();
        cal.step(FRAME, Input { aim, fire_pressed: false, fire_held: false });
    }
    sizes.dedup();
    assert!(sizes.len() >= 5, "{sizes:?}");
    assert!(sizes.last().unwrap() < &precise.targets.radius, "{sizes:?}");
}

/// Scoring review, finding 6's follow-up: Strafe Clicks was a copy of Timed
/// Strafe Clicks in adaptive mode. It's rated on strafe speed now, with a fixed
/// lifetime, so the pair covers speed and time separately.
#[test]
fn strafe_clicks_is_rated_on_target_speed() {
    let s = short("builtin:strafe-clicks");
    assert_eq!(s.adaptive.resolve(&s).knob, Knob::TargetSpeed);
    assert!(s.problems().is_empty(), "{:?}", s.problems());
    let prior = prior_at(Knob::TargetSpeed, 20.0, None);
    let sim = Sim::adaptive(s.clone(), 4, Some(prior), Mix::Balanced);
    assert!(sim.rated_per_target());
    assert!(sim.targets().iter().all(|t| t.lifetime == s.targets.lifetime_s));
    let quick = click_every(Sim::adaptive(s.clone(), 4, Some(prior), Mix::Balanced), 0.3, 0);
    assert_eq!(quick.adaptive.unwrap().success_rate, Some(1.0));
    let late = click_every(Sim::adaptive(s, 4, Some(prior), Mix::Balanced), 2.5, 0);
    assert_eq!(late.adaptive.unwrap().success_rate, Some(0.0));
}

/// Scoring review, finding 5: Control Track promised smooth arcs but moved in
/// straight lines with sharp turns. Curve movement keeps a steady speed while
/// the heading turns a little every frame.
#[test]
fn curve_movement_turns_smoothly_at_constant_speed() {
    let control = builtin("builtin:control-track");
    assert_eq!(control.targets.movement.kind, MovementKind::Curve);
    let mv = control.targets.movement.clone();
    let mut s = mover(mv.clone());
    s.targets.spawn = control.targets.spawn.clone();
    let mut sim = Sim::new(s, 5);
    let heading = |t: &Target| {
        let (x, y) = screen(t);
        (y.atan2(x).to_degrees(), (x * x + y * y).sqrt())
    };
    let (mut prev, speed0) = heading(&sim.targets()[0]);
    let max_step = mv.turn_deg_s.1 * FRAME * 1.05;
    let (mut turned, mut smooth, mut frames) = (0.0f32, 0, 0);
    for _ in 0..(240 * 20) {
        let before = sim.targets()[0].rate;
        sim.step(FRAME, look_away());
        let t = &sim.targets()[0];
        let (h, speed) = heading(t);
        assert!((speed / speed0 - 1.0).abs() < 0.08, "speed {speed:.1} vs {speed0:.1} °/s");
        let step = (h - prev + 540.0).rem_euclid(360.0) - 180.0;
        // Bounces off the area's edge flip a component; everything else is a small turn.
        let bounced = t.rate[0].signum() != before[0].signum() || t.rate[1].signum() != before[1].signum();
        if !bounced {
            assert!(step.abs() <= max_step, "heading jumped {step:.2}° in a frame");
            turned += step.abs();
            smooth += 1;
        }
        frames += 1;
        prev = h;
    }
    assert!(smooth > frames * 9 / 10, "mostly smooth: {smooth} of {frames}");
    assert!(turned > 360.0, "it keeps curving: {turned:.0}°");
}

/// Median width of a strafe: degrees of yaw between one reversal and the next,
/// following the first target (standard mode, nobody shooting).
fn strafe_sweep(s: &Scenario, seed: u64) -> f32 {
    let mut s = s.clone();
    s.timing.duration_s = 40.0;
    let mut sim = Sim::new(s, seed);
    let (mut id, mut last_turn, mut dir) = (0, None::<f32>, 0.0f32);
    let mut sweeps = Vec::new();
    while sim.phase() != Phase::Finished {
        sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
        let Some(t) = sim.targets().first() else { continue };
        if t.id != id {
            (id, last_turn, dir) = (t.id, None, 0.0);
        }
        let d = t.rate[0].signum();
        if t.rate[0].abs() > 1e-3 && d != dir {
            if dir != 0.0 {
                if let Some(y) = last_turn {
                    sweeps.push((t.yaw - y).abs());
                }
                last_turn = Some(t.yaw);
            }
            dir = d;
        }
    }
    sweeps.sort_by(f32::total_cmp);
    sweeps[sweeps.len() / 2]
}

/// Playtest: Hop Strafe Track barely moved side to side. It reversed every
/// 0.35–1 s but took 0.4 s to turn around, so most of each strafe was spent
/// turning: its strafes were 10.7° wide against Strafe Track's 15.4°. Drills
/// with gradual turns must sweep at least as wide as the plain version.
#[test]
fn gradual_turns_still_sweep_as_wide_as_plain_strafes() {
    let sweep =
        |id: &str| (1..=3).map(|seed| strafe_sweep(&builtin(&format!("builtin:{id}")), seed)).sum::<f32>() / 3.0;
    let plain_track = sweep("strafe-track");
    let plain_clicks = sweep("timed-strafe-clicks");
    for (id, plain) in
        [("hop-strafe-track", plain_track), ("unbroken-strafes", plain_track), ("hop-clicks", plain_clicks)]
    {
        let w = sweep(id);
        assert!(w >= plain * 0.98, "{id} strafes {w:.1}° wide, the plain version {plain:.1}°");
    }
    let w = sweep("hop-switch");
    assert!(w >= 12.0, "hop-switch strafes {w:.1}° wide");
}

fn builtin(id: &str) -> Scenario {
    let e = crate::library::builtins().into_iter().find(|e| e.id == id).unwrap();
    no_countdown(e.scenario.unwrap())
}

/// Tracks the first target perfectly until it dies, then looks away for
/// `travel` seconds before starting on the next one.
fn track_with_travel(mut sim: Sim, travel: f32) -> RunResult {
    let mut away_until = 0.0;
    while sim.phase() != Phase::Finished {
        let kills = sim.stats().kills;
        let aim = if sim.elapsed() < away_until { Vec3::Y } else { sim.targets()[0].pos.normalize() };
        sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: true });
        if sim.stats().kills > kills {
            away_until = sim.elapsed() + travel;
        }
    }
    sim.result()
}

/// Playtest: Stability Switch's rating was mostly the trip to the next target
/// after each kill, which counted as off-target time. Now only time while a
/// target is engaged (damaged, not yet healed) counts.
#[test]
fn stability_switch_ignores_travel_between_targets() {
    let mut s = builtin("builtin:stability-switch");
    s.timing.duration_s = 30.0;
    assert!(s.adaptive.engaged_only && s.targets.regen_per_s > 0.0 && s.targets.health_bar);
    let prior = prior_at(Knob::TargetSpeed, 12.0, None);
    let play = |s: &Scenario, travel| {
        let r = track_with_travel(Sim::adaptive(s.clone(), 4, Some(prior), Mix::Steady), travel);
        r.adaptive.unwrap().success_rate.unwrap()
    };
    let quick = play(&s, 0.0);
    let slow = play(&s, 0.6);
    assert!(quick > 0.97 && slow > 0.97, "travel doesn't count: {quick} vs {slow}");
    // The old scoring, for contrast: every trip is off-target time.
    let mut old = s.clone();
    old.adaptive.engaged_only = false;
    let slow_old = play(&old, 0.6);
    assert!(slow_old < 0.85, "travel used to count: {slow_old}");
}

/// Playtest: Stability Switch targets didn't move enough and were too big to
/// make staying on them hard. Each sweep (one direction, between reversals)
/// should average at least 15 degrees, on targets at most 3.5 degrees across.
#[test]
fn stability_switch_sweeps_wide_on_small_targets() {
    let mut s = builtin("builtin:stability-switch");
    s.targets.health = 1e9;
    s.timing.duration_s = 30.0;
    let d = (s.targets.spawn.distance.0 + s.targets.spawn.distance.1) / 2.0;
    let size = 2.0 * (s.targets.radius / d).atan().to_degrees();
    assert!(size <= 3.5, "targets are {size:.1} deg across");
    let mut sim = Sim::new(s, 3);
    let yaw = |t: &Target| t.pos.x.atan2(-t.pos.z).to_degrees();
    let mut last: HashMap<u64, (f32, f32)> = HashMap::new(); // yaw, direction
    let mut sweeps = Vec::new();
    let mut current: HashMap<u64, f32> = HashMap::new();
    while sim.phase() != Phase::Finished {
        sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
        for t in sim.targets() {
            let y = yaw(t);
            if let Some(&(prev, dir)) = last.get(&t.id) {
                let dy = y - prev;
                let now_dir = dy.signum();
                if dy.abs() > 1e-4 && dir != 0.0 && now_dir != dir {
                    sweeps.push(current.insert(t.id, 0.0).unwrap_or(0.0));
                }
                *current.entry(t.id).or_default() += dy.abs();
                last.insert(t.id, (y, if dy.abs() > 1e-4 { now_dir } else { dir }));
            } else {
                last.insert(t.id, (y, 0.0));
            }
        }
    }
    let mean = sweeps.iter().sum::<f32>() / sweeps.len().max(1) as f32;
    assert!(sweeps.len() >= 20, "{} sweeps", sweeps.len());
    assert!(mean >= 15.0, "average sweep {mean:.1} deg");
}

#[test]
fn leaving_a_damaged_target_counts_until_it_heals() {
    let mut s = builtin("builtin:stability-switch");
    s.targets.count = 1;
    s.targets.movement.kind = MovementKind::Static;
    s.timing.duration_s = 10.0;
    let mut sim = Sim::adaptive(s.clone(), 1, Some(prior_at(Knob::TargetSpeed, 12.0, None)), Mix::Steady);
    let at = sim.targets()[0].pos.normalize();
    let held = |aim| Input { aim, fire_pressed: false, fire_held: true };
    // Half a second on target, then look away.
    for _ in 0..120 {
        sim.step(FRAME, held(at));
    }
    assert!(sim.targets()[0].engaged());
    let mut engaged_away = 0;
    for _ in 0..(240 * 4) {
        sim.step(FRAME, held(Vec3::Y));
        engaged_away += usize::from(sim.targets()[0].engaged());
    }
    assert_eq!(sim.targets()[0].health_fraction(), 1.0, "healed back to full");
    let healed_after = engaged_away as f32 * FRAME;
    // 50 damage at 40/s regen: 1.25 s to heal.
    assert!((healed_after - 1.25).abs() < 0.05, "engaged while healing: {healed_after}");
    let r = sim.result();
    let engaged: Vec<_> = r.telemetry.samples.iter().filter(|x| x.engaged).collect();
    let on = engaged.iter().filter(|x| x.on_target).count() as f32 / engaged.len() as f32;
    assert!((on - 0.5 / 1.75).abs() < 0.05, "half a second on, 1.25 s healing off: {on}");
}

#[test]
fn flick_telemetry_measures_reaction_reach_and_overshoot() {
    let mut s = no_countdown(Scenario::default());
    s.targets.spawn.yaw = Range(20.0, 20.0);
    s.targets.spawn.pitch = Range(0.0, 0.0);
    let mut sim = Sim::new(s, 1);
    // Wait 0.2 s, then sweep right at 100°/s past the target, then come back and shoot.
    let mut yaw = 0.0f32;
    for i in 0..240 {
        if i >= 48 {
            yaw = (yaw + 100.0 * FRAME).min(24.0);
        }
        sim.step(FRAME, Input { aim: from_angles(yaw, 0.0), fire_pressed: false, fire_held: false });
    }
    sim.step(FRAME, Input { aim: from_angles(20.5, 0.0), fire_pressed: true, fire_held: true });
    let r = sim.result();
    let t = &r.telemetry.targets[0];
    assert_eq!(t.outcome, Outcome::Killed);
    assert!((t.distance_deg - 20.0).abs() < 0.01 && t.direction_deg.abs() < 0.5, "{t:?}");
    let reaction = t.reaction_s.unwrap();
    assert!((0.2..0.24).contains(&reaction), "moved 2° after 0.2 s wait: {reaction}");
    let reach = t.reach_s.unwrap();
    assert!((0.36..0.40).contains(&reach), "{reach}");
    assert!((t.overshoot_deg - (4.0 - t.radius_deg)).abs() < 0.05, "{t:?}");
    assert_eq!(t.entries, 2, "went past and came back");
    let shot = &r.telemetry.shots[0];
    assert!(shot.hit && shot.along > 0.0 && shot.across.abs() < 0.01, "{shot:?}");
}

#[test]
fn replay_frames_follow_every_target() {
    let mut s = no_countdown(Scenario::default());
    s.targets.count = 2;
    s.targets.movement = Movement { kind: MovementKind::Strafe, ..Default::default() };
    let mut sim = Sim::new(s, 7);
    for _ in 0..240 {
        sim.step(FRAME, idle());
    }
    // Shoot one target dead, then keep going.
    let (no, at) = (sim.targets()[0].no(), sim.targets()[0].pos);
    while sim.targets().iter().any(|t| t.no() == no) {
        sim.step(FRAME, shoot(sim.targets().iter().find(|t| t.no() == no).map_or(at, |t| t.pos)));
    }
    let killed_at = sim.elapsed();
    for _ in 0..60 {
        sim.step(FRAME, idle());
    }
    let frames = sim.result().telemetry.frames;
    let span = frames.last().unwrap().t - frames[0].t;
    assert!((frames.len() as f32 / span - 60.0).abs() < 3.0, "{} frames in {span} s", frames.len());
    let last = frames.last().unwrap();
    assert_eq!(last.targets.len(), 2, "a new target replaced the dead one");
    assert!(frames.iter().filter(|f| f.t > killed_at + 0.02).all(|f| f.targets.iter().all(|p| p.no != no)));
    // Poses match the live targets.
    for (p, t) in last.targets.iter().zip(sim.targets()) {
        let (yaw, pitch) = angles(t.pos);
        assert_eq!(p.no, t.no());
        assert!((p.yaw_deg - yaw).abs() < 0.5 && (p.pitch_deg - pitch).abs() < 0.5, "{p:?}");
        assert!(p.radius_deg > 0.1 && p.height_deg == 0.0 && p.health > 0.0, "{p:?}");
    }
}

#[test]
fn tracking_samples_show_lead_and_lag() {
    let mut s = no_countdown(Scenario::default());
    s.weapon.kind = WeaponKind::Beam;
    s.weapon.trigger = Trigger::Always;
    s.targets.health = 1e9;
    s.targets.spawn.yaw = Range(-30.0, 30.0);
    s.targets.movement =
        Movement { kind: MovementKind::Strafe, change_every_s: Range(30.0, 30.0), ..Default::default() };
    let mut sim = Sim::new(s, 4);
    for _ in 0..120 {
        let t = &sim.targets()[0];
        let (yaw, pitch) = angles(t.pos);
        // Aim 1° behind the target's motion.
        let behind = -t.rate[0].signum();
        sim.step(FRAME, Input { aim: from_angles(yaw + behind, pitch), fire_pressed: false, fire_held: false });
    }
    let samples = &sim.result().telemetry.samples;
    assert!(samples.len() >= 14, "{}", samples.len());
    let lead: f32 = samples.iter().skip(1).map(|s| s.lead_deg).sum::<f32>() / (samples.len() - 1) as f32;
    assert!(lead < -0.8, "behind the target reads as negative lead: {lead}");
    assert!(samples[3].target_speed_dps > 10.0);
}

// Helper bots (plan K3): pushes that move the targets around.

use crate::world::Pusher;

/// One ball of radius 0.5 at `at` in an empty world, with a bot's physics (a gravity
/// of `gravity` m/s², no friction in the air) so that kicks stay in its velocity.
fn pushed(at: [f32; 3], gravity: f32, meshes: Vec<crate::world::WorldMesh>, pushers: Vec<Pusher>) -> Sim {
    let mut s = no_countdown(builtin("builtin:static-grid"));
    s.targets.count = 1;
    s.targets.radius = 0.5;
    s.targets.height = 1.0;
    s.world = Some(crate::world::World {
        spawns: vec![crate::world::WorldSpawn { pos: at, ..Default::default() }],
        meshes,
        motion: Some(crate::motion::Motion { gravity, ..Default::default() }),
        pushers,
        ..Default::default()
    });
    Sim::new(s, 1)
}

/// A pinned pusher 5 m ahead, facing away from the player: its reach is a ball of
/// radius 1 centred 1 m in front of it, and it kicks every 0.1 s.
fn pinned() -> Pusher {
    Pusher { pos: [0.0, 0.0, -5.0], radius: 1.0, every_s: 0.1, push: 2.0, ..Default::default() }
}

/// A chasing pusher on a plane just under a floor at y = 0, facing up.
fn under_the_floor() -> Pusher {
    Pusher {
        pos: [0.0, -0.1, -10.0],
        facing: [0.0, 1.0, 0.0],
        chases: true,
        radius: 0.6,
        every_s: 0.02,
        push: 3.0,
        lift: 0.5,
        ..Default::default()
    }
}

fn vel(sim: &Sim) -> Vec3 {
    sim.targets()[0].bot.as_ref().unwrap().vel
}

/// Steps for `secs` seconds at 240 Hz.
fn run_for(sim: &mut Sim, secs: f32) {
    for _ in 0..(secs * 240.0).round() as usize {
        sim.step(FRAME, idle());
    }
}

/// A target whose body overlaps the reach is kicked, one whose body doesn't is left
/// alone, and so is one behind the pusher. The reach is a ball of the pusher's
/// radius whose centre is that far ahead, so along its facing it ends at twice the
/// radius, plus the target's own.
#[test]
fn a_pinned_pushers_reach_is_a_ball_ahead_of_it() {
    // (where the target is, whether it is kicked)
    for (at, kicked) in [
        ([0.0, 0.0, -7.45], true),  // just inside 2 * 1 + 0.5 from the pusher
        ([0.0, 0.0, -7.55], false), // just outside
        ([0.0, 0.0, -4.0], false),  // behind it
        ([0.0, 0.0, -5.5], true),   // beside it, inside the ball
        ([0.0, 1.45, -6.0], true),  // above the ball's middle
        ([0.0, 1.55, -6.0], false),
    ] {
        let mut sim = pushed(at, 0.0, Vec::new(), vec![pinned()]);
        run_for(&mut sim, 0.125);
        assert_eq!(vel(&sim) != Vec3::ZERO, kicked, "target at {at:?}: {}", vel(&sim));
    }
}

/// A kick adds to the target's speed, level and away from the pusher, with the height
/// difference dropped, and the lift is added to its upward speed. Speeds add up
/// (they aren't set), and the push doesn't fade with distance.
#[test]
fn a_kick_adds_a_level_push_and_a_lift() {
    let p = Pusher { lift: 0.5, ..pinned() };
    // Right of and above the pusher: the push is along (1, 0, -1), not up toward it.
    let mut sim = pushed([1.0, 0.3, -6.0], 0.0, Vec::new(), vec![p]);
    run_for(&mut sim, 0.125);
    let side = 2.0 / 2f32.sqrt();
    assert!((vel(&sim) - Vec3::new(side, 0.5, -side)).length() < 1e-4, "{}", vel(&sim));
    // The next kick adds the same again; nothing slows it in the air. (It is out of
    // the reach by then, so the pusher is made to reach everywhere.)
    let wide = Pusher { radius: 50.0, lift: 0.5, ..pinned() };
    let mut sim = pushed([1.0, 0.3, -6.0], 0.0, Vec::new(), vec![wide]);
    run_for(&mut sim, 0.225);
    assert!((vel(&sim) - Vec3::new(2.0 * side, 1.0, -2.0 * side)).length() < 1e-3, "{}", vel(&sim));
    // The push is as hard from far away.
    let mut sim = pushed([0.0, 0.0, -30.0], 0.0, Vec::new(), vec![Pusher { radius: 50.0, ..pinned() }]);
    run_for(&mut sim, 0.125);
    assert!((vel(&sim) - Vec3::new(0.0, 0.0, -2.0)).length() < 1e-4, "{}", vel(&sim));
}

/// A pusher under a target has no level direction to push in: it lifts it and
/// doesn't throw it sideways (a divide by a distance of nothing would give NaN).
#[test]
fn a_kick_from_straight_below_has_no_sideways_push() {
    for x in [0.0, 5e-5, 0.3] {
        let mut sim = pushed([x, 1.0, -10.0], 0.0, Vec::new(), vec![under_the_floor()]);
        run_for(&mut sim, 0.025);
        assert!((vel(&sim) - Vec3::new(0.0, 0.5, 0.0)).length() < 1e-4, "x {x}: {}", vel(&sim));
    }
    // Chasing along a wall (a plane facing -z at z = -5) pushes straight away from
    // it, wherever the target is on it.
    let wall = Pusher { pos: [0.0, 0.0, -5.0], facing: [0.0, 0.0, -1.0], push: 2.0, radius: 3.0, ..under_the_floor() };
    let mut sim = pushed([1.0, 0.3, -8.0], 0.0, Vec::new(), vec![wall]);
    run_for(&mut sim, 0.025);
    assert!((vel(&sim) - Vec3::new(0.0, 0.5, -2.0)).length() < 1e-4, "{}", vel(&sim));
}

/// A chasing pusher only reaches targets in front of its plane.
#[test]
fn a_chasing_pusher_ignores_targets_behind_its_plane() {
    let mut sim = pushed([0.0, -0.5, -10.0], 0.0, Vec::new(), vec![under_the_floor()]);
    run_for(&mut sim, 0.125);
    assert_eq!(vel(&sim), Vec3::ZERO);
}

/// The number of kicks is the number of times the run passes a multiple of the
/// period, whatever the step: a period that isn't a multiple of the frame time
/// neither loses a kick nor doubles one, even with uneven frames.
#[test]
fn kicks_come_on_the_period_whatever_the_step() {
    let p = Pusher { radius: 100.0, every_s: 0.07, push: 1.0, ..pinned() };
    let kicks = |frame: &dyn Fn(usize) -> f32, secs: f32| {
        let mut sim = pushed([0.0, 0.0, -8.0], 0.0, Vec::new(), vec![p.clone()]);
        let (mut t, mut i) = (0.0, 0);
        while t < secs {
            let dt = frame(i);
            sim.step(dt, idle());
            t += dt;
            i += 1;
        }
        vel(&sim).length()
    };
    // 1.0 s holds 14 periods of 0.07 s (0.98 s), 2.0 s holds 28 (1.96 s).
    assert!((kicks(&|_| FRAME, 1.0) - 14.0).abs() < 1e-3);
    assert!((kicks(&|_| FRAME, 2.0) - 28.0).abs() < 1e-3);
    // Uneven frames, some slower than the period: 1.1 s holds 15 (1.05 s).
    assert!((kicks(&|i| if i % 2 == 0 { 0.1 } else { 0.01 }, 1.095) - 15.0).abs() < 1e-3);
    // The kicks start with the run: none before one whole period has passed.
    assert_eq!(kicks(&|_| FRAME, 0.06), 0.0);
}

/// A pusher that has to see its target is stopped by a solid wall between them, but
/// not by an invisible clip wall; one that doesn't need sight ignores both.
#[test]
fn sight_is_blocked_by_solid_walls_but_not_clip_walls() {
    let wall = |clip: bool| crate::world::WorldMesh {
        positions: vec![[-5.0, -5.0, -6.0], [5.0, -5.0, -6.0], [5.0, 5.0, -6.0], [-5.0, 5.0, -6.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        clip,
        ..Default::default()
    };
    // The pusher is at z = -5, the wall at -6, the target at -6.8 (inside the reach).
    for (clip, needs_sight, kicked) in [(false, true, false), (true, true, true), (false, false, true)] {
        let p = Pusher { needs_sight, ..pinned() };
        let mut sim = pushed([0.0, 0.0, -6.8], 0.0, vec![wall(clip)], vec![p]);
        run_for(&mut sim, 0.125);
        assert_eq!(vel(&sim) != Vec3::ZERO, kicked, "clip {clip}, needs sight {needs_sight}");
    }
}

/// A target that falls onto an invisible floor lands on it; with a chasing pusher
/// under the floor that lifts what comes near, it slows, turns back and never
/// touches it.
#[test]
fn a_chasing_pusher_under_a_clip_floor_holds_a_bot_up() {
    let floor = crate::world::WorldMesh {
        positions: vec![[-30.0, 0.0, -30.0], [30.0, 0.0, -30.0], [30.0, 0.0, 5.0], [-30.0, 0.0, 5.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        clip: true,
        ..Default::default()
    };
    // The reach tops out 1.6 m above the plane for a ball of this size, and each kick
    // adds 0.5 m/s against a pull of 0.16 m/s per kick.
    let lowest = |pushers: Vec<Pusher>| {
        let mut sim = pushed([0.0, 3.0, -10.0], 8.0, vec![floor.clone()], pushers);
        let mut lowest = f32::MAX;
        let mut touched = false;
        for _ in 0..720 {
            sim.step(FRAME, idle());
            lowest = lowest.min(sim.targets()[0].pos.y);
            touched |= sim.targets()[0].bot.as_ref().unwrap().grounded;
        }
        (lowest, touched)
    };
    let (low, touched) = lowest(Vec::new());
    assert!(touched && (low - 0.5).abs() < 0.05, "it lands on the floor: {low}");
    let (low, touched) = lowest(vec![Pusher { push: 0.0, ..under_the_floor() }]);
    assert!(!touched && low > 0.75, "it stays up: {low}");
}

/// A bot with its own `invincible` flag takes hits and never dies while another kind
/// in the same world does (a character with `InvincibleBots`).
#[test]
fn a_kind_can_be_invincible_on_its_own() {
    use crate::world::{WorldKind, WorldSpawn};
    let mut s = on_a_floor(Vec::new(), None);
    s.weapon.damage = 4.0;
    s.targets.count = 2;
    let w = s.world.as_mut().unwrap();
    w.spawns = vec![
        WorldSpawn { pos: [-3.0, -1.0, -10.0], ..Default::default() },
        WorldSpawn { pos: [3.0, -1.0, -10.0], ..Default::default() },
    ];
    w.kinds = vec![
        WorldKind { health: 1.0, invincible: true, spawns: vec![0], ..Default::default() },
        WorldKind { health: 1.0, spawns: vec![1], ..Default::default() },
    ];
    w.slots = vec![vec![(0, 1.0)], vec![(1, 1.0)]];
    let mut sim = Sim::new(s, 3);
    let at = |sim: &Sim, kind: u32| sim.targets().iter().find(|t| t.kind() == Some(kind)).unwrap().pos.normalize();
    for kind in [0, 1] {
        let aim = at(&sim, kind);
        sim.step(FRAME, Input { aim, fire_pressed: true, fire_held: false });
        for _ in 0..240 {
            sim.step(FRAME, idle());
        }
    }
    assert_eq!(sim.stats().hits, 2);
    assert_eq!(sim.stats().kills, 1, "only the kind that isn't invincible dies");
    assert!(sim.targets().iter().any(|t| t.kind() == Some(0) && t.health == 1.0));
}

/// A world scenario (an empty floor, one target) whose clock runs at `time_scale`.
fn timed(time_scale: f32) -> Scenario {
    let mut s = on_a_floor(Vec::new(), None);
    s.world.as_mut().unwrap().time_scale = time_scale;
    s
}

/// Steps `secs` real seconds at 240 Hz, looking away from everything.
fn real_seconds(sim: &mut Sim, secs: f32) {
    for _ in 0..(secs * 240.0).round() as usize {
        sim.step(FRAME, idle());
    }
}

/// Bug: the importer ignored `Timescale`, so a scenario at 0.5 played at normal
/// speed. The run's clock counts game seconds: 10 real seconds at 0.5 are 5.
#[test]
fn a_time_scale_makes_the_run_clock_run_at_that_rate() {
    for (scale, want) in [(0.5, 5.0), (2.0, 20.0), (1.0, 10.0)] {
        let mut s = timed(scale);
        s.timing.duration_s = 100.0;
        let mut sim = Sim::new(s, 1);
        assert_eq!(sim.time_scale(), scale);
        real_seconds(&mut sim, 10.0);
        assert!((sim.elapsed() - want).abs() < 0.01, "at {scale}: {} game seconds", sim.elapsed());
    }
}

/// Bug: the importer ignored `Timescale`, so a 20 s scenario at 0.5 ended after 20
/// real seconds instead of 40 (and at 2.0 after 20 instead of 10).
#[test]
fn a_time_scaled_run_lasts_its_length_divided_by_the_scale() {
    for (scale, real) in [(0.5, 40.0), (2.0, 10.0)] {
        let mut s = timed(scale);
        s.timing.duration_s = 20.0;
        let mut sim = Sim::new(s, 1);
        real_seconds(&mut sim, real - 0.5);
        assert_eq!(sim.phase(), Phase::Running, "at {scale}, not over after {} s", real - 0.5);
        real_seconds(&mut sim, 1.0);
        assert_eq!(sim.phase(), Phase::Finished, "at {scale}, over after {} s", real + 0.5);
        assert!((sim.elapsed() - 20.0).abs() < 1e-3, "the timer counts game seconds");
        assert!((sim.real_elapsed() - real).abs() < 0.01);
        assert!((sim.result().duration_s - real).abs() < 0.01, "the saved run counts real seconds");
    }
}

/// The countdown is real time at any time scale, and only the part of a step past it
/// is scaled.
#[test]
fn the_countdown_takes_the_same_real_time_at_any_time_scale() {
    for scale in [0.5, 1.0, 2.0] {
        let mut s = timed(scale);
        s.timing.countdown_s = 3.0;
        s.timing.duration_s = 100.0;
        let mut sim = Sim::new(s, 1);
        real_seconds(&mut sim, 2.9);
        assert_eq!(sim.phase(), Phase::Countdown, "at {scale}");
        assert!((sim.countdown_left() - 0.1).abs() < 0.01, "at {scale}: {}", sim.countdown_left());
        // One step of 1 s crosses the end: 0.1 s of it is countdown, 0.9 s is play.
        sim.step(1.0, idle());
        assert_eq!(sim.phase(), Phase::Running, "at {scale}");
        assert!((sim.elapsed() - 0.9 * scale).abs() < 0.02, "at {scale}: {}", sim.elapsed());
    }
}

/// Bug: the importer ignored `Timescale`, so bots moved at normal speed in slow
/// motion. A falling bot covers the same distance in 2 real seconds at 0.5 as in 1 at 1.0.
#[test]
fn bots_move_on_the_game_clock() {
    let fall = |scale: f32, secs: f32| {
        let mut s = timed(scale);
        s.timing.duration_s = 100.0;
        s.world.as_mut().unwrap().spawns[0].pos = [0.0, 20.0, -10.0];
        s.world.as_mut().unwrap().motion = Some(crate::motion::Motion { gravity: 9.8, ..Default::default() });
        let mut sim = Sim::new(s, 1);
        let start = sim.targets()[0].pos.y;
        real_seconds(&mut sim, secs);
        start - sim.targets()[0].pos.y
    };
    let normal = fall(1.0, 1.0);
    assert!(normal > 2.0, "it falls: {normal} m");
    let slow = fall(0.5, 2.0);
    assert!((slow - normal).abs() < 0.1, "{slow} m in 2 s at 0.5, {normal} m in 1 s at 1.0");
    let slower = fall(0.5, 1.0);
    assert!(slower < normal * 0.5, "{slower} m in 1 s at 0.5");
}

/// Bug: the importer ignored `Timescale`, so a held automatic weapon fired at its
/// full rate in slow motion. Shots per real second are the rate times the time scale.
#[test]
fn a_held_weapon_fires_at_the_rate_times_the_time_scale() {
    let shots = |scale: f32| {
        let mut s = timed(scale);
        s.timing.duration_s = 100.0;
        s.weapon.trigger = Trigger::Hold;
        s.weapon.fire_rate = 10.0;
        let mut sim = Sim::new(s, 1);
        let held = Input { aim: Vec3::Y, fire_pressed: false, fire_held: true };
        for _ in 0..(10 * 240) {
            sim.step(FRAME, held);
        }
        sim.stats().shots as f32
    };
    let (normal, slow, fast) = (shots(1.0), shots(0.5), shots(2.0));
    assert!((normal - 100.0).abs() <= 2.0, "{normal} shots in 10 s at 10/s");
    assert!((slow - 50.0).abs() <= 2.0, "{slow} shots in 10 s at 0.5");
    assert!((fast - 200.0).abs() <= 3.0, "{fast} shots in 10 s at 2.0");
}

/// The time scale is part of a scenario's identity only when it isn't 1.0, so the
/// fingerprint (and the bests tied to it) of every earlier scenario stays the same.
#[test]
fn a_time_scale_of_one_leaves_the_fingerprint_alone() {
    let plain = timed(1.0);
    assert!(!plain.to_toml().contains("time_scale"));
    let slow = timed(0.5);
    assert!(slow.to_toml().contains("time_scale"));
    assert_ne!(plain.fingerprint(), slow.fingerprint());
    assert!(plain.problems().is_empty(), "{:?}", plain.problems());
    assert!(slow.problems().is_empty());
    for bad in [0.0, 0.05, 10.5, f32::NAN] {
        assert!(!timed(bad).problems().is_empty(), "{bad} is refused");
    }
}

/// Bug: the importer ignored `Timescale`, so a run's saved length, its kill times and
/// its replay timeline were in game seconds, and a replay of a slow-motion run played
/// back twice too fast. The result reports real seconds, with replay frames and the
/// aim trace still at 60 and 240 per real second.
#[test]
fn a_time_scaled_result_is_in_real_seconds() {
    let mut s = timed(0.5);
    s.timing.duration_s = 10.0;
    let mut sim = Sim::new(s, 1);
    // Click the target 4 real seconds in (2 game seconds), then wait out the run.
    real_seconds(&mut sim, 4.0);
    let at = sim.targets()[0].pos;
    sim.step(FRAME, shoot(at));
    real_seconds(&mut sim, 17.0);
    assert_eq!(sim.phase(), Phase::Finished);
    let r = sim.result();
    assert_eq!(r.kills.len(), 1);
    assert!((r.kills[0].at_s - 4.0).abs() < 0.05, "kill at {} real s", r.kills[0].at_s);
    assert!((r.kills[0].ttk_s - 4.0).abs() < 0.05, "time to kill {} real s", r.kills[0].ttk_s);
    assert!((r.duration_s - 20.0).abs() < 0.01);
    let last = r.telemetry.trace.last().unwrap()[0];
    assert!(last > 19.9 && last <= 20.01, "trace ends at {last}");
    assert!(r.telemetry.shots.iter().all(|s| (s.t - 4.0).abs() < 0.05));
    let frames = &r.telemetry.frames;
    assert!(frames.last().unwrap().t > 19.9);
    let per_s = frames.len() as f32 / 20.0;
    assert!((per_s - FRAME_HZ).abs() < 2.0, "{per_s} frames per real second");
}

/// Bug: the importer ignored `Timescale`, so the time estimates for workouts and the
/// program counted a scenario's duration as real seconds. Slow motion takes longer.
#[test]
fn a_scenarios_real_length_follows_its_time_scale() {
    let mut s = timed(0.5);
    s.timing.duration_s = 30.0;
    s.timing.countdown_s = 3.0;
    assert_eq!(s.real_seconds(), 63.0);
    s.world.as_mut().unwrap().time_scale = 2.0;
    assert_eq!(s.real_seconds(), 18.0);
    s.world = None;
    assert_eq!(s.real_seconds(), 33.0);
}

/// A world scenario with one target that has 100 health, scoring nothing for kills and
/// hits (so the rules under test are all that count), a weapon doing 10 per hit, and a
/// run of `duration_s` game seconds.
fn scored(duration_s: f32, time_score: f32) -> Scenario {
    let mut s = timed(1.0);
    s.timing.duration_s = duration_s;
    s.scoring =
        crate::scenario::Scoring { per_kill: 0.0, per_hit: 0.0, per_miss: 0.0, per_damage: 0.0, per_timeout: 0.0 };
    s.targets.health = 100.0;
    s.weapon.damage = 10.0;
    s.world.as_mut().unwrap().time_score = time_score;
    s
}

/// Fires at the target (`hit`) or away from it, then waits a real second.
fn click(sim: &mut Sim, hit: bool) {
    let aim = if hit { sim.targets()[0].pos } else { Vec3::Y };
    sim.step(FRAME, Input { aim: aim.normalize(), fire_pressed: true, fire_held: true });
    real_seconds(sim, 1.0);
}

/// The time score is the rate times the game seconds left when the run ends on its kill
/// limit, in real seconds or at a time scale (the game clock is what counts).
#[test]
fn the_time_score_is_the_rate_times_the_game_seconds_left_at_the_end() {
    for (scale, real_wait) in [(1.0, 4.0), (0.9, 4.0), (2.0, 4.0)] {
        let mut s = scored(100.0, 2.0);
        s.world.as_mut().unwrap().time_scale = scale;
        s.timing.kill_limit = 1;
        s.weapon.damage = 100.0;
        let mut sim = Sim::new(s, 1);
        real_seconds(&mut sim, real_wait);
        let at = sim.targets()[0].pos;
        sim.step(FRAME, shoot(at));
        assert_eq!(sim.phase(), Phase::Finished, "at {scale}");
        let played = real_wait * scale;
        assert!((sim.elapsed() - played).abs() < 0.05, "at {scale}: {} game seconds", sim.elapsed());
        let want = 2.0 * (100.0 - sim.elapsed());
        assert!((sim.score() - want).abs() < 1e-3, "at {scale}: {} for {want}", sim.score());
        assert!((sim.score() - 2.0 * (100.0 - played)).abs() < 0.2, "at {scale}: game seconds, not real");
        assert!((sim.result().score - sim.score()).abs() < 1e-6, "the saved score is the same");
    }
}

/// A run that times out has no time left, so no points for it; before that the same
/// formula shows what the time left is worth now, and counts down.
#[test]
fn the_time_score_is_zero_at_a_timeout_and_counts_down_before() {
    let mut sim = Sim::new(scored(10.0, 3.0), 1);
    assert!((sim.score() - 30.0).abs() < 1e-3, "all the time is left at the start: {}", sim.score());
    real_seconds(&mut sim, 2.0);
    assert!((sim.score() - 24.0).abs() < 0.02, "{}", sim.score());
    real_seconds(&mut sim, 3.0);
    assert!((sim.score() - 15.0).abs() < 0.02, "{}", sim.score());
    real_seconds(&mut sim, 6.0);
    assert_eq!(sim.phase(), Phase::Finished);
    assert_eq!(sim.score(), 0.0);
    assert_eq!(sim.result().score, 0.0);
}

/// Four shots at a target with 100 health, three of them hits of 10 damage: 30 done of 40
/// possible. Scored by damage only, the efficiency factor is 30/40.
#[test]
fn damage_efficiency_multiplies_the_score_by_damage_done_over_possible() {
    let mut s = scored(100.0, 0.0);
    s.scoring.per_damage = 1.0;
    let play = |s: Scenario| {
        let mut sim = Sim::new(s, 1);
        real_seconds(&mut sim, 0.1);
        for hit in [true, true, false, true] {
            click(&mut sim, hit);
        }
        sim
    };
    let plain = play(s.clone());
    assert_eq!((plain.stats().damage_done, plain.stats().damage_possible), (30.0, 40.0));
    assert_eq!(plain.score(), 30.0);
    s.world.as_mut().unwrap().damage_efficiency = true;
    let sim = play(s.clone());
    assert!((sim.score() - 30.0 * 30.0 / 40.0).abs() < 1e-4, "{}", sim.score());
    // With the accuracy multiplier too (3 hits of 4 shots), both apply.
    s.world.as_mut().unwrap().accuracy_mult = crate::world::AccuracyMult::Linear;
    let sim = play(s.clone());
    assert!((sim.score() - 30.0 * 0.75 * 0.75).abs() < 1e-4, "{}", sim.score());
    // The multipliers apply to the points for time left as well.
    s.world.as_mut().unwrap().time_score = 1.0;
    let sim = play(s);
    let want = (30.0 + sim.time_left()) * 0.75 * 0.75;
    assert!((sim.score() - want).abs() < 1e-3, "{} for {want}", sim.score());
}

/// With no shots there is no efficiency to speak of, so the score isn't cut: the same as
/// the accuracy multiplier with no shots.
#[test]
fn damage_efficiency_with_no_shots_leaves_the_score_alone() {
    let mut s = scored(10.0, 1.0);
    s.world.as_mut().unwrap().damage_efficiency = true;
    s.world.as_mut().unwrap().accuracy_mult = crate::world::AccuracyMult::Linear;
    let sim = Sim::new(s, 1);
    assert_eq!(sim.stats().damage_efficiency(), None);
    assert!((sim.score() - 10.0).abs() < 1e-4, "{}", sim.score());
}

/// A hit does no more damage than the target had left, so overkill counts as damage possible
/// and not as damage done: the kill shot of 10 on a target with 4 left is 4 of 10. Not
/// measured in KovaaK's.
#[test]
fn overkill_counts_as_possible_damage_and_not_as_done() {
    let mut s = scored(100.0, 0.0);
    s.targets.health = 4.0;
    s.scoring.per_damage = 1.0;
    s.world.as_mut().unwrap().damage_efficiency = true;
    let mut sim = Sim::new(s, 1);
    real_seconds(&mut sim, 0.1);
    click(&mut sim, true);
    let st = sim.stats();
    assert_eq!((st.kills, st.damage_done, st.damage_possible), (1, 4.0, 10.0));
    assert!((sim.score() - 4.0 * 0.4).abs() < 1e-4, "{}", sim.score());
}

/// The beam adds its damage per tick to damage possible whether or not it is on a target
/// (as the run file does), so efficiency is the share of firing time spent on the target.
#[test]
fn the_beam_has_the_share_of_time_on_target_as_its_efficiency() {
    let mut s = scored(100.0, 0.0);
    s.weapon.kind = WeaponKind::Beam;
    s.weapon.trigger = Trigger::Always;
    s.weapon.damage_per_s = 10.0;
    s.targets.health = 1e6;
    s.scoring.per_damage = 1.0;
    s.world.as_mut().unwrap().damage_efficiency = true;
    let mut sim = Sim::new(s, 1);
    let at = sim.targets()[0].pos.normalize();
    for aim in [at, Vec3::Y, at, Vec3::Y] {
        for _ in 0..240 {
            sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: false });
        }
    }
    let st = sim.stats();
    assert!((st.damage_possible - 40.0).abs() < 0.5, "{}", st.damage_possible);
    assert!((st.damage_done - 20.0).abs() < 0.5, "{}", st.damage_done);
    assert!((sim.score() - st.damage_done * st.damage_done / st.damage_possible).abs() < 1e-3);
}

/// The new fields are part of a scenario's identity only when they are set, so the
/// fingerprint (and the bests and ratings tied to it) of every earlier scenario stays the
/// same. `imported_fixtures_keep_their_fingerprints` pins two real imports.
#[test]
fn a_default_world_serialises_without_the_score_rules() {
    let plain = timed(1.0);
    let text = plain.to_toml();
    assert!(!text.contains("time_score") && !text.contains("damage_efficiency"), "{text}");
    let mut with_time = timed(1.0);
    with_time.world.as_mut().unwrap().time_score = 1.0;
    assert!(with_time.to_toml().contains("time_score"));
    assert_ne!(plain.fingerprint(), with_time.fingerprint());
    let mut with_eff = timed(1.0);
    with_eff.world.as_mut().unwrap().damage_efficiency = true;
    assert!(with_eff.to_toml().contains("damage_efficiency"));
    assert_ne!(plain.fingerprint(), with_eff.fingerprint());
    // They survive a round trip.
    let back = Scenario::from_toml(&with_time.to_toml()).unwrap();
    assert_eq!(back.world.unwrap().time_score, 1.0);
    let mut bad = timed(1.0);
    bad.world.as_mut().unwrap().time_score = f32::NAN;
    assert!(!bad.problems().is_empty());
}

// Plan K6a: where bots appear and stand.

/// A world with a point spawn at (0, -1, -10) and two kinds of bot, one slot each. Nothing
/// has a motion, so a bot stays where it appears. The first kind has none of the K6a
/// fields; `second` is the other.
fn two_kinds(second: crate::world::WorldKind) -> Sim {
    use crate::world::WorldKind;
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.count = 2;
    let w = s.world.as_mut().unwrap();
    w.kinds = vec![WorldKind { health: 1.0, ..Default::default() }, WorldKind { health: 1.0, ..second }];
    w.slots = vec![vec![(0, 1.0)], vec![(1, 1.0)]];
    Sim::new(s, 3)
}

fn kind_pos(sim: &Sim, kind: u32) -> Vec3 {
    sim.targets().iter().find(|t| t.kind() == Some(kind)).unwrap().pos
}

/// Bug (plan K6): every kind of bot started at the first kind's height. A kind's `rise`
/// puts it that much higher (or lower) than the first kind from the same spawn point. A
/// world whose kinds have none spawns exactly where it did (the imported fixtures'
/// pinned fingerprints and the layouts of their seeds cover that too).
#[test]
fn a_kind_with_a_rise_starts_that_much_higher() {
    let sim = two_kinds(crate::world::WorldKind { rise: 0.5, ..Default::default() });
    assert_eq!(kind_pos(&sim, 0), Vec3::new(0.0, -1.0, -10.0));
    assert_eq!(kind_pos(&sim, 1), Vec3::new(0.0, -0.5, -10.0));
    let sim = two_kinds(crate::world::WorldKind { rise: -0.25, ..Default::default() });
    assert_eq!(kind_pos(&sim, 1), Vec3::new(0.0, -1.25, -10.0));
    let sim = two_kinds(crate::world::WorldKind::default());
    assert_eq!(kind_pos(&sim, 1), kind_pos(&sim, 0));
}

/// Bug (plan K6): every kind of bot used the first kind's spawn offsets. A kind with its
/// own pair uses it, along the spawn point's axes, and the first kind keeps the world's.
#[test]
fn a_kind_with_its_own_offsets_uses_them() {
    let sim =
        two_kinds(crate::world::WorldKind { offsets: Some([[1.0, 0.5, 0.0], [1.0, 0.5, 0.0]]), ..Default::default() });
    assert_eq!(kind_pos(&sim, 0), Vec3::new(0.0, -1.0, -10.0));
    assert_eq!(kind_pos(&sim, 1), Vec3::new(1.0, -0.5, -10.0));
    // The world's offsets are for the kinds that don't say.
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.count = 2;
    let w = s.world.as_mut().unwrap();
    (w.offset_min, w.offset_max) = ([0.0, 0.0, 2.0], [0.0, 0.0, 2.0]);
    w.kinds = vec![
        crate::world::WorldKind { health: 1.0, ..Default::default() },
        crate::world::WorldKind { health: 1.0, offsets: Some([[0.0; 3], [0.0; 3]]), ..Default::default() },
    ];
    w.slots = vec![vec![(0, 1.0)], vec![(1, 1.0)]];
    let sim = Sim::new(s, 3);
    assert_eq!(kind_pos(&sim, 0), Vec3::new(0.0, -1.0, -8.0));
    assert_eq!(kind_pos(&sim, 1), Vec3::new(0.0, -1.0, -10.0));
}

/// A kind's own offsets and rise draw the same random numbers as the world's, so a
/// world that doesn't use them keeps its layout, and one that does moves only the
/// bot of that kind.
#[test]
fn a_kinds_rise_and_offsets_draw_no_extra_random_numbers() {
    let mut spawns = Vec::new();
    for second in [
        crate::world::WorldKind::default(),
        crate::world::WorldKind { rise: 0.4, offsets: Some([[0.0, 0.0, 0.0], [3.0, 0.0, 0.0]]), ..Default::default() },
    ] {
        let mut s = on_a_floor(Vec::new(), None);
        s.targets.count = 2;
        let w = s.world.as_mut().unwrap();
        w.offset_max = [3.0, 0.0, 0.0];
        w.kinds = vec![
            crate::world::WorldKind { health: 1.0, ..Default::default() },
            crate::world::WorldKind { health: 1.0, ..second },
        ];
        w.slots = vec![vec![(0, 1.0)], vec![(1, 1.0)]];
        let mut sim = Sim::new(s, 11);
        // A third, fourth, ... bot, from the same stream.
        let mut seen = vec![kind_pos(&sim, 0), kind_pos(&sim, 1)];
        for _ in 0..4 {
            sim.step(FRAME, Input { aim: kind_pos(&sim, 0).normalize(), fire_pressed: true, fire_held: false });
            seen.push(kind_pos(&sim, 0));
        }
        spawns.push(seen);
    }
    // Kind 0 follows the same stream in both worlds.
    let first = |v: &Vec<Vec3>| v.iter().enumerate().filter(|(i, _)| *i != 1).map(|(_, p)| *p).collect::<Vec<_>>();
    assert_eq!(first(&spawns[0]), first(&spawns[1]));
}

/// One ball of radius 0.5 with `motion` appears at `at` over a floor at y = 0 (which
/// stops it).
fn over_a_floor(motion: crate::motion::Motion, at: [f32; 3]) -> Sim {
    let floor = crate::world::WorldMesh {
        positions: vec![[-30.0, 0.0, -30.0], [30.0, 0.0, -30.0], [30.0, 0.0, 5.0], [-30.0, 0.0, 5.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        ..Default::default()
    };
    let mut s = no_countdown(builtin("builtin:static-grid"));
    s.targets.count = 1;
    s.targets.radius = 0.5;
    s.targets.height = 1.0;
    s.world = Some(crate::world::World {
        spawns: vec![crate::world::WorldSpawn { pos: at, ..Default::default() }],
        meshes: vec![floor],
        motion: Some(motion),
        ..Default::default()
    });
    Sim::new(s, 1)
}

fn bot_y(sim: &Sim) -> f32 {
    sim.targets()[0].pos.y
}

fn grounded(sim: &Sim) -> bool {
    sim.targets()[0].bot.as_ref().unwrap().grounded
}

/// A bot that doesn't move by itself but has gravity (what the importer gives a still
/// bot whose character has some) falls from where it appeared and rests on the floor.
#[test]
fn a_still_bot_with_gravity_falls_and_rests_on_the_floor() {
    let still = crate::motion::Motion { gravity: 8.0, ..Default::default() };
    let mut sim = over_a_floor(still, [0.0, 3.0, -10.0]);
    assert_eq!(bot_y(&sim), 3.0);
    run_for(&mut sim, 1.0);
    assert!((bot_y(&sim) - 0.5).abs() < 0.01 && grounded(&sim), "{}", bot_y(&sim));
}

/// A bot that appears with a floor within its step-up height below its feet starts on
/// it, grounded, in its first frame. With the floor further down it starts in the air
/// and falls. A ball's feet are 0.5 below its middle, so at 0.75 the floor is 0.25 down.
#[test]
fn a_bot_starts_on_a_floor_within_its_step_up_height() {
    let walker = |step_up: f32| crate::motion::Motion { gravity: 8.0, step_up, ..Default::default() };
    let sim = over_a_floor(walker(0.3), [0.0, 0.75, -10.0]);
    assert!((bot_y(&sim) - 0.5).abs() < 0.01 && grounded(&sim), "{}", bot_y(&sim));
    let mut sim = over_a_floor(walker(0.2), [0.0, 0.75, -10.0]);
    assert_eq!((bot_y(&sim), grounded(&sim)), (0.75, false));
    run_for(&mut sim, 1.0);
    assert!((bot_y(&sim) - 0.5).abs() < 0.01 && grounded(&sim), "it fell onto the floor: {}", bot_y(&sim));
    // Just within the limit counts; no step-up means no pull.
    let sim = over_a_floor(walker(0.26), [0.0, 0.75, -10.0]);
    assert!((bot_y(&sim) - 0.5).abs() < 0.01 && grounded(&sim));
    assert_eq!(bot_y(&over_a_floor(walker(0.0), [0.0, 0.75, -10.0])), 0.75);
}

/// The step-up pull is for bots that fall and walk: one with no gravity stays where it
/// appeared, and so does one that flies.
#[test]
fn a_bot_without_gravity_or_that_flies_isnt_pulled_to_the_floor() {
    let float = crate::motion::Motion { gravity: 0.0, step_up: 0.3, ..Default::default() };
    assert_eq!(bot_y(&over_a_floor(float, [0.0, 0.75, -10.0])), 0.75);
    let flier = crate::motion::Motion {
        gravity: 8.0,
        step_up: 0.3,
        flight: Some(crate::motion::Flight { up_speed: 2.0, down_speed: 2.0, ..Default::default() }),
        ..Default::default()
    };
    assert_eq!(bot_y(&over_a_floor(flier, [0.0, 0.75, -10.0])), 0.75);
}

/// A jumper that appears standing on a floor (within its step-up height) with its key
/// held jumps at once, and one that appears in the air only when it lands. Neither
/// jumps again with the key held.
#[test]
fn a_jumper_that_appears_standing_with_the_key_held_jumps_at_once() {
    let hopper = crate::motion::Motion {
        gravity: 8.0,
        step_up: 0.3,
        jump_speed: [3.0, 3.0],
        dodges: vec![crate::motion::Dodge {
            strafe: false,
            jump_chance: 1.0,
            jump_s: [100.0, 100.0],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut sim = over_a_floor(hopper.clone(), [0.0, 0.75, -10.0]);
    assert!(grounded(&sim) && (bot_y(&sim) - 0.5).abs() < 0.01);
    run_for(&mut sim, 0.1);
    assert!(bot_y(&sim) > 0.6 && !grounded(&sim), "left the floor: {}", bot_y(&sim));
    run_for(&mut sim, 3.0);
    assert!(grounded(&sim), "back down, and no second jump with the key held");
    // From the air: down first, then the one jump at the landing (about 0.8 s in).
    let mut sim = over_a_floor(hopper, [0.0, 3.0, -10.0]);
    run_for(&mut sim, 0.5);
    assert!(bot_y(&sim) < 3.0 && !grounded(&sim));
    run_for(&mut sim, 0.5);
    assert!(bot_y(&sim) > 0.7 && !grounded(&sim), "up again after landing: {}", bot_y(&sim));
    run_for(&mut sim, 3.0);
    assert!(grounded(&sim), "and then stayed down");
}
