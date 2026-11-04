//! Tests against real KovaaK's stats files copied into `tests/fixtures`.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use rustyaim_core::{Db, import::import_dir_at, parse_file, summary};

const CLICKING: &str = "shimPressure Micro Novice - Challenge - 2026.11.03-06.31.39 Stats.csv";
const TRACKING: &str = "Whisphere 80% - Challenge - 2026.11.02-11.19.59 Stats.csv";
const SHORT: &str = "eth FP OGG Micro 60 easy - Challenge - 2026.11.03-06.24.21 Stats.csv";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn now() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(12, 0, 0).unwrap()
}

#[test]
fn parses_clicking_run() {
    let run = parse_file(&fixtures().join(CLICKING)).unwrap();
    assert_eq!(run.scenario, "shimPressure Micro Novice");
    assert_eq!(run.mode, "Challenge");
    assert_eq!(run.field("Scenario"), Some("shimPressure Micro Novice"));
    assert_eq!(run.score(), Some(132.5));
    assert!((run.accuracy().unwrap() - 155.0 / 182.0).abs() < 1e-9);
    // 06:30:39.748 → 06:31:39
    assert!((run.duration_s().unwrap() - 59.252).abs() < 1e-6);

    assert_eq!(run.kills.len(), 155);
    let fifth = &run.kills[4];
    assert_eq!(fifth.kill_no, 5);
    assert_eq!(fifth.ttk_s, Some(0.592));
    assert_eq!(fifth.shots, Some(2));
    assert_eq!(fifth.hits, Some(1));
    assert_eq!(fifth.cheated, Some(false));

    assert_eq!(run.weapons.len(), 1);
    assert_eq!(run.weapons[0].weapon, "BB Gun");
    assert_eq!(run.weapons[0].shots, Some(182));
    assert_eq!(run.weapons[0].hits, Some(155));

    assert_eq!(run.field_f64("Horiz Sens"), Some(5.3));
    assert_eq!(run.field("Sens Scale"), Some("Overwatch"));
    assert_eq!(run.field_i64("DPI"), Some(400));
    assert_eq!(run.field_f64("FOV"), Some(104.0));
}

#[test]
fn parses_tracking_run_without_kills() {
    let run = parse_file(&fixtures().join(TRACKING)).unwrap();
    assert_eq!(run.scenario, "Whisphere 80%");
    assert!(run.kills.is_empty());
    assert_eq!(run.weapons[0].weapon, "LG");
    assert_eq!(run.weapons[0].shots, Some(23680));
    assert_eq!(run.weapons[0].hits, Some(3630));
    assert_eq!(run.score(), Some(3630.0));
    assert!((run.accuracy().unwrap() - 0.153).abs() < 0.001);
}

#[test]
fn parses_very_short_scenario() {
    let run = parse_file(&fixtures().join(SHORT)).unwrap();
    assert_eq!(run.score(), Some(0.0));
    assert!(run.duration_s().unwrap() < 3.0);
}

#[test]
fn import_is_incremental_and_records_errors() {
    let dir = tempfile::tempdir().unwrap();
    for name in [CLICKING, TRACKING, SHORT] {
        std::fs::copy(fixtures().join(name), dir.path().join(name)).unwrap();
    }
    std::fs::write(dir.path().join("broken Stats.csv"), "Score:,1\n").unwrap();
    std::fs::write(dir.path().join("readme.txt"), "not a stats file").unwrap();

    let mut db = Db::open_in_memory().unwrap();
    let first = import_dir_at(&mut db, dir.path(), now()).unwrap();
    assert_eq!(first.found, 4);
    assert_eq!(first.imported, 3);
    assert_eq!(first.skipped, 0);
    assert_eq!(first.errors.len(), 1);
    assert_eq!(first.errors[0].0, "broken Stats.csv");
    assert_eq!(db.import_errors().unwrap().len(), 1);

    let second = import_dir_at(&mut db, dir.path(), now()).unwrap();
    assert_eq!(second.imported, 0);
    assert_eq!(second.skipped, 3);

    let totals = summary::totals(&db).unwrap();
    assert_eq!(totals.runs, 3);
    assert_eq!(totals.scenarios, 3);
    assert_eq!(totals.last.as_deref(), Some("2026-11-03T06:31:39.000"));

    let kills: i64 = db.conn().query_row("SELECT COUNT(*) FROM kills", [], |r| r.get(0)).unwrap();
    assert_eq!(kills, 155 + run_kills(SHORT));
    let fields: i64 = db.conn().query_row("SELECT COUNT(*) FROM run_fields", [], |r| r.get(0)).unwrap();
    assert!(fields > 3 * 40, "expected every Key:,value line to be stored, got {fields}");
}

#[test]
fn reopening_database_keeps_schema() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let version = Db::open(&path).unwrap().schema_version().unwrap();
    assert_eq!(version, 1);
    assert_eq!(Db::open(&path).unwrap().schema_version().unwrap(), 1);
}

fn run_kills(name: &str) -> i64 {
    parse_file(&fixtures().join(name)).unwrap().kills.len() as i64
}
