//! Commands for the Play, workout and Insights screens: listing scenarios and
//! workouts with their bests and levels, saving and deleting the player's own,
//! engine settings, analytics, replays, and starting the trainer.
//!
//! The trainer runs as a second copy of this executable (`--play <id>` or
//! `--workout <id>`), so the app stays one file and a crash in the game window
//! can't take the app down with it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use aimadapt_core::db::SOURCE_ENGINE;
use aimadapt_core::insights;
use aimadapt_sim::profiles::{self, Profile, Profiles};
use aimadapt_sim::workout::{self, Workout};
use aimadapt_sim::{Scenario, library, spec};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::marks::{self, Marks};
use crate::{AppData, err};

pub struct TrainerState {
    pub data_dir: PathBuf,
    pub playing: AtomicBool,
}

impl TrainerState {
    fn scenarios_dir(&self) -> PathBuf {
        aimadapt_engine::scenarios_dir(&self.data_dir)
    }

    fn workouts_dir(&self) -> PathBuf {
        aimadapt_engine::workouts_dir(&self.data_dir)
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
    /// Standard-mode history for this exact version of the scenario.
    runs: i64,
    best: Option<f64>,
    last_played: Option<String>,
    /// Adaptive mode: runs, the knob, the latest and best ratings in its units, and
    /// the hardest value ever hit.
    adaptive_runs: i64,
    knob: Option<String>,
    rating: Option<f64>,
    best_rating: Option<f64>,
    fastest: Option<f64>,
}

#[derive(Serialize)]
pub struct TrainerWorkout {
    id: String,
    builtin: bool,
    workout: Option<Workout>,
    error: Option<String>,
    /// Total play time in seconds, from each scenario's duration.
    seconds: f64,
    /// When a run of it last finished, if it's been played.
    last_played: Option<String>,
}

#[derive(Serialize)]
pub struct Trainer {
    scenarios: Vec<TrainerScenario>,
    workouts: Vec<TrainerWorkout>,
    profiles: Profiles,
    /// Games whose sensitivity can be typed in their own units.
    games: &'static [profiles::Game],
    /// Whether `engine.toml` exists; if not, `profiles` came from KovaaK's or defaults.
    settings_saved: bool,
    /// A profile matched to the latest KovaaK's run, if known.
    kovaaks_profile: Option<Profile>,
    fields: &'static [spec::Field],
    scenarios_dir: String,
    workouts_dir: String,
    playing: bool,
    /// Favorites and hidden items on the Scenarios page.
    marks: Marks,
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
        .query_map([SOURCE_ENGINE], |r| {
            Ok(((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default()), (r.get(2)?, r.get(3)?, r.get(4)?)))
        })
        .map_err(err)?;
    for row in rows {
        let (k, v) = row.map_err(err)?;
        history.insert(k, v);
    }
    let levels: HashMap<String, insights::ScenarioSummary> =
        insights::overview(&db).map_err(err)?.into_iter().map(|s| (s.scenario.clone(), s)).collect();

