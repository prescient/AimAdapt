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

use aimadapt_core::insights;
use aimadapt_core::programs::recap;
use aimadapt_sim::adaptive::Mix;
use aimadapt_sim::prefs::TrainingPrefs;
use aimadapt_sim::profiles::{self, Profile, Profiles};
use aimadapt_sim::workout::{self, Workout};
use aimadapt_sim::{Scenario, library, spec};
use serde::{Deserialize, Serialize};
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
type History = aimadapt_core::trainer_runs::Played;

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

/// The Training tab's settings as the pages use them: the mix in use and the daily goal.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Training {
    mix: Mix,
    daily_goal_min: f64,
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
    /// Set when `engine.toml` exists but can't be used: `profiles` are defaults.
    profiles_note: Option<String>,
    /// A profile matched to the latest KovaaK's run, if known.
    kovaaks_profile: Option<Profile>,
    fields: &'static [spec::Field],
    /// A scenario with every setting at its default, for the editor to tell which sections hold only defaults.
    defaults: Scenario,
    scenarios_dir: String,
    workouts_dir: String,
    playing: bool,
    /// Favorites and hidden items on the Scenarios page.
    marks: Marks,
    /// The mix in use (for every profile) and the daily goal.
    training: Training,
}

#[tauri::command(async)]
pub fn trainer(state: State<'_, AppData>, t: State<'_, TrainerState>) -> Result<Trainer, String> {
    build_trainer(&state, &t)
}

