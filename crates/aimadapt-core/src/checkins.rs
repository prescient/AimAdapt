//! Check-ins: how you feel and how much energy you have, answered on Today with
//! three faces each, and how your runs went with each answer.
//!
//! A check-in holds two answers, feeling and energy, each 1 (low: the sad face),
//! 2 (okay) or 3 (high: the happy face), and either can be left out. It counts for
//! the runs of its training day from when it was made until the next one. Runs
//! earlier that day, before its first check-in, count with the first, so checking
//! in after playing still covers the day. Changing an answer before a run has
//! finished since the last check-in changes that check-in. After a run it starts a
//! new one, so the runs already played keep the answers they were played with.
//!
//! A run is measured against what was expected of you:
//! - **Adaptive runs** with a plan (not calibration runs): the share of targets you
//!   succeeded on against the chance the trainer planned at your rating. 0.80
//!   against a planned 0.85 is 6% below.
//! - **Scored runs** (KovaaK's and standard runs): the score against the median of
//!   that scenario's scores on other days within two weeks either side. A run needs
//!   three of those, so a scenario's first days are left out.
//!
//! A day's figure is the mean of its runs, and an answer's figure is the mean of
//! its days, with a likely range of one standard error either side. Days are what
//! count, not runs, since the runs of one day share how that day went.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::params;

use crate::db::{Db, DbError, TIME_FORMAT};
use crate::programs::Spread;
use crate::progress::{Progress, ScenarioRun};
use crate::training::TimeSettings;

/// Schema v11: check-ins.
pub(crate) const MIGRATION: &str = r#"
-- How you felt and how much energy you had, from Today: 1 low, 2 okay, 3 high.
CREATE TABLE checkins (
    id      INTEGER PRIMARY KEY,
    at      TEXT NOT NULL,
    feeling INTEGER CHECK (feeling BETWEEN 1 AND 3),
    energy  INTEGER CHECK (energy BETWEEN 1 AND 3)
);
CREATE INDEX checkins_at ON checkins (at);
"#;

/// Answers run from 1 (low) to this (high).
pub const MAX_ANSWER: u8 = 3;

/// Days an answer needs before it can be the standout.
pub const STANDOUT_DAYS: usize = 3;

/// The smallest gap from your usual that is named as the standout (0.03 = 3%).
pub const STANDOUT_CHANGE: f64 = 0.03;

/// How many days either side of a scored run its usual reaches.
pub const USUAL_WINDOW_DAYS: i64 = 14;

/// The scores a scored run's usual needs.
pub const USUAL_MIN_RUNS: usize = 3;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckIn {
    #[cfg_attr(feature = "serde", serde(skip))]
    id: i64,
    pub at: NaiveDateTime,
    pub feeling: Option<u8>,
    pub energy: Option<u8>,
}

/// The two questions.
#[cfg_attr(feature = "serde", derive(serde::Serialize), serde(rename_all = "snake_case"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Question {
    Feeling,
    Energy,
}

impl Question {
    pub const ALL: [Question; 2] = [Question::Feeling, Question::Energy];

    fn answer(self, c: &CheckIn) -> Option<u8> {
        match self {
            Question::Feeling => c.feeling,
            Question::Energy => c.energy,
        }
    }
}

/// How the runs went with one answer to one question.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct AnswerStat {
    /// 1 (low) to 3 (high).
    pub answer: u8,
    /// Training days with runs on this answer. A day split between two answers
    /// counts for each.
    pub days: usize,
    pub runs: usize,
    /// The mean of the days' figures: 0.05 is 5% above what's usual for you. `None`
    /// with no runs.
    pub change: Option<f64>,
    /// One standard error either side of `change`. `None` under two days.
    pub spread: Option<Spread>,
}

