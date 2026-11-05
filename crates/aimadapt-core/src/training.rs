//! Training time: sessions, training days, streaks, goals and when you train.
//!
//! - **Active time** is time inside runs (run length minus pauses).
//! - **Session time** is first run start to last run finish, so it includes
//!   menus and breaks. A gap longer than [`TimeSettings::session_gap_min`]
//!   between runs starts a new session.
//! - A **training day** starts at [`TimeSettings::day_start_hour`] (default 4 AM)
//!   so a late-night session counts toward the day it started on.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};

use crate::db::{Db, DbError};

const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f";

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSettings {
    /// A break longer than this (minutes) between runs starts a new session.
    pub session_gap_min: i64,
    /// Hour (0–23) at which a new training day begins.
    pub day_start_hour: u32,
    /// A run shorter than this fraction of its scenario's usual length counts as restarted.
    pub restart_fraction: f64,
    /// Only flag restarts for scenarios with at least this many runs, so "usual" means something.
    pub restart_min_runs: usize,
    /// Daily goal in minutes of active time.
    pub daily_goal_min: f64,
}

impl Default for TimeSettings {
    fn default() -> Self {
        Self {
            session_gap_min: 15,
            day_start_hour: 4,
            restart_fraction: 0.5,
            restart_min_runs: 3,
            daily_goal_min: 30.0,
        }
    }
}

