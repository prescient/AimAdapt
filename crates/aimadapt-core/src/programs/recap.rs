//! What the last session changed: each scenario's rating before and after it, the
//! breakthroughs, the time played and one note. The app shows it when the trainer
//! closes and on Today.

use chrono::{NaiveDate, NaiveDateTime};

use super::{Row, Spread, load, sd_of_diff};
use crate::db::{Db, DbError};
use crate::insights::{self, Held};
use crate::knob::Knob;
use crate::results;
use crate::training::{self, Session, TimeSettings};

/// How many of the session's scenarios are asked for a note, most played first.
const NOTE_SCENARIOS: usize = 3;

/// A program drill's change since the same day of the program before.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct WeekChange {
    /// The training day it is compared with.
    pub date: NaiveDate,
    /// The change since then as a fraction, better positive (0.05 = 5% better).
    pub change: f64,
    pub spread: Option<Spread>,
}

/// One scenario in the session.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioRecap {
    pub scenario: String,
    /// The trainer's runs of it in the session.
    pub runs: usize,
    /// The knob its rating is in, when the session has a rated run of it.
    pub knob: Option<String>,
    /// The rating before the session: the last one on the same version and knob.
    pub before: Option<f64>,
    /// The training day of that rating.
    pub before_date: Option<NaiveDate>,
    /// The rating after the session's last rated run.
    pub after: Option<f64>,
    /// The change from before to after as a fraction, better positive.
    pub change: Option<f64>,
    pub spread: Option<Spread>,
    /// Runs in the session that held a harder level than any run before.
    pub breakthroughs: usize,
    /// The best score of the session's standard runs.
    pub best_score: Option<f64>,
    /// For a program drill: the change since the same day of the program before,
    /// when that is another run than the one `before` comes from.
    pub week: Option<WeekChange>,
}

/// The note shown with the session, and the scenario it is about.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct RecapNote {
    pub scenario: String,
    pub title: String,
    pub body: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecap {
    pub day: NaiveDate,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    /// Every run in the session, KovaaK's runs included.
    pub runs: usize,
    /// The trainer's runs in it.
    pub trainer_runs: usize,
    pub active_s: f64,
    pub length_s: f64,
    /// In the order first played.
    pub scenarios: Vec<ScenarioRecap>,
    pub breakthroughs: usize,
    pub note: Option<RecapNote>,
}

/// The session that holds the latest trainer run, or `None` before the first one.
/// Sessions are the ones on Progress › Time, so KovaaK's runs played in the same
/// sitting count toward its length.
pub fn last_session(db: &Db, time: &TimeSettings) -> Result<Option<SessionRecap>, DbError> {
    let rows = load(db, time)?;
    let Some(last) = rows.last() else { return Ok(None) };
    let sessions = training::group_sessions(&training::load_runs(db, time)?, time);
    let Some(session) = sessions.iter().rev().find(|s| s.start <= last.finished && last.finished <= s.end) else {
        return Ok(None);
    };
    let mut recap = recap_of(session, &rows);
    recap.note = note(db, &recap.scenarios)?;
    Ok(Some(recap))
}

fn inside(session: &Session, r: &Row) -> bool {
    session.start <= r.finished && r.finished <= session.end
}

/// The session's figures from every trainer run, oldest first. The note is added by
/// [`last_session`].
fn recap_of(session: &Session, rows: &[Row]) -> SessionRecap {
    let mut names: Vec<&str> = Vec::new();
    for r in rows.iter().filter(|r| inside(session, r)) {
        if !names.contains(&r.scenario.as_str()) {
            names.push(&r.scenario);
        }
    }
    let scenarios: Vec<ScenarioRecap> = names
        .iter()
        .map(|name| {
            let all: Vec<&Row> = rows.iter().filter(|r| r.scenario == *name).collect();
            scenario_recap(name, &all, session)
        })
        .collect();
    SessionRecap {
        day: session.day,
        start: session.start,
        end: session.end,
        runs: session.runs,
        trainer_runs: scenarios.iter().map(|s| s.runs).sum(),
        active_s: session.active_s,
        length_s: session.length_s(),
        breakthroughs: scenarios.iter().map(|s| s.breakthroughs).sum(),
        scenarios,
        note: None,
    }
}