/// The answer whose runs are furthest from your usual, when that's clear of noise.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Standout {
    pub question: Question,
    pub answer: u8,
    pub days: usize,
    pub change: f64,
    pub spread: Spread,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Today's answers as they stand: the training day's latest check-in. `None`
    /// before the first one today.
    pub today: Option<CheckIn>,
    /// One per answer, 1 to 3.
    pub feeling: Vec<AnswerStat>,
    pub energy: Vec<AnswerStat>,
    /// Training days with at least one answer.
    pub days: usize,
    /// At least three days, outside its range of noise and 3% or more from your usual.
    pub standout: Option<Standout>,
}

/// Every check-in, oldest first.
pub fn load(db: &Db) -> Result<Vec<CheckIn>, DbError> {
    let mut stmt = db.conn().prepare("SELECT id, at, feeling, energy FROM checkins ORDER BY at, id")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<u8>>(2)?, r.get::<_, Option<u8>>(3)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, at, feeling, energy) = row?;
        let Ok(at) = NaiveDateTime::parse_from_str(&at, TIME_FORMAT) else { continue };
        out.push(CheckIn { id, at, feeling, energy });
    }
    Ok(out)
}

/// The latest check-in of `now`'s training day.
fn current(all: &[CheckIn], now: NaiveDateTime, time: &TimeSettings) -> Option<CheckIn> {
    let day = time.training_day(now);
    all.iter().rev().find(|c| time.training_day(c.at) == day).copied()
}

/// Whether any run finished after `t`.
fn run_since(db: &Db, t: NaiveDateTime) -> Result<bool, DbError> {
    let at = t.format(TIME_FORMAT).to_string();
    Ok(db.conn().query_row("SELECT EXISTS (SELECT 1 FROM runs WHERE finished_at > ?1)", [at], |r| r.get(0))?)
}

/// Saves today's answers as they now stand (`None` is no answer). Answers are 1 to
/// [`MAX_ANSWER`]; the database refuses anything else.
pub fn save(
    db: &Db,
    now: NaiveDateTime,
    time: &TimeSettings,
    feeling: Option<u8>,
    energy: Option<u8>,
) -> Result<(), DbError> {
    let insert = || -> Result<(), DbError> {
        db.conn().execute(
            "INSERT INTO checkins (at, feeling, energy) VALUES (?1, ?2, ?3)",
            params![now.format(TIME_FORMAT).to_string(), feeling, energy],
        )?;
        Ok(())
    };
    match current(&load(db)?, now, time) {
        Some(c) if c.feeling == feeling && c.energy == energy => {}
        Some(c) if !run_since(db, c.at)? => {
            db.conn().execute(
                "UPDATE checkins SET feeling = ?1, energy = ?2 WHERE id = ?3",
                params![feeling, energy, c.id],
            )?;
        }
        Some(_) => insert()?,
        None if feeling.is_some() || energy.is_some() => insert()?,
        None => {}
    }
    Ok(())
}

/// Today's answers and how your runs have gone with each answer.
pub fn report(db: &Db, progress: &Progress, time: &TimeSettings, now: NaiveDateTime) -> Result<Report, DbError> {
    let all = load(db)?;
    let mut runs = Vec::new();
    if !all.is_empty() {
        let days: BTreeSet<NaiveDate> = all.iter().map(|c| time.training_day(c.at)).collect();
        runs = planned_runs(db)?;
        runs.extend(scored_runs(progress, time, &days));
    }
    Ok(build(&all, &runs, time, now))
}

/// Adaptive runs with a plan: when each finished, and its success against the
/// planned chance (-0.06 is 6% below).
fn planned_runs(db: &Db) -> Result<Vec<(NaiveDateTime, f64)>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT r.finished_at, e.success_rate, e.planned_chance
         FROM engine_runs e JOIN runs r ON r.id = e.run_id
         WHERE e.adaptive = 1 AND e.run_pick IN ('near', 'stretch', 'easy')
           AND e.success_rate IS NOT NULL AND e.planned_chance > 0",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, f64>(2)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (finished, success, planned) = row?;
        let Ok(finished) = NaiveDateTime::parse_from_str(&finished, TIME_FORMAT) else { continue };
        out.push((finished, success / planned - 1.0));
    }
    Ok(out)
}

