//! Analytics for runs played in AimAdapt's trainer, built from the per-target,
//! per-shot and tracking records in [`crate::telemetry`]. Each section answers a
//! question that helps you decide what to practice:
//!
//! - **Rating:** the speed you hit 85% of the time (in seconds or °/s), run by
//!   run, and the speed–accuracy curve behind it.
//! - **Speed vs. distance:** how long flicks take for their size and distance (a
//!   Fitts' law fit). Throughput in bits per second is one number for flick speed
//!   that doesn't depend on how far or small the targets were.
//! - **Directions:** success and time by flick direction, to find weak sides.
//! - **Shot placement:** where shots land relative to the target, turned so the
//!   flick goes right: overshooting, undershooting, or a vertical bias.
//! - **Reaction:** time before the crosshair starts moving.
//! - **Tracking:** time on target by target speed, how quickly you recover after
//!   the target changes direction, and whether you trail or lead it.
//! - **Within a run:** success by 10-second segment, for warm-up and fatigue.
//!
//! "Recent" means the last [`RECENT_RUNS`] runs of the scenario; "before" is the same
//! number of runs before those, for comparison.

use crate::knob::Knob;
use rusqlite::OptionalExtension;
use std::collections::HashMap;

use crate::db::{Db, DbError};
use crate::plan_check::{self, PlanCheck};
use crate::telemetry::parse_timeline;

