//! Adaptive difficulty. A scenario's difficulty is one number, the level (0 = easy,
//! 100 = hard), which sets one or more knobs such as target lifetime or speed. In
//! adaptive mode the level moves after every outcome so the player succeeds about
//! `goal` of the time (85% by default):
//!
//! - Clicking: a kill is a success; a miss or a timeout is a failure.
//! - Beams: every `window_s` seconds, the share of beam ticks on target.
//!
//! The update is a weighted staircase, `level += step × (outcome − goal)`, so it
//! settles where the success rate equals the goal. The step starts large to find the
//! player's level quickly, then shrinks each time the direction reverses (Kesten's
//! rule) to narrow in on what they can reliably do.

use serde::{Deserialize, Serialize};

use crate::scenario::{MovementKind, Range, Scenario, WeaponKind};

pub const MAX_LEVEL: f32 = 100.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Adaptive {
    /// Success rate to hold, 0.5–0.98.
    pub goal: f32,
    /// Level for a player's first adaptive run; later runs start where the last ended.
    pub start_level: f32,
    /// Seconds per tracking outcome (beam weapons).
    pub window_s: f32,
    /// What the level changes. Empty means a sensible choice for the scenario type
    /// (see [`Adaptive::resolved_knobs`]).
    pub knobs: Vec<KnobRange>,
}

impl Default for Adaptive {
    fn default() -> Self {
        Adaptive { goal: 0.85, start_level: 25.0, window_s: 0.5, knobs: Vec::new() }
    }
}

/// A setting the level controls, with its value at level 0 and level 100. Values
/// in between are spaced geometrically, so each level is the same relative step.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnobRange {
    pub knob: Knob,
    pub easy: f32,
    pub hard: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Knob {
    /// `targets.lifetime_s`: seconds before a target times out.
    Lifetime,
    /// `targets.radius` in meters.
    Radius,
    /// Top of `targets.movement.speed` (the bottom keeps its ratio), m/s.
    Speed,
    /// Top of `targets.movement.change_every_s` (bottom keeps its ratio), seconds.
    ChangeEvery,
    /// Multiplier on the spawn area's yaw and pitch spread.
    Spread,
    /// `targets.health`.
    Health,
    /// `targets.respawn_delay_s`.
    RespawnDelay,
}

impl Knob {
    pub const ALL: [Knob; 7] =
        [Knob::Lifetime, Knob::Radius, Knob::Speed, Knob::ChangeEvery, Knob::Spread, Knob::Health, Knob::RespawnDelay];

    pub fn label(self) -> &'static str {
        match self {
            Knob::Lifetime => "Target lifetime (s)",
            Knob::Radius => "Target radius (m)",
            Knob::Speed => "Target speed (m/s)",
            Knob::ChangeEvery => "Direction change (s)",
            Knob::Spread => "Spawn spread (×)",
            Knob::Health => "Target health",
            Knob::RespawnDelay => "Respawn delay (s)",
        }
    }

    /// Current value in `s`, in the knob's units.
    pub fn read(self, s: &Scenario) -> f32 {
        let t = &s.targets;
        match self {
            Knob::Lifetime => t.lifetime_s,
            Knob::Radius => t.radius,
            Knob::Speed => t.movement.speed.1,
            Knob::ChangeEvery => t.movement.change_every_s.1,
            Knob::Spread => 1.0,
            Knob::Health => t.health,
            Knob::RespawnDelay => t.respawn_delay_s,
        }
    }

    /// Sets the knob on `s` (a copy of `base`) to `v`.
    pub fn write(self, s: &mut Scenario, base: &Scenario, v: f32) {
        let scale = |r: Range, top: f32| {
            let k = if r.1 > 0.0 { top / r.1 } else { 1.0 };
            Range(r.0 * k, top)
        };
        let t = &mut s.targets;
        match self {
            Knob::Lifetime => t.lifetime_s = v,
            Knob::Radius => t.radius = v,
            Knob::Speed => t.movement.speed = scale(base.targets.movement.speed, v),
            Knob::ChangeEvery => t.movement.change_every_s = scale(base.targets.movement.change_every_s, v.max(0.05)),
            Knob::Spread => {
                let spread = |r: Range| {
                    let mid = (r.0 + r.1) / 2.0;
                    let half = (r.1 - r.0) / 2.0 * v;
                    Range(mid - half, mid + half)
                };
                t.spawn.yaw = spread(base.targets.spawn.yaw);
                let p = spread(base.targets.spawn.pitch);
                t.spawn.pitch = Range(p.0.max(-85.0), p.1.min(85.0));
            }
            Knob::Health => t.health = v,
            Knob::RespawnDelay => t.respawn_delay_s = v,
        }
    }
}