impl TimeSettings {
    pub fn training_day(&self, t: NaiveDateTime) -> NaiveDate {
        (t - Duration::hours(i64::from(self.day_start_hour))).date()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct RunTiming {
    pub run_id: i64,
    pub scenario: String,
    pub started_at: NaiveDateTime,
    pub finished_at: NaiveDateTime,
    /// Run length minus pauses, in seconds.
    pub active_s: f64,
    /// Ended well short of the scenario's usual length (quit or restarted).
    pub restarted: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub day: NaiveDate,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub runs: usize,
    pub restarted: usize,
    pub active_s: f64,
}

impl Session {
    pub fn length_s(&self) -> f64 {
        (self.end - self.start).num_milliseconds() as f64 / 1000.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct DayStat {
    pub day: NaiveDate,
    pub runs: usize,
    pub sessions: usize,
    pub active_s: f64,
    pub session_s: f64,
    pub goal_met: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct WeekStat {
    /// The Monday the week starts on.
    pub week_start: NaiveDate,
    pub days_trained: usize,
    pub active_s: f64,
    pub session_s: f64,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Streak {
    /// Consecutive training days up to today, or up to yesterday if today isn't done yet.
    pub current: u32,
    pub best: u32,
    pub trained_today: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct TrainingReport {
    pub today: NaiveDate,
    pub runs: Vec<RunTiming>,
    pub sessions: Vec<Session>,
    pub days: Vec<DayStat>,
    pub weeks: Vec<WeekStat>,
    pub streak: Streak,
    /// Average active minutes per weekday (Monday first), over every occurrence of that
    /// weekday since the first training day. `None` if that weekday hasn't come up yet.
    pub weekday_avg_min: [Option<f64>; 7],
    /// Runs started in each hour of the day (local time).
    pub runs_by_hour: [usize; 24],
}

impl TrainingReport {
    pub fn active_s(&self) -> f64 {
        self.sessions.iter().map(|s| s.active_s).sum()
    }

    pub fn session_s(&self) -> f64 {
        self.sessions.iter().map(Session::length_s).sum()
    }
}

/// Builds the full report from the database. `now` decides what "today" is.
pub fn report(db: &Db, settings: &TimeSettings, now: NaiveDateTime) -> Result<TrainingReport, DbError> {
    let runs = load_runs(db, settings)?;
    Ok(report_from_runs(runs, settings, now))
}

pub fn report_from_runs(runs: Vec<RunTiming>, settings: &TimeSettings, now: NaiveDateTime) -> TrainingReport {
    let today = settings.training_day(now);
    let sessions = group_sessions(&runs, settings);
    let days = daily(&runs, &sessions, settings);
    let weeks = weekly(&days);
    let streak = streak(&days, today);
    let weekday_avg_min = weekday_averages(&days, today);
    let mut runs_by_hour = [0; 24];
    for r in &runs {
        runs_by_hour[r.started_at.hour() as usize] += 1;
    }
    TrainingReport { today, runs, sessions, days, weeks, streak, weekday_avg_min, runs_by_hour }
}

/// Loads every run's timing, sorted by start, with restarts flagged.
pub fn load_runs(db: &Db, settings: &TimeSettings) -> Result<Vec<RunTiming>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT id, scenario, started_at, finished_at, duration_s, pause_duration FROM runs ORDER BY finished_at",
    )?;
    let rows = stmt.query_map([], |r| {
        let finished: String = r.get(3)?;
        let Some(finished_at) = parse_time(&finished) else { return Ok(None) };
        let started: Option<String> = r.get(2)?;
        let duration: Option<f64> = r.get(4)?;
        let pause: Option<f64> = r.get(5)?;
        Ok(Some(RunTiming {
            run_id: r.get(0)?,
            scenario: r.get(1)?,
            started_at: started.as_deref().and_then(parse_time).unwrap_or(finished_at),
            finished_at,
            active_s: active_seconds(duration.unwrap_or(0.0), pause.unwrap_or(0.0)),
            restarted: false,
        }))
    })?;
    let mut runs: Vec<RunTiming> = rows.filter_map(Result::transpose).collect::<Result<_, _>>()?;
    runs.sort_by_key(|r| r.started_at);
    flag_restarts(&mut runs, settings);
    Ok(runs)
}

/// Run length minus pauses. `Pause Duration` is assumed to be seconds; a value as long
/// as the run itself can't be right, so it's ignored rather than zeroing the run.
fn active_seconds(duration_s: f64, pause_s: f64) -> f64 {
    if pause_s > 0.0 && pause_s < duration_s { duration_s - pause_s } else { duration_s }
}

fn parse_time(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, TIME_FORMAT).ok()
}

/// Flags runs much shorter than their scenario's median length. Done per scenario
/// because some scenarios are meant to last a second or two.
pub fn flag_restarts(runs: &mut [RunTiming], settings: &TimeSettings) {
    let mut lengths: HashMap<&str, Vec<f64>> = HashMap::new();
    for r in runs.iter() {
        lengths.entry(r.scenario.as_str()).or_default().push(r.active_s);
    }
    let medians: HashMap<String, f64> = lengths
        .into_iter()
        .filter(|(_, v)| v.len() >= settings.restart_min_runs)
        .map(|(k, mut v)| {
            v.sort_by(f64::total_cmp);
            (k.to_string(), v[v.len() / 2])
        })
        .collect();
    for r in runs.iter_mut() {
        r.restarted = medians.get(&r.scenario).is_some_and(|m| r.active_s < m * settings.restart_fraction);
    }
}

/// Groups runs (sorted by start) into sessions split by breaks longer than the gap.
pub fn group_sessions(runs: &[RunTiming], settings: &TimeSettings) -> Vec<Session> {
    let gap = Duration::minutes(settings.session_gap_min);
    let mut sessions: Vec<Session> = Vec::new();
    for r in runs {
        match sessions.last_mut() {
            Some(s) if r.started_at - s.end <= gap => {
                s.end = s.end.max(r.finished_at);
                s.runs += 1;
                s.restarted += usize::from(r.restarted);
                s.active_s += r.active_s;
            }
            _ => sessions.push(Session {
                day: settings.training_day(r.started_at),
                start: r.started_at,
                end: r.finished_at,
                runs: 1,
                restarted: usize::from(r.restarted),
                active_s: r.active_s,
            }),
        }
    }
    sessions
}

/// One entry per training day that has runs, oldest first. A session counts toward
/// the day it started on.
pub fn daily(runs: &[RunTiming], sessions: &[Session], settings: &TimeSettings) -> Vec<DayStat> {
    let mut days: BTreeMap<NaiveDate, DayStat> = BTreeMap::new();
    fn entry(days: &mut BTreeMap<NaiveDate, DayStat>, day: NaiveDate) -> &mut DayStat {
        days.entry(day).or_insert(DayStat { day, runs: 0, sessions: 0, active_s: 0.0, session_s: 0.0, goal_met: false })
    }
    for r in runs {
        entry(&mut days, settings.training_day(r.started_at)).runs += 1;
    }
    for s in sessions {
        let d = entry(&mut days, s.day);
        d.sessions += 1;
        d.active_s += s.active_s;
        d.session_s += s.length_s();
    }
    for d in days.values_mut() {
        d.goal_met = d.active_s >= settings.daily_goal_min * 60.0;
    }
    days.into_values().collect()
}

pub fn weekly(days: &[DayStat]) -> Vec<WeekStat> {
    let mut weeks: BTreeMap<NaiveDate, WeekStat> = BTreeMap::new();
    for d in days {
        let week_start = d.day - Duration::days(i64::from(d.day.weekday().num_days_from_monday()));
        let w =
            weeks.entry(week_start).or_insert(WeekStat { week_start, days_trained: 0, active_s: 0.0, session_s: 0.0 });
        w.days_trained += 1;
        w.active_s += d.active_s;
        w.session_s += d.session_s;
    }
    weeks.into_values().collect()
}

/// `days` must be sorted oldest first (as [`daily`] returns them).
pub fn streak(days: &[DayStat], today: NaiveDate) -> Streak {
    let mut best = 0;
    let mut run = 0;
    let mut prev: Option<NaiveDate> = None;
    for d in days {
        run = match prev {
            Some(p) if d.day - p == Duration::days(1) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(d.day);
    }
    let trained_today = prev == Some(today);
    let alive = trained_today || prev == Some(today - Duration::days(1));
    Streak { current: if alive { run } else { 0 }, best, trained_today }
}

/// Average active minutes per weekday over every occurrence since the first training
/// day. Days with no training count as zero; today only counts once you've trained.
pub fn weekday_averages(days: &[DayStat], today: NaiveDate) -> [Option<f64>; 7] {
    let Some(first) = days.first().map(|d| d.day) else { return [None; 7] };
    let active: HashMap<NaiveDate, f64> = days.iter().map(|d| (d.day, d.active_s)).collect();
    let last = if active.contains_key(&today) { today } else { today - Duration::days(1) };

    let mut sum = [0.0; 7];
    let mut count = [0u32; 7];
    let mut day = first;
    while day <= last {
        let i = day.weekday().num_days_from_monday() as usize;
        sum[i] += active.get(&day).copied().unwrap_or(0.0) / 60.0;
        count[i] += 1;
        day += Duration::days(1);
    }
    std::array::from_fn(|i| (count[i] > 0).then(|| sum[i] / f64::from(count[i])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(day: u32, h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap().and_hms_opt(h, m, s).unwrap()
    }

    fn run(scenario: &str, start: NaiveDateTime, secs: i64) -> RunTiming {
        RunTiming {
            run_id: 0,
            scenario: scenario.into(),
            started_at: start,
            finished_at: start + Duration::seconds(secs),
            active_s: secs as f64,
            restarted: false,
        }
    }

    #[test]
    fn splits_sessions_on_long_breaks() {
        let s = TimeSettings::default();
        let runs = vec![
            run("a", t(2, 7, 0, 0), 60),
            run("a", t(2, 7, 10, 0), 60), // 9 min after previous end: same session
            run("a", t(2, 7, 30, 0), 60), // 19 min gap: new session
        ];
        let sessions = group_sessions(&runs, &s);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].runs, 2);
        assert_eq!(sessions[0].length_s(), 11.0 * 60.0);
        assert_eq!(sessions[0].active_s, 120.0);
        assert_eq!(sessions[1].runs, 1);
    }

    #[test]
    fn late_night_counts_toward_previous_day() {
        let s = TimeSettings::default();
        assert_eq!(s.training_day(t(3, 1, 30, 0)), NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
        assert_eq!(s.training_day(t(3, 4, 0, 0)), NaiveDate::from_ymd_opt(2026, 11, 3).unwrap());
    }

    #[test]
    fn flags_restarts_per_scenario() {
        let s = TimeSettings::default();
        let mut runs = vec![
            run("long", t(2, 7, 0, 0), 60),
            run("long", t(2, 7, 2, 0), 60),
            run("long", t(2, 7, 4, 0), 8), // quit early
            run("short", t(2, 7, 6, 0), 1),
            run("short", t(2, 7, 7, 0), 2),
            run("short", t(2, 7, 8, 0), 1),
            run("rare", t(2, 7, 9, 0), 5), // too few runs to judge
        ];
        flag_restarts(&mut runs, &s);
        let flagged: Vec<_> = runs.iter().filter(|r| r.restarted).map(|r| (r.scenario.as_str(), r.active_s)).collect();
        assert_eq!(flagged, vec![("long", 8.0)]);
    }

    #[test]
    fn streaks_and_goals() {
        let s = TimeSettings::default();
        let runs = vec![
            run("a", t(1, 8, 0, 0), 40 * 60),
            run("a", t(2, 8, 0, 0), 10 * 60),
            run("a", t(3, 8, 0, 0), 10 * 60),
            run("a", t(5, 8, 0, 0), 10 * 60),
        ];
        let sessions = group_sessions(&runs, &s);
        let days = daily(&runs, &sessions, &s);
        assert_eq!(days.len(), 4);
        assert!(days[0].goal_met);
        assert!(!days[1].goal_met);

        let day = |d| NaiveDate::from_ymd_opt(2026, 11, d).unwrap();
        assert_eq!(streak(&days, day(5)), Streak { current: 1, best: 3, trained_today: true });
        assert_eq!(streak(&days, day(6)), Streak { current: 1, best: 3, trained_today: false });
        assert_eq!(streak(&days, day(7)), Streak { current: 0, best: 3, trained_today: false });
    }

    #[test]
    fn weekday_average_counts_missed_days_as_zero() {
        let s = TimeSettings::default();
        // Mon Nov 2 and Mon Nov 9 trained 20 and 0 minutes; checked on Tue Nov 10.
        let runs = vec![run("a", t(2, 8, 0, 0), 20 * 60)];
        let days = daily(&runs, &group_sessions(&runs, &s), &s);
        let avg = weekday_averages(&days, NaiveDate::from_ymd_opt(2026, 11, 10).unwrap());
        assert_eq!(avg[0], Some(10.0)); // Monday: (20 + 0) / 2
        assert_eq!(avg[1], Some(0.0)); // Tuesday Nov 3 only; today (Nov 10) not trained, so excluded
        assert_eq!(avg[6], Some(0.0));
    }

    #[test]
    fn pauses_are_subtracted_when_plausible() {
        assert_eq!(active_seconds(60.0, 10.0), 50.0);
        assert_eq!(active_seconds(60.0, 0.0), 60.0);
        assert_eq!(active_seconds(60.0, 5000.0), 60.0);
    }
}