    let scenarios: Vec<TrainerScenario> = library::load_all(&t.scenarios_dir())
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
            let lv = scenario.as_ref().and_then(|s| levels.get(&s.name));
            TrainerScenario {
                id: e.id,
                builtin: e.builtin,
                scenario,
                error,
                runs,
                best,
                last_played: last_played.or_else(|| lv.map(|l| l.last_played.clone())),
                adaptive_runs: lv.map_or(0, |l| l.adaptive_runs as i64),
                knob: lv.and_then(|l| l.knob.clone()),
                rating: lv.and_then(|l| l.latest_rating),
                best_rating: lv.and_then(|l| l.best_rating),
                fastest: lv.and_then(|l| l.fastest),
            }
        })
        .collect();

    // Play time plus about 12 s per run for the results screen and loading, the
    // same estimate the Program page uses.
    let duration: HashMap<&str, f64> = scenarios
        .iter()
        .filter_map(|e| {
            e.scenario.as_ref().map(|s| (e.id.as_str(), f64::from(s.timing.duration_s + s.timing.countdown_s) + 12.0))
        })
        .collect();
    let workout_last = db.workouts_last_played().map_err(err)?;
    let workouts = workout::load_all(&t.workouts_dir())
        .into_iter()
        .map(|e| {
            let seconds = e.workout.as_ref().map_or(0.0, |w| {
                w.items
                    .iter()
                    .map(|i| duration.get(i.scenario.as_str()).copied().unwrap_or(75.0) * f64::from(i.runs))
                    .sum()
            });
            let (workout, error) = match e.workout {
                Ok(w) => (Some(w), None),
                Err(m) => (None, Some(m)),
            };
            let last_played = workout.as_ref().and_then(|w| workout_last.get(&w.name).cloned());
            TrainerWorkout { id: e.id, builtin: e.builtin, workout, error, seconds, last_played }
        })
        .collect();

    Ok(Trainer {
        scenarios,
        workouts,
        profiles: aimadapt_engine::load_profiles(&t.data_dir, Some(&db)),
        games: profiles::GAMES,
        settings_saved: aimadapt_engine::settings_path(&t.data_dir).exists(),
        kovaaks_profile: aimadapt_engine::kovaaks_profile(&db),
        fields: spec::FIELDS,
        scenarios_dir: t.scenarios_dir().display().to_string(),
        workouts_dir: t.workouts_dir().display().to_string(),
        playing: t.playing.load(Ordering::SeqCst),
        marks: marks::load(&t.data_dir),
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
pub fn save_workout(
    t: State<'_, TrainerState>,
    workout: serde_json::Value,
    replace: Option<String>,
) -> Result<String, String> {
    let w: Workout = serde_json::from_value(workout).map_err(err)?;
    let replace = replace.filter(|id| id.starts_with("user:"));
    workout::save_user(&t.workouts_dir(), &w, replace.as_deref())
}

#[tauri::command]
pub fn delete_workout(t: State<'_, TrainerState>, id: String) -> Result<(), String> {
    workout::delete_user(&t.workouts_dir(), &id)
}

/// Saves the Scenarios page's favorites and hidden items, and returns them tidied.
#[tauri::command]
pub fn save_marks(t: State<'_, TrainerState>, marks: Marks) -> Result<Marks, String> {
    marks::save(&t.data_dir, marks)
}

/// Saves every settings profile and which one is active.
#[tauri::command]
pub fn save_profiles(t: State<'_, TrainerState>, profiles: serde_json::Value) -> Result<(), String> {
    let p: Profiles = serde_json::from_value(profiles).map_err(err)?;
    aimadapt_engine::save_profiles(&t.data_dir, &p)
}

/// Problems with profiles being edited, in plain words, plus each one's cm/360.
#[tauri::command]
pub fn check_profiles(profiles: serde_json::Value) -> (Vec<String>, Vec<f32>) {
    match serde_json::from_value::<Profiles>(profiles) {
        Ok(p) => (p.problems(), p.profiles.iter().map(Profile::cm_per_360).collect()),
        Err(e) => (vec![e.to_string()], Vec::new()),
    }
}

#[derive(Serialize)]
pub struct InsightsOverview {
    scenarios: Vec<insights::ScenarioSummary>,
}

#[tauri::command]
pub fn insights_overview(state: State<'_, AppData>) -> Result<InsightsOverview, String> {
    let db = state.db.lock().map_err(err)?;
    Ok(InsightsOverview { scenarios: insights::overview(&db).map_err(err)? })
}

#[tauri::command]
pub fn insights(state: State<'_, AppData>, scenario: String) -> Result<Option<insights::Insights>, String> {
    let db = state.db.lock().map_err(err)?;
    insights::scenario(&db, &scenario).map_err(err)
}

/// A trainer run's replay, or `None` for a run without one.
#[tauri::command]
pub fn replay(state: State<'_, AppData>, run_id: i64) -> Result<Option<aimadapt_core::replay::Replay>, String> {
    let db = state.db.lock().map_err(err)?;
    aimadapt_core::replay::replay(&db, run_id).map_err(err)
}

/// Trainer arguments for a scenario. The app's `main` must recognize every
/// `*_args` function's result as a trainer launch (see its tests).
pub(crate) fn play_args(id: &str, adaptive: bool) -> Vec<String> {
    let mut a = vec!["--play".to_string(), id.to_string()];
    if adaptive {
        a.push("--adaptive".into());
    }
    a
}

/// A KovaaK's scenario file, played on its own map.
pub(crate) fn sce_args(path: &str) -> Vec<String> {
    vec!["--sce".into(), path.into()]
}

pub(crate) fn workout_args(id: &str) -> Vec<String> {
    vec!["--workout".into(), id.into()]
}

pub(crate) fn program_args(id: &str, day: &str) -> Vec<String> {
    vec!["--program".into(), id.into(), "--day".into(), day.into()]
}

/// `--anchor x,y,width` for the trainer, from the app window's outer position and width.
pub(crate) fn anchor_args(x: i32, y: i32, width: u32) -> [String; 2] {
    ["--anchor".into(), format!("{x},{y},{width}")]
}

fn launch(app: AppHandle, state: &AppData, t: &TrainerState, args: &[String]) -> Result<(), String> {
    if t.playing.swap(true, Ordering::SeqCst) {
        return Err("The trainer is already open".into());
    }
    let exe = std::env::current_exe().map_err(err)?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args).arg("--db").arg(&state.db_path);
    // Open the trainer over this window, where the player keeps it.
    {
        use tauri::Manager;
        if let Some(w) = app.get_webview_window("main")
            && let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size())
        {
            cmd.args(anchor_args(pos.x, pos.y, size.width));
        }
    }
    let child = cmd.spawn().map_err(|e| {
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

/// Starts the trainer on a scenario. The UI hears `trainer-closed` when it exits.
#[tauri::command]
pub fn play(
    app: AppHandle,
    state: State<'_, AppData>,
    t: State<'_, TrainerState>,
    id: String,
    adaptive: bool,
) -> Result<(), String> {
    // Check it loads first so a broken file shows an error here, not a silent exit.
    library::resolve(&id, &t.scenarios_dir())?;
    launch(app, &state, &t, &play_args(&id, adaptive))
}

/// A KovaaK's scenario's `.sce` file and whether the trainer can play it, with its
/// runs in the trainer (kept apart from KovaaK's own runs).
#[derive(Serialize)]
pub struct SceInfo {
    /// None when no file with the scenario's name is in the KovaaK's folders.
    path: Option<String>,
    playable: bool,
    /// Why it can't be played yet, in plain words.
    reasons: Vec<String>,
    /// What plays differently from KovaaK's.
    notes: Vec<String>,
    runs: i64,
    best: Option<f64>,
    last_played: Option<String>,
}

/// The `.sce` file named after a scenario, among these files.
pub(crate) fn sce_file_for(name: &str, files: &[PathBuf]) -> Option<PathBuf> {
    let want = name.trim().to_lowercase();
    files.iter().find(|f| f.file_stem().is_some_and(|s| s.to_string_lossy().trim().to_lowercase() == want)).cloned()
}

#[tauri::command]
pub fn sce_info(state: State<'_, AppData>, name: String) -> Result<SceInfo, String> {
    let files = aimadapt_sce::find_files(&aimadapt_core::discover::find_scenario_dirs());
    let path = sce_file_for(&name, &files);
    let (playable, reasons, notes) = match path.as_deref().map(aimadapt_sce::import) {
        None => (false, Vec::new(), Vec::new()),
        Some(Ok(i)) => (true, Vec::new(), i.notes),
        Some(Err(r)) => (false, r, Vec::new()),
    };
    let db = state.db.lock().map_err(err)?;
    let (runs, best, last_played) = db
        .conn()
        .query_row(
            "SELECT COUNT(*), MAX(score), MAX(finished_at) FROM runs WHERE source = ?1 AND mode = ?2 AND scenario = ?3",
            (SOURCE_ENGINE, aimadapt_core::db::MODE_IMPORTED, &name),
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(err)?;
    Ok(SceInfo { path: path.map(|p| p.display().to_string()), playable, reasons, notes, runs, best, last_played })
}

/// One of your KovaaK's scenario files, for Browse on Scenarios.
#[derive(Debug, Serialize)]
pub struct SceEntry {
    /// The file's name without `.sce`: KovaaK's stats use it, so history matches.
    name: String,
    path: String,
    /// `AimTypeTag`: Clicking, Tracking, Target Switching or Other; "" when unset.
    aim_type: String,
    description: String,
    /// The file's game and weapon tags, lowercase.
    tags: Vec<String>,
    playable: bool,
}

/// Every KovaaK's scenario in your KovaaK's and Workshop folders, with whether the
/// trainer can play it. Reading a few hundred files takes a second or two, so it
/// runs off the main thread.
#[tauri::command(async)]
pub fn sce_library() -> Vec<SceEntry> {
    sce_entries(&aimadapt_sce::find_files(&aimadapt_core::discover::find_scenario_dirs()))
}

/// One entry per name (the first file wins, as in [`sce_file_for`]), skipping files
/// that can't be read. Files are only read.
pub(crate) fn sce_entries(files: &[PathBuf]) -> Vec<SceEntry> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for path in files {
        let Some(name) = path.file_stem().map(|s| s.to_string_lossy().trim().to_string()) else { continue };
        if name.is_empty() || !seen.insert(name.to_lowercase()) {
            continue;
        }
        let Ok(sce) = aimadapt_sce::Sce::read(path) else { continue };
        let top = &sce.settings.top;
        let mut tags: Vec<String> = Vec::new();
        for key in ["GameTag", "WeaponHeroTag"] {
            for t in top.str(key).split(',').map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()) {
                if !tags.contains(&t) {
                    tags.push(t);
                }
            }
        }
        out.push(SceEntry {
            name,
            path: path.display().to_string(),
            aim_type: top.str("AimTypeTag").trim().to_string(),
            description: top.str("Description").replace("[nl]", " ").trim().to_string(),
            tags,
            playable: aimadapt_sce::import(path).is_ok(),
        });
    }
    out
}

/// Starts the trainer on a KovaaK's scenario file.
#[tauri::command]
pub fn play_sce(
    app: AppHandle,
    state: State<'_, AppData>,
    t: State<'_, TrainerState>,
    path: String,
) -> Result<(), String> {
    aimadapt_sce::import(std::path::Path::new(&path)).map_err(|r| format!("Can't play this yet: {}", r.join("; ")))?;
    launch(app, &state, &t, &sce_args(&path))
}

/// Starts the trainer on a workout.
#[tauri::command]
pub fn play_workout(
    app: AppHandle,
    state: State<'_, AppData>,
    t: State<'_, TrainerState>,
    id: String,
) -> Result<(), String> {
    let w = workout::resolve(&id, &t.workouts_dir())?;
    for item in &w.items {
        library::resolve(&item.scenario, &t.scenarios_dir()).map_err(|e| format!("{}: {e}", item.scenario))?;
    }
    launch(app, &state, &t, &workout_args(&id))
}

#[derive(Serialize)]
pub struct ProgramDay {
    day: &'static str,
    name: &'static str,
    focus: Option<String>,
    weak_spots: bool,
    /// The day's workout (built from the skill profile for weak-spots days).
    workout: Option<Workout>,
    runs: u32,
    minutes: f64,
}

#[derive(Serialize)]
pub struct ProgramPage {
    id: String,
    name: String,
    description: String,
    today: &'static str,
    days: Vec<ProgramDay>,
    progress: aimadapt_core::programs::ProgramProgress,
    playing: bool,
}

#[tauri::command]
pub fn program(state: State<'_, AppData>, t: State<'_, TrainerState>) -> Result<ProgramPage, String> {
    build_program(&state, &t)
}

pub fn build_program(state: &AppData, t: &TrainerState) -> Result<ProgramPage, String> {
    use aimadapt_sim::program::{self, Weekday};
    let id = "builtin:core-week";
    let prog = program::resolve(id)?;
    let db = state.db.lock().map_err(err)?;
    let durations: HashMap<String, f64> = library::load_all(&t.scenarios_dir())
        .into_iter()
        .filter_map(|e| e.scenario.ok().map(|s| (e.id, f64::from(s.timing.duration_s + s.timing.countdown_s))))
        .collect();
    let week = aimadapt_engine::training_week();
    let days: Vec<ProgramDay> = Weekday::ALL
        .into_iter()
        .map(|d| {
            let pd = prog.day(d);
            let workout =
                pd.and_then(|_| aimadapt_engine::workout_for_day(&prog, d, Some(&db), &t.workouts_dir(), week).ok());
            let runs = workout.as_ref().map_or(0, Workout::total_runs);
            // About 12 s per run for the results screen and loading.
            let seconds: f64 = workout.as_ref().map_or(0.0, |w| {
                w.items
                    .iter()
                    .map(|i| (durations.get(&i.scenario).copied().unwrap_or(63.0) + 12.0) * f64::from(i.runs))
                    .sum()
            });
            ProgramDay {
                day: d.id(),
                name: d.name(),
                focus: pd.map(|p| p.focus.clone()),
                weak_spots: pd.is_some_and(|p| p.weak_spots),
                workout,
                runs,
                minutes: seconds / 60.0,
            }
        })
        .collect();
    let plan: Vec<(String, u32)> = days.iter().map(|d| (d.day.to_string(), d.runs)).collect();
    let now = chrono::Local::now().naive_local();
    let progress = aimadapt_core::programs::progress(&db, &prog.name, now, &plan).map_err(err)?;
    Ok(ProgramPage {
        id: id.into(),
        name: prog.name.clone(),
        description: prog.description.clone(),
        today: aimadapt_engine::today().id(),
        days,
        progress,
        playing: t.playing.load(Ordering::SeqCst),
    })
}

/// Starts a program day in the trainer (today if `day` is empty).
#[tauri::command]
pub fn play_program(
    app: AppHandle,
    state: State<'_, AppData>,
    t: State<'_, TrainerState>,
    id: String,
    day: Option<String>,
) -> Result<(), String> {
    aimadapt_sim::program::resolve(&id)?;
    let day = day.unwrap_or_else(|| aimadapt_engine::today().id().to_string());
    launch(app, &state, &t, &program_args(&id, &day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kovaaks_files_are_listed_once_each_with_whether_they_play() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../aimadapt-sce/tests/fixtures");
        let mut files = aimadapt_sce::find_files(std::slice::from_ref(&dir));
        assert_eq!(files.len(), 3, "{files:?}");
        // The same scenario in a second folder (the Workshop, say) is listed once.
        files.push(files[0].clone());
        let list = sce_entries(&files);
        assert_eq!(list.len(), 3);
        let wall = list.iter().find(|e| e.name == "AimAdapt Test - Still Wall").expect("the still-wall fixture");
        assert!(wall.playable);
        assert_eq!(wall.aim_type, "Clicking");
        // [nl] is a line break in KovaaK's descriptions.
        assert!(wall.description.contains("tests. Three"), "{}", wall.description);
        assert!(list.iter().any(|e| !e.playable), "the moving-bot fixture can't play yet");
    }

    #[test]
    fn a_scenarios_file_is_found_by_its_name() {
        let files = vec![PathBuf::from("C:/S/VT 1w4ts Novice S5.sce"), PathBuf::from("C:/S/vt ww5t novice s5.sce")];
        assert_eq!(sce_file_for("VT 1w4ts Novice S5", &files), Some(files[0].clone()));
        assert_eq!(sce_file_for("VT ww5t Novice S5", &files), Some(files[1].clone()));
        assert_eq!(sce_file_for("VT 1w4ts Novice S5 Hard", &files), None);
    }
}