pub const RECENT_RUNS: usize = 10;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioSummary {
    pub scenario: String,
    pub category: String,
    pub runs: usize,
    pub adaptive_runs: usize,
    pub last_played: String,
    /// Adaptive mode: the knob, the latest rating and the best one, and the
    /// hardest value the goal rate was held at for a whole run.
    pub knob: Option<String>,
    pub latest_rating: Option<f64>,
    pub best_rating: Option<f64>,
    pub fastest: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct RunPoint {
    pub run_id: i64,
    pub finished_at: String,
    pub adaptive: bool,
    pub knob: Option<String>,
    /// The rating and its ±1 SD range, in the knob's units.
    pub rating: Option<f64>,
    pub rating_low: Option<f64>,
    pub rating_high: Option<f64>,
    pub fastest: Option<f64>,
    pub success_rate: Option<f64>,
    pub score: Option<f64>,
    pub accuracy: Option<f64>,
    pub throughput: Option<f64>,
    pub reaction_ms: Option<f64>,
    pub on_target: Option<f64>,
    pub workout: Option<String>,
    /// The kind of an adaptive run, as stored: `calibrate`, `near`, `stretch` or `easy`.
    /// `None` for standard runs and for runs saved before kinds were planned.
    pub pick: Option<String>,
    /// The run held the goal at a harder value than any earlier run of the same
    /// version of the scenario. Worked out from the history, see [`breakthroughs`].
    pub breakthrough: bool,
    /// The run recorded replay frames (runs from before replays didn't).
    pub replay: bool,
    /// The run is of the scenario's current version (same settings fingerprint as its
    /// latest rated run). Ratings from other versions are on another scale, so
    /// charts and bests should use only runs where this is true.
    pub current: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct FlickPoint {
    pub distance_deg: f64,
    /// Index of difficulty, log2(distance / width + 1).
    pub id_bits: f64,
    pub time_ms: f64,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Fitts {
    /// Time = intercept + slope × ID.
    pub intercept_ms: f64,
    pub slope_ms_per_bit: f64,
    pub r2: f64,
    /// Mean of ID / time: bits per second.
    pub throughput: f64,
    pub throughput_before: Option<f64>,
    pub points: Vec<FlickPoint>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct DirectionStat {
    /// 0 = right, 45 = up-right, … (degrees, counter-clockwise).
    pub direction_deg: f64,
    pub label: &'static str,
    pub targets: usize,
    pub success: f64,
    pub median_ms: Option<f64>,
    pub overshoot_deg: f64,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct DistanceStat {
    pub from_deg: f64,
    pub to_deg: f64,
    pub targets: usize,
    pub success: f64,
    pub median_ms: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ShotMap {
    /// (along, across) in target radii, flick direction = +along. Hits and misses.
    pub points: Vec<(f64, f64, bool)>,
    /// Average along/across of misses. Positive along = overshooting.
    pub miss_along: Option<f64>,
    pub miss_across: Option<f64>,
    /// Share of misses that went past the target (vs. stopped short).
    pub overshoot_share: Option<f64>,
    /// Average vertical offset of all shots in degrees (positive = high).
    pub vertical_deg: Option<f64>,
    pub horizontal_deg: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Reaction {
    pub median_ms: f64,
    pub p25_ms: f64,
    pub p75_ms: f64,
    pub median_before_ms: Option<f64>,
    /// Counts in 50 ms bins from 0 to 600 ms (the last bin includes slower).
    pub histogram: Vec<usize>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Bucket {
    pub label: String,
    pub samples: usize,
    pub on_target: f64,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Tracking {
    pub on_target: f64,
    pub on_target_before: Option<f64>,
    pub median_error_deg: f64,
    /// Average offset along the target's motion: negative = trailing behind.
    pub lead_deg: f64,
    pub by_speed: Vec<Bucket>,
    /// On-target share by time since the target changed direction.
    pub after_change: Vec<Bucket>,
    /// Seconds after a direction change until on-target share is back to normal.
    pub recovery_s: Option<f64>,
    /// Unbroken stretches on target: half of all time on target came in stretches
    /// at least this long (seconds). Share of time on target treats ten short slips
    /// like one long one; this tells steady tracking from jittery tracking.
    pub stretch_s: Option<f64>,
    pub stretch_before_s: Option<f64>,
    /// Median distance from the target's center while on target (degrees): how
    /// centered you stay, which the share of time on target can't see.
    pub centered_deg: Option<f64>,
    pub centered_before_deg: Option<f64>,
}

/// Tracking samples are taken this often (`aimadapt_sim::sim::SAMPLE_HZ`).
pub const SAMPLE_S: f64 = 1.0 / 30.0;

/// The stretch length that half of all on-target time comes from, over these
/// runs (stretches end at a run's end).
fn unbroken_stretch(db: &Db, ids: &[i64]) -> Result<Option<f64>, DbError> {
    if ids.is_empty() {
        return Ok(None);
    }
    let sql =
        format!("SELECT run_id, on_target FROM engine_samples WHERE run_id IN ({}) ORDER BY run_id, t", in_list(ids));
    let mut stmt = db.conn().prepare(&sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?)))?;
    let (mut stretches, mut cur, mut run) = (Vec::new(), 0usize, None);
    for row in rows {
        let (id, on) = row?;
        if run != Some(id) || !on {
            if cur > 0 {
                stretches.push(cur);
            }
            cur = 0;
            run = Some(id);
        }
        if on {
            cur += 1;
        }
    }
    if cur > 0 {
        stretches.push(cur);
    }
    let total: usize = stretches.iter().sum();
    if total < 60 {
        return Ok(None);
    }
    stretches.sort_unstable_by(|a, b| b.cmp(a));
    let mut acc = 0;
    for len in stretches {
        acc += len;
        if acc * 2 >= total {
            return Ok(Some(len as f64 * SAMPLE_S));
        }
    }
    Ok(None)
}

/// Switching, split into its two parts: getting to the next target after a kill,
/// and staying on a target until it dies. Kills per minute is the overall pace.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Switching {
    pub kills_per_min: Option<f64>,
    pub kills_per_min_before: Option<f64>,
    /// Median time from the previous kill to first reaching the next target.
    pub acquire_ms: Option<f64>,
    pub acquire_before_ms: Option<f64>,
    /// Share of time on target while a target was engaged (damaged, not yet healed).
    pub engaged_on_target: Option<f64>,
    pub engaged_on_target_before: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub from_s: f64,
    pub to_s: f64,
    pub success: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Insights {
    pub scenario: String,
    pub category: String,
    pub runs: Vec<RunPoint>,
    /// Adaptive knob of the latest adaptive run, and its rating over time.
    pub knob: Option<String>,
    pub latest_timeline: Vec<(f64, f64)>,
    /// Speed–accuracy curve from the latest adaptive run.
    pub curve: Option<Curve>,
    pub recent_runs: usize,
    pub targets: usize,
    pub success: Option<f64>,
    pub fitts: Option<Fitts>,
    pub directions: Vec<DirectionStat>,
    pub distances: Vec<DistanceStat>,
    pub shots: Option<ShotMap>,
    pub reaction: Option<Reaction>,
    /// Average times the crosshair moved onto a target before killing it (1 = clean).
    pub entries_per_kill: Option<f64>,
    pub tracking: Option<Tracking>,
    pub switching: Option<Switching>,
    pub segments: Vec<Segment>,
    /// Planned against actual success by kind of run, for this scenario.
    pub plan: PlanCheck,
}

/// Chance of success at each value of the knob, from the latest rating, plus
/// what actually happened at each value in recent runs.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub knob: String,
    /// (value, chance) from the model.
    pub model: Vec<(f64, f64)>,
    /// (value, share hit, targets) in bins, clicking only.
    pub observed: Vec<(f64, f64, usize)>,
    /// Values at 95%, 85% and 50% chance: how quickly you fall off.
    pub at95: f64,
    pub at85: f64,
    pub at50: f64,
}

#[derive(Debug, Clone)]
struct T {
    run_id: i64,
    start_at: f64,
    ended_at: f64,
    killed: bool,
    open: bool,
    distance: f64,
    direction: f64,
    radius: f64,
    reaction: Option<f64>,
    reach: Option<f64>,
    overshoot: f64,
    entries: i64,
    value: Option<f64>,
}

fn median(v: &mut [f64]) -> Option<f64> {
    quantile(v, 0.5)
}

fn quantile(v: &mut [f64], q: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let pos = q * (v.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    Some(v[lo] + (v[hi] - v[lo]) * (pos - lo as f64))
}

fn mean(v: impl IntoIterator<Item = f64>) -> Option<f64> {
    let (s, n) = v.into_iter().fold((0.0, 0usize), |(s, n), x| (s + x, n + 1));
    (n > 0).then(|| s / n as f64)
}

fn id_bits(t: &T) -> f64 {
    (t.distance / (2.0 * t.radius).max(1e-3) + 1.0).log2()
}

/// Every scenario played in the trainer, most recent first.
pub fn overview(db: &Db) -> Result<Vec<ScenarioSummary>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT e.base_scenario, e.category, COUNT(*), SUM(e.adaptive), MAX(r.finished_at)
         FROM engine_runs e JOIN runs r ON r.id = e.run_id
         GROUP BY e.base_scenario ORDER BY MAX(r.finished_at) DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ScenarioSummary {
            scenario: r.get(0)?,
            category: r.get(1)?,
            runs: r.get::<_, i64>(2)? as usize,
            adaptive_runs: r.get::<_, i64>(3)? as usize,
            last_played: r.get(4)?,
            knob: None,
            latest_rating: None,
            best_rating: None,
            fastest: None,
        })
    })?;
    let mut out: Vec<ScenarioSummary> = rows.collect::<Result<_, _>>()?;
    for s in &mut out {
        if let Some(last) = db.last_rating(&s.scenario)? {
            // Only the current version's runs: other versions are on another scale.
            let hash = current_hash(db, &s.scenario)?.flatten();
            let agg = if Knob::from_id(&last.knob).higher_is_harder() { "MAX" } else { "MIN" };
            let version = |col: &str, extra: &str| -> Result<Option<f64>, DbError> {
                Ok(db.conn().query_row(
                    &format!(
                        "SELECT {agg}(e.{col}) FROM engine_runs e JOIN runs r ON r.id = e.run_id
                         WHERE e.base_scenario = ?1 AND e.knob = ?2 AND r.scenario_hash IS ?3 {extra}"
                    ),
                    rusqlite::params![s.scenario, last.knob, hash],
                    |r| r.get(0),
                )?)
            };
            s.best_rating = version("rating", "")?;
            s.fastest = version("fastest", "AND e.run_value IS NOT NULL")?;
            s.latest_rating = Some(last.rating);
            s.knob = Some(last.knob);
        }
    }
    Ok(out)
}

/// The scenario's current version: the settings fingerprint (`runs.scenario_hash`)
/// of its latest rated adaptive run. The outer `Option` is whether there is one; a
/// run without a fingerprint is a version of its own (`Some(None)`).
fn current_hash(db: &Db, base_scenario: &str) -> Result<Option<Option<String>>, DbError> {
    Ok(db
        .conn()
        .query_row(
            "SELECT r.scenario_hash FROM engine_runs e JOIN runs r ON r.id = e.run_id
             WHERE e.base_scenario = ?1 AND e.adaptive = 1 AND e.rating IS NOT NULL
             ORDER BY r.finished_at DESC, r.id DESC LIMIT 1",
            [base_scenario],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?)
}

pub fn scenario(db: &Db, base_scenario: &str) -> Result<Option<Insights>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT r.id, r.finished_at, e.adaptive, e.rating, e.success_rate, r.score, r.accuracy, e.workout,
                e.category, e.level_timeline, e.knob, e.rating_low, e.rating_high,
                CASE WHEN e.run_value IS NOT NULL THEN e.fastest END, e.theta, e.slope, e.frames IS NOT NULL, r.scenario_hash,
                e.run_pick, e.run_value
         FROM engine_runs e JOIN runs r ON r.id = e.run_id
         WHERE e.base_scenario = ?1 ORDER BY r.finished_at, r.id",
    )?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(RunPoint, String, Option<String>, Option<(f64, f64)>, Option<String>, Option<f64>)> = stmt
        .query_map([base_scenario], |r| {
            Ok((
                RunPoint {
                    run_id: r.get(0)?,
                    finished_at: r.get(1)?,
                    adaptive: r.get(2)?,
                    knob: r.get(10)?,
                    rating: r.get(3)?,
                    rating_low: r.get(11)?,
                    rating_high: r.get(12)?,
                    fastest: r.get(13)?,
                    success_rate: r.get(4)?,
                    score: r.get(5)?,
                    accuracy: r.get(6)?,
                    throughput: None,
                    reaction_ms: None,
                    on_target: None,
                    workout: r.get(7)?,
                    pick: r.get(18)?,
                    breakthrough: false,
                    replay: r.get(16)?,
                    current: false,
                },
                r.get(8)?,
                r.get(9)?,
                match (r.get::<_, Option<f64>>(14)?, r.get::<_, Option<f64>>(15)?) {
                    (Some(t), Some(sl)) => Some((t, sl)),
                    _ => None,
                },
                r.get(17)?,
                r.get(19)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Ok(None);
    }
    let category = rows.last().map(|r| r.1.clone()).unwrap_or_default();
    let latest = rows.iter().rev().find(|r| r.0.adaptive && r.0.rating.is_some());
    let latest_timeline = latest.and_then(|r| r.2.as_deref()).map(parse_timeline).unwrap_or_default();
    let knob = latest.and_then(|r| r.0.knob.clone());
    let model = latest.and_then(|r| r.3).zip(knob.clone());
    // The current version is the latest rated run's; a run with no fingerprint is a version too.
    let current = latest.map(|r| r.4.clone());
    let held: Vec<Held> = rows
        .iter()
        .map(|r| Held { knob: r.0.knob.as_deref(), hash: r.4.as_deref(), run_value: r.5, fastest: r.0.fastest })
        .collect();
    let broke = breakthroughs(&held);
    let mut runs: Vec<RunPoint> = rows
        .into_iter()
        .zip(broke)
        .map(|(r, breakthrough)| RunPoint { current: current.as_ref().is_none_or(|c| *c == r.4), breakthrough, ..r.0 })
        .collect();

    // Targets for every run, grouped by run.
    let mut by_run: HashMap<i64, Vec<T>> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT t.run_id, t.start_at, t.ended_at, t.outcome, t.distance_deg, t.direction_deg, t.radius_deg,
                t.reaction_s, t.overshoot_deg, t.entries, t.value, t.reach_s
         FROM engine_targets t JOIN engine_runs e ON e.run_id = t.run_id
         WHERE e.base_scenario = ?1",
    )?;
    let iter = stmt.query_map([base_scenario], |r| {
        let outcome: String = r.get(3)?;
        Ok(T {
            run_id: r.get(0)?,
            start_at: r.get(1)?,
            ended_at: r.get(2)?,
            killed: outcome == "killed",
            open: outcome == "open",
            distance: r.get(4)?,
            direction: r.get(5)?,
            radius: r.get(6)?,
            reaction: r.get(7)?,
            overshoot: r.get(8)?,
            entries: r.get(9)?,
            value: r.get(10)?,
            reach: r.get(11)?,
        })
    })?;
    for t in iter {
        let t = t?;
        by_run.entry(t.run_id).or_default().push(t);
    }

    let on_by_run = tracking_samples_by_run(conn, base_scenario)?;

    // Per-run figures.
    for run in &mut runs {
        let ts = by_run.get(&run.run_id).map(Vec::as_slice).unwrap_or(&[]);
        let ratios: Vec<f64> = ts
            .iter()
            .filter(|t| t.killed && t.distance >= 2.0 && t.ended_at > t.start_at)
            .map(|t| id_bits(t) / (t.ended_at - t.start_at))
            .collect();
        run.throughput = (ratios.len() >= 5).then(|| mean(ratios).unwrap_or(0.0));
        let mut rx = reactions_ms(ts);
        run.reaction_ms = if rx.len() >= 5 { median(&mut rx) } else { None };
        run.on_target = on_by_run.get(&run.run_id).filter(|(n, _)| *n > 0).map(|(n, on)| *on as f64 / *n as f64);
    }

    let recent_ids: Vec<i64> = runs.iter().rev().take(RECENT_RUNS).map(|r| r.run_id).collect();
    let before_ids: Vec<i64> = runs.iter().rev().skip(RECENT_RUNS).take(RECENT_RUNS).map(|r| r.run_id).collect();
    let pick =
        |ids: &[i64]| -> Vec<T> { ids.iter().flat_map(|id| by_run.get(id).cloned().unwrap_or_default()).collect() };
    let recent = pick(&recent_ids);
    let before = pick(&before_ids);
    let decided: Vec<&T> = recent.iter().filter(|t| !t.open).collect();
    let success =
        (!decided.is_empty()).then(|| decided.iter().filter(|t| t.killed).count() as f64 / decided.len() as f64);

    let fitts = fit_fitts(&recent, &before);
    let directions = direction_stats(&decided);
    let distances = distance_stats(&decided);
    let reaction = reaction_stats(&recent, &before);
    let killed: Vec<&&T> = decided.iter().filter(|t| t.killed).collect();
    let entries_per_kill = (killed.len() >= 5).then(|| mean(killed.iter().map(|t| t.entries as f64)).unwrap_or(0.0));
    let shots = shot_map(db, &recent_ids)?;
    let tracking = tracking_stats(db, &recent_ids, &before_ids)?;
    let switching = if category == "switching" {
        Some(switching_stats(db, (&recent_ids, &recent), (&before_ids, &before))?)
    } else {
        None
    };
    let segments = segment_stats(&decided);
    let curve = model.map(|((theta, slope), knob)| speed_curve(&knob, theta, slope, &decided));

    Ok(Some(Insights {
        scenario: base_scenario.to_string(),
        category,
        runs,
        knob,
        latest_timeline,
        curve,
        recent_runs: recent_ids.len(),
        targets: decided.len(),
        success,
        fitts,
        directions,
        distances,
        shots,
        reaction,
        entries_per_kill,
        tracking,
        switching,
        segments,
        plan: plan_check::by_kind(db, Some(base_scenario))?,
    }))
}

/// Tracking samples per run: (samples, samples on target).
fn tracking_samples_by_run(
    conn: &rusqlite::Connection,
    base_scenario: &str,
) -> Result<HashMap<i64, (usize, usize)>, DbError> {
    let mut on_by_run = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT s.run_id, COUNT(*), SUM(s.on_target) FROM engine_samples s
         JOIN engine_runs e ON e.run_id = s.run_id WHERE e.base_scenario = ?1 GROUP BY s.run_id",
    )?;
    for row in stmt.query_map([base_scenario], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as usize, r.get::<_, i64>(2)? as usize))
    })? {
        let (id, n, on) = row?;
        on_by_run.insert(id, (n, on));
    }
    Ok(on_by_run)
}

