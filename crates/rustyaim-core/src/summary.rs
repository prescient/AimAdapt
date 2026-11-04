//! Simple totals over imported runs. Session grouping and active-vs-session time
//! come in Epic 2; this only sums run lengths.

use rusqlite::params;

use crate::db::{Db, DbError};

#[derive(Debug, Clone, PartialEq)]
pub struct Totals {
    pub runs: i64,
    pub scenarios: i64,
    pub first: Option<String>,
    pub last: Option<String>,
    pub run_seconds: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DayTotal {
    /// `YYYY-MM-DD`, by the run's finish time.
    pub day: String,
    pub runs: i64,
    pub run_seconds: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioTotal {
    pub scenario: String,
    pub runs: i64,
    pub best: Option<f64>,
    pub first_score: Option<f64>,
}

pub fn totals(db: &Db) -> Result<Totals, DbError> {
    db.conn().query_row(
        "SELECT COUNT(*), COUNT(DISTINCT scenario), MIN(finished_at), MAX(finished_at),
                COALESCE(SUM(duration_s), 0)
         FROM runs",
        [],
        |r| {
            Ok(Totals { runs: r.get(0)?, scenarios: r.get(1)?, first: r.get(2)?, last: r.get(3)?, run_seconds: r.get(4)? })
        },
    )
}

pub fn by_day(db: &Db) -> Result<Vec<DayTotal>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT substr(finished_at, 1, 10) AS day, COUNT(*), COALESCE(SUM(duration_s), 0)
         FROM runs GROUP BY day ORDER BY day",
    )?;
    stmt.query_map([], |r| Ok(DayTotal { day: r.get(0)?, runs: r.get(1)?, run_seconds: r.get(2)? }))?
        .collect()
}

pub fn top_scenarios(db: &Db, limit: usize) -> Result<Vec<ScenarioTotal>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT scenario, COUNT(*) AS n, MAX(score),
                (SELECT score FROM runs r2 WHERE r2.scenario = runs.scenario ORDER BY finished_at LIMIT 1)
         FROM runs GROUP BY scenario ORDER BY n DESC, scenario LIMIT ?1",
    )?;
    stmt.query_map(params![limit as i64], |r| {
        Ok(ScenarioTotal { scenario: r.get(0)?, runs: r.get(1)?, best: r.get(2)?, first_score: r.get(3)? })
    })?
    .collect()
}
