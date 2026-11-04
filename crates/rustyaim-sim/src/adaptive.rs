//! Adaptive mode: rate the player on one difficulty setting, in real units, and
//! keep play near the speed they hit about 85% of the time.
//!
//! Each scenario names one **knob**: target lifetime in seconds (timed clicking),
//! target speed in degrees per second (tracking), or target radius in meters. The
//! rating is a curve, the chance of success at each knob value, modeled as
//!
//! ```text
//! P(success at difficulty h) = lapse/2 + (1 − lapse) / (1 + e^((h − θ)/s))
//! ```
//!
//! where `h` is the knob on a log scale with higher meaning harder (so each step is
//! the same relative change), `θ` is where the player succeeds half the time, and
//! `s` is how quickly success falls off (small = consistent). The headline number
//! is the knob value at the goal rate (85%).
//!
//! The estimate is Bayesian, like Glicko: a posterior over (θ, s) on a grid, with
//! its uncertainty. Each outcome updates it by how surprising it was, so the first
//! 15–25 targets find the player's speed and later ones refine it. Between sessions
//! the uncertainty grows with the days away.
//!
//! Each target's difficulty comes from a **mix**: most targets near the goal
//! rate, some stretch targets (45–70% chance) that move the rating a lot when hit,
//! and a few easy ones. Picking by chance of success keeps the mix right at any
//! skill level.

use serde::{Deserialize, Serialize};

use crate::scenario::{MovementKind, Scenario, WeaponKind};
use crate::sim::Rng;

/// Chance of a slip (misclick, glance away) even on trivial targets.
const LAPSE: f32 = 0.04;
/// Weight of one tracking segment relative to one clicking target. Tracking ticks
/// within a segment are strongly correlated, so a segment counts as a few targets.
const SEGMENT_WEIGHT: f32 = 3.0;
const SLOPES: [f32; 6] = [0.04, 0.07, 0.11, 0.17, 0.26, 0.4];
const GRID: usize = 161;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Adaptive {
    /// Success rate the headline number is measured at, 0.6–0.95.
    pub goal: f32,
    pub knob: Knob,
    /// Easiest and hardest values the trainer will use, in the knob's units. 0 means
    /// a default for the scenario.
    pub easy: f32,
    pub hard: f32,
    /// First guess at the player's goal-rate value. 0 means a default.
    pub start: f32,
    /// Seconds per tracking segment: the target speed is set at the start of each
    /// segment and the share of time on target is the outcome.
    pub segment_s: f32,
}

