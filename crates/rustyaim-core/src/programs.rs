//! Progress for training programs, built only from the adaptive ratings earned
//! while training (no separate tests):
//!
//! - **Skill profile:** each scenario's rating as a % change from where you started
//!   on it (the end of your first day on it: its last three runs, so the first
//!   run's search for your level doesn't count), averaged into nine sub-skills, three
//!   groups (clicking, tracking, switching) and one overall number. Lifetimes and
//!   speeds are compared on a log scale with "better" positive, so a faster lifetime
//!   and a higher tracking speed both count as gains.
//! - **Week:** for each program day, runs done vs. planned, minutes, and how the
//!   day's main drills compare with the same day last week.
//! - **Session gains:** for each drill in a session, rep 1 vs. the last rep, and how
//!   much of last session's rating carried over.
//! - **Weekly summary and streak.**
//!
//! Dates are training days: a day starts at 4 AM, like the training-time report.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::db::{Db, DbError};

/// The nine sub-skills and their groups (mirrors `rustyaim_sim::scenario::Skill`).
pub const SKILLS: [(&str, &str); 9] = [
    ("static_clicking", "clicking"),
    ("dynamic_clicking", "clicking"),
    ("linear_clicking", "clicking"),
    ("precise_tracking", "tracking"),
    ("reactive_tracking", "tracking"),
    ("control_tracking", "tracking"),
    ("speed_switching", "switching"),
    ("evasive_switching", "switching"),
    ("stability_switching", "switching"),
];

const DAY_START_HOUR: i64 = 4;
const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f";

