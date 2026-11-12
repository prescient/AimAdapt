//! How targets move each step.

use super::firing::is_invincible;
use super::*;

impl Sim {
    pub(super) fn move_targets(&mut self, dt: f32, events: &mut Vec<Event>) {
        let now = self.elapsed();
        let mv = self.current.targets.movement.clone();
        let shrink = self.current.targets.shrink_to;
        let area = Area::of(&self.scenario);
        let mut timed_out: Vec<f32> = Vec::new();
        let mut i = 0;
        while i < self.targets.len() {
            let t = &mut self.targets[i];
            t.age += dt;
            // Negative regen drains a target, once its delay has passed since the last
            // hit, until it goes as a timeout (VT Aether's targets last 19 s).
            let drained = t.regen.0 < 0.0 && now - t.last_hit_at >= t.regen.1 && {
                t.health += t.regen.0 * dt;
                t.health <= 1e-4
            };
            if t.lifetime > 0.0 || drained {
                if drained || t.age >= t.lifetime {
                    self.leave_as_timeout(i, now, events, &mut timed_out);
                    continue;
                }
                let k = t.age / t.lifetime;
                t.radius = t.base_radius * (1.0 + (shrink - 1.0) * k);
            }
            if let Some(b) = &mut t.bot
                && let Some(m) = self.current.world.as_ref().and_then(|w| w.motion_of(t.kind))
            {
                // An imported scenario's bot: dodge profile and character movement.
                let half = half_segment(t.shape, t.radius, t.height * t.radius / t.base_radius.max(1e-6));
                let (yaw, pitch) = (t.yaw, t.pitch);
                // Only abilities use sight: a ray from the bot to the eye against the walls
                // that stop shots (not the invisible clip walls).
                let seen = m.abilities.is_none() || t.pos.length() < 1e-3 || {
                    let to_bot = t.pos.normalize();
                    !blocked(&self.walls, to_bot, t.pos.length())
                };
                b.step_seen(m, &mut self.rng, &self.bot_walls, &mut t.pos, t.radius, half, now, dt, seen);
                // An ability that costs health takes it at the use. That isn't a hit: it
                // restarts no regen delay and counts as no damage. Bots that can't die
                // pay nothing, as with shots.
                let owed = b.take_owed_health();
                let spent = owed > 0.0 && !is_invincible(self.current.world.as_ref(), t.kind);
                if spent {
                    t.health -= owed;
                }
                (t.yaw, t.pitch) = angles(t.pos);
                t.distance = t.pos.length();
                // Angular speed, for tracking telemetry.
                let dyaw = (t.yaw - yaw + 540.0).rem_euclid(360.0) - 180.0;
                t.rate = [dyaw / dt.max(1e-6), (t.pitch - pitch) / dt.max(1e-6)];
                if spent && t.health <= 1e-4 {
                    self.leave_as_timeout(i, now, events, &mut timed_out);
                    continue;
                }
                i += 1;
                continue;
            }
            if mv.kind != MovementKind::Static || mv.approaches() || mv.jumps() {
                move_free(t, &mv, &area, &mut self.rng, now, dt);
            }
            i += 1;
        }
        if self.current.world.as_ref().is_some_and(World::bots_bump) {
            let mut bodies: Vec<(Vec3, f32, f32)> = self
                .targets
                .iter()
                .map(|t| {
                    (t.pos, t.radius, half_segment(t.shape, t.radius, t.height * t.radius / t.base_radius.max(1e-6)))
                })
                .collect();
            separate(&mut bodies);
            for (t, (p, _, _)) in self.targets.iter_mut().zip(bodies) {
                t.pos = p;
            }
        }
        if !timed_out.is_empty() {
            self.next_target(now);
            if self.per_target() {
                for value in timed_out.into_iter().filter(|v| v.is_finite()) {
                    self.outcome(value, 0.0, false);
                }
            }
        }
    }
}

impl Sim {
    /// A target goes away by itself: its lifetime ran out, it drained away or an
    /// ability spent its last health. It counts as a timeout, with no kill, score or
    /// damage, and its slot fills again after its respawn delay. `timed_out` gets the
    /// value of its track (NaN for none), for `move_targets` to settle afterwards.
    fn leave_as_timeout(&mut self, i: usize, now: f32, events: &mut Vec<Event>, timed_out: &mut Vec<f32>) {
        let mut t = self.targets.swap_remove(i);
        self.stats.timeouts += 1;
        self.telemetry.targets.push(t.record(now, Outcome::TimedOut));
        events.push(Event::TimedOut { id: t.id });
        self.queue_respawn(&mut t, now);
        timed_out.push(t.track.value.map_or(f32::NAN, |(value, _, _)| value));
    }
}

/// Where a built-in scenario's targets may go: their spawn area and the room.
struct Area {
    yaw: Range,
    pitch: Range,
    distance: Range,
    shape: Shape,
    half_w: f32,
    half_d: f32,
    floor: f32,
    ceiling: f32,
}

impl Area {
    fn of(s: &Scenario) -> Area {
        let (sp, a) = (&s.targets.spawn, &s.arena);
        Area {
            yaw: sp.yaw,
            pitch: sp.pitch,
            distance: sp.distance,
            shape: s.targets.shape,
            half_w: a.size[0] / 2.0,
            half_d: a.size[2] / 2.0,
            floor: -a.eye_height,
            ceiling: a.size[1] - a.eye_height,
        }
    }
}