/// Scored runs on the given training days: when each finished, and its score
/// against the median of the scenario's scores on other days within
/// [`USUAL_WINDOW_DAYS`] either side.
fn scored_runs(p: &Progress, time: &TimeSettings, days: &BTreeSet<NaiveDate>) -> Vec<(NaiveDateTime, f64)> {
    let counted = |r: &&ScenarioRun| !r.restarted && r.score.is_some();
    let mut by: HashMap<&str, Vec<(NaiveDate, f64)>> = HashMap::new();
    for r in p.runs.iter().filter(counted) {
        by.entry(r.scenario.as_str()).or_default().push((time.training_day(r.finished_at), r.score.unwrap_or(0.0)));
    }
    let mut out = Vec::new();
    for r in p.runs.iter().filter(counted) {
        let day = time.training_day(r.finished_at);
        if !days.contains(&day) {
            continue;
        }
        let mut usual: Vec<f64> = by[r.scenario.as_str()]
            .iter()
            .filter(|(d, _)| *d != day && (*d - day).num_days().abs() <= USUAL_WINDOW_DAYS)
            .map(|(_, s)| *s)
            .collect();
        if usual.len() < USUAL_MIN_RUNS {
            continue;
        }
        let m = median(&mut usual);
        if let Some(score) = r.score.filter(|_| m > 0.0) {
            out.push((r.finished_at, score / m - 1.0));
        }
    }
    out
}

