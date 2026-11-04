//! Detailed records for runs played in RustyAim's own trainer: one row per target,
//! per shot and per tracking sample, plus the adaptive rating and its history and the
//! aim trace. KovaaK's runs have none of this; [`crate::insights`] builds analytics on it.

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
    pub rating_sd: Option<f64>,
    /// Hardest value succeeded at this run.
    pub fastest: Option<f64>,
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
    /// Little-endian f32 triples (seconds, yaw°, pitch°).
    pub trace: Vec<u8>,
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

/// A rating as stored, for starting the next run.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredRating {
    pub knob: String,
    pub rating: f64,
    pub theta: f64,
    pub slope: f64,
    pub sd: f64,
    pub finished_at: String,
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
        let Some(run_id) =
            tx.query_row("SELECT id FROM runs WHERE file_name = ?1", [file_name], |r| r.get::<_, i64>(0)).optional()?
        else {
            return Ok(false);
        };
        let changed = tx.execute(
            "INSERT OR IGNORE INTO engine_runs (run_id, base_scenario, category, adaptive, goal, success_rate,
                level_timeline, workout, trace, knob, rating, rating_low, rating_high, start_rating, theta, slope,
                rating_sd, fastest, mix, skill, program, program_day, rep, reps, block)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                     ?21, ?22, ?23, ?24, ?25)",
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
                    since_change_s)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for s in &e.samples {
                st.execute(params![
                    run_id,
                    s.t,
                    s.on_target,
                    s.error_deg,
                    s.target_speed_dps,
                    s.lead_deg,
                    s.since_change_s
                ])?;
            }
        }
        tx.commit()?;
        Ok(true)
    }

    /// The latest adaptive rating for a scenario, to start the next run from.
    pub fn last_rating(&self, base_scenario: &str) -> Result<Option<StoredRating>, DbError> {
        self.conn()
            .query_row(
                "SELECT e.knob, e.rating, e.theta, e.slope, e.rating_sd, r.finished_at
                 FROM engine_runs e JOIN runs r ON r.id = e.run_id
                 WHERE e.base_scenario = ?1 AND e.adaptive = 1 AND e.rating IS NOT NULL AND e.theta IS NOT NULL
                 ORDER BY r.finished_at DESC LIMIT 1",
                [base_scenario],
                |r| {
                    Ok(StoredRating {
                        knob: r.get(0)?,
                        rating: r.get(1)?,
                        theta: r.get(2)?,
                        slope: r.get(3)?,
                        sd: r.get(4)?,
                        finished_at: r.get(5)?,
                    })
                },
            )
            .optional()
    }

    /// The hardest value ever succeeded at on a scenario with this knob: the
    /// shortest lifetime or smallest size, or the highest tracking speed.
    pub fn fastest_hit(&self, base_scenario: &str, knob: &str) -> Result<Option<f64>, DbError> {
        let agg = if knob == "target_speed" { "MAX" } else { "MIN" };
        self.conn().query_row(
            &format!("SELECT {agg}(fastest) FROM engine_runs WHERE base_scenario = ?1 AND knob = ?2"),
            [base_scenario, knob],
            |r| r.get(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_round_trips() {
        let points = vec![(0.0, 25.0), (1.5, 31.25), (60.0, 47.5)];
        assert_eq!(parse_timeline(&timeline_json(&points)), points);
        assert!(parse_timeline("[]").is_empty());
    }
}