impl KnobRange {
    pub fn value_at(&self, level: f32) -> f32 {
        let k = (level / MAX_LEVEL).clamp(0.0, 1.0);
        if self.easy > 0.0 && self.hard > 0.0 {
            self.easy * (self.hard / self.easy).powf(k)
        } else {
            self.easy + (self.hard - self.easy) * k
        }
    }
}

impl Adaptive {
    /// The knobs to use: the scenario's own, or a default for its type.
    ///
    /// - Hitscan: target lifetime (adding a timeout if the scenario has none) and size.
    /// - Beam on moving targets: target speed and size.
    /// - Beam on still targets: size.
    pub fn resolved_knobs(&self, s: &Scenario) -> Vec<KnobRange> {
        if !self.knobs.is_empty() {
            return self.knobs.clone();
        }
        let r = s.targets.radius;
        let radius = KnobRange { knob: Knob::Radius, easy: r * 1.3, hard: r * 0.6 };
        match s.weapon.kind {
            WeaponKind::Hitscan => {
                let life = if s.targets.lifetime_s > 0.0 { s.targets.lifetime_s } else { 1.2 };
                vec![KnobRange { knob: Knob::Lifetime, easy: life * 2.0, hard: life * 0.4 }, radius]
            }
            WeaponKind::Beam if s.targets.movement.kind != MovementKind::Static => {
                let top = s.targets.movement.speed.1.max(0.5);
                vec![KnobRange { knob: Knob::Speed, easy: top * 0.5, hard: top * 2.0 }, radius]
            }
            WeaponKind::Beam => vec![KnobRange { knob: Knob::Radius, easy: r * 1.5, hard: r * 0.4 }],
        }
    }

    /// `base` with every knob set for `level`.
    pub fn apply(&self, base: &Scenario, level: f32) -> Scenario {
        let mut s = base.clone();
        for k in self.resolved_knobs(base) {
            k.knob.write(&mut s, base, k.value_at(level));
        }
        s
    }

    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if !(0.5..=0.98).contains(&self.goal) {
            p.push("adaptive goal must be 0.5–0.98".into());
        }
        if !(0.0..=MAX_LEVEL).contains(&self.start_level) {
            p.push("adaptive start level must be 0–100".into());
        }
        if !(0.2..=5.0).contains(&self.window_s) {
            p.push("adaptive tracking window must be 0.2–5 s".into());
        }
        for k in &self.knobs {
            if !(k.easy.is_finite() && k.hard.is_finite()) || k.easy < 0.0 || k.hard < 0.0 {
                p.push(format!("{}: easy and hard must be 0 or more", k.knob.label()));
            }
            if matches!(k.knob, Knob::Lifetime | Knob::Radius | Knob::Speed | Knob::Health | Knob::Spread)
                && (k.easy <= 0.0 || k.hard <= 0.0)
            {
                p.push(format!("{}: easy and hard must be above 0", k.knob.label()));
            }
        }
        let mut seen = Vec::new();
        for k in &self.knobs {
            if seen.contains(&k.knob) {
                p.push(format!("{} is listed twice", k.knob.label()));
            }
            seen.push(k.knob);
        }
        p
    }
}

/// Moves the level toward the player's reliable difficulty.
#[derive(Debug, Clone, PartialEq)]
pub struct Controller {
    pub level: f32,
    goal: f32,
    step: f32,
    first_step: f32,
    min_step: f32,
    reversals: u32,
    last_sign: f32,
    /// (seconds into the run, level after the update)
    pub timeline: Vec<(f32, f32)>,
    outcomes: Vec<f32>,
}

impl Controller {
    /// `known` says whether the start level came from earlier runs; if so the search
    /// starts with smaller steps.
    pub fn new(start: f32, goal: f32, known: bool) -> Controller {
        let first_step = if known { 10.0 } else { 24.0 };
        Controller {
            level: start.clamp(0.0, MAX_LEVEL),
            goal,
            step: first_step,
            first_step,
            min_step: 3.0,
            reversals: 0,
            last_sign: 0.0,
            timeline: vec![(0.0, start.clamp(0.0, MAX_LEVEL))],
            outcomes: Vec::new(),
        }
    }

