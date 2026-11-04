//! Commands for the Play screen and the scenario editor: listing scenarios with
//! their bests, saving and deleting the player's own, engine settings, and starting
//! the trainer.
//!
//! The trainer runs as a second copy of this executable (`--play <id>`), so the app
//! stays one file and a crash in the game window can't take the app down with it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use rustyaim_core::db::SOURCE_RUSTYAIM;
use rustyaim_sim::{PlayerSettings, Scenario, library, spec};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{AppData, err};

pub struct TrainerState {
    pub data_dir: PathBuf,
    pub playing: AtomicBool,
}

impl TrainerState {
    fn scenarios_dir(&self) -> PathBuf {
        rustyaim_engine::scenarios_dir(&self.data_dir)
    }
}

/// Runs, best score and last played time for one scenario version.
type History = (i64, Option<f64>, Option<String>);

#[derive(Serialize)]
pub struct TrainerScenario {
    id: String,
    builtin: bool,
    scenario: Option<Scenario>,
    error: Option<String>,
    /// History for this exact version of the scenario (same name and settings).
    runs: i64,
    best: Option<f64>,
    last_played: Option<String>,
}

#[derive(Serialize)]
pub struct Trainer {
    scenarios: Vec<TrainerScenario>,
    settings: PlayerSettings,
    /// Whether `engine.toml` exists; if not, `settings` came from KovaaK's or defaults.
    settings_saved: bool,
    /// Sensitivity and FOV matched to the latest KovaaK's run, if known.
    kovaaks_settings: Option<PlayerSettings>,
    fields: &'static [spec::Field],
    scenarios_dir: String,
    playing: bool,
}

#[tauri::command]
pub fn trainer(state: State<'_, AppData>, t: State<'_, TrainerState>) -> Result<Trainer, String> {
    build_trainer(&state, &t)
}

pub fn build_trainer(state: &AppData, t: &TrainerState) -> Result<Trainer, String> {
    let db = state.db.lock().map_err(err)?;
    let mut history: HashMap<(String, String), History> = HashMap::new();
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT scenario, scenario_hash, COUNT(*), MAX(score), MAX(finished_at) FROM runs
             WHERE source = ?1 GROUP BY scenario, scenario_hash",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([SOURCE_RUSTYAIM], |r| {
            Ok(((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default()), (r.get(2)?, r.get(3)?, r.get(4)?)))
        })
        .map_err(err)?;
    for row in rows {
        let (k, v) = row.map_err(err)?;
        history.insert(k, v);
    }

    let scenarios = library::load_all(&t.scenarios_dir())
        .into_iter()
        .map(|e| {
            let (scenario, error) = match e.scenario {
                Ok(s) => (Some(s), None),
                Err(m) => (None, Some(m)),
            };
            let (runs, best, last_played) = scenario
                .as_ref()
                .and_then(|s| history.get(&(s.name.clone(), s.fingerprint())).cloned())
                .unwrap_or((0, None, None));
            TrainerScenario { id: e.id, builtin: e.builtin, scenario, error, runs, best, last_played }
        })
        .collect();

    Ok(Trainer {
        scenarios,
        settings: rustyaim_engine::load_settings(&t.data_dir, Some(&db)),
        settings_saved: rustyaim_engine::settings_path(&t.data_dir).exists(),
        kovaaks_settings: rustyaim_engine::from_kovaaks(&db),
        fields: spec::FIELDS,
        scenarios_dir: t.scenarios_dir().display().to_string(),
        playing: t.playing.load(Ordering::SeqCst),
    })
}

fn parse_scenario(value: serde_json::Value) -> Result<Scenario, Vec<String>> {
    let s: Scenario = serde_json::from_value(value).map_err(|e| vec![e.to_string()])?;
    let p = s.problems();
    if p.is_empty() { Ok(s) } else { Err(p) }
}

/// Problems with a scenario being edited, in plain words. Empty when it's valid.
#[tauri::command]
pub fn check_scenario(scenario: serde_json::Value) -> Vec<String> {
    parse_scenario(scenario).err().unwrap_or_default()
}

/// Saves a scenario to the player's folder. `replace` is the id being edited; editing
/// a built-in scenario always saves a copy. Returns the saved id.
#[tauri::command]
pub fn save_scenario(
    t: State<'_, TrainerState>,
    scenario: serde_json::Value,
    replace: Option<String>,
) -> Result<String, String> {
    let s = parse_scenario(scenario).map_err(|p| p.join("; "))?;
    let replace = replace.filter(|id| id.starts_with("user:"));
    library::save_user(&t.scenarios_dir(), &s, replace.as_deref())
}

#[tauri::command]
pub fn delete_scenario(t: State<'_, TrainerState>, id: String) -> Result<(), String> {
    library::delete_user(&t.scenarios_dir(), &id)
}

#[tauri::command]
pub fn save_engine_settings(t: State<'_, TrainerState>, settings: serde_json::Value) -> Result<(), String> {
    let s: PlayerSettings = serde_json::from_value(settings).map_err(err)?;
    rustyaim_engine::save_settings(&t.data_dir, &s)
}

/// Starts the trainer on a scenario. The UI hears `trainer-closed` when it exits.
#[tauri::command]
pub fn play(app: AppHandle, state: State<'_, AppData>, t: State<'_, TrainerState>, id: String) -> Result<(), String> {
    // Check it loads first so a broken file shows an error here, not a silent exit.
    library::resolve(&id, &t.scenarios_dir())?;
    if t.playing.swap(true, Ordering::SeqCst) {
        return Err("The trainer is already open".into());
    }
    let exe = std::env::current_exe().map_err(err)?;
    let child =
        std::process::Command::new(exe).arg("--play").arg(&id).arg("--db").arg(&state.db_path).spawn().map_err(|e| {
            t.playing.store(false, Ordering::SeqCst);
            err(e)
        });
    let mut child = child?;
    std::thread::spawn(move || {
        let _ = child.wait();
        use tauri::Manager;
        app.state::<TrainerState>().playing.store(false, Ordering::SeqCst);
        let _ = app.emit("trainer-closed", ());
    });
    Ok(())
}