/// What [`breakthroughs`] needs to know about one run.
pub(crate) struct Held<'a> {
    pub(crate) knob: Option<&'a str>,
    /// The scenario's settings fingerprint: runs of another version are on another scale.
    pub(crate) hash: Option<&'a str>,
    /// Set when the run was played at one value (not a calibration run).
    pub(crate) run_value: Option<f64>,
    /// The value the goal was held at for the whole run.
    pub(crate) fastest: Option<f64>,
}

/// Which runs, in time order, were breakthroughs: held the goal at a value harder than
/// every earlier held run with the same knob and version. The first held run of a version
/// is never one. This is how `Rating::finish_run` in the sim decides, with the best-before
/// from `Db::fastest_hit`; breakthroughs are not stored, so they are derived here.
pub(crate) fn breakthroughs(runs: &[Held]) -> Vec<bool> {
    let mut best: HashMap<(&str, Option<&str>), f64> = HashMap::new();
    runs.iter()
        .map(|r| {
            let (Some(knob), Some(_), Some(fastest)) = (r.knob, r.run_value, r.fastest) else {
                return false;
            };
            let h = Knob::from_id(knob).h(fastest);
            match best.entry((knob, r.hash)) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(h);
                    false
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    let harder = h > *e.get() + 1e-6;
                    if harder {
                        e.insert(h);
                    }
                    harder
                }
            }
        })
        .collect()
}