    /// Records an outcome (1 = success, 0 = failure, or a share in between) and
    /// returns the new level.
    pub fn update(&mut self, t: f32, outcome: f32) -> f32 {
        let outcome = outcome.clamp(0.0, 1.0);
        self.outcomes.push(outcome);
        let err = outcome - self.goal;
        let sign = err.signum();
        if self.last_sign != 0.0 && sign != self.last_sign {
            self.reversals += 1;
            self.step = (self.first_step / (1.0 + 0.5 * self.reversals as f32)).max(self.min_step);
        }
        self.last_sign = sign;
        self.level = (self.level + self.step * err).clamp(0.0, MAX_LEVEL);
        self.timeline.push((t, self.level));
        self.level
    }

    /// Share of successes so far.
    pub fn success_rate(&self) -> Option<f32> {
        (!self.outcomes.is_empty()).then(|| self.outcomes.iter().sum::<f32>() / self.outcomes.len() as f32)
    }

    pub fn outcomes(&self) -> usize {
        self.outcomes.len()
    }

    /// The run's result: the average level over the second half of the run, once the
    /// search has settled. Falls back to the current level with few outcomes.
    pub fn settled_level(&self) -> f32 {
        let n = self.timeline.len();
        if n < 6 {
            return self.level;
        }
        let half = &self.timeline[n / 2..];
        half.iter().map(|(_, l)| l).sum::<f32>() / half.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Rng;

    #[test]
    fn knob_values_are_geometric() {
        let k = KnobRange { knob: Knob::Lifetime, easy: 2.0, hard: 0.5 };
        assert_eq!(k.value_at(0.0), 2.0);
        assert!((k.value_at(50.0) - 1.0).abs() < 1e-5);
        assert!((k.value_at(100.0) - 0.5).abs() < 1e-5);
        assert_eq!(k.value_at(150.0), k.value_at(100.0), "clamped");
    }

    #[test]
    fn default_knobs_add_a_timeout_to_clicking() {
        let s = Scenario::default();
        let hard = s.adaptive.apply(&s, 100.0);
        let easy = s.adaptive.apply(&s, 0.0);
        assert!(hard.targets.lifetime_s > 0.0 && hard.targets.lifetime_s < easy.targets.lifetime_s);
        assert!(hard.targets.radius < easy.targets.radius);
        assert!(hard.problems().is_empty() && easy.problems().is_empty(), "{:?}", hard.problems());
    }

    #[test]
    fn spread_scales_around_the_middle() {
        let mut s = Scenario::default();
        s.targets.spawn.yaw = Range(-20.0, 40.0);
        s.adaptive.knobs = vec![KnobRange { knob: Knob::Spread, easy: 0.5, hard: 2.0 }];
        let e = s.adaptive.apply(&s, 0.0);
        assert_eq!(e.targets.spawn.yaw, Range(-5.0, 25.0));
    }

    /// A simulated player who succeeds with a probability that falls as the level rises
    /// (50% at their true level `skill`). The staircase should settle where they succeed
    /// 85% of the time.
    fn settle(skill: f32, start: f32, known: bool) -> (f32, f32) {
        let mut rng = Rng::new(42);
        let mut c = Controller::new(start, 0.85, known);
        let p = |level: f32| 1.0 / (1.0 + ((level - skill) / 6.0).exp());
        for i in 0..400 {
            let ok = rng.next_f32() < p(c.level);
            c.update(i as f32, if ok { 1.0 } else { 0.0 });
        }
        (c.settled_level(), p(c.settled_level()))
    }

    #[test]
    fn staircase_settles_at_the_goal_rate() {
        for (skill, start) in [(70.0, 25.0), (30.0, 80.0), (55.0, 55.0)] {
            let (level, rate) = settle(skill, start, false);
            assert!((rate - 0.85).abs() < 0.07, "skill {skill} from {start}: level {level}, rate {rate}");
        }
    }

    #[test]
    fn reversals_shrink_the_step() {
        let mut c = Controller::new(50.0, 0.85, false);
        let a = c.update(1.0, 0.0);
        let b = c.update(2.0, 1.0);
        let c2 = c.update(3.0, 0.0);
        assert!((50.0 - a) > (b - a) * 3.0, "a failure drops more than a success climbs");
        assert!((b - c2) < (50.0 - a), "later failures move less: {a} {b} {c2}");
    }
}
