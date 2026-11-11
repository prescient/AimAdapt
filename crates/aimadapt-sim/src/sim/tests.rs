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
                        Shape::Capsule => (t.height * t.radius / t.base_radius.max(1e-6) / 2.0).max(t.radius),
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
        theta: knob.h(value) + 0.12 * (0.85f32 / 0.15).ln(),
        slope: 0.12,
        sd: 0.08,
        slope_sd: None,
        days: 1.0,
        fastest,
        last_run: Some((Pick::Near, 0.85)),
    }
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