/// Must match `aimadapt_sim::adaptive` (lapse rate and the log-scale knob); a test
/// in aimadapt-engine, which has both crates, checks.
pub const LAPSE: f64 = 0.04;

/// The curve's logit at which the chance of success, slips included, is `chance`.
/// Must match `aimadapt_sim::adaptive::logit_with_slips`, which the same test checks.
pub fn logit_with_slips(chance: f64) -> f64 {
    let q = ((chance - LAPSE / 2.0) / (1.0 - LAPSE)).clamp(0.01, 0.99);
    (q / (1.0 - q)).ln()
}

fn knob_h(knob: &str, v: f64) -> f64 {
    Knob::from_id(knob).h(v)
}

fn knob_value(knob: &str, h: f64) -> f64 {
    Knob::from_id(knob).value(h)
}

fn speed_curve(knob: &str, theta: f64, slope: f64, decided: &[&T]) -> Curve {
    let chance = |h: f64| LAPSE / 2.0 + (1.0 - LAPSE) / (1.0 + ((h - theta) / slope).exp());
    // Value at a given chance, inverting the model.
    let at = |p: f64| knob_value(knob, theta - slope * logit_with_slips(p));
    let (h95, h20) = (knob_h(knob, at(0.95)), knob_h(knob, at(0.2)));
    let model = (0..=40)
        .map(|i| {
            let h = h95 - 0.15 + (h20 - h95 + 0.3) * f64::from(i) / 40.0;
            (knob_value(knob, h), chance(h))
        })
        .collect();
    // Observed: recent targets in bins of equal width on the log scale.
    let mut pts: Vec<(f64, bool)> =
        decided.iter().filter_map(|t| t.value.map(|v| (knob_h(knob, v), t.killed))).collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut observed = Vec::new();
    if pts.len() >= 20 {
        let (lo, hi) = (pts[0].0, pts[pts.len() - 1].0);
        let bins = 8;
        for b in 0..bins {
            let (a, z) = (lo + (hi - lo) * b as f64 / bins as f64, lo + (hi - lo) * (b + 1) as f64 / bins as f64);
            let g: Vec<&(f64, bool)> =
                pts.iter().filter(|p| p.0 >= a && (p.0 < z || (b == bins - 1 && p.0 <= z))).collect();
            if g.len() >= 4 {
                let hit = g.iter().filter(|p| p.1).count() as f64 / g.len() as f64;
                observed.push((knob_value(knob, (a + z) / 2.0), hit, g.len()));
            }
        }
    }
    Curve { knob: knob.to_string(), model, observed, at95: at(0.95), at85: at(0.85), at50: at(0.5) }
}

