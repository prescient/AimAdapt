//! Hurt volumes and deadly teleporters (plan K12a). The rules were measured in KovaaK's 3.9.13.
//!
//! A body only has to touch a volume, and a fast one does not skip a thin volume, so the test sweeps
//! the body's box along the way it moved since the last pass. Each pair of a bot slot and a volume has
//! its own cooldown, which a bot keeps over its respawns. A body's first frame (the run's start, or the
//! frame it respawns) is special: a hit that falls due starts the cooldown and removes the body only if
//! the damage would kill it at full health, and otherwise does no damage.
//!
//! A death by a volume is a timeout: no kill, no damage and no score. Shots pass through volumes, and
//! volumes do not stop bodies.

use super::firing::is_invincible;
use super::*;
use crate::world::WorldVolume;

/// Slack when comparing a time since the last hit with a cooldown, in seconds.
const COOLDOWN_SLACK_S: f32 = 1e-4;
/// Slack when comparing positions, so a body that just touches a face counts, in meters.
const TOUCH_SLACK: f32 = 1e-5;

impl WorldVolume {
    /// Whether the volume has one vertical edge (along Y) and two horizontal ones.
    pub(crate) fn has_one_vertical_edge(&self) -> bool {
        let (mut vertical, mut flat) = (0, 0);
        for e in self.edges.map(Vec3::from_array) {
            let len = e.length();
            if e.x.abs() <= 1e-4 * len && e.z.abs() <= 1e-4 * len {
                vertical += 1;
            } else if e.y.abs() <= 1e-4 * len {
                flat += 1;
            }
        }
        vertical == 1 && flat == 2
    }

    /// Whether an upright box with half-extents `half` (across, up, across), taken as aligned with the
    /// volume's own axes, touches the volume anywhere along the straight way from `from` to `to`. Touching
    /// counts. The test is a slab test in the volume's own frame against the volume grown by the half-extents.
    fn touched_along(&self, from: Vec3, to: Vec3, half: Vec3) -> bool {
        let corner = Vec3::from_array(self.corner);
        let (mut t_in, mut t_out) = (0.0_f32, 1.0_f32);
        for e in self.edges.map(Vec3::from_array) {
            let len = e.length();
            if len <= 1e-9 {
                return false;
            }
            let axis = e / len;
            let reach = if axis.y.abs() > 0.5 { half.y } else { half.x.max(half.z) } + TOUCH_SLACK;
            let (lo, hi) = (-reach, len + reach);
            let (a, b) = ((from - corner).dot(axis), (to - corner).dot(axis));
            let d = b - a;
            if d.abs() < 1e-12 {
                if a < lo || a > hi {
                    return false;
                }
                continue;
            }
            let (t0, t1) = ((lo - a) / d, (hi - a) / d);
            t_in = t_in.max(t0.min(t1));
            t_out = t_out.min(t0.max(t1));
            if t_in > t_out {
                return false;
            }
        }
        true
    }
}

impl Sim {
    /// What the world's volumes do to the targets this frame: those that moved since the last pass, and
    /// those that appeared this frame. A target that dies of it goes as a timeout.
    pub(super) fn volume_pass(&mut self, now: f32, events: &mut Vec<Event>) {
        let Some(world) = self.current.world.as_ref().filter(|w| !w.volumes.is_empty()) else { return };
        let frame_began = self.frame_began;
        let mut leaving = Vec::new();
        for (i, t) in self.targets.iter_mut().enumerate() {
            // A new body has no earlier place: never sweep from where the last one in its slot died.
            let first = t.volume_from.is_none();
            let from = t.volume_from.unwrap_or(t.pos);
            t.volume_from = Some(t.pos);
            let half = Vec3::new(t.radius, t.bump_half_height(), t.radius);
            let invincible = is_invincible(Some(world), t.kind);
            for (vi, v) in world.volumes.iter().enumerate() {
                if !v.touched_along(from, t.pos, half) {
                    continue;
                }
                // Kill: it leaves at once and the cooldown is ignored.
                let mut gone = v.kill;
                if !gone {
                    let key = (t.slot, vi as u32);
                    let last = self.volume_hits.get(&key).copied();
                    if last.is_none_or(|l| now - l >= v.cooldown_s - COOLDOWN_SLACK_S) {
                        self.volume_hits.insert(key, now);
                        if first {
                            gone = !invincible && v.damage >= t.health;
                        } else {
                            // The same as a hit's effect on health and healing, without the bot's reaction.
                            if !invincible {
                                t.health -= v.damage;
                            }
                            t.last_hit_at = now;
                            if t.regen.1 > 0.0 {
                                t.heal_at = frame_began + t.regen.1;
                            }
                            gone = t.health <= 1e-4;
                        }
                    }
                }
                if gone {
                    leaving.push(i);
                    break;
                }
            }
        }
        // Highest first: each removal moves the last target into the gap.
        let mut timed_out = Vec::new();
        for i in leaving.into_iter().rev() {
            self.leave_as_timeout(i, now, events, &mut timed_out);
        }
        self.settle_timeouts(now, timed_out);
    }
}