impl Default for Adaptive {
    fn default() -> Self {
        Adaptive { goal: 0.85, knob: Knob::Auto, easy: 0.0, hard: 0.0, start: 0.0, segment_s: 2.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Knob {
    /// Lifetime for hitscan scenarios, target speed for moving beam scenarios,
    /// radius otherwise.
    Auto,
    /// Seconds before a target times out. Lower is harder.
    Lifetime,
    /// Target speed across the view in degrees per second. Higher is harder.
    TargetSpeed,
    /// Target radius in meters. Lower is harder.
    Radius,
}

impl Knob {
    pub fn unit(self) -> &'static str {
        match self {
            Knob::Lifetime => "s",
            Knob::TargetSpeed => "°/s",
            Knob::Radius | Knob::Auto => "m",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Knob::Auto => "Automatic",
            Knob::Lifetime => "Target lifetime",
            Knob::TargetSpeed => "Target speed",
            Knob::Radius => "Target size",
        }
    }

    /// Whether a bigger value is harder.
    pub fn higher_is_harder(self) -> bool {
        self == Knob::TargetSpeed
    }

    /// Log-scale difficulty of a value: higher is harder.
    pub fn h(self, v: f32) -> f32 {
        let l = v.max(1e-4).ln();
        if self.higher_is_harder() { l } else { -l }
    }

    pub fn value(self, h: f32) -> f32 {
        if self.higher_is_harder() { h.exp() } else { (-h).exp() }
    }

    pub fn format(self, v: f32) -> String {
        match self {
            Knob::Lifetime => format!("{v:.2} s"),
            Knob::TargetSpeed => format!("{v:.0} °/s"),
            Knob::Radius | Knob::Auto => format!("{v:.2} m"),
        }
    }
}

/// The knob, range and first guess for a scenario, with defaults filled in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved {
    pub knob: Knob,
    pub easy: f32,
    pub hard: f32,
    pub start: f32,
    pub goal: f32,
    pub segment_s: f32,
}

/// Mean angular speed of a scenario's targets in degrees per second.
pub fn angular_speed(s: &Scenario) -> f32 {
    let d = (s.targets.spawn.distance.0 + s.targets.spawn.distance.1) / 2.0;
    let mps = (s.targets.movement.speed.0 + s.targets.movement.speed.1) / 2.0;
    (mps / d.max(0.1)).to_degrees()
}

impl Adaptive {
    pub fn resolve(&self, s: &Scenario) -> Resolved {
        let beam = s.weapon.kind == WeaponKind::Beam;
        let moving = s.targets.movement.kind != MovementKind::Static;
        let knob = match self.knob {
            Knob::Auto if !beam => Knob::Lifetime,
            Knob::Auto if moving => Knob::TargetSpeed,
            Knob::Auto => Knob::Radius,
            k => k,
        };
        let (easy, hard, start) = match knob {
            Knob::Lifetime => {
                let life = if s.targets.lifetime_s > 0.0 { s.targets.lifetime_s } else { 1.2 };
                (3.0, 0.25, life)
            }
            Knob::TargetSpeed => {
                let v = angular_speed(s).max(3.0);
                (3.0, 180.0, v)
            }
            Knob::Radius | Knob::Auto => (s.targets.radius * 2.0, s.targets.radius * 0.3, s.targets.radius),
        };
        let pick = |v: f32, d: f32| if v > 0.0 { v } else { d };
        Resolved {
            knob,
            easy: pick(self.easy, easy),
            hard: pick(self.hard, hard),
            start: pick(self.start, start),
            goal: self.goal,
            segment_s: self.segment_s,
        }
    }

    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if !(0.6..=0.95).contains(&self.goal) {
            p.push("adaptive goal must be 0.6–0.95".into());
        }
        if !(0.5..=10.0).contains(&self.segment_s) {
            p.push("adaptive tracking segment must be 0.5–10 s".into());
        }
        for (name, v) in [("easy", self.easy), ("hard", self.hard), ("start", self.start)] {
            if !(v.is_finite() && v >= 0.0) {
                p.push(format!("adaptive {name} can't be negative"));
            }
        }
        if self.easy > 0.0 && self.hard > 0.0 && self.knob != Knob::Auto {
            let (he, hh) = (self.knob.h(self.easy), self.knob.h(self.hard));
            if he >= hh {
                p.push(format!(
                    "adaptive easy must be easier than hard ({} is {})",
                    self.knob.label().to_lowercase(),
                    if self.knob.higher_is_harder() { "harder when higher" } else { "harder when lower" }
                ));
            }
        }
        p
    }
}

/// How each target's difficulty is chosen around the player's rating.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mix {
    /// Mostly targets near the goal rate.
    Steady,
    /// 65% near the goal rate, 20% stretch, 15% easy.
    #[default]
    Balanced,
    /// More stretch targets.
    Push,
}

impl Mix {
    /// Shares of (near, stretch, easy) targets.
    fn shares(self) -> (f32, f32, f32) {
        match self {
            Mix::Steady => (0.85, 0.05, 0.10),
            Mix::Balanced => (0.65, 0.20, 0.15),
            Mix::Push => (0.50, 0.35, 0.15),
        }
    }
}

/// What kind of target was picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Near,
    Stretch,
    Easy,
    /// While the rating is still uncertain.
    Calibrate,
}

impl Pick {
    pub fn as_str(self) -> &'static str {
        match self {
            Pick::Near => "near",
            Pick::Stretch => "stretch",
            Pick::Easy => "easy",
            Pick::Calibrate => "calibrate",
        }
    }
}

/// A rating carried between runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prior {
    pub theta: f32,
    pub slope: f32,
    /// Uncertainty of the goal-rate point, in log units.
    pub sd: f32,
    /// Days since that rating was set.
    pub days: f32,
    /// Best value ever succeeded at, in knob units.
    pub fastest: Option<f32>,
}

/// The current rating: a posterior over (θ, s).
#[derive(Debug, Clone)]
pub struct Rating {
    pub cfg: Resolved,
    thetas: Vec<f32>,
    /// Log posterior, `thetas.len() × SLOPES.len()`.
    logp: Vec<f64>,
    outcomes: Vec<f32>,
    /// (seconds, goal-rate value)
    pub timeline: Vec<(f32, f32)>,
    pub start_value: f32,
    pub best_before: Option<f32>,
    pub fastest: Option<f32>,
    last_pick: Pick,
    last_failed: bool,
}

