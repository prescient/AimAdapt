//! Adaptive mode: rate the player on one difficulty setting, in real units, and
//! keep play near the speed they hit about 85% of the time.
//!
//! Each scenario names one **knob**: target lifetime in seconds (timed clicking),
//! target speed in degrees per second (tracking), target radius in meters, or kill
//! pace in seconds between kills (speed switching). The
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
//! **Runs, not targets.** The first adaptive run on a scenario is a calibration
//! run: each target (or tracking segment) gets its own difficulty so the rating
//! finds the player quickly. Every later run plays at one fixed difficulty, picked
//! from a **mix**: most runs near the goal rate, some stretch runs that move the
//! rating a lot when you hold the goal, and a few easy ones. Picking by chance of
//! success keeps the mix right at any skill level. A run at one value is still
//! plenty of evidence (40–80 targets or ~30 segments), and the rating absorbs it
//! the same way. A breakthrough is holding the goal rate for a whole run at a
//! value harder than your best.

use serde::{Deserialize, Serialize};

use crate::rng::Rng;
use crate::scenario::{MovementKind, Scenario, WeaponKind};

/// Chance of a slip (misclick, glance away) even on trivial targets.
pub const LAPSE: f32 = 0.04;
/// Weight of one tracking segment relative to one clicking target. Tracking ticks
/// within a segment are strongly correlated, so a segment counts as a few targets.
const SEGMENT_WEIGHT: f32 = 3.0;
const SLOPES: [f32; 6] = [0.04, 0.07, 0.11, 0.17, 0.26, 0.4];
/// A fixed-difficulty run needs at least this many outcomes to count as holding the
/// goal, and a short one also needs a margin over the goal rate (see [`EVIDENCE_FULL`]).
pub const MIN_RUN_OUTCOMES: usize = 10;
/// A run with this much evidence, in target-equivalents (a clicking target counts 1,
/// a tracking segment [`SEGMENT_WEIGHT`]), holds the goal at the goal rate itself.
/// A shorter run needs a margin of one binomial standard deviation on top.
const EVIDENCE_FULL: f32 = 30.0;
/// A run this far below the goal rate rules out a stretch run next.
const WELL_BELOW: f32 = 0.10;
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
    /// Tracking: count time only while a target is engaged (damaged and not yet
    /// healed back to full), so moving to a fresh target after a kill isn't off
    /// target. For switching, with `regen_per_s` so an abandoned target heals.
    pub engaged_only: bool,
}

