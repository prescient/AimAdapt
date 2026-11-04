//! RustyAim desktop app: a Tauri shell over `rustyaim-core`. The UI in `ui/` asks
//! for a snapshot of everything it shows, and the folder watcher tells it when new
//! runs arrive so it can refresh.

// Hide the console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use chrono::Local;
use rustyaim_core::progress::{self, ColdStart, FatigueBucket, ProgressSettings, ScenarioRun, ScenarioStats, SensPeriod};
use rustyaim_core::training::{self, TimeSettings, TrainingReport};
use rustyaim_core::{Db, ImportReport, discover, import};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppData {
    db: Mutex<Db>,
    db_path: PathBuf,
    stats_dir: Option<PathBuf>,
    last_import: Mutex<Option<ImportReport>>,
    time: TimeSettings,
    progress: ProgressSettings,
}

#[derive(Serialize)]
struct GameInfo {
    avg_fps: Option<f64>,
    max_fps: Option<f64>,
    resolution: Option<String>,
    game_version: Option<String>,
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
    game: Option<GameInfo>,
}

#[derive(Serialize)]
struct ScenarioDetail {
    stats: ScenarioStats,
    history: Vec<ScenarioRun>,
    cold_starts: Vec<ColdStart>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn snapshot(state: State<'_, AppData>) -> Result<Snapshot, String> {
    build_snapshot(&state)
}

#[tauri::command]
fn scenario(state: State<'_, AppData>, name: String) -> Result<Option<ScenarioDetail>, String> {
    build_scenario(&state, &name)
}

fn build_snapshot(state: &AppData) -> Result<Snapshot, String> {
    let db = state.db.lock().map_err(err)?;
    let training = training::report(&db, &state.time, Local::now().naive_local()).map_err(err)?;
    let p = progress::load(&db, &state.time).map_err(err)?;
    let game = db
        .conn()
        .query_row(
            "SELECT avg_fps, max_fps, resolution, game_version FROM runs ORDER BY finished_at DESC LIMIT 1",
            [],
            |r| Ok(GameInfo { avg_fps: r.get(0)?, max_fps: r.get(1)?, resolution: r.get(2)?, game_version: r.get(3)? }),
        )
        .ok();
    Ok(Snapshot {
        db_path: state.db_path.display().to_string(),
        stats_dir: state.stats_dir.as_ref().map(|d| d.display().to_string()),
        last_import: state.last_import.lock().map_err(err)?.clone(),
        import_errors: db.import_errors().map_err(err)?,
        time_settings: state.time.clone(),
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
    let p = progress::load(&db, &state.time).map_err(err)?;
    Ok(scenario_detail(&p, &state.progress, name))
}

fn scenario_detail(p: &progress::Progress, settings: &ProgressSettings, name: &str) -> Option<ScenarioDetail> {
    Some(ScenarioDetail {
        stats: p.stats_for(name, settings)?,
        history: p.history(name).into_iter().cloned().collect(),
        cold_starts: p.cold_starts().into_iter().filter(|c| c.scenario == name).collect(),
    })
}

/// `rustyaim-app --dump-snapshot <file>` writes what the UI would receive (the snapshot
/// plus every scenario's detail) as JSON and exits. Lets the UI be previewed in a
/// browser against real data: `ui/app.js` loads `dev-snapshot.json` when not in Tauri.
fn dump_snapshot(state: &AppData, path: &std::path::Path) -> Result<(), String> {
    let snapshot = build_snapshot(state)?;
    let db = state.db.lock().map_err(err)?;
    let p = progress::load(&db, &state.time).map_err(err)?;
    let scenarios: std::collections::BTreeMap<String, ScenarioDetail> = p
        .scenarios()
        .into_iter()
        .filter_map(|name| scenario_detail(&p, &state.progress, &name).map(|d| (name, d)))
        .collect();
    #[derive(Serialize)]
    struct Dump<'a> {
        snapshot: &'a Snapshot,
        scenarios: &'a std::collections::BTreeMap<String, ScenarioDetail>,
    }
    let json = serde_json::to_string(&Dump { snapshot: &snapshot, scenarios: &scenarios }).map_err(err)?;
    std::fs::write(path, json).map_err(err)
}

#[tauri::command]
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

fn main() {
    let db_path = rustyaim_core::db::default_path().expect("couldn't find a folder for the database");
    let db = Db::open(&db_path).expect("couldn't open the database");
    let data = AppData {
        db: Mutex::new(db),
        db_path,
        stats_dir: discover::find_stats_dir(),
        last_import: Mutex::new(None),
        time: TimeSettings::default(),
        progress: ProgressSettings::default(),
    };
    // Catch up on runs played while the app was closed before the first screen loads.
    let _ = import_now(&data);

    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--dump-snapshot") {
        let path = args.get(i + 1).expect("--dump-snapshot needs a file path");
        dump_snapshot(&data, std::path::Path::new(path)).expect("couldn't write the snapshot");
        return;
    }

    let watch_dir = data.stats_dir.clone();

    tauri::Builder::default()
        .manage(data)
        .setup(move |app| {
            if let Some(dir) = watch_dir {
                let handle = app.handle().clone();
                std::thread::spawn(move || watch_stats(handle, dir));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![snapshot, scenario, rescan])
        .run(tauri::generate_context!())
        .expect("error while running RustyAim");
}
