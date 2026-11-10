//! Tests for Insights.

use super::*;
use crate::knob::Knob;

/// Playtest: whole degrees per second were too coarse for changes.
#[test]
fn target_speed_and_its_changes_show_one_decimal() {
    assert_eq!(Knob::TargetSpeed.format(48.46), "48.5 °/s");
    assert_eq!(Knob::TargetSpeed.format((48.9f64 - 48.4).abs()), "0.5 °/s");
}
use crate::parse::RunFile;
use crate::telemetry::{EngineRun, SampleRow, ShotRow, TargetRow};
use chrono::NaiveDate;
use std::collections::BTreeMap;

fn save(db: &mut Db, n: u32, e: &EngineRun) {
    let at = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(8, n, 0).unwrap();
    let mut fields = BTreeMap::new();
    fields.insert("Score".to_string(), "50".to_string());
    fields.insert("Source".to_string(), "rustyaim".to_string());
    // A 60 s run, with its kills.
    let start = at - chrono::Duration::seconds(60);
    fields.insert("Challenge Start".to_string(), start.format("%H:%M:%S%.3f").to_string());
    let kills = e.targets.iter().filter(|t| t.outcome == "killed").count();
    fields.insert("Kills".to_string(), kills.to_string());
    let file_name = format!("rustyaim/test/{n}");
    let run = RunFile {
        file_name: file_name.clone(),
        scenario: e.base_scenario.clone(),
        mode: "RustyAim".into(),
        finished_at: at,
        fields,
        kills: vec![],
        weapons: vec![],
    };
    db.insert_runs(&[run], at).unwrap();
    assert!(db.save_engine_run(&file_name, e).unwrap());
    assert!(!db.save_engine_run(&file_name, e).unwrap(), "details are saved once");
}

/// A clicking run where flicks to the left are slow and often time out, and
/// misses land past the target.
fn clicking_run(rating: f64) -> EngineRun {
    let mut targets = Vec::new();
    let mut shots = Vec::new();
    for i in 0..80 {
        let dir = (i % 8) as f64 * 45.0;
        let left = dir == 180.0;
        let distance = 5.0 + (i % 5) as f64 * 6.0;
        let time = 0.25 + distance / 60.0 + if left { 0.25 } else { 0.0 };
        let killed = !(left && i % 16 < 8);
        let start = i as f64 * 0.7;
        targets.push(TargetRow {
            no: i,
            spawned_at: start,
            start_at: start,
            ended_at: start + time,
            outcome: if killed { "killed" } else { "timed_out" }.into(),
            distance_deg: distance,
            direction_deg: dir,
            radius_deg: 1.0,
            lifetime_s: 1.5,
            reaction_s: Some(0.2 + (i % 3) as f64 * 0.02),
            reach_s: Some(time - 0.05),
            overshoot_deg: 0.3,
            entries: 1,
            shots: 1,
            hits: i64::from(killed),
            value: Some(0.4 + (i % 10) as f64 * 0.08),
            chance: Some(0.85),
            pick: Some("near".into()),
        });
        shots.push(ShotRow {
            t: start + time,
            hit: killed,
            target_no: Some(i),
            dx_deg: 0.0,
            dy_deg: -0.2,
            along: if killed { 0.1 } else { 1.6 },
            across: 0.0,
        });
    }
    EngineRun {
        base_scenario: "Timed Flicks".into(),
        category: "clicking".into(),
        adaptive: true,
        knob: Some("lifetime".into()),
        rating: Some(rating),
        rating_low: Some(rating * 0.95),
        rating_high: Some(rating * 1.05),
        start_rating: Some(rating + 0.1),
        theta: Some(-(rating.ln()) + 0.12 * 1.7346),
        slope: Some(0.12),
        rating_sd: Some(0.05),
        fastest: Some(rating * 0.8),
        run_value: Some(rating * 0.8),
        run_pick: Some("stretch".into()),
        planned_chance: Some(0.6),
        mix: Some("balanced".into()),
        goal: Some(0.85),
        success_rate: Some(0.85),
        rating_timeline: vec![(0.0, rating + 0.1), (60.0, rating)],
        targets,
        shots,
        ..Default::default()
    }
}

