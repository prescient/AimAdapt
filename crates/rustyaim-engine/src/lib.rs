//! RustyAim's built-in aim trainer. Runs a scenario in its own full-screen window
//! (Bevy), with raw mouse input and no frame cap, and saves each finished run to the
//! shared database.
//!
//! The desktop app starts it by running its own executable with `--play <id>`, so
//! the whole program stays a single file.

mod config;
mod game;
pub mod record;

use std::path::PathBuf;

pub use config::{from_kovaaks, load as load_settings, save as save_settings, scenarios_dir, settings_path};
pub use game::{PlayOptions, run};

/// Command-line entry point: `--play <scenario id or .toml path> [--db <file>]
/// [--windowed] [--no-save] [--seed <n>] [--smoke <folder>]`. Returns the process
/// exit code. `--smoke` runs a short bot-played version of the scenario, saves
/// `play.png` and `results.png` in the folder and exits; it's for checking the
/// engine works on a machine.
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
    let mut db = None;
    let mut windowed = false;
    let mut save = true;
    let mut seed = None;
    let mut smoke = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--play" => id = it.next().cloned(),
            "--db" => db = it.next().map(PathBuf::from),
            "--windowed" => windowed = true,
            "--no-save" => save = false,
            "--seed" => seed = it.next().and_then(|s| s.parse().ok()),
            "--smoke" => smoke = it.next().map(PathBuf::from),
            other if id.is_none() && !other.starts_with("--") => id = Some(other.to_string()),
            _ => {}
        }
    }
    let id = id.ok_or("give a scenario to play, like --play builtin:static-grid")?;
    let data = rustyaim_core::db::data_dir().map_err(|e| e.to_string())?;
    let mut scenario = rustyaim_sim::library::resolve(&id, &scenarios_dir(&data))?;
    let db_path = match db {
        Some(p) => p,
        None => rustyaim_core::db::default_path().map_err(|e| e.to_string())?,
    };
    let db_handle = rustyaim_core::Db::open(&db_path).ok();
    let mut settings = load_settings(&data, db_handle.as_ref());
    drop(db_handle);
    if windowed || smoke.is_some() {
        settings.fullscreen = false;
    }
    if let Some(dir) = &smoke {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        scenario.timing.countdown_s = 0.5;
        scenario.timing.duration_s = 4.0;
        settings.show_fps = true;
    }
    let seed = seed.unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
    });
    run(PlayOptions { scenario, settings, db_path: save.then_some(db_path), seed, smoke });
    Ok(())
}