/// The pure part of [`report`]: check-ins oldest first, and runs as (finished,
/// figure against what was expected).
fn build(all: &[CheckIn], runs: &[(NaiveDateTime, f64)], time: &TimeSettings, now: NaiveDateTime) -> Report {
    let mut by_day: BTreeMap<NaiveDate, Vec<&CheckIn>> = BTreeMap::new();
    for c in all {
        by_day.entry(time.training_day(c.at)).or_default().push(c);
    }
    // Each question's run figures, by day and answer.
    let mut groups: BTreeMap<(Question, NaiveDate, u8), Vec<f64>> = BTreeMap::new();
    for &(finished, value) in runs {
        let day = time.training_day(finished);
        let Some(list) = by_day.get(&day) else { continue };
        let c = list.iter().rev().find(|c| c.at <= finished).unwrap_or(&list[0]);
        for q in Question::ALL {
            if let Some(a) = q.answer(c) {
                groups.entry((q, day, a)).or_default().push(value);
            }
        }
    }
    let stats = |q: Question| -> Vec<AnswerStat> {
        (1..=MAX_ANSWER)
            .map(|a| {
                let days: Vec<&Vec<f64>> =
                    groups.iter().filter(|((gq, _, ga), _)| *gq == q && *ga == a).map(|(_, v)| v).collect();
                let day_figures: Vec<f64> = days.iter().map(|v| mean(v)).collect();
                let change = (!day_figures.is_empty()).then(|| mean(&day_figures));
                AnswerStat {
                    answer: a,
                    days: day_figures.len(),
                    runs: days.iter().map(|v| v.len()).sum(),
                    change,
                    spread: change.and_then(|m| spread(&day_figures, m)),
                }
            })
            .collect()
    };
    let (feeling, energy) = (stats(Question::Feeling), stats(Question::Energy));
    let standout = [(Question::Feeling, &feeling), (Question::Energy, &energy)]
        .into_iter()
        .flat_map(|(q, list)| list.iter().map(move |s| (q, s)))
        .filter_map(|(q, s)| {
            let (change, spread) = (s.change?, s.spread?);
            (s.days >= STANDOUT_DAYS && !spread.noise && change.abs() >= STANDOUT_CHANGE).then_some(Standout {
                question: q,
                answer: s.answer,
                days: s.days,
                change,
                spread,
            })
        })
        .fold(None, |best: Option<Standout>, s| match best {
            Some(b) if b.change.abs() >= s.change.abs() => Some(b),
            _ => Some(s),
        });
    let answered = by_day.iter().filter(|(_, l)| l.iter().any(|c| c.feeling.is_some() || c.energy.is_some())).count();
    Report { today: current(all, now, time), feeling, energy, days: answered, standout }
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

/// One standard error either side of the mean `m` of `v`, and whether zero is
/// inside. `None` under two values.
fn spread(v: &[f64], m: f64) -> Option<Spread> {
    let n = v.len();
    if n < 2 {
        return None;
    }
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64;
    let se = (var / n as f64).sqrt();
    Some(Spread { low: m - se, high: m + se, noise: m.abs() <= se })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::RunFile;
    use crate::telemetry::EngineRun;

    fn at(d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap().and_hms_opt(h, min, 0).unwrap()
    }

    fn time() -> TimeSettings {
        TimeSettings::default()
    }

    fn check(at: NaiveDateTime, feeling: Option<u8>, energy: Option<u8>) -> CheckIn {
        CheckIn { id: 0, at, feeling, energy }
    }

    /// A run of `scenario` finishing at `t`, scored, or adaptive with its success
    /// and planned chance (no plan: a calibration run).
    fn store(db: &mut Db, scenario: &str, t: NaiveDateTime, score: Option<f64>, adaptive: Option<(f64, Option<f64>)>) {
        let file = format!("{scenario} - {t}");
        let mut fields = BTreeMap::new();
        if let Some(s) = score {
            fields.insert("Score".to_string(), s.to_string());
        }
        let mode = if adaptive.is_some() { "RustyAim Adaptive" } else { "Challenge" };
        let run = RunFile {
            file_name: file.clone(),
            scenario: scenario.into(),
            mode: mode.into(),
            finished_at: t,
            fields,
            kills: vec![],
            weapons: vec![],
        };
        db.insert_runs(&[run], t).unwrap();
        if let Some((success, planned)) = adaptive {
            let e = EngineRun {
                base_scenario: scenario.into(),
                category: "x".into(),
                adaptive: true,
                run_pick: planned.map(|_| "near".into()),
                planned_chance: planned,
                success_rate: Some(success),
                ..Default::default()
            };
            assert!(db.save_engine_run(&file, &e).unwrap());
        }
    }

    fn stat(r: &Report, q: Question, answer: u8) -> &AnswerStat {
        let list = if q == Question::Feeling { &r.feeling } else { &r.energy };
        &list[usize::from(answer) - 1]
    }

    #[test]
    fn a_check_in_counts_from_when_it_was_made_and_covers_the_runs_before_it() {
        let all = [
            check(at(16, 9, 0), Some(1), Some(2)),
            check(at(16, 12, 0), Some(3), None),
            check(at(17, 20, 0), Some(2), None),
        ];
        let runs = [
            (at(16, 8, 0), -0.1),  // before the day's first check-in: counts with it
            (at(16, 10, 0), -0.2), // after the first
            (at(16, 13, 0), 0.1),  // after the second
            (at(15, 10, 0), 0.5),  // a day with no check-in
        ];
        let r = build(&all, &runs, &time(), at(17, 21, 0));
        let bad = stat(&r, Question::Feeling, 1);
        assert_eq!((bad.days, bad.runs), (1, 2));
        assert!((bad.change.unwrap() + 0.15).abs() < 1e-9);
        assert_eq!(stat(&r, Question::Feeling, 3).runs, 1);
        // The second check-in left energy out, so its run has no energy answer.
        assert_eq!(stat(&r, Question::Energy, 2).runs, 2);
        assert_eq!(r.days, 2);
        assert_eq!(r.today, Some(all[2]));
    }

    #[test]
    fn the_check_in_day_starts_at_the_day_start_hour() {
        let all = [check(at(16, 23, 0), Some(2), Some(1))];
        // 2 AM is still the 16th's training day; 5 AM is the next.
        assert_eq!(build(&all, &[], &time(), at(17, 2, 0)).today, Some(all[0]));
        assert_eq!(build(&all, &[], &time(), at(17, 5, 0)).today, None);
        let r = build(&all, &[(at(17, 1, 30), -0.1)], &time(), at(17, 2, 0));
        assert_eq!(stat(&r, Question::Energy, 1).runs, 1);
    }

    #[test]
    fn changing_an_answer_edits_the_check_in_until_a_run_is_played() {
        let mut db = Db::open_in_memory().unwrap();
        let t = time();
        save(&db, at(16, 9, 0), &t, Some(1), None).unwrap();
        save(&db, at(16, 9, 1), &t, Some(1), Some(2)).unwrap();
        let all = load(&db).unwrap();
        assert_eq!(all.len(), 1, "no run in between: the same check-in");
        assert_eq!((all[0].at, all[0].feeling, all[0].energy), (at(16, 9, 0), Some(1), Some(2)));

        store(&mut db, "Tile Frenzy", at(16, 9, 30), Some(100.0), None);
        save(&db, at(16, 9, 40), &t, Some(1), Some(2)).unwrap();
        assert_eq!(load(&db).unwrap().len(), 1, "the same answers add nothing");
        save(&db, at(16, 9, 41), &t, Some(2), Some(2)).unwrap();
        let all = load(&db).unwrap();
        assert_eq!(all.len(), 2, "after a run: a new check-in, and the run keeps the old one");
        assert_eq!(all[0].feeling, Some(1));
        assert_eq!((all[1].at, all[1].feeling), (at(16, 9, 41), Some(2)));

        // Clearing both answers leaves an empty check-in, so the day shows none.
        save(&db, at(16, 9, 42), &t, None, None).unwrap();
        let all = load(&db).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(current(&all, at(16, 10, 0), &t).map(|c| (c.feeling, c.energy)), Some((None, None)));
        // A new day starts with no check-in.
        assert_eq!(current(&all, at(17, 10, 0), &t), None);
    }

    #[test]
    fn answers_outside_one_to_three_are_refused() {
        let db = Db::open_in_memory().unwrap();
        assert!(save(&db, at(16, 9, 0), &time(), Some(4), None).is_err());
        assert!(save(&db, at(16, 9, 0), &time(), None, Some(0)).is_err());
        assert!(load(&db).unwrap().is_empty());
    }

    #[test]
    fn runs_are_measured_against_what_was_expected() {
        let mut db = Db::open_in_memory().unwrap();
        let t = time();
        // Adaptive: 0.68 against a planned 0.85 is 20% below. A calibration run has
        // no plan and is left out.
        store(&mut db, "Timed Flicks", at(16, 10, 0), Some(0.9), Some((0.68, Some(0.85))));
        store(&mut db, "Timed Flicks", at(16, 10, 5), Some(0.9), Some((0.10, None)));
        // Scored: other days within two weeks have 90, 100 and 110 (median 100); a
        // score from 15 days before is outside the window. 80 is 20% below.
        store(&mut db, "Tile Frenzy", at(1, 10, 0), Some(10.0), None);
        for (d, s) in [(9, 90.0), (12, 110.0), (20, 100.0)] {
            store(&mut db, "Tile Frenzy", at(d, 10, 0), Some(s), None);
        }
        store(&mut db, "Tile Frenzy", at(16, 11, 0), Some(80.0), None);
        // Only two other days: no usual yet.
        store(&mut db, "Pasu", at(12, 10, 0), Some(50.0), None);
        store(&mut db, "Pasu", at(14, 10, 0), Some(50.0), None);
        store(&mut db, "Pasu", at(16, 11, 30), Some(10.0), None);
        save(&db, at(16, 9, 0), &t, Some(1), None).unwrap();

        let p = crate::progress::load(&db, &t).unwrap();
        let r = report(&db, &p, &t, at(16, 12, 0)).unwrap();
        let bad = stat(&r, Question::Feeling, 1);
        assert_eq!((bad.days, bad.runs), (1, 2));
        assert!((bad.change.unwrap() + 0.2).abs() < 1e-9, "{:?}", bad.change);
        assert_eq!(bad.spread, None, "one day has no range");
    }

    #[test]
    fn days_count_and_not_runs() {
        let all = [
            check(at(10, 9, 0), None, Some(1)),
            check(at(11, 9, 0), None, Some(1)),
            check(at(12, 9, 0), None, Some(1)),
        ];
        let mut runs: Vec<(NaiveDateTime, f64)> = (0..5).map(|i| (at(10, 10, i), -0.10)).collect();
        runs.push((at(11, 10, 0), -0.08));
        runs.push((at(12, 10, 0), -0.12));
        let r = build(&all, &runs, &time(), at(12, 20, 0));
        let low = stat(&r, Question::Energy, 1);
        assert_eq!((low.days, low.runs), (3, 7));
        assert!((low.change.unwrap() + 0.10).abs() < 1e-9, "each day weighs the same");
        let sp = low.spread.unwrap();
        // The days' figures have a standard deviation of 0.02, so a standard error of 0.02/√3.
        assert!((sp.high - sp.low - 2.0 * 0.02 / 3f64.sqrt()).abs() < 1e-9);
        assert!(!sp.noise);
        let s = r.standout.unwrap();
        assert_eq!((s.question, s.answer, s.days), (Question::Energy, 1, 3));
    }

    #[test]
    fn no_standout_under_three_days_inside_noise_or_under_three_percent() {
        let days = |figures: &[f64]| {
            let all: Vec<CheckIn> = (0..figures.len() as u32).map(|i| check(at(10 + i, 9, 0), Some(1), None)).collect();
            let runs: Vec<(NaiveDateTime, f64)> =
                figures.iter().enumerate().map(|(i, f)| (at(10 + i as u32, 10, 0), *f)).collect();
            build(&all, &runs, &time(), at(20, 9, 0)).standout
        };
        assert!(days(&[-0.2, -0.21, -0.19]).is_some());
        assert!(days(&[-0.2, -0.21]).is_none(), "two days");
        assert!(days(&[-0.2, 0.15, -0.1]).is_none(), "zero is inside the range");
        assert!(days(&[-0.02, -0.021, -0.019]).is_none(), "clear of noise but small");
    }

    #[test]
    fn the_standout_is_the_answer_furthest_from_usual() {
        let mut all: Vec<CheckIn> = (0..3).map(|i| check(at(10 + i, 9, 0), Some(3), Some(1))).collect();
        let mut runs = Vec::new();
        for i in 0..3 {
            runs.push((at(10 + i, 10, 0), -0.05 - 0.001 * f64::from(i)));
        }
        // Feeling good and low energy share these days and runs, so they tie: the
        // first asked (feeling) is named.
        let r = build(&all, &runs, &time(), at(20, 9, 0));
        let s = r.standout.unwrap();
        assert_eq!((s.question, s.answer), (Question::Feeling, 3));
        // Three more days of low energy alone, further below.
        for i in 3..6 {
            all.push(check(at(10 + i, 9, 0), None, Some(1)));
            runs.push((at(10 + i, 10, 0), -0.30 - 0.001 * f64::from(i)));
        }
        let s = build(&all, &runs, &time(), at(20, 9, 0)).standout.unwrap();
        assert_eq!((s.question, s.answer, s.days), (Question::Energy, 1, 6));
    }

    #[test]
    fn no_check_ins_means_an_empty_report() {
        let db = Db::open_in_memory().unwrap();
        let t = time();
        let p = crate::progress::load(&db, &t).unwrap();
        let r = report(&db, &p, &t, at(16, 9, 0)).unwrap();
        assert_eq!((r.today, r.days, r.standout), (None, 0, None));
        assert_eq!(r.feeling.len(), 3);
        assert!(r.feeling.iter().chain(&r.energy).all(|s| s.days == 0 && s.change.is_none()));
    }
}