/// Summary statistics of the posterior.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    /// Goal-rate point on the log scale, and its standard deviation.
    pub h_goal: f32,
    pub sd: f32,
    pub theta: f32,
    pub slope: f32,
}

fn logistic(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn p_success(h: f32, theta: f32, s: f32) -> f32 {
    LAPSE / 2.0 + (1.0 - LAPSE) * logistic((theta - h) / s)
}

impl Rating {
    pub fn new(cfg: Resolved, prior: Option<Prior>) -> Rating {
        let k = cfg.knob;
        let (he, hh) = (k.h(cfg.easy), k.h(cfg.hard));
        let (lo, hi) = (he.min(hh) - 1.0, he.max(hh) + 1.0);
        let thetas: Vec<f32> = (0..GRID).map(|i| lo + (hi - lo) * i as f32 / (GRID - 1) as f32).collect();
        let logit_goal = (cfg.goal / (1.0 - cfg.goal)).ln();
        let (theta0, sd0, slope0, slope_sd) = match prior {
            Some(p) => {
                // Skill drifts: widen the prior with time away, like Glicko.
                let sd = (p.sd * p.sd + (0.02 * p.days).powi(2)).sqrt().clamp(0.05, 0.6);
                (p.theta, sd, p.slope, 0.3)
            }
            None => (k.h(cfg.start) + 0.12 * logit_goal, 0.6, 0.12, 1.0),
        };
        let mut logp = Vec::with_capacity(GRID * SLOPES.len());
        for &t in &thetas {
            for &s in &SLOPES {
                let zt = (t - theta0) / sd0;
                let zs = (s / slope0).ln() / slope_sd;
                logp.push(f64::from(-0.5 * zt * zt - 0.5 * zs * zs));
            }
        }
        let mut r = Rating {
            cfg,
            thetas,
            logp,
            outcomes: Vec::new(),
            timeline: Vec::new(),
            start_value: 0.0,
            best_before: prior.and_then(|p| p.fastest),
            fastest: None,
            last_pick: Pick::Near,
            last_failed: false,
        };
        r.start_value = r.value();
        r.timeline.push((0.0, r.start_value));
        r
    }

    fn weights(&self) -> Vec<f64> {
        let m = self.logp.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let w: Vec<f64> = self.logp.iter().map(|l| (l - m).exp()).collect();
        let sum: f64 = w.iter().sum();
        w.into_iter().map(|x| x / sum).collect()
    }

    pub fn estimate(&self) -> Estimate {
        let logit_goal = (self.cfg.goal / (1.0 - self.cfg.goal)).ln();
        let w = self.weights();
        let (mut mh, mut mh2, mut mt, mut ms) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for (i, &t) in self.thetas.iter().enumerate() {
            for (j, &s) in SLOPES.iter().enumerate() {
                let p = w[i * SLOPES.len() + j];
                let h = f64::from(t - s * logit_goal);
                mh += p * h;
                mh2 += p * h * h;
                mt += p * f64::from(t);
                ms += p * f64::from(s).ln();
            }
        }
        Estimate {
            h_goal: mh as f32,
            sd: (mh2 - mh * mh).max(0.0).sqrt() as f32,
            theta: mt as f32,
            slope: ms.exp() as f32,
        }
    }

    /// The goal-rate value in knob units (the headline number).
    pub fn value(&self) -> f32 {
        let e = self.estimate();
        self.cfg.knob.value(e.h_goal.clamp(self.h_easy(), self.h_hard()))
    }

    /// Goal-rate value one standard deviation easier and harder.
    pub fn range(&self) -> (f32, f32) {
        let e = self.estimate();
        (self.cfg.knob.value(e.h_goal - e.sd), self.cfg.knob.value(e.h_goal + e.sd))
    }

    fn h_easy(&self) -> f32 {
        self.cfg.knob.h(self.cfg.easy)
    }

    fn h_hard(&self) -> f32 {
        self.cfg.knob.h(self.cfg.hard)
    }

    /// Whether the rating has settled enough to start mixing in stretch and easy targets.
    pub fn calibrated(&self) -> bool {
        self.estimate().sd < 0.12 || self.outcomes.len() >= 25
    }

    /// Chooses the next target's difficulty. Returns the knob value, the planned
    /// chance of success, and what kind of pick it was.
    pub fn pick(&mut self, rng: &mut Rng, mix: Mix) -> (f32, f32, Pick) {
        let e = self.estimate();
        let (near, stretch, _) = mix.shares();
        let (kind, chance) = if !self.calibrated() {
            (Pick::Calibrate, 0.65 + rng.next_f32() * 0.25)
        } else {
            let r = rng.next_f32();
            // Guard rails: no stretch right after a miss or two in a row.
            let stretch_ok = !self.last_failed && self.last_pick != Pick::Stretch;
            if r < near || (!stretch_ok && r < near + stretch) {
                (Pick::Near, 0.80 + rng.next_f32() * 0.10)
            } else if r < near + stretch {
                (Pick::Stretch, 0.45 + rng.next_f32() * 0.25)
            } else {
                (Pick::Easy, 0.95 + rng.next_f32() * 0.03)
            }
        };
        // Invert the model at the posterior mean: the difficulty with that chance.
        let q = ((chance - LAPSE / 2.0) / (1.0 - LAPSE)).clamp(0.01, 0.99);
        let h = (e.theta - e.slope * (q / (1.0 - q)).ln()).clamp(self.h_easy(), self.h_hard());
        self.last_pick = kind;
        (self.cfg.knob.value(h), chance, kind)
    }

    /// Records an outcome at a knob value: 1 = success, 0 = failure, or the share
    /// of time on target for a tracking segment. Returns true for a breakthrough:
    /// a success harder than anything before.
    pub fn update(&mut self, t: f32, value: f32, outcome: f32, tracking: bool) -> bool {
        let outcome = outcome.clamp(0.0, 1.0);
        let h = self.cfg.knob.h(value);
        let w = if tracking { SEGMENT_WEIGHT } else { 1.0 };
        for (i, &th) in self.thetas.iter().enumerate() {
            for (j, &s) in SLOPES.iter().enumerate() {
                let p = p_success(h, th, s);
                let ll = w * (outcome * p.ln() + (1.0 - outcome) * (1.0 - p).ln());
                self.logp[i * SLOPES.len() + j] += f64::from(ll);
            }
        }
        self.outcomes.push(outcome);
        let success = outcome >= if tracking { self.cfg.goal } else { 0.5 };
        self.last_failed = !success;
        self.timeline.push((t, self.value()));
        let k = self.cfg.knob;
        let harder = |a: f32, b: f32| k.h(a) > k.h(b) + 1e-6;
        let mut breakthrough = false;
        if success {
            if self.fastest.is_none_or(|f| harder(value, f)) {
                breakthrough = self.best_before.is_some_and(|b| harder(value, b)) && self.outcomes.len() > 5;
                self.fastest = Some(value);
            }
            if let Some(b) = self.best_before
                && harder(value, b)
            {
                self.best_before = Some(value);
            }
        }
        breakthrough
    }

    pub fn success_rate(&self) -> Option<f32> {
        (!self.outcomes.is_empty()).then(|| self.outcomes.iter().sum::<f32>() / self.outcomes.len() as f32)
    }

    pub fn outcomes(&self) -> usize {
        self.outcomes.len()
    }

    /// The chance of success at a value, from the current estimate.
    pub fn chance_at(&self, value: f32) -> f32 {
        let e = self.estimate();
        p_success(self.cfg.knob.h(value), e.theta, e.slope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Resolved {
        Resolved { knob: Knob::Lifetime, easy: 3.0, hard: 0.25, start: 1.2, goal: 0.85, segment_s: 2.0 }
    }

    /// A simulated player whose true 85% lifetime is `true85` with slope `s`.
    fn player(true85: f32, s: f32) -> impl Fn(f32) -> f32 {
        let theta = Knob::Lifetime.h(true85) + s * (0.85f32 / 0.15).ln();
        move |v: f32| p_success(Knob::Lifetime.h(v), theta, s)
    }

    fn play(r: &mut Rating, p: &dyn Fn(f32) -> f32, n: usize, mix: Mix, seed: u64) -> f32 {
        let mut rng = Rng::new(seed);
        let mut hits = 0.0;
        for i in 0..n {
            let (v, _, _) = r.pick(&mut rng, mix);
            let ok = rng.next_f32() < p(v);
            hits += f32::from(u8::from(ok));
            r.update(i as f32, v, if ok { 1.0 } else { 0.0 }, false);
        }
        hits / n as f32
    }

    #[test]
    fn finds_the_players_speed_quickly() {
        for (true85, start) in [(0.5, 1.2), (1.6, 1.2), (0.35, 2.0)] {
            let p = player(true85, 0.12);
            let mut r = Rating::new(Resolved { start, ..cfg() }, None);
            play(&mut r, &p, 25, Mix::Balanced, 3);
            let v = r.value();
            assert!((v / true85 - 1.0).abs() < 0.25, "after 25 targets: {v} vs {true85}");
            play(&mut r, &p, 75, Mix::Balanced, 4);
            let v = r.value();
            assert!((v / true85 - 1.0).abs() < 0.1, "after 100 targets: {v} vs {true85}");
        }
    }

    #[test]
    fn mix_keeps_success_near_the_goal() {
        let p = player(0.6, 0.1);
        let prior =
            Prior { theta: Knob::Lifetime.h(0.6) + 0.1 * 1.7346, slope: 0.1, sd: 0.05, days: 0.0, fastest: None };
        for (mix, lo, hi) in [(Mix::Steady, 0.82, 0.92), (Mix::Balanced, 0.78, 0.9), (Mix::Push, 0.72, 0.86)] {
            let mut r = Rating::new(cfg(), Some(prior));
            let rate = play(&mut r, &p, 600, mix, 9);
            assert!(rate > lo && rate < hi, "{mix:?}: {rate}");
        }
    }

    #[test]
    fn surprises_move_the_rating_most() {
        let prior =
            Prior { theta: Knob::Lifetime.h(0.6) + 0.12 * 1.7346, slope: 0.12, sd: 0.1, days: 0.0, fastest: Some(0.5) };
        let base = Rating::new(cfg(), Some(prior)).value();
        let moved = |v: f32, outcome: f32| {
            let mut r = Rating::new(cfg(), Some(prior));
            r.update(1.0, v, outcome, false);
            (r.value() - base).abs()
        };
        // Hitting a stretch target (0.42 s) says more than hitting one at your level...
        assert!(moved(0.42, 1.0) > 1.5 * moved(0.6, 1.0), "{} vs {}", moved(0.42, 1.0), moved(0.6, 1.0));
        // ...and missing a stretch target costs less than missing one at your level.
        assert!(moved(0.42, 0.0) < moved(0.6, 0.0), "{} vs {}", moved(0.42, 0.0), moved(0.6, 0.0));
        let mut a = Rating::new(cfg(), Some(prior));
        assert!(!a.update(1.0, 0.42, 1.0, false), "no breakthroughs in the first few targets");
        assert_eq!(a.fastest, Some(0.42));
    }

    #[test]
    fn breakthroughs_beat_the_previous_best() {
        let prior =
            Prior { theta: Knob::Lifetime.h(0.6) + 0.12 * 1.7346, slope: 0.12, sd: 0.1, days: 0.0, fastest: Some(0.5) };
        let mut r = Rating::new(cfg(), Some(prior));
        for i in 0..6 {
            assert!(!r.update(i as f32, 0.7, 1.0, false));
        }
        assert!(r.update(7.0, 0.45, 1.0, false), "faster than the 0.5 s best");
        assert!(!r.update(8.0, 0.48, 1.0, false), "slower than this run's 0.45");
    }

    #[test]
    fn time_away_widens_the_prior() {
        let p = |days| Prior { theta: 0.0, slope: 0.12, sd: 0.05, days, fastest: None };
        let fresh = Rating::new(cfg(), Some(p(0.0))).estimate().sd;
        let later = Rating::new(cfg(), Some(p(30.0))).estimate().sd;
        assert!(later > fresh * 2.0, "{fresh} -> {later}");
    }

    #[test]
    fn tracking_speed_is_harder_when_higher() {
        let k = Knob::TargetSpeed;
        assert!(k.h(60.0) > k.h(30.0));
        assert!((k.value(k.h(42.0)) - 42.0).abs() < 1e-3);
        assert!(Knob::Lifetime.h(0.5) > Knob::Lifetime.h(1.0));
        let a = Adaptive { knob: Knob::TargetSpeed, easy: 90.0, hard: 10.0, ..Default::default() };
        assert!(!a.problems().is_empty(), "easy must be easier than hard");
    }

    #[test]
    fn defaults_pick_a_knob_by_scenario_type() {
        let mut s = Scenario::default();
        assert_eq!(s.adaptive.resolve(&s).knob, Knob::Lifetime);
        s.weapon.kind = WeaponKind::Beam;
        s.targets.movement.kind = MovementKind::Strafe;
        let r = s.adaptive.resolve(&s);
        assert_eq!(r.knob, Knob::TargetSpeed);
        assert!((r.start - angular_speed(&s)).abs() < 1e-3);
    }
}
