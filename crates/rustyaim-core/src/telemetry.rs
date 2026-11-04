//! Detailed records for runs played in RustyAim's own trainer: one row per target,
//! per shot and per tracking sample, plus the adaptive level history and the aim
//! trace. KovaaK's runs have none of this; [`crate::insights`] builds analytics on it.

use rusqlite::{OptionalExtension, params};

use crate::db::{Db, DbError};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngineRun {
    /// Scenario name without the " (Adaptive)" suffix, so both modes group together.
    pub base_scenario: String,
    pub category: String,
    pub adaptive: bool,
    pub start_level: Option<f64>,
    pub level: Option<f64>,
    pub final_level: Option<f64>,
    pub goal: Option<f64>,
    pub success_rate: Option<f64>,
    /// (seconds, level)
    pub level_timeline: Vec<(f64, f64)>,
    pub workout: Option<String>,
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
    pub level: Option<f64>,
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
            "INSERT OR IGNORE INTO engine_runs (run_id, base_scenario, category, adaptive, start_level, level,
                final_level, goal, success_rate, level_timeline, workout, trace)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                run_id,
                e.base_scenario,
                e.category,
                e.adaptive,
                e.start_level,
                e.level,
                e.final_level,
                e.goal,
                e.success_rate,
                timeline_json(&e.level_timeline),
                e.workout,
                e.trace,
            ],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        {
            let mut st = tx.prepare_cached(
                "INSERT INTO engine_targets (run_id, no, spawned_at, start_at, ended_at, outcome, distance_deg,
                    direction_deg, radius_deg, lifetime_s, reaction_s, reach_s, overshoot_deg, entries, shots,
                    hits, level)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
                    t.level
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

    /// The level the player settled at in their latest adaptive run of a scenario.
    pub fn last_adaptive_level(&self, base_scenario: &str) -> Result<Option<f64>, DbError> {
        self.conn()
            .query_row(
                "SELECT e.level FROM engine_runs e JOIN runs r ON r.id = e.run_id
                 WHERE e.base_scenario = ?1 AND e.adaptive = 1 AND e.level IS NOT NULL
                 ORDER BY r.finished_at DESC LIMIT 1",
                [base_scenario],
                |r| r.get(0),
            )
            .optional()
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
