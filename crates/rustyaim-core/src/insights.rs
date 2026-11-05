//! Analytics for runs played in RustyAim's trainer, built from the per-target,
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

use std::collections::HashMap;

use crate::db::{Db, DbError};
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

/// Whether a bigger value of this knob is harder.
pub fn higher_is_harder(knob: &str) -> bool {
    knob == "target_speed"
}

/// Formats a knob value with its unit.
pub fn format_value(knob: &str, v: f64) -> String {
    match knob {
        "lifetime" | "kill_pace" => format!("{v:.2} s"),
        "target_speed" => format!("{v:.0} °/s"),
        _ => format!("{v:.3} m"),
    }
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

/// Tracking samples are taken this often (`rustyaim_sim::sim::SAMPLE_HZ`).
const SAMPLE_S: f64 = 1.0 / 30.0;

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
pub struct Note {
    pub title: String,
    pub body: String,
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
    pub notes: Vec<Note>,
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
            let agg = if higher_is_harder(&last.knob) { "MAX" } else { "MIN" };
            s.best_rating = db.conn().query_row(
                &format!("SELECT {agg}(rating) FROM engine_runs WHERE base_scenario = ?1 AND knob = ?2"),
                [&s.scenario, &last.knob],
                |r| r.get(0),
            )?;
            s.fastest = db.fastest_hit(&s.scenario, &last.knob, None)?;
            s.latest_rating = Some(last.rating);
            s.knob = Some(last.knob);
        }
    }
    Ok(out)
}

pub fn scenario(db: &Db, base_scenario: &str) -> Result<Option<Insights>, DbError> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT r.id, r.finished_at, e.adaptive, e.rating, e.success_rate, r.score, r.accuracy, e.workout,
                e.category, e.level_timeline, e.knob, e.rating_low, e.rating_high,
                CASE WHEN e.run_value IS NOT NULL THEN e.fastest END, e.theta, e.slope
         FROM engine_runs e JOIN runs r ON r.id = e.run_id
         WHERE e.base_scenario = ?1 ORDER BY r.finished_at, r.id",
    )?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(RunPoint, String, Option<String>, Option<(f64, f64)>)> = stmt
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
                },
                r.get(8)?,
                r.get(9)?,
                match (r.get::<_, Option<f64>>(14)?, r.get::<_, Option<f64>>(15)?) {
                    (Some(t), Some(sl)) => Some((t, sl)),
                    _ => None,
                },
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
    let mut runs: Vec<RunPoint> = rows.into_iter().map(|r| r.0).collect();

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

    // Tracking samples per run (on-target share, plus everything for recent runs).
    let mut on_by_run: HashMap<i64, (usize, usize)> = HashMap::new();
    {
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
    }

    // Per-run figures.
    for run in &mut runs {
        let ts = by_run.get(&run.run_id).map(Vec::as_slice).unwrap_or(&[]);
        let ratios: Vec<f64> = ts
            .iter()
            .filter(|t| t.killed && t.distance >= 2.0 && t.ended_at > t.start_at)
            .map(|t| id_bits(t) / (t.ended_at - t.start_at))
            .collect();
        run.throughput = (ratios.len() >= 5).then(|| mean(ratios).unwrap_or(0.0));
        let mut rx: Vec<f64> = ts.iter().filter_map(|t| t.reaction).map(|r| r * 1000.0).collect();
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

    let mut ins = Insights {
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
        notes: Vec::new(),
    };
    ins.notes = notes(&ins);
    Ok(Some(ins))
}

/// Must match `rustyaim_sim::adaptive` (lapse rate and the log-scale knob).
const LAPSE: f64 = 0.04;

fn knob_h(knob: &str, v: f64) -> f64 {
    let l = v.max(1e-4).ln();
    if higher_is_harder(knob) { l } else { -l }
}

fn knob_value(knob: &str, h: f64) -> f64 {
    if higher_is_harder(knob) { h.exp() } else { (-h).exp() }
}