/// One scenario's part of the session. `all` is every trainer run of it, oldest first.
fn scenario_recap(name: &str, all: &[&Row], session: &Session) -> ScenarioRecap {
    let rated = |r: &Row| r.adaptive && r.rating.is_some() && r.knob.is_some();
    let held: Vec<Held> = all
        .iter()
        .map(|r| Held { knob: r.knob.as_deref(), hash: r.hash.as_deref(), run_value: r.run_value, fastest: r.fastest })
        .collect();
    let breakthroughs =
        insights::breakthroughs(&held).iter().zip(all).filter(|(broke, r)| **broke && inside(session, r)).count();
    let best_score = all
        .iter()
        .filter(|r| inside(session, r) && !r.adaptive)
        .filter_map(|r| r.score)
        .fold(None, |best: Option<f64>, s| Some(best.map_or(s, |b| b.max(s))));
    let mut out = ScenarioRecap {
        scenario: name.to_string(),
        runs: all.iter().filter(|r| inside(session, r)).count(),
        knob: None,
        before: None,
        before_date: None,
        after: None,
        change: None,
        spread: None,
        breakthroughs,
        best_score,
        week: None,
    };
    let Some(last) = all.iter().rev().find(|r| inside(session, r) && rated(r)) else { return out };
    let knob = Knob::from_id(last.knob.as_deref().unwrap_or_default());
    out.knob = last.knob.clone();
    out.after = last.rating;
    // Ratings of another version or another knob are on another scale.
    let same = |r: &Row| rated(r) && r.knob == last.knob && r.hash == last.hash;
    let since = |earlier: &Row| -> Option<(f64, Option<Spread>)> {
        let m = knob.h(last.rating?) - knob.h(earlier.rating?);
        m.is_finite().then(|| (m.exp() - 1.0, Spread::maybe(m, sd_of_diff(earlier.sd, last.sd))))
    };
    let before = all.iter().rev().find(|r| r.finished < session.start && same(r));
    if let Some(b) = before {
        out.before = b.rating;
        out.before_date = Some(b.date);
        if let Some((change, spread)) = since(b) {
            out.change = Some(change);
            out.spread = spread;
        }
    }
    // A program drill is also compared with the same day of the program before,
    // unless that is the run the change above is already measured from.
    if let Some(day) = &last.day {
        let earlier = all.iter().rev().find(|r| r.date < session.day && r.day.as_ref() == Some(day) && same(r));
        if let Some(w) = earlier.filter(|w| before.is_none_or(|b| b.run_id != w.run_id))
            && let Some((change, spread)) = since(w)
        {
            out.week = Some(WeekChange { date: w.date, change, spread });
        }
    }
    out
}

/// The scenarios to ask for a note: the most played first, and of two played as
/// much, the one played later.
fn note_order(scenarios: &[ScenarioRecap]) -> Vec<&str> {
    let mut order: Vec<(usize, &ScenarioRecap)> = scenarios.iter().enumerate().collect();
    order.sort_by(|a, b| b.1.runs.cmp(&a.1.runs).then(b.0.cmp(&a.0)));
    order.into_iter().take(NOTE_SCENARIOS).map(|(_, s)| s.scenario.as_str()).collect()
}

