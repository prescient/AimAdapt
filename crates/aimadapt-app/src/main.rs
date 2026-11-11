//! AimAdapt desktop app: a Tauri shell over `aimadapt-core`. The UI in `ui/` asks
//! for a snapshot of everything it shows, and the folder watcher tells it when new
//! runs arrive so it can refresh.
//!
//! Run with `--play <scenario id>`, `--workout <id>` or `--program <id>`, the same
//! executable is the aim trainer instead (see `aimadapt-engine`); the Play and
//! Program screens start it that way.

// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use aimadapt_core::progress::{
    self, ColdStart, FatigueBucket, ProgressSettings, ScenarioRun, ScenarioStats, SensPeriod,
};
use aimadapt_core::training::{self, TimeSettings, TrainingReport};
use aimadapt_core::{Db, ImportReport, discover, import};
use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

mod marks;
mod trainer;

pub(crate) struct AppData {
    db: Mutex<Db>,
    /// Why the database couldn't be opened. The window still opens and shows it
    /// (`snapshot` fails with it), where a panic would show nothing at all.
    startup_error: Option<String>,
    db_path: PathBuf,
    stats_dir: Option<PathBuf>,
    last_import: Mutex<Option<ImportReport>>,
    /// The daily goal can change while the app runs (Settings › Training).
    time: Mutex<TimeSettings>,
    progress: ProgressSettings,
}

impl AppData {
    /// A copy of the time settings. A poisoned lock still gives them.
    pub(crate) fn time(&self) -> TimeSettings {
        self.time.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    pub(crate) fn set_daily_goal(&self, minutes: f64) {
        self.time.lock().unwrap_or_else(std::sync::PoisonError::into_inner).daily_goal_min = minutes;
    }
}

/// The time settings at startup: the defaults, with the daily goal saved on Settings › Training.
fn time_settings(data_dir: &std::path::Path) -> TimeSettings {
    TimeSettings { daily_goal_min: aimadapt_sim::prefs::load(data_dir).daily_goal_min, ..TimeSettings::default() }
}

/// Everything the screens need in one call.
#[derive(Serialize)]
struct Snapshot {
    db_path: String,
    stats_dir: Option<String>,
    last_import: Option<ImportReport>,
    import_errors: Vec<(String, String)>,
    time_settings: TimeSettings,
    progress_settings: ProgressSettings,
    training: TrainingReport,
    scenarios: Vec<ScenarioStats>,
    sens_periods: Vec<SensPeriod>,
    cold_starts: Vec<ColdStart>,
    fatigue: Vec<FatigueBucket>,
    game: Option<aimadapt_core::trainer_runs::GameInfo>,
}

#[derive(Serialize)]
struct ScenarioDetail {
    stats: ScenarioStats,
    history: Vec<ScenarioRun>,
    cold_starts: Vec<ColdStart>,
}

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command(async)]
fn snapshot(state: State<'_, AppData>) -> Result<Snapshot, String> {
    build_snapshot(&state)
}

#[tauri::command(async)]
fn scenario(state: State<'_, AppData>, name: String) -> Result<Option<ScenarioDetail>, String> {
    build_scenario(&state, &name)
}

fn build_snapshot(state: &AppData) -> Result<Snapshot, String> {
    if let Some(e) = &state.startup_error {
        return Err(e.clone());
    }
    let db = state.db.lock().map_err(err)?;
    let time = state.time();
    let training = training::report(&db, &time, Local::now().naive_local()).map_err(err)?;
    let p = progress::load(&db, &time).map_err(err)?;
    let game = db.latest_game_info().map_err(err)?;
    Ok(Snapshot {
        db_path: state.db_path.display().to_string(),
        stats_dir: state.stats_dir.as_ref().map(|d| d.display().to_string()),
        last_import: state.last_import.lock().map_err(err)?.clone(),
        import_errors: db.import_errors().map_err(err)?,
        time_settings: time,
        progress_settings: state.progress.clone(),
        training,
        scenarios: p.stats(&state.progress),
        sens_periods: p.sens_periods.clone(),
        cold_starts: p.cold_starts(),
        fatigue: p.fatigue(&state.progress),
        game,
    })
}

fn build_scenario(state: &AppData, name: &str) -> Result<Option<ScenarioDetail>, String> {
    let db = state.db.lock().map_err(err)?;
    let p = progress::load(&db, &state.time()).map_err(err)?;
    Ok(scenario_detail(&p, &state.progress, name))
}

fn scenario_detail(p: &progress::Progress, settings: &ProgressSettings, name: &str) -> Option<ScenarioDetail> {
    Some(ScenarioDetail {
        stats: p.stats_for(name, settings)?,
        history: p.history(name).into_iter().cloned().collect(),
        cold_starts: p.cold_starts().into_iter().filter(|c| c.scenario == name).collect(),
    })
}