/// Bug: the notes compared the latest rating with the first one of any knob, so
/// after a scenario moved from lifetime to kill pace it said you were better
/// "than your first rating" in a different measure.
#[test]
fn notes_compare_ratings_of_the_same_knob() {
    let mut db = Db::open_in_memory().unwrap();
    save(&mut db, 1, &clicking_run(1.2));
    for (n, rating) in [(2, 0.8), (3, 0.7)] {
        let mut run = clicking_run(rating);
        run.knob = Some("kill_pace".into());
        save(&mut db, n, &run);
    }
    let i = scenario(&db, "Timed Flicks").unwrap().unwrap();
    let note = i.notes.iter().find(|n| n.title.starts_with("85% kill pace")).expect("the rating note");
    assert!(note.body.contains("Your first rating was 0.80 s 1 run ago"), "{}", note.body);
}

/// Bug: the chart was meant to keep the most recent 400 flicks but kept the
/// oldest, because the list starts with the newest run.
#[test]
fn fitts_chart_keeps_the_newest_flicks() {
    let mut db = Db::open_in_memory().unwrap();
    for n in 1..=10 {
        let mut run = clicking_run(0.8);
        if n == 10 {
            // Only the newest run flicks this far.
            run.targets.iter_mut().for_each(|t| t.distance_deg += 100.0);
        }
        save(&mut db, n, &run);
    }
    let i = scenario(&db, "Timed Flicks").unwrap().unwrap();
    let f = i.fitts.as_ref().unwrap();
    assert_eq!(f.points.len(), 400);
    assert!(f.points.iter().any(|p| p.distance_deg > 100.0), "the newest run's flicks are left out");
}

#[test]
fn clicking_insights_find_the_weak_side_and_overshoot() {
    let mut db = Db::open_in_memory().unwrap();
    save(&mut db, 1, &clicking_run(0.8));
    save(&mut db, 2, &clicking_run(0.62));
    let i = scenario(&db, "Timed Flicks").unwrap().unwrap();
    assert_eq!(i.runs.len(), 2);
    assert_eq!(i.latest_timeline.last(), Some(&(60.0, 0.62)));
    let c = i.curve.as_ref().unwrap();
    assert!((c.at85 - 0.62).abs() < 0.01, "{c:?}");
    assert!(c.at95 > c.at85 && c.at85 > c.at50, "lifetime: reliable is slower than coin-flip");
    assert!(!c.observed.is_empty());
    let left = i.directions.iter().find(|d| d.label == "Left").unwrap();
    assert!((left.success - 0.5).abs() < 1e-9, "{left:?}");
    let f = i.fitts.as_ref().unwrap();
    assert!(f.slope_ms_per_bit > 0.0 && f.throughput > 0.0, "{f:?}");
    assert!(i.runs[1].throughput.is_some());
    let m = i.shots.as_ref().unwrap();
    assert_eq!(m.overshoot_share, Some(1.0));
    assert!((m.vertical_deg.unwrap() + 0.2).abs() < 1e-9);
    let r = i.reaction.as_ref().unwrap();
    assert!((r.median_ms - 220.0).abs() < 1.0, "{r:?}");
    let titles: Vec<&str> = i.notes.iter().map(|n| n.title.as_str()).collect();
    assert!(titles.contains(&"Weakest direction: left"), "{titles:?}");
    assert!(titles.contains(&"You overshoot"), "{titles:?}");
    assert!(titles.contains(&"Shots land low"), "{titles:?}");
    assert!(titles.contains(&"85% speed: 0.62 s"), "{titles:?}");
    assert!(titles.contains(&"Best held: 0.50 s"), "{titles:?}");
    let better = &i.notes.iter().find(|n| n.title.starts_with("85% speed")).unwrap().body;
    assert!(better.contains("better by 0.18 s"), "{better}");
    // Scoring review, finding 9: the reaction floor. 0.80 → 0.62 s is 23% on the
    // rating, but the part you control fell from 0.58 to 0.40 s, 31%.
    let floor = &i.notes.iter().find(|n| n.title == "Of that, 0.22 s is reaction").unwrap().body;
    assert!(floor.contains("other 0.40 s") && floor.contains("down 31%") && floor.contains("down 23%"), "{floor}");

    let o = overview(&db).unwrap();
    assert_eq!(o[0].scenario, "Timed Flicks");
    assert_eq!(o[0].latest_rating, Some(0.62));
    assert_eq!(o[0].best_rating, Some(0.62), "lower lifetime is better");
    assert_eq!(o[0].fastest, Some(0.62 * 0.8));
    assert_eq!(db.last_rating("Timed Flicks").unwrap().unwrap().knob, "lifetime");
}