impl Default for Adaptive {
    fn default() -> Self {
        Adaptive { goal: 0.85, knob: Knob::Auto, easy: 0.0, hard: 0.0, start: 0.0, segment_s: 2.0, engaged_only: false }
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
    /// Seconds allowed from one kill to the next. A kill inside that time is a
    /// success; each stretch of that length with no kill is a failure. Lower is
    /// harder. Rates switching on pace rather than on each target's own timer.
    KillPace,
}

impl Knob {
    pub fn unit(self) -> &'static str {
        match self {
            Knob::Lifetime | Knob::KillPace => "s",
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
            Knob::KillPace => "Kill pace",
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
            Knob::Lifetime | Knob::KillPace => format!("{v:.2} s"),
            Knob::TargetSpeed => format!("{v:.1} °/s"),
            Knob::Radius | Knob::Auto => format!("{v:.3} m"),
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
            Knob::KillPace => (3.0, 0.3, 1.5),
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

    /// Problems with the adaptive settings. `resolved` is these settings with the
    /// defaults for the scenario filled in, so easy and hard are compared even when
    /// the file sets only one of them.
    pub fn problems(&self, resolved: &Resolved) -> Vec<String> {
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
        let knob = resolved.knob;
        if knob.h(resolved.easy) >= knob.h(resolved.hard) {
            p.push(format!(
                "adaptive easy must be easier than hard ({} is {}; easy is {}, hard is {})",
                knob.label().to_lowercase(),
                if knob.higher_is_harder() { "harder when higher" } else { "harder when lower" },
                knob.format(resolved.easy),
                knob.format(resolved.hard),
            ));
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

/// The kind of each run in a block of `reps` runs of one drill, built from the mix
/// (`rep` counts from 1). `None` for a single run, which keeps the random draw, and
/// for a `rep` outside the block. Counts are the mix's shares of the block, rounded.
/// The first run is at level. Easy runs go at the end, never first. Stretch runs sit
/// in the middle, spread evenly, never first, never last and never side by side; if
/// the rounded count doesn't fit under those rules, as many as fit are used.
pub fn planned_kind(rep: u32, reps: u32, mix: Mix) -> Option<Pick> {
    if reps < 2 || rep == 0 || rep > reps {
        return None;
    }
    let (_, stretch_share, easy_share) = mix.shares();
    let n = reps as usize;
    let easy = ((easy_share * reps as f32).round() as usize).min(n - 1);
    // Positions 2 to n - max(easy, 1) can take a stretch run, no two side by side.
    let room = n.saturating_sub(easy.max(1) + 1);
    let stretch = ((stretch_share * reps as f32).round() as usize).min(room.div_ceil(2));
    let at = rep as usize;
    if at > n - easy {
        return Some(Pick::Easy);
    }
    if stretch > 0 && at >= 2 {
        let i = at - 2;
        let is_stretch = if 2 * stretch > room {
            i.is_multiple_of(2)
        } else {
            (0..stretch).any(|j| (2 * j + 1) * room / (2 * stretch) == i)
        };
        if is_stretch {
            return Some(Pick::Stretch);
        }
    }
    Some(Pick::Near)
}

/// What kind of difficulty was picked, for a target or a whole run.
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

    pub fn parse(s: &str) -> Option<Pick> {
        [Pick::Near, Pick::Stretch, Pick::Easy, Pick::Calibrate].into_iter().find(|p| p.as_str() == s)
    }

    /// How the game names a run of this kind.
    pub fn run_label(self) -> &'static str {
        match self {
            Pick::Near => "At your level",
            Pick::Stretch => "Stretch run",
            Pick::Easy => "Easy run",
            Pick::Calibrate => "Calibration run",
        }
    }
}

/// How a run came out against the chance of success it was planned at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanVerdict {
    Below,
    AsPlanned,
    Above,
}

/// The gap from plan that luck alone explains in a run with this much evidence (in
/// target-equivalents, as for `holds_goal`): 1.5 binomial standard deviations, never
/// under five points. Zero evidence means anything goes.
pub fn plan_tolerance(planned: f32, evidence: f32) -> f32 {
    if evidence <= 0.0 {
        return 1.0;
    }
    (1.5 * (planned * (1.0 - planned) / evidence).sqrt()).max(0.05)
}

/// Judges a run's success rate against the chance it was planned at.
pub fn judge_against_plan(planned: f32, rate: f32, evidence: f32) -> PlanVerdict {
    let gap = rate - planned;
    if gap.abs() <= plan_tolerance(planned, evidence) + 1e-6 {
        PlanVerdict::AsPlanned
    } else if gap > 0.0 {
        PlanVerdict::Above
    } else {
        PlanVerdict::Below
    }
}

/// A rating carried between runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prior {
    pub theta: f32,
    pub slope: f32,
    /// Uncertainty of the goal-rate point, in log units.
    pub sd: f32,
    /// Uncertainty of the slope, as the standard deviation of ln(slope). `None` for
    /// runs saved before it was recorded; the old fixed width is used then.
    pub slope_sd: Option<f32>,
    /// Days since that rating was set.
    pub days: f32,
    /// Hardest value the goal rate was held at for a whole run, in knob units.
    pub fastest: Option<f32>,
    /// The previous run's pick and success rate, for the guard rails.
    pub last_run: Option<(Pick, f32)>,
}

/// Days away after which an unsure rating is no longer trusted for a whole run.
pub const BREAK_DAYS: f32 = 3.0;
/// The sd (log units) below which a rating counts as settled.
pub const SETTLED_SD: f32 = 0.12;
/// How much the sd grows per day away, in log units.
pub const DRIFT_PER_DAY: f32 = 0.02;

impl Prior {
    /// The prior's sd after time away: skill drifts, so widen it, like Glicko.
    fn widened_sd(&self) -> f32 {
        (self.sd * self.sd + (DRIFT_PER_DAY * self.days).powi(2)).sqrt().clamp(0.05, 0.6)
    }

