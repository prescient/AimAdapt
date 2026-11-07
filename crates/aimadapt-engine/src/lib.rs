//! AimAdapt's built-in aim trainer. Runs a scenario or a whole workout in its own
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

use aimadapt_sim::program::{self, Program, Weekday};
use aimadapt_sim::scenario::Skill;
use aimadapt_sim::settings::DisplayMode;
use aimadapt_sim::workout::{self, Mode, Workout};
use chrono::Datelike;

pub use config::{
    from_kovaaks, kovaaks_profile, load as load_settings, load_profiles, save_profiles, scenarios_dir, settings_path,
    workouts_dir,
};
pub use game::{PlayOptions, PlayStep, run};

/// Command-line entry point. Returns the process exit code.
///
/// - `--play <scenario id or .toml path> [--adaptive]`: one scenario.
/// - `--workout <workout id or .toml path>`: every run of a workout in turn.
/// - `--program <id> [--day mon…sun]`: a program day (today by default).
/// - `--sce <file>`: a KovaaK's scenario file, on its own map (standard mode).
///
/// Options: `--profile <name>` (default: the active settings profile), `--db <file>`, `--windowed`, `--no-save`, `--seed <n>`, and
/// `--smoke <folder>`, which has a bot play short versions of the runs
/// (`--smoke-seconds`, default 4), saves screenshots in the folder and exits; it's
/// for checking the engine works.
/// Development options that must come with `--db`.
pub const DEV_FLAGS: &[&str] = &["--smoke"];

pub fn run_from_args(args: &[String]) -> i32 {
    if let Err(e) = aimadapt_core::db::require_test_db(args, DEV_FLAGS) {
        eprintln!("aimadapt: {e}");
        return 2;
    }
    match parse_and_run(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("aimadapt: {e}");
            2
        }
    }
}