/// Bug: the Runs table's Reaction column counted flicks under 3°, but the
/// Reaction card didn't, so in micro-flick scenarios the two medians disagreed.
#[test]
fn run_reaction_leaves_out_short_flicks_like_the_card() {
    let mut db = Db::open_in_memory().unwrap();
    let mut e = clicking_run(0.8);
    for (i, t) in e.targets.iter_mut().enumerate() {
        // Half the flicks are 1° micro-corrections that "react" almost at once.
        if i % 2 == 0 {
            t.distance_deg = 1.0;
            t.reaction_s = Some(0.05);
        } else {
            t.reaction_s = Some(0.25);
        }
    }
    save(&mut db, 1, &e);
    let i = scenario(&db, "Timed Flicks").unwrap().unwrap();
    let card = i.reaction.as_ref().unwrap().median_ms;
    assert!((card - 250.0).abs() < 1e-6, "{card}");
    let run = i.runs[0].reaction_ms.unwrap();
    assert!((run - card).abs() < 1e-6, "run column {run} ms, card {card} ms");
}

#[test]
fn switching_insights_split_acquisition_from_time_on_target() {
    let mut db = Db::open_in_memory().unwrap();
    let mut e = clicking_run(1.5);
    e.base_scenario = "Stability Switch".into();
    e.category = "switching".into();
    e.knob = Some("target_speed".into());
    for (i, t) in e.targets.iter_mut().enumerate() {
        t.outcome = "killed".into();
        t.reach_s = Some(0.2 + (i % 3) as f64 * 0.05);
    }
    // 300 engaged samples, 240 on target; 300 more between targets, all off.
    e.samples = (0..600)
        .map(|i| SampleRow { t: i as f64 / 30.0, on_target: i < 240, engaged: i < 300, ..Default::default() })
        .collect();
    save(&mut db, 1, &e);
    let i = scenario(&db, "Stability Switch").unwrap().unwrap();
    let sw = i.switching.as_ref().unwrap();
    assert!((sw.acquire_ms.unwrap() - 250.0).abs() < 1.0, "{sw:?}");
    assert!((sw.engaged_on_target.unwrap() - 0.8).abs() < 1e-9, "travel isn't counted: {sw:?}");
    assert!(i.tracking.as_ref().unwrap().on_target < 0.5, "all samples: {:?}", i.tracking);
    let titles: Vec<&str> = i.notes.iter().map(|n| n.title.as_str()).collect();
    assert!((sw.kills_per_min.unwrap() - 80.0).abs() < 1e-6, "80 kills in a minute: {sw:?}");
    assert!(titles.contains(&"Switching: 80 kills a minute"), "{titles:?}");
    // Not shown for other categories.
    save(&mut db, 2, &clicking_run(0.6));
    assert!(scenario(&db, "Timed Flicks").unwrap().unwrap().switching.is_none());
}

#[test]
fn unbroken_stretch_is_where_half_the_time_on_target_comes_from() {
    let mut db = Db::open_in_memory().unwrap();
    // Four 1 s stretches and two 3 s ones, with short gaps: 10 s on target, of
    // which the two 3 s stretches hold 6 s, more than half.
    let mut on = Vec::new();
    for len in [30, 90, 30, 30, 90, 30] {
        on.extend(std::iter::repeat_n(true, len));
        on.extend(std::iter::repeat_n(false, 3));
    }
    let samples = on
        .iter()
        .enumerate()
        .map(|(i, &o)| SampleRow { t: i as f64 / 30.0, on_target: o, ..Default::default() })
        .collect();
    let e =
        EngineRun { base_scenario: "Float Track".into(), category: "tracking".into(), samples, ..Default::default() };
    save(&mut db, 1, &e);
    let t = scenario(&db, "Float Track").unwrap().unwrap().tracking.unwrap();
    assert!((t.stretch_s.unwrap() - 3.0).abs() < 1e-9, "{:?}", t.stretch_s);
}

