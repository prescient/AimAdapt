//! Saving finished runs to the shared database. A run is stored as a [`RunFile`]
//! with the same field names KovaaK's uses, so every progress and training screen
//! counts RustyAim runs without special cases; `Source` marks where it came from.
//! The trainer's extra detail (adaptive rating, per-target and per-shot records,
//! tracking samples, aim trace) goes into the `engine_*` tables for Insights.
//!
//! Adaptive runs are stored under "<name> (Adaptive)" so their score (the rating,
//! in seconds or degrees per second) never mixes with standard points.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::NaiveDateTime;
use rustyaim_core::db::SOURCE_RUSTYAIM;
use rustyaim_core::parse::{Kill, WeaponTotal};
use rustyaim_core::telemetry::{EngineRun, SampleRow, ShotRow, TargetRow};
use rustyaim_core::{Db, RunFile};
use rustyaim_sim::adaptive::{Knob, Mix, Pick, Prior};
use rustyaim_sim::scenario::WeaponKind;
use rustyaim_sim::{PlayerSettings, RunResult, Scenario};

/// Mode names stored with engine runs (KovaaK's uses "Challenge").
pub const MODE: &str = "RustyAim";
pub const MODE_ADAPTIVE: &str = "RustyAim Adaptive";

pub struct RunInfo<'a> {
    pub scenario: &'a Scenario,
    pub settings: &'a PlayerSettings,
    pub result: &'a RunResult,
    pub started_at: NaiveDateTime,
    pub finished_at: NaiveDateTime,
    pub avg_fps: Option<f32>,
    pub resolution: Option<String>,
    pub workout: Option<String>,
    /// Program name and weekday id when played as part of a program.
    pub program: Option<(String, String)>,
    pub rep: Option<u32>,
    pub reps: Option<u32>,
    pub block: Option<rustyaim_sim::workout::Block>,
}

/// Name a run is stored and charted under.
pub fn history_name(scenario: &Scenario, adaptive: bool) -> String {
    if adaptive { format!("{} (Adaptive)", scenario.name) } else { scenario.name.clone() }
}

fn category(s: &Scenario) -> String {
    format!("{:?}", s.category).to_lowercase()
}

