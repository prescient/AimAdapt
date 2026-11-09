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
];

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
