//! SQLite storage. Times are stored as local-time ISO text (`2026-11-03T06:31:39.000`),
//! matching what KovaaK's writes; KovaaK's records no time zone.

use std::collections::HashSet;
use std::path::Path;

use chrono::NaiveDateTime;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::parse::RunFile;

pub use rusqlite::Error as DbError;

const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f";

/// Each entry upgrades the schema by one version (tracked in `PRAGMA user_version`).
/// Never edit an entry once released; add a new one.
const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE runs (
    id                INTEGER PRIMARY KEY,
    file_name         TEXT NOT NULL UNIQUE,
    scenario          TEXT NOT NULL,
    mode              TEXT NOT NULL,
    started_at        TEXT,
    finished_at       TEXT NOT NULL,
    duration_s        REAL,
    score             REAL,
    accuracy          REAL,
    kills             INTEGER,
    deaths            INTEGER,
    hits              INTEGER,
    misses            INTEGER,
    avg_ttk_s         REAL,
    damage_done       REAL,
    damage_taken      REAL,
    pause_count       INTEGER,
    pause_duration    REAL,
    scenario_hash     TEXT,
    game_version      TEXT,
    sens_scale        TEXT,
    horiz_sens        REAL,
    vert_sens         REAL,
    dpi               INTEGER,
    fov               REAL,
    fov_scale         TEXT,
    max_fps           REAL,
    avg_fps           REAL,
    resolution        TEXT,
    avg_target_scale  REAL,
    avg_time_dilation REAL,
    imported_at       TEXT NOT NULL
);
CREATE INDEX runs_scenario ON runs (scenario, finished_at);
CREATE INDEX runs_finished ON runs (finished_at);

-- Every `Key:,value` line from the file, so fields we don't model yet aren't lost.
CREATE TABLE run_fields (
    run_id INTEGER NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    key    TEXT NOT NULL,
    value  TEXT NOT NULL,
    PRIMARY KEY (run_id, key)
);

CREATE TABLE kills (
    run_id          INTEGER NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    kill_no         INTEGER NOT NULL,
    timestamp       TEXT,
    bot             TEXT,
    weapon          TEXT,
    ttk_s           REAL,
    shots           INTEGER,
    hits            INTEGER,
    damage_done     REAL,
    damage_possible REAL,
    cheated         INTEGER,
    overshots       INTEGER,
    PRIMARY KEY (run_id, kill_no)
);

CREATE TABLE weapons (
    run_id          INTEGER NOT NULL REFERENCES runs (id) ON DELETE CASCADE,
    weapon          TEXT NOT NULL,
    shots           INTEGER,
    hits            INTEGER,
    damage_done     REAL,
    damage_possible REAL
);

CREATE TABLE import_errors (
    file_name TEXT PRIMARY KEY,
    error     TEXT NOT NULL,
    seen_at   TEXT NOT NULL
);
"#];

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, DbError> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn schema_version(&self) -> Result<u32, DbError> {
        self.conn.pragma_query_value(None, "user_version", |r| r.get(0))
    }

    pub fn known_file_names(&self) -> Result<HashSet<String>, DbError> {
        let mut stmt = self.conn.prepare("SELECT file_name FROM runs")?;
        let names = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        Ok(names)
    }

    /// Inserts runs in one transaction. Runs already in the database are skipped.
    /// Returns how many were actually inserted.
    pub fn insert_runs(&mut self, runs: &[RunFile], imported_at: NaiveDateTime) -> Result<usize, DbError> {
        let tx = self.conn.transaction()?;
        let mut inserted = 0;
        for run in runs {
            if insert_run(&tx, run, imported_at)? {
                inserted += 1;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    pub fn record_error(&self, file_name: &str, error: &str, seen_at: NaiveDateTime) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO import_errors (file_name, error, seen_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (file_name) DO UPDATE SET error = excluded.error, seen_at = excluded.seen_at",
            params![file_name, error, fmt(seen_at)],
        )?;
        Ok(())
    }

    pub fn clear_error(&self, file_name: &str) -> Result<(), DbError> {
        self.conn.execute("DELETE FROM import_errors WHERE file_name = ?1", [file_name])?;
        Ok(())
    }

    pub fn import_errors(&self) -> Result<Vec<(String, String)>, DbError> {
        let mut stmt = self.conn.prepare("SELECT file_name, error FROM import_errors ORDER BY file_name")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn last_finished_at(&self) -> Result<Option<String>, DbError> {
        self.conn
            .query_row("SELECT MAX(finished_at) FROM runs", [], |r| r.get(0))
            .optional()
            .map(Option::flatten)
    }
}

fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in (0i64..).zip(MIGRATIONS).skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i + 1)?;
        tx.commit()?;
    }
    Ok(())
}