pub fn to_run_file(info: &RunInfo) -> RunFile {
    let r = info.result;
    let st = &r.stats;
    let s = info.scenario;
    let adaptive = r.adaptive.as_ref();
    let mut fields = BTreeMap::new();
    let mut put = |k: &str, v: String| {
        fields.insert(k.to_string(), v);
    };
    put("Source", SOURCE_RUSTYAIM.into());
    put("Scenario", history_name(s, adaptive.is_some()));
    put("Score", format!("{:.3}", r.score));
    put("Hit Count", st.hits.to_string());
    put("Miss Count", st.misses.to_string());
    put("Kills", st.kills.to_string());
    put("Timeouts", st.timeouts.to_string());
    put("Damage Done", format!("{:.3}", st.damage_done));
    put("Damage Possible", format!("{:.3}", st.damage_possible));
    if let Some(ttk) = r.avg_ttk_s() {
        put("Avg TTK", format!("{ttk:.3}"));
    }
    put("Challenge Start", info.started_at.format("%H:%M:%S%.3f").to_string());
    put("Play Time", format!("{:.3}", r.duration_s));
    put("Hash", s.fingerprint());
    put("Game Version", format!("RustyAim {}", env!("CARGO_PKG_VERSION")));
    put("Category", category(s));
    // Sensitivity in cm/360 so the sensitivity history compares it directly.
    put("Sens Scale", "cm/360".into());
    put("Horiz Sens", format!("{}", info.settings.cm_per_360));
    put("Vert Sens", format!("{}", info.settings.cm_per_360));
    put("DPI", info.settings.dpi.to_string());
    put("FOV", format!("{}", info.settings.fov));
    put("FOVScale", "Horizontal 16:9".into());
    if let Some(fps) = info.avg_fps {
        put("Avg FPS", format!("{fps:.1}"));
    }
    if let Some(res) = &info.resolution {
        put("Resolution", res.clone());
    }
    if let Some(a) = adaptive {
        put("Adaptive Knob", knob_id(a.knob).into());
        put("Rating", format!("{:.4}", a.value));
        put("Start Rating", format!("{:.4}", a.start_value));
        put("Run", a.pick.as_str().into());
        if let Some(v) = a.run_value {
            put("Run Value", format!("{v:.4}"));
        }
        if let Some(f) = a.fastest {
            put("Held Goal At", format!("{f:.4}"));
        }
        put("Goal", format!("{}", a.goal));
        if let Some(rate) = a.success_rate {
            put("Success Rate", format!("{rate:.4}"));
        }
    }
    if let Some(w) = &info.workout {
        put("Workout", w.clone());
    }

    let weapon = match s.weapon.kind {
        WeaponKind::Hitscan => "Hitscan",
        WeaponKind::Beam => "Beam",
    };
    let kills = r
        .kills
        .iter()
        .zip(1..)
        .map(|(k, n)| Kill {
            kill_no: n,
            timestamp: None,
            bot: Some("Target".into()),
            weapon: Some(weapon.into()),
            ttk_s: Some(f64::from(k.ttk_s)),
            shots: Some(i64::from(k.shots)),
            hits: Some(i64::from(k.hits)),
            damage_done: Some(f64::from(k.damage)),
            damage_possible: None,
            cheated: Some(false),
            overshots: None,
        })
        .collect();

    RunFile {
        // Unique per run; never a real file.
        file_name: format!(
            "rustyaim/{}{}/{}",
            rustyaim_sim::library::slug(&s.name),
            if adaptive.is_some() { "-adaptive" } else { "" },
            info.finished_at.format("%Y-%m-%dT%H-%M-%S%.3f")
        ),
        scenario: history_name(s, adaptive.is_some()),
        mode: if adaptive.is_some() { MODE_ADAPTIVE } else { MODE }.into(),
        finished_at: info.finished_at,
        fields,
        kills,
        weapons: vec![WeaponTotal {
            weapon: weapon.into(),
            shots: Some(i64::from(st.shots)),
            hits: Some(i64::from(st.hits)),
            damage_done: Some(f64::from(st.damage_done)),
            damage_possible: Some(f64::from(st.damage_possible)),
        }],
    }
}