fn fit_fitts(recent: &[T], before: &[T]) -> Option<Fitts> {
    let pts = |ts: &[T]| -> Vec<FlickPoint> {
        ts.iter()
            .filter(|t| t.killed && t.distance >= 2.0 && t.ended_at > t.start_at)
            .map(|t| FlickPoint {
                distance_deg: t.distance,
                id_bits: id_bits(t),
                time_ms: (t.ended_at - t.start_at) * 1000.0,
            })
            .collect()
    };
    let points = pts(recent);
    if points.len() < 10 {
        return None;
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.id_bits).sum::<f64>() / n;
    let my = points.iter().map(|p| p.time_ms).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.id_bits - mx).powi(2)).sum();
    let sxy: f64 = points.iter().map(|p| (p.id_bits - mx) * (p.time_ms - my)).sum();
    let syy: f64 = points.iter().map(|p| (p.time_ms - my).powi(2)).sum();
    let slope = if sxx > 1e-9 { sxy / sxx } else { 0.0 };
    let intercept = my - slope * mx;
    let r2 = if sxx > 1e-9 && syy > 1e-9 { (sxy * sxy) / (sxx * syy) } else { 0.0 };
    let tp = |p: &[FlickPoint]| mean(p.iter().map(|p| p.id_bits / (p.time_ms / 1000.0)));
    let prev = pts(before);
    let throughput_before = if prev.len() >= 10 { tp(&prev) } else { None };
    let throughput = tp(&points).unwrap_or(0.0);
    // Keep the chart light: the most recent 400 points. `recent` lists the newest
    // run first.
    let points = points.into_iter().take(400).collect();
    Some(Fitts { intercept_ms: intercept, slope_ms_per_bit: slope, r2, throughput, throughput_before, points })
}