fn fmt(t: NaiveDateTime) -> String {
    t.format(TIME_FORMAT).to_string()
}

fn insert_run(tx: &Transaction, run: &RunFile, imported_at: NaiveDateTime) -> Result<bool, DbError> {
    let changed = tx.execute(
        "INSERT OR IGNORE INTO runs (
            file_name, scenario, mode, started_at, finished_at, duration_s, score, accuracy,
            kills, deaths, hits, misses, avg_ttk_s, damage_done, damage_taken, pause_count,
            pause_duration, scenario_hash, game_version, sens_scale, horiz_sens, vert_sens, dpi,
            fov, fov_scale, max_fps, avg_fps, resolution, avg_target_scale, avg_time_dilation,
            imported_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                  ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31)",
        params![
            run.file_name,
            // The file name is the source of truth; `Scenario:` should match it.
            run.scenario,
            run.mode,
            run.started_at().map(fmt),
            fmt(run.finished_at),
            run.duration_s(),
            run.score(),
            run.accuracy(),
            run.field_i64("Kills"),
            run.field_i64("Deaths"),
            run.field_i64("Hit Count"),
            run.field_i64("Miss Count"),
            run.field_f64("Avg TTK"),
            run.field_f64("Damage Done"),
            run.field_f64("Damage Taken"),
            run.field_i64("Pause Count"),
            run.field_f64("Pause Duration"),
            run.field("Hash"),
            run.field("Game Version"),
            run.field("Sens Scale"),
            run.field_f64("Horiz Sens"),
            run.field_f64("Vert Sens"),
            run.field_i64("DPI"),
            run.field_f64("FOV"),
            run.field("FOVScale"),
            run.field_f64("Max FPS (config)"),
            run.field_f64("Avg FPS"),
            run.field("Resolution"),
            run.field_f64("Avg Target Scale"),
            run.field_f64("Avg Time Dilation"),
            fmt(imported_at),
        ],
    )?;
    if changed == 0 {
        return Ok(false);
    }
    let run_id = tx.last_insert_rowid();

    let mut field = tx.prepare_cached("INSERT INTO run_fields (run_id, key, value) VALUES (?1, ?2, ?3)")?;
    for (key, value) in &run.fields {
        field.execute(params![run_id, key, value])?;
    }

    let mut kill = tx.prepare_cached(
        "INSERT OR IGNORE INTO kills (run_id, kill_no, timestamp, bot, weapon, ttk_s, shots, hits,
            damage_done, damage_possible, cheated, overshots)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    for k in &run.kills {
        kill.execute(params![
            run_id, k.kill_no, k.timestamp, k.bot, k.weapon, k.ttk_s, k.shots, k.hits, k.damage_done,
            k.damage_possible, k.cheated, k.overshots
        ])?;
    }

    let mut weapon = tx.prepare_cached(
        "INSERT INTO weapons (run_id, weapon, shots, hits, damage_done, damage_possible)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for w in &run.weapons {
        weapon.execute(params![run_id, w.weapon, w.shots, w.hits, w.damage_done, w.damage_possible])?;
    }
    Ok(true)
}
