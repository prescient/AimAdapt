//! Tests for very thin bodies: a speck of a body under a head, a needle 2.8 m tall and 6 mm wide.

use glam::Vec3;

use super::tests::DT;
use super::*;
use crate::collide::Collider;
use crate::rng::Rng;
use crate::world::WorldMesh;

/// A closed box from `min` to `max`.
fn solid(min: [f32; 3], max: [f32; 3]) -> WorldMesh {
    let p = |x: usize, y: usize, z: usize| [[min[0], max[0]][x], [min[1], max[1]][y], [min[2], max[2]][z]];
    WorldMesh {
        positions: vec![p(0, 0, 0), p(1, 0, 0), p(1, 1, 0), p(0, 1, 0), p(0, 0, 1), p(1, 0, 1), p(1, 1, 1), p(0, 1, 1)],
        triangles: vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [3, 6, 2],
            [3, 7, 6],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ],
        ..WorldMesh::default()
    }
}

/// The floor's top.
const TOP: f32 = -2.0;
/// The needle: its radius and half the length of its middle line.
const RADIUS: f32 = 0.003;
const HALF: f32 = 1.4;

/// A floor 1 m thick from x = -8 to 8, a wall 1 m thick standing on it whose face is at x = 4, and a wall
/// beside the floor's other edge (its face at x = -8) that goes on below the floor.
fn yard() -> Collider {
    Collider::new(&[
        solid([-8.0, TOP - 1.0, -30.0], [8.0, TOP, 10.0]),
        solid([4.0, TOP, -30.0], [5.0, TOP + 8.0, 10.0]),
        solid([-9.0, TOP - 6.0, -30.0], [-8.0, TOP + 8.0, 10.0]),
    ])
}

/// A walker that strafes one way along x for a long time, however the player stands to it.
fn walker(speed: f32) -> Motion {
    Motion {
        max_speed: speed,
        accel: 40.0,
        gravity: 20.0,
        faces_player: false,
        dodges: vec![Dodge { strafe_s: [30.0, 30.0], ..Dodge::default() }],
        ..Motion::default()
    }
}

/// What a played body did: how low its bottom came against the floor's top once it had landed, how far to
/// either side its middle got, and whether it ended on the floor.
#[derive(Debug)]
struct Seen {
    lowest: f32,
    left: f32,
    right: f32,
    grounded: bool,
}

/// Drops a body with its bottom `drop` above the floor and plays `seconds`. `right` is the way its spawn
/// point's right hand points, which turns the way it walks.
fn play(m: &Motion, radius: f32, half: f32, right: Vec3, drop: f32, seconds: f32) -> Seen {
    let walls = yard();
    let mut rng = Rng::new(7);
    let start = Vec3::new(0.0, TOP + drop + half + radius, -10.0);
    let mut s = BotState::new(m, &mut rng, start, right);
    let (mut p, mut t, mut landed) = (start, 0.0, false);
    let mut seen = Seen { lowest: f32::MAX, left: 0.0, right: 0.0, grounded: false };
    while t < seconds {
        s.step(m, &mut rng, &walls, &mut p, radius, half, t, DT);
        t += DT;
        landed |= s.grounded;
        if landed {
            seen.lowest = seen.lowest.min(p.y - half - radius - TOP);
        }
        seen.left = seen.left.min(p.x);
        seen.right = seen.right.max(p.x);
        seen.grounded = s.grounded;
    }
    seen
}

/// Bug: walls only push a body out as far as its radius reaches. A speck of a body under a head (an
/// imported bot's body can be a few millimetres wide) walks 1.7 cm a substep, so one substep carried its
/// middle line through a wall's face, where the wall pushed it on inward instead of back. It came out of
/// the wall's far side, and where the wall stood beside the floor's edge it fell out of the map inside it,
/// again and again.
#[test]
fn a_needle_that_walks_into_a_wall_stops_at_its_face() {
    for drop in [0.0, 0.5, 30.0] {
        for speed in [4.0, 10.0] {
            for right in [Vec3::X, Vec3::NEG_X] {
                let seen = play(&walker(speed), RADIUS, HALF, right, drop, 14.0);
                let case = format!("dropped {drop} m at {speed} m/s, right hand {right}: {seen:?}");
                assert!(seen.right < 4.0 - RADIUS + 1e-3, "it went into the wall that stands on the floor: {case}");
                assert!(seen.left > -8.0 + RADIUS - 1e-3, "it went into the wall beside the floor's edge: {case}");
                assert!(seen.lowest > -0.005, "it sank into the floor: {case}");
                assert!(seen.grounded, "{case}");
                assert!(seen.right - seen.left > 3.5, "it walked: {case}");
            }
        }
    }
}

/// Bug: the same fault let a small ball that fell fast drop through the floor. At 40 m/s a body falls 17 cm
/// a substep; a ball 5 cm in radius ended with its middle under the floor's top and was pushed on down.
#[test]
fn a_small_ball_that_falls_fast_lands_on_the_floor() {
    for radius in [0.05, 0.02, 0.1] {
        let seen = play(&walker(0.0), radius, 0.0, Vec3::X, 60.0, 6.0);
        assert!(seen.grounded && seen.lowest.abs() < 0.005, "radius {radius}: {seen:?}");
    }
}

/// And a small ball that runs fast can't pass a wall either.
#[test]
fn a_small_ball_that_runs_fast_stops_at_a_wall() {
    for right in [Vec3::X, Vec3::NEG_X] {
        let radius = 0.05;
        let seen = play(&walker(25.0), radius, 0.0, right, 0.0, 8.0);
        let case = format!("right hand {right}: {seen:?}");
        assert!(seen.right < 4.0 - radius + 1e-3 && seen.left > -8.0 + radius - 1e-3, "{case}");
        assert!(seen.grounded && seen.lowest > -0.005, "{case}");
    }
}