/// The trainer's detailed records for a run, for the `engine_*` tables.
pub fn to_engine_run(info: &RunInfo) -> EngineRun {
    let r = info.result;
    let tel = &r.telemetry;
    let a = r.adaptive.as_ref();
    let f = |v: f32| f64::from(v);
    EngineRun {
        base_scenario: info.scenario.name.clone(),
        category: category(info.scenario),
        adaptive: a.is_some(),
        knob: a.map(|a| knob_id(a.knob).into()),
        rating: a.map(|a| f(a.value)),
        rating_low: a.map(|a| f(a.range.0.min(a.range.1))),
        rating_high: a.map(|a| f(a.range.0.max(a.range.1))),
        start_rating: a.map(|a| f(a.start_value)),
        theta: a.map(|a| f(a.theta)),
        slope: a.map(|a| f(a.slope)),
        rating_sd: a.map(|a| f(a.sd)),
        fastest: a.and_then(|a| a.fastest).map(f),
        run_value: a.and_then(|a| a.run_value).map(f),
        run_pick: a.map(|a| a.pick.as_str().into()),
        planned_chance: a.and_then(|a| a.planned_chance).map(f),
        mix: a.map(|_| mix_id(info.settings.challenge).into()),
        goal: a.map(|a| f(a.goal)),
        success_rate: a.and_then(|a| a.success_rate).map(f),
        rating_timeline: a.map(|a| a.timeline.iter().map(|(t, l)| (f(*t), f(*l))).collect()).unwrap_or_default(),
        workout: info.workout.clone(),
        skill: Some(info.scenario.skill).filter(|s| *s != rustyaim_sim::scenario::Skill::None).map(|s| s.id().into()),
        program: info.program.as_ref().map(|p| p.0.clone()),
        program_day: info.program.as_ref().map(|p| p.1.clone()),
        rep: info.rep.map(i64::from),
        reps: info.reps.map(i64::from),
        block: info.block.filter(|b| *b != rustyaim_sim::workout::Block::None).map(|b| b.id().into()),
        trace: tel.trace_bytes(),
        targets: tel
            .targets
            .iter()
            .map(|t| TargetRow {
                no: i64::from(t.no),
                spawned_at: f(t.spawned_at),
                start_at: f(t.start_at),
                ended_at: f(t.ended_at),
                outcome: t.outcome.as_str().into(),
                distance_deg: f(t.distance_deg),
                direction_deg: f(t.direction_deg),
                radius_deg: f(t.radius_deg),
                lifetime_s: f(t.lifetime_s),
                reaction_s: t.reaction_s.map(f),
                reach_s: t.reach_s.map(f),
                overshoot_deg: f(t.overshoot_deg),
                entries: i64::from(t.entries),
                shots: i64::from(t.shots),
                hits: i64::from(t.hits),
                value: t.value.map(f),
                chance: t.chance.map(f),
                pick: t.pick.map(String::from),
            })
            .collect(),
        shots: tel
            .shots
            .iter()
            .map(|s| ShotRow {
                t: f(s.t),
                hit: s.hit,
                target_no: s.target_no.map(i64::from),
                dx_deg: f(s.dx_deg),
                dy_deg: f(s.dy_deg),
                along: f(s.along),
                across: f(s.across),
            })
            .collect(),
        samples: tel
            .samples
            .iter()
            .map(|s| SampleRow {
                t: f(s.t),
                on_target: s.on_target,
                error_deg: f(s.error_deg),
                target_speed_dps: f(s.target_speed_dps),
                lead_deg: f(s.lead_deg),
                since_change_s: f(s.since_change_s),
                engaged: s.engaged,
            })
            .collect(),
    }
}

/// Best earlier standard score for exactly this scenario (same name and settings).
pub fn best_score(db: &Db, scenario: &Scenario) -> Option<f64> {
    db.conn()
        .query_row(
            "SELECT MAX(score) FROM runs WHERE source = ?1 AND scenario = ?2 AND scenario_hash = ?3",
            (SOURCE_RUSTYAIM, &scenario.name, scenario.fingerprint()),
            |r| r.get::<_, Option<f64>>(0),
        )
        .ok()
        .flatten()
}

pub fn knob_id(k: Knob) -> &'static str {
    match k {
        Knob::Lifetime => "lifetime",
        Knob::TargetSpeed => "target_speed",
        Knob::KillPace => "kill_pace",
        Knob::Radius | Knob::Auto => "radius",
    }
}

fn mix_id(m: Mix) -> &'static str {
    match m {
        Mix::Steady => "steady",
        Mix::Balanced => "balanced",
        Mix::Push => "push",
    }
}

/// The rating to start an adaptive run from: the last one for this scenario with
/// the same knob, aged by the days since, and the best value ever hit.
pub fn prior(db: &Db, scenario: &Scenario, now: chrono::NaiveDateTime) -> Option<Prior> {
    let knob = knob_id(scenario.adaptive.resolve(scenario).knob);
    let last = db.last_rating(&scenario.name).ok()??;
    if last.knob != knob {
        return None;
    }
    let then = chrono::NaiveDateTime::parse_from_str(&last.finished_at, "%Y-%m-%dT%H:%M:%S%.3f").ok()?;
    let days = ((now - then).num_minutes() as f32 / 1440.0).max(0.0);
    let fastest = db.fastest_hit(&scenario.name, knob).ok().flatten().map(|v| v as f32);
    let last_run = last.run_pick.as_deref().and_then(Pick::parse).zip(last.success_rate.map(|r| r as f32));
    Some(Prior { theta: last.theta as f32, slope: last.slope as f32, sd: last.sd as f32, days, fastest, last_run })
}

