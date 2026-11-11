//! Detailed records for runs played in AimAdapt's own trainer: one row per target,
//! per shot and per tracking sample, plus the adaptive rating and its history and the
//! aim trace. KovaaK's runs have none of this; [`crate::insights`] builds analytics on it.

use std::collections::HashMap;

use rusqlite::{OptionalExtension, params};

use crate::db::{Db, DbError};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngineRun {
    /// Scenario name without the " (Adaptive)" suffix, so both modes group together.
    pub base_scenario: String,
    pub category: String,
    pub adaptive: bool,
    /// Adaptive mode: `lifetime`, `target_speed` or `radius`.
    pub knob: Option<String>,
    /// The rating in the knob's units, with its ±1 SD range and the rating going in.
    pub rating: Option<f64>,
    pub rating_low: Option<f64>,
    pub rating_high: Option<f64>,
    pub start_rating: Option<f64>,
    /// Model parameters (log scale), to start the next run from.
    pub theta: Option<f64>,
    pub slope: Option<f64>,
    /// Standard deviation of ln(slope), so the next run knows how well the slope is
    /// known. `None` for runs saved before it was recorded.
    pub slope_sd: Option<f64>,
    pub rating_sd: Option<f64>,
    /// How long the run was, in target-equivalents (1 per target, 3 per tracking
    /// segment). `None` for runs saved before it was recorded and for standard runs.
    pub evidence: Option<f64>,
    /// This run's value if it held the goal rate for the whole run.
    pub fastest: Option<f64>,
    /// The value the whole run was played at, `None` for a calibration run.
    pub run_value: Option<f64>,
    /// The run's pick (`near`, `stretch`, `easy`, or `calibrate`) and its planned chance.
    pub run_pick: Option<String>,
    pub planned_chance: Option<f64>,
    /// Target mix: `steady`, `balanced` or `push`.
    pub mix: Option<String>,
    pub goal: Option<f64>,
    pub success_rate: Option<f64>,
    /// (seconds, rating)
    pub rating_timeline: Vec<(f64, f64)>,
    pub workout: Option<String>,
    /// The scenario's sub-skill (e.g. `static_clicking`).
    pub skill: Option<String>,
    /// Program name and weekday (`mon`…`sun`) when played as part of a program.
    pub program: Option<String>,
    pub program_day: Option<String>,
    /// Rep number within its workout item, the item's rep count, and its block
    /// (`warmup`, `main`, `secondary`, `maintenance`).
    pub rep: Option<i64>,
    pub reps: Option<i64>,
    pub block: Option<String>,
    /// The aim trace, packed by [`encode_trace`].
    pub trace: Vec<u8>,
    /// Where the targets were, for replays (stored packed, see [`encode_frames`]).
    pub frames: Vec<Frame>,
    pub targets: Vec<TargetRow>,
    pub shots: Vec<ShotRow>,
    pub samples: Vec<SampleRow>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TargetRow {
    pub no: i64,
    pub spawned_at: f64,
    pub start_at: f64,
    pub ended_at: f64,
    /// `killed`, `timed_out` or `open`.
    pub outcome: String,
    pub distance_deg: f64,
    pub direction_deg: f64,
    pub radius_deg: f64,
    pub lifetime_s: f64,
    pub reaction_s: Option<f64>,
    pub reach_s: Option<f64>,
    pub overshoot_deg: f64,
    pub entries: i64,
    pub shots: i64,
    pub hits: i64,
    /// Adaptive mode: the knob value this target was given, its planned chance of
    /// success, and the kind of pick (`near`, `stretch`, `easy`, `calibrate`).
    pub value: Option<f64>,
    pub chance: Option<f64>,
    pub pick: Option<String>,
}

/// One target at one moment, for replays. Angles in degrees.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pose {
    pub no: i64,
    pub yaw: f64,
    pub pitch: f64,
    pub radius: f64,
    /// Capsule height; 0 for spheres.
    pub height: f64,
    /// Health left, 0 to 1.
    pub health: f64,
}

/// Every target alive at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    pub t: f64,
    pub targets: Vec<Pose>,
}

