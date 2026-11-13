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

/// Bug guard (found in the review of plan K9): placing a target in the room measured a ball by its radius
/// alone once heads were added, where it had always kept the larger of the radius and half the height
/// setting from the floor, the ceiling and the walls. The built-in ball scenarios (a radius of 0.3 with a
/// height setting of 1.8) would have had targets placed up to 0.6 m nearer the room's sides. A ball with a
/// head keeps at least half of body and head together.
#[test]
fn a_ball_keeps_its_old_distance_from_the_rooms_sides_when_it_is_placed() {
    let s = no_countdown(Scenario::default());
    assert_eq!((s.targets.shape, s.targets.head.is_none()), (Shape::Sphere, true));
    let keep = s.targets.radius.max(s.targets.height / 2.0);
    assert!(keep > s.targets.radius, "the default ball's height setting is the larger of the two");
    let a = s.arena.clone();
    let corner = |sim: &Sim, keep: f32| {
        let high = sim.fit_in_room(Vec3::new(1000.0, 1000.0, -1000.0));
        assert_eq!(high, Vec3::new(a.size[0] / 2.0 - keep, a.size[1] - a.eye_height - keep, keep - a.size[2] / 2.0));
        let low = sim.fit_in_room(Vec3::new(-1000.0, -1000.0, 1000.0));
        assert_eq!(low, Vec3::new(keep - a.size[0] / 2.0, keep - a.eye_height, a.size[2] / 2.0 - keep));
    };
    corner(&Sim::new(s.clone(), 1), keep);
    // With a head 1.0 in radius resting on the ball, body and head are 2.6 tall: 1.3 is kept.
    let mut headed = s.clone();
    headed.targets.head = Some(a_head(1.0, 0.0));
    assert!(headed.problems().is_empty(), "{:?}", headed.problems());
    corner(&Sim::new(headed, 1), 1.3);
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

/// An imported scenario with one still bot that regenerates, a weapon that holds fire at `rate` shots a
/// second for `damage` each, and a respawn so late that the first kill is the last thing that happens.
fn healer(health: f32, regen: f32, delay: f32, rate: f32, damage: f32) -> Scenario {
    let mut s = on_a_floor(Vec::new(), None);
    s.timing.duration_s = 600.0;
    s.weapon.trigger = Trigger::Hold;
    s.weapon.fire_rate = rate;
    s.weapon.damage = damage;
    s.targets.health = health;
    s.targets.regen_per_s = regen;
    s.targets.regen_delay_s = delay;
    s.targets.respawn_delay_s = 60.0;
    s
}

/// Hits taken to kill the first bot with fire held on it, at `hz` frames a second, or `None` if it
/// isn't dead after `limit_s`.
fn hits_to_kill(s: Scenario, hz: f32, limit_s: f32) -> Option<u32> {
    let mut sim = Sim::new(s, 1);
    let at = sim.targets()[0].pos;
    while sim.elapsed() < limit_s {
        sim.step(1.0 / hz, shoot(at));
        if sim.stats().kills == 1 {
            return Some(sim.stats().hits);
        }
    }
    None
}

/// Bug: imported bots healed continuously between shots, so a bot whose regen delay equals the shot
/// interval couldn't be killed. KovaaK's heals in steps of a tenth of a second and checks the timers
/// after the frame's shots, so a hit on the frame a countdown ends wins: 100 hits (measured there).
#[test]
fn imported_bots_with_a_delay_of_one_shot_interval_die_in_health_over_damage_hits() {
    for hz in [60.0, 90.0, 120.0, 144.0, 240.0] {
        for regen in [1000.0, 50.0] {
            let hits = hits_to_kill(healer(100.0, regen, 0.01, 100.0, 1.0), hz, 30.0);
            assert_eq!(hits, Some(100), "regen {regen} at {hz} frames a second");
        }
    }
}

/// With no delay a step lands every 0.1 s whatever the hits (KovaaK's: 190 hits, 913 hits; continuous
/// healing gives 200 and 1000).
#[test]
fn imported_bots_with_no_delay_heal_in_steps_that_hits_dont_pause() {
    for (regen, lo, hi) in [(50.0, 188, 196), (90.0, 900, 920)] {
        for hz in [60.0, 120.0, 240.0] {
            let hits = hits_to_kill(healer(100.0, regen, 0.0, 100.0, 1.0), hz, 60.0).unwrap_or(0);
            assert!((lo..=hi).contains(&hits), "regen {regen} at {hz} frames a second: {hits} hits");
        }
    }
}

/// One step lands after each shot, or several (measured in KovaaK's: 33, 96 and 16 hits).
#[test]
fn imported_bots_heal_a_step_after_each_shot_or_several() {
    for hz in [60.0, 120.0, 240.0] {
        assert_eq!(hits_to_kill(healer(100.0, 20.0, 0.03, 20.0, 5.0), hz, 60.0), Some(33), "{hz} fps");
        assert_eq!(hits_to_kill(healer(100.0, 40.0, 0.05, 10.0, 5.0), hz, 60.0), Some(96), "{hz} fps");
        assert_eq!(hits_to_kill(healer(100.0, 40.0, 0.05, 2.0, 25.0), hz, 120.0), Some(16), "{hz} fps");
    }
}

/// A delay shorter than the shot interval: a step lands on the first frame at or after the countdown's
/// end that has no hit, so the count depends on the frame rate. KovaaK's gave 164 at 120 frames a
/// second; this is pinned at 120 only.
#[test]
fn imported_bots_with_a_delay_under_the_interval_heal_when_a_frame_has_no_hit() {
    let hits = hits_to_kill(healer(100.0, 20.0, 0.005, 100.0, 1.0), 120.0, 60.0).unwrap_or(0);
    assert!((163..=167).contains(&hits), "{hits} hits");
}

/// When the countdown ends the bot gets one step at once, then one every 0.1 s.
#[test]
fn imported_bots_get_a_step_when_the_countdown_ends_then_one_every_tenth_of_a_second() {
    let mut sim = Sim::new(healer(100.0, 50.0, 0.01, 100.0, 1.0), 1);
    let at = sim.targets()[0].pos;
    while sim.stats().hits < 10 {
        sim.step(FRAME, shoot(at));
    }
    let last_hit = sim.elapsed();
    assert!((sim.targets()[0].health - 90.0).abs() < 0.01, "{}", sim.targets()[0].health);
    while sim.elapsed() < last_hit + 0.05 {
        sim.step(FRAME, idle());
    }
    let h = sim.targets()[0].health;
    assert!((h - 95.0).abs() < 0.01, "one step of 5 about 0.05 s after the last hit, got {h}");
    while sim.elapsed() < last_hit + 0.12 {
        sim.step(FRAME, idle());
    }
    let h = sim.targets()[0].health;
    assert!((h - 100.0).abs() < 0.01, "full about 0.12 s after the last hit, got {h}");
}

/// Bug: the healing timers were read at the end of a frame and the shots at its start. On frames of
/// uneven length a countdown could then run out one frame before the shot that should have restarted
/// it, and a bot whose delay equals the shot interval got a step it never gets in KovaaK's. Both are
/// read at the start of the frame now, so the shot that is due always comes first.
#[test]
fn uneven_frames_dont_let_a_step_in_before_the_shot_thats_due() {
    // Frame lengths that go up and down, as a real frame rate does.
    let lengths = [1.0 / 200.0, 1.0 / 90.0, 1.0 / 144.0, 1.0 / 60.0, 1.0 / 240.0, 1.0 / 110.0, 1.0 / 170.0];
    for regen in [1000.0, 50.0] {
        let mut sim = Sim::new(healer(100.0, regen, 0.01, 100.0, 1.0), 1);
        let at = sim.targets()[0].pos;
        let mut frame = 0;
        while sim.stats().kills == 0 && sim.elapsed() < 30.0 {
            sim.step(lengths[frame % lengths.len()], shoot(at));
            frame += 1;
        }
        assert_eq!((sim.stats().kills, sim.stats().hits), (1, 100), "regen {regen}");
    }
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

use crate::world::ChaseFlight;

/// A chaser on the plane y = 0 (facing up) that starts at x = 0, 10 m ahead, with a
/// reach too short to kick anything and no push.
fn flier(accel: f32, max_speed: f32, drag: f32) -> Pusher {
    Pusher {
        pos: [0.0, 0.0, -10.0],
        facing: [0.0, 1.0, 0.0],
        chases: true,
        flight: Some(ChaseFlight { accel, max_speed, drag }),
        ..Default::default()
    }
}

/// A sim with one ball at `at`, no gravity and one chaser with a flight.
fn chased(at: [f32; 3], p: Pusher) -> Sim {
    pushed(at, 0.0, Vec::new(), vec![p])
}

/// Where the first chaser is.
fn place(sim: &Sim) -> Vec3 {
    sim.fliers[0].unwrap().place
}

/// Steps `secs` seconds at 240 Hz with the ball moved by hand to (`speed` * time, `h`,
/// -10) before each step, and returns how far the chaser is behind it along x.
fn trail_behind(p: Pusher, speed: f32, h: f32, secs: f32) -> f32 {
    let mut sim = chased([0.0, h, -10.0], p);
    for i in 0..(secs * 240.0) as usize {
        sim.targets[0].pos = Vec3::new(speed * i as f32 / 240.0, h, -10.0);
        sim.step(FRAME, idle());
    }
    sim.targets[0].pos.x - place(&sim).x
}

/// A chaser trails a ball that moves steadily at v along a lane h from its plane, and
/// once it keeps pace the line from it to the ball leans off the plane's normal by an
/// angle with sin = drag * v / acceleration, so it is behind by h * tan(angle).
#[test]
fn a_chaser_trails_a_moving_ball_by_the_lag_the_law_gives() {
    let (accel, v, h) = (100.0_f32, 5.0_f32, 5.0_f32);
    for drag in [4.0_f32, 8.0] {
        let want = h * (drag * v / accel).asin().tan();
        let got = trail_behind(flier(accel, 50.0, drag), v, h, 6.0);
        assert!((got - want).abs() < 0.05 * want, "drag {drag}: behind by {got} m, the law gives {want}");
    }
}

/// A chaser starts at its pusher's position and needs time to cross to a far ball.
#[test]
fn a_chaser_starts_where_it_spawns_and_takes_time_to_arrive() {
    let mut sim = chased([20.0, 5.0, -10.0], flier(20.0, 50.0, 1.0));
    assert_eq!(place(&sim), Vec3::new(0.0, 0.0, -10.0));
    run_for(&mut sim, 0.5);
    let x = place(&sim).x;
    assert!(x > 0.2 && x < 20.0, "on its way after 0.5 s: {x}");
    assert!((place(&sim).y, place(&sim).z) == (0.0, -10.0), "it stays on its plane: {}", place(&sim));
    run_for(&mut sim, 0.5);
    assert!(place(&sim).x > x, "still going");
}

/// A low top speed binds: the chaser never flies faster, and it gets there.
#[test]
fn a_chasers_top_speed_binds_when_it_is_low() {
    let mut sim = chased([100.0, 5.0, -10.0], flier(100.0, 3.0, 0.5));
    let mut fastest: f32 = 0.0;
    for _ in 0..480 {
        sim.step(FRAME, idle());
        fastest = fastest.max(sim.fliers[0].unwrap().vel.length());
    }
    assert!(fastest <= 3.0 + 1e-4, "{fastest} m/s");
    assert!(fastest > 2.5, "it reaches its top speed: {fastest} m/s");
}

/// With no target alive the chaser has nothing to aim at and the drag brings it to a
/// stop. Nothing is kicked.
#[test]
fn a_chaser_with_no_target_coasts_to_a_stop() {
    let mut sim = chased([0.0, 3.0, -10.0], Pusher { radius: 50.0, push: 1.0, lift: 1.0, ..flier(100.0, 50.0, 2.0) });
    sim.targets.clear();
    sim.fliers[0].as_mut().unwrap().vel = Vec3::new(5.0, 0.0, 0.0);
    run_for(&mut sim, 0.5);
    assert!(sim.targets.is_empty());
    let (slowed, x) = (sim.fliers[0].unwrap().vel.length(), place(&sim).x);
    assert!(slowed < 5.0 && x > 0.5, "it coasts on: {slowed} m/s, x {x}");
    run_for(&mut sim, 5.0);
    assert!(sim.fliers[0].unwrap().vel.length() < 1e-3, "{}", sim.fliers[0].unwrap().vel);
}

/// The kick comes from where the chaser is: behind a ball it pushes it on along its
/// way. A chaser with no flight is at the point of its plane nearest the ball and has
/// no level push to give a ball straight above it.
#[test]
fn a_chasers_kick_comes_from_its_place() {
    let behind = Pusher { pos: [-3.0, 0.0, -10.0], radius: 50.0, push: 2.0, ..flier(0.001, 50.0, 1.0) };
    let mut sim = chased([0.0, 3.0, -10.0], behind.clone());
    run_for(&mut sim, 0.03);
    assert!((vel(&sim) - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-3, "{}", vel(&sim));
    // The same chaser arriving at once is under the ball and pushes it up only.
    let mut sim = chased([0.0, 3.0, -10.0], Pusher { flight: None, ..behind });
    run_for(&mut sim, 0.03);
    assert!(vel(&sim).length() < 1e-4, "{}", vel(&sim));
}

/// The reach is a sphere centred a radius from the chaser along the line to its ball.
/// This ball is in it, and would be out of reach if the sphere sat along the plane's
/// normal.
#[test]
fn a_chasers_reach_points_at_the_ball() {
    let p = Pusher { radius: 0.5, lift: 1.0, ..flier(0.001, 50.0, 1.0) };
    let ball = Vec3::new(1.3, 0.5, -10.0);
    assert!(!p.reaches(Vec3::new(0.0, 0.0, -10.0), ball, 0.5, 0.0), "out of reach along the normal");
    let mut sim = chased(ball.to_array(), p.clone());
    run_for(&mut sim, 0.03);
    assert!(vel(&sim).y > 0.9, "kicked: {}", vel(&sim));
    // Farther along the line it is out of reach: the centre is 0.5 from the chaser, the
    // reach 1.0 with the ball's radius.
    let mut sim = chased([1.6, 0.5, -10.0], p);
    run_for(&mut sim, 0.03);
    assert_eq!(vel(&sim), Vec3::ZERO);
}

/// A ball behind the plane is still skipped by a chaser with a flight.
#[test]
fn a_chaser_with_a_flight_ignores_a_ball_behind_its_plane() {
    let mut sim = chased([0.0, -0.5, -10.0], Pusher { radius: 50.0, lift: 1.0, ..flier(0.001, 50.0, 1.0) });
    run_for(&mut sim, 0.1);
    assert_eq!(vel(&sim), Vec3::ZERO);
}

/// `start_s` holds the first kick back: none before it, the usual schedule after it.
/// With 0 it is the schedule there always was.
#[test]
fn a_start_delay_holds_the_first_kick_back() {
    let on = |p: Pusher, secs: f32| {
        let mut sim = chased([0.0, 3.0, -10.0], p);
        run_for(&mut sim, secs);
        vel(&sim).length()
    };
    // Pinned, kicking every 0.1 s for 2 m/s, with a reach that holds the ball however
    // far the kicks push it.
    let ball = [0.0, 0.0, -7.0];
    let pin = |start_s: f32, secs: f32| {
        let mut sim = pushed(ball, 0.0, Vec::new(), vec![Pusher { start_s, radius: 50.0, ..pinned() }]);
        run_for(&mut sim, secs);
        vel(&sim).length()
    };
    assert_eq!(pin(0.55, 0.54), 0.0);
    // Kicks at 0.6 and 0.7 are counted by 0.76 s (not the ones at 0.1 to 0.5).
    assert!((pin(0.55, 0.76) - 4.0).abs() < 1e-3);
    assert!((pin(0.0, 0.76) - 14.0).abs() < 1e-3, "7 kicks with no delay");
    // A chaser flies during the wait and is kicked from where it is after it. It lifts 1.
    let c = |start_s: f32| Pusher { radius: 100.0, lift: 1.0, every_s: 0.1, start_s, ..flier(10.0, 50.0, 1.0) };
    assert_eq!(on(c(0.55), 0.54), 0.0);
    assert!((on(c(0.55), 0.76) - 2.0).abs() < 1e-3);
    assert!((on(c(0.0), 0.76) - 7.0).abs() < 1e-3);
    // The count itself.
    let k = |start_s: f32, before: f64, now: f64| {
        super::pushing::kicks(&Pusher { start_s, every_s: 0.1, ..pinned() }, now, before)
    };
    assert_eq!(k(0.0, 0.05, 0.35), 3.0);
    assert_eq!(k(0.55, 0.05, 0.35), 0.0);
    assert_eq!(k(0.55, 0.35, 0.75), 2.0);
    assert_eq!(k(0.55, 0.65, 0.75), 1.0);
}

/// The chaser's path doesn't depend much on the frame length: a long frame is flown in
/// steps of 1/120 s at most, so frames of 1/10 s and of 1/240 s give the same place
/// after a second. One step per 1/10 s frame would put it about a tenth further on.
/// The top speed doesn't bind.
#[test]
fn a_chasers_path_is_the_same_for_different_step_lengths() {
    let at = |frame: f32| {
        let mut sim = chased([20.0, 5.0, -10.0], flier(40.0, 50.0, 8.0));
        for _ in 0..(1.0 / frame).round() as usize {
            sim.step(frame, idle());
        }
        place(&sim).x
    };
    let (coarse, fine) = (at(1.0 / 10.0), at(1.0 / 240.0));
    assert!(fine > 1.0, "it has moved: {fine}");
    assert!((coarse - fine).abs() < 0.02 * fine, "{coarse} m at 1/10 s, {fine} m at 1/240 s");
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

/// Bug: a hit was capped at the health the target had left, so the kill shot of 10 on a target with 4 left read
/// 4 done of 10 possible. In KovaaK's 3.9.13 (test maps, 12 November 2026) a killing shot counts its whole damage as
/// done: 10 of 10, scoring 10 per damage, and the damage efficiency is 1.
#[test]
fn a_kill_shot_counts_its_whole_damage_as_done() {
    let mut s = scored(100.0, 0.0);
    s.targets.health = 4.0;
    s.scoring.per_damage = 1.0;
    s.world.as_mut().unwrap().damage_efficiency = true;
    let mut sim = Sim::new(s, 1);
    real_seconds(&mut sim, 0.1);
    click(&mut sim, true);
    let st = sim.stats();
    assert_eq!((st.kills, st.damage_done, st.damage_possible), (1, 10.0, 10.0));
    assert_eq!(st.damage_efficiency(), Some(1.0));
    assert!((sim.score() - 10.0).abs() < 1e-4, "{}", sim.score());
}

/// The same bug over several clicks: 10 a click at a target of health 25 reads 10/10, 20/20, and the kill at 30/30
/// (30 done against 25 health), and the kill's record carries the whole 30.
#[test]
fn three_clicks_at_a_target_of_25_read_10_20_and_30_of_damage() {
    let mut s = scored(100.0, 0.0);
    s.targets.health = 25.0;
    let mut sim = Sim::new(s, 1);
    real_seconds(&mut sim, 0.1);
    for (n, want) in [(1, 10.0), (2, 20.0), (3, 30.0)] {
        click(&mut sim, true);
        let st = sim.stats();
        assert_eq!((st.damage_done, st.damage_possible), (want, want), "after click {n}");
        assert_eq!(st.kills, u32::from(n == 3), "after click {n}");
    }
    assert_eq!(sim.kills.len(), 1);
    assert_eq!(sim.kills[0].damage, 30.0);
}

/// A scenario with no world (every built-in) keeps the cap at the health left, so no built-in score moves.
#[test]
fn a_built_in_scenario_still_caps_a_hit_at_the_health_left() {
    let mut s = no_countdown(Scenario::default());
    s.targets.health = 4.0;
    s.weapon.damage = 10.0;
    s.scoring.per_damage = 1.0;
    s.scoring.per_kill = 0.0;
    assert!(s.world.is_none());
    let mut sim = Sim::new(s, 1);
    let at = sim.targets()[0].pos;
    sim.step(FRAME, shoot(at));
    let st = sim.stats();
    assert_eq!((st.kills, st.damage_done, st.damage_possible), (1, 4.0, 10.0));
    assert!((sim.score() - 4.0).abs() < 1e-4, "{}", sim.score());
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

// Plan K15a: an ability that costs the bot health.

/// A ball that hops straight up and pays `cost` health for each hop, with a charge back every
/// 0.5 s, so about two hops a second. It comes back 0.5 s after it leaves. A kill is worth 5
/// points, a hit 1 and a point of damage 1.
fn hopper(health: f32, cost: f32) -> Scenario {
    use crate::motion::{Abilities, Ability, Motion};
    let hop = Ability { charge_s: 0.5, up_speed: 4.0, health_cost: cost, ..Ability::default() };
    let abilities =
        Abilities { press_every_s: 0.05, chance: 1.0, window_s: [0.05, 0.05], list: vec![hop], ..Default::default() };
    let motion = Motion { gravity: 8.0, abilities: Some(abilities), ..Default::default() };
    let mut s = on_a_floor(Vec::new(), Some(motion));
    s.targets.health = health;
    s.targets.respawn_delay_s = 0.5;
    s.scoring =
        crate::scenario::Scoring { per_kill: 5.0, per_hit: 1.0, per_damage: 1.0, per_miss: 0.0, per_timeout: 0.0 };
    s.weapon.damage = 1.0;
    s
}

fn uses_of(sim: &Sim) -> u32 {
    sim.targets().first().and_then(|t| t.bot.as_ref()).map_or(0, |b| b.ability_uses())
}

/// Plays until the first bot goes away, up to `limit_s`. Gives how many uses it had made by then
/// (counting the one of the frame it left in), and all the events up to and including that frame.
fn until_it_leaves(sim: &mut Sim, limit_s: f32) -> Option<(u32, Vec<Event>)> {
    let mut all = Vec::new();
    for _ in 0..(limit_s / FRAME) as usize {
        let before = uses_of(sim);
        let events = sim.step(FRAME, idle());
        let left = events.iter().any(|e| matches!(e, Event::TimedOut { .. } | Event::Killed { .. }));
        all.extend(events);
        if left {
            return Some((before + 1, all));
        }
    }
    None
}

/// Each use takes its cost off the bot's health at once: 100 health and a cost of 40 is 60 after
/// the first use and 20 after the second.
#[test]
fn each_use_takes_its_cost_off_the_bots_health() {
    let mut sim = Sim::new(hopper(100.0, 40.0), 1);
    for (uses, health) in [(1, 60.0), (2, 20.0)] {
        while uses_of(&sim) < uses {
            sim.step(FRAME, idle());
        }
        assert_eq!(sim.targets()[0].health, health, "after use {uses}");
    }
    assert_eq!(sim.stats().timeouts, 0);
}

/// A bot dies on the use that takes it to exactly 0 or below it, and a bot that can't pay for a
/// single use dies on its first, in every life.
#[test]
fn a_bot_dies_of_the_use_that_spends_its_last_health() {
    // (health, cost, the use it dies on)
    for (health, cost, dies_on) in [(80.0, 40.0, 2), (100.0, 25.0, 4), (100.0, 40.0, 3), (30.0, 40.0, 1)] {
        let mut sim = Sim::new(hopper(health, cost), 1);
        for life in 1..=2 {
            let (uses, events) = until_it_leaves(&mut sim, 10.0).unwrap_or_else(|| panic!("{health}/{cost}"));
            assert_eq!(uses, dies_on, "{health} health, cost {cost}, life {life}");
            assert!(events.iter().any(|e| matches!(e, Event::TimedOut { .. })), "{events:?}");
            assert_eq!(sim.stats().timeouts, life);
        }
    }
}

/// The death is a target that went away: no kill, no score, no damage, and it doesn't count toward
/// a kill limit, however many times it happens.
#[test]
fn a_bot_that_dies_of_its_cost_gives_no_kill_score_or_damage() {
    let mut s = hopper(100.0, 40.0);
    s.timing.kill_limit = 1;
    let mut sim = Sim::new(s, 1);
    let mut events = Vec::new();
    for _ in 0..(10.0 / FRAME) as usize {
        events.extend(sim.step(FRAME, idle()));
    }
    let st = sim.stats();
    assert!(st.timeouts >= 3, "{st:?}");
    assert_eq!((st.kills, st.hits, st.shots), (0, 0, 0));
    assert_eq!((st.damage_done, st.damage_possible), (0.0, 0.0));
    assert_eq!(sim.score(), 0.0);
    assert!(events.iter().any(|e| matches!(e, Event::TimedOut { .. })));
    assert!(!events.iter().any(|e| matches!(e, Event::Killed { .. })));
    assert_eq!(sim.phase(), Phase::Running, "a kill limit of 1 isn't reached");
    assert!(sim.telemetry.targets.iter().all(|t| t.outcome == Outcome::TimedOut));
}

/// The bot comes back after its respawn delay at full health, with its abilities fresh.
#[test]
fn a_bot_that_dies_of_its_cost_comes_back_at_full_health_after_its_delay() {
    let mut sim = Sim::new(hopper(100.0, 40.0), 1);
    until_it_leaves(&mut sim, 10.0).unwrap();
    let left_at = sim.elapsed();
    assert!(sim.targets().is_empty());
    while sim.elapsed() < left_at + 0.45 {
        sim.step(FRAME, idle());
    }
    assert!(sim.targets().is_empty(), "still away before its 0.5 s are over");
    while sim.targets().is_empty() {
        sim.step(FRAME, idle());
    }
    assert!((sim.elapsed() - (left_at + 0.5)).abs() < 0.02, "back after its delay: {}", sim.elapsed() - left_at);
    assert_eq!(sim.targets()[0].health, 100.0);
    assert_eq!(uses_of(&sim), 0, "it starts over");
}

/// A shot at a bot its cost has weakened is a kill, and the damage done is the shot's.
#[test]
fn a_shot_that_finishes_a_bot_its_cost_weakened_is_a_kill() {
    let mut s = hopper(80.0, 40.0);
    s.weapon.damage = 40.0;
    let mut sim = Sim::new(s, 1);
    while uses_of(&sim) < 1 {
        sim.step(FRAME, idle());
    }
    assert_eq!(sim.targets()[0].health, 40.0);
    let at = sim.targets()[0].pos;
    let events = sim.step(FRAME, shoot(at));
    assert!(events.iter().any(|e| matches!(e, Event::Killed { .. })), "{events:?}");
    let st = sim.stats();
    assert_eq!((st.kills, st.timeouts, st.hits, st.damage_done), (1, 0, 1, 40.0));
    // 5 for the kill, 1 for the hit and 40 for the damage dealt: the cost's 40 isn't counted.
    assert_eq!(sim.score(), 46.0);
}

/// A bot that can't die pays nothing: not when the world's bots are invincible, and not when its own
/// kind is.
#[test]
fn a_bot_that_cant_die_pays_nothing_for_its_ability() {
    for per_kind in [false, true] {
        let mut s = hopper(100.0, 40.0);
        let w = s.world.as_mut().unwrap();
        if per_kind {
            let motion = w.motion.take();
            w.kinds = vec![crate::world::WorldKind { health: 100.0, invincible: true, motion, ..Default::default() }];
            w.slots = vec![vec![(0, 1.0)]];
        } else {
            w.invincible = true;
        }
        let mut sim = Sim::new(s, 1);
        for _ in 0..(5.0 / FRAME) as usize {
            sim.step(FRAME, idle());
        }
        assert!(uses_of(&sim) >= 5, "it kept hopping (per kind {per_kind})");
        assert_eq!((sim.stats().timeouts, sim.targets()[0].health), (0, 100.0), "per kind {per_kind}");
    }
}

/// The cost isn't a hit: it doesn't restart the wait before a bot's health regenerates. A bot shot at
/// the start has its regen begin 1 s later, and the hop at 0.05 s doesn't push that back.
#[test]
fn the_cost_doesnt_restart_the_regen_delay() {
    let mut s = hopper(100.0, 40.0);
    s.targets.regen_per_s = 100.0;
    s.targets.regen_delay_s = 1.0;
    // Only the first hop: the charge doesn't come back.
    if let Some(a) = s.world.as_mut().and_then(|w| w.motion.as_mut()).and_then(|m| m.abilities.as_mut()) {
        a.list[0].charge_s = 100.0;
    }
    let mut sim = Sim::new(s, 1);
    let at = sim.targets()[0].pos;
    sim.step(FRAME, shoot(at));
    while sim.elapsed() < 0.5 {
        sim.step(FRAME, idle());
    }
    assert_eq!(uses_of(&sim), 1);
    assert_eq!(sim.targets()[0].health, 59.0, "100, a hit of 1 and a cost of 40, and no regen yet");
    while sim.elapsed() < 1.04 {
        sim.step(FRAME, idle());
    }
    // Imported bots heal in steps of a tenth of a second: one step of 10 (100 a second) came about 1 s
    // after the shot, so 59 + 10. If the hop at 0.05 s had restarted the wait it would still be 59.
    assert_eq!(sim.targets()[0].health, 69.0);
}

/// A bot that can't be hurt, standing still with one hop it uses only when hit: a reaction to damage
/// wasn't played. Hits on it count with the weapon's damage.
fn flincher(threshold: f32) -> Scenario {
    use crate::motion::{Abilities, Ability, Motion, Reaction};
    let reaction = Reaction { threshold, reset_s: 1.0, ignore_chance: 0.0, delay_s: [0.05, 0.05] };
    let hop =
        Ability { max_charges: 9, spawn_charges: 9, up_speed: 4.0, reaction: Some(reaction), ..Ability::default() };
    let abilities =
        Abilities { press_every_s: 0.05, chance: 0.0, window_s: [1.0, 1.0], list: vec![hop], ..Default::default() };
    let motion = Motion { gravity: 8.0, abilities: Some(abilities), ..Default::default() };
    let mut s = on_a_floor(Vec::new(), Some(motion));
    s.targets.health = 1000.0;
    s.weapon.damage = 10.0;
    s
}

/// Plan K15d, through the sim: with a threshold of 30 under shots of 10, the reaction follows the third
/// hit and not the first two, whether the bot can't be hurt through the world or through its kind.
#[test]
fn a_bot_that_cant_be_hurt_reacts_to_the_third_hit_of_ten() {
    for per_kind in [false, true] {
        let mut s = flincher(30.0);
        let w = s.world.as_mut().unwrap();
        if per_kind {
            let motion = w.motion.take();
            w.kinds = vec![crate::world::WorldKind { health: 1000.0, invincible: true, motion, ..Default::default() }];
            w.slots = vec![vec![(0, 1.0)]];
        } else {
            w.invincible = true;
        }
        let mut sim = Sim::new(s, 1);
        for hit in 1..=3u32 {
            let at = sim.targets()[0].pos;
            sim.step(FRAME, shoot(at));
            // A quarter of a second is well past the reaction's wait of 0.05 s.
            for _ in 0..(0.25 / FRAME) as usize {
                sim.step(FRAME, idle());
            }
            assert_eq!(sim.stats().hits, hit, "per kind {per_kind}");
            assert_eq!(uses_of(&sim), u32::from(hit == 3), "after hit {hit}, per kind {per_kind}");
        }
        assert_eq!(sim.stats().damage_done, 30.0, "hits on a bot that can't be hurt count with the weapon's damage");
    }
}

// What a bot's abilities keep when it respawns. All of these guard against one bug: the abilities of a bot
// that came back started all their timers again, the start block and the frequency windows among them.

/// A still bot of this kind: one ability that moves nothing (so that a use is all that happens), a key that
/// ticks every `tick` s after `block` s, and a health cost per use.
fn pressing_kind(block: f32, tick: f32, cost: f32) -> crate::motion::Motion {
    use crate::motion::{Abilities, Ability, Motion};
    let ability = Ability { max_charges: 99, spawn_charges: 99, health_cost: cost, ..Ability::default() };
    let abilities = Abilities {
        press_every_s: tick,
        chance: 1.0,
        window_s: [1.0, 1.0],
        start_block_s: block,
        list: vec![ability],
        ..Default::default()
    };
    Motion { abilities: Some(abilities), ..Default::default() }
}

/// What a run of a sim shows of each bot: the time (in seconds of play) at which it appeared, and at which it
/// first used an ability.
#[derive(Default)]
struct Lives {
    seen: Vec<(u64, f32, Option<f32>)>,
}

impl Lives {
    fn look(&mut self, sim: &Sim) {
        for t in sim.targets() {
            let uses = t.bot.as_ref().map_or(0, |b| b.ability_uses());
            match self.seen.iter_mut().find(|s| s.0 == t.id) {
                None => self.seen.push((t.id, sim.elapsed(), (uses > 0).then(|| sim.elapsed()))),
                Some(s) if s.2.is_none() && uses > 0 => s.2 = Some(sim.elapsed()),
                Some(_) => {}
            }
        }
    }

    fn life(&self, id: u64) -> &(u64, f32, Option<f32>) {
        self.seen.iter().find(|s| s.0 == id).unwrap_or_else(|| panic!("no bot {id}: {:?}", self.seen))
    }

    /// When the bot with this id first used an ability, in seconds of play; infinity if it hadn't by the
    /// time the run ended.
    fn first_use(&self, id: u64) -> f32 {
        self.life(id).2.unwrap_or(f32::INFINITY)
    }

    /// The same, counted from when the bot appeared.
    fn first_use_after_appearing(&self, id: u64) -> f32 {
        self.first_use(id) - self.life(id).1
    }
}

/// Plays idle frames until the sim reaches `at`.
fn idle_until(sim: &mut Sim, lives: &mut Lives, at: f32) {
    while sim.elapsed() < at {
        sim.step(FRAME, idle());
        lives.look(sim);
    }
}

/// L: a kill. One bot with an ability (block 3.0, a tick every 0.1 s), a respawn delay of 0.5. It is shot at
/// 4.0 s. The new bot's first use comes about 0.1 s after it appears; the first bot's came 3.0 s after.
#[test]
fn a_bot_that_comes_back_after_a_kill_doesnt_wait_out_the_start_block_again() {
    let mut s = on_a_floor(Vec::new(), Some(pressing_kind(3.0, 0.1, 0.0)));
    s.targets.health = 10.0;
    s.targets.respawn_delay_s = 0.5;
    s.weapon.damage = 10.0;
    let mut sim = Sim::new(s, 1);
    let mut lives = Lives::default();
    lives.look(&sim);
    idle_until(&mut sim, &mut lives, 4.0);
    let at = sim.targets()[0].pos;
    sim.step(FRAME, shoot(at));
    assert_eq!(sim.stats().kills, 1);
    idle_until(&mut sim, &mut lives, 6.0);
    let first = lives.first_use_after_appearing(1);
    assert!((3.0..3.05).contains(&first), "the first bot waited out the block: {first}");
    let second = lives.first_use_after_appearing(2);
    assert!((0.08..0.14).contains(&second), "the next one used at its first tick: {second}");
}

/// M: a bot that goes by itself. Health 20, an ability that costs 10, block 3.0, a tick of 1.0, a respawn
/// delay of 0.5: life one uses at 3.0 and 4.0 and leaves as a timeout; life two appears at 4.5, uses at 5.5
/// and 6.5 and leaves; and so on.
#[test]
fn a_bot_that_goes_by_itself_comes_back_with_its_block_over() {
    let mut s = on_a_floor(Vec::new(), Some(pressing_kind(3.0, 1.0, 10.0)));
    s.targets.health = 20.0;
    s.targets.respawn_delay_s = 0.5;
    let mut sim = Sim::new(s, 1);
    let mut left = Vec::new();
    while sim.elapsed() < 12.0 {
        let events = sim.step(FRAME, idle());
        if events.iter().any(|e| matches!(e, Event::TimedOut { .. })) {
            left.push(sim.elapsed());
        }
    }
    let want = [4.0, 6.5, 9.0, 11.5];
    let fits = left.len() == want.len() && left.iter().zip(want).all(|(l, w)| (w - 0.005..w + 0.05).contains(l));
    assert!(fits, "timeouts at {left:?}, wanted about {want:?}");
}

/// A block still running when a bot is killed ends at its time in the run: the time the bot was away counts.
/// Block 3.0, a tick every 0.1 s, a respawn delay of 0.5: the bot is shot at 1.0 s, the next one appears at
/// about 1.5 s and first uses its ability 3.0 s into the run (3.5 s if the time away were left out).
#[test]
fn a_block_still_running_at_a_kill_ends_at_its_time_in_the_run() {
    let mut s = on_a_floor(Vec::new(), Some(pressing_kind(3.0, 0.1, 0.0)));
    s.targets.health = 10.0;
    s.targets.respawn_delay_s = 0.5;
    s.weapon.damage = 10.0;
    let mut sim = Sim::new(s, 1);
    let mut lives = Lives::default();
    lives.look(&sim);
    idle_until(&mut sim, &mut lives, 1.0);
    let at = sim.targets()[0].pos;
    sim.step(FRAME, shoot(at));
    assert_eq!(sim.stats().kills, 1);
    idle_until(&mut sim, &mut lives, 5.0);
    assert_eq!(lives.first_use(1), f32::INFINITY, "the first bot left before its block ended");
    let used = lives.first_use(2);
    assert!((2.99..3.11).contains(&used), "the block ended 3.0 s into the run: {used}");
}

/// Two kinds of still bot with abilities in two slots, one kind to a slot (a respawn delay of 0.5 s, one hit
/// kills).
fn two_slots_of(motions: [crate::motion::Motion; 2]) -> (Sim, Lives) {
    use crate::world::{WorldKind, WorldSpawn};
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.count = 2;
    s.weapon.damage = 10.0;
    let w = s.world.as_mut().unwrap();
    w.spawns = vec![
        WorldSpawn { pos: [-3.0, -1.0, -10.0], ..Default::default() },
        WorldSpawn { pos: [3.0, -1.0, -10.0], ..Default::default() },
    ];
    let kind = |(spawn, motion): (usize, crate::motion::Motion)| WorldKind {
        health: 10.0,
        respawn_delay_s: 0.5,
        spawns: vec![spawn as u32],
        motion: Some(motion),
        ..Default::default()
    };
    w.kinds = motions.into_iter().enumerate().map(kind).collect();
    w.slots = vec![vec![(0, 1.0)], vec![(1, 1.0)]];
    let sim = Sim::new(s, 3);
    let mut lives = Lives::default();
    lives.look(&sim);
    (sim, lives)
}

/// The same, each kind with its own start block and a tick of 0.1 s.
fn two_slots(blocks: [f32; 2]) -> (Sim, Lives) {
    two_slots_of(blocks.map(|block| pressing_kind(block, 0.1, 0.0)))
}

/// N: slots. Two slots with one kind each, the first kind with a block of 3.0 and the second with 6.0. The
/// first slot's bot is killed at about 4.0 and comes back: it uses its ability at its first tick, while the
/// second slot's bot, which never left, still waits until 6.0.
#[test]
fn a_brain_goes_to_the_bot_that_takes_its_slot() {
    let (mut sim, mut lives) = two_slots([3.0, 6.0]);
    idle_until(&mut sim, &mut lives, 4.0);
    let of_kind = |sim: &Sim, kind: u32| sim.targets().iter().find(|t| t.kind() == Some(kind)).map(|t| (t.id, t.pos));
    let (first, at) = of_kind(&sim, 0).unwrap();
    let (other, _) = of_kind(&sim, 1).unwrap();
    sim.step(FRAME, shoot(at));
    assert_eq!(sim.stats().kills, 1);
    idle_until(&mut sim, &mut lives, 7.0);
    let (back, _) = of_kind(&sim, 0).unwrap();
    assert_ne!(back, first);
    let used = lives.first_use_after_appearing(back);
    assert!((0.08..0.14).contains(&used), "the new bot of the first slot used at its first tick: {used}");
    let waited = lives.first_use_after_appearing(other);
    assert!((5.99..6.05).contains(&waited), "the second slot's bot waited out its own block: {waited}");
}

/// A brain goes to its own slot, whichever bot comes back first. Two slots, both kinds with a reuse time of
/// 2.0 s: the first slot's bots try at 0.1 and 2.2, the second's (a block of 1.0) at 1.0 and 3.1. The second
/// slot's bot is killed at about 3.35 and the first's at about 3.55. The first slot is refilled first, at
/// about 3.85, and its bot's next try is the first tick more than 2.0 s after 2.2; the second slot's, at
/// about 4.05, waits until more than 2.0 s after 3.1. Brains handed out in the order they were left would
/// swap the two.
#[test]
fn a_brain_goes_to_its_own_slot_whichever_bot_comes_back_first() {
    let kinds = [0.0, 1.0].map(|block| {
        let mut m = pressing_kind(block, 0.1, 0.0);
        m.abilities.as_mut().unwrap().list[0].reuse_s = 2.0;
        m
    });
    let (mut sim, mut lives) = two_slots_of(kinds);
    let of_kind = |sim: &Sim, kind: u32| sim.targets().iter().find(|t| t.kind() == Some(kind)).map(|t| (t.id, t.pos));
    for (kind, at, kills) in [(1, 3.35, 1), (0, 3.55, 2)] {
        idle_until(&mut sim, &mut lives, at);
        let (_, pos) = of_kind(&sim, kind).unwrap();
        sim.step(FRAME, shoot(pos));
        lives.look(&sim);
        assert_eq!(sim.stats().kills, kills);
    }
    idle_until(&mut sim, &mut lives, 6.0);
    let (first, _) = of_kind(&sim, 0).unwrap();
    let (second, _) = of_kind(&sim, 1).unwrap();
    assert!(lives.life(first).1 < lives.life(second).1, "the first slot was refilled first");
    let used = lives.first_use(first);
    assert!((4.2..4.32).contains(&used), "the first slot's bot went on from the try at 2.2: {used}");
    let used = lives.first_use(second);
    assert!((5.1..5.22).contains(&used), "the second slot's bot went on from the try at 3.1: {used}");
}

/// Two slots that hold the same kind keep a brain each. One kind with a reuse time of 2.0 s fills both slots.
/// The first slot's bot is shot before its first tick, so the bot that takes its place first tries at about
/// 0.65 while the second slot's bot tried at 0.1. The second slot's bot is shot at 1.0 and the first's at
/// 1.2. The first slot is refilled first, at about 1.5, and its bot goes on from the try at 0.65: it uses at
/// about 2.7. The second slot's bot, back at about 1.7, goes on from 0.1 and uses at about 2.2. Brains
/// matched by kind alone would swap the two.
#[test]
fn two_slots_of_one_kind_keep_a_brain_each() {
    use crate::world::{WorldKind, WorldSpawn};
    let mut s = on_a_floor(Vec::new(), None);
    s.targets.count = 2;
    s.weapon.damage = 10.0;
    let mut motion = pressing_kind(0.0, 0.1, 0.0);
    motion.abilities.as_mut().unwrap().list[0].reuse_s = 2.0;
    let w = s.world.as_mut().unwrap();
    w.spawns = vec![
        WorldSpawn { pos: [-3.0, -1.0, -10.0], ..Default::default() },
        WorldSpawn { pos: [3.0, -1.0, -10.0], ..Default::default() },
    ];
    let kind = WorldKind {
        health: 10.0,
        respawn_delay_s: 0.5,
        spawns: vec![0, 1],
        motion: Some(motion),
        ..Default::default()
    };
    w.kinds = vec![kind];
    w.slots = vec![vec![(0, 1.0)], vec![(0, 1.0)]];
    let mut sim = Sim::new(s, 3);
    let mut lives = Lives::default();
    lives.look(&sim);
    let in_slot = |sim: &Sim, slot: u32| sim.targets().iter().find(|t| t.slot == slot).map(|t| (t.id, t.pos));
    for (slot, at, kills) in [(0, 0.05, 1), (1, 1.0, 2), (0, 1.2, 3)] {
        idle_until(&mut sim, &mut lives, at);
        let (_, pos) = in_slot(&sim, slot).unwrap();
        sim.step(FRAME, shoot(pos));
        lives.look(&sim);
        assert_eq!(sim.stats().kills, kills);
    }
    idle_until(&mut sim, &mut lives, 4.0);
    let (first, _) = in_slot(&sim, 0).unwrap();
    let (second, _) = in_slot(&sim, 1).unwrap();
    assert!(lives.life(first).1 < lives.life(second).1, "the first slot was refilled first");
    let used = lives.first_use(first);
    assert!((2.55..2.85).contains(&used), "the first slot's bot went on from the try at 0.65: {used}");
    let used = lives.first_use(second);
    assert!((2.05..2.3).contains(&used), "the second slot's bot went on from the try at 0.1: {used}");
}

/// A slot whose kinds take turns keeps each kind's brain for the next bot of that kind (not measured: a
/// decision). One slot, two kinds in turn, both with a block of 2.0 and a respawn delay of 0.5. The lives
/// are shot at 1.0, 2.5 and 4.0. The first two never use the ability: each kind's block counts from its own
/// first bot (0 and about 1.5). The third and fourth are their kinds' second bots, and use it at their first
/// tick.
#[test]
fn a_slot_whose_kinds_take_turns_keeps_each_kinds_brain() {
    use crate::world::WorldKind;
    let mut s = on_a_floor(Vec::new(), None);
    s.weapon.damage = 10.0;
    let w = s.world.as_mut().unwrap();
    let kind = || WorldKind {
        health: 10.0,
        respawn_delay_s: 0.5,
        spawns: vec![0],
        motion: Some(pressing_kind(2.0, 0.1, 0.0)),
        ..Default::default()
    };
    w.kinds = vec![kind(), kind()];
    w.slots = vec![vec![(0, 1.0), (1, 1.0)]];
    w.rotations = vec![Some(0)];
    let mut sim = Sim::new(s, 3);
    let mut lives = Lives::default();
    lives.look(&sim);
    let mut kinds = Vec::new();
    for (at, kills) in [(1.0, 1), (2.5, 2), (4.0, 3)] {
        idle_until(&mut sim, &mut lives, at);
        let (kind, pos) = (sim.targets()[0].kind(), sim.targets()[0].pos);
        kinds.push(kind);
        sim.step(FRAME, shoot(pos));
        lives.look(&sim);
        assert_eq!(sim.stats().kills, kills);
    }
    idle_until(&mut sim, &mut lives, 5.0);
    kinds.push(sim.targets()[0].kind());
    assert_eq!(kinds, vec![Some(0), Some(1), Some(0), Some(1)]);
    for (id, used) in [(1, false), (2, false), (3, true), (4, true)] {
        let after = lives.first_use_after_appearing(id);
        let fits = if used { (0.08..0.14).contains(&after) } else { after == f32::INFINITY };
        assert!(fits, "bot {id} first used its ability {after} s after it appeared");
    }
}

/// O: no slots, two targets up. Brains are handed out first in, first out, and the list never holds more than
/// two.
#[test]
fn brains_are_handed_out_first_in_first_out_and_never_pile_up() {
    use crate::world::WorldSpawn;
    let mut s = on_a_floor(Vec::new(), Some(pressing_kind(2.0, 0.1, 0.0)));
    s.targets.count = 2;
    s.targets.health = 10.0;
    s.targets.respawn_delay_s = 0.5;
    s.weapon.damage = 10.0;
    s.timing.duration_s = 60.0;
    s.world.as_mut().unwrap().spawns = vec![
        WorldSpawn { pos: [-3.0, -1.0, -10.0], ..Default::default() },
        WorldSpawn { pos: [3.0, -1.0, -10.0], ..Default::default() },
    ];
    let mut sim = Sim::new(s, 3);
    let mut lives = Lives::default();
    assert!(sim.brains.is_empty());
    // One is killed at 1.0 s and the other at 1.2 s. The first comes back at 1.5 s and takes the older brain.
    for (at, in_list) in [(1.0, 1), (1.2, 2)] {
        idle_until(&mut sim, &mut lives, at);
        let pos = sim.targets()[0].pos;
        sim.step(FRAME, shoot(pos));
        assert_eq!(sim.brains.len(), in_list);
    }
    assert!((sim.brains[0].at - 1.0).abs() < 0.02 && (sim.brains[1].at - 1.2).abs() < 0.02);
    while sim.targets().is_empty() {
        sim.step(FRAME, idle());
    }
    assert_eq!(sim.brains.len(), 1, "the first bot back took one");
    assert!((sim.brains[0].at - 1.2).abs() < 0.02, "the one that stayed is the newer: {}", sim.brains[0].at);
    // Kill whichever is first up every 0.3 s for a while: the list never holds more than two.
    let mut most = 0;
    while sim.elapsed() < 50.0 {
        for _ in 0..72 {
            sim.step(FRAME, idle());
            most = most.max(sim.brains.len());
        }
        if let Some(t) = sim.targets().first() {
            let pos = t.pos;
            sim.step(FRAME, shoot(pos));
            most = most.max(sim.brains.len());
        }
    }
    assert!(sim.stats().kills > 50, "{}", sim.stats().kills);
    assert!(most <= 2, "{most}");
}

// Plan K8a: bots that blow themselves up (a weapon ability with a fuse, see `Fuse`). All numbers are made up.

/// The motion of a fuse bot: a key that ticks every 0.1 s with no block, and one weapon ability whose weapon
/// fires every `shot_every` s, blows up `burn` s after a shot and takes `damage` health off the bot.
fn fuse_motion(shot_every: f32, burn: f32, damage: f32) -> crate::motion::Motion {
    use crate::motion::{Abilities, Ability, Fuse, Motion};
    let fuse = Fuse { shot_every_s: shot_every, burn_s: burn, damage };
    let ability = Ability { fuse: Some(fuse), ..Ability::default() };
    let abilities = Abilities {
        press_every_s: 0.1,
        chance: 1.0,
        window_s: [1.0, 1.0],
        start_block_s: 0.0,
        list: vec![ability],
        ..Default::default()
    };
    Motion { stays_put: true, abilities: Some(abilities), ..Default::default() }
}

/// One fuse ball with `health`, coming back `respawn` s after it leaves. A kill pays 5 and a timeout nothing,
/// and the player's shot kills it in one hit.
fn fuse_scenario(health: f32, respawn: f32, motion: crate::motion::Motion) -> Scenario {
    let mut s = on_a_floor(Vec::new(), Some(motion));
    s.timing.duration_s = 60.0;
    s.targets.health = health;
    s.targets.respawn_delay_s = respawn;
    s.scoring =
        crate::scenario::Scoring { per_kill: 5.0, per_hit: 1.0, per_damage: 1.0, per_miss: 0.0, per_timeout: 0.0 };
    s.weapon.damage = health;
    s
}

/// What happened in a run and when, in seconds of play: "spawn", "hurt" (the bot up has less health than a
/// frame ago), "timeout" and "kill".
fn log_of(
    sim: &mut Sim,
    until: f32,
    mut frame: impl FnMut() -> f32,
    mut input: impl FnMut(&Sim) -> Input,
) -> Vec<(&'static str, f32)> {
    let mut log = Vec::new();
    let mut health = sim.targets().first().map(|t| (t.id, t.health));
    while sim.elapsed() < until {
        let i = input(sim);
        let events = sim.step(frame(), i);
        for e in &events {
            match e {
                Event::Spawned { .. } => log.push(("spawn", sim.elapsed())),
                Event::TimedOut { .. } => log.push(("timeout", sim.elapsed())),
                Event::Killed { .. } => log.push(("kill", sim.elapsed())),
                _ => {}
            }
        }
        let now = sim.targets().first().map(|t| (t.id, t.health));
        if let (Some((id, was)), Some((same, is))) = (health, now)
            && id == same
            && is < was
        {
            log.push(("hurt", sim.elapsed()));
        }
        health = now;
    }
    log
}

/// Frames of 1/240 s, or uneven ones between 1/240 and 1/60 s drawn with the seed.
fn frames_of(seed: Option<u64>) -> Box<dyn FnMut() -> f32> {
    match seed {
        None => Box::new(|| FRAME),
        Some(seed) => {
            let mut rng = Rng::new(seed);
            Box::new(move || 1.0 / (60.0 + 180.0 * rng.next_f32()))
        }
    }
}

const FUSE_FRAMES: [Option<u64>; 4] = [None, Some(1), Some(2), Some(3)];

/// Checks a log against the events wanted, each at its time up to `frames` frames later (and a hair before).
#[track_caller]
fn assert_log(got: &[(&'static str, f32)], want: &[(&'static str, f32)], seed: Option<u64>, frames: f32) {
    let late = frames * if seed.is_none() { FRAME } else { 1.0 / 60.0 } + 0.01;
    let fits = got.len() == want.len()
        && got.iter().zip(want).all(|((g, at), (w, t))| g == w && (-0.005..late).contains(&(at - t)));
    assert!(fits, "frames {seed:?}: happened {got:?}, wanted {want:?}");
}

/// G: timeline A through a respawn of 1.0 s. Health 3, blasts of 2, 0.6 s between shots, 0.05 s from shot to
/// blast, a tick of 0.1 s. The first bot is hurt at 0.15 and leaves as a timeout at 0.75; the next appears at
/// 1.75, fires at once (the weapon was ready long ago), is hurt at 1.90 and leaves at 2.50. It counts as no
/// kill, no shot, no hit and no damage, and no score.
#[test]
fn a_fuse_bot_blows_itself_up_and_comes_back_as_a_timeout() {
    for seed in FUSE_FRAMES {
        let mut sim = Sim::new(fuse_scenario(3.0, 1.0, fuse_motion(0.6, 0.05, 2.0)), 1);
        let log = log_of(&mut sim, 2.6, frames_of(seed), |_| idle());
        let want = [("hurt", 0.15), ("timeout", 0.75), ("spawn", 1.75), ("hurt", 1.90), ("timeout", 2.50)];
        assert_log(&log, &want, seed, 3.0);
        let st = sim.stats();
        assert_eq!(st.timeouts, 2, "{st:?}");
        assert_eq!((st.kills, st.hits, st.shots), (0, 0, 0));
        assert_eq!((st.damage_done, st.damage_possible), (0.0, 0.0));
        assert_eq!(sim.score(), 0.0);
        assert!(sim.telemetry.targets.iter().all(|t| t.outcome == Outcome::TimedOut));
    }
}

/// H: timeline E in the sim. A weapon of 1.5 s fires at 0.1 and 1.6 and the second blast, at 1.65, kills the
/// bot of 3. It comes back at 2.65 with its weapon ready 0.45 s into its life, so its first blast is at 3.15.
/// (Without the carried cooldown it would be at 2.80.)
#[test]
fn a_fuse_bots_cooldown_goes_on_through_its_death_and_respawn() {
    for seed in FUSE_FRAMES {
        let mut sim = Sim::new(fuse_scenario(3.0, 1.0, fuse_motion(1.5, 0.05, 2.0)), 1);
        let log = log_of(&mut sim, 3.25, frames_of(seed), |_| idle());
        let want = [("hurt", 0.15), ("timeout", 1.65), ("spawn", 2.65), ("hurt", 3.15)];
        assert_log(&log, &want, seed, 5.0);
    }
}

/// The player fires once, at the first ball up, at the first frame at or after `at` seconds.
fn shot_at(at: f32) -> impl FnMut(&Sim) -> Input {
    let mut fired = false;
    move |sim: &Sim| match sim.targets().first() {
        Some(t) if sim.elapsed() >= at && !fired => {
            fired = true;
            shoot(t.pos)
        }
        _ => idle(),
    }
}

/// J: the player kills a fuse bot between its shot and its blast. Health 3, a weapon of 1.55 s whose blast
/// comes 0.5 s after the shot; the bot fires at 0.1 and is shot dead at 0.3. That is a kill: the blast that
/// was on its way is dropped, so no timeout follows. The bot that comes back at 1.3 keeps the weapon's
/// cooldown, which counts straight through the time it was away: its weapon is ready at 1.65 and it is hurt at
/// 2.15 (without the cooldown it fires at its first tick and is hurt at 1.9).
#[test]
fn a_kill_between_a_fuse_bots_shot_and_its_blast_is_a_kill_and_the_next_bot_keeps_the_cooldown() {
    for seed in FUSE_FRAMES {
        let mut sim = Sim::new(fuse_scenario(3.0, 1.0, fuse_motion(1.55, 0.5, 2.0)), 1);
        let log = log_of(&mut sim, 2.3, frames_of(seed), shot_at(0.3));
        // The kill and the respawn follow the player's shot, which is a frame or so after 0.3.
        let late = if seed.is_none() { 0.03 } else { 0.07 };
        let want = [("kill", 0.3), ("spawn", 1.3), ("hurt", 2.15)];
        let fits = log.len() == want.len()
            && log.iter().zip(want).all(|((g, at), (w, t))| *g == w && (-0.02..late).contains(&(at - t)));
        assert!(fits, "frames {seed:?}: happened {log:?}, wanted {want:?}");
        let st = sim.stats();
        assert_eq!((st.kills, st.timeouts), (1, 0), "{st:?}");
    }
}

/// K: a fuse bot that can't die isn't hurt (the rule for a health cost): the world's `invincible` keeps the
/// ball at full health, and it never leaves.
#[test]
fn an_invincible_fuse_bot_is_not_hurt() {
    let mut s = fuse_scenario(3.0, 1.0, fuse_motion(0.6, 0.05, 2.0));
    s.world.as_mut().unwrap().invincible = true;
    let mut sim = Sim::new(s, 1);
    let log = log_of(&mut sim, 3.0, frames_of(None), |_| idle());
    assert_eq!(log, vec![], "an invincible bot took nothing");
    assert_eq!(sim.targets()[0].health, 3.0);
    assert_eq!(sim.stats().timeouts, 0);
    assert!(sim.targets()[0].bot.as_ref().unwrap().ability_uses() >= 5, "it did fire");
}

/// I: a fuse bot never moves. A spawn point that overlaps a wall and one with a floor just below the bot's
/// feet (within a step-up height) both leave it where the spawn point is: the same place, every frame, as the
/// same scenario with no motion at all gives. The same ability on a bot that doesn't stay put is lifted out
/// of the wall and dropped onto the floor, so the test can tell.
#[test]
fn a_fuse_bot_stays_exactly_where_it_spawned() {
    let wall = crate::world::WorldMesh {
        positions: vec![[-2.0, -1.5, -10.0], [2.0, -1.5, -10.0], [2.0, 3.0, -10.0], [-2.0, 3.0, -10.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        ..Default::default()
    };
    for (what, spawn_y, walls) in [("a wall", -1.0, vec![wall]), ("a floor just below", -0.9, vec![])] {
        let motion = |stays_put: bool| crate::motion::Motion {
            gravity: 8.0,
            step_up: 0.3,
            stays_put,
            ..fuse_motion(0.6, 0.05, 0.001)
        };
        let make = |motion: Option<crate::motion::Motion>| {
            let mut s = on_a_floor(walls.clone(), motion);
            s.timing.duration_s = 60.0;
            s.targets.health = 1000.0;
            s.world.as_mut().unwrap().spawns[0].pos = [0.0, spawn_y, -10.0];
            Sim::new(s, 1)
        };
        let mut plain = make(None);
        let mut fuse = make(Some(motion(true)));
        let mut loose = make(Some(motion(false)));
        let start = plain.targets()[0].pos;
        assert_eq!(fuse.targets()[0].pos, start, "{what}: at the spawn point");
        for _ in 0..720 {
            for sim in [&mut plain, &mut fuse, &mut loose] {
                sim.step(FRAME, idle());
            }
            assert_eq!(fuse.targets()[0].pos, start, "{what}: moved");
            assert_eq!(plain.targets()[0].pos, start);
        }
        assert_ne!(loose.targets()[0].pos, start, "{what}: a bot that doesn't stay put is moved by it");
        assert!(!fuse.targets()[0].bot.as_ref().unwrap().grounded);
    }
}

/// I, with others: fuse bots that overlap and bump into each other (the motion's default) don't push each
/// other.
#[test]
fn fuse_bots_that_overlap_do_not_push_each_other() {
    use crate::world::WorldSpawn;
    let mut motion = fuse_motion(0.6, 0.05, 0.001);
    motion.bump_bots = true;
    let mut s = fuse_scenario(1000.0, 1.0, motion);
    s.targets.count = 2;
    s.world.as_mut().unwrap().spawns = vec![
        WorldSpawn { pos: [-0.3, -1.0, -10.0], ..Default::default() },
        WorldSpawn { pos: [0.3, -1.0, -10.0], ..Default::default() },
    ];
    let mut sim = Sim::new(s, 1);
    let start: Vec<Vec3> = sim.targets().iter().map(|t| t.pos).collect();
    assert_eq!(start.len(), 2);
    assert!(start[0].distance(start[1]) < 1.0, "they overlap: {start:?}");
    for _ in 0..480 {
        sim.step(FRAME, idle());
        let now: Vec<Vec3> = sim.targets().iter().map(|t| t.pos).collect();
        assert_eq!(now, start);
    }
}

/// I, in mixed company: a fuse bot beside a bot of another kind that bumps into bots. The other kind makes the
/// world push its bots apart, and the fuse bot still stays exactly on its spawn point while the other one is
/// pushed clear of it.
#[test]
fn a_fuse_bot_is_not_pushed_by_a_bot_of_another_kind() {
    use crate::world::{WorldKind, WorldSpawn};
    let mut fuse = fuse_motion(0.6, 0.05, 0.001);
    fuse.bump_bots = true;
    let bumper = crate::motion::Motion { bump_bots: true, ..Default::default() };
    let mut s = fuse_scenario(1000.0, 1.0, fuse.clone());
    s.targets.count = 2;
    let w = s.world.as_mut().unwrap();
    w.spawns = vec![
        WorldSpawn { pos: [-0.3, -1.0, -10.0], ..Default::default() },
        WorldSpawn { pos: [0.3, -1.0, -10.0], ..Default::default() },
    ];
    w.kinds = vec![
        WorldKind { health: 1000.0, motion: Some(fuse), spawns: vec![0], ..Default::default() },
        WorldKind { health: 1000.0, motion: Some(bumper), spawns: vec![1], ..Default::default() },
    ];
    w.slots = vec![vec![(0, 1.0)], vec![(1, 1.0)]];
    let mut sim = Sim::new(s, 1);
    let of = |sim: &Sim, kind: u32| sim.targets().iter().find(|t| t.kind() == Some(kind)).unwrap().pos;
    let (fuse_at, other_at) = (of(&sim, 0), of(&sim, 1));
    assert!(fuse_at.distance(other_at) < 0.7, "they overlap: {fuse_at:?} and {other_at:?}");
    for _ in 0..480 {
        sim.step(FRAME, idle());
        assert_eq!(of(&sim, 0), fuse_at, "the fuse bot moved");
    }
    let apart = of(&sim, 1).distance(fuse_at);
    assert!(apart > 0.9, "the other bot was pushed clear of it: {apart} m apart");
}

// Plan K9: bots with heads. The numbers come from test maps in KovaaK's 3.9.13 (12 November 2026).

fn a_head(radius: f32, offset: f32) -> crate::scenario::Head {
    crate::scenario::Head { radius, offset, ..Default::default() }
}

fn only_head(radius: f32, offset: f32) -> crate::scenario::Head {
    crate::scenario::Head { headshots_only: true, ..a_head(radius, offset) }
}

/// One still bot 100 m ahead in an imported scenario with this body and head, 100 health and a weapon
/// that does 10 per shot. At that distance, aiming at a point on the plane through the bot's middle gives
/// rays that are near enough parallel to read the geometry off.
fn headed_bot(shape: Shape, radius: f32, height: f32, head: Option<crate::scenario::Head>) -> Sim {
    let mut s = no_countdown(Scenario::default());
    s.targets.count = 1;
    s.targets.shape = shape;
    s.targets.radius = radius;
    s.targets.height = height;
    s.targets.health = 100.0;
    s.targets.head = head;
    s.weapon.damage = 10.0;
    s.world = Some(crate::world::World {
        spawns: vec![crate::world::WorldSpawn { pos: [0.0, 0.0, -100.0], ..Default::default() }],
        ..Default::default()
    });
    Sim::new(s, 1)
}

/// The ray from the eye to the point `x` right of and `h` above the bot's middle, on the plane through it.
fn on_the_plane(sim: &Sim, x: f32, h: f32) -> Vec3 {
    (sim.targets()[0].pos + Vec3::new(x, h, 0.0)).normalize()
}

/// The part of the bot a ray at (`x`, `h`) meets first, if it meets it at all.
fn part_at(sim: &Sim, x: f32, h: f32) -> Option<Part> {
    sim.targets()[0].hit_by(on_the_plane(sim, x, h)).map(|(_, part)| part)
}

/// Checks the part met at each height on the bot's centre line (x = 0).
fn assert_centre_line(sim: &Sim, expected: &[(f32, Option<Part>)]) {
    for &(h, part) in expected {
        assert_eq!(part_at(sim, 0.0, h), part, "at {h} above the middle");
    }
}

const BODY: Option<Part> = Some(Part::Body);
const HEAD: Option<Part> = Some(Part::Head);

/// A capsule of radius 1.0 and height 6.0 with a head of 0.6 resting on top (offset 0): the body runs
/// from -3.6 to 2.4 above the character's middle and the head from 2.4 to 3.6 (measured).
#[test]
fn a_head_resting_on_a_capsule_is_its_own_part_above_the_bodys_top() {
    let sim = headed_bot(Shape::Capsule, 1.0, 6.0, Some(a_head(0.6, 0.0)));
    assert_centre_line(&sim, &[(-3.7, None), (-3.5, BODY), (2.3, BODY), (2.5, HEAD), (3.5, HEAD), (3.7, None)]);
}

/// The same with a head sunk 0.5 into the body (offset -0.5): the body runs from -3.35 to 2.65 and the
/// head's middle is at 2.75. Where they overlap the nearer surface wins: on the centre line the body's cap
/// is in front below about 2.49 and the head above it. At height 3.2 the head is 0.397 wide to each side.
#[test]
fn a_head_sunk_into_a_capsule_wins_where_its_surface_is_nearer() {
    let sim = headed_bot(Shape::Capsule, 1.0, 6.0, Some(a_head(0.6, -0.5)));
    assert_centre_line(&sim, &[(2.3, BODY), (2.6, HEAD), (3.3, HEAD), (3.4, None)]);
    assert_eq!(part_at(&sim, 0.35, 3.2), HEAD);
    assert_eq!(part_at(&sim, 0.45, 3.2), None);
    // The switch from body to head is where the two surfaces are equally near: about 2.49. The rays from
    // the eye are not quite parallel at 100 m (a point 0.5 m in front of the plane is 0.5% lower), so the
    // heights tried keep 0.04 clear of it.
    assert_eq!(part_at(&sim, 0.0, 2.45), BODY);
    assert_eq!(part_at(&sim, 0.0, 2.53), HEAD);
}

/// A head 0.5 above the body's top (offset +0.5) leaves a gap that shots pass through: the body runs from
/// -3.85 to 2.15 and the head from 2.65 to 3.85.
#[test]
fn a_gap_between_the_body_and_a_raised_head_lets_shots_through() {
    let sim = headed_bot(Shape::Capsule, 1.0, 6.0, Some(a_head(0.6, 0.5)));
    assert_centre_line(&sim, &[(2.1, BODY), (2.4, None), (2.7, HEAD), (3.8, HEAD), (3.9, None)]);
}

/// A ball of radius 1.5 with a 0.6 head on it: the ball runs from -2.1 to 0.9 and the head from 0.9 to 2.1.
/// A spheroid measured the same whatever its height setting said, so the ball's size is its radius.
#[test]
fn a_ball_with_a_head_is_a_ball_of_its_radius_under_a_head() {
    let sim = headed_bot(Shape::Sphere, 1.5, 6.0, Some(a_head(0.6, 0.0)));
    assert_centre_line(&sim, &[(-2.2, None), (-2.0, BODY), (0.8, BODY), (1.0, HEAD), (2.0, HEAD), (2.2, None)]);
    // The height setting of a ball plays no part: another one is the same.
    let other = headed_bot(Shape::Sphere, 1.5, 1.0, Some(a_head(0.6, 0.0)));
    for h in [-2.2, -2.0, 0.8, 1.0, 2.0, 2.2] {
        assert_eq!(part_at(&sim, 0.0, h), part_at(&other, 0.0, h), "at {h}");
    }
}

/// A head that lies inside its ball (R 1.5, head 1.2 at offset -2.4) is never in front of it: every hit
/// is a body hit.
#[test]
fn a_head_inside_the_ball_is_never_a_head_hit() {
    let sim = headed_bot(Shape::Sphere, 1.5, 3.0, Some(a_head(1.2, -2.4)));
    assert_centre_line(&sim, &[(0.0, BODY), (1.0, BODY), (1.4, BODY)]);
}

/// A ball inside its head (R 0.5, head 1.5 at offset -2.9): the ball runs from -0.55 to 0.45 and the head
/// from -2.45 to 0.55, and every hit is a head hit.
#[test]
fn a_ball_inside_a_big_head_is_always_a_head_hit() {
    let sim = headed_bot(Shape::Sphere, 0.5, 1.0, Some(a_head(1.5, -2.9)));
    assert_centre_line(&sim, &[(0.5, HEAD), (0.0, HEAD), (-2.4, HEAD), (0.6, None), (-2.5, None)]);
}

fn damage_bot(multiplier: f32, health: f32, head: crate::scenario::Head) -> Sim {
    let mut sim = headed_bot(Shape::Capsule, 1.0, 6.0, Some(head));
    sim.scenario.weapon.damage = 10.0;
    sim.scenario.weapon.headshot_multiplier = multiplier;
    sim.targets[0].health = health;
    sim.targets[0].max_health = health;
    sim
}

/// A head hit does the damage times the headshot multiplier; a hit on the body does the plain damage.
#[test]
fn a_head_hit_does_the_damage_times_the_multiplier_and_a_body_hit_the_plain_damage() {
    let mut head = damage_bot(4.0, 100.0, a_head(0.6, 0.0));
    head.step(FRAME, shoot(on_the_plane(&head, 0.0, 3.0)));
    assert_eq!((head.targets()[0].health, head.stats().damage_done), (60.0, 40.0));
    let mut body = damage_bot(4.0, 100.0, a_head(0.6, 0.0));
    body.step(FRAME, shoot(on_the_plane(&body, 0.0, 0.0)));
    assert_eq!((body.targets()[0].health, body.stats().damage_done), (90.0, 10.0));
    // With the multiplier at its default a head hit does the plain damage.
    let mut plain = damage_bot(1.0, 100.0, a_head(0.6, 0.0));
    plain.step(FRAME, shoot(on_the_plane(&plain, 0.0, 3.0)));
    assert_eq!(plain.targets()[0].health, 90.0);
}

/// What damage efficiency divides by is the damage times the multiplier for every shot of a weapon that can
/// headshot, whether any bot has a head or not (three shots at a headless target: 120 possible). With the
/// multiplier at its default it is the plain damage, 30, as before. One head kill of a 40-health target
/// by a damage of 10 and a multiplier of 4 has done all the damage that was possible.
#[test]
fn possible_damage_is_the_damage_times_the_multiplier_with_or_without_a_head() {
    let possible = |multiplier: f32| {
        let mut sim = headed_bot(Shape::Capsule, 1.0, 6.0, None);
        sim.scenario.weapon.headshot_multiplier = multiplier;
        sim.targets[0].health = 1000.0;
        sim.targets[0].max_health = 1000.0;
        for _ in 0..3 {
            sim.step(FRAME, shoot(on_the_plane(&sim, 0.0, 0.0)));
        }
        sim.stats().damage_possible
    };
    assert_eq!((possible(4.0), possible(1.0)), (120.0, 30.0));
    let mut kill = damage_bot(4.0, 40.0, a_head(0.6, 0.0));
    kill.step(FRAME, shoot(on_the_plane(&kill, 0.0, 3.0)));
    assert_eq!(kill.stats().kills, 1);
    assert_eq!(kill.stats().damage_efficiency(), Some(1.0));
}

/// A bot with a head that only takes headshots, with regen, in an imported scenario: 100 health, a weapon
/// that does 10 times 4 on a head, regen 10 a second after a delay of 1 s. A capsule of radius 0.5 and
/// height 2.0 with a head of 0.3 resting on it.
fn headshot_only_bot() -> Sim {
    let mut sim = headed_bot(Shape::Capsule, 0.5, 2.0, Some(only_head(0.3, 0.0)));
    for s in [&mut sim.scenario, &mut sim.current] {
        s.weapon.headshot_multiplier = 4.0;
        s.targets.regen_per_s = 10.0;
        s.targets.regen_delay_s = 1.0;
    }
    sim.targets[0].regen = (10.0, 1.0);
    sim
}

/// A hit on the body of a bot that only takes headshots is a miss in every way: no damage and no hit, the
/// stats and the pace count a miss, the event is a miss, the regen delay isn't restarted (the bot heals when
/// the first hit's delay ends), and the target's own shots and misses go up. A hit on its head does the damage
/// times the multiplier.
#[test]
fn a_body_hit_on_a_headshots_only_bot_is_a_miss_in_every_way() {
    let mut sim = headshot_only_bot();
    run_for(&mut sim, 0.3);
    let head_at = sim.targets()[0].head_middle().unwrap();
    let body_at = sim.targets()[0].body_middle();
    let events = sim.step(FRAME, shoot(head_at));
    assert!(events.contains(&Event::Hit { id: sim.targets()[0].id }), "{events:?}");
    assert_eq!((sim.targets()[0].health, sim.stats().damage_done, sim.stats().hits), (60.0, 40.0, 1));
    let (heal_at, last_hit_at) = (sim.targets()[0].heal_at, sim.targets()[0].last_hit_at);
    run_for(&mut sim, 0.5);
    let events = sim.step(FRAME, shoot(body_at));
    assert_eq!(events, vec![Event::Miss]);
    let t = &sim.targets()[0];
    assert_eq!((t.health, t.hits, t.shots, t.misses), (60.0, 1, 2, 1));
    let st = sim.stats();
    assert_eq!((st.shots, st.hits, st.misses, st.damage_done), (2, 1, 1, 40.0));
    assert_eq!(sim.pace.misses, 1);
    assert_eq!((t.heal_at, t.last_hit_at), (heal_at, last_hit_at), "the regen delay was not restarted");
    assert!(!sim.result().telemetry.shots.last().unwrap().hit);
    // The first hit's delay ends at 1.3 s: by 1.45 s a lump of regen (10 a second for a tenth of a second)
    // has landed. A body hit that restarted the delay would have pushed it past 1.8 s.
    run_for(&mut sim, 0.6);
    assert!(sim.targets()[0].health > 60.5, "a lump of regen landed on time: {}", sim.targets()[0].health);
}

/// A bot that stands still until it is hurt doesn't wake for a body hit on a bot that only takes headshots
/// (no damage reaction), and does for a head hit.
#[test]
fn a_body_hit_on_a_headshots_only_bot_sets_off_no_damage_reaction() {
    let strafing = crate::motion::Motion {
        max_speed: 4.0,
        accel: 40.0,
        still_until_hurt: true,
        dodges: vec![crate::motion::Dodge { strafe_s: [0.5, 0.5], ..Default::default() }],
        ..Default::default()
    };
    let bot = || {
        let mut s = on_a_floor(Vec::new(), Some(strafing.clone()));
        s.world.as_mut().unwrap().spawns[0].pos = [0.0, 1.0, -10.0];
        s.targets.health = 100.0;
        s.weapon.damage = 1.0;
        s.targets.shape = Shape::Capsule;
        s.targets.radius = 0.5;
        s.targets.height = 2.0;
        s.targets.head = Some(only_head(0.3, 0.0));
        Sim::new(s, 1)
    };
    let mut body = bot();
    let start = body.targets()[0].pos;
    let aim = body.targets()[0].body_middle();
    body.step(FRAME, shoot(aim));
    assert_eq!(body.stats().hits, 0);
    run_for(&mut body, 1.0);
    assert_eq!(body.targets()[0].pos, start, "a body hit woke it");
    let mut head = bot();
    let aim = head.targets()[0].head_middle().unwrap();
    head.step(FRAME, shoot(aim));
    run_for(&mut head, 1.0);
    assert!(head.targets()[0].pos.distance(start) > 0.5, "a head hit woke it");
}

/// The sim doesn't pierce: the body of a bot that only takes headshots, in front of another bot's head,
/// stops the shot. It is a miss, and the bot behind is unhurt.
#[test]
fn the_body_of_a_headshots_only_bot_blocks_a_shot_at_the_bot_behind_it() {
    use crate::world::{WorldKind, WorldSpawn};
    let kind = |only: bool, spawn: u32| WorldKind {
        shape: Shape::Capsule,
        radius: 0.5,
        height: 2.0,
        health: 100.0,
        spawns: vec![spawn],
        head: Some(crate::scenario::Head { headshots_only: only, ..a_head(0.3, 0.0) }),
        ..Default::default()
    };
    let play = |front_only: bool| {
        let mut s = no_countdown(Scenario::default());
        s.targets.count = 2;
        s.weapon.damage = 10.0;
        s.weapon.headshot_multiplier = 4.0;
        s.world = Some(crate::world::World {
            spawns: vec![
                WorldSpawn { pos: [0.0, 0.0, -10.0], ..Default::default() },
                WorldSpawn { pos: [0.0, 0.0, -20.0], ..Default::default() },
            ],
            kinds: vec![kind(front_only, 0), kind(false, 1)],
            slots: vec![vec![(0, 1.0)], vec![(1, 1.0)]],
            ..Default::default()
        });
        let mut sim = Sim::new(s, 1);
        // Aim at the back bot's head: the ray crosses the front bot's body on the way.
        let back = sim.targets().iter().find(|t| t.kind() == Some(1)).unwrap().head_middle().unwrap();
        let events = sim.step(FRAME, shoot(back));
        (sim, events)
    };
    let (sim, events) = play(true);
    assert_eq!(events, vec![Event::Miss]);
    assert!(sim.targets().iter().all(|t| t.health == 100.0), "nobody was hurt");
    assert_eq!((sim.stats().hits, sim.stats().misses), (0, 1));
    // The same shot with a front bot that takes body hits hits that bot's body, and not the one behind.
    let (sim, _) = play(false);
    let hurt: Vec<u32> = sim.targets().iter().filter(|t| t.health < 100.0).filter_map(|t| t.kind()).collect();
    assert_eq!(hurt, vec![0]);
}

/// A capsule with a head stands with its body's bottom on the floor, so it is `B + 2r + offset` tall: the
/// middle of a capsule of radius 0.4 and height 1.8 with a head of 0.25 at offset -0.1 comes to rest 1.1
/// above the floor (0.9 without the head), measured. A ball of radius 0.5 with the same head rests with its
/// middle 0.7 above it (a decision: stood as the capsule was measured to).
#[test]
fn a_bot_with_a_head_stands_with_the_bottom_of_its_stack_on_the_floor() {
    let stand = |shape: Shape, radius: f32, height: f32, head: Option<crate::scenario::Head>| {
        let floor = crate::world::WorldMesh {
            positions: vec![[-30.0, 0.0, -30.0], [30.0, 0.0, -30.0], [30.0, 0.0, 5.0], [-30.0, 0.0, 5.0]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..Default::default()
        };
        let mut s = no_countdown(Scenario::default());
        s.targets.count = 1;
        s.targets.shape = shape;
        s.targets.radius = radius;
        s.targets.height = height;
        s.targets.head = head;
        s.world = Some(crate::world::World {
            spawns: vec![crate::world::WorldSpawn { pos: [0.0, 3.0, -10.0], ..Default::default() }],
            meshes: vec![floor],
            motion: Some(crate::motion::Motion { gravity: 8.0, ..Default::default() }),
            ..Default::default()
        });
        let mut sim = Sim::new(s, 1);
        run_for(&mut sim, 2.0);
        assert!(grounded(&sim), "it came to rest");
        bot_y(&sim)
    };
    let head = Some(a_head(0.25, -0.1));
    let (capsule, bare, ball) = (
        stand(Shape::Capsule, 0.4, 1.8, head.clone()),
        stand(Shape::Capsule, 0.4, 1.8, None),
        stand(Shape::Sphere, 0.5, 1.0, head),
    );
    assert!((capsule - 1.1).abs() < 0.01, "{capsule}");
    assert!((bare - 0.9).abs() < 0.01, "{bare}");
    assert!((ball - 0.7).abs() < 0.01, "{ball}");
}

/// A target that shrinks scales its head and its offset with its radius, as its height does (a decision,
/// not measured): at 0.5 of its size the head is 0.5 as wide and sits 0.5 as far in.
#[test]
fn a_shrinking_target_scales_its_head_and_offset() {
    let mut s = no_countdown(Scenario::default());
    s.targets.count = 1;
    s.targets.shape = Shape::Capsule;
    s.targets.radius = 1.0;
    s.targets.height = 6.0;
    s.targets.head = Some(a_head(0.6, -0.5));
    s.targets.health = 100.0;
    s.targets.lifetime_s = 10.0;
    s.targets.shrink_to = 0.2;
    s.world = Some(crate::world::World {
        spawns: vec![crate::world::WorldSpawn { pos: [0.0, 0.0, -100.0], ..Default::default() }],
        ..Default::default()
    });
    let mut sim = Sim::new(s, 1);
    // 6.25 s of 10 takes it from 1.0 to 0.2 + 0.8 * (1 - 0.625) = 0.5 of its size.
    sim.step(6.25, idle());
    let t = &sim.targets()[0];
    assert!((t.radius - 0.5).abs() < 1e-5, "{}", t.radius);
    assert!((t.head_radius().unwrap() - 0.3).abs() < 1e-5);
    assert!((t.stack_height() - (3.0 + 0.6 - 0.25)).abs() < 1e-5, "{}", t.stack_height());
    // The part heights of the case with offset -0.5, halved: the body's top 1.325 and bottom -1.675, the head from
    // 1.075 to 1.675, nothing above 1.675.
    for (h, part) in [(1.15, BODY), (1.3, HEAD), (1.65, HEAD), (1.7, None), (-1.65, BODY), (-1.7, None)] {
        assert_eq!(part_at(&sim, 0.0, h), part, "at {h}");
    }
    assert_eq!(part_at(&sim, 0.175, 1.6), HEAD);
    assert_eq!(part_at(&sim, 0.225, 1.6), None);
}

/// A tick of a beam on a head does the tick's damage times the headshot multiplier, a tick on the body the
/// plain damage, and every tick's possible damage includes the multiplier. On the body of a bot that only
/// takes headshots a tick is a missed tick.
#[test]
fn a_beam_tick_on_a_head_does_the_damage_times_the_multiplier() {
    let beam = |only: bool| {
        let mut s = no_countdown(Scenario::default());
        s.targets.count = 1;
        s.targets.shape = Shape::Capsule;
        s.targets.radius = 1.0;
        s.targets.height = 6.0;
        s.targets.health = 1e6;
        s.targets.head = Some(crate::scenario::Head { headshots_only: only, ..a_head(0.6, 0.0) });
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Always;
        s.weapon.damage_per_s = 120.0;
        s.weapon.headshot_multiplier = 4.0;
        s.world = Some(crate::world::World {
            spawns: vec![crate::world::WorldSpawn { pos: [0.0, 0.0, -100.0], ..Default::default() }],
            ..Default::default()
        });
        Sim::new(s, 1)
    };
    let per_tick = 120.0 / BEAM_TICK_HZ;
    let aim_for = |sim: &mut Sim, h: f32, secs: f32| {
        let aim = on_the_plane(sim, 0.0, h);
        for _ in 0..(secs * 240.0) as usize {
            sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: true });
        }
    };
    let mut sim = beam(false);
    aim_for(&mut sim, 3.0, 0.5);
    let on_head = sim.stats().clone();
    assert!(on_head.shots >= 29, "{on_head:?}");
    assert!((on_head.damage_done - on_head.shots as f32 * per_tick * 4.0).abs() < 1e-2, "{on_head:?}");
    assert!((on_head.damage_possible - on_head.damage_done).abs() < 1e-2, "{on_head:?}");
    aim_for(&mut sim, 0.0, 0.5);
    let st = sim.stats();
    let body_ticks = (st.shots - on_head.shots) as f32;
    assert!((st.damage_done - on_head.damage_done - body_ticks * per_tick).abs() < 1e-2, "{st:?}");
    assert!((st.damage_possible - st.shots as f32 * per_tick * 4.0).abs() < 1e-2, "{st:?}");
    // Headshots only: ticks on the body miss.
    let mut only = beam(true);
    aim_for(&mut only, 0.0, 0.5);
    let st = only.stats();
    assert!(st.shots >= 29, "{st:?}");
    assert_eq!((st.hits, st.misses, st.damage_done), (0, st.shots, 0.0));
    assert_eq!(only.targets()[0].health, 1e6);
}

/// "On target" in the tracking samples is the crosshair on a part that counts: for a bot that only takes
/// headshots, the head alone.
#[test]
fn on_target_for_a_headshots_only_bot_means_on_its_head() {
    let on_target = |h: f32| {
        let mut s = no_countdown(Scenario::default());
        s.targets.count = 1;
        s.targets.shape = Shape::Capsule;
        s.targets.radius = 1.0;
        s.targets.height = 6.0;
        s.targets.health = 1e6;
        s.targets.head = Some(only_head(0.6, 0.0));
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Always;
        s.world = Some(crate::world::World {
            spawns: vec![crate::world::WorldSpawn { pos: [0.0, 0.0, -100.0], ..Default::default() }],
            ..Default::default()
        });
        let mut sim = Sim::new(s, 1);
        let aim = on_the_plane(&sim, 0.0, h);
        for _ in 0..120 {
            sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: true });
        }
        let samples = sim.result().telemetry.samples;
        assert!(samples.len() >= 10, "{}", samples.len());
        samples.iter().map(|s| s.on_target).collect::<Vec<bool>>()
    };
    assert!(on_target(3.0).iter().all(|on| *on), "on the head");
    assert!(on_target(0.0).iter().all(|on| !*on), "on the body");
}

/// The part's geometry in one place: where the body and the head are, how tall the stack is and where to aim.
#[test]
fn a_targets_parts_are_where_the_stack_puts_them() {
    let sim = headed_bot(Shape::Capsule, 1.0, 6.0, Some(a_head(0.6, -0.5)));
    let t = &sim.targets()[0];
    let up = |v: Vec3| v.y - t.pos.y;
    assert!((up(t.body_middle()) + 0.35).abs() < 1e-5);
    assert!((up(t.head_middle().unwrap()) - 2.75).abs() < 1e-5);
    assert!((t.stack_height() - 6.7).abs() < 1e-5 && (t.top() - 3.35).abs() < 1e-5);
    assert_eq!(t.aim_point(), t.body_middle());
    let only = headed_bot(Shape::Capsule, 1.0, 6.0, Some(only_head(0.6, -0.5)));
    assert_eq!(only.targets()[0].aim_point(), only.targets()[0].head_middle().unwrap());
    assert!((only.targets()[0].aim_radius() - 0.6).abs() < 1e-6);
    // A ball with a head stands and bumps as a capsule of the stack's height; one without a head as a ball.
    let ball = headed_bot(Shape::Sphere, 0.5, 1.0, Some(a_head(0.25, -0.1)));
    assert!((ball.targets()[0].bump_half_segment() - 0.2).abs() < 1e-5);
    assert_eq!(headed_bot(Shape::Sphere, 0.5, 1.0, None).targets()[0].bump_half_segment(), 0.0);
    // A head sunk deeper than twice its radius can't make the capsule smaller than the ball.
    let deep = headed_bot(Shape::Sphere, 1.5, 3.0, Some(a_head(1.2, -4.0)));
    assert_eq!(deep.targets()[0].bump_half_segment(), 0.0);
}

// Guards from the mutation check of plan K9: one test for each change of one line that the first tests let through.

/// Guard (mutation check of plan K9): a head sunk deeper than its own diameter has its top below the body's,
/// so the target's top is the body's. A capsule of radius 1 and height 6 with a head of 0.6 sunk 2.0 (the
/// diameter is 1.2) has its body's top 3.4 above the middle and its head's top 2.6 above it.
#[test]
fn a_head_sunk_deeper_than_its_diameter_leaves_the_top_at_the_body() {
    let sim = headed_bot(Shape::Capsule, 1.0, 6.0, Some(a_head(0.6, -2.0)));
    let t = &sim.targets()[0];
    assert!((t.top() - 3.4).abs() < 1e-5, "{}", t.top());
    // A head sunk less deeply than its diameter tops the stack.
    let shallow = headed_bot(Shape::Capsule, 1.0, 6.0, Some(a_head(0.6, -1.0)));
    assert!((shallow.targets()[0].top() - 3.1).abs() < 1e-5, "{}", shallow.targets()[0].top());
}

/// Guard (mutation check of plan K9): a ball with a head is a ball of its radius in the stack, whatever its
/// height setting says. Radius 0.5 and a height setting of 3.0 with a head of 0.25 sunk 0.1: the stack is
/// 1.0 + 0.5 - 0.1 = 1.4 tall, so the middle segment of the capsule it bumps as has a half-length of 0.2.
/// A free mover bounces at half the stack's height, 0.7, where a headless ball does at its radius.
#[test]
fn a_ball_with_a_head_stands_and_bumps_by_its_radius_and_not_its_height_setting() {
    let sim = headed_bot(Shape::Sphere, 0.5, 3.0, Some(a_head(0.25, -0.1)));
    let t = &sim.targets()[0];
    assert!((t.bump_half_segment() - 0.2).abs() < 1e-5, "{}", t.bump_half_segment());
    assert!((t.bump_half_height() - 0.7).abs() < 1e-5, "{}", t.bump_half_height());
    assert!((t.stack_height() - 1.4).abs() < 1e-5, "{}", t.stack_height());
    // The head's middle is (B + offset) / 2 above the position, with B = 2R.
    let up = t.head_middle().unwrap().y - t.pos.y;
    assert!((up - 0.45).abs() < 1e-5, "{up}");
}

/// Guard (mutation check of plan K9): a target that moves freely bounces off the floor and ceiling at half the
/// stack's height when it has a head, and at its body's half height when it has none.
#[test]
fn a_free_mover_with_a_head_bounces_at_half_its_stacks_height() {
    // Capsule of radius 1 and height 6 with a head of 0.6 resting on it: the stack is 7.2 tall.
    let sim = headed_bot(Shape::Capsule, 1.0, 6.0, Some(a_head(0.6, 0.0)));
    assert!((sim.targets()[0].bump_half_height() - 3.6).abs() < 1e-5);
    let bare = headed_bot(Shape::Capsule, 1.0, 6.0, None);
    assert!((bare.targets()[0].bump_half_height() - 3.0).abs() < 1e-5);
    let ball = headed_bot(Shape::Sphere, 1.0, 6.0, None);
    assert!((ball.targets()[0].bump_half_height() - 1.0).abs() < 1e-5);
}

/// Guard (mutation check of plan K9): a target made at a size other than the scenario's radius gets a head
/// scaled in proportion, radius and offset. The scenario's capsule has radius 1.0 and a head of 0.6 sunk 0.5;
/// a target made at half the radius has a head of 0.3 sunk 0.25 and its middle (3 - 0.25) / 2 above its
/// position.
#[test]
fn a_target_made_smaller_than_the_scenarios_radius_gets_a_scaled_head() {
    let mut sim = headed_bot(Shape::Capsule, 1.0, 6.0, Some(a_head(0.6, -0.5)));
    sim.current.targets.radius = 0.5;
    sim.targets.clear();
    sim.spawn(&mut Vec::new());
    let t = &sim.targets()[0];
    assert!((t.radius - 0.5).abs() < 1e-6);
    assert!((t.head_radius().unwrap() - 0.3).abs() < 1e-5, "{:?}", t.head_radius());
    let head = t.head.unwrap();
    assert!((head.radius - 0.3).abs() < 1e-5 && (head.offset + 0.25).abs() < 1e-5, "{head:?}");
    let up = t.head_middle().unwrap().y - t.pos.y;
    assert!((up - 1.375).abs() < 1e-4, "{up}");
}

/// Guard (mutation check of plan K9): a kind's own head setting replaces the scenario's `targets.head`, also
/// when the kind has none.
#[test]
fn a_kind_without_a_head_has_none_whatever_the_scenario_says() {
    use crate::world::{WorldKind, WorldSpawn};
    let mut s = no_countdown(Scenario::default());
    s.targets.count = 1;
    s.targets.head = Some(a_head(0.3, 0.0));
    s.world = Some(crate::world::World {
        spawns: vec![WorldSpawn { pos: [0.0, 0.0, -10.0], ..Default::default() }],
        kinds: vec![WorldKind { shape: Shape::Capsule, radius: 0.5, height: 2.0, health: 100.0, ..Default::default() }],
        slots: vec![vec![(0, 1.0)]],
        ..Default::default()
    });
    let sim = Sim::new(s, 1);
    let t = &sim.targets()[0];
    assert_eq!(t.kind(), Some(0));
    assert!(t.head.is_none() && t.head_radius().is_none(), "{:?}", t.head);
    assert_eq!(t.stack_height(), 2.0);
}

/// Guard (mutation check of plan K9): a bot with a head that is placed standing on a floor as it appears
/// stands on the bottom of its stack at once, before any movement settles it. A capsule of radius 0.4 and
/// height 1.8 with a head of 0.25 sunk 0.1 is 2.2 tall: appearing 0.2 above a floor, within a step-up of 0.3, its
/// middle goes to 1.1. Judged as its body alone (feet 0.9 below the middle) the floor would be 1.3 down, out of
/// reach.
#[test]
fn a_bot_with_a_head_that_appears_near_a_floor_stands_on_the_bottom_of_its_stack() {
    let floor = crate::world::WorldMesh {
        positions: vec![[-30.0, 0.0, -30.0], [30.0, 0.0, -30.0], [30.0, 0.0, 5.0], [-30.0, 0.0, 5.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        ..Default::default()
    };
    let mut s = no_countdown(Scenario::default());
    s.targets.count = 1;
    s.targets.shape = Shape::Capsule;
    s.targets.radius = 0.4;
    s.targets.height = 1.8;
    s.targets.head = Some(a_head(0.25, -0.1));
    s.world = Some(crate::world::World {
        spawns: vec![crate::world::WorldSpawn { pos: [0.0, 1.3, -10.0], ..Default::default() }],
        meshes: vec![floor],
        motion: Some(crate::motion::Motion { gravity: 8.0, step_up: 0.3, ..Default::default() }),
        ..Default::default()
    });
    let sim = Sim::new(s, 1);
    assert!((bot_y(&sim) - 1.1).abs() < 0.01 && grounded(&sim), "{}", bot_y(&sim));
}

/// Two still bots that bump into each other, 1.5 m apart in height and `apart` m apart sideways, each a capsule
/// of radius 0.4 and height 1.0 (a half segment of 0.1 as a body, 0.6 with a head of 0.5 resting on it: a
/// stack of 2.0). Returns how far apart they are sideways after a frame.
fn bumping_pair(head: Option<crate::scenario::Head>, apart: f32) -> f32 {
    let mut s = no_countdown(Scenario::default());
    s.targets.count = 2;
    s.targets.shape = Shape::Capsule;
    s.targets.radius = 0.4;
    s.targets.height = 1.0;
    s.targets.head = head;
    s.world = Some(crate::world::World {
        spawns: vec![
            crate::world::WorldSpawn { pos: [0.0, 0.0, -10.0], ..Default::default() },
            crate::world::WorldSpawn { pos: [5.0, 0.0, -10.0], ..Default::default() },
        ],
        motion: Some(crate::motion::Motion { bump_bots: true, ..Default::default() }),
        ..Default::default()
    });
    let mut sim = Sim::new(s, 1);
    sim.targets[0].pos = Vec3::new(0.0, 0.0, -10.0);
    sim.targets[1].pos = Vec3::new(apart, 1.5, -10.0);
    sim.step(FRAME, idle());
    let (a, b) = (sim.targets()[0].pos, sim.targets()[1].pos);
    Vec3::new(b.x - a.x, 0.0, b.z - a.z).length()
}

/// Guard (mutation check of plan K9): bots with heads bump into each other as capsules of their stack's height.
/// Two bots 1.5 m apart in height and 0.5 m apart sideways touch when each is 2.0 tall (they are pushed apart
/// to 0.8, the sum of their radii) and don't as bodies alone (1.0 tall).
#[test]
fn bots_with_heads_bump_into_each_other_across_the_stacks_extra_height() {
    let with_heads = bumping_pair(Some(a_head(0.5, 0.0)), 0.5);
    assert!((with_heads - 0.8).abs() < 1e-4, "{with_heads}");
    let bare = bumping_pair(None, 0.5);
    assert!((bare - 0.5).abs() < 1e-4, "{bare}");
}

/// Guard (mutation check of plan K9): a pushing helper reaches a bot with a head as far as its stack goes. The
/// pinned pusher's reach is a ball of radius 1 centred at (0, 0, -6); a ball of radius 0.5 with a head of 0.25
/// resting on it has a half segment of 0.25, so a bot 1.65 above that centre is reached (1.65 - 0.25 is inside
/// 1 + 0.5) and, as a ball alone, is not.
#[test]
fn a_pusher_reaches_a_bot_with_a_head_across_the_stacks_extra_height() {
    let kicked = |head: bool| {
        let mut sim = pushed([0.0, 1.65, -6.0], 0.0, Vec::new(), vec![pinned()]);
        if head {
            sim.targets[0].head = Some(crate::sim::geometry::HeadShape { radius: 0.25, offset: 0.0, only: false });
        }
        run_for(&mut sim, 0.125);
        vel(&sim) != Vec3::ZERO
    };
    assert!(kicked(true), "a bot with a head is kicked");
    assert!(!kicked(false), "a ball alone is out of reach");
}

/// Guard (mutation check of plan K9): a beam on the body of a bot that only takes headshots is a beam on
/// nothing, so the bot heals as it would with the beam off it (regen 10 a second after a delay of 1 s that has
/// long passed: it has never been hit). The bot of [`headshot_only_bot`] starts at half its health.
#[test]
fn a_beam_on_the_body_of_a_headshots_only_bot_doesnt_hold_back_its_regen() {
    let mut sim = headshot_only_bot();
    sim.scenario.weapon.kind = WeaponKind::Beam;
    sim.scenario.weapon.trigger = Trigger::Always;
    sim.scenario.weapon.damage_per_s = 120.0;
    sim.targets[0].health = 50.0;
    let aim = sim.targets()[0].body_middle().normalize();
    for _ in 0..120 {
        sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: true });
    }
    let t = &sim.targets()[0];
    assert!(t.health > 54.0 && t.health <= 55.1, "{}", t.health);
    assert_eq!((sim.stats().hits, sim.stats().damage_done), (0, 0.0));
}

/// Guard (mutation check of plan K9): the body of a bot that only takes headshots hides the bot behind it from a
/// beam as well as from a shot. A beam through the front bot's body at the head of a second bot behind it does
/// the second bot nothing.
#[test]
fn the_body_of_a_headshots_only_bot_hides_the_bot_behind_it_from_a_beam() {
    use crate::world::{WorldKind, WorldSpawn};
    let kind = |only: bool, spawn: u32| WorldKind {
        shape: Shape::Capsule,
        radius: 0.5,
        height: 2.0,
        health: 1e6,
        spawns: vec![spawn],
        head: Some(crate::scenario::Head { headshots_only: only, ..a_head(0.3, 0.0) }),
        ..Default::default()
    };
    let play = |front_only: bool| {
        let mut s = no_countdown(Scenario::default());
        s.targets.count = 2;
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Always;
        s.weapon.damage_per_s = 120.0;
        s.weapon.headshot_multiplier = 4.0;
        s.world = Some(crate::world::World {
            spawns: vec![
                WorldSpawn { pos: [0.0, 0.0, -10.0], ..Default::default() },
                WorldSpawn { pos: [0.0, 0.0, -20.0], ..Default::default() },
            ],
            kinds: vec![kind(front_only, 0), kind(false, 1)],
            slots: vec![vec![(0, 1.0)], vec![(1, 1.0)]],
            ..Default::default()
        });
        let mut sim = Sim::new(s, 1);
        // Aim at the back bot's head: the ray crosses the front bot's body on the way.
        let aim = sim.targets().iter().find(|t| t.kind() == Some(1)).unwrap().head_middle().unwrap().normalize();
        for _ in 0..60 {
            sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: true });
        }
        sim
    };
    let sim = play(true);
    assert!(sim.targets().iter().all(|t| t.health == 1e6), "nobody was hurt");
    assert_eq!(sim.stats().hits, 0);
    assert!(sim.stats().misses > 0);
    // With a front bot that takes body hits, the beam hurts that bot and not the one behind.
    let sim = play(false);
    let hurt: Vec<u32> = sim.targets().iter().filter(|t| t.health < 1e6).filter_map(|t| t.kind()).collect();
    assert_eq!(hurt, vec![0]);
}

/// Guard (mutation check of plan K9): the aim resting on the body of a bot that only takes headshots is not
/// "on target" for the bot's track: it never enters it and never reaches it. On its head it does.
#[test]
fn aim_on_the_body_of_a_headshots_only_bot_never_enters_its_track() {
    let mut sim = headshot_only_bot();
    let body = sim.targets()[0].body_middle().normalize();
    for _ in 0..30 {
        sim.step(FRAME, Input { aim: body, fire_pressed: false, fire_held: false });
    }
    let k = &sim.targets()[0].track;
    assert_eq!((k.entries, k.reach_s, k.inside), (0, None, false));
    let head = sim.targets()[0].head_middle().unwrap().normalize();
    sim.step(FRAME, Input { aim: head, fire_pressed: false, fire_held: false });
    let k = &sim.targets()[0].track;
    assert_eq!((k.entries, k.inside), (1, true));
    assert!(k.reach_s.is_some());
}