/// Before fixed-difficulty runs, `fastest` was the hardest single target hit,
/// which a whole run can rarely match. Those old values must not count as the
/// best held, or breakthroughs would almost never happen.
#[test]
fn single_target_bests_from_old_runs_are_ignored() {
    let mut db = Db::open_in_memory().unwrap();
    let old = EngineRun { fastest: Some(0.2), run_value: None, run_pick: None, ..clicking_run(0.8) };
    save(&mut db, 1, &old);
    assert_eq!(db.fastest_hit("Timed Flicks", "lifetime", None).unwrap(), None);
    save(&mut db, 2, &clicking_run(0.62));
    assert_eq!(db.fastest_hit("Timed Flicks", "lifetime", None).unwrap(), Some(0.62 * 0.8));
    let i = scenario(&db, "Timed Flicks").unwrap().unwrap();
    assert_eq!(i.runs[0].fastest, None);
    assert_eq!(i.runs[1].fastest, Some(0.62 * 0.8));
}

#[test]
fn tracking_insights_measure_lag_and_recovery() {
    let mut db = Db::open_in_memory().unwrap();
    let samples = (0..900)
        .map(|i| {
            let since = (i % 30) as f64 / 30.0;
            // Off target for the first 200 ms after each change.
            let on = since >= 0.2 && i % 7 != 0;
            SampleRow {
                t: i as f64 / 30.0,
                on_target: on,
                error_deg: if on { 0.3 + (i % 3) as f64 * 0.1 } else { 2.0 },
                target_speed_dps: 25.0,
                lead_deg: -0.5,
                since_change_s: since,
                engaged: false,
            }
        })
        .collect();
    let e =
        EngineRun { base_scenario: "Strafe Track".into(), category: "tracking".into(), samples, ..Default::default() };
    save(&mut db, 1, &e);
    let i = scenario(&db, "Strafe Track").unwrap().unwrap();
    let t = i.tracking.as_ref().unwrap();
    assert!((t.lead_deg + 0.5).abs() < 1e-9);
    // Scoring review, finding 7: how centered you stay while on target.
    assert!((t.centered_deg.unwrap() - 0.4).abs() < 1e-9, "{t:?}");
    assert_eq!(t.centered_before_deg, None);
    assert_eq!(t.recovery_s, Some(0.2), "{t:?}");
    assert!(t.after_change[0].on_target < 0.01);
    assert!(i.runs[0].on_target.unwrap() > 0.6);
    let titles: Vec<&str> = i.notes.iter().map(|n| n.title.as_str()).collect();
    assert!(titles.contains(&"You trail the target"), "{titles:?}");
    assert!(titles.iter().any(|t| t.starts_with("Direction changes cost you 200 ms")), "{titles:?}");
}

/// How long a scenario's insights take to load with a long tracking history: 200
/// runs of 60 s, 360,000 samples. Run by hand with
/// `cargo test -p aimadapt-core --release -- --ignored insights_load_time`.
#[test]
#[ignore = "timing; run by hand"]
fn insights_load_time() {
    let mut db = Db::open_in_memory().unwrap();
    for n in 0..200u32 {
        let mut run = clicking_run(0.8);
        run.samples = (0..1800)
            .map(|k| crate::telemetry::SampleRow {
                t: f64::from(k) / 30.0,
                on_target: k % 3 != 0,
                error_deg: 0.5,
                target_speed_dps: 40.0,
                lead_deg: 0.1,
                since_change_s: 0.4,
                engaged: true,
            })
            .collect();
        let at = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap().and_hms_opt(8, 0, 0).unwrap()
            + chrono::Duration::minutes(i64::from(n) * 2);
        let mut fields = BTreeMap::new();
        fields.insert("Score".to_string(), "50".to_string());
        fields.insert("Source".to_string(), "rustyaim".to_string());
        let file_name = format!("rustyaim/timing/{n}");
        let file = RunFile {
            file_name: file_name.clone(),
            scenario: run.base_scenario.clone(),
            mode: "RustyAim".into(),
            finished_at: at,
            fields,
            kills: vec![],
            weapons: vec![],
        };
        db.save_trainer_run(&file, &run, at).unwrap();
    }
    let start = std::time::Instant::now();
    let i = scenario(&db, "Timed Flicks").unwrap().unwrap();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    println!("insights for {} runs: {ms:.0} ms", i.runs.len());
    assert!(ms < 1000.0, "{ms:.0} ms");
}