/// `aimadapt-app --dump-snapshot <file>` writes what the UI would receive (the snapshot
/// plus every scenario's detail) as JSON and exits. Lets the UI be previewed in a
/// browser against real data: `ui/app.js` loads `dev/dev-snapshot.json` when not in
/// Tauri. Never write it into `ui/`, which is built into the exe (`build.rs` refuses).
fn dump_snapshot(state: &AppData, path: &std::path::Path) -> Result<(), String> {
    let snapshot = build_snapshot(state)?;
    let db = state.db.lock().map_err(err)?;
    let p = progress::load(&db, &state.time()).map_err(err)?;
    let scenarios: std::collections::BTreeMap<String, ScenarioDetail> = p
        .scenarios()
        .into_iter()
        .filter_map(|name| scenario_detail(&p, &state.progress, &name).map(|d| (name, d)))
        .collect();
    drop(db);
    let t = trainer::TrainerState {
        data_dir: aimadapt_core::db::data_dir().map_err(err)?,
        playing: std::sync::atomic::AtomicBool::new(false),
    };
    let trainer = trainer::build_trainer(state, &t)?;
    let program = trainer::build_program(state, &t)?;
    let db = state.db.lock().map_err(err)?;
    let overview = aimadapt_core::insights::overview(&db).map_err(err)?;
    let last_session = aimadapt_core::programs::recap::last_session(&db, &state.time()).map_err(err)?;
    let mut insights = std::collections::BTreeMap::new();
    // The latest replay of each scenario, so the preview can show one.
    let mut replays = std::collections::BTreeMap::new();
    for o in &overview {
        if let Some(i) = aimadapt_core::insights::scenario(&db, &o.scenario).map_err(err)? {
            if let Some(r) = i.runs.iter().rev().find(|r| r.replay)
                && let Some(replay) = aimadapt_core::replay::replay(&db, r.run_id).map_err(err)?
            {
                replays.insert(r.run_id.to_string(), replay);
            }
            insights.insert(o.scenario.clone(), i);
        }
    }
    drop(db);
    #[derive(Serialize)]
    struct Dump<'a> {
        snapshot: &'a Snapshot,
        scenarios: &'a std::collections::BTreeMap<String, ScenarioDetail>,
        trainer: &'a trainer::Trainer,
        program: &'a trainer::ProgramPage,
        insights_overview: &'a [aimadapt_core::insights::ScenarioSummary],
        insights: &'a std::collections::BTreeMap<String, aimadapt_core::insights::Insights>,
        replays: &'a std::collections::BTreeMap<String, aimadapt_core::replay::Replay>,
        last_session: &'a Option<aimadapt_core::programs::recap::SessionRecap>,
    }
    let dump = Dump {
        snapshot: &snapshot,
        scenarios: &scenarios,
        trainer: &trainer,
        program: &program,
        insights_overview: &overview,
        insights: &insights,
        replays: &replays,
        last_session: &last_session,
    };
    let json = serde_json::to_string(&dump).map_err(err)?;
    std::fs::write(path, json).map_err(err)
}

#[tauri::command(async)]
fn rescan(state: State<'_, AppData>) -> Result<ImportReport, String> {
    import_now(&state)
}

fn import_now(state: &AppData) -> Result<ImportReport, String> {
    let dir = state.stats_dir.as_ref().ok_or("KovaaK's stats folder wasn't found")?;
    let mut db = state.db.lock().map_err(err)?;
    let report = import::import_dir(&mut db, dir).map_err(err)?;
    *state.last_import.lock().map_err(err)? = Some(report.clone());
    Ok(report)
}

/// Imports new runs as KovaaK's writes them and tells the UI to refresh.
fn watch_stats(app: AppHandle, dir: PathBuf) {
    use notify::{RecursiveMode, Watcher};
    let (tx, rx) = mpsc::channel();
    let Ok(mut watcher) = notify::recommended_watcher(tx) else { return };
    if watcher.watch(&dir, RecursiveMode::NonRecursive).is_err() {
        return;
    }
    while let Ok(event) = rx.recv() {
        let touches_stats = event.is_ok_and(|e| e.paths.iter().any(|p| import::is_stats_file(p)));
        if !touches_stats {
            continue;
        }
        // KovaaK's may still be writing the file; wait for events to settle.
        while rx.recv_timeout(Duration::from_millis(750)).is_ok() {}
        let state = app.state::<AppData>();
        if let Ok(report) = import_now(&state)
            && report.imported > 0
        {
            let _ = app.emit("runs-imported", report);
        }
    }
}