/// Moves a built-in scenario's target for `dt`: across the view (straight or
/// curving), forward and back, and up in jumps, bouncing off the edges of its
/// spawn area and off the room.
fn move_free(t: &mut Target, mv: &Movement, area: &Area, rng: &mut Rng, now: f32, dt: f32) {
    if mv.kind != MovementKind::Static {
        t.next_change -= dt;
        if t.next_change <= 0.0 {
            t.next_change = rng.range(mv.change_every_s);
            if mv.kind == MovementKind::Curve {
                // Keep going, but bend the other way (or the same way at a new rate).
                t.turn = new_turn(rng, mv);
            } else {
                let own = Movement { speed: t.speed.unwrap_or(mv.speed), ..mv.clone() };
                t.goal = new_rate(rng, &own, t.distance, t.pitch, t.goal);
                if mv.accel <= 0.0 {
                    t.rate = t.goal;
                }
            }
            t.last_change = now;
        }
        if mv.kind == MovementKind::Curve {
            t.rate = rotate(t.rate, t.turn * dt, t.pitch);
            t.goal = t.rate;
        } else if mv.accel > 0.0 {
            t.rate = steer(t.rate, t.goal, mv.accel, t.distance, t.pitch, dt);
        }
    }
    if mv.approaches() {
        t.next_radial -= dt;
        if t.next_radial <= 0.0 {
            let every = if mv.approach_every_s.1 > 0.0 { mv.approach_every_s } else { mv.change_every_s };
            t.next_radial = rng.range(every);
            let side = if t.radial_goal > 0.0 { -1.0 } else { 1.0 };
            t.radial_goal = side * rng.range(mv.approach_speed);
        }
        t.radial = if mv.accel > 0.0 {
            let step = mv.accel * dt;
            t.radial + (t.radial_goal - t.radial).clamp(-step, step)
        } else {
            t.radial_goal
        };
        let dr = area.distance;
        let (d, v) = bounce(t.distance + t.radial * dt, t.radial, dr);
        if v.signum() != t.radial.signum() {
            t.radial_goal = t.radial_goal.abs() * v.signum();
        }
        // Keep sideways speed in meters per second: closer is faster across the view.
        let k = t.distance / d.max(0.1);
        t.rate = [t.rate[0] * k, t.rate[1] * k];
        t.goal = [t.goal[0] * k, t.goal[1] * k];
        (t.distance, t.radial) = (d, v);
    }
    if mv.jumps() {
        let (h, air) = (mv.jump_height, mv.jump_air_s);
        let g = 8.0 * h / (air * air);
        if t.lift <= 0.0 && t.lift_v <= 0.0 {
            t.next_jump -= dt;
            if t.next_jump <= 0.0 {
                t.lift_v = 4.0 * h / air;
            }
        }
        if t.lift > 0.0 || t.lift_v > 0.0 {
            t.lift += t.lift_v * dt;
            t.lift_v -= g * dt;
            if t.lift <= 0.0 {
                t.lift = 0.0;
                t.lift_v = 0.0;
                t.next_jump = rng.range(mv.jump_every_s);
            }
        }
    }
    // Bounce off the edges of the spawn area, and off the floor, ceiling
    // and walls: the spawn area can reach further than the room allows.
    let half_h = match area.shape {
        Shape::Sphere => t.radius,
        Shape::Capsule | Shape::Box => (t.height * t.radius / t.base_radius.max(1e-6) / 2.0).max(t.radius),
    };
    let (yr, mut pr) = (area.yaw, area.pitch);
    let d = t.distance.max(1e-3);
    let pitch_at = |y: f32| (y / d).clamp(-1.0, 1.0).asin().to_degrees();
    pr = Range(pr.0.max(pitch_at(area.floor + half_h)), pr.1.min(pitch_at(area.ceiling - half_h)));
    if pr.0 > pr.1 {
        let mid = (pr.0 + pr.1) / 2.0;
        pr = Range(mid, mid);
    }
    let before = t.rate;
    let (old_yaw, old_rate) = (t.yaw, t.rate[0]);
    (t.yaw, t.rate[0]) = bounce(t.yaw + t.rate[0] * dt, t.rate[0], yr);
    (t.pitch, t.rate[1]) = bounce(t.pitch + t.rate[1] * dt, t.rate[1], pr);
    let p = from_angles(t.yaw, t.pitch) * t.distance;
    if p.x.abs() + t.radius > area.half_w || p.z.abs() + t.radius > area.half_d {
        // A side or back wall: stay put sideways and turn around.
        t.yaw = old_yaw;
        t.rate[0] = -old_rate;
    }
    // A bounce turns the target around, so it should head that way too.
    for ((goal, rate), was) in t.goal.iter_mut().zip(t.rate).zip(before) {
        if rate.signum() != was.signum() && rate != 0.0 {
            *goal = goal.abs() * rate.signum();
        }
    }
    if before != t.rate {
        t.last_change = now;
    }
    // Jumps go straight up from the target's base, under the ceiling.
    let base = from_angles(t.yaw, t.pitch) * t.distance;
    t.lift = t.lift.min((area.ceiling - half_h - base.y).max(0.0));
    t.pos = base + Vec3::Y * t.lift;
}
