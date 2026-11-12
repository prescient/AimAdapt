//! Helper bots: the pushes that move imported scenarios' targets around.

use super::*;
use crate::world::{ChaseFlight, Pusher};

/// The longest step a chaser's flight is integrated in, in seconds.
const LONGEST_FLIGHT_STEP_S: f64 = 1.0 / 120.0;

/// Where a chasing helper with a flight is, and how fast it moves.
#[derive(Debug, Clone, Copy)]
pub(super) struct Flier {
    pub(super) place: Vec3,
    pub(super) vel: Vec3,
}

/// A flier for each pusher of the world that has a flight: at its starting point and
/// at rest.
pub(super) fn fliers(world: Option<&World>) -> Vec<Option<Flier>> {
    let Some(world) = world else { return Vec::new() };
    let start = |p: &Pusher| p.flight.as_ref().map(|_| Flier { place: Vec3::from_array(p.pos), vel: Vec3::ZERO });
    world.pushers.iter().map(start).collect()
}

/// The middle of the target nearest to `from`, among those that can be pushed.
fn nearest(targets: &[Target], from: Vec3) -> Option<Vec3> {
    let near = |a: &&Target, b: &&Target| a.pos.distance_squared(from).total_cmp(&b.pos.distance_squared(from));
    targets.iter().filter(|t| t.bot.is_some()).min_by(near).map(|t| t.pos)
}

impl Flier {
    /// Flies through `dt` seconds in steps of equal length, none longer than
    /// [`LONGEST_FLIGHT_STEP_S`], toward the nearest target. The targets stay where
    /// they are for the whole time.
    fn fly(&mut self, flight: &ChaseFlight, facing: Vec3, targets: &[Target], dt: f64) {
        if dt <= 0.0 {
            return;
        }
        let steps = (dt / LONGEST_FLIGHT_STEP_S).ceil().max(1.0);
        let h = (dt / steps) as f32;
        for _ in 0..steps as u32 {
            // With nothing to chase there is no pull, and the drag brings it to a stop.
            let aim = nearest(targets, self.place).map(|to| (to - self.place).normalize_or_zero());
            self.vel *= (1.0 - flight.drag * h).max(0.0);
            if let Some(aim) = aim {
                self.vel += aim * flight.accel * h;
            }
            self.vel = self.vel.clamp_length_max(flight.max_speed);
            // The plane takes away the part that points into it (and the part that
            // points away: it stays on its plane).
            self.vel -= facing * self.vel.dot(facing);
            self.place += self.vel * h;
        }
    }
}

/// How many kicks a pusher makes between `before` and `now` seconds into the run:
/// the times the run passes a multiple of its period, from the start delay on.
pub(super) fn kicks(pusher: &Pusher, now: f64, before: f64) -> f32 {
    let every = f64::from(pusher.every_s);
    let from = before.max(f64::from(pusher.start_s));
    ((now / every).floor() - (from / every).floor()).max(0.0) as f32
}

/// Where a kick on a target at `at` comes from, or `None` when the pusher can't reach
/// it from its side of the plane. A chaser with a flight kicks from where it is, one
/// without from the point of its plane nearest the target.
fn origin(pusher: &Pusher, flier: Option<&Flier>, at: Vec3) -> Option<Vec3> {
    let facing = Vec3::from_array(pusher.facing);
    let pos = Vec3::from_array(pusher.pos);
    if !pusher.chases {
        return Some(pos);
    }
    let from = flier.map_or(pos, |f| f.place);
    let d = (at - from).dot(facing);
    if d <= 0.0 {
        return None;
    }
    Some(if flier.is_some() { from } else { at - facing * d })
}

impl Sim {
    /// Applies the kicks of every helper bot that are due in this step, before the
    /// bots move. A pusher hits on a fixed schedule from the start of the run (or
    /// from its start delay), and each hit adds to the speed of every target it
    /// reaches (and can see). A chaser with a flight first flies through the time
    /// since the last step.
    pub(super) fn push_targets(&mut self) {
        let now = f64::from(self.elapsed());
        let before = std::mem::replace(&mut self.pushed_to, now);
        let Some(world) = &self.current.world else { return };
        for (pusher, flier) in world.pushers.iter().zip(&mut self.fliers) {
            if let (Some(flight), Some(f)) = (&pusher.flight, flier.as_mut()) {
                f.fly(flight, Vec3::from_array(pusher.facing), &self.targets, now - before);
            }
            // From one step's end to the next's, so every kick is counted once.
            let hits = kicks(pusher, now, before);
            if hits >= 1.0 {
                kick_targets(&mut self.targets, &self.walls, pusher, flier.as_ref(), hits);
            }
        }
    }
}

/// `hits` kicks from one pusher on every target it reaches.
fn kick_targets(targets: &mut [Target], walls: &Collider, pusher: &Pusher, flier: Option<&Flier>, hits: f32) {
    // A chaser with a flight reaches along the line to its (nearest) target.
    let toward = flier.and_then(|f| nearest(targets, f.place).map(|to| (to - f.place).normalize_or_zero()));
    for t in targets {
        let half = t.bump_half_segment();
        let Some(b) = &mut t.bot else { continue };
        let Some(from) = origin(pusher, flier, t.pos) else { continue };
        let reached = match toward {
            Some(dir) => {
                pusher.reaches_along(from, dir.normalize_or(Vec3::from_array(pusher.facing)), t.pos, t.radius, half)
            }
            None => pusher.reaches(from, t.pos, t.radius, half),
        };
        if !reached {
            continue;
        }
        if pusher.needs_sight {
            let to = t.pos - from;
            let len = to.length();
            if len > 1e-6 && blocked_from(walls, from, to / len, len) {
                continue;
            }
        }
        let level = Vec3::new(t.pos.x - from.x, 0.0, t.pos.z - from.z);
        if level.length() >= 1e-4 {
            b.vel += level.normalize() * pusher.push * hits;
        }
        b.vel.y += pusher.lift * hits;
    }
}

/// Whether a wall is nearer than `d` along `dir` from `from`.
fn blocked_from(walls: &Collider, from: Vec3, dir: Vec3, d: f32) -> bool {
    !walls.is_empty() && walls.ray(from, dir, d).is_some()
}
