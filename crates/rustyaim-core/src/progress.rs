//! Progress over time: per-scenario history, personal bests, trends, plateaus,
//! cold starts, fatigue within a session, and sensitivity changes.
//!
//! Restarted runs (see [`crate::training`]) stay in the history but never count
//! as personal bests and are left out of averages and trends.

use std::collections::{BTreeMap, HashMap};

use chrono::{NaiveDate, NaiveDateTime};

use crate::db::{Db, DbError};
use crate::training::{self, TimeSettings};

const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f";

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressSettings {
    /// Number of recent runs compared against the runs before them for the trend.
    pub trend_window: usize,
    /// Runs without a new best before a plateau can be flagged.
    pub plateau_runs: usize,
    /// A trend within ± this percentage counts as flat.
    pub plateau_flat_pct: f64,
    /// Minutes per bucket in the fatigue curve.
    pub fatigue_bucket_min: i64,
}

impl Default for ProgressSettings {
    fn default() -> Self {
        Self { trend_window: 5, plateau_runs: 10, plateau_flat_pct: 3.0, fatigue_bucket_min: 10 }
    }
}

/// The sensitivity-related settings a run was played with.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SensSettings {
    pub sens_scale: Option<String>,
    pub horiz_sens: Option<f64>,
    pub dpi: Option<i64>,
    pub fov: Option<f64>,
}

impl SensSettings {
    /// Mouse distance for a full 360° turn, when the sensitivity scale is known.
    pub fn cm_per_360(&self) -> Option<f64> {
        cm_per_360(self.sens_scale.as_deref()?, self.horiz_sens?, self.dpi?)
    }

    fn same_as(&self, other: &Self) -> bool {
        let close = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-6,
            (a, b) => a.is_none() && b.is_none(),
        };
        self.sens_scale == other.sens_scale
            && self.dpi == other.dpi
            && close(self.horiz_sens, other.horiz_sens)
            && close(self.fov, other.fov)
    }
}

/// Degrees turned per mouse count at sensitivity 1, by KovaaK's `Sens Scale` name.
/// Only "Overwatch" has been checked against real files; others are standard game
/// values and return `None` if KovaaK's names them differently.
fn yaw_degrees(scale: &str) -> Option<f64> {
    match scale {
        "Overwatch" => Some(0.0066),
        "Valorant" => Some(0.07),
        "CS:GO" | "CS2" | "Counter-Strike 2" | "Source" | "Quake/Source" | "Apex Legends" | "Apex" => Some(0.022),
        _ => None,
    }
}