/// The best rating so far for a scenario's knob (lowest lifetime or size, highest speed).
pub fn best_rating(db: &Db, scenario: &Scenario) -> Option<f64> {
    let knob = scenario.adaptive.resolve(scenario).knob;
    let agg = if knob.higher_is_harder() { "MAX" } else { "MIN" };
    db.conn()
        .query_row(
            &format!("SELECT {agg}(rating) FROM engine_runs WHERE base_scenario = ?1 AND knob = ?2"),
            [&scenario.name, knob_id(knob)],
            |r| r.get::<_, Option<f64>>(0),
        )
        .ok()
        .flatten()
}

pub fn save(db_path: &Path, info: &RunInfo) -> Result<(), String> {
    let mut db = Db::open(db_path).map_err(|e| e.to_string())?;
    let now = chrono::Local::now().naive_local();
    let run = to_run_file(info);
    db.insert_runs(std::slice::from_ref(&run), now).map_err(|e| e.to_string())?;
    db.save_engine_run(&run.file_name, &to_engine_run(info)).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyaim_sim::sim::{Input, Phase, Sim};

    fn finished() -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(7, 30, 0).unwrap()
    }

    /// Plays a short run with a bot that kills every target straight away.
    fn play(scenario: &Scenario, adaptive: bool, prior: Option<Prior>) -> RunResult {
        let mut s = scenario.clone();
        s.timing.countdown_s = 0.0;
        s.timing.duration_s = 10.0;
        let mut sim = if adaptive { Sim::adaptive(s, 7, prior, Mix::Balanced) } else { Sim::new(s, 7) };
        while sim.phase() != Phase::Finished {
            let aim = sim.targets()[0].pos.normalize();
            sim.step(1.0 / 240.0, Input { aim, fire_pressed: true, fire_held: true });
            sim.step(1.0 / 240.0, Input { aim, fire_pressed: false, fire_held: false });
        }
        sim.result()
    }

    fn info<'a>(scenario: &'a Scenario, result: &'a RunResult, settings: &'a PlayerSettings, n: i64) -> RunInfo<'a> {
        let at = finished() + chrono::Duration::minutes(n);
        RunInfo {
            scenario,
            settings,
            result,
            started_at: at - chrono::Duration::seconds(10),
            finished_at: at,
            avg_fps: Some(500.0),
            resolution: Some("2560x1440".into()),
            workout: (n == 2).then(|| "Daily Fundamentals".into()),
            program: (n == 2).then(|| ("Core Week".into(), "mon".into())),
            rep: Some(1),
            reps: Some(5),
            block: Some(rustyaim_sim::workout::Block::Main),
        }
    }

    #[test]
    fn engine_runs_show_up_like_kovaaks_runs() {
        let scenario = Scenario { name: "Test Grid".into(), ..Default::default() };
        let settings = PlayerSettings::default();
        let result = play(&scenario, false, None);
        let run = to_run_file(&info(&scenario, &result, &settings, 0));
        assert_eq!(run.score(), Some(f64::from(result.score)));
        assert_eq!(run.accuracy(), Some(1.0));
        assert_eq!(run.duration_s(), Some(10.0));

        let mut db = Db::open_in_memory().unwrap();
        assert_eq!(db.insert_runs(std::slice::from_ref(&run), finished()).unwrap(), 1);
        assert_eq!(db.insert_runs(&[run], finished()).unwrap(), 0, "same run saved twice is ignored");
        let source: String = db.conn().query_row("SELECT source FROM runs", [], |r| r.get(0)).unwrap();
        assert_eq!(source, SOURCE_RUSTYAIM);
        assert_eq!(best_score(&db, &scenario), Some(f64::from(result.score)));
        assert!(db.latest_kovaaks_settings().unwrap().is_none(), "engine runs aren't KovaaK's settings");
        let mut harder = scenario.clone();
        harder.targets.radius = 0.2;
        assert_eq!(best_score(&db, &harder), None, "variants keep separate bests");
    }

    #[test]
    fn adaptive_runs_save_rating_and_telemetry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let scenario = Scenario { name: "Timed Flicks".into(), ..Default::default() };
        let settings = PlayerSettings::default();

        let first = play(&scenario, true, None);
        save(&path, &info(&scenario, &first, &settings, 1)).unwrap();
        let db = Db::open(&path).unwrap();
        let rating = db.last_rating("Timed Flicks").unwrap().unwrap();
        assert_eq!(rating.knob, "lifetime");
        assert!((rating.rating - f64::from(first.adaptive.as_ref().unwrap().value)).abs() < 1e-3);

        // The next run starts from the saved rating, a day later.
        let p = prior(&db, &scenario, finished() + chrono::Duration::days(1) + chrono::Duration::minutes(1)).unwrap();
        assert!((p.days - 1.0).abs() < 0.01, "{}", p.days);
        assert!(first.adaptive.as_ref().unwrap().calibration(), "the first adaptive run calibrates");
        assert_eq!(p.fastest, None, "a calibration run sets no best");
        assert_eq!(p.last_run.map(|r| r.0), Some(Pick::Calibrate));
        let second = play(&scenario, true, Some(p));
        let a2 = second.adaptive.as_ref().unwrap();
        let start = a2.start_value;
        assert!((f64::from(start) / rating.rating - 1.0).abs() < 0.05, "{start} vs {}", rating.rating);
        assert!(a2.run_value.is_some(), "later runs play at one value");
        save(&path, &info(&scenario, &second, &settings, 2)).unwrap();
        let db = Db::open(&path).unwrap();
        let p = prior(&db, &scenario, finished() + chrono::Duration::days(2)).unwrap();
        assert_eq!(p.last_run.map(|r| r.0), Some(a2.pick), "the guard rails see the last run's pick");
        let (run_value, run_pick): (Option<f64>, Option<String>) = db
            .conn()
            .query_row("SELECT run_value, run_pick FROM engine_runs ORDER BY run_id DESC LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(run_value.map(|v| v as f32), a2.run_value);
        assert_eq!(run_pick.as_deref(), Some(a2.pick.as_str()));
        let standard = play(&scenario, false, None);
        save(&path, &info(&scenario, &standard, &settings, 3)).unwrap();

        let db = Db::open(&path).unwrap();
        let names: Vec<String> = db
            .conn()
            .prepare("SELECT scenario FROM runs ORDER BY finished_at")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(names, ["Timed Flicks (Adaptive)", "Timed Flicks (Adaptive)", "Timed Flicks"]);
        let targets: i64 = db.conn().query_row("SELECT COUNT(*) FROM engine_targets", [], |r| r.get(0)).unwrap();
        assert!(targets > 30, "{targets}");
        let trace: Vec<u8> = db.conn().query_row("SELECT trace FROM engine_runs LIMIT 1", [], |r| r.get(0)).unwrap();
        assert!(trace.len() > 1000);
        let workout: Option<String> = db
            .conn()
            .query_row("SELECT workout FROM engine_runs ORDER BY run_id LIMIT 1 OFFSET 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(workout.as_deref(), Some("Daily Fundamentals"));

        let insights = rustyaim_core::insights::scenario(&db, "Timed Flicks").unwrap().unwrap();
        assert_eq!(insights.runs.len(), 3, "both modes group under one scenario");
        assert!(insights.fitts.is_some() && !insights.directions.is_empty());
        let best = best_rating(&db, &scenario).unwrap();
        assert!(best <= rating.rating + 1e-9, "lower lifetime is better: {best}");
    }
}
