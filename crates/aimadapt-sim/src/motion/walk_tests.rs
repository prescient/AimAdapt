//! Tests for the walking law: the top speed of a bot that strafes and goes forward or back at once.
//!
//! Bug: such a bot walked at 1.414 times its top speed across the diagonal. KovaaK's 3.9.13 keeps the
//! top speed in total (measured with filmed test maps, walkers with a `MaxSpeed` of 300 uu/s).

use glam::Vec3;

use super::tests::{DT, arena};
use super::*;

/// A walker with a top speed of 3 m/s whose input never switches during a test.
fn walker(d: Dodge) -> Motion {
    Motion { max_speed: 3.0, accel: 40.0, dodges: vec![d], ..Motion::default() }
}

/// A dodge that strafes and goes forward or back at once, starting on the given sides (-1, 0 or 1),
/// with timers too long to switch.
fn two_way(side: f32, fore: f32) -> Dodge {
    Dodge {
        strafe_s: [100.0, 100.0],
        advance_s: [100.0, 100.0],
        start_side: if side < 0.0 { Start::Minus } else { Start::Plus },
        start_forward: if fore < 0.0 { Start::Minus } else { Start::Plus },
        advance: true,
        ..Dodge::default()
    }
}

/// The flat velocity after 0.3 s, split into the part across (always positive) and the part
/// toward the player (negative when it backs off), and the total.
fn flat_speed(m: &Motion) -> (f32, f32, f32) {
    let mut rng = Rng::new(7);
    let walls = arena();
    let start = Vec3::new(0.0, -1.5, -45.0);
    let mut s = BotState::new(m, &mut rng, start, Vec3::X);
    let mut p = start;
    let mut t = 0.0;
    while t < 0.3 {
        s.step(m, &mut rng, &walls, &mut p, 0.5, 0.0, t, DT);
        t += DT;
    }
    let toward = Vec3::new(-p.x, 0.0, -p.z).normalize();
    let right = toward.cross(Vec3::Y);
    let v = Vec3::new(s.vel.x, 0.0, s.vel.z);
    (v.dot(right).abs(), v.dot(toward), v.length())
}

fn near(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() <= want.abs() * 0.02 + 0.005, "{what}: got {got}, want {want}");
}

#[test]
fn a_bot_that_strafes_and_comes_closer_keeps_its_top_speed_in_total() {
    // Bug: this was 4.24, the top speed times the square root of two.
    let m = walker(two_way(1.0, 1.0));
    let (_, _, total) = flat_speed(&m);
    near(total, 3.0, "total");
}

#[test]
fn the_forward_bias_tilts_the_direction_walked_and_not_the_speed() {
    let m = Motion { forward_bias: 0.25, ..walker(two_way(1.0, 1.0)) };
    let (across, toward, total) = flat_speed(&m);
    near(total, 3.0, "total");
    near(across, 2.910, "across");
    near(toward, 0.728, "toward");
}

#[test]
fn backing_off_on_a_diagonal_slows_by_the_backward_share() {
    let m = Motion { back_mult: 0.8, ..walker(two_way(1.0, -1.0)) };
    let (across, toward, total) = flat_speed(&m);
    near(total, 2.576, "total");
    near(across, 1.821, "across");
    near(toward, -1.821, "toward");
    let m = Motion { forward_bias: 0.25, ..m };
    let (across, toward, _) = flat_speed(&m);
    near(across, 2.499, "across at bias 0.25");
    near(toward, -0.625, "away at bias 0.25");
}

#[test]
fn the_strafe_multiplier_does_nothing_on_a_diagonal() {
    let m = Motion { strafe_mult: 0.5, back_mult: 0.5, ..walker(two_way(1.0, 1.0)) };
    near(flat_speed(&m).2, 3.0, "forward diagonal");
    let m = Motion { strafe_mult: 0.5, back_mult: 0.5, ..walker(two_way(1.0, -1.0)) };
    near(flat_speed(&m).2, 1.939, "back diagonal");
    let m = Motion { strafe_mult: 1.0, back_mult: 0.5, ..walker(two_way(1.0, -1.0)) };
    near(flat_speed(&m).2, 1.939, "back diagonal, strafe 1");
}

/// Guards: one way only, these were right before the fix.
#[test]
fn one_way_only_keeps_its_speed() {
    let strafe_only = Dodge { strafe_s: [100.0, 100.0], ..Dodge::default() };
    let m = Motion { strafe_mult: 0.5, ..walker(strafe_only.clone()) };
    near(flat_speed(&m).2, 1.5, "strafe only, strafe 0.5");
    let m = Motion { forward_bias: 0.25, ..walker(strafe_only) };
    near(flat_speed(&m).2, 3.0, "strafe only, bias 0.25");
    let fore_only = |fore: f32| Dodge { strafe: false, ..two_way(1.0, fore) };
    let m = Motion { back_mult: 0.5, ..walker(fore_only(-1.0)) };
    near(flat_speed(&m).2, 1.5, "back only, back 0.5");
    near(flat_speed(&walker(fore_only(1.0))).2, 3.0, "forward only");
    let m = Motion { forward_bias: 0.25, ..walker(fore_only(1.0)) };
    near(flat_speed(&m).2, 3.0, "forward only, bias 0.25");
}

#[test]
fn a_strafer_pushed_by_its_distance_band_keeps_its_top_speed_in_total() {
    // Bug: outside the band `fore` is set by the band, and the bot walked 4.24 across the diagonal.
    let band = Dodge { strafe_s: [100.0, 100.0], max_distance: 8.0, ..Dodge::default() };
    let (_, toward, total) = flat_speed(&walker(band));
    near(total, 3.0, "total");
    assert!(toward > 1.0, "it is coming closer: {toward}");
}

/// A stored bias below 0 counts as 0 (the importer never writes one, a hand-edited file could).
#[test]
fn a_negative_forward_bias_never_turns_the_bot_round() {
    let m = Motion { forward_bias: -1.0, ..walker(two_way(1.0, 1.0)) };
    let (across, toward, total) = flat_speed(&m);
    near(total, 3.0, "total");
    near(across, 3.0, "across");
    assert!(toward.abs() < 0.05, "it neither comes closer nor backs off: {toward}");
}

#[test]
fn a_forward_bias_of_one_is_not_written_and_a_missing_one_reads_as_one() {
    let m = Motion::default();
    let text = toml::to_string(&m).unwrap();
    assert!(!text.contains("forward_bias"), "{text}");
    let back: Motion = toml::from_str(&text).unwrap();
    assert_eq!(back.forward_bias, 1.0);
    let biased = Motion { forward_bias: 0.25, ..m };
    let text = toml::to_string(&biased).unwrap();
    assert!(text.contains("forward_bias = 0.25"), "{text}");
    assert_eq!(toml::from_str::<Motion>(&text).unwrap(), biased);
}