pub fn cm_per_360(scale: &str, sens: f64, dpi: i64) -> Option<f64> {
    if scale.eq_ignore_ascii_case("cm/360") {
        return Some(sens);
    }
    let counts = 360.0 / (yaw_degrees(scale)? * sens);
    (dpi > 0 && sens > 0.0).then(|| counts / dpi as f64 * 2.54)
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioRun {
    pub run_id: i64,
    pub scenario: String,
    pub started_at: NaiveDateTime,
    pub finished_at: NaiveDateTime,
    pub day: NaiveDate,
    pub score: Option<f64>,
    pub accuracy: Option<f64>,
    pub avg_ttk_s: Option<f64>,
    pub restarted: bool,
    /// Set a new personal best for this scenario when it was played.
    pub is_pb: bool,
    /// Minutes from the start of its session to the start of this run.
    pub minutes_into_session: f64,
    /// Index into [`Progress::sens_periods`].
    pub sens_period: usize,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SensPeriod {
    pub from: NaiveDateTime,
    pub to: NaiveDateTime,
    pub settings: SensSettings,
    pub runs: usize,
    /// [`SensSettings::cm_per_360`], stored so it's available after serializing.
    pub cm_per_360: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioStats {
    pub scenario: String,
    /// All runs, including restarted ones.
    pub runs: usize,
    /// Runs that count (not restarted, with a score).
    pub counted_runs: usize,
    pub first_played: NaiveDateTime,
    pub last_played: NaiveDateTime,
    pub first_score: Option<f64>,
    pub best_score: Option<f64>,
    pub best_at: Option<NaiveDateTime>,
    pub last_score: Option<f64>,
    pub average: Option<f64>,
    /// First counted run → best, in percent. `None` when the first score isn't positive.
    pub improvement_pct: Option<f64>,
    /// Average of the last `trend_window` runs vs. the `trend_window` before them, in percent.
    pub trend_pct: Option<f64>,
    /// Coefficient of variation (stdev / mean) of recent runs, in percent. Lower is steadier.
    pub variation_pct: Option<f64>,
    pub recent_accuracy: Option<f64>,
    pub runs_since_pb: usize,
    pub plateau: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ColdStart {
    pub scenario: String,
    pub day: NaiveDate,
    pub first_score: f64,
    /// Best score from earlier days.
    pub previous_best: f64,
    pub vs_previous_best_pct: f64,
    /// First run vs. the average of that scenario's later runs the same day.
    pub vs_rest_of_day_pct: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct FatigueBucket {
    pub from_min: i64,
    pub to_min: i64,
    pub runs: usize,
    /// Average score as a percentage of each scenario's own average (100 = typical).
    pub relative_score_pct: f64,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// Every run, oldest first.
    pub runs: Vec<ScenarioRun>,
    pub sens_periods: Vec<SensPeriod>,
}

impl Progress {
    pub fn history(&self, scenario: &str) -> Vec<&ScenarioRun> {
        self.runs.iter().filter(|r| r.scenario == scenario).collect()
    }

    pub fn scenarios(&self) -> Vec<String> {
        let mut names: Vec<String> = self.runs.iter().map(|r| r.scenario.clone()).collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        names
    }

    pub fn stats(&self, settings: &ProgressSettings) -> Vec<ScenarioStats> {
        let mut by: BTreeMap<&str, Vec<&ScenarioRun>> = BTreeMap::new();
        for r in &self.runs {
            by.entry(r.scenario.as_str()).or_default().push(r);
        }
        by.into_values().map(|h| scenario_stats(&h, settings)).collect()
    }

    pub fn stats_for(&self, scenario: &str, settings: &ProgressSettings) -> Option<ScenarioStats> {
        let h = self.history(scenario);
        (!h.is_empty()).then(|| scenario_stats(&h, settings))
    }

    /// The first run of each day on each scenario, compared with what you'd already done.
    /// Only days after the scenario's first day are included.
    pub fn cold_starts(&self) -> Vec<ColdStart> {
        let mut by: BTreeMap<&str, Vec<&ScenarioRun>> = BTreeMap::new();
        for r in self.runs.iter().filter(|r| counts(r)) {
            by.entry(r.scenario.as_str()).or_default().push(r);
        }
        let mut out = Vec::new();
        for (scenario, history) in by {
            let mut best_before: Option<f64> = None;
            let mut i = 0;
            while i < history.len() {
                let day = history[i].day;
                let same_day: Vec<f64> =
                    history[i..].iter().take_while(|r| r.day == day).filter_map(|r| r.score).collect();
                let first = same_day[0];
                if let Some(prev) = best_before.filter(|p| *p > 0.0) {
                    let rest = &same_day[1..];
                    let rest_avg = (!rest.is_empty()).then(|| mean(rest));
                    out.push(ColdStart {
                        scenario: scenario.to_string(),
                        day,
                        first_score: first,
                        previous_best: prev,
                        vs_previous_best_pct: pct_change(prev, first),
                        vs_rest_of_day_pct: rest_avg.filter(|a| *a > 0.0).map(|a| pct_change(a, first)),
                    });
                }
                let day_best = same_day.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                best_before = Some(best_before.map_or(day_best, |b| b.max(day_best)));
                i += same_day.len();
            }
        }
        out
    }

    /// Score relative to each scenario's own average, grouped by minutes into the session.
    /// Only scenarios with 3+ counted runs and a positive average are used.
    pub fn fatigue(&self, settings: &ProgressSettings) -> Vec<FatigueBucket> {
        let mut scores: HashMap<&str, Vec<f64>> = HashMap::new();
        for r in self.runs.iter().filter(|r| counts(r)) {
            scores.entry(r.scenario.as_str()).or_default().extend(r.score);
        }
        let averages: HashMap<&str, f64> = scores
            .into_iter()
            .filter(|(_, v)| v.len() >= 3)
            .map(|(k, v)| (k, mean(&v)))
            .filter(|(_, m)| *m > 0.0)
            .collect();

        let width = settings.fatigue_bucket_min.max(1);
        let mut buckets: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
        for r in self.runs.iter().filter(|r| counts(r)) {
            let (Some(avg), Some(score)) = (averages.get(r.scenario.as_str()), r.score) else { continue };
            let bucket = (r.minutes_into_session as i64) / width;
            buckets.entry(bucket).or_default().push(score / avg * 100.0);
        }
        buckets
            .into_iter()
            .map(|(b, v)| FatigueBucket {
                from_min: b * width,
                to_min: (b + 1) * width,
                runs: v.len(),
                relative_score_pct: mean(&v),
            })
            .collect()
    }
}

fn counts(r: &ScenarioRun) -> bool {
    !r.restarted && r.score.is_some()
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn pct_change(from: f64, to: f64) -> f64 {
    (to / from - 1.0) * 100.0
}

fn scenario_stats(history: &[&ScenarioRun], settings: &ProgressSettings) -> ScenarioStats {
    let counted: Vec<&ScenarioRun> = history.iter().copied().filter(|r| counts(r)).collect();
    let scores: Vec<f64> = counted.iter().filter_map(|r| r.score).collect();
    let best = counted.iter().filter_map(|r| r.score.map(|s| (s, r.finished_at))).max_by(|a, b| a.0.total_cmp(&b.0));
    let first_score = scores.first().copied();
    let last_pb = counted.iter().rposition(|r| r.is_pb);
    let runs_since_pb = last_pb.map_or(counted.len(), |i| counted.len() - 1 - i);

    let w = settings.trend_window.max(1);
    let trend_pct = (scores.len() >= 2 * w)
        .then(|| {
            let recent = mean(&scores[scores.len() - w..]);
            let before = mean(&scores[scores.len() - 2 * w..scores.len() - w]);
            (before > 0.0).then(|| pct_change(before, recent))
        })
        .flatten();

    let recent = &scores[scores.len().saturating_sub(2 * w)..];
    let variation_pct = (recent.len() >= 3)
        .then(|| {
            let m = mean(recent);
            let var = recent.iter().map(|s| (s - m).powi(2)).sum::<f64>() / (recent.len() - 1) as f64;
            (m > 0.0).then(|| var.sqrt() / m * 100.0)
        })
        .flatten();

    let recent_acc: Vec<f64> = counted.iter().rev().take(w).filter_map(|r| r.accuracy).collect();

    ScenarioStats {
        scenario: history[0].scenario.clone(),
        runs: history.len(),
        counted_runs: counted.len(),
        first_played: history[0].finished_at,
        last_played: history[history.len() - 1].finished_at,
        first_score,
        best_score: best.map(|b| b.0),
        best_at: best.map(|b| b.1),
        last_score: scores.last().copied(),
        average: (!scores.is_empty()).then(|| mean(&scores)),
        improvement_pct: match (first_score, best) {
            (Some(f), Some((b, _))) if f > 0.0 => Some(pct_change(f, b)),
            _ => None,
        },
        trend_pct,
        variation_pct,
        recent_accuracy: (!recent_acc.is_empty()).then(|| mean(&recent_acc)),
        runs_since_pb,
        plateau: runs_since_pb >= settings.plateau_runs
            && trend_pct.is_some_and(|t| t.abs() <= settings.plateau_flat_pct),
    }
}

/// Loads every run with scores, restart flags, session position and settings.
pub fn load(db: &Db, time: &TimeSettings) -> Result<Progress, DbError> {
    let timings = training::load_runs(db, time)?;
    let sessions = training::group_sessions(&timings, time);

    // Sessions are built from the sorted runs in order, so walk them in step.
    let mut session_start: HashMap<i64, NaiveDateTime> = HashMap::new();
    let mut iter = timings.iter();
    for s in &sessions {
        for r in iter.by_ref().take(s.runs) {
            session_start.insert(r.run_id, s.start);
        }
    }
    let timing: HashMap<i64, &training::RunTiming> = timings.iter().map(|t| (t.run_id, t)).collect();

    let mut stmt = db.conn().prepare(
        "SELECT id, scenario, finished_at, score, accuracy, avg_ttk_s, sens_scale, horiz_sens, dpi, fov
         FROM runs WHERE mode != 'RustyAim Adaptive' ORDER BY finished_at, id",
    )?;
    let rows = stmt.query_map([], |r| {
        let finished: String = r.get(2)?;
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            finished,
            r.get::<_, Option<f64>>(3)?,
            r.get::<_, Option<f64>>(4)?,
            r.get::<_, Option<f64>>(5)?,
            SensSettings { sens_scale: r.get(6)?, horiz_sens: r.get(7)?, dpi: r.get(8)?, fov: r.get(9)? },
        ))
    })?;

    let mut runs: Vec<ScenarioRun> = Vec::new();
    let mut periods: Vec<SensPeriod> = Vec::new();
    let mut best: HashMap<String, f64> = HashMap::new();

    for row in rows {
        let (run_id, scenario, finished, score, accuracy, avg_ttk_s, sens) = row?;
        let Ok(finished_at) = NaiveDateTime::parse_from_str(&finished, TIME_FORMAT) else { continue };
        let t = timing.get(&run_id);
        let started_at = t.map_or(finished_at, |t| t.started_at);
        let restarted = t.is_some_and(|t| t.restarted);

        match periods.last_mut() {
            Some(p) if p.settings.same_as(&sens) => {
                p.to = finished_at;
                p.runs += 1;
            }
            _ => periods.push(SensPeriod {
                from: started_at,
                to: finished_at,
                cm_per_360: sens.cm_per_360(),
                settings: sens,
                runs: 1,
            }),
        }

        let mut is_pb = false;
        if let (Some(s), false) = (score, restarted) {
            let prev = best.get(&scenario).copied();
            is_pb = prev.is_none_or(|p| s > p);
            if is_pb {
                best.insert(scenario.clone(), s);
            }
        }

        let minutes_into_session =
            session_start.get(&run_id).map_or(0.0, |s| (started_at - *s).num_seconds().max(0) as f64 / 60.0);

        runs.push(ScenarioRun {
            run_id,
            scenario,
            started_at,
            finished_at,
            day: time.training_day(started_at),
            score,
            accuracy,
            avg_ttk_s,
            restarted,
            is_pb,
            minutes_into_session,
            sens_period: periods.len() - 1,
        });
    }
    Ok(Progress { runs, sens_periods: periods })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn t(day: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    fn run(day: u32, h: u32, m: u32, score: f64) -> ScenarioRun {
        let start = t(day, h, m);
        ScenarioRun {
            run_id: 0,
            scenario: "s".into(),
            started_at: start,
            finished_at: start + Duration::seconds(60),
            day: start.date(),
            score: Some(score),
            accuracy: Some(0.8),
            avg_ttk_s: None,
            restarted: false,
            is_pb: false,
            minutes_into_session: 0.0,
            sens_period: 0,
        }
    }

    fn with_pbs(mut runs: Vec<ScenarioRun>) -> Progress {
        let mut best = f64::NEG_INFINITY;
        for r in &mut runs {
            let s = r.score.unwrap();
            r.is_pb = s > best;
            best = best.max(s);
        }
        Progress { runs, sens_periods: vec![] }
    }

    #[test]
    fn cm_per_360_overwatch() {
        // 5.3 Overwatch at 400 DPI, checked against the mockup's 65.4 cm.
        assert!((cm_per_360("Overwatch", 5.3, 400).unwrap() - 65.36).abs() < 0.01);
        assert!((cm_per_360("Overwatch", 5.0, 400).unwrap() - 69.27).abs() < 0.01);
        assert_eq!(cm_per_360("Some Game", 1.0, 800), None);
        assert_eq!(cm_per_360("cm/360", 34.0, 800), Some(34.0));
    }

    #[test]
    fn stats_improvement_and_pb_count() {
        let p = with_pbs(vec![
            run(2, 21, 59, 3150.0),
            run(2, 22, 0, 5500.0),
            run(2, 22, 2, 8000.0),
            run(2, 22, 3, 8350.0),
            run(3, 6, 28, 7250.0),
            run(3, 6, 29, 9200.0),
        ]);
        let s = p.stats_for("s", &ProgressSettings::default()).unwrap();
        assert_eq!(s.best_score, Some(9200.0));
        assert_eq!(s.runs_since_pb, 0);
        assert!((s.improvement_pct.unwrap() - 192.06).abs() < 0.01);
        assert!((s.average.unwrap() - 6908.33).abs() < 0.01);
        assert_eq!(s.trend_pct, None); // needs 10 runs with the default window of 5
        assert!(!s.plateau);
    }

    #[test]
    fn detects_plateau() {
        let mut runs = vec![run(1, 8, 0, 1000.0)];
        for i in 0..12 {
            runs.push(run(2 + i / 6, 8, i % 6, 950.0 + f64::from(i % 3) * 10.0));
        }
        let s = with_pbs(runs).stats_for("s", &ProgressSettings::default()).unwrap();
        assert_eq!(s.runs_since_pb, 12);
        assert!(s.trend_pct.unwrap().abs() < 3.0);
        assert!(s.plateau);
    }

    #[test]
    fn cold_start_compares_with_earlier_days() {
        let p =
            with_pbs(vec![run(2, 22, 0, 8000.0), run(2, 22, 3, 8350.0), run(3, 6, 28, 7250.0), run(3, 6, 29, 9200.0)]);
        let cs = p.cold_starts();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].previous_best, 8350.0);
        assert!((cs[0].vs_previous_best_pct - -13.17).abs() < 0.01);
        assert!((cs[0].vs_rest_of_day_pct.unwrap() - -21.20).abs() < 0.01);
    }

    #[test]
    fn restarted_runs_dont_count() {
        let mut runs = vec![run(2, 8, 0, 100.0), run(2, 8, 2, 500.0), run(2, 8, 4, 120.0)];
        runs[1].restarted = true;
        let p = Progress { runs, sens_periods: vec![] };
        let s = p.stats_for("s", &ProgressSettings::default()).unwrap();
        assert_eq!(s.runs, 3);
        assert_eq!(s.counted_runs, 2);
        assert_eq!(s.best_score, Some(120.0));
    }

    #[test]
    fn fatigue_buckets_by_minutes_into_session() {
        let mut runs = vec![run(2, 8, 0, 100.0), run(2, 8, 5, 100.0), run(2, 8, 20, 50.0), run(2, 8, 25, 50.0)];
        for (r, m) in runs.iter_mut().zip([0.0, 5.0, 20.0, 25.0]) {
            r.minutes_into_session = m;
        }
        let f = Progress { runs, sens_periods: vec![] }.fatigue(&ProgressSettings::default());
        assert_eq!(f.len(), 2);
        assert_eq!((f[0].from_min, f[0].runs), (0, 2));
        assert!((f[0].relative_score_pct - 133.33).abs() < 0.01);
        assert_eq!(f[1].from_min, 20);
        assert!((f[1].relative_score_pct - 66.67).abs() < 0.01);
    }
}
