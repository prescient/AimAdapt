//! RustyAim's built-in aim trainer. Runs a scenario or a whole workout in its own
//! window (Bevy), with raw mouse input and no frame cap, and saves each finished
//! run to the shared database with its adaptive level and telemetry.
//!
//! The desktop app starts it by running its own executable with `--play <id>` or
//! `--workout <id>`, so the whole program stays a single file.

mod bot;
mod config;
mod game;
pub mod record;

use std::path::PathBuf;

use rustyaim_sim::settings::DisplayMode;
use rustyaim_sim::workout::{self, Mode};

pub use config::{
    from_kovaaks, kovaaks_profile, load as load_settings, load_profiles, save_profiles, scenarios_dir, settings_path,
    workouts_dir,
};
pub use game::{PlayOptions, PlayStep, run};

/// Command-line entry point. Returns the process exit code.
///
/// - `--play <scenario id or .toml path> [--adaptive]`: one scenario.
/// - `--workout <workout id or .toml path>`: every run of a workout in turn.
///
/// Options: `--profile <name>` (default: the active settings profile), `--db <file>`, `--windowed`, `--no-save`, `--seed <n>`, and
/// `--smoke <folder>`, which has a bot play short versions of the runs
/// (`--smoke-seconds`, default 4), saves screenshots in the folder and exits; it's
/// for checking the engine works.
pub fn run_from_args(args: &[String]) -> i32 {
    match parse_and_run(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("rustyaim: {e}");
            2
        }
    }
}

fn parse_and_run(args: &[String]) -> Result<(), String> {
    let mut id = None;
    let mut workout_id = None;
    let mut adaptive = false;
    let mut profile = None;
    let mut db = None;
    let mut windowed = false;
    let mut save = true;
    let mut seed = None;
    let mut smoke = None;
    let mut smoke_seconds = 4.0;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--play" => id = it.next().cloned(),
            "--workout" => workout_id = it.next().cloned(),
            "--adaptive" => adaptive = true,
            "--profile" => profile = it.next().cloned(),
            "--db" => db = it.next().map(PathBuf::from),
            "--windowed" => windowed = true,
            "--no-save" => save = false,
            "--seed" => seed = it.next().and_then(|s| s.parse().ok()),
            "--smoke" => smoke = it.next().map(PathBuf::from),
            "--smoke-seconds" => smoke_seconds = it.next().and_then(|s| s.parse().ok()).unwrap_or(4.0),
            other if id.is_none() && !other.starts_with("--") => id = Some(other.to_string()),
            _ => {}
        }
    }
    let data = rustyaim_core::db::data_dir().map_err(|e| e.to_string())?;
    let scen_dir = scenarios_dir(&data);
    let (mut steps, workout_name) = match (workout_id, id) {
        (Some(w), _) => {
            let w = workout::resolve(&w, &workouts_dir(&data))?;
            let steps = w
                .steps()
                .into_iter()
                .map(|s| {
                    let scenario = rustyaim_sim::library::resolve(&s.scenario, &scen_dir)?;
                    let label = (s.of > 1).then(|| format!("run {} of {}", s.run, s.of));
                    Ok(PlayStep { scenario, adaptive: s.mode == Mode::Adaptive, label })
                })
                .collect::<Result<Vec<_>, String>>()?;
            (steps, Some(w.name))
        }
        (None, Some(id)) => {
            let scenario = rustyaim_sim::library::resolve(&id, &scen_dir)?;
            (vec![PlayStep { scenario, adaptive, label: None }], None)
        }
        (None, None) => return Err("give a scenario (--play builtin:timed-flicks) or a workout (--workout …)".into()),
    };
    let db_path = match db {
        Some(p) => p,
        None => rustyaim_core::db::default_path().map_err(|e| e.to_string())?,
    };
    let db_handle = rustyaim_core::Db::open(&db_path).ok();
    let mut settings = load_settings(&data, db_handle.as_ref(), profile.as_deref())?;
    drop(db_handle);
    if windowed || smoke.is_some() {
        settings.display.mode = DisplayMode::Windowed;
        if smoke.is_some() {
            settings.display.resolution = Some([1600, 900]);
        }
    }
    if let Some(dir) = &smoke {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        for s in &mut steps {
            s.scenario.timing.countdown_s = 0.5;
            s.scenario.timing.duration_s = smoke_seconds;
        }
        settings.show_fps = true;
    }
    let seed = seed.unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
    });
    run(PlayOptions { steps, workout: workout_name, settings, db_path: save.then_some(db_path), seed, smoke });
    Ok(())
}