    /// Whether the next run should be a calibration run again: at least [`BREAK_DAYS`]
    /// days away and the widened sd back at [`SETTLED_SD`] or more. A settled rating
    /// (sd 0.05) gets there after about five and a half days; a fresh, unsure one
    /// from a short first run does not, because no time has passed.
    pub fn needs_calibration(&self) -> bool {
        self.days >= BREAK_DAYS && self.widened_sd() >= SETTLED_SD
    }
}

/// The current rating: a posterior over (θ, s).
#[derive(Debug, Clone)]
pub struct Rating {
    pub cfg: Resolved,
    thetas: Vec<f32>,
    /// Log posterior, `thetas.len() × SLOPES.len()`.
    logp: Vec<f64>,
    outcomes: Vec<f32>,
    /// The outcomes so far in target-equivalents: 1 per target, [`SEGMENT_WEIGHT`] per segment.
    evidence: f32,
    /// (seconds, goal-rate value)
    pub timeline: Vec<(f32, f32)>,
    pub start_value: f32,
    pub best_before: Option<f32>,
    /// Set by [`Rating::finish_run`] when this run held the goal rate.
    pub fastest: Option<f32>,
    last_pick: Pick,
    last_failed: bool,
    last_run: Option<(Pick, f32)>,
}

/// Summary statistics of the posterior.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    /// Goal-rate point on the log scale, and its standard deviation.
    pub h_goal: f32,
    pub sd: f32,
    pub theta: f32,
    pub slope: f32,
    /// Standard deviation of ln(slope).
    pub slope_sd: f32,
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
        let (theta0, sd0, slope0, slope_sd, anchored) = match prior {
            Some(p) => {
                // Skill drifts: widen the prior with time away, like Glicko.
                (p.theta, p.widened_sd(), p.slope, p.slope_sd.unwrap_or(0.3).clamp(0.15, 1.0), true)
            }
            None => (k.h(cfg.start) + 0.12 * logit_goal, 0.6, 0.12, 1.0, false),
        };
        let mut logp = Vec::with_capacity(GRID * SLOPES.len());
        for &t in &thetas {
            for &s in &SLOPES {
                // With a prior, `sd` was measured on the goal-rate point, so apply it there.
                let zt = if anchored {
                    ((t - s * logit_goal) - (theta0 - slope0 * logit_goal)) / sd0
                } else {
                    (t - theta0) / sd0
                };
                let zs = (s / slope0).ln() / slope_sd;
                logp.push(f64::from(-0.5 * zt * zt - 0.5 * zs * zs));
            }
        }
        let mut r = Rating {
            cfg,
            thetas,
            logp,
            outcomes: Vec::new(),
            evidence: 0.0,
            timeline: Vec::new(),
            start_value: 0.0,
            best_before: prior.and_then(|p| p.fastest),
            fastest: None,
            last_pick: Pick::Near,
            last_failed: false,
            last_run: prior.and_then(|p| p.last_run),
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
        let (mut mh, mut mh2, mut mt, mut ms, mut ms2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for (i, &t) in self.thetas.iter().enumerate() {
            for (j, &s) in SLOPES.iter().enumerate() {
                let p = w[i * SLOPES.len() + j];
                let h = f64::from(t - s * logit_goal);
                mh += p * h;
                mh2 += p * h * h;
                mt += p * f64::from(t);
                let ls = f64::from(s).ln();
                ms += p * ls;
                ms2 += p * ls * ls;
            }
        }
        Estimate {
            h_goal: mh as f32,
            sd: (mh2 - mh * mh).max(0.0).sqrt() as f32,
            theta: mt as f32,
            slope: ms.exp() as f32,
            slope_sd: (ms2 - ms * ms).max(0.0).sqrt() as f32,
        }
    }

    /// The goal-rate value in knob units (the headline number).
    pub fn value(&self) -> f32 {
        let e = self.estimate();
        // Scenario files are checked for easy ≥ hard, but `clamp` panics on reversed
        // limits, so don't trust that here.
        let (lo, hi) = (self.h_easy().min(self.h_hard()), self.h_easy().max(self.h_hard()));
        self.cfg.knob.value(e.h_goal.clamp(lo, hi))
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
        self.estimate().sd < SETTLED_SD || self.outcomes.len() >= 25
    }

    /// Chooses one difficulty for a whole run from the mix. Guard rails: no stretch
    /// run right after one well below the goal, and never two stretch runs in a row.
    pub fn pick_run(&self, rng: &mut Rng, mix: Mix) -> (f32, f32, Pick) {
        let (near, stretch, _) = mix.shares();
        let stretch_ok = self.last_run.is_none_or(|(p, s)| p != Pick::Stretch && s >= self.cfg.goal - WELL_BELOW);
        let r = rng.next_f32();
        let (kind, chance) = if r < near || (!stretch_ok && r < near + stretch) {
            (Pick::Near, 0.80 + rng.next_f32() * 0.10)
        } else if r < near + stretch {
            (Pick::Stretch, 0.55 + rng.next_f32() * 0.17)
        } else {
            (Pick::Easy, 0.93 + rng.next_f32() * 0.04)
        };
        (self.value_at(chance), chance, kind)
    }

    /// Like [`Rating::pick_run`] for a run whose kind the plan has already set (see
    /// [`planned_kind`]): only the chance of success is drawn, inside that kind's band.
    /// A planned stretch run still becomes an at-level run when a stretch isn't allowed.
    pub fn pick_planned(&self, rng: &mut Rng, planned: Pick) -> (f32, f32, Pick) {
        let stretch_ok = self.last_run.is_none_or(|(p, s)| p != Pick::Stretch && s >= self.cfg.goal - WELL_BELOW);
        let (kind, chance) = match planned {
            Pick::Stretch if stretch_ok => (Pick::Stretch, 0.55 + rng.next_f32() * 0.17),
            Pick::Easy => (Pick::Easy, 0.93 + rng.next_f32() * 0.04),
            _ => (Pick::Near, 0.80 + rng.next_f32() * 0.10),
        };
        (self.value_at(chance), chance, kind)
    }

    /// The knob value with this chance of success, at the posterior mean.
    fn value_at(&self, chance: f32) -> f32 {
        let e = self.estimate();
        let q = ((chance - LAPSE / 2.0) / (1.0 - LAPSE)).clamp(0.01, 0.99);
        let h = (e.theta - e.slope * (q / (1.0 - q)).ln()).clamp(self.h_easy(), self.h_hard());
        self.cfg.knob.value(h)
    }

    /// Whether the run so far holds the goal. It needs at least `MIN_RUN_OUTCOMES`
    /// outcomes and a success rate of the goal plus a margin, capped at 100%. The
    /// margin is one binomial standard deviation, sqrt(goal * (1 - goal) / evidence),
    /// and is zero once there are `EVIDENCE_FULL` target-equivalents. So a short run
    /// can't hold the goal by luck: with an 85% goal, 9 of 10 doesn't, 10 of 10 does.
    fn holds_goal(&self) -> bool {
        let goal = self.cfg.goal;
        let margin = if self.evidence >= EVIDENCE_FULL { 0.0 } else { (goal * (1.0 - goal) / self.evidence).sqrt() };
        self.outcomes.len() >= MIN_RUN_OUTCOMES
            && self.success_rate().is_some_and(|r| r >= (goal + margin).min(1.0) - 1e-6)
    }

    /// Ends a fixed-difficulty run at `value`. If the run held the goal (see
    /// `holds_goal`), that value becomes this run's best, and the result is true for a
    /// breakthrough: held at a value harder than the best before.
    pub fn finish_run(&mut self, value: f32) -> bool {
        if !self.holds_goal() {
            return false;
        }
        self.fastest = Some(value);
        let k = self.cfg.knob;
        self.best_before.is_some_and(|b| k.h(value) > k.h(b) + 1e-6)
    }

    /// Chooses the next target's difficulty in a calibration run. Returns the knob
    /// value, the planned chance of success, and what kind of pick it was.
    pub fn pick(&mut self, rng: &mut Rng, mix: Mix) -> (f32, f32, Pick) {
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
        self.last_pick = kind;
        (self.value_at(chance), chance, kind)
    }

    /// Records an outcome at a knob value: 1 = success, 0 = failure, or the share
    /// of time on target for a tracking segment.
    pub fn update(&mut self, t: f32, value: f32, outcome: f32, tracking: bool) {
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
        self.evidence += w;
        let success = outcome >= if tracking { self.cfg.goal } else { 0.5 };
        self.last_failed = !success;
        self.timeline.push((t, self.value()));
    }

    pub fn success_rate(&self) -> Option<f32> {
        (!self.outcomes.is_empty()).then(|| self.outcomes.iter().sum::<f32>() / self.outcomes.len() as f32)
    }

    pub fn outcomes(&self) -> usize {
        self.outcomes.len()
    }

    /// The outcomes so far in target-equivalents: 1 per clicking target, a tracking
    /// segment counts more.
    pub fn evidence(&self) -> f32 {
        self.evidence
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

    #[test]
    fn plan_tolerance_has_a_five_point_floor_and_widens_for_short_runs() {
        // Long run: luck alone explains under a point, so the floor applies.
        assert_eq!(plan_tolerance(0.95, 200.0), 0.05);
        // Short run: 1.5 * sqrt(0.6 * 0.4 / 10) is about 0.232.
        assert!((plan_tolerance(0.6, 10.0) - 0.232).abs() < 0.001);
        assert!(plan_tolerance(0.6, 10.0) > plan_tolerance(0.6, 40.0));
        // No evidence: anything is as planned.
        assert_eq!(plan_tolerance(0.6, 0.0), 1.0);
    }

    #[test]
    fn a_run_is_judged_against_its_plan_with_room_for_luck() {
        // Stretch (0.6) over 100 targets: tolerance 1.5 * sqrt(0.0024) = 0.0735.
        assert_eq!(judge_against_plan(0.6, 0.6, 100.0), PlanVerdict::AsPlanned);
        assert_eq!(judge_against_plan(0.6, 0.67, 100.0), PlanVerdict::AsPlanned);
        assert_eq!(judge_against_plan(0.6, 0.54, 100.0), PlanVerdict::AsPlanned);
        assert_eq!(judge_against_plan(0.6, 0.68, 100.0), PlanVerdict::Above);
        assert_eq!(judge_against_plan(0.6, 0.52, 100.0), PlanVerdict::Below);
        // Easy (0.95) over 200 targets: the five-point floor, so 90% is the edge.
        assert_eq!(judge_against_plan(0.95, 0.90, 200.0), PlanVerdict::AsPlanned);
        assert_eq!(judge_against_plan(0.95, 0.89, 200.0), PlanVerdict::Below);
        assert_eq!(judge_against_plan(0.95, 1.0, 200.0), PlanVerdict::AsPlanned);
        assert_eq!(judge_against_plan(0.7, 0.76, 200.0), PlanVerdict::Above);
        // A short run: 3 of 10 against a plan of 0.6 is still as planned.
        assert_eq!(judge_against_plan(0.6, 0.4, 10.0), PlanVerdict::AsPlanned);
        assert_eq!(judge_against_plan(0.6, 0.3, 10.0), PlanVerdict::Below);
        // No evidence.
        assert_eq!(judge_against_plan(0.6, 0.0, 0.0), PlanVerdict::AsPlanned);
    }

    /// Playtest: whole degrees per second were too coarse to see a change (a rating
    /// moving from 48.4 to 48.9 °/s showed as 48 → 49, or not at all).
    #[test]
    fn target_speed_shows_one_decimal() {
        assert_eq!(Knob::TargetSpeed.format(48.46), "48.5 °/s");
        assert_eq!(Knob::TargetSpeed.format(12.0), "12.0 °/s");
        assert_eq!(Knob::Lifetime.format(0.624), "0.62 s");
    }

    fn cfg() -> Resolved {
        Resolved { knob: Knob::Lifetime, easy: 3.0, hard: 0.25, start: 1.2, goal: 0.85, segment_s: 2.0 }
    }

    /// Bug: reversed limits made `value()` panic in `clamp`. Scenario files are now
    /// checked for this, but the rating mustn't crash the trainer either way.
    #[test]
    fn reversed_limits_dont_panic() {
        let r = Rating::new(Resolved { easy: 0.25, hard: 3.0, ..cfg() }, None);
        assert!(r.value().is_finite());
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
        let prior = Prior {
            theta: Knob::Lifetime.h(0.6) + 0.1 * 1.7346,
            slope: 0.1,
            sd: 0.05,
            slope_sd: None,
            days: 0.0,
            fastest: None,
            last_run: None,
        };
        for (mix, lo, hi) in [(Mix::Steady, 0.82, 0.92), (Mix::Balanced, 0.78, 0.9), (Mix::Push, 0.72, 0.86)] {
            let mut r = Rating::new(cfg(), Some(prior));
            let rate = play(&mut r, &p, 600, mix, 9);
            assert!(rate > lo && rate < hi, "{mix:?}: {rate}");
        }
    }

    fn rated(at: f32, sd: f32, fastest: Option<f32>, last_run: Option<(Pick, f32)>) -> Prior {
        Prior {
            theta: Knob::Lifetime.h(at) + 0.12 * 1.7346,
            slope: 0.12,
            sd,
            slope_sd: None,
            days: 0.0,
            fastest,
            last_run,
        }
    }

    #[test]
    fn surprises_move_the_rating_most() {
        let prior = rated(0.6, 0.1, Some(0.5), None);
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
    }

    /// A whole run at one value: `hits` successes out of `n`.
    fn run_at(prior: Prior, value: f32, hits: usize, n: usize) -> Rating {
        let mut r = Rating::new(cfg(), Some(prior));
        for i in 0..n {
            r.update(i as f32, value, if i < hits { 1.0 } else { 0.0 }, false);
        }
        r
    }

    #[test]
    fn breakthroughs_hold_the_goal_for_a_whole_run() {
        let prior = rated(0.6, 0.1, Some(0.5), None);
        // Held 90% at 0.45 s, faster than the 0.5 s best.
        let mut r = run_at(prior, 0.45, 45, 50);
        assert!(r.finish_run(0.45));
        assert_eq!(r.fastest, Some(0.45));
        // Held it, but slower than the best: a good run, not a breakthrough.
        let mut r = run_at(prior, 0.55, 45, 50);
        assert!(!r.finish_run(0.55));
        assert_eq!(r.fastest, Some(0.55));
        // Fast, but below the goal: no breakthrough and no best.
        let mut r = run_at(prior, 0.45, 35, 50);
        assert!(!r.finish_run(0.45));
        assert_eq!(r.fastest, None);
        // Too short a run to count.
        let mut r = run_at(prior, 0.45, 5, 5);
        assert!(!r.finish_run(0.45));
    }

    /// Bug: 9 hits of 10 (90%) counted as holding the goal and announced a breakthrough,
    /// though a run that short clears 85% by luck. Short runs now need a margin.
    #[test]
    fn short_runs_need_a_margin_over_the_goal_to_hold_it() {
        let prior = rated(0.6, 0.1, Some(0.5), None);
        // (hits, n, holds) at the 85% goal.
        let cases = [
            (9, 10, false),
            (10, 10, true),
            (18, 20, false),
            (19, 20, true),
            (26, 29, false),
            (27, 29, true),
            (25, 30, false),
            (26, 30, true),
        ];
        for (hits, n, holds) in cases {
            let mut r = run_at(prior, 0.45, hits, n);
            assert_eq!(r.finish_run(0.45), holds, "{hits} of {n}: breakthrough");
            assert_eq!(r.fastest.is_some(), holds, "{hits} of {n}: best held");
        }
    }

    /// Bug: a short lucky run set `fastest`, which then hid a real breakthrough at the
    /// same value later.
    #[test]
    fn a_short_lucky_run_does_not_hide_a_later_breakthrough() {
        let prior = rated(0.6, 0.1, Some(0.5), None);
        let mut lucky = run_at(prior, 0.45, 9, 10);
        assert!(!lucky.finish_run(0.45));
        assert_eq!(lucky.fastest, None);
        // The next run starts from the best held so far, as the trainer reads it back from
        // its saved runs: the lucky run's value if it had set one, else the old 0.5 s.
        let next = rated(0.6, 0.1, lucky.fastest.or(prior.fastest), None);
        let mut full = run_at(next, 0.45, 45, 50);
        assert!(full.finish_run(0.45), "a full run at the same value is still a breakthrough");
    }

    /// Tracking segments count as three targets each, so 10 of them need no margin.
    #[test]
    fn tracking_runs_hold_the_goal_by_segments() {
        let prior = rated(0.6, 0.1, Some(0.5), None);
        let track = |n: usize| {
            let mut r = Rating::new(cfg(), Some(prior));
            for i in 0..n {
                r.update(i as f32, 0.45, 0.86, true);
            }
            r
        };
        let mut r = track(10);
        assert!(r.finish_run(0.45));
        assert_eq!(r.fastest, Some(0.45));
        let mut r = track(9);
        assert!(!r.finish_run(0.45));
        assert_eq!(r.fastest, None);
    }

    /// The required rate is capped at 100%, so a high goal stays reachable.
    #[test]
    fn a_goal_near_one_stays_reachable() {
        let prior = rated(0.6, 0.1, Some(0.5), None);
        let mut r = Rating::new(Resolved { goal: 0.95, ..cfg() }, Some(prior));
        for i in 0..10 {
            r.update(i as f32, 0.45, 1.0, false);
        }
        assert!(r.finish_run(0.45));
    }

    #[test]
    fn a_full_run_moves_the_rating_toward_what_happened() {
        let prior = rated(0.6, 0.1, None, None);
        let base = Rating::new(cfg(), Some(prior)).value();
        // All hits at 0.6 s: faster rating (shorter lifetime). Half misses: slower.
        assert!(run_at(prior, 0.6, 50, 50).value() < base);
        assert!(run_at(prior, 0.6, 25, 50).value() > base * 1.05);
    }

    #[test]
    fn run_picks_follow_the_mix_with_guard_rails() {
        let count = |last_run, mix| {
            let r = Rating::new(cfg(), Some(rated(0.6, 0.05, None, last_run)));
            let mut rng = Rng::new(11);
            let mut n = [0usize; 3];
            for _ in 0..2000 {
                let (v, chance, kind) = r.pick_run(&mut rng, mix);
                assert!((r.chance_at(v) - chance).abs() < 0.03, "{kind:?} at {v} has the planned chance");
                n[match kind {
                    Pick::Near => 0,
                    Pick::Stretch => 1,
                    _ => 2,
                }] += 1;
            }
            n.map(|c| c as f32 / 2000.0)
        };
        let [near, stretch, easy] = count(Some((Pick::Near, 0.86)), Mix::Balanced);
        assert!((near - 0.65).abs() < 0.04 && (stretch - 0.20).abs() < 0.04 && (easy - 0.15).abs() < 0.04);
        // After a stretch run, or a run well below the goal: no stretch.
        assert_eq!(count(Some((Pick::Stretch, 0.9)), Mix::Push)[1], 0.0);
        assert_eq!(count(Some((Pick::Near, 0.70)), Mix::Push)[1], 0.0);
        // A run just under the goal is fine.
        assert!(count(Some((Pick::Near, 0.80)), Mix::Push)[1] > 0.3);
        // Stretch runs are harder than near runs, easy runs easier.
        let r = Rating::new(cfg(), Some(rated(0.6, 0.05, None, None)));
        let mut rng = Rng::new(3);
        for _ in 0..200 {
            let (v, _, kind) = r.pick_run(&mut rng, Mix::Push);
            match kind {
                Pick::Stretch => assert!(v < 0.6),
                Pick::Easy => assert!(v > 0.6),
                _ => assert!((v / 0.6 - 1.0).abs() < 0.12, "near run at {v}"),
            }
        }
    }

    fn letters(reps: u32, mix: Mix) -> String {
        (1..=reps)
            .map(|rep| match planned_kind(rep, reps, mix).unwrap() {
                Pick::Near => 'N',
                Pick::Stretch => 'S',
                _ => 'E',
            })
            .collect()
    }

    #[test]
    fn planned_blocks_pin_the_examples() {
        assert_eq!(letters(5, Mix::Balanced), "NNSNE");
        assert_eq!(letters(5, Mix::Push), "NSNSE");
        assert_eq!(letters(10, Mix::Balanced).chars().rev().take(2).collect::<String>(), "EE");
        assert_eq!(letters(2, Mix::Steady), "NN");
        // Shares that land on a half round up: Steady gets its easy run at five runs
        // (0.10 x 5) and its first stretch run at ten (0.05 x 10).
        assert_eq!(letters(4, Mix::Steady), "NNNN");
        assert_eq!(letters(5, Mix::Steady), "NNNNE");
        assert_eq!(letters(9, Mix::Steady), "NNNNNNNNE");
        assert_eq!(letters(10, Mix::Steady), "NNNNNSNNNE");
        // The short blocks the built-in workouts use.
        assert_eq!(letters(3, Mix::Balanced), "NSN");
        assert_eq!(letters(4, Mix::Balanced), "NNSE");
        assert_eq!(letters(3, Mix::Push), "NSN");
        assert_eq!(letters(4, Mix::Push), "NNSE");
    }

    /// Planned order: a block never opens on a stretch run or ends on one, stretch
    /// runs are never side by side, and easy runs close the block.
    #[test]
    fn planned_blocks_follow_the_rules_for_every_mix_and_length() {
        for mix in [Mix::Steady, Mix::Balanced, Mix::Push] {
            let (_, ss, es) = mix.shares();
            for reps in 2..=60u32 {
                let kinds: Vec<Pick> = (1..=reps).map(|r| planned_kind(r, reps, mix).unwrap()).collect();
                let n = reps as usize;
                let count = |k| kinds.iter().filter(|&&p| p == k).count();
                assert_eq!(kinds[0], Pick::Near, "{mix:?} {reps}");
                assert_ne!(kinds[n - 1], Pick::Stretch, "{mix:?} {reps}");
                assert!(kinds.windows(2).all(|w| w != [Pick::Stretch, Pick::Stretch]), "{mix:?} {reps}");
                // Easy: the rounded share, never the first run, all at the end.
                let easy = ((es * reps as f32).round() as usize).min(n - 1);
                assert_eq!(count(Pick::Easy), easy, "{mix:?} {reps}");
                assert!(kinds[n - easy..].iter().all(|&p| p == Pick::Easy), "{mix:?} {reps}");
                // Stretch: the rounded share, capped at what fits between the first
                // run and the closing run(s) with a gap between each.
                let room = n.saturating_sub(easy.max(1) + 1);
                let want = (ss * reps as f32).round() as usize;
                assert_eq!(count(Pick::Stretch), want.min(room.div_ceil(2)), "{mix:?} {reps}");
                assert_eq!(count(Pick::Near) + count(Pick::Stretch) + count(Pick::Easy), n);
            }
        }
    }

    #[test]
    fn planned_kind_is_none_outside_a_block() {
        for mix in [Mix::Steady, Mix::Balanced, Mix::Push] {
            assert_eq!(planned_kind(1, 0, mix), None);
            assert_eq!(planned_kind(1, 1, mix), None);
            assert_eq!(planned_kind(0, 5, mix), None);
            assert_eq!(planned_kind(6, 5, mix), None);
            assert!(planned_kind(5, 5, mix).is_some());
        }
    }

    #[test]
    fn planned_runs_play_their_kind_inside_its_band() {
        let r = Rating::new(cfg(), Some(rated(0.6, 0.05, None, Some((Pick::Near, 0.86)))));
        let mut rng = Rng::new(5);
        for (kind, lo, hi) in [(Pick::Near, 0.80, 0.90), (Pick::Stretch, 0.55, 0.72), (Pick::Easy, 0.93, 0.97)] {
            for _ in 0..100 {
                let (v, chance, got) = r.pick_planned(&mut rng, kind);
                assert_eq!(got, kind);
                assert!((lo..=hi).contains(&chance), "{kind:?}: {chance}");
                assert!((r.chance_at(v) - chance).abs() < 0.03);
            }
        }
        // Guard rail: no planned stretch after a stretch run or a run well below the goal.
        for last in [(Pick::Stretch, 0.9), (Pick::Near, 0.70)] {
            let r = Rating::new(cfg(), Some(rated(0.6, 0.05, None, Some(last))));
            let (_, chance, got) = r.pick_planned(&mut rng, Pick::Stretch);
            assert_eq!(got, Pick::Near, "{last:?}");
            assert!((0.80..=0.90).contains(&chance));
        }
    }

    /// The single-run draw is unchanged by the planned order (values recorded from
    /// the code before the change).
    #[test]
    fn unplanned_pick_run_is_unchanged() {
        let r = Rating::new(cfg(), Some(rated(0.6, 0.05, None, None)));
        let mut rng = Rng::new(11);
        let got: Vec<(f32, Pick)> = (0..6).map(|_| r.pick_run(&mut rng, Mix::Push)).map(|(v, _, k)| (v, k)).collect();
        let want = [(0.5887758, Pick::Near), (0.5169579, Pick::Stretch), (0.60640925, Pick::Near)]
            .into_iter()
            .chain([(0.62342125, Pick::Near), (0.63956356, Pick::Near), (0.588328, Pick::Near)])
            .collect::<Vec<_>>();
        assert_eq!(got, want);
    }

    #[test]
    fn picks_round_trip_as_text() {
        for p in [Pick::Near, Pick::Stretch, Pick::Easy, Pick::Calibrate] {
            assert_eq!(Pick::parse(p.as_str()), Some(p));
        }
        assert_eq!(Pick::parse("level"), None);
    }

    #[test]
    fn time_away_widens_the_prior() {
        let p = |days| Prior { theta: 0.0, slope: 0.12, sd: 0.05, slope_sd: None, days, fastest: None, last_run: None };
        let fresh = Rating::new(cfg(), Some(p(0.0))).estimate().sd;
        let later = Rating::new(cfg(), Some(p(30.0))).estimate().sd;
        assert!(later > fresh * 2.0, "{fresh} -> {later}");
    }

    fn prior_of(sd: f32, days: f32) -> Prior {
        Prior { theta: 0.0, slope: 0.12, sd, slope_sd: None, days, fastest: None, last_run: None }
    }

    #[test]
    fn a_long_break_with_an_unsure_rating_asks_for_calibration() {
        // Both lines, both sides.
        assert!(!prior_of(0.3, 2.9).needs_calibration(), "under 3 days is never a break");
        assert!(prior_of(0.12, 3.0).needs_calibration(), "3 days with a wide sd is a break");
        assert!(!prior_of(0.05, 5.0).needs_calibration(), "a settled rating survives 5 days");
        assert!(prior_of(0.05, 6.0).needs_calibration(), "but not 6");
        // A fresh, unsure rating (short first run) must not loop back into calibration.
        assert!(!prior_of(0.3, 0.0).needs_calibration());
    }

    #[test]
    fn a_rating_after_a_break_keeps_the_old_best() {
        let p = Prior { fastest: Some(0.7), ..prior_of(0.05, 10.0) };
        assert_eq!(Rating::new(cfg(), Some(p)).best_before, Some(0.7));
    }

    #[test]
    fn rating_new_keeps_its_estimate_for_a_prior_with_days() {
        let p =
            Prior { theta: 0.4, slope: 0.15, sd: 0.07, slope_sd: Some(0.25), days: 4.0, fastest: None, last_run: None };
        let e = Rating::new(cfg(), Some(p)).estimate();
        assert_eq!((e.h_goal, e.sd), (0.13980983, 0.10630146));
    }

    /// A player of fixed skill plays `runs` runs in a row, each started from the
    /// previous run's rating the way the trainer does. `change` is (run number,
    /// factor): from that run on the true goal-rate value is multiplied by the factor.
    /// Returns the rating's value after each run.
    fn run_chain(seed: u64, runs: usize, change: Option<(usize, f32)>) -> Vec<f32> {
        let mut rng = Rng::new(seed);
        let mut values = Vec::new();
        let mut prior: Option<Prior> = None;
        for run in 0..runs {
            let true85 = match change {
                Some((at, f)) if run >= at => 0.6 * f,
                _ => 0.6,
            };
            let p = player(true85, 0.12);
            let mut r = Rating::new(cfg(), prior);
            // The first run is a calibration run, later ones play at one value.
            let (value, kind) = if prior.is_some() {
                let (v, _, kind) = r.pick_run(&mut rng, Mix::Balanced);
                (Some(v), kind)
            } else {
                (None, Pick::Calibrate)
            };
            for i in 0..60 {
                let v = value.unwrap_or_else(|| r.pick(&mut rng, Mix::Balanced).0);
                let ok = rng.next_f32() < p(v);
                r.update(i as f32, v, if ok { 1.0 } else { 0.0 }, false);
            }
            if let Some(v) = value {
                r.finish_run(v);
            }
            let e = r.estimate();
            values.push(r.value());
            prior = Some(Prior {
                theta: e.theta,
                slope: e.slope,
                sd: e.sd,
                slope_sd: Some(e.slope_sd),
                days: 1.0,
                fastest: None,
                last_run: Some((kind, r.success_rate().unwrap_or(0.0))),
            });
        }
        values
    }

    /// Bug: between runs the rating forgot how sure it was about the slope (every run
    /// restarted with the same wide guess) and applied the carried uncertainty to the
    /// wrong point of the curve, so a player whose skill never changed saw the rating
    /// move about 4.5% from run to run in this test. It is about 2.2% with the fix.
    #[test]
    fn rating_of_a_steady_player_barely_moves_between_runs() {
        let (mut total, mut n) = (0.0f32, 0u32);
        for seed in 1..=24 {
            let v = run_chain(seed, 30, None);
            for i in 5..v.len() {
                total += (v[i] / v[i - 1]).ln().abs();
                n += 1;
            }
        }
        let movement = total / n as f32;
        assert!(movement < 0.034, "average run-to-run movement {:.2}%", movement * 100.0);
    }

    /// The steadier rating must still follow a real change: after the true goal-rate
    /// value drops 10% (the player improves), most of that step shows within five runs.
    #[test]
    fn rating_still_follows_a_real_improvement() {
        let step = 0.9f32;
        let mut share = [0.0f32; 5];
        let seeds = 24;
        for seed in 1..=seeds {
            let v = run_chain(seed, 25, Some((15, step)));
            let before = v[14];
            for (k, s) in share.iter_mut().enumerate() {
                *s += (v[15 + k] / before).ln() / step.ln();
            }
        }
        let share = share.map(|s| s / seeds as f32);
        eprintln!("step shown after runs 1..5: {share:?}");
        assert!(share[4] > 0.7, "share of a 10% step shown after five runs: {share:?}");
    }

    #[test]
    fn tracking_speed_is_harder_when_higher() {
        let k = Knob::TargetSpeed;
        assert!(k.h(60.0) > k.h(30.0));
        assert!((k.value(k.h(42.0)) - 42.0).abs() < 1e-3);
        assert!(Knob::Lifetime.h(0.5) > Knob::Lifetime.h(1.0));
        let a = Adaptive { knob: Knob::TargetSpeed, easy: 90.0, hard: 10.0, ..Default::default() };
        let mut s = Scenario::default();
        s.targets.movement.kind = MovementKind::Strafe;
        assert!(!a.problems(&a.resolve(&s)).is_empty(), "easy must be easier than hard");
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
