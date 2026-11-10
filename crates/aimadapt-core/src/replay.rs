//! Replays of runs played in AimAdapt's own trainer, redrawn from what the run
//! recorded: the 240 Hz aim trace, where every target was 60 times a second, and
//! each shot. Nothing is re-simulated, so a replay stays true to what happened even
//! after the game's rules change.

use std::collections::{HashMap, HashSet};

use rusqlite::OptionalExtension;

use crate::db::{Db, DbError};
use crate::knob::Knob;
use crate::telemetry::{decode_frames, decode_trace};

/// Field of view to show when a run didn't record one (the trainer's default).
const DEFAULT_FOV: f64 = 103.0;

/// Each scenario keeps the replays of its latest this many runs, and of its best run.
pub const REPLAYS_KEPT: usize = 50;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Replay {
    pub run_id: i64,
    /// The run's name as played, with " (Adaptive)" for adaptive runs.
    pub scenario: String,
    pub finished_at: String,
    pub duration_s: f64,
    /// Horizontal field of view at 16:9, degrees.
    pub fov: f64,
    /// (seconds, yaw°, pitch°) of the crosshair.
    pub trace: Vec<[f32; 3]>,
    pub frames: Vec<ReplayFrame>,
    pub shots: Vec<ReplayShot>,
    /// Targets killed or timed out, in order.
    pub ends: Vec<ReplayEnd>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayFrame {
    pub t: f64,
    /// Each target as [number, yaw°, pitch°, radius°, height° (0 = sphere), health 0–1],
    /// kept as arrays so a minute of frames stays small for the UI.
    pub targets: Vec<[f64; 6]>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayShot {
    pub t: f64,
    pub hit: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayEnd {
    pub t: f64,
    pub no: i64,
    pub killed: bool,
}

/// The replay of a run, or `None` if it isn't a trainer run or was played before
/// replays were recorded.
pub fn replay(db: &Db, run_id: i64) -> Result<Option<Replay>, DbError> {
    let conn = db.conn();
    #[allow(clippy::type_complexity)]
    let row: Option<(String, String, Option<f64>, Option<f64>, Option<Vec<u8>>, Option<Vec<u8>>)> = conn
        .query_row(
            "SELECT r.scenario, r.finished_at, r.duration_s, r.fov, e.trace, e.frames
             FROM engine_runs e JOIN runs r ON r.id = e.run_id WHERE r.id = ?1",
            [run_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?;
    let Some((scenario, finished_at, duration_s, fov, trace, Some(frames))) = row else {
        return Ok(None);
    };
    let trace = trace.map(|b| decode_trace(&b)).unwrap_or_default();
    let frames: Vec<ReplayFrame> = decode_frames(&frames)
        .into_iter()
        .map(|f| ReplayFrame {
            t: f.t,
            targets: f.targets.iter().map(|p| [p.no as f64, p.yaw, p.pitch, p.radius, p.height, p.health]).collect(),
        })
        .collect();
    let mut stmt = conn.prepare("SELECT t, hit FROM engine_shots WHERE run_id = ?1 ORDER BY t")?;
    let shots =
        stmt.query_map([run_id], |r| Ok(ReplayShot { t: r.get(0)?, hit: r.get(1)? }))?.collect::<Result<_, _>>()?;
    let mut stmt = conn.prepare(
        "SELECT ended_at, no, outcome = 'killed' FROM engine_targets
         WHERE run_id = ?1 AND outcome != 'open' ORDER BY ended_at, no",
    )?;
    let ends = stmt
        .query_map([run_id], |r| Ok(ReplayEnd { t: r.get(0)?, no: r.get(1)?, killed: r.get(2)? }))?
        .collect::<Result<_, _>>()?;
    let last = |t: Option<f64>| t.unwrap_or(0.0);
    let duration_s = duration_s
        .filter(|d| *d > 0.0)
        .unwrap_or_else(|| last(trace.last().map(|s| f64::from(s[0]))).max(last(frames.last().map(|f| f.t))));
    Ok(Some(Replay {
        run_id,
        scenario,
        finished_at,
        duration_s,
        fov: fov.filter(|f| (30.0..=150.0).contains(f)).unwrap_or(DEFAULT_FOV),
        trace,
        frames,
        shots,
        ends,
    }))
}

/// What a best is kept for: a version of the scenario (its fingerprint), the mode
/// (adaptive or not) and the knob.
type Version<'a> = (Option<&'a str>, bool, Option<&'a str>);

/// A trainer run, as [`prune`] weighs it.
struct Kept {
    run_id: i64,
    hash: Option<String>,
    adaptive: bool,
    knob: Option<String>,
    score: Option<f64>,
    rating: Option<f64>,
    has_replay: bool,
}

/// Drops the replays (aim trace and target frames) of a scenario's older trainer
/// runs, keeping its latest [`REPLAYS_KEPT`] and the best run on each version of
/// it: the top score, or for adaptive runs the best rating for the knob, and the
/// first run to reach it. The runs' stats, targets, shots and tracking samples
/// stay, so progress and insights don't change. Returns how many were dropped.
pub(crate) fn prune(tx: &rusqlite::Transaction, base_scenario: &str) -> Result<usize, DbError> {
    let mut st = tx.prepare_cached(
        "SELECT e.run_id, r.scenario_hash, e.adaptive, e.knob, r.score, e.rating,
                e.trace IS NOT NULL OR e.frames IS NOT NULL
         FROM engine_runs e JOIN runs r ON r.id = e.run_id
         WHERE e.base_scenario = ?1 ORDER BY r.finished_at, r.id",
    )?;
    let runs: Vec<Kept> = st
        .query_map([base_scenario], |r| {
            Ok(Kept {
                run_id: r.get(0)?,
                hash: r.get(1)?,
                adaptive: r.get(2)?,
                knob: r.get(3)?,
                score: r.get(4)?,
                rating: r.get(5)?,
                has_replay: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    if runs.len() <= REPLAYS_KEPT {
        return Ok(0);
    }
    // The best run of each version and mode; a later tie doesn't replace it.
    let mut best: HashMap<Version, (i64, f64)> = HashMap::new();
    for r in &runs {
        let Some(v) = (if r.adaptive { r.rating } else { r.score }).filter(|v| v.is_finite()) else { continue };
        let knob = Knob::from_id(r.knob.as_deref().unwrap_or_default());
        let better = |a: f64, b: f64| if r.adaptive { knob.better(a, b) } else { a > b };
        best.entry((r.hash.as_deref(), r.adaptive, r.knob.as_deref()))
            .and_modify(|b| {
                if better(v, b.1) {
                    *b = (r.run_id, v);
                }
            })
            .or_insert((r.run_id, v));
    }
    let keep: HashSet<i64> =
        runs.iter().rev().take(REPLAYS_KEPT).map(|r| r.run_id).chain(best.values().map(|b| b.0)).collect();
    let mut drop = tx.prepare_cached("UPDATE engine_runs SET trace = NULL, frames = NULL WHERE run_id = ?1")?;
    let mut dropped = 0;
    for r in runs.iter().filter(|r| r.has_replay && !keep.contains(&r.run_id)) {
        dropped += drop.execute([r.run_id])?;
    }
    Ok(dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::RunFile;
    use crate::telemetry::{EngineRun, Frame, Pose, ShotRow, TargetRow};
    use chrono::NaiveDate;
    use std::collections::BTreeMap;

    fn save(db: &mut Db, name: &str, e: &EngineRun) -> i64 {
        save_at(db, name, e, 0, &[])
    }

    /// Saves a two-second run that finished `minutes` after 9:00, with `extra` stats fields.
    fn save_at(db: &mut Db, name: &str, e: &EngineRun, minutes: i64, extra: &[(&str, String)]) -> i64 {
        let at = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap().and_hms_opt(9, 0, 0).unwrap()
            + chrono::Duration::minutes(minutes);
        let mut fields = BTreeMap::new();
        fields.insert("Source".to_string(), "rustyaim".to_string());
        fields.insert("FOV".to_string(), "90".to_string());
        let start = at - chrono::Duration::seconds(2);
        fields.insert("Challenge Start".to_string(), start.format("%H:%M:%S%.3f").to_string());
        for (k, v) in extra {
            fields.insert((*k).to_string(), v.clone());
        }
        let run = RunFile {
            file_name: name.into(),
            scenario: e.base_scenario.clone(),
            mode: "RustyAim".into(),
            finished_at: at,
            fields,
            kills: vec![],
            weapons: vec![],
        };
        db.insert_runs(&[run], at).unwrap();
        assert!(db.save_engine_run(name, e).unwrap());
        db.conn().query_row("SELECT id FROM runs WHERE file_name = ?1", [name], |r| r.get(0)).unwrap()
    }

    fn run() -> EngineRun {
        let pose = |no, yaw| Pose { no, yaw, pitch: 1.0, radius: 0.8, height: 0.0, health: 1.0 };
        EngineRun {
            base_scenario: "Timed Flicks".into(),
            category: "clicking".into(),
            trace: [[0.0f32, 0.0, 0.0], [1.0, 10.0, 1.0]].iter().flatten().flat_map(|v| v.to_le_bytes()).collect(),
            frames: vec![
                Frame { t: 0.0, targets: vec![pose(1, 10.0)] },
                Frame { t: 1.0, targets: vec![pose(2, -20.0)] },
            ],
            targets: vec![TargetRow { no: 1, ended_at: 0.95, outcome: "killed".into(), ..Default::default() }],
            shots: vec![ShotRow { t: 0.95, hit: true, ..Default::default() }],
            ..Default::default()
        }
    }

    #[test]
    fn a_replay_has_the_trace_targets_shots_and_kills() {
        let mut db = Db::open_in_memory().unwrap();
        let id = save(&mut db, "rustyaim/test/1", &run());
        let r = replay(&db, id).unwrap().unwrap();
        assert_eq!(r.scenario, "Timed Flicks");
        assert_eq!((r.duration_s, r.fov), (2.0, 90.0));
        assert_eq!(r.trace, vec![[0.0, 0.0, 0.0], [1.0, 10.0, 1.0]]);
        assert_eq!(r.frames.len(), 2);
        assert_eq!(r.frames[1].targets, vec![[2.0, -20.0, 1.0, 0.8, 0.0, 1.0]]);
        assert_eq!(r.shots, vec![ReplayShot { t: 0.95, hit: true }]);
        assert_eq!(r.ends, vec![ReplayEnd { t: 0.95, no: 1, killed: true }]);
        let i = crate::insights::scenario(&db, "Timed Flicks").unwrap().unwrap();
        assert!(i.runs[0].replay);
    }

    fn has_replay(db: &Db, id: i64) -> bool {
        replay(db, id).unwrap().is_some()
    }

    /// Replays took about 170 KB a minute of play and were kept forever. A
    /// scenario keeps the replays of its latest runs and its best run (the first
    /// to reach the top score); older runs lose only the replay.
    #[test]
    fn old_replays_are_dropped_but_the_latest_and_the_best_stay() {
        let mut db = Db::open_in_memory().unwrap();
        let other = save(&mut db, "rustyaim/test/other", &EngineRun { base_scenario: "Other".into(), ..run() });
        let n = REPLAYS_KEPT + 10;
        let ids: Vec<i64> = (0..n)
            .map(|i| {
                // Runs 5 and 7 tie on the best score; 5 reached it first.
                let score = if i == 5 || i == 7 { 900.0 } else { 100.0 + i as f64 };
                let extra = [("Score", score.to_string()), ("Hash", "v1".to_string())];
                save_at(&mut db, &format!("rustyaim/test/{i}"), &run(), i as i64 + 1, &extra)
            })
            .collect();
        let kept: Vec<usize> = (0..n).filter(|&i| has_replay(&db, ids[i])).collect();
        let expected: Vec<usize> = std::iter::once(5).chain(10..n).collect();
        assert_eq!(kept, expected);
        assert!(has_replay(&db, other), "other scenarios keep theirs");
        // Only the replay goes: the run, its targets and shots stay for stats and insights.
        let count = |table: &str| -> i64 {
            db.conn().query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
        };
        assert_eq!(
            (count("runs"), count("engine_runs"), count("engine_targets"), count("engine_shots")),
            (61, 61, 61, 61)
        );
        let i = crate::insights::scenario(&db, "Timed Flicks").unwrap().unwrap();
        assert_eq!(i.runs.len(), n);
        assert_eq!(i.runs.iter().filter(|r| r.replay).count(), REPLAYS_KEPT + 1, "the Replay link goes with it");
    }

    /// An adaptive run's best is its rating, and a shorter lifetime is the better
    /// one. Each version of a scenario keeps its own best, as its rating does.
    #[test]
    fn adaptive_replays_keep_the_best_rating_of_each_version() {
        let mut db = Db::open_in_memory().unwrap();
        let rated =
            |rating: f64| EngineRun { adaptive: true, knob: Some("lifetime".into()), rating: Some(rating), ..run() };
        let mut save_rated = |i: i64, rating: f64, hash: &str| {
            save_at(&mut db, &format!("rustyaim/test/{i}"), &rated(rating), i, &[("Hash", hash.to_string())])
        };
        let best = save_rated(0, 0.6, "v1");
        let worse = save_rated(1, 3.0, "v1");
        let other_version = save_rated(2, 2.0, "v2");
        for i in 3..3 + REPLAYS_KEPT as i64 {
            save_rated(i, 1.0, "v1");
        }
        assert!(has_replay(&db, best), "the lowest lifetime is the best");
        assert!(!has_replay(&db, worse));
        assert!(has_replay(&db, other_version), "the best on its version");
    }

    #[test]
    fn runs_from_before_replays_have_none() {
        let mut db = Db::open_in_memory().unwrap();
        let id = save(&mut db, "rustyaim/test/old", &EngineRun { frames: vec![], ..run() });
        assert_eq!(replay(&db, id).unwrap(), None);
        assert_eq!(replay(&db, 999).unwrap(), None, "no such run");
        let i = crate::insights::scenario(&db, "Timed Flicks").unwrap().unwrap();
        assert!(!i.runs[0].replay, "the Replay link is hidden");
    }
}
