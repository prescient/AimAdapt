//! Schema v10: stored adaptive ratings count slips.
//!
//! Until then the goal point of a rating was `θ − s · ln(goal / (1 − goal))`, which ignores
//! the lapse rate, so the chance of success at the rating was about 83.6% for an 85% goal.
//! The point now uses [`logit_with_slips`], and a run stores the curve midpoint that puts
//! the goal rate exactly at its rating. This converts the ratings already saved: the
//! `engine_runs` columns `rating`, `rating_low`, `rating_high`, `start_rating`, `theta` and
//! `level_timeline`, and for an adaptive run its copies of the rating, which are `runs.score`
//! and the `Score`, `Rating` and `Start Rating` rows of `run_fields`. The old values are kept in
//! `engine_ratings_v9`, with the old score. Played values (`Run Value`, `Held Goal At`) stay.
//!
//! A rating held at one of the scenario's limits stays there unless its real point has moved
//! inside. A timeline of such a run keeps its points at the limit and shifts the others. The
//! limit isn't stored, so a point that sat at a limit inside a run that did not end at one is
//! shifted like the rest.

use rusqlite::{Transaction, params};

use crate::db::{DbError, MODE_ADAPTIVE};
use crate::insights::logit_with_slips;
use crate::knob::Knob;
use crate::telemetry::{parse_timeline, timeline_json};

/// Copies the old ratings aside, before [`convert_ratings`] rewrites them.
pub(crate) const MIGRATION: &str = r#"
CREATE TABLE engine_ratings_v9 (
    run_id         INTEGER PRIMARY KEY REFERENCES runs (id) ON DELETE CASCADE,
    rating         REAL,
    rating_low     REAL,
    rating_high    REAL,
    start_rating   REAL,
    theta          REAL,
    level_timeline TEXT,
    score          REAL
);
INSERT INTO engine_ratings_v9
SELECT e.run_id, e.rating, e.rating_low, e.rating_high, e.start_rating, e.theta, e.level_timeline, r.score
FROM engine_runs e JOIN runs r ON r.id = e.run_id WHERE e.rating IS NOT NULL;
"#;

/// A rating as stored before the change.
struct Old {
    run_id: i64,
    knob: Knob,
    goal: f64,
    slope: f64,
    rating: f64,
    low: Option<f64>,
    high: Option<f64>,
    start: Option<f64>,
    theta: Option<f64>,
    timeline: Option<String>,
    /// Whether the run is an adaptive one, whose score is its rating.
    adaptive_run: bool,
}

/// The same rating after it.
struct New {
    rating: f64,
    low: Option<f64>,
    high: Option<f64>,
    start: Option<f64>,
    theta: Option<f64>,
    timeline: Option<String>,
}

/// Rewrites every stored rating to count slips (the Rust step of schema v10).
pub(crate) fn convert_ratings(tx: &Transaction) -> Result<(), DbError> {
    let mut select = tx.prepare(
        "SELECT e.run_id, e.knob, e.goal, e.slope, e.rating, e.rating_low, e.rating_high, e.start_rating, e.theta,
                e.level_timeline, r.mode = ?1
         FROM engine_runs e JOIN runs r ON r.id = e.run_id
         WHERE e.knob IS NOT NULL AND e.goal IS NOT NULL AND e.slope IS NOT NULL AND e.rating IS NOT NULL",
    )?;
    let rows = select
        .query_map([MODE_ADAPTIVE], |r| {
            let knob: String = r.get(1)?;
            Ok(Old {
                run_id: r.get(0)?,
                knob: Knob::from_id(&knob),
                goal: r.get(2)?,
                slope: r.get(3)?,
                rating: r.get(4)?,
                low: r.get(5)?,
                high: r.get(6)?,
                start: r.get(7)?,
                theta: r.get(8)?,
                timeline: r.get(9)?,
                adaptive_run: r.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(select);
    let mut update = tx.prepare(
        "UPDATE engine_runs SET rating = ?2, rating_low = ?3, rating_high = ?4, start_rating = ?5, theta = ?6,
                level_timeline = ?7
         WHERE run_id = ?1",
    )?;
    for old in &rows {
        if let Some(n) = convert(old) {
            update.execute(params![old.run_id, n.rating, n.low, n.high, n.start, n.theta, n.timeline])?;
            if old.adaptive_run {
                convert_copies(tx, old.run_id, &n)?;
            }
        }
    }
    Ok(())
}

/// An adaptive run's score is its rating, and the text fields repeat it: bring them along,
/// rounded as the trainer stores them. A run with no such fields just has none to update.
fn convert_copies(tx: &Transaction, run_id: i64, n: &New) -> Result<(), DbError> {
    let score = format!("{:.3}", n.rating);
    tx.execute("UPDATE runs SET score = ?2 WHERE id = ?1", params![run_id, score.parse::<f64>().unwrap_or(n.rating)])?;
    let mut set = tx.prepare("UPDATE run_fields SET value = ?3 WHERE run_id = ?1 AND key = ?2")?;
    set.execute(params![run_id, "Score", score])?;
    set.execute(params![run_id, "Rating", format!("{:.4}", n.rating)])?;
    if let Some(start) = n.start {
        set.execute(params![run_id, "Start Rating", format!("{start:.4}")])?;
    }
    Ok(())
}

/// The converted rating, or `None` when its numbers can't be a rating (it stays as it was).
fn convert(old: &Old) -> Option<New> {
    let sane = old.goal > 0.0 && old.goal < 1.0 && old.slope > 0.0 && old.rating > 0.0 && old.rating.is_finite();
    if !sane {
        return None;
    }
    let k = old.knob;
    let old_logit = (old.goal / (1.0 - old.goal)).ln();
    let dh = old.slope * (logit_with_slips(old.goal) - old_logit);
    let shift = |v: f64| k.value(k.h(v) - dh);
    // The old unclamped goal point. The range was stored without the clamp, so it knows
    // the point even when the rating was held at one of the scenario's limits.
    let h_goal = match (old.low, old.high) {
        (Some(lo), Some(hi)) => (k.h(lo) + k.h(hi)) / 2.0,
        _ => k.h(old.rating),
    };
    let limit = k.h(old.rating);
    let held = (h_goal - limit).abs() > 1e-4;
    let rating = if held {
        // A rating at a limit stays there unless its real point has moved inside.
        let t = h_goal - dh;
        k.value(if h_goal > limit { t.min(limit) } else { t.max(limit) })
    } else {
        shift(old.rating)
    };
    let start = old.start.map(|s| if held && (k.h(s) - limit).abs() < 1e-6 { s } else { shift(s) });
    // Timeline values are printed to two decimals, so that is how close a point must be to the
    // limit to count as at it. A held rating is the limit's value.
    let at_limit = |v: f64| held && (v - old.rating).abs() < 0.005 + 1e-9;
    let timeline = match &old.timeline {
        Some(text) => {
            let points = parse_timeline(text);
            if points.is_empty() {
                old.timeline.clone()
            } else {
                let moved: Vec<(f64, f64)> =
                    points.into_iter().map(|(t, v)| (t, if at_limit(v) { v } else { shift(v) })).collect();
                Some(timeline_json(&moved))
            }
        }
        None => None,
    };
    Some(New {
        rating,
        low: old.low.map(shift),
        high: old.high.map(shift),
        start,
        theta: old.theta.map(|_| h_goal + old.slope * old_logit),
        timeline,
    })
}
