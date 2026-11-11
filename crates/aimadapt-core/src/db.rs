//! SQLite storage. Times are stored as local-time ISO text (`2026-11-03T06:31:39.000`),
//! matching what KovaaK's writes; KovaaK's records no time zone.

use std::collections::HashSet;
use std::path::Path;

use chrono::NaiveDateTime;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::parse::RunFile;

/// Why a database call failed.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    /// The database was written by a newer AimAdapt, whose tables this one doesn't
    /// know. Opening it anyway could store runs the newer version can't read.
    #[error(
        "this history was saved by a newer version of AimAdapt (database version {found}, this version knows up to {known}). Update AimAdapt to open it."
    )]
    Newer { found: i64, known: usize },
}

/// How times are stored in the database: local time with milliseconds.
pub(crate) const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f";

/// Each entry upgrades the schema by one version (tracked in `PRAGMA user_version`).
/// Never edit an entry once released; add a new one.
const MIGRATIONS: &[&str] = &[
    r#"
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
"#,
    r#"
-- Where a run came from: 'kovaaks' (imported stats file) or 'rustyaim' (built-in engine).
ALTER TABLE runs ADD COLUMN source TEXT NOT NULL DEFAULT 'kovaaks';
CREATE INDEX runs_source ON runs (source, scenario);
"#,
    crate::telemetry::MIGRATION,
    crate::telemetry::MIGRATION_RATINGS,
    crate::telemetry::MIGRATION_PROGRAMS,
    crate::telemetry::MIGRATION_RUN_LEVEL,
    crate::telemetry::MIGRATION_REPLAY,
    crate::telemetry::MIGRATION_SLOPE_SD,
    crate::telemetry::MIGRATION_EVIDENCE,
    crate::slips::MIGRATION,
];

/// Steps in Rust that run right after the SQL of the given schema version, in the same
/// transaction (a migration that has to do arithmetic the database can't).
const RUST_STEPS: &[(usize, MigrationStep)] = &[(10, crate::slips::convert_ratings)];

type MigrationStep = fn(&Transaction) -> Result<(), DbError>;

/// Value of the `Source` field (and `runs.source`) for runs played in AimAdapt's
/// engine. Stored in every run, so it keeps the app's old name (RustyAim).
pub const SOURCE_ENGINE: &str = "rustyaim";

/// `runs.source` of runs imported from KovaaK's stats files.
pub const SOURCE_KOVAAKS: &str = "kovaaks";

/// `runs.mode` of trainer runs (KovaaK's uses "Challenge"). They keep the app's
/// old name so runs recorded before the rename stay in the same history.
pub const MODE_STANDARD: &str = "RustyAim";
pub const MODE_ADAPTIVE: &str = "RustyAim Adaptive";

/// `runs.mode` of trainer runs of a scenario imported from a KovaaK's `.sce` file.
/// They're kept apart from KovaaK's own runs of the same name (Progress leaves them
/// out). Named like the other stored values, with the app's old name.
pub const MODE_IMPORTED: &str = "RustyAim Imported";

/// The data folder and database file, and their names before the app was renamed
/// from RustyAim.
const DATA_DIR: &str = "AimAdapt";
const OLD_DATA_DIR: &str = "RustyAim";
const DB_FILE: &str = "aimadapt.db";
const OLD_DB_FILE: &str = "rustyaim.db";

/// `%APPDATA%\AimAdapt\aimadapt.db`, creating the folder if needed. Shared by the CLI and app.
pub fn default_path() -> std::io::Result<std::path::PathBuf> {
    Ok(db_in(&data_dir()?))
}