/// The session's note: the top note (see [`results::top_note`]) of the most played
/// scenario that has one.
fn note(db: &Db, scenarios: &[ScenarioRecap]) -> Result<Option<RecapNote>, DbError> {
    for name in note_order(scenarios) {
        let Some(i) = insights::scenario(db, name)? else { continue };
        if let Some(n) = results::top_note(&i.notes) {
            return Ok(Some(RecapNote { scenario: name.to_string(), title: n.title.clone(), body: n.body.clone() }));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Duration;

    use super::*;
    use crate::insights::tests::{clicking_run, save};
    use crate::parse::RunFile;
    use crate::telemetry::EngineRun;

    /// One trainer run to save. Made by [`play`] or [`standard`].
    #[derive(Clone)]
    struct Play {
        at: NaiveDateTime,
        scenario: &'static str,
        adaptive: bool,
        knob: Option<&'static str>,
        rating: Option<f64>,
        sd: Option<f64>,
        hash: Option<&'static str>,
        day: Option<&'static str>,
        run_value: Option<f64>,
        fastest: Option<f64>,
        score: Option<f64>,
    }

    fn at(m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap().and_hms_opt(h, min, 0).unwrap()
    }

    /// An adaptive lifetime run of `scenario` rated `rating`.
    fn play(at: NaiveDateTime, scenario: &'static str, rating: f64) -> Play {
        Play {
            at,
            scenario,
            adaptive: true,
            knob: Some("lifetime"),
            rating: Some(rating),
            sd: None,
            hash: None,
            day: None,
            run_value: None,
            fastest: None,
            score: None,
        }
    }

    /// A standard run of `scenario` with this score.
    fn standard(at: NaiveDateTime, scenario: &'static str, score: f64) -> Play {
        Play { adaptive: false, knob: None, rating: None, score: Some(score), ..play(at, scenario, 0.0) }
    }

    /// A run file that took 60 s and ended at `at`.
    fn file(at: NaiveDateTime, name: String, scenario: &str, mut fields: BTreeMap<String, String>) -> RunFile {
        fields.insert("Challenge Start".into(), (at - Duration::seconds(60)).format("%H:%M:%S%.3f").to_string());
        RunFile {
            file_name: name,
            scenario: scenario.into(),
            mode: "Test".into(),
            finished_at: at,
            fields,
            kills: vec![],
            weapons: vec![],
        }
    }

    fn put(db: &mut Db, p: &Play) {
        let name = format!("rustyaim/{}/{}", p.scenario, p.at);
        let mut fields = BTreeMap::from([("Source".to_string(), "rustyaim".to_string())]);
        if let Some(h) = p.hash {
            fields.insert("Hash".into(), h.into());
        }
        if let Some(s) = p.score {
            fields.insert("Score".into(), s.to_string());
        }
        db.insert_runs(&[file(p.at, name.clone(), p.scenario, fields)], p.at).unwrap();
        let e = EngineRun {
            base_scenario: p.scenario.into(),
            category: "x".into(),
            adaptive: p.adaptive,
            knob: p.knob.map(String::from),
            rating: p.rating,
            rating_sd: p.sd,
            program: p.day.map(|_| "Core Week".to_string()),
            program_day: p.day.map(String::from),
            run_value: p.run_value,
            fastest: p.fastest,
            ..Default::default()
        };
        assert!(db.save_engine_run(&name, &e).unwrap());
    }

    /// A run saved the way an import saves it: no `engine_runs` row.
    fn kovaaks(db: &mut Db, at: NaiveDateTime, scenario: &str) {
        let name = format!("kovaaks/{scenario}/{at}");
        db.insert_runs(&[file(at, name, scenario, BTreeMap::new())], at).unwrap();
    }

    fn put_all(db: &mut Db, plays: &[Play]) {
        for p in plays {
            put(db, p);
        }
    }

    fn recap(db: &Db) -> SessionRecap {
        last_session(db, &TimeSettings::default()).unwrap().expect("a session")
    }

    fn only<'a>(r: &'a SessionRecap, name: &str) -> &'a ScenarioRecap {
        r.scenarios.iter().find(|s| s.scenario == name).expect("the scenario is in the session")
    }

    /// Three runs on Monday, Nov 9 evening, the last rated `last`.
    fn today(scenario: &'static str, last: f64) -> Vec<Play> {
        [(0, last + 0.04), (2, last + 0.02), (4, last)]
            .iter()
            .map(|(m, r)| play(at(11, 9, 18, *m), scenario, *r))
            .collect()
    }

    #[test]
    fn no_trainer_runs_is_no_session() {
        let mut db = Db::open_in_memory().unwrap();
        assert_eq!(last_session(&db, &TimeSettings::default()).unwrap(), None);
        kovaaks(&mut db, at(11, 9, 18, 0), "1wall6targets TE");
        assert_eq!(last_session(&db, &TimeSettings::default()).unwrap(), None);
    }

    #[test]
    fn the_session_is_the_one_with_the_latest_trainer_run() {
        let mut db = Db::open_in_memory().unwrap();
        put_all(&mut db, &today("Timed Flicks", 0.5));
        kovaaks(&mut db, at(11, 10, 18, 0), "1wall6targets TE");
        let r = recap(&db);
        assert_eq!(r.day, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
        assert_eq!(r.start, at(11, 9, 17, 59));
        assert_eq!(r.end, at(11, 9, 18, 4));
        assert_eq!(r.trainer_runs, 3);
        assert_eq!(r.runs, 3);
    }

    #[test]
    fn kovaaks_runs_in_the_sitting_count_toward_the_session_only() {
        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &play(at(11, 9, 18, 0), "Timed Flicks", 0.6));
        put(&mut db, &play(at(11, 9, 18, 5), "Timed Flicks", 0.58));
        kovaaks(&mut db, at(11, 9, 18, 10), "1wall6targets TE");
        let r = recap(&db);
        assert_eq!((r.runs, r.trainer_runs), (3, 2));
        assert_eq!(r.scenarios.len(), 1);
        assert!(r.scenarios.iter().all(|s| s.scenario != "1wall6targets TE"));
        assert!(r.end >= at(11, 9, 18, 10), "the sitting's length includes the KovaaK's run");
    }

    #[test]
    fn a_rating_is_compared_with_the_last_one_before_the_session() {
        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &play(at(11, 2, 18, 0), "Timed Flicks", 0.60));
        put_all(&mut db, &today("Timed Flicks", 0.54));
        let r = recap(&db);
        let s = only(&r, "Timed Flicks");
        assert_eq!(s.knob.as_deref(), Some("lifetime"));
        assert_eq!(s.before, Some(0.60));
        assert_eq!(s.after, Some(0.54));
        assert_eq!(s.before_date, Some(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()));
        assert!((s.change.unwrap() - (0.60 / 0.54 - 1.0)).abs() < 1e-9, "a shorter lifetime is better: {s:?}");

        let mut db = Db::open_in_memory().unwrap();
        let speed = |at: NaiveDateTime, v: f64| Play { knob: Some("target_speed"), ..play(at, "Strafe Track", v) };
        put(&mut db, &speed(at(11, 2, 18, 0), 20.0));
        put(&mut db, &speed(at(11, 9, 18, 0), 21.0));
        put(&mut db, &speed(at(11, 9, 18, 2), 22.0));
        let r = recap(&db);
        let s = only(&r, "Strafe Track");
        assert_eq!((s.before, s.after), (Some(20.0), Some(22.0)));
        assert!((s.change.unwrap() - 0.10).abs() < 1e-9, "a higher speed is better: {s:?}");
    }

    #[test]
    fn the_change_has_a_range_from_both_ratings() {
        let ranged = |after: f64| {
            let mut db = Db::open_in_memory().unwrap();
            put(&mut db, &Play { sd: Some(0.03), ..play(at(11, 2, 18, 0), "Timed Flicks", 0.60) });
            put(&mut db, &Play { sd: Some(0.04), ..play(at(11, 9, 18, 0), "Timed Flicks", after) });
            recap(&db).scenarios.remove(0)
        };
        // 0.60 to 0.58 is a 3.4% change in log units, inside the 0.05 range.
        let small = ranged(0.58);
        let spread = small.spread.expect("both ratings have a range");
        assert!((spread.sd() - 0.05).abs() < 1e-9, "{spread:?}");
        assert!(spread.noise, "{spread:?}");
        // 0.60 to 0.50 is 18% in log units, outside it.
        let big = ranged(0.50);
        assert!(!big.spread.unwrap().noise);

        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &play(at(11, 2, 18, 0), "Timed Flicks", 0.60));
        put(&mut db, &play(at(11, 9, 18, 0), "Timed Flicks", 0.50));
        let s = recap(&db).scenarios.remove(0);
        assert!(s.change.is_some());
        assert_eq!(s.spread, None, "no stored range on either side");
    }

    #[test]
    fn a_first_session_has_no_change() {
        let mut db = Db::open_in_memory().unwrap();
        put_all(&mut db, &today("Timed Flicks", 0.54));
        let r = recap(&db);
        let s = only(&r, "Timed Flicks");
        assert_eq!(s.after, Some(0.54));
        assert_eq!((s.before, s.before_date, s.change, s.spread, s.week.is_some()), (None, None, None, None, false));
    }

    #[test]
    fn another_version_or_knob_is_not_the_before() {
        let versioned = |hash: &'static str, at, rating| Play { hash: Some(hash), ..play(at, "Timed Flicks", rating) };
        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &versioned("a", at(11, 2, 18, 0), 0.60));
        put(&mut db, &versioned("b", at(11, 5, 18, 0), 0.40));
        put(&mut db, &versioned("a", at(11, 9, 18, 0), 0.54));
        assert_eq!(recap(&db).scenarios[0].before, Some(0.60), "the newer rating is of another version");

        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &versioned("b", at(11, 5, 18, 0), 0.40));
        put(&mut db, &versioned("a", at(11, 9, 18, 0), 0.54));
        let s = recap(&db).scenarios.remove(0);
        assert_eq!((s.before, s.change), (None, None));

        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &play(at(11, 2, 18, 0), "Timed Flicks", 0.60));
        put(&mut db, &Play { knob: Some("kill_pace"), ..play(at(11, 5, 18, 0), "Timed Flicks", 0.40) });
        put(&mut db, &play(at(11, 9, 18, 0), "Timed Flicks", 0.54));
        assert_eq!(recap(&db).scenarios[0].before, Some(0.60), "the newer rating is of another knob");

        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &Play { knob: Some("kill_pace"), ..play(at(11, 5, 18, 0), "Timed Flicks", 0.40) });
        put(&mut db, &play(at(11, 9, 18, 0), "Timed Flicks", 0.54));
        assert_eq!(recap(&db).scenarios[0].before, None);
    }

    #[test]
    fn breakthroughs_count_the_sessions_only() {
        let held = |at, scenario, v: f64| Play { run_value: Some(v), fastest: Some(v), ..play(at, scenario, v) };
        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &held(at(11, 2, 18, 0), "Timed Flicks", 0.60));
        put(&mut db, &held(at(11, 9, 18, 0), "Timed Flicks", 0.55));
        put(&mut db, &held(at(11, 9, 18, 2), "Timed Flicks", 0.50));
        // The first held run of a scenario is never a breakthrough.
        put(&mut db, &held(at(11, 9, 18, 4), "Other Flicks", 0.70));
        let r = recap(&db);
        assert_eq!(only(&r, "Timed Flicks").breakthroughs, 2);
        assert_eq!(only(&r, "Other Flicks").breakthroughs, 0);
        assert_eq!(r.breakthroughs, 2);
    }

    #[test]
    fn standard_runs_show_the_best_score() {
        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &standard(at(11, 9, 18, 0), "Timed Flicks", 410.0));
        put(&mut db, &standard(at(11, 9, 18, 2), "Timed Flicks", 455.0));
        let r = recap(&db);
        let s = only(&r, "Timed Flicks");
        assert_eq!(s.best_score, Some(455.0));
        assert_eq!((s.after, s.change, s.runs), (None, None, 2));
    }

    #[test]
    fn a_program_drill_is_compared_with_the_same_day_before() {
        let drill = |m, d, day, rating| Play { day: Some(day), ..play(at(m, d, 18, 0), "Timed Flicks", rating) };
        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &drill(11, 2, "mon", 0.60));
        put(&mut db, &drill(11, 5, "thu", 0.58));
        put(&mut db, &drill(11, 9, "mon", 0.54));
        let s = recap(&db).scenarios.remove(0);
        assert_eq!(s.before, Some(0.58), "the last rating, played on another program day");
        let week = s.week.expect("last Monday's run is another run than the before");
        assert_eq!(week.date, NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
        assert!((week.change - (0.60 / 0.54 - 1.0)).abs() < 1e-9, "{week:?}");

        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &drill(11, 2, "mon", 0.60));
        put(&mut db, &drill(11, 9, "mon", 0.54));
        let s = recap(&db).scenarios.remove(0);
        assert_eq!(s.before, Some(0.60));
        assert_eq!(s.week, None, "that run is already the before");
    }

    #[test]
    fn scenarios_are_in_the_order_first_played() {
        let mut db = Db::open_in_memory().unwrap();
        put(&mut db, &play(at(11, 9, 18, 0), "B Flicks", 0.6));
        put(&mut db, &play(at(11, 9, 18, 2), "A Flicks", 0.6));
        put(&mut db, &play(at(11, 9, 18, 4), "B Flicks", 0.58));
        let r = recap(&db);
        let order: Vec<(&str, usize)> = r.scenarios.iter().map(|s| (s.scenario.as_str(), s.runs)).collect();
        assert_eq!(order, [("B Flicks", 2), ("A Flicks", 1)]);
    }

    fn with_runs(name: &str, runs: usize) -> ScenarioRecap {
        ScenarioRecap {
            scenario: name.into(),
            runs,
            knob: None,
            before: None,
            before_date: None,
            after: None,
            change: None,
            spread: None,
            breakthroughs: 0,
            best_score: None,
            week: None,
        }
    }

    #[test]
    fn the_note_comes_from_the_most_played_scenarios() {
        let three = [with_runs("a", 2), with_runs("b", 5), with_runs("c", 5)];
        assert_eq!(note_order(&three), ["c", "b", "a"], "of two played as much, the later one first");
        let five: Vec<ScenarioRecap> = (0..5).map(|i| with_runs(&format!("s{i}"), 1 + i)).collect();
        assert_eq!(note_order(&five), ["s4", "s3", "s2"]);

        let mut db = Db::open_in_memory().unwrap();
        assert_eq!(note(&db, &three).unwrap(), None, "no insights for those scenarios");
        for (n, rating) in [(1, 1.2), (2, 1.1), (3, 1.0)] {
            save(&mut db, n, &clicking_run(rating));
        }
        let r = recap(&db);
        let name = &r.scenarios[0].scenario;
        let insights = insights::scenario(&db, name).unwrap().unwrap();
        let expected = results::top_note(&insights.notes).map(|n| (n.title.clone(), n.body.clone()));
        assert!(expected.is_some(), "the data has a note to show");
        let got = r.note.as_ref().map(|n| (n.scenario.as_str(), n.title.clone(), n.body.clone()));
        assert_eq!(got, expected.map(|(t, b)| (name.as_str(), t, b)));
    }
}
