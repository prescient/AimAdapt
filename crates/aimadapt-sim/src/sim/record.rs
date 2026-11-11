//! What a run leaves behind: the aim trace, tracking samples and the result.

use super::*;

impl Sim {
    pub fn result(&self) -> RunResult {
        let mut telemetry = self.telemetry.clone();
        let now = self.elapsed();
        telemetry.targets.extend(self.targets.iter().map(|t| t.record(now, Outcome::Open)));
        telemetry.targets.sort_by_key(|t| t.no);
        let adaptive = self.adaptive.as_ref().map(|a| {
            let r = &a.rating;
            let step = r.timeline.len().div_ceil(300).max(1);
            let mut timeline: Vec<(f32, f32)> = r.timeline.iter().step_by(step).copied().collect();
            if let Some(last) = r.timeline.last()
                && timeline.last() != Some(last)
            {
                timeline.push(*last);
            }
            let e = r.estimate();
            AdaptiveSummary {
                knob: r.cfg.knob,
                value: r.value(),
                range: r.range(),
                start_value: r.start_value,
                theta: e.theta,
                slope: e.slope,
                slope_sd: e.slope_sd,
                sd: e.sd,
                goal: r.cfg.goal,
                success_rate: r.success_rate(),
                outcomes: r.outcomes(),
                fastest: r.fastest,
                timeline,
                run_value: a.run.map(|x| x.0),
                pick: a.run.map_or(Pick::Calibrate, |x| x.2),
                planned_chance: a.run.map(|x| x.1),
                evidence: r.evidence(),
                breakthrough: a.breakthrough,
                break_days: a.break_days,
            }
        });
        RunResult {
            score: adaptive.as_ref().map_or(self.score(), |a| a.value),
            stats: self.stats.clone(),
            kills: self.kills.clone(),
            duration_s: self.elapsed(),
            adaptive,
            telemetry,
        }
    }

    /// Updates per-target flick telemetry, tracking samples and the aim trace.
    pub(super) fn track_aim(&mut self, now: f32, aim: Vec3) {
        let walls = &self.walls;
        for t in &mut self.targets {
            let on = t.hit_by(aim).is_some_and(|d| !blocked(walls, aim, d));
            let k = &mut t.track;
            let since = now - k.start_at;
            if k.reaction_s.is_none() {
                let moved = k.distance_deg - angle_deg(aim, t.pos);
                if moved >= (0.1 * k.distance_deg).max(0.3) {
                    k.reaction_s = Some(since);
                }
            }
            if on && !k.inside {
                k.entries += 1;
                if k.reach_s.is_none() {
                    k.reach_s = Some(since);
                }
            }
            k.inside = on;
            if k.reach_s.is_some() {
                let (x, y) = screen_offset(t.pos.normalize(), aim);
                let along = x * k.flick.0 + y * k.flick.1;
                k.overshoot_deg = k.overshoot_deg.max(along - k.radius_deg);
            }
        }

        if now >= self.next_trace {
            self.next_trace = now + 1.0 / TRACE_HZ;
            let (yaw, pitch) = angles(aim);
            self.telemetry.trace.push([now, yaw, pitch]);
        }

        if now >= self.next_frame {
            // Step on from the last frame's slot, not from now, so frames keep to
            // 60 Hz instead of slipping to every third game frame.
            self.next_frame = (self.next_frame + 1.0 / FRAME_HZ).max(now);
            let targets = self.targets.iter().map(|t| t.pose()).collect();
            self.telemetry.frames.push(TargetFrame { t: now, targets });
        }

        if self.scenario.weapon.kind == WeaponKind::Beam && now >= self.next_sample {
            self.next_sample = now + 1.0 / SAMPLE_HZ;
            let nearest = self.targets.iter().min_by(|a, b| angle_deg(aim, a.pos).total_cmp(&angle_deg(aim, b.pos)));
            if let Some(t) = nearest {
                let motion = (t.rate[0] * t.pitch.to_radians().cos(), t.rate[1]);
                let speed = (motion.0 * motion.0 + motion.1 * motion.1).sqrt();
                let (x, y) = screen_offset(t.pos.normalize(), aim);
                let lead = if speed > 1e-3 { (x * motion.0 + y * motion.1) / speed } else { 0.0 };
                self.telemetry.samples.push(TrackSample {
                    t: now,
                    on_target: t.hit_by(aim).is_some_and(|d| !blocked(&self.walls, aim, d)),
                    error_deg: angle_deg(aim, t.pos),
                    target_speed_dps: speed,
                    lead_deg: lead,
                    since_change_s: now - t.last_change,
                    engaged: self.targets.iter().any(Target::engaged),
                });
            }
        }
    }
}
