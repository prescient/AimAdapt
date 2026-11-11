//! Helper bots: the pushes that move imported scenarios' targets around.

use super::*;

impl Sim {
    /// Applies the kicks of every helper bot that are due in this step, before the
    /// bots move. A pusher hits on a fixed schedule from the start of the run, and
    /// each hit adds to the speed of every target it reaches (and can see).
    pub(super) fn push_targets(&mut self) {
        let now = f64::from(self.elapsed());
        let before = std::mem::replace(&mut self.pushed_to, now);
        let Some(world) = &self.current.world else { return };
        for pusher in &world.pushers {
            // From one step's end to the next's, so every kick is counted once.
            let every = f64::from(pusher.every_s);
            let hits = ((now / every).floor() - (before / every).floor()).max(0.0) as f32;
            if hits < 1.0 {
                continue;
            }
            let facing = Vec3::from_array(pusher.facing);
            let at = Vec3::from_array(pusher.pos);
            for t in &mut self.targets {
                let Some(b) = &mut t.bot else { continue };
                let height = t.height * t.radius / t.base_radius.max(1e-6);
                let from = if pusher.chases {
                    // Stopped by the plane it slides on, at the point nearest the target.
                    let d = (t.pos - at).dot(facing);
                    if d <= 0.0 {
                        continue;
                    }
                    t.pos - facing * d
                } else {
                    at
                };
                if !pusher.reaches(from, t.pos, t.radius, half_segment(t.shape, t.radius, height)) {
                    continue;
                }
                if pusher.needs_sight {
                    let to = t.pos - from;
                    let len = to.length();
                    if len > 1e-6 && blocked_from(&self.walls, from, to / len, len) {
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
    }
}

/// Whether a wall is nearer than `d` along `dir` from `from`.
fn blocked_from(walls: &Collider, from: Vec3, dir: Vec3, d: f32) -> bool {
    !walls.is_empty() && walls.ray(from, dir, d).is_some()
}