/// Whether these arguments start the trainer rather than the app. Every way the
/// app launches the trainer must be listed here, or the launch opens a second app
/// window instead of the game.
fn is_trainer_launch(args: &[String]) -> bool {
    args.iter().any(|a| matches!(a.as_str(), "--play" | "--workout" | "--program" | "--sce"))
}

/// Development options that must come with `--db`.
const DEV_FLAGS: &[&str] = &["--dump-snapshot"];

/// The database to use: `--db <file>`, or the one in the data folder.
fn open_database(args: &[String]) -> (PathBuf, Result<Db, String>) {
    let path = match args.iter().position(|a| a == "--db").and_then(|i| args.get(i + 1)) {
        Some(p) => PathBuf::from(p),
        None => match aimadapt_core::db::default_path() {
            Ok(p) => p,
            Err(e) => return (PathBuf::new(), Err(format!("couldn't find a folder for AimAdapt's data: {e}"))),
        },
    };
    let db = Db::open(&path).map_err(|e| format!("couldn't open {}: {e}", path.display()));
    (path, db)
}

/// Writes panics to `crash.log` in the data folder (release builds have no
/// console), and in the app, says so in a message box if the window never opened.
fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let dir = aimadapt_core::db::data_dir().unwrap_or_else(|_| std::env::temp_dir());
        let path = dir.join("crash.log");
        let text = format!("{} {info}\n", Local::now().format("%Y-%m-%d %H:%M:%S"));
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| {
            use std::io::Write;
            f.write_all(text.as_bytes())
        });
        #[cfg(all(windows, not(debug_assertions)))]
        if !is_trainer_launch(&std::env::args().collect::<Vec<_>>()) {
            show_crash(&format!("AimAdapt stopped: {info}\n\nDetails are in {}", path.display()));
        }
    }));
}

/// A message box without a console or another dependency: PowerShell's, with the
/// text passed in an environment variable so nothing in it is run.
#[cfg(all(windows, not(debug_assertions)))]
fn show_crash(text: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show($env:AIMADAPT_CRASH, 'AimAdapt')",
        ])
        .env("AIMADAPT_CRASH", text)
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