/// Format of the packed replay frames, stored as their first byte.
const FRAMES_V1: u8 = 1;
/// Bytes per target in a packed frame.
const POSE_BYTES: usize = 11;

/// Packs replay frames small: per frame the time (f32) and target count (u8), then
/// per target its number (u16), yaw and pitch (i16, 0.01°), radius and height (u16,
/// 0.001°) and health (u8). A frame identical to the one before is left out, except
/// the last of a still stretch, so a replay doesn't blend a sudden change (a hit on a
/// static target) over the whole stretch.
pub fn encode_frames(frames: &[Frame]) -> Vec<u8> {
    fn body(f: &Frame) -> Vec<u8> {
        let q = |v: f64, scale: f64, lo: f64, hi: f64| (v * scale).round().clamp(lo, hi);
        let n = f.targets.len().min(255);
        let mut b = vec![n as u8];
        for p in &f.targets[..n] {
            b.extend((p.no.clamp(0, 65535) as u16).to_le_bytes());
            b.extend((q(p.yaw, 100.0, -32768.0, 32767.0) as i16).to_le_bytes());
            b.extend((q(p.pitch, 100.0, -32768.0, 32767.0) as i16).to_le_bytes());
            b.extend((q(p.radius, 1000.0, 0.0, 65535.0) as u16).to_le_bytes());
            b.extend((q(p.height, 1000.0, 0.0, 65535.0) as u16).to_le_bytes());
            b.push(q(p.health, 255.0, 0.0, 255.0) as u8);
        }
        b
    }
    fn push(out: &mut Vec<u8>, t: f64, b: &[u8]) {
        out.extend((t as f32).to_le_bytes());
        out.extend_from_slice(b);
    }
    let mut out = vec![FRAMES_V1];
    let (mut last, mut held): (Option<Vec<u8>>, Option<f64>) = (None, None);
    for f in frames {
        let b = body(f);
        if last.as_ref() == Some(&b) {
            held = Some(f.t);
            continue;
        }
        if let (Some(t), Some(prev)) = (held.take(), &last) {
            push(&mut out, t, prev);
        }
        push(&mut out, f.t, &b);
        last = Some(b);
    }
    if let (Some(t), Some(prev)) = (held, &last) {
        push(&mut out, t, prev);
    }
    out
}

/// Unpacks [`encode_frames`]. Stops at a frame that's cut short; an unknown format
/// gives no frames.
pub fn decode_frames(bytes: &[u8]) -> Vec<Frame> {
    let mut frames = Vec::new();
    if bytes.first() != Some(&FRAMES_V1) {
        return frames;
    }
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let i16_at = |b: &[u8], i: usize| i16::from_le_bytes([b[i], b[i + 1]]);
    let mut rest = &bytes[1..];
    while rest.len() >= 5 {
        let t = f32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
        let len = usize::from(rest[4]) * POSE_BYTES;
        let Some(body) = rest.get(5..5 + len) else { break };
        let targets = body
            .as_chunks::<POSE_BYTES>()
            .0
            .iter()
            .map(|c| Pose {
                no: i64::from(u16_at(c, 0)),
                yaw: f64::from(i16_at(c, 2)) / 100.0,
                pitch: f64::from(i16_at(c, 4)) / 100.0,
                radius: f64::from(u16_at(c, 6)) / 1000.0,
                height: f64::from(u16_at(c, 8)) / 1000.0,
                health: f64::from(c[10]) / 255.0,
            })
            .collect();
        frames.push(Frame { t: f64::from(t), targets });
        rest = &rest[5 + len..];
    }
    frames
}

/// Starts a packed aim trace (format 2). As a little-endian f32 it's a NaN, which
/// can't start a format 1 trace: that begins with its first sample's time.
const TRACE_V2: [u8; 4] = [0x02, 0x00, 0xC0, 0x7F];
/// Time steps in 0.1 ms; this one says a full time follows (a step too long for u16).
const TRACE_FULL_TIME: u16 = u16::MAX;

