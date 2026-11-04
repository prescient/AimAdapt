//! Saving finished runs to the shared database. A run is stored as a [`RunFile`]
//! with the same field names KovaaK's uses, so every progress and training screen
//! counts RustyAim runs without special cases; `Source` marks where it came from.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::NaiveDateTime;
use rustyaim_core::db::SOURCE_RUSTYAIM;
use rustyaim_core::parse::{Kill, WeaponTotal};
use rustyaim_core::{Db, RunFile};
use rustyaim_sim::{PlayerSettings, RunResult, Scenario};

/// Mode name stored with engine runs (KovaaK's uses "Challenge").
pub const MODE: &str = "RustyAim";

pub struct RunInfo<'a> {
    pub scenario: &'a Scenario,
    pub settings: &'a PlayerSettings,
    pub result: &'a RunResult,
    pub started_at: NaiveDateTime,
    pub finished_at: NaiveDateTime,
    pub avg_fps: Option<f32>,
    pub resolution: Option<String>,
}

pub fn to_run_file(info: &RunInfo) -> RunFile {
    let r = info.result;
    let st = &r.stats;
    let s = info.scenario;
    let mut fields = BTreeMap::new();
    let mut put = |k: &str, v: String| {
        fields.insert(k.to_string(), v);
    };
    put("Source", SOURCE_RUSTYAIM.into());
    put("Scenario", s.name.clone());
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
    put("Category", format!("{:?}", s.category).to_lowercase());
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

    let weapon = match s.weapon.kind {
        rustyaim_sim::scenario::WeaponKind::Hitscan => "Hitscan",
        rustyaim_sim::scenario::WeaponKind::Beam => "Beam",
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
            "rustyaim/{}/{}",
            rustyaim_sim::library::slug(&s.name),
            info.finished_at.format("%Y-%m-%dT%H-%M-%S%.3f")
        ),
        scenario: s.name.clone(),
        mode: MODE.into(),
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

/// Best earlier score for exactly this scenario (same name and settings).
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

pub fn save(db_path: &Path, info: &RunInfo) -> Result<(), String> {
    let mut db = Db::open(db_path).map_err(|e| e.to_string())?;
    let now = chrono::Local::now().naive_local();
    db.insert_runs(&[to_run_file(info)], now).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyaim_sim::sim::{KillRecord, Stats};

    #[test]
    fn engine_runs_show_up_like_kovaaks_runs() {
        let scenario = Scenario { name: "Test Grid".into(), ..Default::default() };
        let settings = PlayerSettings::default();
        let result = RunResult {
            score: 1200.0,
            stats: Stats { shots: 15, hits: 12, misses: 3, kills: 12, damage_done: 1200.0, ..Default::default() },
            kills: vec![KillRecord { at_s: 1.0, ttk_s: 0.5, shots: 1, hits: 1, damage: 100.0 }; 12],
            duration_s: 60.0,
        };
        let finished = chrono::NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(7, 30, 0).unwrap();
        let started = finished - chrono::Duration::seconds(63);
        let info = RunInfo {
            scenario: &scenario,
            settings: &settings,
            result: &result,
            started_at: started,
            finished_at: finished,
            avg_fps: Some(500.0),
            resolution: Some("2560x1440".into()),
        };
        let run = to_run_file(&info);
        assert_eq!(run.score(), Some(1200.0));
        assert_eq!(run.accuracy(), Some(0.8));
        assert_eq!(run.duration_s(), Some(63.0));

        let mut db = Db::open_in_memory().unwrap();
        assert_eq!(db.insert_runs(std::slice::from_ref(&run), finished).unwrap(), 1);
        assert_eq!(db.insert_runs(&[run], finished).unwrap(), 0, "same run saved twice is ignored");
        let source: String = db.conn().query_row("SELECT source FROM runs", [], |r| r.get(0)).unwrap();
        assert_eq!(source, SOURCE_RUSTYAIM);
        assert_eq!(best_score(&db, &scenario), Some(1200.0));
        assert!(db.latest_kovaaks_settings().unwrap().is_none(), "engine runs aren't KovaaK's settings");
        let mut harder = scenario.clone();
        harder.targets.radius = 0.2;
        assert_eq!(best_score(&db, &harder), None, "variants keep separate bests");
    }
}
