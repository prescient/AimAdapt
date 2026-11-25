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
mod results;
mod sound;

pub use sound::{Shot, load_shot, shot_path};

use std::path::PathBuf;

use aimadapt_sim::program::{self, Program, Weekday};
use aimadapt_sim::scenario::Skill;
use aimadapt_sim::settings::DisplayMode;
use aimadapt_sim::workout::{self, Mode, Workout};
use chrono::Datelike;

pub use config::{
    from_kovaaks, kovaaks_profile, load as load_settings, load_profiles, profiles_note, save_profiles, scenarios_dir,
    settings_path, workouts_dir,
};
pub use game::{PlayOptions, PlayStep, run};
pub use results::Open;

/// Command-line entry point. Returns the process exit code.
///
/// - `--play <scenario id or .toml path> [--adaptive]`: one scenario.
/// - `--workout <workout id or .toml path>`: every run of a workout in turn.
/// - `--program <id> [--day mon…sun]`: a program day (today by default).
/// - `--sce <file> [--adaptive]`: a KovaaK's scenario file, on its own map.
///
/// Options: `--profile <name>` (default: the active settings profile), `--db <file>`, `--windowed`, `--no-save`, `--seed <n>`,
/// `--handoff` (the app is waiting: the results screen offers keys that open a page in it), and
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
    let mut handoff = false;
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
            "--handoff" => handoff = true,
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
    // KovaaK's scenarios in a workout are found in your KovaaK's folders; one that can't play is left out.
    let to_steps = |w: &Workout| {
        let (steps, skipped) = workout_steps(w, &scen_dir, &aimadapt_core::discover::find_scenario_dirs())?;
        for s in &skipped {
            eprintln!("aimadapt: left out of {}: {s}", w.name);
        }
        Ok::<_, String>(steps)
    };
    let mut program_day = None;
    let (mut steps, workout_name) = match (program_id, workout_id, id) {
        // A KovaaK's scenario file, played on its own map.
        _ if sce_path.is_some() => {
            let path = sce_path.as_deref().unwrap_or(std::path::Path::new(""));
            (vec![sce_step(path, adaptive)?], None)
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
    // An imported scenario can keep the field of view within a range; each run applies its own (see `game`).
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
    // The player's own shot sound, or the pop. One that can't be played plays the pop.
    let shot = match sound::load_shot(&data) {
        Some(Ok(shot)) => Some(shot),
        Some(Err(e)) => {
            eprintln!("aimadapt: {} can't be played ({e}); shots pop instead", shot_path(&data).display());
            None
        }
        None => None,
    };
    run(PlayOptions {
        steps,
        workout: workout_name,
        program: program_day,
        settings,
        db_path: save.then_some(db_path),
        seed,
        anchor: if smoke.is_some() { None } else { anchor },
        smoke,
        handoff,
        shot,
    });
    Ok(())
}

/// A workout's runs, in order, and what was left out of it. An item `kovaaks:<name>` plays your own
/// KovaaK's file of that name from these folders. One that isn't there, or that the trainer can't play
/// yet, is left out and named in the second list, so the rest of the workout still plays. Any other
/// scenario that can't be found is an error, and so is a workout with nothing left to play.
pub fn workout_steps(
    w: &Workout,
    scenarios: &std::path::Path,
    kovaaks_dirs: &[PathBuf],
) -> Result<(Vec<PlayStep>, Vec<String>), String> {
    // The folders are only read when an item needs them.
    let mut files: Option<Vec<PathBuf>> = None;
    let mut steps = Vec::new();
    let mut skipped = Vec::new();
    for item in &w.items {
        let scenario = match workout::kovaaks_name(&item.scenario) {
            None => aimadapt_sim::library::resolve(&item.scenario, scenarios)?,
            Some(name) => {
                let files = files.get_or_insert_with(|| aimadapt_sce::find_files(kovaaks_dirs));
                let Some(path) = aimadapt_sce::file_named(name, files) else {
                    skipped.push(format!("{name}: not in your KovaaK's folders"));
                    continue;
                };
                match sce_step(&path, false) {
                    Ok(step) => step.scenario,
                    Err(e) => {
                        skipped.push(format!("{name}: {e}"));
                        continue;
                    }
                }
            }
        };
        for run in 1..=item.runs {
            steps.push(PlayStep {
                scenario: scenario.clone(),
                adaptive: item.mode == Mode::Adaptive,
                label: (item.runs > 1).then(|| format!("run {run} of {}", item.runs)),
                rep: run,
                reps: item.runs,
                block: item.block,
            });
        }
    }
    if steps.is_empty() {
        return Err(format!("nothing in {} can play here ({})", w.name, skipped.join("; ")));
    }
    Ok((steps, skipped))
}

/// The run of a KovaaK's scenario file: standard, or adaptive on the knob its kind gets (speed or kill
/// pace, see `aimadapt_sim::adaptive::imported_rule`). The importer's notes go to the console.
fn sce_step(path: &std::path::Path, adaptive: bool) -> Result<PlayStep, String> {
    let imported = aimadapt_sce::import(path).map_err(|r| format!("can't play this yet: {}", join(&r)))?;
    for n in &imported.notes {
        eprintln!("aimadapt: note: {n}");
    }
    Ok(PlayStep { scenario: imported.scenario, adaptive, label: None, rep: 1, reps: 1, block: workout::Block::None })
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
/// weak-spots workout built from the skill profile (the skills least trained first,
/// then the least improved), with each drill's variant for that week.
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
            Some(db) => {
                aimadapt_core::programs::skill_profile(db, None, &aimadapt_core::training::TimeSettings::default())
                    .map_err(|e| e.to_string())?
                    .ranked()
                    .iter()
                    .filter_map(|id| Skill::from_id(id))
                    .collect()
            }
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

    /// Phase 5 (BACKLOG 9): `--sce` with `--adaptive` was refused ("imported KovaaK's scenarios play in
    /// standard mode for now"). It now plays the file adaptively, on the knob its kind gets.
    #[test]
    fn a_kovaaks_file_plays_adaptively() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../aimadapt-sce/tests/fixtures");
        for (name, knob) in [("Slow Strafe", "time_scale"), ("Slow Taps", "kill_pace")] {
            let step = sce_step(&dir.join(format!("AimAdapt Test - {name}.sce")), true).unwrap();
            assert!(step.adaptive, "{name}");
            let s = &step.scenario;
            assert_eq!(record::knob_id(s.adaptive.resolve(s).knob), knob, "{name}");
        }
        let standard = sce_step(&dir.join("AimAdapt Test - Slow Strafe.sce"), false).unwrap();
        assert!(!standard.adaptive);
        assert!(sce_step(&dir.join("AimAdapt Test - Slow Gate.sce"), true).is_err(), "a file that can't play");
    }

    /// A workout can list KovaaK's scenarios by name (the Viscose Easier workouts do). Each plays your own
    /// file, and one you haven't downloaded or that can't play yet is left out, so the rest still play.
    #[test]
    fn a_workout_plays_your_kovaaks_files_and_leaves_out_the_rest() {
        let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../aimadapt-sce/tests/fixtures")];
        let none = std::path::Path::new("");
        let w = Workout::from_toml(
            r#"name = "Mixed"
[[items]]
scenario = "builtin:timed-flicks"
[[items]]
scenario = "kovaaks:aimadapt test - slow strafe"
runs = 2
[[items]]
scenario = "kovaaks:AimAdapt Test - Slow Gate"
[[items]]
scenario = "kovaaks:Not Downloaded"
mode = "standard""#,
        )
        .unwrap();
        let (steps, skipped) = workout_steps(&w, none, &dirs).unwrap();
        let names: Vec<&str> = steps.iter().map(|s| s.scenario.name.as_str()).collect();
        let strafe = aimadapt_sce::import(&dirs[0].join("AimAdapt Test - Slow Strafe.sce")).unwrap().scenario;
        assert_eq!(names, ["Timed Flicks", strafe.name.as_str(), strafe.name.as_str()]);
        assert_eq!(steps[1].scenario, strafe, "the file as imported");
        assert!(steps[1].adaptive && steps[2].adaptive);
        assert_eq!((steps[2].rep, steps[2].reps, steps[2].label.as_deref()), (2, 2, Some("run 2 of 2")));
        assert_eq!(skipped.len(), 2, "{skipped:?}");
        assert!(skipped[0].starts_with("AimAdapt Test - Slow Gate: can't play this yet"), "{skipped:?}");
        assert_eq!(skipped[1], "Not Downloaded: not in your KovaaK's folders");

        // With nothing left to play it says why; an unknown built-in is still an error.
        let only = Workout::from_toml("name = \"Gone\"\n[[items]]\nscenario = \"kovaaks:Not Downloaded\"").unwrap();
        let err = workout_steps(&only, none, &dirs).unwrap_err();
        assert_eq!(err, "nothing in Gone can play here (Not Downloaded: not in your KovaaK's folders)");
        let bad = Workout::from_toml("name = \"Bad\"\n[[items]]\nscenario = \"builtin:no-such\"").unwrap();
        assert!(workout_steps(&bad, none, &dirs).is_err());
    }

    /// Every built-in workout's runs resolve, KovaaK's ones included (all left out with no folders).
    #[test]
    fn every_built_in_workout_resolves() {
        for e in workout::builtins() {
            let w = e.workout.unwrap();
            match workout_steps(&w, std::path::Path::new(""), &[]) {
                Ok((steps, skipped)) => {
                    assert!(skipped.is_empty() && steps.len() == w.total_runs() as usize, "{}", w.name)
                }
                Err(err) => {
                    assert!(w.name.starts_with("Viscose Easier") && err.starts_with("nothing in"), "{}: {err}", w.name)
                }
            }
        }
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

/// aimadapt-core can't depend on the sim, so it keeps copies of a few of its numbers
/// and lists. This crate has both, so it checks they still agree.
fn join(reasons: &[aimadapt_sce::Gap]) -> String {
    reasons.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod core_copies {
    /// Insights rebuild success chances with the sim's lapse rate and read tracking
    /// samples at its rate; wrong copies would quietly skew both.
    #[test]
    fn core_copies_of_sim_values_match() {
        assert!((aimadapt_core::insights::LAPSE - f64::from(aimadapt_sim::adaptive::LAPSE)).abs() < 1e-6);
        for chance in [0.6f32, 0.85, 0.95] {
            let sim = f64::from(aimadapt_sim::adaptive::logit_with_slips(chance));
            let core = aimadapt_core::insights::logit_with_slips(f64::from(chance));
            assert!((sim - core).abs() < 1e-5, "{chance}: {sim} vs {core}");
        }
        let sample_s = 1.0 / f64::from(aimadapt_sim::sim::SAMPLE_HZ);
        assert!((aimadapt_core::insights::SAMPLE_S - sample_s).abs() < 1e-9);
    }

    /// The plan check marks a gap with core's copy of the sim's tolerance.
    #[test]
    fn core_plan_tolerance_is_the_sims() {
        for planned in [0.05f32, 0.3, 0.55, 0.72, 0.85, 0.93, 0.97] {
            for evidence in [0.0f32, 1.0, 12.0, 40.0, 150.0, 900.0, 20_000.0] {
                let sim = f64::from(aimadapt_sim::adaptive::plan_tolerance(planned, evidence));
                let core = aimadapt_core::plan_check::tolerance(f64::from(planned), f64::from(evidence));
                assert!((sim - core).abs() < 1e-5, "{planned} {evidence}: {sim} vs {core}");
            }
        }
    }

    /// Core reads stored ratings with its own `Knob`: same ids, same direction,
    /// same units as the sim's.
    #[test]
    fn core_knobs_are_the_sims_knobs() {
        use aimadapt_sim::adaptive::Knob;
        for k in [Knob::Lifetime, Knob::TargetSpeed, Knob::KillPace, Knob::Radius, Knob::TimeScale] {
            let id = crate::record::knob_id(k);
            let core = aimadapt_core::knob::Knob::from_id(id);
            assert_eq!(core.id(), id);
            assert_eq!(core.higher_is_harder(), k.higher_is_harder(), "{id}");
            assert_eq!(core.format(0.625), k.format(0.625), "{id}");
            assert!((core.h(0.625) - f64::from(k.h(0.625))).abs() < 1e-6, "{id}");
        }
    }

    /// The Skills page groups scenarios with core's list of sub-skills.
    #[test]
    fn core_skills_are_the_sims_skills() {
        let field = aimadapt_sim::spec::FIELDS.iter().find(|f| f.path == "skill").unwrap();
        let aimadapt_sim::spec::Kind::Choice { options } = field.kind else { panic!("skill is a choice") };
        let mut sim: Vec<&str> = options.iter().map(|o| o.value).filter(|v| !v.is_empty() && *v != "none").collect();
        let mut core: Vec<&str> = aimadapt_core::programs::SKILLS.iter().map(|(id, _)| *id).collect();
        sim.sort_unstable();
        core.sort_unstable();
        assert_eq!(core, sim);
        // Each skill's group is its category: the scenario check refuses a mismatch.
        for (id, group) in aimadapt_core::programs::SKILLS {
            let text = format!("name = \"x\"\ncategory = \"{group}\"\nskill = \"{id}\"\n");
            aimadapt_sim::Scenario::from_toml(&text).unwrap_or_else(|e| panic!("{id} in {group}: {e}"));
        }
    }
}