const DIRECTIONS: [&str; 8] = ["Right", "Up-right", "Up", "Up-left", "Left", "Down-left", "Down", "Down-right"];

fn direction_stats(ts: &[&T]) -> Vec<DirectionStat> {
    let mut groups: Vec<Vec<&T>> = vec![Vec::new(); 8];
    for t in ts.iter().filter(|t| t.distance >= 2.0) {
        let i = ((t.direction / 45.0).round() as i64).rem_euclid(8) as usize;
        groups[i].push(t);
    }
    groups
        .into_iter()
        .enumerate()
        .filter(|(_, g)| g.len() >= 3)
        .map(|(i, g)| {
            let mut times: Vec<f64> =
                g.iter().filter(|t| t.killed).map(|t| (t.ended_at - t.start_at) * 1000.0).collect();
            DirectionStat {
                direction_deg: i as f64 * 45.0,
                label: DIRECTIONS[i],
                targets: g.len(),
                success: g.iter().filter(|t| t.killed).count() as f64 / g.len() as f64,
                median_ms: median(&mut times),
                overshoot_deg: mean(g.iter().map(|t| t.overshoot.max(0.0))).unwrap_or(0.0),
            }
        })
        .collect()
}

fn distance_stats(ts: &[&T]) -> Vec<DistanceStat> {
    let edges = [0.0, 5.0, 10.0, 20.0, 40.0, 180.0];
    edges
        .windows(2)
        .filter_map(|w| {
            let g: Vec<&&T> = ts.iter().filter(|t| t.distance >= w[0] && t.distance < w[1]).collect();
            if g.len() < 3 {
                return None;
            }
            let mut times: Vec<f64> =
                g.iter().filter(|t| t.killed).map(|t| (t.ended_at - t.start_at) * 1000.0).collect();
            Some(DistanceStat {
                from_deg: w[0],
                to_deg: w[1],
                targets: g.len(),
                success: g.iter().filter(|t| t.killed).count() as f64 / g.len() as f64,
                median_ms: median(&mut times),
            })
        })
        .collect()
}