/// The database in a data folder. A database from before the rename is renamed to
/// the new file name, unless it wasn't closed cleanly (a `-wal` file holds changes
/// not yet in it): then it's used where it is, under its old name.
fn db_in(dir: &std::path::Path) -> std::path::PathBuf {
    let (new, old) = (dir.join(DB_FILE), dir.join(OLD_DB_FILE));
    if new.exists() || !old.exists() {
        return new;
    }
    let side = |suffix: &str| dir.join(format!("{OLD_DB_FILE}{suffix}"));
    if side("-wal").exists() || std::fs::rename(&old, &new).is_err() {
        return old;
    }
    // A leftover shared-memory file only matters alongside a -wal file.
    let _ = std::fs::remove_file(side("-shm"));
    new
}

/// Options that exist for development and testing (`--dump-snapshot`, `--smoke`)
/// must name their own database with `--db`. Run from a packaged tool such as an
/// agent's shell, a write under `%APPDATA%` lands in that tool's private copy,
/// which then hides the real database from it; and bot runs don't belong in a
/// player's history anyway.
pub fn require_test_db(args: &[String], dev_flags: &[&str]) -> Result<(), String> {
    let has = |f: &str| args.iter().any(|a| a == f);
    match dev_flags.iter().find(|f| has(f)) {
        Some(flag) if !has("--db") => {
            Err(format!("{flag} only runs against a test database: add --db <file>. It never uses your real history."))
        }
        _ => Ok(()),
    }
}

/// `%APPDATA%\AimAdapt`, created if needed. Holds the database, engine settings and
/// the player's own scenarios and workouts.
pub fn data_dir() -> std::io::Result<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA isn't set"))?;
    data_dir_in(&base)
}

