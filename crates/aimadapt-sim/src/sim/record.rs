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
                theta: r.carry_theta(),
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
        let mut result = RunResult {
            score: adaptive.as_ref().map_or(self.score(), |a| a.value),
            stats: self.stats.clone(),
            kills: self.kills.clone(),
            duration_s: self.elapsed(),
            adaptive,
            telemetry,
        };
        // The run is played on the game clock but reported in real seconds, as
        // KovaaK's reports it: time played, time to kill, and the replay's timeline.
        let scale = self.time_scale();
        if scale != 1.0 {
            result.convert_to_real_seconds(scale);
        }
        result
    }

    /// Updates per-target flick telemetry, tracking samples and the aim trace.
    pub(super) fn track_aim(&mut self, now: f32, aim: Vec3) {
        let walls = &self.walls;
        for t in &mut self.targets {
            // On a part that counts: for a bot that only takes headshots, the head where it is the first surface.
            let on = t.counted_hit(aim).is_some_and(|d| !blocked(walls, aim, d));
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
            self.next_trace = now + self.time_scale() / TRACE_HZ;
            let (yaw, pitch) = angles(aim);
            self.telemetry.trace.push([now, yaw, pitch]);
        }

        if now >= self.next_frame {
            // Step on from the last frame's slot, not from now, so frames keep to
            // 60 Hz instead of slipping to every third game frame. On a time-scaled
            // scenario the rates are per real second, so the slot is `time_scale` as long.
            self.next_frame = (self.next_frame + self.time_scale() / FRAME_HZ).max(now);
            let targets = self.targets.iter().map(|t| t.pose()).collect();
            self.telemetry.frames.push(TargetFrame { t: now, targets });
        }

        if self.scenario.weapon.kind == WeaponKind::Beam && now >= self.next_sample {
            self.next_sample = now + self.time_scale() / SAMPLE_HZ;
            let nearest = self.targets.iter().min_by(|a, b| angle_deg(aim, a.pos).total_cmp(&angle_deg(aim, b.pos)));
            if let Some(t) = nearest {
                let motion = (t.rate[0] * t.pitch.to_radians().cos(), t.rate[1]);
                let speed = (motion.0 * motion.0 + motion.1 * motion.1).sqrt();
                let (x, y) = screen_offset(t.pos.normalize(), aim);
                let lead = if speed > 1e-3 { (x * motion.0 + y * motion.1) / speed } else { 0.0 };
                self.telemetry.samples.push(TrackSample {
                    t: now,
                    on_target: t.counted_hit(aim).is_some_and(|d| !blocked(&self.walls, aim, d)),
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

impl RunResult {
    /// Turns every time in the result from game seconds into real seconds, given the
    /// game clock's rate (`scale` game seconds per real second).
    fn convert_to_real_seconds(&mut self, scale: f32) {
        self.duration_s /= scale;
        for k in &mut self.kills {
            k.at_s /= scale;
            k.ttk_s /= scale;
        }
        let t = &mut self.telemetry;
        for r in &mut t.targets {
            for v in [&mut r.spawned_at, &mut r.start_at, &mut r.ended_at, &mut r.lifetime_s] {
                *v /= scale;
            }
            for v in [&mut r.reaction_s, &mut r.reach_s].into_iter().flatten() {
                *v /= scale;
            }
        }
        for s in &mut t.shots {
            s.t /= scale;
        }
        for s in &mut t.samples {
            s.t /= scale;
            s.since_change_s /= scale;
            s.target_speed_dps *= scale;
        }
        for p in &mut t.trace {
            p[0] /= scale;
        }
        for f in &mut t.frames {
            f.t /= scale;
        }
    }
}