/// Shortest flick, in degrees, whose reaction time counts. On shorter ones
/// "started moving" hardly means anything.
const MIN_REACTION_FLICK_DEG: f64 = 3.0;

/// Reaction times in ms, for flicks long enough to count. The Runs table and the
/// Reaction card both use this, so they agree.
fn reactions_ms(ts: &[T]) -> Vec<f64> {
    ts.iter().filter(|t| t.distance >= MIN_REACTION_FLICK_DEG).filter_map(|t| t.reaction).map(|r| r * 1000.0).collect()
}

fn reaction_stats(recent: &[T], before: &[T]) -> Option<Reaction> {
    let ms = reactions_ms;
    let mut v = ms(recent);
    if v.len() < 10 {
        return None;
    }
    let mut histogram = vec![0usize; 12];
    for x in &v {
        histogram[((x / 50.0) as usize).min(11)] += 1;
    }
    let mut b = ms(before);
    Some(Reaction {
        median_ms: median(&mut v)?,
        p25_ms: quantile(&mut v, 0.25)?,
        p75_ms: quantile(&mut v, 0.75)?,
        median_before_ms: if b.len() >= 10 { median(&mut b) } else { None },
        histogram,
    })
}

fn in_list(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

fn shot_map(db: &Db, ids: &[i64]) -> Result<Option<ShotMap>, DbError> {
    if ids.is_empty() {
        return Ok(None);
    }
    let sql = format!(
        "SELECT along, across, hit, dx_deg, dy_deg FROM engine_shots WHERE run_id IN ({}) ORDER BY run_id DESC, t",
        in_list(ids)
    );
    let mut stmt = db.conn().prepare(&sql)?;
    let rows: Vec<(f64, f64, bool, f64, f64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<_, _>>()?;
    if rows.len() < 10 {
        return Ok(None);
    }
    // Ignore wild misses (shots at a different target than the one recorded).
    let near: Vec<&(f64, f64, bool, f64, f64)> = rows.iter().filter(|r| r.0.hypot(r.1) <= 6.0).collect();
    let misses: Vec<&&(f64, f64, bool, f64, f64)> = near.iter().filter(|r| !r.2).collect();
    let miss_along = (misses.len() >= 3).then(|| mean(misses.iter().map(|r| r.0)).unwrap_or(0.0));
    let miss_across = (misses.len() >= 3).then(|| mean(misses.iter().map(|r| r.1)).unwrap_or(0.0));
    let overshoot_share =
        (misses.len() >= 3).then(|| misses.iter().filter(|r| r.0 > 0.0).count() as f64 / misses.len() as f64);
    Ok(Some(ShotMap {
        points: near.iter().take(600).map(|r| (r.0, r.1, r.2)).collect(),
        miss_along,
        miss_across,
        overshoot_share,
        vertical_deg: mean(near.iter().map(|r| r.4)),
        horizontal_deg: mean(near.iter().map(|r| r.3)),
    }))
}

/// (on target, error°, target speed °/s, lead°, seconds since direction change)
type Sample = (bool, f64, f64, f64, f64);

fn switching_stats(db: &Db, recent: (&[i64], &[T]), before: (&[i64], &[T])) -> Result<Switching, DbError> {
    let kpm = |ids: &[i64]| -> Result<Option<f64>, DbError> {
        if ids.is_empty() {
            return Ok(None);
        }
        let (kills, secs): (Option<f64>, Option<f64>) = db.conn().query_row(
            &format!("SELECT SUM(kills), SUM(duration_s) FROM runs WHERE id IN ({})", in_list(ids)),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(kills.zip(secs).filter(|(_, s)| *s >= 10.0).map(|(k, s)| k * 60.0 / s))
    };
    let acquire = |ts: &[T]| {
        let mut v: Vec<f64> = ts.iter().filter(|t| t.killed).filter_map(|t| t.reach).map(|r| r * 1000.0).collect();
        if v.len() >= 5 { median(&mut v) } else { None }
    };
    let engaged = |ids: &[i64]| -> Result<Option<f64>, DbError> {
        if ids.is_empty() {
            return Ok(None);
        }
        let (n, on): (i64, Option<i64>) = db.conn().query_row(
            &format!(
                "SELECT COUNT(*), SUM(on_target) FROM engine_samples WHERE engaged = 1 AND run_id IN ({})",
                in_list(ids)
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((n >= 30).then(|| on.unwrap_or(0) as f64 / n as f64))
    };
    Ok(Switching {
        kills_per_min: kpm(recent.0)?,
        kills_per_min_before: kpm(before.0)?,
        acquire_ms: acquire(recent.1),
        acquire_before_ms: acquire(before.1),
        engaged_on_target: engaged(recent.0)?,
        engaged_on_target_before: engaged(before.0)?,
    })
}

fn tracking_stats(db: &Db, recent: &[i64], before: &[i64]) -> Result<Option<Tracking>, DbError> {
    if recent.is_empty() {
        return Ok(None);
    }
    let load = |ids: &[i64]| -> Result<Vec<Sample>, DbError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT on_target, error_deg, target_speed_dps, lead_deg, since_change_s FROM engine_samples
             WHERE run_id IN ({})",
            in_list(ids)
        );
        let mut stmt = db.conn().prepare(&sql)?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    };
    let s = load(recent)?;
    if s.len() < 60 {
        return Ok(None);
    }
    let share = |v: &[&Sample]| v.iter().filter(|x| x.0).count() as f64 / v.len().max(1) as f64;
    let all: Vec<&Sample> = s.iter().collect();
    let on_target = share(&all);
    let b = load(before)?;
    let on_target_before = (b.len() >= 60).then(|| b.iter().filter(|x| x.0).count() as f64 / b.len() as f64);
    let mut err: Vec<f64> = s.iter().map(|x| x.1).collect();
    let centered = |v: &[Sample]| {
        let mut on: Vec<f64> = v.iter().filter(|x| x.0).map(|x| x.1).collect();
        if on.len() >= 30 { median(&mut on) } else { None }
    };
    let lead = mean(s.iter().filter(|x| x.2 > 2.0).map(|x| x.3)).unwrap_or(0.0);

    let bucket = |label: String, v: Vec<&Sample>| Bucket { label, samples: v.len(), on_target: share(&v) };
    let speed_edges = [0.0, 10.0, 20.0, 35.0, 60.0, f64::INFINITY];
    let by_speed: Vec<Bucket> = speed_edges
        .windows(2)
        .map(|w| {
            let label =
                if w[1].is_infinite() { format!("{}+ °/s", w[0]) } else { format!("{}–{} °/s", w[0], w[1]) };
            bucket(label, s.iter().filter(|x| x.2 >= w[0] && x.2 < w[1]).collect())
        })
        .filter(|b| b.samples >= 15)
        .collect();
    let change_edges = [0.0, 0.1, 0.2, 0.3, 0.45, 0.7, f64::INFINITY];
    let after_change: Vec<Bucket> = change_edges
        .windows(2)
        .map(|w| {
            let label = if w[1].is_infinite() {
                format!("{}+ ms", (w[0] * 1000.0) as i64)
            } else {
                format!("{}–{} ms", (w[0] * 1000.0) as i64, (w[1] * 1000.0) as i64)
            };
            bucket(label, s.iter().filter(|x| x.4 >= w[0] && x.4 < w[1]).collect())
        })
        .collect();
    // Recovery: the first bucket after a change whose on-target share is within 5
    // points of the steady (700 ms+) share.
    let steady = after_change.last().filter(|b| b.samples >= 15).map(|b| b.on_target);
    let recovery_s = steady.and_then(|st| {
        after_change
            .iter()
            .zip(change_edges.windows(2))
            .find(|(b, _)| b.samples >= 10 && b.on_target >= st - 0.05)
            .map(|(_, w)| w[0])
    });
    Ok(Some(Tracking {
        on_target,
        on_target_before,
        median_error_deg: median(&mut err).unwrap_or(0.0),
        lead_deg: lead,
        by_speed,
        after_change: after_change.into_iter().filter(|b| b.samples >= 10).collect(),
        recovery_s,
        stretch_s: unbroken_stretch(db, recent)?,
        stretch_before_s: unbroken_stretch(db, before)?,
        centered_deg: centered(&s),
        centered_before_deg: centered(&b),
    }))
}

fn segment_stats(ts: &[&T]) -> Vec<Segment> {
    let end = ts.iter().map(|t| t.ended_at).fold(0.0, f64::max);
    if end < 20.0 {
        return Vec::new();
    }
    let n = (end / 10.0).ceil() as usize;
    (0..n)
        .map(|i| {
            let (a, b) = (i as f64 * 10.0, (i + 1) as f64 * 10.0);
            let g: Vec<&&T> = ts.iter().filter(|t| t.ended_at >= a && t.ended_at < b).collect();
            Segment {
                from_s: a,
                to_s: b,
                success: (g.len() >= 3).then(|| g.iter().filter(|t| t.killed).count() as f64 / g.len() as f64),
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests;