fn parse_and_run(args: &[String]) -> Result<(), String> {
    let mut id = None;
    let mut sce_path: Option<PathBuf> = None;
    let mut workout_id = None;
    let mut program_id = None;
    let mut day_id = None;
    let mut adaptive = false;
    let mut profile = None;
    let mut db = None;
    let mut windowed = false;
    let mut save = true;
    let mut seed = None;
    let mut smoke = None;
    let mut smoke_seconds = 4.0;
    let mut anchor = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--play" => id = it.next().cloned(),
            "--sce" => sce_path = it.next().map(PathBuf::from),
            "--workout" => workout_id = it.next().cloned(),
            "--program" => program_id = it.next().cloned(),
            "--day" => day_id = it.next().cloned(),
            "--adaptive" => adaptive = true,
            "--profile" => profile = it.next().cloned(),
            "--db" => db = it.next().map(PathBuf::from),
            "--windowed" => windowed = true,
            "--no-save" => save = false,
            "--seed" => seed = it.next().and_then(|s| s.parse().ok()),
            "--smoke" => smoke = it.next().map(PathBuf::from),
            "--smoke-seconds" => smoke_seconds = it.next().and_then(|s| s.parse().ok()).unwrap_or(4.0),
            "--anchor" => anchor = it.next().and_then(|s| parse_anchor(s)),
            other if id.is_none() && !other.starts_with("--") => id = Some(other.to_string()),
            _ => {}
        }
    }
    let data = aimadapt_core::db::data_dir().map_err(|e| e.to_string())?;
    let scen_dir = scenarios_dir(&data);
    let db_path = match db {
        Some(p) => p,
        None => aimadapt_core::db::default_path().map_err(|e| e.to_string())?,
    };
    let to_steps = |w: &Workout| {
        w.steps()
            .into_iter()
            .map(|s| {
                let scenario = aimadapt_sim::library::resolve(&s.scenario, &scen_dir)?;
                let label = (s.of > 1).then(|| format!("run {} of {}", s.run, s.of));
                Ok(PlayStep {
                    scenario,
                    adaptive: s.mode == Mode::Adaptive,
                    label,
                    rep: s.run,
                    reps: s.of,
                    block: s.block,
                })
            })
            .collect::<Result<Vec<_>, String>>()
    };
    let mut program_day = None;
    let (mut steps, workout_name) = match (program_id, workout_id, id) {
        // A KovaaK's scenario file, played on its own map (standard mode for now).
        _ if sce_path.is_some() => {
            let path = sce_path.as_deref().unwrap_or(std::path::Path::new(""));
            if adaptive {
                return Err("imported KovaaK's scenarios play in standard mode for now".into());
            }
            let imported = aimadapt_sce::import(path).map_err(|r| format!("can't play this yet: {}", r.join("; ")))?;
            for n in &imported.notes {
                eprintln!("aimadapt: note: {n}");
            }
            let step = PlayStep {
                scenario: imported.scenario,
                adaptive: false,
                label: None,
                rep: 1,
                reps: 1,
                block: workout::Block::None,
            };
            (vec![step], None)
        }
        (Some(p), _, _) => {
            let prog = program::resolve(&p)?;
            let day = match day_id {
                Some(d) => Weekday::from_id(&d).ok_or(format!("unknown day {d}"))?,
                None => today(),
            };
            let db_handle = aimadapt_core::Db::open(&db_path).ok();
            let w = workout_for_day(&prog, day, db_handle.as_ref(), &workouts_dir(&data), training_week())?;
            program_day = Some((prog.name.clone(), day.id().to_string()));
            (to_steps(&w)?, Some(w.name))
        }
        (None, Some(w), _) => {
            let w = workout::resolve(&w, &workouts_dir(&data))?;
            (to_steps(&w)?, Some(w.name))
        }
        (None, None, Some(id)) => {
            let scenario = aimadapt_sim::library::resolve(&id, &scen_dir)?;
            let step = PlayStep { scenario, adaptive, label: None, rep: 1, reps: 1, block: workout::Block::None };
            (vec![step], None)
        }
        (None, None, None) => {
            return Err(
                "give a scenario (--play builtin:timed-flicks), a workout (--workout …) or a program (--program …)"
                    .into(),
            );
        }
    };
    let db_handle = aimadapt_core::Db::open(&db_path).ok();
    let mut settings = load_settings(&data, db_handle.as_ref(), profile.as_deref())?;
    drop(db_handle);
    // An imported scenario can keep the field of view within a range, as KovaaK's does.
    if let Some([lo, hi]) = steps.first().and_then(|s| s.scenario.world.as_ref()).and_then(|w| w.fov_range) {
        settings.fov = settings.fov.clamp(lo, hi);
    }
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
    run(PlayOptions {
        steps,
        workout: workout_name,
        program: program_day,
        settings,
        db_path: save.then_some(db_path),
        seed,
        anchor: if smoke.is_some() { None } else { anchor },
        smoke,
    });
    Ok(())
}

/// `--anchor x,y,width`: the app window's outer position and width in physical
/// pixels, so the trainer opens over it.
pub fn parse_anchor(s: &str) -> Option<[i32; 3]> {
    let v: Vec<i32> = s.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    match v[..] {
        [x, y, w] if w > 0 => Some([x, y, w]),
        _ => None,
    }
}

/// Today's weekday, with days starting at 4 AM like the rest of AimAdapt.
pub fn today() -> Weekday {
    let now = chrono::Local::now().naive_local() - chrono::Duration::hours(4);
    Weekday::from_index(now.weekday().num_days_from_monday() as usize)
}

/// The ISO week of the current training day (which starts at 4 AM), for rotating
/// drill variants.
pub fn training_week() -> u32 {
    (chrono::Local::now().naive_local() - chrono::Duration::hours(4)).iso_week().week()
}

