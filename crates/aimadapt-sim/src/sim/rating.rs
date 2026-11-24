//! Adaptive play: pace, outcomes, tracking segments and the next target's difficulty.

use super::*;

impl Sim {
    /// Whether each target is one outcome (lifetime, size or speed when clicking, or
    /// an imported scenario whose bots can time out) rather than each tracking segment.
    /// A built-in scenario's tracking segments come from beam ticks, so a hitscan one
    /// has to be rated target by target.
    pub(super) fn per_target(&self) -> bool {
        self.adaptive.as_ref().is_some_and(|a| {
            let hitscan = self.scenario.weapon.kind == WeaponKind::Hitscan;
            match a.rating.cfg.knob {
                Knob::Lifetime => true,
                Knob::Radius => hitscan,
                // With a beam and target lifetimes, each target is a kill to finish in
                // time (regen makes that a test of unbroken tracking).
                Knob::TargetSpeed => hitscan || self.scenario.targets.lifetime_s > 0.0,
                Knob::TimeScale => a.rule == Some(ImportedRule::Timed),
                Knob::KillPace | Knob::Auto => false,
            }
        })
    }

    /// Whether a kill after more than [`MAX_MISSES_PER_TARGET`] misses fails. A held
    /// trigger fires between targets, so on an imported scenario only a click weapon
    /// has the rule.
    pub(super) fn counts_misses(&self) -> bool {
        self.scenario.world.is_none() || self.scenario.weapon.trigger == Trigger::Click
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

    /// Kill pace: each window that runs out with no kill is a failure. A window is
    /// real seconds, so on a time-scaled import it is the clock's rate times as long
    /// on the game clock (which the pace goes by).
    pub(super) fn pace_tick(&mut self, now: f32) {
        for _ in 0..100 {
            let Some((value, _, _)) = self.pace_window() else { return };
            let span = value * self.rate.now;
            if now - self.pace.since < span {
                return;
            }
            self.outcome(value, 0.0, false);
            self.pace.since += span;
            self.pace.missed = true;
            self.pace.window = None;
        }
    }

    /// Kill pace: a kill inside the window is a success, and starts the next one.
    pub(super) fn pace_kill(&mut self, now: f32) {
        let Some((value, _, _)) = self.pace_window() else { return };
        if !self.pace.missed && now - self.pace.since <= value * self.rate.now {
            let ok = !self.counts_misses() || self.pace.misses <= MAX_MISSES_PER_TARGET;
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
    /// In a fixed-difficulty run every segment keeps the run's value. A speed run has
    /// segments whatever it rates, as its clock's rate is the whole game's.
    pub(super) fn start_segment(&mut self) {
        if self.per_target() && self.knob() != Some(Knob::TimeScale) {
            return;
        }
        let file_rate = self.file_rate();
        let now = self.elapsed();
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
            // Everything on the game clock changes speed from the next frame on.
            Knob::TimeScale => self.rate.set(now, file_rate * value),
            Knob::Lifetime | Knob::KillPace | Knob::Auto => {}
        }
    }

    /// A speed run, after each frame: its segments are real seconds, ticked at
    /// [`BEAM_TICK_HZ`] whatever the clock's rate. Tracking scores each segment by the
    /// share of ticks with the aim on a part of a bot that counts, firing or not (a
    /// held trigger can't game it), and only ticks with a bot up count, so a respawn
    /// gap isn't the player's. A timed file rates each bot instead, and its segments
    /// only set the calibration run's speed.
    pub(super) fn speed_ticks(&mut self, real: f32, aim: Vec3) {
        if self.knob() != Some(Knob::TimeScale) {
            return;
        }
        let tick = 1.0 / BEAM_TICK_HZ;
        let tracking = !self.per_target();
        let on = self.pick_counted(aim).is_some();
        self.speed_acc += real;
        while self.speed_acc >= tick && self.phase == Phase::Running {
            self.speed_acc -= tick;
            if tracking && self.targets.is_empty() {
                continue;
            }
            self.segment_tick(on, tick, tracking);
        }
    }

    pub(super) fn adaptive_tick(&mut self, on: bool, tick: f32) {
        // A speed run ticks on real time (`speed_ticks`).
        if self.per_target() || matches!(self.knob(), Some(Knob::KillPace | Knob::TimeScale)) {
            return;
        }
        self.segment_tick(on, tick, true);
    }

    /// One tick of a tracking segment. At the segment's end its share of ticks on
    /// target is an outcome, when `rated`, and the next segment starts.
    fn segment_tick(&mut self, on: bool, tick: f32, rated: bool) {
        let Some(a) = &self.adaptive else { return };
        let (segment, current) = (a.rating.cfg.segment_s, a.segment);
        self.window.0 += 1;
        self.window.1 += u32::from(on);
        self.window.2 += tick;
        if self.window.2 >= segment {
            let share = self.window.1 as f32 / self.window.0.max(1) as f32;
            self.window = (0, 0, 0.0);
            if rated && let Some((value, _, _)) = current {
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