fn speed_curve(knob: &str, theta: f64, slope: f64, decided: &[&T]) -> Curve {
    let chance = |h: f64| LAPSE / 2.0 + (1.0 - LAPSE) / (1.0 + ((h - theta) / slope).exp());
    // Value at a given chance, inverting the model.
    let at = |p: f64| {
        let q = ((p - LAPSE / 2.0) / (1.0 - LAPSE)).clamp(1e-3, 1.0 - 1e-3);
        knob_value(knob, theta - slope * (q / (1.0 - q)).ln())
    };
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
    // Keep the chart light: the most recent 400 points.
    let points = points.into_iter().rev().take(400).collect();
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

fn reaction_stats(recent: &[T], before: &[T]) -> Option<Reaction> {
    let ms = |ts: &[T]| -> Vec<f64> {
        // Only flicks long enough that "started moving" means something.
        ts.iter().filter(|t| t.distance >= 3.0).filter_map(|t| t.reaction).map(|r| r * 1000.0).collect()
    };
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
        rows.collect()
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

/// Plain-language takeaways, strongest first.
fn notes(i: &Insights) -> Vec<Note> {
    let mut out = Vec::new();
    let mut push = |title: String, body: String| out.push(Note { title, body });

    let rated: Vec<&RunPoint> = i.runs.iter().filter(|r| r.adaptive && r.rating.is_some()).collect();
    if let (Some(knob), Some(last)) = (&i.knob, rated.last()) {
        let fmt = |v: f64| format_value(knob, v);
        let now = last.rating.unwrap_or(0.0);
        let unit_word = match knob.as_str() {
            "target_speed" => "tracking speed",
            "lifetime" => "speed",
            "kill_pace" => "kill pace",
            _ => "size",
        };
        let mut body = format!("You hit {:.0}% at {}.", 85.0, fmt(now));
        if rated.len() >= 2 {
            let first = rated[0].rating.unwrap_or(now);
            let better = if higher_is_harder(knob) { now > first } else { now < first };
            body = format!(
                "{body} Your first rating was {} {} runs ago, so you're {} by {}.",
                fmt(first),
                rated.len() - 1,
                if better { "better" } else { "behind" },
                fmt((now - first).abs())
            );
        }
        push(format!("85% {unit_word}: {}", fmt(now)), body);
        // A lifetime includes your reaction time, which practice barely moves, so
        // the part you control improves by more than the rating shows.
        if knob == "lifetime" {
            let (react, whose) = match &i.reaction {
                Some(r) => (r.median_ms / 1000.0, "your median reaction"),
                None => (0.25, "a typical reaction"),
            };
            let rest = now - react;
            if rest > 0.05 {
                let mut body = format!(
                    "About {react:.2} s of your {} is reaction time ({whose}), which practice barely changes. The part you control is the other {rest:.2} s.",
                    fmt(now)
                );
                let first = rated[0].rating.unwrap_or(now);
                if rated.len() >= 2 && first > now && first - react > 0.05 {
                    body += &format!(
                        " Since your first rating that part is down {:.0}%, while the rating is down {:.0}%.",
                        (1.0 - rest / (first - react)) * 100.0,
                        (1.0 - now / first) * 100.0
                    );
                }
                push(format!("Of that, {react:.2} s is reaction"), body);
            }
        }
        if let Some(f) = rated
            .iter()
            .filter_map(|r| r.fastest)
            .reduce(|a, b| if higher_is_harder(knob) { a.max(b) } else { a.min(b) })
        {
            push(
                format!("Best held: {}", fmt(f)),
                "The hardest difficulty you've held the goal rate at for a whole run. Stretch runs give you the chance to beat it."
                    .into(),
            );
        }
    }
    if let Some(sw) = &i.switching
        && let Some(kpm) = sw.kills_per_min
    {
        let mut body = String::new();
        if let Some(a) = sw.acquire_ms {
            body += &format!("After a kill you reach the next target in {a:.0} ms (median). ");
        }
        if let Some(e) = sw.engaged_on_target {
            body += &format!("Once you're on a kill, you stay on target {:.0}% of the time. ", e * 100.0);
        }
        body += "Faster switching comes from both: getting to the next target sooner, and not slipping off the one you're killing.";
        push(format!("Switching: {kpm:.0} kills a minute"), body);
    }
    if let Some(c) = &i.curve {
        let fmt = |v: f64| format_value(&c.knob, v);
        // Spread between reliable (95%) and coin-flip (50%) on a relative scale.
        let spread = (knob_h(&c.knob, c.at50) - knob_h(&c.knob, c.at95)).abs();
        let (title, advice) = if spread < 0.25 {
            (
                "Consistent: you drop off sharply",
                "You're reliable up to your rating and fall away quickly past it. Push speed with stretch targets (the Push mix).",
            )
        } else {
            (
                "You fade gradually",
                "Your hit rate falls slowly as targets get faster, so misses happen even at easy speeds. Work on consistency (the Steady mix).",
            )
        };
        push(title.into(), format!("Reliable (95%) at {}, coin-flip (50%) at {}. {advice}", fmt(c.at95), fmt(c.at50)));
    }

    if i.directions.len() >= 4 {
        let worst = i.directions.iter().min_by(|a, b| a.success.total_cmp(&b.success)).unwrap();
        let best = i.directions.iter().max_by(|a, b| a.success.total_cmp(&b.success)).unwrap();
        if best.success - worst.success >= 0.12 {
            push(
                format!("Weakest direction: {}", worst.label.to_lowercase()),
                format!(
                    "You hit {:.0}% of targets to the {} vs. {:.0}% to the {}. Practice flicks that way, or check that your arm and mousepad aren't limiting that side.",
                    worst.success * 100.0,
                    worst.label.to_lowercase(),
                    best.success * 100.0,
                    best.label.to_lowercase()
                ),
            );
        }
        let slow = i
            .directions
            .iter()
            .filter(|d| d.median_ms.is_some())
            .max_by(|a, b| a.median_ms.unwrap_or(0.0).total_cmp(&b.median_ms.unwrap_or(0.0)));
        let fast = i
            .directions
            .iter()
            .filter(|d| d.median_ms.is_some())
            .min_by(|a, b| a.median_ms.unwrap_or(0.0).total_cmp(&b.median_ms.unwrap_or(0.0)));
        if let (Some(s), Some(f)) = (slow, fast)
            && s.median_ms.unwrap_or(0.0) > f.median_ms.unwrap_or(0.0) * 1.2
        {
            push(
                format!("Slower flicking {}", s.label.to_lowercase()),
                format!(
                    "{:.0} ms to the {} vs. {:.0} ms to the {} (median time to kill).",
                    s.median_ms.unwrap_or(0.0),
                    s.label.to_lowercase(),
                    f.median_ms.unwrap_or(0.0),
                    f.label.to_lowercase()
                ),
            );
        }
    }

    if let Some(m) = &i.shots
        && let Some(share) = m.overshoot_share
    {
        if share >= 0.65 {
            push(
                "You overshoot".into(),
                format!(
                    "{:.0}% of your misses land past the target. Slow down as you arrive, or try a slightly lower sensitivity.",
                    share * 100.0
                ),
            );
        } else if share <= 0.35 {
            push(
                "You stop short".into(),
                format!(
                    "{:.0}% of your misses land before the target. Commit to the full flick, or try a slightly higher sensitivity.",
                    (1.0 - share) * 100.0
                ),
            );
        }
        if let Some(v) = m.vertical_deg
            && v.abs() >= 0.15
        {
            push(
                format!("Shots land {}", if v > 0.0 { "high" } else { "low" }),
                format!(
                    "On average {:.2}° {} the target center. Watch where you settle vertically.",
                    v.abs(),
                    if v > 0.0 { "above" } else { "below" }
                ),
            );
        }
    }

    if let Some(e) = i.entries_per_kill
        && e >= 1.5
    {
        push(
            "Lots of corrections".into(),
            format!(
                "Your crosshair moves onto a target {e:.1} times per kill on average. Aim for one clean motion, then click."
            ),
        );
    }

    if let Some(r) = &i.reaction {
        let change = r
            .median_before_ms
            .map(|b| format!(" ({:+.0} ms vs. the runs before)", r.median_ms - b))
            .unwrap_or_default();
        push(
            format!("Reaction {:.0} ms", r.median_ms),
            format!(
                "Median time before your crosshair starts moving toward a new target{change}. Half your reactions fall between {:.0} and {:.0} ms.",
                r.p25_ms, r.p75_ms
            ),
        );
    }

    if let Some(f) = &i.fitts {
        let change = f
            .throughput_before
            .map(|b| format!(", {:+.0}% vs. the runs before", (f.throughput / b - 1.0) * 100.0))
            .unwrap_or_default();
        push(
            format!("Flick speed {:.1} bits/s", f.throughput),
            format!(
                "Throughput measures how fast you hit targets for their size and distance{change}. Each extra bit of difficulty costs you {:.0} ms.",
                f.slope_ms_per_bit
            ),
        );
    }

    if let Some(t) = &i.tracking {
        if t.lead_deg <= -0.3 {
            push(
                "You trail the target".into(),
                format!(
                    "Your crosshair sits {:.2}° behind the target's motion on average. Try to stay level with it or slightly ahead.",
                    -t.lead_deg
                ),
            );
        } else if t.lead_deg >= 0.3 {
            push(
                "You run ahead".into(),
                format!("Your crosshair leads the target by {:.2}° on average. Ease off a little.", t.lead_deg),
            );
        }
        if let Some(rec) = t.recovery_s
            && rec >= 0.2
        {
            push(
                format!("Direction changes cost you {:.0} ms", rec * 1000.0),
                "That's how long your time on target takes to recover after the target changes direction. Reactive tracking practice (strafes) trains this.".into(),
            );
        }
        if let (Some(fast), Some(slow)) = (t.by_speed.last(), t.by_speed.first())
            && slow.on_target - fast.on_target >= 0.15
        {
            push(
                "Fast targets are the gap".into(),
                format!(
                    "{:.0}% on target at {} vs. {:.0}% at {}.",
                    fast.on_target * 100.0,
                    fast.label,
                    slow.on_target * 100.0,
                    slow.label
                ),
            );
        }
    }

    let seg: Vec<f64> = i.segments.iter().filter_map(|s| s.success).collect();
    if seg.len() >= 4 {
        let start = (seg[0] + seg[1]) / 2.0;
        let end = (seg[seg.len() - 1] + seg[seg.len() - 2]) / 2.0;
        if end < start - 0.1 {
            push(
                "You fade late in runs".into(),
                format!(
                    "Success drops from {:.0}% in the first 20 s to {:.0}% in the last 20 s. Short breaks between runs may help.",
                    start * 100.0,
                    end * 100.0
                ),
            );
        } else if start < end - 0.1 {
            push(
                "Slow starts".into(),
                format!(
                    "Success climbs from {:.0}% in the first 20 s to {:.0}% by the end. A short warm-up before this scenario could help.",
                    start * 100.0,
                    end * 100.0
                ),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::RunFile;
    use crate::telemetry::{EngineRun, SampleRow, ShotRow, TargetRow};
    use chrono::NaiveDate;
    use std::collections::BTreeMap;

    fn save(db: &mut Db, n: u32, e: &EngineRun) {
        let at = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(8, n, 0).unwrap();
        let mut fields = BTreeMap::new();
        fields.insert("Score".to_string(), "50".to_string());
        fields.insert("Source".to_string(), "rustyaim".to_string());
        // A 60 s run, with its kills.
        let start = at - chrono::Duration::seconds(60);
        fields.insert("Challenge Start".to_string(), start.format("%H:%M:%S%.3f").to_string());
        let kills = e.targets.iter().filter(|t| t.outcome == "killed").count();
        fields.insert("Kills".to_string(), kills.to_string());
        let file_name = format!("rustyaim/test/{n}");
        let run = RunFile {
            file_name: file_name.clone(),
            scenario: e.base_scenario.clone(),
            mode: "RustyAim".into(),
            finished_at: at,
            fields,
            kills: vec![],
            weapons: vec![],
        };
        db.insert_runs(&[run], at).unwrap();
        assert!(db.save_engine_run(&file_name, e).unwrap());
        assert!(!db.save_engine_run(&file_name, e).unwrap(), "details are saved once");
    }

    /// A clicking run where flicks to the left are slow and often time out, and
    /// misses land past the target.
    fn clicking_run(rating: f64) -> EngineRun {
        let mut targets = Vec::new();
        let mut shots = Vec::new();
        for i in 0..80 {
            let dir = (i % 8) as f64 * 45.0;
            let left = dir == 180.0;
            let distance = 5.0 + (i % 5) as f64 * 6.0;
            let time = 0.25 + distance / 60.0 + if left { 0.25 } else { 0.0 };
            let killed = !(left && i % 16 < 8);
            let start = i as f64 * 0.7;
            targets.push(TargetRow {
                no: i,
                spawned_at: start,
                start_at: start,
                ended_at: start + time,
                outcome: if killed { "killed" } else { "timed_out" }.into(),
                distance_deg: distance,
                direction_deg: dir,
                radius_deg: 1.0,
                lifetime_s: 1.5,
                reaction_s: Some(0.2 + (i % 3) as f64 * 0.02),
                reach_s: Some(time - 0.05),
                overshoot_deg: 0.3,
                entries: 1,
                shots: 1,
                hits: i64::from(killed),
                value: Some(0.4 + (i % 10) as f64 * 0.08),
                chance: Some(0.85),
                pick: Some("near".into()),
            });
            shots.push(ShotRow {
                t: start + time,
                hit: killed,
                target_no: Some(i),
                dx_deg: 0.0,
                dy_deg: -0.2,
                along: if killed { 0.1 } else { 1.6 },
                across: 0.0,
            });
        }
        EngineRun {
            base_scenario: "Timed Flicks".into(),
            category: "clicking".into(),
            adaptive: true,
            knob: Some("lifetime".into()),
            rating: Some(rating),
            rating_low: Some(rating * 0.95),
            rating_high: Some(rating * 1.05),
            start_rating: Some(rating + 0.1),
            theta: Some(-(rating.ln()) + 0.12 * 1.7346),
            slope: Some(0.12),
            rating_sd: Some(0.05),
            fastest: Some(rating * 0.8),
            run_value: Some(rating * 0.8),
            run_pick: Some("stretch".into()),
            planned_chance: Some(0.6),
            mix: Some("balanced".into()),
            goal: Some(0.85),
            success_rate: Some(0.85),
            rating_timeline: vec![(0.0, rating + 0.1), (60.0, rating)],
            targets,
            shots,
            ..Default::default()
        }
    }

    #[test]
    fn clicking_insights_find_the_weak_side_and_overshoot() {
        let mut db = Db::open_in_memory().unwrap();
        save(&mut db, 1, &clicking_run(0.8));
        save(&mut db, 2, &clicking_run(0.62));
        let i = scenario(&db, "Timed Flicks").unwrap().unwrap();
        assert_eq!(i.runs.len(), 2);
        assert_eq!(i.latest_timeline.last(), Some(&(60.0, 0.62)));
        let c = i.curve.as_ref().unwrap();
        assert!((c.at85 - 0.62).abs() < 0.01, "{c:?}");
        assert!(c.at95 > c.at85 && c.at85 > c.at50, "lifetime: reliable is slower than coin-flip");
        assert!(!c.observed.is_empty());
        let left = i.directions.iter().find(|d| d.label == "Left").unwrap();
        assert!((left.success - 0.5).abs() < 1e-9, "{left:?}");
        let f = i.fitts.as_ref().unwrap();
        assert!(f.slope_ms_per_bit > 0.0 && f.throughput > 0.0, "{f:?}");
        assert!(i.runs[1].throughput.is_some());
        let m = i.shots.as_ref().unwrap();
        assert_eq!(m.overshoot_share, Some(1.0));
        assert!((m.vertical_deg.unwrap() + 0.2).abs() < 1e-9);
        let r = i.reaction.as_ref().unwrap();
        assert!((r.median_ms - 220.0).abs() < 1.0, "{r:?}");
        let titles: Vec<&str> = i.notes.iter().map(|n| n.title.as_str()).collect();
        assert!(titles.contains(&"Weakest direction: left"), "{titles:?}");
        assert!(titles.contains(&"You overshoot"), "{titles:?}");
        assert!(titles.contains(&"Shots land low"), "{titles:?}");
        assert!(titles.contains(&"85% speed: 0.62 s"), "{titles:?}");
        assert!(titles.contains(&"Best held: 0.50 s"), "{titles:?}");
        let better = &i.notes.iter().find(|n| n.title.starts_with("85% speed")).unwrap().body;
        assert!(better.contains("better by 0.18 s"), "{better}");
        // Scoring review, finding 9: the reaction floor. 0.80 → 0.62 s is 23% on the
        // rating, but the part you control fell from 0.58 to 0.40 s, 31%.
        let floor = &i.notes.iter().find(|n| n.title == "Of that, 0.22 s is reaction").unwrap().body;
        assert!(floor.contains("other 0.40 s") && floor.contains("down 31%") && floor.contains("down 23%"), "{floor}");

        let o = overview(&db).unwrap();
        assert_eq!(o[0].scenario, "Timed Flicks");
        assert_eq!(o[0].latest_rating, Some(0.62));
        assert_eq!(o[0].best_rating, Some(0.62), "lower lifetime is better");
        assert_eq!(o[0].fastest, Some(0.62 * 0.8));
        assert_eq!(db.last_rating("Timed Flicks").unwrap().unwrap().knob, "lifetime");
    }

    #[test]
    fn switching_insights_split_acquisition_from_time_on_target() {
        let mut db = Db::open_in_memory().unwrap();
        let mut e = clicking_run(1.5);
        e.base_scenario = "Stability Switch".into();
        e.category = "switching".into();
        e.knob = Some("target_speed".into());
        for (i, t) in e.targets.iter_mut().enumerate() {
            t.outcome = "killed".into();
            t.reach_s = Some(0.2 + (i % 3) as f64 * 0.05);
        }
        // 300 engaged samples, 240 on target; 300 more between targets, all off.
        e.samples = (0..600)
            .map(|i| SampleRow { t: i as f64 / 30.0, on_target: i < 240, engaged: i < 300, ..Default::default() })
            .collect();
        save(&mut db, 1, &e);
        let i = scenario(&db, "Stability Switch").unwrap().unwrap();
        let sw = i.switching.as_ref().unwrap();
        assert!((sw.acquire_ms.unwrap() - 250.0).abs() < 1.0, "{sw:?}");
        assert!((sw.engaged_on_target.unwrap() - 0.8).abs() < 1e-9, "travel isn't counted: {sw:?}");
        assert!(i.tracking.as_ref().unwrap().on_target < 0.5, "all samples: {:?}", i.tracking);
        let titles: Vec<&str> = i.notes.iter().map(|n| n.title.as_str()).collect();
        assert!((sw.kills_per_min.unwrap() - 80.0).abs() < 1e-6, "80 kills in a minute: {sw:?}");
        assert!(titles.contains(&"Switching: 80 kills a minute"), "{titles:?}");
        // Not shown for other categories.
        save(&mut db, 2, &clicking_run(0.6));
        assert!(scenario(&db, "Timed Flicks").unwrap().unwrap().switching.is_none());
    }

    #[test]
    fn unbroken_stretch_is_where_half_the_time_on_target_comes_from() {
        let mut db = Db::open_in_memory().unwrap();
        // Four 1 s stretches and two 3 s ones, with short gaps: 10 s on target, of
        // which the two 3 s stretches hold 6 s, more than half.
        let mut on = Vec::new();
        for len in [30, 90, 30, 30, 90, 30] {
            on.extend(std::iter::repeat_n(true, len));
            on.extend(std::iter::repeat_n(false, 3));
        }
        let samples = on
            .iter()
            .enumerate()
            .map(|(i, &o)| SampleRow { t: i as f64 / 30.0, on_target: o, ..Default::default() })
            .collect();
        let e = EngineRun {
            base_scenario: "Float Track".into(),
            category: "tracking".into(),
            samples,
            ..Default::default()
        };
        save(&mut db, 1, &e);
        let t = scenario(&db, "Float Track").unwrap().unwrap().tracking.unwrap();
        assert!((t.stretch_s.unwrap() - 3.0).abs() < 1e-9, "{:?}", t.stretch_s);
    }

    /// Before fixed-difficulty runs, `fastest` was the hardest single target hit,
    /// which a whole run can rarely match. Those old values must not count as the
    /// best held, or breakthroughs would almost never happen.
    #[test]
    fn single_target_bests_from_old_runs_are_ignored() {
        let mut db = Db::open_in_memory().unwrap();
        let old = EngineRun { fastest: Some(0.2), run_value: None, run_pick: None, ..clicking_run(0.8) };
        save(&mut db, 1, &old);
        assert_eq!(db.fastest_hit("Timed Flicks", "lifetime", None).unwrap(), None);
        save(&mut db, 2, &clicking_run(0.62));
        assert_eq!(db.fastest_hit("Timed Flicks", "lifetime", None).unwrap(), Some(0.62 * 0.8));
        let i = scenario(&db, "Timed Flicks").unwrap().unwrap();
        assert_eq!(i.runs[0].fastest, None);
        assert_eq!(i.runs[1].fastest, Some(0.62 * 0.8));
    }

    #[test]
    fn tracking_insights_measure_lag_and_recovery() {
        let mut db = Db::open_in_memory().unwrap();
        let samples = (0..900)
            .map(|i| {
                let since = (i % 30) as f64 / 30.0;
                // Off target for the first 200 ms after each change.
                let on = since >= 0.2 && i % 7 != 0;
                SampleRow {
                    t: i as f64 / 30.0,
                    on_target: on,
                    error_deg: if on { 0.3 + (i % 3) as f64 * 0.1 } else { 2.0 },
                    target_speed_dps: 25.0,
                    lead_deg: -0.5,
                    since_change_s: since,
                    engaged: false,
                }
            })
            .collect();
        let e = EngineRun {
            base_scenario: "Strafe Track".into(),
            category: "tracking".into(),
            samples,
            ..Default::default()
        };
        save(&mut db, 1, &e);
        let i = scenario(&db, "Strafe Track").unwrap().unwrap();
        let t = i.tracking.as_ref().unwrap();
        assert!((t.lead_deg + 0.5).abs() < 1e-9);
        // Scoring review, finding 7: how centered you stay while on target.
        assert!((t.centered_deg.unwrap() - 0.4).abs() < 1e-9, "{t:?}");
        assert_eq!(t.centered_before_deg, None);
        assert_eq!(t.recovery_s, Some(0.2), "{t:?}");
        assert!(t.after_change[0].on_target < 0.01);
        assert!(i.runs[0].on_target.unwrap() > 0.6);
        let titles: Vec<&str> = i.notes.iter().map(|n| n.title.as_str()).collect();
        assert!(titles.contains(&"You trail the target"), "{titles:?}");
        assert!(titles.iter().any(|t| t.starts_with("Direction changes cost you 200 ms")), "{titles:?}");
    }
}
