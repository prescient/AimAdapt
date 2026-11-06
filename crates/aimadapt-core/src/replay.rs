//! Replays of runs played in AimAdapt's own trainer, redrawn from what the run
//! recorded: the 240 Hz aim trace, where every target was 60 times a second, and
//! each shot. Nothing is re-simulated, so a replay stays true to what happened even
//! after the game's rules change.

use rusqlite::OptionalExtension;

use crate::db::{Db, DbError};
use crate::telemetry::{decode_frames, decode_trace};

/// Field of view to show when a run didn't record one (the trainer's default).
const DEFAULT_FOV: f64 = 103.0;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::RunFile;
    use crate::telemetry::{EngineRun, Frame, Pose, ShotRow, TargetRow};
    use chrono::NaiveDate;
    use std::collections::BTreeMap;

    fn save(db: &mut Db, name: &str, e: &EngineRun) -> i64 {
        let at = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap().and_hms_opt(9, 0, 0).unwrap();
        let mut fields = BTreeMap::new();
        fields.insert("Source".to_string(), "rustyaim".to_string());
        fields.insert("FOV".to_string(), "90".to_string());
        fields.insert("Challenge Start".to_string(), "08:59:58.000".to_string());
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