/// Packs the aim trace, (seconds, yaw°, pitch°) at 240 Hz, in 6 bytes a sample
/// where format 1 took 12: the time since the previous sample in 0.1 ms (u16),
/// then yaw and pitch in 0.01° (i16). Times are rounded from the start, so the
/// steps don't add up rounding errors.
pub fn encode_trace(trace: &[[f32; 3]]) -> Vec<u8> {
    let mut out = TRACE_V2.to_vec();
    let angle = |v: f32| ((f64::from(v) * 100.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes();
    let mut prev = 0u32;
    for [t, yaw, pitch] in trace {
        let at = (f64::from(*t) * 10_000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32;
        match u16::try_from(at.saturating_sub(prev)) {
            Ok(step) if step != TRACE_FULL_TIME => out.extend(step.to_le_bytes()),
            _ => {
                out.extend(TRACE_FULL_TIME.to_le_bytes());
                out.extend(at.to_le_bytes());
            }
        }
        prev = at;
        out.extend(angle(*yaw));
        out.extend(angle(*pitch));
    }
    out
}

/// Unpacks the aim trace, (seconds, yaw°, pitch°), in either format: 2 from
/// [`encode_trace`], or 1 (before November 2026), little-endian f32 triples.
pub fn decode_trace(bytes: &[u8]) -> Vec<[f32; 3]> {
    let Some(mut rest) = bytes.strip_prefix(&TRACE_V2) else {
        return bytes
            .as_chunks::<12>()
            .0
            .iter()
            .map(|c| {
                let f = |i: usize| f32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]);
                [f(0), f(4), f(8)]
            })
            .collect();
    };
    let mut out = Vec::new();
    let mut at = 0u32;
    let angle = |b: &[u8]| f32::from(i16::from_le_bytes([b[0], b[1]])) / 100.0;
    while rest.len() >= 6 {
        let step = u16::from_le_bytes([rest[0], rest[1]]);
        if step == TRACE_FULL_TIME {
            let Some(b) = rest.get(2..6) else { break };
            at = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            rest = &rest[4..];
            if rest.len() < 6 {
                break;
            }
        } else {
            at += u32::from(step);
        }
        out.push([(f64::from(at) / 10_000.0) as f32, angle(&rest[2..4]), angle(&rest[4..6])]);
        rest = &rest[6..];
    }
    out
}

/// A rating as stored, for starting the next run.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredRating {
    pub knob: String,
    pub rating: f64,
    pub theta: f64,
    pub slope: f64,
    /// `None` for runs saved before it was recorded.
    pub slope_sd: Option<f64>,
    pub sd: f64,
    pub finished_at: String,
    /// That run's pick and success rate, for the next run's guard rails.
    pub run_pick: Option<String>,
    pub success_rate: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShotRow {
    pub t: f64,
    pub hit: bool,
    pub target_no: Option<i64>,
    pub dx_deg: f64,
    pub dy_deg: f64,
    pub along: f64,
    pub across: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SampleRow {
    pub t: f64,
    pub on_target: bool,
    pub error_deg: f64,
    pub target_speed_dps: f64,
    pub lead_deg: f64,
    pub since_change_s: f64,
    /// Some target was damaged and not yet healed.
    pub engaged: bool,
}

pub(crate) const MIGRATION: &str = r#"
CREATE TABLE engine_runs (
    run_id         INTEGER PRIMARY KEY REFERENCES runs (id) ON DELETE CASCADE,
    base_scenario  TEXT NOT NULL,
    category       TEXT NOT NULL,
    adaptive       INTEGER NOT NULL,
    start_level    REAL,
    level          REAL,
    final_level    REAL,
    goal           REAL,
    success_rate   REAL,
    level_timeline TEXT,
    workout        TEXT,
    trace          BLOB
);
CREATE INDEX engine_runs_scenario ON engine_runs (base_scenario, adaptive);

CREATE TABLE engine_targets (
    run_id        INTEGER NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    no            INTEGER NOT NULL,
    spawned_at    REAL NOT NULL,
    start_at      REAL NOT NULL,
    ended_at      REAL NOT NULL,
    outcome       TEXT NOT NULL,
    distance_deg  REAL NOT NULL,
    direction_deg REAL NOT NULL,
    radius_deg    REAL NOT NULL,
    lifetime_s    REAL NOT NULL,
    reaction_s    REAL,
    reach_s       REAL,
    overshoot_deg REAL NOT NULL,
    entries       INTEGER NOT NULL,
    shots         INTEGER NOT NULL,
    hits          INTEGER NOT NULL,
    level         REAL,
    PRIMARY KEY (run_id, no)
);

CREATE TABLE engine_shots (
    run_id    INTEGER NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    t         REAL NOT NULL,
    hit       INTEGER NOT NULL,
    target_no INTEGER,
    dx_deg    REAL NOT NULL,
    dy_deg    REAL NOT NULL,
    along     REAL NOT NULL,
    across    REAL NOT NULL
);
CREATE INDEX engine_shots_run ON engine_shots (run_id);

CREATE TABLE engine_samples (
    run_id           INTEGER NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    t                REAL NOT NULL,
    on_target        INTEGER NOT NULL,
    error_deg        REAL NOT NULL,
    target_speed_dps REAL NOT NULL,
    lead_deg         REAL NOT NULL,
    since_change_s   REAL NOT NULL
);
CREATE INDEX engine_samples_run ON engine_samples (run_id);
"#;

/// Schema v9: how long each run was, in target-equivalents.
pub(crate) const MIGRATION_EVIDENCE: &str = r#"
ALTER TABLE engine_runs ADD COLUMN evidence REAL;
"#;

/// Schema v8: how well the slope is known after a run, in log units.
pub(crate) const MIGRATION_SLOPE_SD: &str = r#"
ALTER TABLE engine_runs ADD COLUMN slope_sd REAL;
"#;

/// Schema v7: replay frames (where every target was, 60 times a second).
pub(crate) const MIGRATION_REPLAY: &str = r#"
ALTER TABLE engine_runs ADD COLUMN frames BLOB;
"#;

/// Schema v6: adaptive runs play at one difficulty after a calibration run.
pub(crate) const MIGRATION_RUN_LEVEL: &str = r#"
ALTER TABLE engine_runs ADD COLUMN run_value REAL;
ALTER TABLE engine_runs ADD COLUMN run_pick TEXT;
ALTER TABLE engine_runs ADD COLUMN planned_chance REAL;
ALTER TABLE engine_samples ADD COLUMN engaged INTEGER NOT NULL DEFAULT 0;
"#;

/// Schema v5: skills, programs and reps.
pub(crate) const MIGRATION_PROGRAMS: &str = r#"
ALTER TABLE engine_runs ADD COLUMN skill TEXT;
ALTER TABLE engine_runs ADD COLUMN program TEXT;
ALTER TABLE engine_runs ADD COLUMN program_day TEXT;
ALTER TABLE engine_runs ADD COLUMN rep INTEGER;
ALTER TABLE engine_runs ADD COLUMN reps INTEGER;
ALTER TABLE engine_runs ADD COLUMN block TEXT;
CREATE INDEX engine_runs_program ON engine_runs (program, program_day);
"#;

/// Schema v4: adaptive ratings in real units replace levels.
pub(crate) const MIGRATION_RATINGS: &str = r#"
ALTER TABLE engine_runs ADD COLUMN knob TEXT;
ALTER TABLE engine_runs ADD COLUMN rating REAL;
ALTER TABLE engine_runs ADD COLUMN rating_low REAL;
ALTER TABLE engine_runs ADD COLUMN rating_high REAL;
ALTER TABLE engine_runs ADD COLUMN start_rating REAL;
ALTER TABLE engine_runs ADD COLUMN theta REAL;
ALTER TABLE engine_runs ADD COLUMN slope REAL;
ALTER TABLE engine_runs ADD COLUMN rating_sd REAL;
ALTER TABLE engine_runs ADD COLUMN fastest REAL;
ALTER TABLE engine_runs ADD COLUMN mix TEXT;
ALTER TABLE engine_targets ADD COLUMN value REAL;
ALTER TABLE engine_targets ADD COLUMN chance REAL;
ALTER TABLE engine_targets ADD COLUMN pick TEXT;
"#;

fn timeline_json(points: &[(f64, f64)]) -> String {
    let inner: Vec<String> = points.iter().map(|(t, l)| format!("[{t:.3},{l:.2}]")).collect();
    format!("[{}]", inner.join(","))
}

pub(crate) fn parse_timeline(text: &str) -> Vec<(f64, f64)> {
    text.trim_matches(|c| c == '[' || c == ']')
        .split("],[")
        .filter_map(|pair| {
            let mut it = pair.split(',').map(|v| v.trim().parse::<f64>());
            Some((it.next()?.ok()?, it.next()?.ok()?))
        })
        .collect()
}

impl Db {
    /// Stores the details of an engine run already saved with `insert_runs`, found
    /// by its file name. Does nothing if that run doesn't exist or already has details.
    pub fn save_engine_run(&mut self, file_name: &str, e: &EngineRun) -> Result<bool, DbError> {
        let tx = self.conn_mut().transaction()?;
        let saved = insert_engine_run(&tx, file_name, e)?;
        tx.commit()?;
        Ok(saved)
    }
}

/// Inserts an engine run's details for the run saved as `file_name`, inside `tx`,
/// and drops the replays its scenario no longer keeps ([`crate::replay::prune`]).
/// Returns false if there's no such run or it already has details.
pub(crate) fn insert_engine_run(tx: &rusqlite::Transaction, file_name: &str, e: &EngineRun) -> Result<bool, DbError> {
    {
        let Some(run_id) =
            tx.query_row("SELECT id FROM runs WHERE file_name = ?1", [file_name], |r| r.get::<_, i64>(0)).optional()?
        else {
            return Ok(false);
        };
        let changed = tx.execute(
            "INSERT OR IGNORE INTO engine_runs (run_id, base_scenario, category, adaptive, goal, success_rate,
                level_timeline, workout, trace, knob, rating, rating_low, rating_high, start_rating, theta, slope,
                rating_sd, fastest, mix, skill, program, program_day, rep, reps, block, run_value, run_pick,
                planned_chance, frames, slope_sd, evidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                     ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31)",
            params![
                run_id,
                e.base_scenario,
                e.category,
                e.adaptive,
                e.goal,
                e.success_rate,
                timeline_json(&e.rating_timeline),
                e.workout,
                e.trace,
                e.knob,
                e.rating,
                e.rating_low,
                e.rating_high,
                e.start_rating,
                e.theta,
                e.slope,
                e.rating_sd,
                e.fastest,
                e.mix,
                e.skill,
                e.program,
                e.program_day,
                e.rep,
                e.reps,
                e.block,
                e.run_value,
                e.run_pick,
                e.planned_chance,
                (!e.frames.is_empty()).then(|| encode_frames(&e.frames)),
                e.slope_sd,
                e.evidence,
            ],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        {
            let mut st = tx.prepare_cached(
                "INSERT INTO engine_targets (run_id, no, spawned_at, start_at, ended_at, outcome, distance_deg,
                    direction_deg, radius_deg, lifetime_s, reaction_s, reach_s, overshoot_deg, entries, shots,
                    hits, value, chance, pick)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            )?;
            for t in &e.targets {
                st.execute(params![
                    run_id,
                    t.no,
                    t.spawned_at,
                    t.start_at,
                    t.ended_at,
                    t.outcome,
                    t.distance_deg,
                    t.direction_deg,
                    t.radius_deg,
                    t.lifetime_s,
                    t.reaction_s,
                    t.reach_s,
                    t.overshoot_deg,
                    t.entries,
                    t.shots,
                    t.hits,
                    t.value,
                    t.chance,
                    t.pick
                ])?;
            }
            let mut st = tx.prepare_cached(
                "INSERT INTO engine_shots (run_id, t, hit, target_no, dx_deg, dy_deg, along, across)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for s in &e.shots {
                st.execute(params![run_id, s.t, s.hit, s.target_no, s.dx_deg, s.dy_deg, s.along, s.across])?;
            }
            let mut st = tx.prepare_cached(
                "INSERT INTO engine_samples (run_id, t, on_target, error_deg, target_speed_dps, lead_deg,
                    since_change_s, engaged)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for s in &e.samples {
                st.execute(params![
                    run_id,
                    s.t,
                    s.on_target,
                    s.error_deg,
                    s.target_speed_dps,
                    s.lead_deg,
                    s.since_change_s,
                    s.engaged
                ])?;
            }
        }
        crate::replay::prune(tx, &e.base_scenario)?;
        Ok(true)
    }
}

impl Db {
    /// When each workout was last played, by name: the finish time of the latest run
    /// saved as part of it. Program days count under their workout's name.
    pub fn workouts_last_played(&self) -> Result<HashMap<String, String>, DbError> {
        let mut stmt = self.conn().prepare(
            "SELECT e.workout, MAX(r.finished_at) FROM engine_runs e JOIN runs r ON r.id = e.run_id
             WHERE e.workout IS NOT NULL GROUP BY e.workout",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The latest adaptive rating for a scenario, for showing.
    pub fn last_rating(&self, base_scenario: &str) -> Result<Option<StoredRating>, DbError> {
        self.last_rating_where(base_scenario, None)
    }

    /// The latest adaptive rating from this exact version of a scenario (its
    /// fingerprint), to start the next run from. A rating earned under other
    /// settings or scoring isn't on the same scale.
    pub fn last_rating_for(&self, base_scenario: &str, hash: &str) -> Result<Option<StoredRating>, DbError> {
        self.last_rating_where(base_scenario, Some(hash))
    }

    fn last_rating_where(&self, base_scenario: &str, hash: Option<&str>) -> Result<Option<StoredRating>, DbError> {
        self.conn()
            .query_row(
                "SELECT e.knob, e.rating, e.theta, e.slope, e.rating_sd, r.finished_at, e.run_pick, e.success_rate,
                        e.slope_sd
                 FROM engine_runs e JOIN runs r ON r.id = e.run_id
                 WHERE e.base_scenario = ?1 AND e.adaptive = 1 AND e.rating IS NOT NULL AND e.theta IS NOT NULL
                   AND (?2 IS NULL OR r.scenario_hash = ?2)
                 ORDER BY r.finished_at DESC LIMIT 1",
                rusqlite::params![base_scenario, hash],
                |r| {
                    Ok(StoredRating {
                        knob: r.get(0)?,
                        rating: r.get(1)?,
                        theta: r.get(2)?,
                        slope: r.get(3)?,
                        sd: r.get(4)?,
                        finished_at: r.get(5)?,
                        run_pick: r.get(6)?,
                        success_rate: r.get(7)?,
                        slope_sd: r.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::from)
    }

    /// The hardest value the goal rate was held at for a whole run on a scenario
    /// with this knob: the shortest lifetime or smallest size, or the highest
    /// tracking speed. Runs from before fixed-difficulty runs (schema v6) stored
    /// single-target bests instead, so they're left out.
    /// With `hash`, only runs of that exact version of the scenario count.
    pub fn fastest_hit(&self, base_scenario: &str, knob: &str, hash: Option<&str>) -> Result<Option<f64>, DbError> {
        let agg = if crate::knob::Knob::from_id(knob).higher_is_harder() { "MAX" } else { "MIN" };
        Ok(self.conn().query_row(
            &format!(
                "SELECT {agg}(e.fastest) FROM engine_runs e JOIN runs r ON r.id = e.run_id
                 WHERE e.base_scenario = ?1 AND e.knob = ?2 AND e.run_value IS NOT NULL
                   AND (?3 IS NULL OR r.scenario_hash = ?3)"
            ),
            rusqlite::params![base_scenario, knob, hash],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(no: i64, yaw: f64, health: f64) -> Pose {
        Pose { no, yaw, pitch: -2.5, radius: 0.75, height: 0.0, health }
    }

    #[test]
    fn replay_frames_round_trip_to_their_precision() {
        let frames = vec![
            Frame { t: 0.0, targets: vec![pose(1, -12.345, 1.0), Pose { height: 4.2, ..pose(2, 30.0, 0.5) }] },
            Frame { t: 1.0 / 60.0, targets: vec![] },
        ];
        let back = decode_frames(&encode_frames(&frames));
        assert_eq!(back.len(), 2);
        let p = &back[0].targets[0];
        assert_eq!(p.no, 1);
        assert!((p.yaw + 12.345).abs() < 0.006 && (p.pitch + 2.5).abs() < 1e-9 && (p.radius - 0.75).abs() < 1e-9);
        let q = &back[0].targets[1];
        assert!((q.height - 4.2).abs() < 1e-9 && (q.health - 0.5).abs() < 0.003, "{q:?}");
        assert!(back[1].targets.is_empty() && (back[1].t - 1.0 / 60.0).abs() < 1e-6);
        assert!(decode_frames(&[]).is_empty() && decode_frames(&[9, 0, 0]).is_empty(), "unknown format");
    }

    #[test]
    fn still_frames_are_dropped_but_changes_stay_sharp() {
        // A static target, hit at 1 s: frames 0 to 60 are all the same.
        let frames: Vec<Frame> = (0..=90)
            .map(|i| Frame { t: i as f64 / 60.0, targets: vec![pose(1, 5.0, if i <= 60 { 1.0 } else { 0.5 })] })
            .collect();
        let back = decode_frames(&encode_frames(&frames));
        let frame_no: Vec<f64> = back.iter().map(|f| (f.t * 60.0).round()).collect();
        // The first frame, the last still one before the hit, the hit, and the end.
        assert_eq!(frame_no, vec![0.0, 60.0, 61.0, 90.0]);
        assert_eq!(back[1].targets[0].health, 1.0, "still full health right up to the hit");
    }

    /// Traces saved before the packed format still play.
    #[test]
    fn old_traces_unpack_as_three_floats_per_sample() {
        let bytes: Vec<u8> = [0.5f32, -10.0, 2.25].iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(decode_trace(&bytes), vec![[0.5, -10.0, 2.25]]);
    }

    /// The aim trace took 12 bytes a sample (173 KB a minute) with no format
    /// marker; it's now 6, marked, and round-trips within its rounding.
    #[test]
    fn traces_pack_to_six_bytes_a_sample_and_round_trip() {
        let trace: Vec<[f32; 3]> =
            (0..14_400).map(|i| [i as f32 / 240.0 + 0.0003, (i as f32 * 0.37).sin() * 170.0, -12.345]).collect();
        let bytes = encode_trace(&trace);
        assert_eq!(bytes.len(), 4 + 6 * trace.len());
        let back = decode_trace(&bytes);
        assert_eq!(back.len(), trace.len());
        for (a, b) in trace.iter().zip(&back) {
            assert!((a[0] - b[0]).abs() <= 0.000_06, "{a:?} {b:?}");
            assert!((a[1] - b[1]).abs() <= 0.0051 && (a[2] - b[2]).abs() <= 0.0051, "{a:?} {b:?}");
        }
        // A gap longer than a u16 of 0.1 ms steps (6.5 s) keeps its time.
        let gap = [[0.0, 1.0, 2.0], [10.0, 3.0, 4.0], [10.004, 5.0, 6.0]];
        let back = decode_trace(&encode_trace(&gap));
        assert_eq!(back.iter().map(|s| (s[0] * 1000.0).round()).collect::<Vec<_>>(), vec![0.0, 10_000.0, 10_004.0]);
        // A cut-off trace gives the samples before the cut.
        let bytes = encode_trace(&trace[..3]);
        assert_eq!(decode_trace(&bytes[..bytes.len() - 1]).len(), 2);
    }

    #[test]
    fn workouts_remember_when_they_were_last_played() {
        use crate::parse::RunFile;
        use chrono::NaiveDate;
        use std::collections::BTreeMap;
        let mut db = Db::open_in_memory().unwrap();
        let mut save = |n: u32, workout: Option<&str>| {
            let at = NaiveDate::from_ymd_opt(2026, 11, n).unwrap().and_hms_opt(7, 0, 0).unwrap();
            let mut fields = BTreeMap::new();
            fields.insert("Source".to_string(), "rustyaim".to_string());
            let name = format!("rustyaim/test/{n}");
            let run = RunFile {
                file_name: name.clone(),
                scenario: "Timed Flicks".into(),
                mode: "RustyAim".into(),
                finished_at: at,
                fields,
                kills: vec![],
                weapons: vec![],
            };
            db.insert_runs(&[run], at).unwrap();
            let e = EngineRun {
                base_scenario: "Timed Flicks".into(),
                category: "clicking".into(),
                workout: workout.map(String::from),
                ..Default::default()
            };
            assert!(db.save_engine_run(&name, &e).unwrap());
        };
        save(2, Some("Quick Warm-up"));
        save(4, Some("Quick Warm-up"));
        save(5, Some("Core Week: Thursday"));
        save(6, None);
        let last = db.workouts_last_played().unwrap();
        assert_eq!(last.len(), 2, "runs outside a workout don't count: {last:?}");
        assert!(last["Quick Warm-up"].starts_with("2026-11-04"), "the latest of its runs: {last:?}");
        assert!(last["Core Week: Thursday"].starts_with("2026-11-05"));
    }

    /// Saves a one-run adaptive history named `n`, with this slope uncertainty.
    fn save_rated(db: &mut Db, n: u32, slope_sd: Option<f64>) {
        use crate::parse::RunFile;
        use chrono::NaiveDate;
        use std::collections::BTreeMap;
        let at = NaiveDate::from_ymd_opt(2026, 11, n).unwrap().and_hms_opt(7, 0, 0).unwrap();
        let name = format!("rustyaim/test/{n}");
        let run = RunFile {
            file_name: name.clone(),
            scenario: "Timed Flicks".into(),
            mode: "RustyAim".into(),
            finished_at: at,
            fields: BTreeMap::from([
                ("Source".to_string(), "rustyaim".to_string()),
                ("Hash".to_string(), "abc123".to_string()),
            ]),
            kills: vec![],
            weapons: vec![],
        };
        db.insert_runs(&[run], at).unwrap();
        let e = EngineRun {
            base_scenario: "Timed Flicks".into(),
            category: "clicking".into(),
            adaptive: true,
            knob: Some("lifetime".into()),
            rating: Some(0.6),
            theta: Some(0.7),
            slope: Some(0.12),
            slope_sd,
            rating_sd: Some(0.05),
            ..Default::default()
        };
        assert!(db.save_engine_run(&name, &e).unwrap());
    }

    /// A1 (steadier ratings): the slope's uncertainty is saved with each run. The
    /// column is new, so a run saved without it reads back as `None` and the rating
    /// falls back to its old fixed width; a run saved with it reads back as saved.
    #[test]
    fn slope_sd_is_saved_and_old_runs_read_back_without_it() {
        let mut db = Db::open_in_memory().unwrap();
        let columns: Vec<String> = db
            .conn()
            .prepare("SELECT name FROM pragma_table_info('engine_runs')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(columns.iter().any(|c| c == "slope_sd"), "{columns:?}");
        save_rated(&mut db, 2, None);
        assert_eq!(db.last_rating("Timed Flicks").unwrap().unwrap().slope_sd, None);
        save_rated(&mut db, 3, Some(0.21));
        assert_eq!(db.last_rating("Timed Flicks").unwrap().unwrap().slope_sd, Some(0.21));
    }

    #[test]
    fn slope_sd_comes_back_through_last_rating_for() {
        let mut db = Db::open_in_memory().unwrap();
        save_rated(&mut db, 4, Some(0.18));
        let stored = db.last_rating_for("Timed Flicks", "abc123").unwrap().expect("matches the fingerprint");
        assert_eq!(stored.slope_sd, Some(0.18));
        assert!(db.last_rating_for("Timed Flicks", "other").unwrap().is_none());
    }

    #[test]
    fn timeline_round_trips() {
        let points = vec![(0.0, 25.0), (1.5, 31.25), (60.0, 47.5)];
        assert_eq!(parse_timeline(&timeline_json(&points)), points);
        assert!(parse_timeline("[]").is_empty());
    }
}