/// A day counts as done at this share of its planned runs.
pub const DONE_SHARE: f64 = 0.75;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SkillStat {
    pub skill: String,
    pub group: String,
    /// Change since you started, e.g. 0.12 = 12% better. `None` until a drill for
    /// this skill has been played on two different days (see `runs` for whether
    /// it's been trained at all).
    pub change: Option<f64>,
    pub scenarios: usize,
    pub runs: usize,
    pub last_trained: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct GroupStat {
    pub group: String,
    pub change: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SkillProfile {
    pub skills: Vec<SkillStat>,
    pub groups: Vec<GroupStat>,
    pub overall: Option<f64>,
}

impl SkillProfile {
    /// Skills from weakest to strongest: untrained first, then lowest change (a
    /// skill with only one day counts as no change). Ties go to the one trained
    /// longest ago.
    pub fn ranked(&self) -> Vec<String> {
        let mut s: Vec<&SkillStat> = self.skills.iter().collect();
        s.sort_by(|a, b| {
            let key = |x: &SkillStat| if x.runs == 0 { f64::NEG_INFINITY } else { x.change.unwrap_or(0.0) };
            key(a).total_cmp(&key(b)).then_with(|| a.last_trained.cmp(&b.last_trained))
        });
        s.into_iter().map(|x| x.skill.clone()).collect()
    }
}

#[derive(Debug, Clone)]
struct Row {
    run_id: i64,
    finished: NaiveDateTime,
    duration_s: f64,
    scenario: String,
    knob: Option<String>,
    rating: Option<f64>,
    adaptive: bool,
    skill: Option<String>,
    program: Option<String>,
    day: Option<String>,
    rep: Option<i64>,
    block: Option<String>,
}

/// "Better" on a log scale: positive is better for every knob.
fn h(knob: &str, v: f64) -> f64 {
    let l = v.max(1e-6).ln();
    if knob == "target_speed" { l } else { -l }
}

fn training_day(t: NaiveDateTime) -> NaiveDate {
    (t - Duration::hours(DAY_START_HOUR)).date()
}

fn load(db: &Db) -> Result<Vec<Row>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT r.id, r.finished_at, COALESCE(r.duration_s, 60), e.base_scenario, e.knob, e.rating, e.adaptive,
                e.skill, e.program, e.program_day, e.rep, e.block
         FROM engine_runs e JOIN runs r ON r.id = e.run_id ORDER BY r.finished_at, r.id",
    )?;
    let rows = stmt.query_map([], |r| {
        let finished: String = r.get(1)?;
        Ok((
            finished,
            Row {
                run_id: r.get(0)?,
                finished: NaiveDateTime::MIN,
                duration_s: r.get(2)?,
                scenario: r.get(3)?,
                knob: r.get(4)?,
                rating: r.get(5)?,
                adaptive: r.get(6)?,
                skill: r.get(7)?,
                program: r.get(8)?,
                day: r.get(9)?,
                rep: r.get(10)?,
                block: r.get(11)?,
            },
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (f, mut r) = row?;
        let Ok(t) = NaiveDateTime::parse_from_str(&f, TIME_FORMAT) else { continue };
        r.finished = t;
        out.push(r);
    }
    Ok(out)
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// The skill profile from runs finished up to `as_of` (all runs if `None`).
pub fn skill_profile(db: &Db, as_of: Option<NaiveDateTime>) -> Result<SkillProfile, DbError> {
    Ok(profile_from(&load(db)?, as_of))
}

fn profile_from(rows: &[Row], as_of: Option<NaiveDateTime>) -> SkillProfile {
    // Rated runs per scenario, in order.
    let mut by_scenario: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.adaptive && r.rating.is_some() && r.knob.is_some() && r.skill.is_some()) {
        if as_of.is_some_and(|t| r.finished > t) {
            continue;
        }
        by_scenario.entry(&r.scenario).or_default().push(r);
    }
    // Scenario change in log units, grouped by skill.
    let mut per_skill: HashMap<&str, Vec<f64>> = HashMap::new();
    let mut runs: HashMap<&str, usize> = HashMap::new();
    let mut last: HashMap<&str, NaiveDateTime> = HashMap::new();
    for rs in by_scenario.values() {
        // Only runs with the scenario's current knob count.
        let knob = rs.last().and_then(|r| r.knob.as_deref()).unwrap_or_default();
        let kept: Vec<&&Row> = rs.iter().filter(|r| r.knob.as_deref() == Some(knob)).collect();
        if kept.is_empty() {
            continue;
        }
        let skill = rs.last().and_then(|r| r.skill.as_deref()).unwrap_or_default();
        *runs.entry(skill).or_default() += rs.len();
        // Baseline: the last three runs of the first day. Now: the last three runs
        // of the latest day. Each is after that day's first run has found the level.
        let first_day = training_day(kept[0].finished);
        let last_day = training_day(kept[kept.len() - 1].finished);
        if first_day != last_day {
            let tail = |day: NaiveDate| {
                let v: Vec<f64> = kept
                    .iter()
                    .filter(|r| training_day(r.finished) == day)
                    .map(|r| h(knob, r.rating.unwrap_or(1.0)))
                    .collect();
                mean(&v[v.len().saturating_sub(3)..])
            };
            if let (Some(base), Some(now)) = (tail(first_day), tail(last_day)) {
                per_skill.entry(skill).or_default().push(now - base);
            }
        }
        let t = rs.last().map(|r| r.finished).unwrap_or_default();
        last.entry(skill).and_modify(|x| *x = (*x).max(t)).or_insert(t);
    }
    let skills: Vec<SkillStat> = SKILLS
        .iter()
        .map(|(id, group)| {
            let d = per_skill.get(id);
            SkillStat {
                skill: id.to_string(),
                group: group.to_string(),
                change: d.and_then(|v| mean(v)).map(|x| x.exp() - 1.0),
                scenarios: d.map_or(0, Vec::len),
                runs: runs.get(id).copied().unwrap_or(0),
                last_trained: last.get(id).map(|t| t.format(TIME_FORMAT).to_string()),
            }
        })
        .collect();
    let log = |c: Option<f64>| c.map(|c| (1.0 + c).ln());
    let groups: Vec<GroupStat> = ["clicking", "tracking", "switching"]
        .iter()
        .map(|g| {
            let v: Vec<f64> = skills.iter().filter(|s| s.group == *g).filter_map(|s| log(s.change)).collect();
            GroupStat { group: g.to_string(), change: mean(&v).map(|x| x.exp() - 1.0) }
        })
        .collect();
    let overall = mean(&groups.iter().filter_map(|g| log(g.change)).collect::<Vec<_>>()).map(|x| x.exp() - 1.0);
    SkillProfile { skills, groups, overall }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct DayStat {
    /// `mon` … `sun`.
    pub day: String,
    pub date: String,
    /// Runs planned (0 = rest day) and done.
    pub planned: u32,
    pub done: u32,
    pub minutes: f64,
    /// `done`, `partial`, `missed`, `today`, `upcoming` or `rest`.
    pub status: String,
    /// Main drills vs. the same day last week, e.g. 0.04 = 4% better.
    pub focus_change: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct DrillSession {
    pub scenario: String,
    pub knob: String,
    pub date: String,
    pub day: String,
    pub block: Option<String>,
    pub reps: usize,
    /// Rating after the first and the last rep.
    pub first: f64,
    pub last: f64,
    /// Last rep vs. first rep.
    pub within: f64,
    /// First rep vs. the last rep of the previous session of this drill: how much
    /// stuck since then (0 = all of it; negative = some was lost).
    pub carryover: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct WeekSummary {
    pub days_done: usize,
    pub days_planned: usize,
    pub minutes: f64,
    /// The skill that improved most this week, and by how much (vs. a week ago).
    pub best: Option<(String, f64)>,
    /// The weakest skill overall.
    pub lagging: Option<String>,
    /// Skills not trained in the last 7 days (or ever).
    pub stale: Vec<String>,
    /// Consecutive program days done, not counting rest days or today if it's not done yet.
    pub streak: usize,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ProgramProgress {
    pub program: String,
    pub today: String,
    pub week_start: String,
    pub week: Vec<DayStat>,
    pub profile: SkillProfile,
    pub sessions: Vec<DrillSession>,
    pub summary: WeekSummary,
}

/// Everything the Program page shows. `plan` gives the runs planned for each
/// weekday (`mon`…`sun`; missing or 0 = rest).
pub fn progress(
    db: &Db,
    program: &str,
    now: NaiveDateTime,
    plan: &[(String, u32)],
) -> Result<ProgramProgress, DbError> {
    let rows = load(db)?;
    let today = training_day(now);
    let week_start = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let planned = |day: &str| plan.iter().find(|(d, _)| d == day).map_or(0, |(_, n)| *n);
    let ids = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    let in_program: Vec<&Row> = rows.iter().filter(|r| r.program.as_deref() == Some(program)).collect();

    // Sessions: program runs grouped by training date.
    let mut by_date: BTreeMap<NaiveDate, Vec<&Row>> = BTreeMap::new();
    for r in &in_program {
        by_date.entry(training_day(r.finished)).or_default().push(r);
    }
    let sessions = drill_sessions(&by_date);

    let mut week = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let date = week_start + Duration::days(i as i64);
        let p = planned(id);
        // A program day counts whenever it's played that week, so a missed day can
        // be caught up later.
        let runs = day_runs(&in_program, week_start, id);
        let done = runs.len() as u32;
        let status = if p == 0 {
            "rest"
        } else if f64::from(done) >= f64::from(p) * DONE_SHARE {
            "done"
        } else if date == today {
            "today"
        } else if done > 0 {
            "partial"
        } else if date > today {
            "upcoming"
        } else {
            "missed"
        };
        let session_date = runs.iter().map(|r| training_day(r.finished)).max();
        let focus_change = session_date.and_then(|d| focus_change(&sessions, &d.to_string(), id));
        week.push(DayStat {
            day: id.to_string(),
            date: date.to_string(),
            planned: p,
            done,
            minutes: runs.iter().map(|r| r.duration_s).sum::<f64>() / 60.0,
            status: status.into(),
            focus_change,
        });
    }

    let profile = profile_from(&rows, None);
    let week_ago = profile_from(&rows, Some(now - Duration::days(7)));
    let best = profile
        .skills
        .iter()
        .filter_map(|s| {
            let then = week_ago.skills.iter().find(|w| w.skill == s.skill)?.change.unwrap_or(0.0);
            let now = s.change?;
            let gain = (1.0 + now) / (1.0 + then) - 1.0;
            (gain > 0.005).then(|| (s.skill.clone(), gain))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));
    let cutoff = now - Duration::days(7);
    let stale = profile
        .skills
        .iter()
        .filter(|s| {
            s.last_trained
                .as_deref()
                .and_then(|t| NaiveDateTime::parse_from_str(t, TIME_FORMAT).ok())
                .is_none_or(|t| t < cutoff)
        })
        .map(|s| s.skill.clone())
        .collect();
    let summary = WeekSummary {
        days_done: week.iter().filter(|d| d.status == "done").count(),
        days_planned: week.iter().filter(|d| d.planned > 0).count(),
        minutes: week.iter().map(|d| d.minutes).sum(),
        best,
        lagging: profile.skills.iter().any(|s| s.runs > 0).then(|| profile.ranked()[0].clone()),
        stale,
        streak: streak(&in_program, today, &planned),
    };
    Ok(ProgramProgress {
        program: program.to_string(),
        today: today.to_string(),
        week_start: week_start.to_string(),
        week,
        profile,
        sessions: sessions.into_iter().rev().take(60).collect(),
        summary,
    })
}

fn drill_sessions(by_date: &BTreeMap<NaiveDate, Vec<&Row>>) -> Vec<DrillSession> {
    let mut out: Vec<DrillSession> = Vec::new();
    let mut last_seen: HashMap<String, f64> = HashMap::new();
    for (date, rows) in by_date {
        let mut per: BTreeMap<(&str, &str), Vec<&Row>> = BTreeMap::new();
        for r in rows.iter().filter(|r| r.adaptive && r.rating.is_some() && r.knob.is_some()) {
            per.entry((r.scenario.as_str(), r.day.as_deref().unwrap_or(""))).or_default().push(r);
        }
        let mut sessions: Vec<(i64, DrillSession)> = Vec::new();
        for ((scenario, day), mut rs) in per {
            rs.sort_by_key(|r| (r.rep.unwrap_or(0), r.run_id));
            let knob = rs[0].knob.clone().unwrap_or_default();
            let first = rs[0].rating.unwrap_or(0.0);
            let last = rs[rs.len() - 1].rating.unwrap_or(0.0);
            let carryover = last_seen.get(scenario).map(|prev| (h(&knob, first) - h(&knob, *prev)).exp() - 1.0);
            sessions.push((
                rs[0].run_id,
                DrillSession {
                    scenario: scenario.to_string(),
                    knob: knob.clone(),
                    date: date.to_string(),
                    day: day.to_string(),
                    block: rs[0].block.clone(),
                    reps: rs.len(),
                    first,
                    last,
                    within: (h(&knob, last) - h(&knob, first)).exp() - 1.0,
                    carryover,
                },
            ));
            last_seen.insert(scenario.to_string(), last);
        }
        sessions.sort_by_key(|(id, _)| *id);
        out.extend(sessions.into_iter().map(|(_, s)| s));
    }
    out
}

/// Main drills of the session on `date` vs. the last earlier session on the same weekday.
fn focus_change(sessions: &[DrillSession], date: &str, day: &str) -> Option<f64> {
    let this: Vec<&DrillSession> =
        sessions.iter().filter(|s| s.date == date && s.day == day && s.block.as_deref() == Some("main")).collect();
    if this.is_empty() {
        return None;
    }
    let prev_date =
        sessions.iter().filter(|s| s.day == day && s.date.as_str() < date).map(|s| s.date.as_str()).max()?;
    let diffs: Vec<f64> = this
        .iter()
        .filter_map(|s| {
            let p = sessions.iter().find(|p| p.date == prev_date && p.scenario == s.scenario)?;
            Some(h(&s.knob, s.last) - h(&s.knob, p.last))
        })
        .collect();
    mean(&diffs).map(|x| x.exp() - 1.0)
}

/// Program runs for weekday `id` played in the week starting `week_start`.
fn day_runs<'a>(rows: &[&'a Row], week_start: NaiveDate, id: &str) -> Vec<&'a Row> {
    let end = week_start + Duration::days(7);
    rows.iter()
        .filter(|r| r.day.as_deref() == Some(id))
        .filter(|r| {
            let d = training_day(r.finished);
            d >= week_start && d < end
        })
        .copied()
        .collect()
}

/// Program days completed in a row, in program order, going back week by week. Days
/// still ahead this week (and today, if not done yet) don't break it.
fn streak(rows: &[&Row], today: NaiveDate, planned: &dyn Fn(&str) -> u32) -> usize {
    let ids = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    if ids.iter().all(|d| planned(d) == 0) {
        return 0;
    }
    let this_week = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let today_index = today.weekday().num_days_from_monday() as usize;
    let mut n = 0;
    for w in 0..60 {
        let start = this_week - Duration::days(7 * w);
        for (i, id) in ids.iter().enumerate().rev() {
            let p = planned(id);
            if p == 0 {
                continue;
            }
            let done = day_runs(rows, start, id).len() as u32;
            if f64::from(done) >= f64::from(p) * DONE_SHARE {
                n += 1;
            } else if w == 0 && i >= today_index {
                continue;
            } else {
                return n;
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::RunFile;
    use crate::telemetry::EngineRun;

    struct Run<'a> {
        at: NaiveDateTime,
        scenario: &'a str,
        skill: &'a str,
        knob: &'a str,
        rating: f64,
        day: &'a str,
        rep: i64,
        block: &'a str,
    }

    fn save(db: &mut Db, r: &Run) {
        let file = format!("rustyaim/{}/{}/{}", r.scenario, r.at, r.rep);
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("Source".into(), "rustyaim".into());
        fields.insert("Challenge Start".into(), (r.at - Duration::seconds(60)).format("%H:%M:%S%.3f").to_string());
        let run = RunFile {
            file_name: file.clone(),
            scenario: format!("{} (Adaptive)", r.scenario),
            mode: "RustyAim Adaptive".into(),
            finished_at: r.at,
            fields,
            kills: vec![],
            weapons: vec![],
        };
        db.insert_runs(&[run], r.at).unwrap();
        let e = EngineRun {
            base_scenario: r.scenario.into(),
            category: "x".into(),
            adaptive: true,
            knob: Some(r.knob.into()),
            rating: Some(r.rating),
            skill: Some(r.skill.into()),
            program: Some("Core Week".into()),
            program_day: Some(r.day.into()),
            rep: Some(r.rep),
            reps: Some(5),
            block: Some(r.block.into()),
            ..Default::default()
        };
        assert!(db.save_engine_run(&file, &e).unwrap());
    }

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 0).unwrap()
    }

    /// Two Mondays (Nov 2 and Nov 9, 2026) of Timed Flicks getting faster, and one
    /// Tuesday of Strafe Track.
    fn db() -> Db {
        let mut db = Db::open_in_memory().unwrap();
        for (day, base) in [(2, 0.80), (9, 0.70)] {
            for rep in 1..=5 {
                let rating = base - 0.02 * f64::from(rep as u32);
                let r = Run {
                    at: at(2026, 11, day, 18, rep as u32 * 2),
                    scenario: "Timed Flicks",
                    skill: "static_clicking",
                    knob: "lifetime",
                    rating,
                    day: "mon",
                    rep,
                    block: "main",
                };
                save(&mut db, &r);
            }
        }
        for rep in 1..=5 {
            let r = Run {
                at: at(2026, 11, 3, 18, rep as u32 * 2),
                scenario: "Strafe Track",
                skill: "reactive_tracking",
                knob: "target_speed",
                rating: 20.0 + f64::from(rep as u32),
                day: "tue",
                rep,
                block: "main",
            };
            save(&mut db, &r);
        }
        db
    }

    fn plan() -> Vec<(String, u32)> {
        ["mon", "tue", "wed", "thu", "fri", "sat"].iter().map(|d| (d.to_string(), 5)).collect()
    }

    #[test]
    fn skill_profile_counts_faster_lifetimes_and_higher_speeds_as_gains() {
        let p = skill_profile(&db(), None).unwrap();
        let s = |id: &str| p.skills.iter().find(|s| s.skill == id).unwrap();
        // Timed Flicks: the first day ends around 0.72 s (last three runs), the
        // latest around 0.62 s (geometric means): 0.72/0.62 − 1.
        let st = s("static_clicking").change.unwrap();
        assert!((st - (0.72 / 0.62 - 1.0)).abs() < 2e-3, "{st}");
        // Strafe Track was only played on one day: trained, but no change yet.
        assert_eq!((s("reactive_tracking").change, s("reactive_tracking").runs), (None, 5));
        assert_eq!(s("speed_switching").change, None);
        assert!(p.groups.iter().find(|g| g.group == "clicking").unwrap().change.unwrap() > 0.0);
        assert!(p.overall.unwrap() > 0.0);
        assert_eq!(p.ranked()[0], "dynamic_clicking", "untrained skills are weakest");
        let ranked = p.ranked();
        let pos = |id: &str| ranked.iter().position(|x| x == id).unwrap();
        assert!(pos("reactive_tracking") < pos("static_clicking"), "one day counts as no change: {ranked:?}");
        assert!(pos("speed_switching") < pos("reactive_tracking"), "untrained before trained: {ranked:?}");
    }

    #[test]
    fn week_shows_progress_against_last_week() {
        let p = progress(&db(), "Core Week", at(2026, 11, 10, 12, 0), &plan()).unwrap();
        assert_eq!(p.week_start, "2026-11-09");
        let mon = &p.week[0];
        assert_eq!((mon.status.as_str(), mon.done, mon.planned), ("done", 5, 5));
        // Monday's last rep: 0.60 s this week vs. 0.70 s last week.
        assert!((mon.focus_change.unwrap() - (0.70 / 0.60 - 1.0)).abs() < 1e-6, "{:?}", mon.focus_change);
        assert_eq!(p.week[1].status, "today");
        assert_eq!(p.week[2].status, "upcoming");
        assert_eq!(p.week[6].status, "rest");
        assert_eq!(p.summary.streak, 1, "Monday this week; last Tuesday onwards were missed");
        assert_eq!(p.summary.days_done, 1);
        assert!(p.summary.stale.contains(&"speed_switching".to_string()));
        assert_eq!(p.summary.best.as_ref().map(|b| b.0.as_str()), Some("static_clicking"));
    }

    #[test]
    fn sessions_measure_reps_and_carryover() {
        let p = progress(&db(), "Core Week", at(2026, 11, 10, 12, 0), &plan()).unwrap();
        let s = p.sessions.iter().find(|s| s.date == "2026-11-09").unwrap();
        assert_eq!(s.reps, 5);
        // 0.68 s after rep 1, 0.60 s after rep 5.
        assert!((s.within - (0.68 / 0.60 - 1.0)).abs() < 1e-6);
        // Last Monday ended at 0.70 s; this Monday's first rep was 0.68 s.
        assert!((s.carryover.unwrap() - (0.70 / 0.68 - 1.0)).abs() < 1e-6);
        let first = p.sessions.iter().find(|s| s.date == "2026-11-02").unwrap();
        assert_eq!(first.carryover, None);
    }

    #[test]
    fn a_missed_day_can_be_caught_up_later_that_week() {
        let mut db = db();
        // Tuesday's workout played on Thursday (Nov 12).
        for rep in 1..=5 {
            let r = Run {
                at: at(2026, 11, 12, 18, rep as u32 * 2),
                scenario: "Strafe Track",
                skill: "reactive_tracking",
                knob: "target_speed",
                rating: 26.0 + f64::from(rep as u32),
                day: "tue",
                rep,
                block: "main",
            };
            save(&mut db, &r);
        }
        let p = progress(&db, "Core Week", at(2026, 11, 12, 20, 0), &plan()).unwrap();
        assert_eq!(p.week[1].status, "done", "{:?}", p.week[1]);
        // 30 °/s at the end vs. 25 °/s last Tuesday.
        assert!((p.week[1].focus_change.unwrap() - (31.0 / 25.0 - 1.0)).abs() < 1e-6, "{:?}", p.week[1]);
        assert_eq!(p.week[2].status, "missed", "Wednesday was skipped");
        assert_eq!(p.summary.streak, 0, "Wednesday breaks it, Thursday (today) doesn't count yet");
    }

    #[test]
    fn early_morning_counts_as_the_day_before() {
        assert_eq!(training_day(at(2026, 11, 10, 3, 59)).to_string(), "2026-11-09");
        assert_eq!(training_day(at(2026, 11, 10, 4, 0)).to_string(), "2026-11-10");
    }
}
