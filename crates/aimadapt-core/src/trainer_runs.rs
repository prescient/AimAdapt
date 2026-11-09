//! Queries over runs played in AimAdapt's own trainer, for the app's lists and the
//! trainer's results screen. They live here, beside the schema, so a migration that
//! breaks one fails a test in this crate.

use std::collections::HashMap;

use rusqlite::OptionalExtension;

use crate::db::{Db, DbError, MODE_IMPORTED, SOURCE_ENGINE};

/// How much a scenario has been played: runs, best score, last finish time.
pub type Played = (i64, Option<f64>, Option<String>);

/// Frame rate and resolution from the latest run, KovaaK's or AimAdapt's.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct GameInfo {
    pub avg_fps: Option<f64>,
    pub max_fps: Option<f64>,
    pub resolution: Option<String>,
    pub game_version: Option<String>,
}

impl Db {
    /// Trainer runs by scenario name and fingerprint.
    pub fn trainer_played(&self) -> Result<HashMap<(String, String), Played>, DbError> {
        let mut stmt = self.conn().prepare(
            "SELECT scenario, scenario_hash, COUNT(*), MAX(score), MAX(finished_at) FROM runs
             WHERE source = ?1 GROUP BY scenario, scenario_hash",
        )?;
        let rows = stmt.query_map([SOURCE_ENGINE], |r| {
            Ok(((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default()), (r.get(2)?, r.get(3)?, r.get(4)?)))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Trainer runs of a scenario imported from a KovaaK's file.
    pub fn imported_played(&self, scenario: &str) -> Result<Played, DbError> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*), MAX(score), MAX(finished_at) FROM runs WHERE source = ?1 AND mode = ?2 AND scenario = ?3",
            (SOURCE_ENGINE, MODE_IMPORTED, scenario),
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?)
    }

    /// Best standard score of a trainer scenario with this fingerprint.
    pub fn best_trainer_score(&self, scenario: &str, hash: &str) -> Result<Option<f64>, DbError> {
        Ok(self.conn().query_row(
            "SELECT MAX(score) FROM runs WHERE source = ?1 AND scenario = ?2 AND scenario_hash = ?3",
            (SOURCE_ENGINE, scenario, hash),
            |r| r.get(0),
        )?)
    }

    /// Best adaptive rating on a scenario with this knob: the highest when a higher
    /// value is harder, otherwise the lowest.
    pub fn best_trainer_rating(
        &self,
        scenario: &str,
        knob: &str,
        higher_is_harder: bool,
    ) -> Result<Option<f64>, DbError> {
        let agg = if higher_is_harder { "MAX" } else { "MIN" };
        Ok(self.conn().query_row(
            &format!("SELECT {agg}(rating) FROM engine_runs WHERE base_scenario = ?1 AND knob = ?2"),
            [scenario, knob],
            |r| r.get(0),
        )?)
    }

    /// Frame rate and resolution of the latest run.
    pub fn latest_game_info(&self) -> Result<Option<GameInfo>, DbError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT avg_fps, max_fps, resolution, game_version FROM runs ORDER BY finished_at DESC LIMIT 1",
                [],
                |r| {
                    Ok(GameInfo {
                        avg_fps: r.get(0)?,
                        max_fps: r.get(1)?,
                        resolution: r.get(2)?,
                        game_version: r.get(3)?,
                    })
                },
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These ran from the app and the engine, outside this crate, where a schema
    /// change could break them unnoticed. On an empty database each one runs.
    #[test]
    fn every_query_runs_on_the_current_schema() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.trainer_played().unwrap().is_empty());
        assert_eq!(db.imported_played("x").unwrap(), (0, None, None));
        assert_eq!(db.best_trainer_score("x", "h").unwrap(), None);
        assert_eq!(db.best_trainer_rating("x", "lifetime", false).unwrap(), None);
        assert_eq!(db.latest_game_info().unwrap(), None);
    }
}