fn main() {
    log_panics();
    let args: Vec<String> = std::env::args().collect();
    // Trainer mode: no app window, no import, just the game.
    if is_trainer_launch(&args) {
        std::process::exit(aimadapt_engine::run_from_args(&args[1..]));
    }

    if let Err(e) = aimadapt_core::db::require_test_db(&args, DEV_FLAGS) {
        eprintln!("aimadapt: {e}");
        std::process::exit(2);
    }
    let no_import = args.iter().any(|a| a == "--no-import");
    // `--db <file>` uses another database, e.g. a test one.
    let (db_path, opened) = open_database(&args);
    let (db, startup_error) = match opened {
        Ok(db) => (db, None),
        Err(e) => (Db::open_in_memory().expect("an in-memory database"), Some(e)),
    };
    // Without a data folder the database didn't open either, and the app only shows that.
    let data_dir = aimadapt_core::db::data_dir().unwrap_or_default();
    let data = AppData {
        db: Mutex::new(db),
        startup_error: startup_error.clone(),
        db_path,
        // `--no-import` leaves KovaaK's out entirely (demo and test databases), and
        // nothing is imported into a database that didn't open.
        stats_dir: if no_import || startup_error.is_some() { None } else { discover::find_stats_dir() },
        last_import: Mutex::new(None),
        time: Mutex::new(time_settings(&data_dir)),
        progress: ProgressSettings::default(),
    };
    // Catch up on runs played while the app was closed before the first screen loads.
    let _ = import_now(&data);

    if let Some(i) = args.iter().position(|a| a == "--dump-snapshot") {
        let result = match args.get(i + 1) {
            Some(path) => dump_snapshot(&data, std::path::Path::new(path)),
            None => Err("--dump-snapshot needs a file path".into()),
        };
        if let Err(e) = result {
            eprintln!("aimadapt: {e}");
            std::process::exit(1);
        }
        return;
    }

    let watch_dir = data.stats_dir.clone();
    let trainer_state = trainer::TrainerState { data_dir, playing: std::sync::atomic::AtomicBool::new(false) };

    tauri::Builder::default()
        .manage(data)
        .manage(trainer_state)
        .setup(move |app| {
            if let Some(dir) = watch_dir {
                let handle = app.handle().clone();
                std::thread::spawn(move || watch_stats(handle, dir));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            scenario,
            rescan,
            trainer::trainer,
            trainer::check_scenario,
            trainer::save_scenario,
            trainer::delete_scenario,
            trainer::save_profiles,
            trainer::save_marks,
            trainer::save_training,
            trainer::check_profiles,
            trainer::save_workout,
            trainer::delete_workout,
            trainer::insights_overview,
            trainer::insights,
            trainer::last_session,
            trainer::replay,
            trainer::play,
            trainer::play_workout,
            trainer::sce_info,
            trainer::sce_library,
            trainer::play_sce,
            trainer::program,
            trainer::play_program,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AimAdapt");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bug: a database that failed to open panicked at startup, and release builds
    /// have no console, so the app just didn't appear. Now the error is kept for
    /// the window to show (a newer version's database is the likely case).
    #[test]
    fn a_database_that_wont_open_is_reported_not_panicked() {
        // A folder isn't a database.
        let dir = std::env::temp_dir();
        let args = vec!["aimadapt".to_string(), "--db".to_string(), dir.display().to_string()];
        let (path, db) = open_database(&args);
        assert_eq!(path, dir);
        let e = db.err().expect("an error");
        assert!(e.starts_with("couldn't open"), "{e}");
    }

    /// The goal saved on Settings › Training is the one the app counts against at startup.
    #[test]
    fn the_app_starts_with_the_saved_goal() {
        let dir = std::env::temp_dir().join(format!("aimadapt-goal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let none = time_settings(&dir);
        assert_eq!(none.daily_goal_min, TimeSettings::default().daily_goal_min);
        let prefs = aimadapt_sim::prefs::TrainingPrefs { daily_goal_min: 45.0, ..Default::default() };
        aimadapt_sim::prefs::save(&dir, &prefs).unwrap();
        let t = time_settings(&dir);
        assert_eq!(t.daily_goal_min, 45.0);
        let expected = TimeSettings { daily_goal_min: 45.0, ..TimeSettings::default() };
        assert_eq!(t, expected, "only the goal changes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The sim crate writes the default goal a second time; the two must agree.
    #[test]
    fn the_default_goal_is_the_same_in_sim_and_core() {
        assert_eq!(
            aimadapt_sim::prefs::TrainingPrefs::default().daily_goal_min,
            TimeSettings::default().daily_goal_min
        );
    }

    fn args(a: &[&str]) -> Vec<String> {
        std::iter::once("AimAdapt.exe").chain(a.iter().copied()).map(String::from).collect()
    }

    /// Regression: a `--dump-snapshot` run from an agent's shell without `--db`
    /// opened the real database. The write landed in the shell's private copy of
    /// `%APPDATA%`, which then hid the real database from every later check.
    #[test]
    fn dump_snapshot_needs_a_test_database() {
        let check = |a: &[&str]| aimadapt_core::db::require_test_db(&args(a), DEV_FLAGS);
        assert!(check(&["--dump-snapshot", "out.json"]).is_err());
        assert!(check(&["--no-import", "--dump-snapshot", "out.json"]).is_err());
        assert!(check(&["--db", "test.db", "--dump-snapshot", "out.json"]).is_ok());
        assert!(check(&[]).is_ok(), "a normal launch uses the real database");
    }

    /// The app tells the trainer where its window is, in a form the trainer reads.
    #[test]
    fn the_trainer_reads_the_apps_window_position() {
        let a = trainer::anchor_args(-1920, 40, 2560);
        assert_eq!(a[0], "--anchor");
        assert_eq!(aimadapt_engine::parse_anchor(&a[1]), Some([-1920, 40, 2560]));
    }

    /// Regression: Start on a program day or workout opened a second app window
    /// because only `--play` was recognized. This checks the exact arguments the
    /// Play and Program screens launch with.
    #[test]
    fn the_apps_own_launches_start_the_trainer() {
        let launches = [
            trainer::play_args("builtin:timed-flicks", false),
            trainer::play_args("builtin:timed-flicks", true),
            trainer::workout_args("builtin:quick-warmup"),
            trainer::program_args("builtin:core-week", "mon"),
            trainer::sce_args("C:/Scenarios/VT 1w4ts Novice S5.sce"),
        ];
        for a in launches {
            let full: Vec<String> = std::iter::once("AimAdapt.exe".to_string()).chain(a.clone()).collect();
            assert!(is_trainer_launch(&full), "{a:?} would open the app instead of the trainer");
        }
    }

    #[test]
    fn every_trainer_launch_starts_the_trainer() {
        assert!(is_trainer_launch(&args(&["--play", "builtin:timed-flicks", "--adaptive", "--db", "x.db"])));
        assert!(is_trainer_launch(&args(&["--workout", "builtin:quick-warmup", "--db", "x.db"])));
        assert!(is_trainer_launch(&args(&["--program", "builtin:core-week", "--day", "mon", "--db", "x.db"])));
        assert!(!is_trainer_launch(&args(&[])));
        assert!(!is_trainer_launch(&args(&["--db", "x.db", "--no-import"])));
        assert!(!is_trainer_launch(&args(&["--dump-snapshot", "out.json"])));
    }
}