/// The data folder inside `base`. On the first run after the rename, the old
/// `RustyAim` folder is moved to `AimAdapt` with everything in it. If it can't be
/// moved (an old copy of the app still has it open), it's used where it is and
/// moved on a later run.
pub fn data_dir_in(base: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let (dir, old) = (base.join(DATA_DIR), base.join(OLD_DATA_DIR));
    if !dir.exists() && old.is_dir() && std::fs::rename(&old, &dir).is_err() {
        return Ok(old);
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Sensitivity and FOV from a run, as KovaaK's records them.
#[derive(Debug, Clone, PartialEq)]
pub struct AimSettings {
    pub sens_scale: String,
    pub horiz_sens: f64,
    pub dpi: i64,
    pub fov: Option<f64>,
    pub fov_scale: Option<String>,
}

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

    /// The SQLite connection, for this crate's queries. Other crates use the named
    /// queries, so a schema change can't break SQL outside this crate; their tests
    /// get it with the `test-util` feature.
    #[cfg(not(feature = "test-util"))]
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    #[cfg(feature = "test-util")]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub(crate) fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn schema_version(&self) -> Result<u32, DbError> {
        Ok(self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    pub fn known_file_names(&self) -> Result<HashSet<String>, DbError> {
        let mut stmt = self.conn.prepare("SELECT file_name FROM runs")?;
        let names = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        Ok(names)
    }

    /// The id of the run stored under `file_name`, if there is one.
    pub fn run_id(&self, file_name: &str) -> Result<Option<i64>, DbError> {
        Ok(self.conn.query_row("SELECT id FROM runs WHERE file_name = ?1", [file_name], |r| r.get(0)).optional()?)
    }

    /// Saves a trainer run and its details in one transaction, so a failure part
    /// way leaves neither. Returns false if the run was already saved.
    pub fn save_trainer_run(
        &mut self,
        run: &RunFile,
        e: &crate::telemetry::EngineRun,
        imported_at: NaiveDateTime,
    ) -> Result<bool, DbError> {
        let tx = self.conn.transaction()?;
        let saved = insert_run(&tx, run, imported_at)? && crate::telemetry::insert_engine_run(&tx, &run.file_name, e)?;
        tx.commit()?;
        Ok(saved)
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

    /// Settings from the most recent KovaaK's run that recorded them, used as the
    /// starting point for the built-in engine's sensitivity and FOV.
    pub fn latest_kovaaks_settings(&self) -> Result<Option<AimSettings>, DbError> {
        self.conn
            .query_row(
                "SELECT sens_scale, horiz_sens, dpi, fov, fov_scale FROM runs
                 WHERE source = ?1 AND sens_scale IS NOT NULL AND horiz_sens IS NOT NULL
                   AND dpi IS NOT NULL
                 ORDER BY finished_at DESC LIMIT 1",
                [SOURCE_KOVAAKS],
                |r| {
                    Ok(AimSettings {
                        sens_scale: r.get(0)?,
                        horiz_sens: r.get(1)?,
                        dpi: r.get(2)?,
                        fov: r.get(3)?,
                        fov_scale: r.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn last_finished_at(&self) -> Result<Option<String>, DbError> {
        Ok(self.conn.query_row("SELECT MAX(finished_at) FROM runs", [], |r| r.get(0)).optional()?.flatten())
    }
}

fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > MIGRATIONS.len() as i64 {
        return Err(DbError::Newer { found: current, known: MIGRATIONS.len() });
    }
    for (i, sql) in (0i64..).zip(MIGRATIONS).skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        for (version, step) in RUST_STEPS {
            if *version as i64 == i + 1 {
                step(&tx)?;
            }
        }
        tx.pragma_update(None, "user_version", i + 1)?;
        tx.commit()?;
    }
    Ok(())
}

fn fmt(t: NaiveDateTime) -> String {
    t.format(TIME_FORMAT).to_string()
}

pub(crate) fn insert_run(tx: &Transaction, run: &RunFile, imported_at: NaiveDateTime) -> Result<bool, DbError> {
    let changed = tx.execute(
        "INSERT OR IGNORE INTO runs (
            file_name, scenario, mode, started_at, finished_at, duration_s, score, accuracy,
            kills, deaths, hits, misses, avg_ttk_s, damage_done, damage_taken, pause_count,
            pause_duration, scenario_hash, game_version, sens_scale, horiz_sens, vert_sens, dpi,
            fov, fov_scale, max_fps, avg_fps, resolution, avg_target_scale, avg_time_dilation,
            imported_at, source
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                  ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32)",
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
            run.field("Source").unwrap_or(SOURCE_KOVAAKS),
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
            run_id,
            k.kill_no,
            k.timestamp,
            k.bot,
            k.weapon,
            k.ttk_s,
            k.shots,
            k.hits,
            k.damage_done,
            k.damage_possible,
            k.cheated,
            k.overshots
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A saved run is found by its file name, and a name that was never saved is not.
    #[test]
    fn a_runs_id_is_found_by_file_name() {
        let mut db = Db::open_in_memory().unwrap();
        let at = chrono::NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(8, 0, 0).unwrap();
        let run = RunFile {
            file_name: "rustyaim/test/1".into(),
            scenario: "Timed Flicks".into(),
            mode: "RustyAim".into(),
            finished_at: at,
            fields: Default::default(),
            kills: vec![],
            weapons: vec![],
        };
        db.insert_runs(&[run], at).unwrap();
        let id = db.run_id("rustyaim/test/1").unwrap().expect("found");
        assert!(id > 0);
        assert_eq!(db.run_id("rustyaim/test/2").unwrap(), None);
    }

    /// Bug: a database from a newer AimAdapt opened without complaint, so an older
    /// build wrote runs into tables it didn't know.
    #[test]
    fn a_newer_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        Db::open(&path).unwrap();
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1).unwrap();
        drop(conn);
        let err = Db::open(&path).err().expect("refused");
        assert!(matches!(err, DbError::Newer { .. }), "{err}");
        assert!(err.to_string().contains("newer version of AimAdapt"), "{err}");
    }

    /// The eighth migration (plan A1) adds the slope's uncertainty to engine runs. A
    /// database saved by the version before it must open, keep its runs, and read an
    /// old rating back with no slope uncertainty.
    #[test]
    fn a_version_7_database_upgrades_and_keeps_its_ratings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..7] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 7).unwrap();
        conn.execute_batch(
            "INSERT INTO runs (id, file_name, scenario, mode, finished_at, imported_at)
             VALUES (1, 'aimadapt/test/1', 'Timed Flicks', 'AimAdapt',
                     '2026-11-01T07:00:00.000', '2026-11-01T07:00:00.000');
             INSERT INTO engine_runs (run_id, base_scenario, category, adaptive, knob, rating, theta, slope, rating_sd)
             VALUES (1, 'Timed Flicks', 'clicking', 1, 'lifetime', 0.6, 0.7, 0.12, 0.05);",
        )
        .unwrap();
        drop(conn);

        let db = Db::open(&path).unwrap();
        assert!(db.schema_version().unwrap() >= 8);
        let stored = db.last_rating("Timed Flicks").unwrap().expect("the old run is still there");
        // This row has no goal, so the version 10 conversion (which needs one) leaves it as it was.
        assert_eq!((stored.rating, stored.theta, stored.slope, stored.sd), (0.6, 0.7, 0.12, 0.05));
        assert_eq!(stored.slope_sd, None, "an old run has no slope uncertainty");
    }

    /// The ninth migration (plan A6) adds each run's length to engine runs. A database
    /// saved by the version before it must open, keep its runs, and read an old run's
    /// length back as none.
    #[test]
    fn a_version_8_database_upgrades_and_keeps_its_runs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..8] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 8).unwrap();
        conn.execute_batch(
            "INSERT INTO runs (id, file_name, scenario, mode, finished_at, imported_at)
             VALUES (1, 'aimadapt/test/1', 'Timed Flicks', 'AimAdapt',
                     '2026-11-01T07:00:00.000', '2026-11-01T07:00:00.000');
             INSERT INTO engine_runs (run_id, base_scenario, category, adaptive, knob, rating, theta, slope, rating_sd,
                                      run_pick, planned_chance, success_rate)
             VALUES (1, 'Timed Flicks', 'clicking', 1, 'lifetime', 0.6, 0.7, 0.12, 0.05, 'near', 0.85, 0.8);",
        )
        .unwrap();
        drop(conn);

        let db = Db::open(&path).unwrap();
        assert!(db.schema_version().unwrap() >= 9);
        let stored = db.last_rating("Timed Flicks").unwrap().expect("the old run is still there");
        assert_eq!(stored.rating, 0.6);
        let evidence: Option<f64> =
            db.conn().query_row("SELECT evidence FROM engine_runs WHERE run_id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(evidence, None, "an old run has no recorded length");
    }

    /// One engine run of a version 9 database, as the old code stored it. `h_goal` is
    /// the unclamped goal point on the log scale (bigger is harder).
    struct OldRun {
        knob: &'static str,
        goal: f64,
        slope: f64,
        h_goal: f64,
        sd: Option<f64>,
        /// The stored rating, when the run was held at a limit instead of at `h_goal`.
        held_at: Option<f64>,
        start: Option<f64>,
        timeline: Option<&'static str>,
        theta: Option<f64>,
    }

    impl OldRun {
        /// The rating as stored: at the limit when held there, else at the goal point.
        fn rating(&self) -> f64 {
            self.held_at.unwrap_or_else(|| value_of(self.knob, self.h_goal))
        }

        fn lifetime(h_goal: f64) -> OldRun {
            OldRun {
                knob: "lifetime",
                goal: 0.85,
                slope: 0.12,
                h_goal,
                sd: None,
                held_at: None,
                start: None,
                timeline: None,
                theta: None,
            }
        }
    }

    /// The knob value at log difficulty `h`, written out here on purpose (see `Knob::value`).
    fn value_of(knob: &str, h: f64) -> f64 {
        if knob == "target_speed" { h.exp() } else { (-h).exp() }
    }

    fn h_of(knob: &str, v: f64) -> f64 {
        if knob == "target_speed" { v.ln() } else { -v.ln() }
    }

    /// The logit of the success chance with slips counted, written out here on purpose.
    fn logit_counting_slips(goal: f64) -> f64 {
        let q = (goal - 0.02) / 0.96;
        (q / (1.0 - q)).ln()
    }

    /// A timeline as the migration writes it: `dh` easier, two decimals, except points
    /// at `limit` (when there is one), which stay.
    fn shifted_timeline(knob: &str, dh: f64, limit: Option<f64>, old: &[(f64, f64)]) -> String {
        let pts: Vec<String> = old
            .iter()
            .map(|&(t, v)| {
                let at_limit = limit.is_some_and(|l| (v - l).abs() < 0.005 + 1e-9);
                format!("[{t:.3},{:.2}]", if at_limit { v } else { value_of(knob, h_of(knob, v) - dh) })
            })
            .collect();
        format!("[{}]", pts.join(","))
    }

    /// The runs of [`version_9_runs`] that have field rows.
    const WITH_FIELDS: [i64; 4] = [1, 2, 3, 8];

    fn rounded(v: f64, places: usize) -> f64 {
        format!("{v:.places$}").parse().unwrap()
    }

    fn field_texts(conn: &Connection, id: i64) -> Vec<(String, String)> {
        let mut stmt = conn.prepare("SELECT key, value FROM run_fields WHERE run_id = ?1 ORDER BY key").unwrap();
        stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
    }

    type Stored = (f64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<String>);

    fn stored(conn: &Connection, table: &str, id: i64) -> Stored {
        conn.query_row(
            &format!(
                "SELECT rating, rating_low, rating_high, start_rating, theta, level_timeline FROM {table}
                 WHERE run_id = ?1"
            ),
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .unwrap()
    }

    fn near(a: f64, b: f64, what: &str) {
        assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{what}: {a} vs {b}");
    }

    /// Version 9 rows, as the old code stored them: runs 1 to 8 except 5, which is a
    /// standard run with no rating.
    fn version_9_runs() -> Vec<(i64, OldRun)> {
        let limit = 0.25; // a lifetime limit: h = ln 4 = 1.3863
        vec![
            // Lifetime, with a range, a start, a timeline and a theta.
            (
                1,
                OldRun {
                    sd: Some(0.05),
                    start: Some(0.9),
                    timeline: Some("[[0.000,0.90],[30.000,0.70],[60.000,0.61]]"),
                    theta: Some(0.7),
                    ..OldRun::lifetime(0.5)
                },
            ),
            // Target speed, another goal and slope.
            (
                2,
                OldRun {
                    knob: "target_speed",
                    goal: 0.8,
                    slope: 0.2,
                    sd: Some(0.04),
                    start: Some(42.0),
                    timeline: Some("[[0.000,42.00],[60.000,50.00]]"),
                    theta: Some(4.0),
                    ..OldRun::lifetime(50f64.ln())
                },
            ),
            // Held at the hardest limit, with the real point well past it.
            (
                3,
                OldRun {
                    sd: Some(0.1),
                    held_at: Some(limit),
                    start: Some(limit),
                    timeline: Some("[[0.000,0.25],[30.000,0.40],[60.000,0.25]]"),
                    theta: Some(1.7),
                    ..OldRun::lifetime(1.5)
                },
            ),
            // Held at the limit, but the new goal point falls just inside it.
            (
                4,
                OldRun {
                    sd: Some(0.1),
                    held_at: Some(limit),
                    start: Some(0.4),
                    timeline: Some("[[0.000,0.40],[60.000,0.25]]"),
                    theta: Some(1.6),
                    ..OldRun::lifetime(1.40)
                },
            ),
            // Held at the easiest limit (1.5 s), with the real point easier still.
            (
                8,
                OldRun {
                    sd: Some(0.1),
                    held_at: Some(1.5),
                    start: Some(1.5),
                    timeline: Some("[[0.000,1.20],[60.000,1.50]]"),
                    theta: Some(-0.4),
                    ..OldRun::lifetime(-0.6)
                },
            ),
            // No range, start, theta or timeline.
            (6, OldRun::lifetime(0.2)),
            // An empty timeline.
            (7, OldRun { timeline: Some(""), theta: Some(0.5), ..OldRun::lifetime(0.3) }),
        ]
    }

    /// The tenth migration makes stored ratings count slips: the goal point moves to where
    /// the chance of success, slips included, is the goal, and the stored theta puts the
    /// curve through it. A version 9 database must keep a copy of what it had, convert
    /// each rating once, and leave a rating that was held at a scenario limit there.
    #[test]
    fn a_version_9_database_converts_its_ratings_to_count_slips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..9] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 9).unwrap();
        let old_runs = version_9_runs();
        for (id, o) in &old_runs {
            conn.execute(
                "INSERT INTO runs (id, file_name, scenario, mode, finished_at, imported_at, score)
                 VALUES (?1, ?2, 'Timed Flicks', ?3, '2026-11-01T07:00:00.000', '2026-11-01T07:00:00.000', ?4)",
                params![id, format!("aimadapt/test/{id}"), MODE_ADAPTIVE, rounded(o.rating(), 3)],
            )
            .unwrap();
            // Some runs also keep the rating as text fields, the way the trainer saves it; the
            // others have no field rows.
            if WITH_FIELDS.contains(id) {
                let start = o.start.unwrap_or(0.0);
                let fields = [
                    ("Score", format!("{:.3}", o.rating())),
                    ("Rating", format!("{:.4}", o.rating())),
                    ("Start Rating", format!("{start:.4}")),
                    ("Run Value", "0.5000".to_string()),
                ];
                for (key, v) in fields {
                    let sql = "INSERT INTO run_fields (run_id, key, value) VALUES (?1, ?2, ?3)";
                    conn.execute(sql, params![id, key, v]).unwrap();
                }
            }
            let rating = o.rating();
            let low = o.sd.map(|sd| value_of(o.knob, o.h_goal - sd));
            let high = o.sd.map(|sd| value_of(o.knob, o.h_goal + sd));
            conn.execute(
                "INSERT INTO engine_runs (run_id, base_scenario, category, adaptive, knob, goal, rating, rating_low,
                                          rating_high, start_rating, theta, slope, rating_sd, level_timeline)
                 VALUES (?1, 'Timed Flicks', 'clicking', 1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![id, o.knob, o.goal, rating, low, high, o.start, o.theta, o.slope, o.sd, o.timeline],
            )
            .unwrap();
        }
        // A standard run has no rating at all.
        conn.execute_batch(
            "INSERT INTO runs (id, file_name, scenario, mode, finished_at, imported_at)
             VALUES (5, 'aimadapt/test/5', 'Standard', 'RustyAim', '2026-11-01T07:00:00.000', '2026-11-01T07:00:00.000');
             UPDATE runs SET score = 7.5 WHERE id = 5;
             INSERT INTO run_fields (run_id, key, value) VALUES (5, 'Score', '7.500'), (5, 'Rating', '0.6000');
             INSERT INTO engine_runs (run_id, base_scenario, category, adaptive, goal, level_timeline)
             VALUES (5, 'Standard', 'clicking', 0, 0.85, '[[0.000,1.00]]');",
        )
        .unwrap();
        let before: Vec<Stored> = old_runs.iter().map(|(id, _)| stored(&conn, "engine_runs", *id)).collect();
        drop(conn);

        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), 10);
        for ((id, o), old) in old_runs.iter().zip(&before) {
            let dh = o.slope * (logit_counting_slips(o.goal) - (o.goal / (1.0 - o.goal)).ln());
            let got = stored(db.conn(), "engine_runs", *id);
            assert_eq!(&stored(db.conn(), "engine_ratings_v9", *id), old, "run {id}: the copy keeps the old values");

            // A held rating stays at its limit unless the real point has moved inside it.
            let new_h = o.h_goal - dh;
            let want = o.held_at.map_or(new_h, |l| {
                let lh = h_of(o.knob, l);
                if o.h_goal > lh { new_h.min(lh) } else { new_h.max(lh) }
            });
            near(got.0, value_of(o.knob, want), &format!("run {id} rating"));
            let (low, high) = (o.sd.map(|sd| o.h_goal - sd - dh), o.sd.map(|sd| o.h_goal + sd - dh));
            near(got.1.unwrap_or(0.0), low.map_or(0.0, |h| value_of(o.knob, h)), &format!("run {id} low"));
            near(got.2.unwrap_or(0.0), high.map_or(0.0, |h| value_of(o.knob, h)), &format!("run {id} high"));
            let start_stays = o.held_at.is_some_and(|l| o.start.is_some_and(|s| (s - l).abs() < 1e-9));
            let want_start = o.start.map(|s| if start_stays { s } else { value_of(o.knob, h_of(o.knob, s) - dh) });
            near(got.3.unwrap_or(0.0), want_start.unwrap_or(0.0), &format!("run {id} start"));
            assert_eq!(got.3.is_some(), o.start.is_some());
            // The new theta is the new goal point plus slope * logit(goal with slips).
            assert_eq!(got.4.is_some(), o.theta.is_some(), "run {id} theta");
            if let Some(t) = got.4 {
                near(t, o.h_goal + o.slope * (o.goal / (1.0 - o.goal)).ln(), &format!("run {id} theta"));
                if o.held_at.is_none() {
                    let through = value_of(o.knob, t - o.slope * crate::insights::logit_with_slips(o.goal));
                    near(through, got.0, &format!("run {id} curve through the rating"));
                }
            }
            let want_timeline = match o.timeline {
                Some(text) if !text.is_empty() => {
                    Some(shifted_timeline(o.knob, dh, o.held_at, &crate::telemetry::parse_timeline(text)))
                }
                text => text.map(str::to_string),
            };
            // The score and the text fields hold the rating too: the new one, rounded as the trainer
            // saves it. Run Value is a played value and stays.
            let score_of = |table: &str| -> f64 {
                let sql =
                    format!("SELECT score FROM {table} WHERE {} = ?1", if table == "runs" { "id" } else { "run_id" });
                db.conn().query_row(&sql, [id], |r| r.get(0)).unwrap()
            };
            assert_eq!(score_of("runs"), rounded(got.0, 3), "run {id} score");
            assert_eq!(score_of("engine_ratings_v9"), rounded(o.rating(), 3), "run {id} copy keeps the score");
            let fields = field_texts(db.conn(), *id);
            if WITH_FIELDS.contains(id) {
                let start = want_start.unwrap();
                let want = [
                    ("Rating".to_string(), format!("{:.4}", got.0)),
                    ("Run Value".to_string(), "0.5000".to_string()),
                    ("Score".to_string(), format!("{:.3}", got.0)),
                    ("Start Rating".to_string(), format!("{start:.4}")),
                ];
                assert_eq!(fields, want, "run {id} fields");
            } else {
                assert!(fields.is_empty(), "run {id} has no field rows");
            }
            assert_eq!(got.5, want_timeline, "run {id} timeline");
        }
        // The standard run is untouched and not copied.
        let (timeline, copied): (String, i64) = db
            .conn()
            .query_row(
                "SELECT level_timeline, (SELECT COUNT(*) FROM engine_ratings_v9 WHERE run_id = 5) FROM engine_runs
                 WHERE run_id = 5",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((timeline.as_str(), copied), ("[[0.000,1.00]]", 0));
        let score: f64 = db.conn().query_row("SELECT score FROM runs WHERE id = 5", [], |r| r.get(0)).unwrap();
        assert_eq!(score, 7.5, "a standard run's score is a game score");
        assert_eq!(
            field_texts(db.conn(), 5),
            [("Rating".to_string(), "0.6000".to_string()), ("Score".to_string(), "7.500".to_string())]
        );
        let copies: i64 = db.conn().query_row("SELECT COUNT(*) FROM engine_ratings_v9", [], |r| r.get(0)).unwrap();
        assert_eq!(copies, old_runs.len() as i64, "every rated run is copied");

        // Opening it again doesn't convert a second time.
        let after: Vec<Stored> = old_runs.iter().map(|(id, _)| stored(db.conn(), "engine_runs", *id)).collect();
        drop(db);
        let db = Db::open(&path).unwrap();
        let again: Vec<Stored> = old_runs.iter().map(|(id, _)| stored(db.conn(), "engine_runs", *id)).collect();
        assert_eq!(after, again);
    }

    fn put(p: &std::path::Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    /// The rename from RustyAim: the first run moves the old data folder, with the
    /// database, settings and the player's own files, and renames the database.
    #[test]
    fn data_from_before_the_rename_moves_to_the_new_folder() {
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join("RustyAim");
        put(&old.join("rustyaim.db"), "history");
        put(&old.join("engine.toml"), "profiles");
        put(&old.join("scenarios").join("mine.toml"), "scenario");
        let dir = data_dir_in(base.path()).unwrap();
        assert_eq!(dir, base.path().join("AimAdapt"));
        assert!(!old.exists(), "moved, not copied");
        assert_eq!(std::fs::read_to_string(dir.join("engine.toml")).unwrap(), "profiles");
        assert_eq!(std::fs::read_to_string(dir.join("scenarios").join("mine.toml")).unwrap(), "scenario");
        let db = db_in(&dir);
        assert_eq!(db, dir.join("aimadapt.db"));
        assert_eq!(std::fs::read_to_string(&db).unwrap(), "history");
        // Later runs find everything where it now is.
        assert_eq!(data_dir_in(base.path()).unwrap(), dir);
        assert_eq!(db_in(&dir), db);
    }

    #[test]
    fn a_new_install_starts_in_the_new_folder() {
        let base = tempfile::tempdir().unwrap();
        let dir = data_dir_in(base.path()).unwrap();
        assert_eq!(dir, base.path().join("AimAdapt"));
        assert!(dir.is_dir());
        assert_eq!(db_in(&dir), dir.join("aimadapt.db"));
    }

    /// If both folders exist (say an old copy of the app ran after the move), the new
    /// one wins and the old one is left alone.
    #[test]
    fn the_new_folder_wins_over_an_old_one() {
        let base = tempfile::tempdir().unwrap();
        put(&base.path().join("AimAdapt").join("aimadapt.db"), "new");
        put(&base.path().join("RustyAim").join("rustyaim.db"), "old");
        let dir = data_dir_in(base.path()).unwrap();
        assert_eq!(std::fs::read_to_string(db_in(&dir)).unwrap(), "new");
        assert!(base.path().join("RustyAim").join("rustyaim.db").exists());
    }

    /// A database that wasn't closed cleanly has changes in its -wal file, which
    /// SQLite only finds next to the file it belongs to: keep its old name.
    #[test]
    fn a_database_with_pending_changes_keeps_its_old_name() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("AimAdapt");
        put(&dir.join("rustyaim.db"), "history");
        put(&dir.join("rustyaim.db-wal"), "pending");
        assert_eq!(db_in(&dir), dir.join("rustyaim.db"));
        assert!(dir.join("rustyaim.db-wal").exists());
        // Once it's been closed cleanly, it's renamed.
        std::fs::remove_file(dir.join("rustyaim.db-wal")).unwrap();
        assert_eq!(db_in(&dir), dir.join("aimadapt.db"));
        assert!(!dir.join("rustyaim.db").exists());
    }
}