/// The workout for a program day in a given week: its own workout, or a
/// weak-spots workout built from the skill profile (untrained skills count as
/// weakest), with each drill's variant for that week.
pub fn workout_for_day(
    prog: &Program,
    day: Weekday,
    db: Option<&aimadapt_core::Db>,
    workouts: &std::path::Path,
    week: u32,
) -> Result<Workout, String> {
    workout_for_day_any_week(prog, day, db, workouts).map(|w| w.for_week(week))
}

fn workout_for_day_any_week(
    prog: &Program,
    day: Weekday,
    db: Option<&aimadapt_core::Db>,
    workouts: &std::path::Path,
) -> Result<Workout, String> {
    let d = prog.day(day).ok_or(format!("{} is a rest day in {}", day.name(), prog.name))?;
    if d.weak_spots {
        let ranked: Vec<Skill> = match db {
            Some(db) => aimadapt_core::programs::skill_profile(db, None)
                .map_err(|e| e.to_string())?
                .ranked()
                .iter()
                .filter_map(|id| Skill::from_id(id))
                .collect(),
            None => Skill::ALL.to_vec(),
        };
        return Ok(program::weak_spots(&format!("{}: {}", prog.name, day.name()), &ranked));
    }
    let id = d.workout.as_deref().unwrap_or_default();
    workout::resolve(id, workouts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_days_resolve_to_workouts() {
        let prog = program::resolve("builtin:core-week").unwrap();
        let none = std::path::Path::new("");
        let mon = workout_for_day(&prog, Weekday::Mon, None, none, 0).unwrap();
        assert_eq!(mon.name, "Core Week: Monday");
        assert!(workout_for_day(&prog, Weekday::Sun, None, none, 0).is_err(), "Sunday is rest");

        // With nothing trained, Saturday drills the first skills in order.
        let db = aimadapt_core::Db::open_in_memory().unwrap();
        let tue = |week| workout_for_day(&prog, Weekday::Tue, None, none, week).unwrap();
        assert_eq!(tue(44).items[1].scenario, "builtin:strafe-track");
        assert_eq!(tue(45).items[1].scenario, "builtin:hop-strafe-track", "this week's variant");
        assert!(tue(45).items.iter().all(|i| i.variants.is_empty()), "a played workout is resolved");
        let sat = workout_for_day(&prog, Weekday::Sat, Some(&db), none, 0).unwrap();
        assert_eq!(sat.items[1].scenario, program::drill(Skill::StaticClicking));
        assert_eq!(sat.total_runs(), 23);
        for item in &sat.items {
            aimadapt_sim::library::resolve(&item.scenario, none).unwrap();
        }
    }

    /// The trainer opens over the app window: centered across it, tops level.
    #[test]
    fn trainer_opens_centered_on_the_app_window() {
        assert_eq!(parse_anchor("100,50,2400"), Some([100, 50, 2400]));
        assert_eq!(parse_anchor("-1920, 0, 1600"), Some([-1920, 0, 1600]), "a monitor left of the main one");
        assert_eq!(parse_anchor("1,2"), None);
        assert_eq!(parse_anchor("1,2,0"), None);
        assert_eq!(parse_anchor("a,b,c"), None);
        assert_eq!(game::window_position([100, 50, 2400], 2120), [240, 50]);
        // A game window wider than the app still centers on it.
        assert_eq!(game::window_position([500, 80, 1200], 2120), [40, 80]);
    }

    /// Bot runs must never land in a player's history, so `--smoke` needs `--db`.
    #[test]
    fn smoke_runs_need_a_test_database() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(run_from_args(&a(&["--play", "builtin:timed-flicks", "--smoke", "shots"])), 2);
        assert!(aimadapt_core::db::require_test_db(&a(&["--smoke", "shots", "--db", "t.db"]), DEV_FLAGS).is_ok());
        assert!(aimadapt_core::db::require_test_db(&a(&["--play", "builtin:timed-flicks"]), DEV_FLAGS).is_ok());
    }
}
