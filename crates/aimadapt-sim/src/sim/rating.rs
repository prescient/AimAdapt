//! Adaptive play: pace, outcomes, tracking segments and the next target's difficulty.

use super::*;

impl Sim {
    /// Whether the adaptive knob is set per target (lifetime, or size or speed when
    /// clicking) rather than per tracking segment. Tracking segments come from beam
    /// ticks, so a hitscan scenario has to be rated target by target.
    pub(super) fn per_target(&self) -> bool {
        self.adaptive.as_ref().is_some_and(|a| {
            let hitscan = self.scenario.weapon.kind == WeaponKind::Hitscan;
            match a.rating.cfg.knob {
                Knob::Lifetime => true,
                Knob::Radius => hitscan,
                // With a beam and target lifetimes, each target is a kill to finish in
                // time (regen makes that a test of unbroken tracking).
                Knob::TargetSpeed => hitscan || self.scenario.targets.lifetime_s > 0.0,
                Knob::KillPace | Knob::Auto => false,
            }
        })
    }

    /// Kill pace: the current window's value, picking one if needed.
    pub(super) fn pace_window(&mut self) -> Option<(f32, f32, Pick)> {
        if self.knob() != Some(Knob::KillPace) {
            return None;
        }
        if self.pace.window.is_none() {
            let a = self.adaptive.as_mut()?;
            self.pace.window = Some(a.next(&mut self.rng));
        }
        self.pace.window
    }

    /// Kill pace: each window that runs out with no kill is a failure.
    pub(super) fn pace_tick(&mut self, now: f32) {
        for _ in 0..100 {
            let Some((value, _, _)) = self.pace_window() else { return };
            if now - self.pace.since < value {
                return;
            }
            self.outcome(value, 0.0, false);
            self.pace.since += value;
            self.pace.missed = true;
            self.pace.window = None;
        }
    }

    /// Kill pace: a kill inside the window is a success, and starts the next one.
    pub(super) fn pace_kill(&mut self, now: f32) {
        let Some((value, _, _)) = self.pace_window() else { return };
        if !self.pace.missed && now - self.pace.since <= value {
            let ok = self.pace.misses <= MAX_MISSES_PER_TARGET;
            self.outcome(value, if ok { 1.0 } else { 0.0 }, false);
        }
        self.pace = Pace { since: now, ..Pace::default() };
    }

    /// Feeds an outcome at a knob value to the rating.
    pub(super) fn outcome(&mut self, value: f32, outcome: f32, tracking: bool) {
        let now = self.elapsed();
        if let Some(a) = &mut self.adaptive {
            a.rating.update(now, value, outcome, tracking);
        }
    }

    /// Tracking: picks the next segment's difficulty and applies it to every target.
    /// In a fixed-difficulty run every segment keeps the run's value.
    pub(super) fn start_segment(&mut self) {
        if self.per_target() {
            return;
        }
        let Some(a) = &mut self.adaptive else { return };
        if a.rating.cfg.knob == Knob::KillPace || (a.run.is_some() && a.segment.is_some()) {
            return;
        }
        let (value, chance, pick) = a.next(&mut self.rng);
        a.segment = Some((value, chance, pick));
        if a.run.is_some() {
            // Already applied to the scenario before the targets spawned.
            return;
        }
        match a.rating.cfg.knob {
            Knob::TargetSpeed => {
                self.current.targets.movement.speed = speed_for(&self.scenario, value);
                // Change speed now, keeping each target's direction.
                for t in &mut self.targets {
                    let screen = (t.rate[0] * t.pitch.to_radians().cos(), t.rate[1]);
                    let now = (screen.0 * screen.0 + screen.1 * screen.1).sqrt();
                    if now > 1e-3 {
                        let k = value * (0.85 + 0.15 * self.rng.next_f32()) / now;
                        t.rate = [t.rate[0] * k, t.rate[1] * k];
                    }
                }
            }
            Knob::Radius => {
                self.current.targets.radius = value;
                for t in &mut self.targets {
                    t.radius = value;
                    t.base_radius = value;
                }
            }
            Knob::Lifetime | Knob::KillPace | Knob::Auto => {}
        }
    }

    pub(super) fn adaptive_tick(&mut self, on: bool, tick: f32) {
        if self.per_target() || self.knob() == Some(Knob::KillPace) {
            return;
        }
        let Some(a) = &self.adaptive else { return };
        let (segment, current) = (a.rating.cfg.segment_s, a.segment);
        self.window.0 += 1;
        self.window.1 += u32::from(on);
        self.window.2 += tick;
        if self.window.2 >= segment {
            let share = self.window.1 as f32 / self.window.0.max(1) as f32;
            self.window = (0, 0, 0.0);
            if let Some((value, _, _)) = current {
                self.outcome(value, share, true);
            }
            self.start_segment();
        }
    }

    /// After a kill or timeout the player starts on the next target, so flicks to the
    /// targets they haven't reached yet are timed from here.
    pub(super) fn next_target(&mut self, now: f32) {
        let aim = self.last_aim;
        for t in &mut self.targets {
            if t.track.reach_s.is_none() {
                t.track.restart(now, aim, t.pos, t.radius);
            }
        }
    }
}