pub fn build_trainer(state: &AppData, t: &TrainerState) -> Result<Trainer, String> {
    let db = state.db.lock().map_err(err)?;
    let history: HashMap<(String, String), History> = db.trainer_played().map_err(err)?;
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
        .filter_map(|e| e.scenario.as_ref().map(|s| (e.id.as_str(), f64::from(s.real_seconds()) + 12.0)))
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

    let profiles = aimadapt_engine::load_profiles(&t.data_dir, Some(&db));
    let prefs = aimadapt_sim::prefs::load(&t.data_dir);
    let training =
        Training { mix: prefs.mix_or(profiles.active().resolved().challenge), daily_goal_min: prefs.daily_goal_min };
    Ok(Trainer {
        scenarios,
        workouts,
        profiles,
        games: profiles::GAMES,
        settings_saved: aimadapt_engine::settings_path(&t.data_dir).exists(),
        profiles_note: aimadapt_engine::profiles_note(&t.data_dir),
        kovaaks_profile: aimadapt_engine::kovaaks_profile(&db),
        fields: spec::FIELDS,
        defaults: Scenario::default(),
        scenarios_dir: t.scenarios_dir().display().to_string(),
        workouts_dir: t.workouts_dir().display().to_string(),
        playing: t.playing.load(Ordering::SeqCst),
        marks: marks::load(&t.data_dir),
        training,
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

/// Saves the Training tab's mix and daily goal, and counts against the new goal from now on.
#[tauri::command]
pub fn save_training(state: State<'_, AppData>, t: State<'_, TrainerState>, training: Training) -> Result<(), String> {
    let prefs = TrainingPrefs { mix: Some(training.mix), daily_goal_min: training.daily_goal_min };
    aimadapt_sim::prefs::save(&t.data_dir, &prefs)?;
    state.set_daily_goal(training.daily_goal_min);
    Ok(())
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

#[tauri::command(async)]
pub fn insights_overview(state: State<'_, AppData>) -> Result<InsightsOverview, String> {
    let db = state.db.lock().map_err(err)?;
    Ok(InsightsOverview { scenarios: insights::overview(&db).map_err(err)? })
}

#[tauri::command(async)]
pub fn insights(state: State<'_, AppData>, scenario: String) -> Result<Option<insights::Insights>, String> {
    let db = state.db.lock().map_err(err)?;
    insights::scenario(&db, &scenario).map_err(err)
}

/// What the last session changed, for the session summary and Today.
#[tauri::command(async)]
pub fn last_session(state: State<'_, AppData>) -> Result<Option<recap::SessionRecap>, String> {
    let db = state.db.lock().map_err(err)?;
    recap::last_session(&db, &state.time()).map_err(err)
}

/// A trainer run's replay, or `None` for a run without one.
#[tauri::command(async)]
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

/// A page the trainer asked the app to open as it closed.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OpenPage {
    /// The replay of the run with this id.
    Replay(i64),
    /// The page of the scenario with this name.
    Scenario(String),
}

impl From<aimadapt_engine::Open> for OpenPage {
    fn from(open: aimadapt_engine::Open) -> Self {
        match open {
            aimadapt_engine::Open::Replay(id) => OpenPage::Replay(id),
            aimadapt_engine::Open::Scenario(name) => OpenPage::Scenario(name),
        }
    }
}

/// What the UI hears when the trainer closes.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TrainerClosed {
    /// Why the trainer failed, in its own words, or `None` when it closed normally.
    pub error: Option<String>,
    /// The page to open, when the player asked for one with a key on the results screen.
    pub open: Option<OpenPage>,
    /// Runs the trainer saved while it was open.
    pub played: u32,
}

/// How many runs the trainer has saved, or `None` when the database can't say.
fn trainer_runs(state: &AppData) -> Option<i64> {
    state.db.lock().ok()?.trainer_run_count().ok()
}

/// Runs saved while the trainer was open, from the counts before and after. A count
/// that went down (a run was deleted meanwhile) is none.
pub(crate) fn played(before: Option<i64>, after: Option<i64>) -> u32 {
    match (before, after) {
        (Some(b), Some(a)) => u32::try_from(a - b).unwrap_or(0),
        _ => 0,
    }
}

/// The trainer reports problems on stderr (`aimadapt: <message>`, or a panic) and
/// exits with a non-zero code. Release builds have no console, so the app passes
/// them on.
pub(crate) fn closed_report(
    success: bool,
    code: Option<i32>,
    stderr: &[String],
    open: Option<OpenPage>,
    played: u32,
) -> TrainerClosed {
    if success {
        return TrainerClosed { error: None, open, played };
    }
    let own: Vec<&str> = stderr.iter().filter_map(|l| l.strip_prefix("aimadapt: ")).collect();
    let lines: Vec<&str> = if own.is_empty() {
        let n = stderr.len().saturating_sub(6);
        stderr[n..].iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect()
    } else {
        own
    };
    let error = if lines.is_empty() {
        match code {
            Some(c) => format!("The trainer closed with an error (exit code {c})."),
            None => "The trainer was stopped.".to_string(),
        }
    } else {
        format!("The trainer closed with an error: {}", lines.join(" "))
    };
    TrainerClosed { error: Some(error), open: None, played }
}

/// Sorts one line of the trainer's stderr: a request to open a page is kept aside,
/// anything else joins the last lines kept for an error report.
fn take_line(line: String, lines: &mut std::collections::VecDeque<String>, open: &mut Option<OpenPage>) {
    if let Some(page) = aimadapt_engine::Open::parse(&line) {
        *open = Some(page.into());
        return;
    }
    if lines.len() == 40 {
        lines.pop_front();
    }
    lines.push_back(line);
}

fn launch(app: AppHandle, state: &AppData, t: &TrainerState, args: &[String]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(err)?;
    let before = trainer_runs(state);
    if t.playing.swap(true, Ordering::SeqCst) {
        return Err("The trainer is already open".into());
    }
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args).arg("--db").arg(&state.db_path).arg("--handoff").stderr(std::process::Stdio::piped());
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
    let stderr = child.stderr.take();
    std::thread::spawn(move || {
        // Keep the last lines the trainer writes; reading also keeps its pipe from filling.
        let mut lines = std::collections::VecDeque::new();
        let mut open = None;
        if let Some(out) = stderr {
            use std::io::BufRead;
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                take_line(line, &mut lines, &mut open);
            }
        }
        let status = child.wait();
        use tauri::Manager;
        app.state::<TrainerState>().playing.store(false, Ordering::SeqCst);
        let played = played(before, trainer_runs(&app.state::<AppData>()));
        let lines: Vec<String> = lines.into();
        let report = match status {
            Ok(s) => closed_report(s.success(), s.code(), &lines, open, played),
            Err(e) => TrainerClosed { error: Some(format!("Lost track of the trainer: {e}")), open: None, played },
        };
        let _ = app.emit("trainer-closed", report);
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

#[tauri::command(async)]
pub fn sce_info(state: State<'_, AppData>, name: String) -> Result<SceInfo, String> {
    let files = aimadapt_sce::find_files(&aimadapt_core::discover::find_scenario_dirs());
    let path = sce_file_for(&name, &files);
    let (playable, reasons, notes) = match path.as_deref().map(aimadapt_sce::import) {
        None => (false, Vec::new(), Vec::new()),
        Some(Ok(i)) => (true, Vec::new(), i.notes),
        Some(Err(r)) => (false, r.iter().map(ToString::to_string).collect(), Vec::new()),
    };
    let db = state.db.lock().map_err(err)?;
    let (runs, best, last_played) = db.imported_played(&name).map_err(err)?;
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
    aimadapt_sce::import(std::path::Path::new(&path)).map_err(|r| {
        format!("Can't play this yet: {}", r.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))
    })?;
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
    /// Planned against actual success by kind of run, over every scenario (the Skills tab).
    plan: aimadapt_core::plan_check::PlanCheck,
    playing: bool,
}

#[tauri::command(async)]
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
        .filter_map(|e| e.scenario.ok().map(|s| (e.id, f64::from(s.real_seconds()))))
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
    let progress = aimadapt_core::programs::progress(&db, &prog.name, now, &plan, &state.time()).map_err(err)?;
    let plan_check = aimadapt_core::plan_check::by_kind(&db, None).map_err(err)?;
    Ok(ProgramPage {
        id: id.into(),
        name: prog.name.clone(),
        description: prog.description.clone(),
        today: aimadapt_engine::today().id(),
        days,
        progress,
        plan: plan_check,
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

    /// Bug: the app ignored how the trainer exited, and release builds have no
    /// console, so a scenario that failed to load closed the game with no word why.
    /// (The banner itself is in the UI; this checks what's passed to it.)
    #[test]
    fn a_failed_trainer_says_why() {
        let lines = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(closed_report(true, Some(0), &[], None, 0), TrainerClosed { error: None, open: None, played: 0 });
        let r =
            closed_report(false, Some(2), &lines(&["some log line", "aimadapt: no built-in scenario nope"]), None, 0);
        assert_eq!(r.error.as_deref(), Some("The trainer closed with an error: no built-in scenario nope"));
        let r = closed_report(false, Some(101), &lines(&["thread 'main' panicked at src/x.rs:1:1:", "boom"]), None, 0);
        assert!(r.error.unwrap().contains("boom"));
        let r = closed_report(false, Some(3), &[], None, 0);
        assert!(r.error.unwrap().contains("exit code 3"));
    }

    /// The app counts the runs the trainer saved between launch and exit and tells the UI, so it
    /// can open the session summary. A crash after some runs still saved them.
    #[test]
    fn the_app_counts_the_runs_a_trainer_visit_saved() {
        assert_eq!(played(Some(3), Some(5)), 2);
        assert_eq!(played(Some(4), Some(4)), 0);
        assert_eq!(played(Some(5), Some(3)), 0, "a run was deleted meanwhile");
        assert_eq!(played(None, Some(3)), 0);
        assert_eq!(played(Some(3), None), 0);
        assert_eq!(closed_report(true, Some(0), &[], None, 2).played, 2);
        assert_eq!(closed_report(false, Some(1), &[], None, 3).played, 3);
        assert_eq!(serde_json::to_value(closed_report(true, Some(0), &[], None, 1)).unwrap()["played"], 1);
    }

    /// V or I on the results screen make the trainer print one line, and the app passes the page on
    /// to the UI. The line stays out of the error report, and a failed trainer opens nothing.
    #[test]
    fn the_trainer_can_ask_for_a_page() {
        assert_eq!(serde_json::to_value(OpenPage::Replay(123)).unwrap(), serde_json::json!({"replay": 123}));
        assert_eq!(
            serde_json::to_value(OpenPage::Scenario("Timed Flicks".into())).unwrap(),
            serde_json::json!({"scenario": "Timed Flicks"})
        );
        let mut lines = std::collections::VecDeque::new();
        let mut open = None;
        for i in 0..50 {
            take_line(format!("log {i}"), &mut lines, &mut open);
        }
        take_line("aimadapt-open: replay 9".into(), &mut lines, &mut open);
        assert_eq!(open, Some(OpenPage::Replay(9)));
        assert_eq!(lines.len(), 40, "the buffer is capped");
        assert!(lines.iter().all(|l| l.starts_with("log ")), "the request is not a log line");
        take_line("aimadapt-open: scenario Timed Flicks".into(), &mut lines, &mut open);
        assert_eq!(open, Some(OpenPage::Scenario("Timed Flicks".into())), "the last request wins");
        let page = Some(OpenPage::Replay(9));
        assert_eq!(closed_report(true, Some(0), &[], page.clone(), 0).open, page);
        assert_eq!(closed_report(false, Some(2), &[], page, 0).open, None);
    }

    /// The editor folds a section when its fields equal the defaults, read from the payload's `defaults`
    /// at each field's path. `build_trainer` needs a database, so this checks what it puts there: the
    /// default scenario serializes with a value at every path the editor draws (a missing one would
    /// read as "not default" and keep its section open).
    #[test]
    fn the_editors_defaults_have_a_value_for_every_field() {
        let defaults = serde_json::to_value(Scenario::default()).unwrap();
        for f in spec::FIELDS {
            let found = f.path.split('.').try_fold(&defaults, |v, k| v.get(k));
            assert!(found.is_some(), "no default at {}", f.path);
        }
    }

    #[test]
    fn kovaaks_files_are_listed_once_each_with_whether_they_play() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../aimadapt-sce/tests/fixtures");
        let mut files = aimadapt_sce::find_files(std::slice::from_ref(&dir));
        assert_eq!(files.len(), 17, "{files:?}");
        // The same scenario in a second folder (the Workshop, say) is listed once.
        files.push(files[0].clone());
        let list = sce_entries(&files);
        assert_eq!(list.len(), 17);
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
