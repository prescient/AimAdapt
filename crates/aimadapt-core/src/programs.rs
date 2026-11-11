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

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::db::{Db, DbError};

/// The nine sub-skills and their groups (mirrors `aimadapt_sim::scenario::Skill`).
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

use crate::db::TIME_FORMAT;
use crate::knob::Knob;
use crate::training::TimeSettings;

pub mod recap;

/// A day counts as done at this share of its planned runs.
pub const DONE_SHARE: f64 = 0.75;

/// Sessions of a drill before its carry-over is named, and of a skill's drills before
/// the skill is ranked by its change.
pub const MIN_SESSIONS: usize = 3;

/// A change's likely range: one standard deviation either side of it, as fractions
/// like the change itself (0.02 = 2% better). `noise` is true when zero is inside the
/// range, so the change can't be told from run-to-run noise.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spread {
    pub low: f64,
    pub high: f64,
    pub noise: bool,
}

impl Spread {
    /// The range of a change `m` with standard deviation `sd`, both in log units.
    fn of(m: f64, sd: f64) -> Spread {
        Spread { low: (m - sd).exp() - 1.0, high: (m + sd).exp() - 1.0, noise: m.abs() <= sd }
    }

    /// The range of change `m` when its standard deviation is known.
    fn maybe(m: f64, sd: Option<f64>) -> Option<Spread> {
        sd.map(|sd| Spread::of(m, sd))
    }

    /// The standard deviation in log units, back from the range.
    fn sd(&self) -> f64 {
        ((1.0 + self.high).ln() - (1.0 + self.low).ln()) / 2.0
    }
}

/// The standard deviation of a difference of two ratings. The two sides are treated as
/// independent, which errs toward a wider range.
fn sd_of_diff(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    Some((a? * a? + b? * b?).sqrt())
}

/// The standard deviation of the mean of ratings, `None` when the list is empty or any
/// entry is unknown.
fn sd_of_mean(sds: &[Option<f64>]) -> Option<f64> {
    let mut sum = 0.0;
    for sd in sds {
        let sd = (*sd)?;
        sum += sd * sd;
    }
    (!sds.is_empty()).then(|| sum.sqrt() / sds.len() as f64)
}

/// The mean of changes (log units) and the range of that mean, from each change's standard deviation.
fn spread_of_mean(items: &[(f64, Option<f64>)]) -> Option<Spread> {
    let m = mean(&items.iter().map(|i| i.0).collect::<Vec<_>>())?;
    Spread::maybe(m, sd_of_mean(&items.iter().map(|i| i.1).collect::<Vec<_>>()))
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SkillStat {
    pub skill: String,
    pub group: String,
    /// Change since you started, e.g. 0.12 = 12% better. `None` until a drill for
    /// this skill has been played on two different days (see `runs` for whether
    /// it's been trained at all).
    pub change: Option<f64>,
    /// The likely range of `change`. `None` when `change` is, or when the runs behind
    /// it were saved without a range.
    pub spread: Option<Spread>,
    pub scenarios: usize,
    pub runs: usize,
    /// The most sessions (training days) on any one of its drills: 0 when untrained.
    pub sessions: usize,
    pub last_trained: Option<String>,
    /// The standard deviation (log units) of where the skill stands now, over the
    /// drills that have a change. The week-on-week gain's range is built from it.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub latest_sd: Option<f64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct GroupStat {
    pub group: String,
    pub change: Option<f64>,
    pub spread: Option<Spread>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SkillProfile {
    pub skills: Vec<SkillStat>,
    pub groups: Vec<GroupStat>,
    pub overall: Option<f64>,
    pub overall_spread: Option<Spread>,
}

impl SkillProfile {
    /// Skills most in need first: the least trained, then the least improved. Untrained
    /// skills come first, then skills with fewer than [`MIN_SESSIONS`] sessions (fewest
    /// first), then the rest by change, smallest first. A skill is ranked by its change
    /// only once it has three sessions, because a change measured on fewer is mostly
    /// noise. Ties go to the one trained longest ago.
    pub fn ranked(&self) -> Vec<String> {
        let mut s: Vec<&SkillStat> = self.skills.iter().collect();
        s.sort_by(|a, b| {
            let key = |x: &SkillStat| match x.sessions {
                _ if x.runs == 0 => (0, 0.0),
                n if n < MIN_SESSIONS => (1, n as f64),
                _ => (2, x.change.unwrap_or(0.0)),
            };
            let (ka, kb) = (key(a), key(b));
            ka.0.cmp(&kb.0).then_with(|| ka.1.total_cmp(&kb.1)).then_with(|| a.last_trained.cmp(&b.last_trained))
        });
        s.into_iter().map(|x| x.skill.clone()).collect()
    }

    /// The skill with the smallest change among those with at least [`MIN_SESSIONS`]
    /// sessions. Named only when two skills have that many: with one there is nothing
    /// to compare it with.
    pub fn least_improved(&self) -> Option<&SkillStat> {
        let ready: Vec<&SkillStat> =
            self.skills.iter().filter(|s| s.sessions >= MIN_SESSIONS && s.change.is_some()).collect();
        if ready.len() < 2 {
            return None;
        }
        ready.into_iter().min_by(|a, b| {
            a.change
                .unwrap_or(0.0)
                .total_cmp(&b.change.unwrap_or(0.0))
                .then_with(|| a.last_trained.cmp(&b.last_trained))
        })
    }

    /// Sessions still needed before a least improved skill can be named: what the two
    /// most-trained skills each lack to reach [`MIN_SESSIONS`].
    fn sessions_needed(&self) -> usize {
        let mut n: Vec<usize> = self.skills.iter().map(|s| s.sessions).collect();
        n.sort_unstable_by(|a, b| b.cmp(a));
        n.iter().take(2).map(|n| MIN_SESSIONS.saturating_sub(*n)).sum()
    }
}

#[derive(Debug, Clone)]
struct Row {
    run_id: i64,
    finished: NaiveDateTime,
    /// The training day it counts for (see [`TimeSettings::day_start_hour`]).
    date: NaiveDate,
    duration_s: f64,
    scenario: String,
    knob: Option<String>,
    rating: Option<f64>,
    /// The scenario's settings fingerprint: its version. Ratings of different
    /// versions aren't comparable.
    hash: Option<String>,
    adaptive: bool,
    skill: Option<String>,
    program: Option<String>,
    day: Option<String>,
    rep: Option<i64>,
    block: Option<String>,
    /// The rating's standard deviation in the knob's log units, when known.
    sd: Option<f64>,
    /// Set when the run was played at one value (not a calibration run).
    run_value: Option<f64>,
    /// The value the goal was held at for the whole run.
    fastest: Option<f64>,
    /// The run's score.
    score: Option<f64>,
}

/// A rating's standard deviation: the stored one when it is usable, otherwise half the
/// distance between the stored range ends on the knob's log scale.
fn rating_sd(knob: Option<&str>, sd: Option<f64>, low: Option<f64>, high: Option<f64>) -> Option<f64> {
    if let Some(sd) = sd.filter(|s| s.is_finite() && *s > 0.0) {
        return Some(sd);
    }
    let knob = Knob::from_id(knob?);
    Some((knob.h(high?) - knob.h(low?)).abs() / 2.0).filter(|s| s.is_finite())
}

fn load(db: &Db, time: &TimeSettings) -> Result<Vec<Row>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT r.id, r.finished_at, COALESCE(r.duration_s, 60), e.base_scenario, e.knob, e.rating, e.adaptive,
                e.skill, e.program, e.program_day, e.rep, e.block, r.scenario_hash,
                e.rating_sd, e.rating_low, e.rating_high, e.run_value, e.fastest, r.score
         FROM engine_runs e JOIN runs r ON r.id = e.run_id ORDER BY r.finished_at, r.id",
    )?;
    let rows = stmt.query_map([], |r| {
        let finished: String = r.get(1)?;
        let knob: Option<String> = r.get(4)?;
        let sd = rating_sd(knob.as_deref(), r.get(13)?, r.get(14)?, r.get(15)?);
        Ok((
            finished,
            Row {
                run_id: r.get(0)?,
                finished: NaiveDateTime::MIN,
                date: NaiveDate::MIN,
                duration_s: r.get(2)?,
                scenario: r.get(3)?,
                knob,
                rating: r.get(5)?,
                adaptive: r.get(6)?,
                skill: r.get(7)?,
                program: r.get(8)?,
                day: r.get(9)?,
                rep: r.get(10)?,
                block: r.get(11)?,
                hash: r.get(12)?,
                sd,
                run_value: r.get(16)?,
                fastest: r.get(17)?,
                score: r.get(18)?,
            },
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (f, mut r) = row?;
        let Ok(t) = NaiveDateTime::parse_from_str(&f, TIME_FORMAT) else { continue };
        r.finished = t;
        r.date = time.training_day(t);
        out.push(r);
    }
    Ok(out)
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// The skill profile from runs finished up to `as_of` (all runs if `None`).
pub fn skill_profile(db: &Db, as_of: Option<NaiveDateTime>, time: &TimeSettings) -> Result<SkillProfile, DbError> {
    Ok(profile_from(&load(db, time)?, as_of))
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
    let mut per_skill: HashMap<&str, Vec<ScenarioChange>> = HashMap::new();
    let mut runs: HashMap<&str, usize> = HashMap::new();
    let mut sessions: HashMap<&str, usize> = HashMap::new();
    let mut last: HashMap<&str, NaiveDateTime> = HashMap::new();
    for rs in by_scenario.values() {
        // Only runs with the scenario's current knob count,
        let knob = rs.last().and_then(|r| r.knob.as_deref()).unwrap_or_default();
        // and of its current version (the latest run's settings fingerprint).
        let hash = rs.last().and_then(|r| r.hash.as_deref());
        let kept: Vec<&&Row> =
            rs.iter().filter(|r| r.knob.as_deref() == Some(knob) && r.hash.as_deref() == hash).collect();
        if kept.is_empty() {
            continue;
        }
        let skill = rs.last().and_then(|r| r.skill.as_deref()).unwrap_or_default();
        *runs.entry(skill).or_default() += rs.len();
        let days: BTreeSet<NaiveDate> = kept.iter().map(|r| r.date).collect();
        let n = sessions.entry(skill).or_default();
        *n = (*n).max(days.len());
        if let Some(c) = scenario_change(&kept, knob) {
            per_skill.entry(skill).or_default().push(c);
        }
        let t = rs.last().map(|r| r.finished).unwrap_or_default();
        last.entry(skill).and_modify(|x| *x = (*x).max(t)).or_insert(t);
    }
    let skills: Vec<SkillStat> = SKILLS
        .iter()
        .map(|(id, group)| {
            let d = per_skill.get(id).map(Vec::as_slice).unwrap_or_default();
            let logs: Vec<(f64, Option<f64>)> = d.iter().map(|c| (c.change, c.sd)).collect();
            let spread = spread_of_mean(&logs);
            let latest: Vec<Option<f64>> = d.iter().map(|c| c.latest_sd).collect();
            SkillStat {
                skill: id.to_string(),
                group: group.to_string(),
                change: mean(&logs.iter().map(|c| c.0).collect::<Vec<_>>()).map(|x| x.exp() - 1.0),
                spread,
                scenarios: d.len(),
                runs: runs.get(id).copied().unwrap_or(0),
                sessions: sessions.get(id).copied().unwrap_or(0),
                last_trained: last.get(id).map(|t| t.format(TIME_FORMAT).to_string()),
                latest_sd: sd_of_mean(&latest),
            }
        })
        .collect();
    let log = |c: Option<f64>| c.map(|c| (1.0 + c).ln());
    // A group's (or the overall) mean of its parts' log changes, with its range.
    let combine = |parts: Vec<(Option<f64>, Option<Spread>)>| {
        let items: Vec<(f64, Option<f64>)> =
            parts.iter().filter_map(|(c, sp)| Some((log(*c)?, sp.map(|s| s.sd())))).collect();
        (mean(&items.iter().map(|i| i.0).collect::<Vec<_>>()).map(|x| x.exp() - 1.0), spread_of_mean(&items))
    };
    let groups: Vec<GroupStat> = ["clicking", "tracking", "switching"]
        .iter()
        .map(|g| {
            let (change, spread) =
                combine(skills.iter().filter(|s| s.group == *g).map(|s| (s.change, s.spread)).collect());
            GroupStat { group: g.to_string(), change, spread }
        })
        .collect();
    let (overall, overall_spread) = combine(groups.iter().map(|g| (g.change, g.spread)).collect());
    SkillProfile { skills, groups, overall, overall_spread }
}

/// One drill's change since its first day.
struct ScenarioChange {
    /// In log units, better positive.
    change: f64,
    /// Its standard deviation, from the two days' tails.
    sd: Option<f64>,
    /// The standard deviation of where the drill stands now.
    latest_sd: Option<f64>,
}

/// A drill's change from the end of its first day to the end of its latest day, from its
/// runs (of one knob and version, in order). Each end is the last three runs of that day,
/// after the day's first run has found the level. `None` on a single day.
fn scenario_change(kept: &[&&Row], knob: &str) -> Option<ScenarioChange> {
    let (first_day, last_day) = (kept[0].date, kept[kept.len() - 1].date);
    if first_day == last_day {
        return None;
    }
    let knob = Knob::from_id(knob);
    // The value of a day's tail and its standard deviation. The deviation is a mean and
    // not a root-sum-of-squares: a rating is a running estimate, so three runs in a row
    // share most of their error.
    let tail = |day: NaiveDate| {
        let v: Vec<&&Row> = kept.iter().filter(|r| r.date == day).copied().collect();
        let v = &v[v.len().saturating_sub(3)..];
        let value = mean(&v.iter().map(|r| knob.h(r.rating.unwrap_or(1.0))).collect::<Vec<_>>())?;
        Some((value, mean(&v.iter().filter_map(|r| r.sd).collect::<Vec<_>>())))
    };
    let ((base, base_sd), (now, now_sd)) = (tail(first_day)?, tail(last_day)?);
    Some(ScenarioChange { change: now - base, sd: sd_of_diff(base_sd, now_sd), latest_sd: now_sd })
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
    /// The likely range of `focus_change`.
    pub focus_spread: Option<Spread>,
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
    pub within_spread: Option<Spread>,
    /// First rep vs. the last rep of the previous session of this drill: how much
    /// stuck since then (0 = all of it; negative = some was lost).
    pub carryover: Option<f64>,
    pub carryover_spread: Option<Spread>,
    /// The scenario's settings fingerprint for this session's runs (not shown).
    #[cfg_attr(feature = "serde", serde(skip))]
    pub version: Option<String>,
    /// The standard deviation of the first and the last rep's rating (not shown).
    #[cfg_attr(feature = "serde", serde(skip))]
    pub first_sd: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub last_sd: Option<f64>,
}

/// How much of a drill's rating carries from one session to the next, averaged over
/// its latest sessions. Named only once the drill has `MIN_SESSIONS` sessions.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct CarryStat {
    pub scenario: String,
    /// Sessions of the drill's current version.
    pub sessions: usize,
    /// Sessions still needed before `kept` is shown (0 once it is).
    pub needed: usize,
    /// Mean carry-over across its latest sessions (up to five gaps).
    pub kept: Option<f64>,
    pub spread: Option<Spread>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct WeekSummary {
    pub days_done: usize,
    pub days_planned: usize,
    pub minutes: f64,
    /// The skill that improved most this week, and by how much (vs. a week ago).
    pub best: Option<(String, f64)>,
    /// The likely range of `best`'s gain.
    pub best_spread: Option<Spread>,
    /// The least improved skill: see [`SkillProfile::least_improved`]. `None` until two
    /// skills have three sessions each.
    pub lagging: Option<String>,
    /// Sessions still needed before `lagging` can be named (0 once it is).
    pub lagging_needs: usize,
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
    /// Carry-over for each drill in `sessions`, most recently played first.
    pub carry: Vec<CarryStat>,
    pub summary: WeekSummary,
}

/// Everything the Program page shows. `plan` gives the runs planned for each
/// weekday (`mon`…`sun`; missing or 0 = rest).
pub fn progress(
    db: &Db,
    program: &str,
    now: NaiveDateTime,
    plan: &[(String, u32)],
    time: &TimeSettings,
) -> Result<ProgramProgress, DbError> {
    let rows = load(db, time)?;
    let today = time.training_day(now);
    let week_start = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let planned = |day: &str| plan.iter().find(|(d, _)| d == day).map_or(0, |(_, n)| *n);
    let ids = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    let in_program: Vec<&Row> = rows.iter().filter(|r| r.program.as_deref() == Some(program)).collect();

    // Sessions: program runs grouped by training date.
    let mut by_date: BTreeMap<NaiveDate, Vec<&Row>> = BTreeMap::new();
    for r in &in_program {
        by_date.entry(r.date).or_default().push(r);
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
        let session_date = runs.iter().map(|r| r.date).max();
        let (focus_change, focus_spread) = session_date
            .and_then(|d| focus_change(&sessions, &d.to_string(), id))
            .map_or((None, None), |(c, sp)| (Some(c), sp));
        week.push(DayStat {
            day: id.to_string(),
            date: date.to_string(),
            planned: p,
            done,
            minutes: runs.iter().map(|r| r.duration_s).sum::<f64>() / 60.0,
            status: status.into(),
            focus_change,
            focus_spread,
        });
    }

    let profile = profile_from(&rows, None);
    let week_ago = profile_from(&rows, Some(now - Duration::days(7)));
    let (best, best_spread) = match best_gain(&profile, &week_ago) {
        Some((skill, gain, spread)) => (Some((skill, gain)), spread),
        None => (None, None),
    };
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
        best_spread,
        lagging: profile.least_improved().map(|s| s.skill.clone()),
        lagging_needs: profile.sessions_needed(),
        stale,
        streak: streak(&in_program, today, &planned),
    };
    let shown: Vec<DrillSession> = sessions.iter().rev().take(60).cloned().collect();
    let carry = carry_stats(&sessions, &shown);
    Ok(ProgramProgress {
        program: program.to_string(),
        today: today.to_string(),
        week_start: week_start.to_string(),
        week,
        profile,
        sessions: shown,
        carry,
        summary,
    })
}

/// The skill that gained most since a week ago, its gain and the gain's range. A skill
/// qualifies when its gain is above 0.5% and, if its range is known, the range excludes
/// zero.
fn best_gain(profile: &SkillProfile, week_ago: &SkillProfile) -> Option<(String, f64, Option<Spread>)> {
    profile
        .skills
        .iter()
        .filter_map(|s| {
            let then = week_ago.skills.iter().find(|w| w.skill == s.skill)?;
            let gain = (1.0 + s.change?) / (1.0 + then.change.unwrap_or(0.0)) - 1.0;
            // A drill not played this week adds its sd on both sides, which errs toward not
            // naming a gain. With no change a week ago, the whole change is the gain.
            let sd = match then.change {
                Some(_) => sd_of_diff(s.latest_sd, then.latest_sd),
                None => s.spread.map(|p| p.sd()),
            };
            let log_gain = gain.ln_1p();
            (gain > 0.005 && sd.is_none_or(|sd| log_gain > sd))
                .then(|| (s.skill.clone(), gain, Spread::maybe(log_gain, sd)))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Carry-over by drill, for each drill in `shown` (the latest sessions), most recently
/// played first. `sessions` is every session in order. Sessions are counted over the
/// drill's current version (that of its latest session), one per training day.
fn carry_stats(sessions: &[DrillSession], shown: &[DrillSession]) -> Vec<CarryStat> {
    let mut seen: Vec<&DrillSession> = Vec::new();
    for s in shown {
        if !seen.iter().any(|x| x.scenario == s.scenario) {
            seen.push(s);
        }
    }
    seen.into_iter()
        .map(|latest| {
            let own: Vec<&DrillSession> =
                sessions.iter().filter(|s| s.scenario == latest.scenario && s.version == latest.version).collect();
            let n = own.iter().map(|s| s.date.as_str()).collect::<BTreeSet<_>>().len();
            let needed = MIN_SESSIONS.saturating_sub(n);
            // The last up to five gaps, in log units.
            let gaps: Vec<(f64, Option<f64>)> = own
                .iter()
                .filter_map(|s| Some(((1.0 + s.carryover?).ln(), s.carryover_spread.map(|p| p.sd()))))
                .collect();
            let gaps = &gaps[gaps.len().saturating_sub(5)..];
            let ready = needed == 0;
            CarryStat {
                scenario: latest.scenario.clone(),
                sessions: n,
                needed,
                kept: mean(&gaps.iter().map(|g| g.0).collect::<Vec<_>>()).filter(|_| ready).map(|x| x.exp() - 1.0),
                spread: spread_of_mean(gaps).filter(|_| ready),
            }
        })
        .collect()
}

fn drill_sessions(by_date: &BTreeMap<NaiveDate, Vec<&Row>>) -> Vec<DrillSession> {
    let mut out: Vec<DrillSession> = Vec::new();
    // Each drill's last rating, version and rating sd.
    let mut last_seen: HashMap<String, (f64, Option<String>, Option<f64>)> = HashMap::new();
    for (date, rows) in by_date {
        let mut per: BTreeMap<(&str, &str), Vec<&Row>> = BTreeMap::new();
        for r in rows.iter().filter(|r| r.adaptive && r.rating.is_some() && r.knob.is_some()) {
            per.entry((r.scenario.as_str(), r.day.as_deref().unwrap_or(""))).or_default().push(r);
        }
        let mut sessions: Vec<(i64, DrillSession)> = Vec::new();
        for ((scenario, day), mut rs) in per {
            rs.sort_by_key(|r| (r.rep.unwrap_or(0), r.run_id));
            // A session that spans an edit counts only the runs of its latest version.
            let version = rs[rs.len() - 1].hash.clone();
            rs.retain(|r| r.hash == version);
            let knob = rs[0].knob.clone().unwrap_or_default();
            let first = rs[0].rating.unwrap_or(0.0);
            let last = rs[rs.len() - 1].rating.unwrap_or(0.0);
            let (first_sd, last_sd) = (rs[0].sd, rs[rs.len() - 1].sd);
            let h = |v: f64| Knob::from_id(&knob).h(v);
            let within = h(last) - h(first);
            // First rep vs. where the previous session of this version ended.
            let carry = last_seen.get(scenario).filter(|(_, v, _)| *v == version).map(|(prev, _, prev_sd)| {
                let m = h(first) - h(*prev);
                (m.exp() - 1.0, Spread::maybe(m, sd_of_diff(first_sd, *prev_sd)))
            });
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
                    within: within.exp() - 1.0,
                    within_spread: Spread::maybe(within, sd_of_diff(first_sd, last_sd)),
                    carryover: carry.map(|c| c.0),
                    carryover_spread: carry.and_then(|c| c.1),
                    version: version.clone(),
                    first_sd,
                    last_sd,
                },
            ));
            last_seen.insert(scenario.to_string(), (last, version, last_sd));
        }
        sessions.sort_by_key(|(id, _)| *id);
        out.extend(sessions.into_iter().map(|(_, s)| s));
    }
    out
}

/// Main drills of the session on `date` vs. the last earlier session on the same weekday,
/// with the change's likely range.
fn focus_change(sessions: &[DrillSession], date: &str, day: &str) -> Option<(f64, Option<Spread>)> {
    let this: Vec<&DrillSession> =
        sessions.iter().filter(|s| s.date == date && s.day == day && s.block.as_deref() == Some("main")).collect();
    if this.is_empty() {
        return None;
    }
    let prev_date =
        sessions.iter().filter(|s| s.day == day && s.date.as_str() < date).map(|s| s.date.as_str()).max()?;
    let diffs: Vec<(f64, Option<f64>)> = this
        .iter()
        .filter_map(|s| {
            let p =
                sessions.iter().find(|p| p.date == prev_date && p.scenario == s.scenario && p.version == s.version)?;
            Some((
                Knob::from_id(&s.knob).h(s.last) - Knob::from_id(&s.knob).h(p.last),
                sd_of_diff(s.last_sd, p.last_sd),
            ))
        })
        .collect();
    let m = mean(&diffs.iter().map(|d| d.0).collect::<Vec<_>>())?;
    Some((m.exp() - 1.0, spread_of_mean(&diffs)))
}

/// Program runs for weekday `id` played in the week starting `week_start`.
fn day_runs<'a>(rows: &[&'a Row], week_start: NaiveDate, id: &str) -> Vec<&'a Row> {
    let end = week_start + Duration::days(7);
    rows.iter()
        .filter(|r| r.day.as_deref() == Some(id))
        .filter(|r| {
            let d = r.date;
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
        /// The scenario's settings fingerprint (its version).
        hash: Option<&'a str>,
    }

    /// The stored range of a rating, when a test wants one.
    #[derive(Default)]
    struct Stored {
        sd: Option<f64>,
        low: Option<f64>,
        high: Option<f64>,
    }

    fn save(db: &mut Db, r: &Run) {
        save_with(db, r, &Stored::default());
    }

    fn save_with(db: &mut Db, r: &Run, stored: &Stored) {
        let file = format!("rustyaim/{}/{}/{}", r.scenario, r.at, r.rep);
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("Source".into(), "rustyaim".into());
        if let Some(h) = r.hash {
            fields.insert("Hash".into(), h.into());
        }
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
            rating_sd: stored.sd,
            rating_low: stored.low,
            rating_high: stored.high,
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
                    hash: None,
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
                hash: None,
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
        let p = skill_profile(&db(), None, &TimeSettings::default()).unwrap();
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
        assert_eq!(p.ranked()[0], "dynamic_clicking", "untrained skills come first");
        let ranked = p.ranked();
        let pos = |id: &str| ranked.iter().position(|x| x == id).unwrap();
        assert!(pos("reactive_tracking") < pos("static_clicking"), "one day counts as no change: {ranked:?}");
        assert!(pos("speed_switching") < pos("reactive_tracking"), "untrained before trained: {ranked:?}");
    }

    #[test]
    fn week_shows_progress_against_last_week() {
        let p = progress(&db(), "Core Week", at(2026, 11, 10, 12, 0), &plan(), &TimeSettings::default()).unwrap();
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
        let p = progress(&db(), "Core Week", at(2026, 11, 10, 12, 0), &plan(), &TimeSettings::default()).unwrap();
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
                hash: None,
            };
            save(&mut db, &r);
        }
        let p = progress(&db, "Core Week", at(2026, 11, 12, 20, 0), &plan(), &TimeSettings::default()).unwrap();
        assert_eq!(p.week[1].status, "done", "{:?}", p.week[1]);
        // 30 °/s at the end vs. 25 °/s last Tuesday.
        assert!((p.week[1].focus_change.unwrap() - (31.0 / 25.0 - 1.0)).abs() < 1e-6, "{:?}", p.week[1]);
        assert_eq!(p.week[2].status, "missed", "Wednesday was skipped");
        assert_eq!(p.summary.streak, 0, "Wednesday breaks it, Thursday (today) doesn't count yet");
    }

    #[test]
    fn early_morning_counts_as_the_day_before() {
        let time = TimeSettings::default();
        assert_eq!(time.training_day(at(2026, 11, 10, 3, 59)).to_string(), "2026-11-09");
        assert_eq!(time.training_day(at(2026, 11, 10, 4, 0)).to_string(), "2026-11-10");
    }

    fn flicks(db: &mut Db, date: (u32, u32), base: f64, day: &str, hash: Option<&str>) {
        for rep in 1..=5 {
            let r = Run {
                at: at(2026, date.0, date.1, 18, rep as u32 * 2),
                scenario: "Timed Flicks",
                skill: "static_clicking",
                knob: "lifetime",
                rating: base - 0.02 * f64::from(rep as u32),
                day,
                rep,
                block: "main",
                hash,
            };
            save(db, &r);
        }
    }

    /// Bug: the skill profile compared a scenario's first day with its latest day even
    /// when its settings changed in between, so an old 0.40 s and a new 0.70 s read as
    /// a 43% loss.
    #[test]
    fn skill_change_ignores_runs_of_other_versions() {
        let mut db = Db::open_in_memory().unwrap();
        flicks(&mut db, (11, 2), 0.40, "mon", Some("old"));
        flicks(&mut db, (11, 9), 0.70, "mon", Some("new"));
        flicks(&mut db, (11, 10), 0.60, "tue", Some("new"));
        let p = skill_profile(&db, None, &TimeSettings::default()).unwrap();
        let s = p.skills.iter().find(|s| s.skill == "static_clicking").unwrap();
        // Last three reps: 0.64, 0.62, 0.60 on the 9th and 0.54, 0.52, 0.50 on the 10th.
        let gm = |v: [f64; 3]| (v.iter().map(|x| x.ln()).sum::<f64>() / 3.0).exp();
        let want = gm([0.64, 0.62, 0.60]) / gm([0.54, 0.52, 0.50]) - 1.0;
        assert!((s.change.unwrap() - want).abs() < 1e-6, "{:?} vs {want}", s.change);
    }

    /// Bug: carryover and the focus change compared a drill with its session before the
    /// scenario's settings changed.
    #[test]
    fn carryover_and_focus_change_skip_other_versions() {
        let mut db = Db::open_in_memory().unwrap();
        flicks(&mut db, (11, 2), 0.40, "mon", Some("old"));
        flicks(&mut db, (11, 9), 0.70, "mon", Some("new"));
        let p = progress(&db, "Core Week", at(2026, 11, 10, 12, 0), &plan(), &TimeSettings::default()).unwrap();
        let s = p.sessions.iter().find(|s| s.date == "2026-11-09").unwrap();
        assert_eq!(s.carryover, None, "the earlier session is another version");
        assert_eq!(p.week[0].focus_change, None, "last Monday is another version");
    }

    /// Programs kept their own 4 AM day start while Time used the setting, so the
    /// two could disagree on which day a run belongs to. (The app has no control
    /// for the day start yet, so players couldn't hit this.)
    #[test]
    fn programs_use_the_day_start_setting() {
        let mut db = Db::open_in_memory().unwrap();
        let r = Run {
            at: at(2026, 11, 10, 5, 0),
            scenario: "Timed Flicks",
            skill: "static_clicking",
            knob: "lifetime",
            rating: 0.7,
            day: "tue",
            rep: 1,
            block: "main",
            hash: None,
        };
        save(&mut db, &r);
        let six = TimeSettings { day_start_hour: 6, ..TimeSettings::default() };
        assert_eq!(load(&db, &six).unwrap()[0].date.to_string(), "2026-11-09");
        assert_eq!(load(&db, &TimeSettings::default()).unwrap()[0].date.to_string(), "2026-11-10");
    }

    /// One drill played on a day: a rep per `(rating, sd)`, 18:02, 18:04 and so on.
    fn play(db: &mut Db, drill: (&str, &str, &str), date: (u32, u32), reps: &[(f64, Option<f64>)]) {
        for (i, (rating, sd)) in reps.iter().enumerate() {
            let rep = i as i64 + 1;
            let r = Run {
                at: at(2026, date.0, date.1, 18, rep as u32 * 2),
                scenario: drill.0,
                skill: drill.1,
                knob: drill.2,
                rating: *rating,
                day: "mon",
                rep,
                block: "main",
                hash: None,
            };
            save_with(db, &r, &Stored { sd: *sd, ..Stored::default() });
        }
    }

    const FLICKS: (&str, &str, &str) = ("Timed Flicks", "static_clicking", "lifetime");
    const STRAFE: (&str, &str, &str) = ("Strafe Track", "reactive_tracking", "target_speed");

    /// Five reps of Timed Flicks at one rating, each with the same sd.
    fn steady(db: &mut Db, date: (u32, u32), rating: f64, sd: Option<f64>) {
        play(db, FLICKS, date, &[(rating, sd); 5]);
    }

    /// Two days of Timed Flicks, the second `gain` better (a lifetime shrinks by that factor).
    fn two_days(gain: f64, sd: Option<f64>) -> Db {
        let mut db = Db::open_in_memory().unwrap();
        steady(&mut db, (11, 2), 0.70, sd);
        steady(&mut db, (11, 9), 0.70 / (1.0 + gain), sd);
        db
    }

    fn near(a: Spread, b: Spread) {
        assert!((a.low - b.low).abs() < 1e-9 && (a.high - b.high).abs() < 1e-9 && a.noise == b.noise, "{a:?} {b:?}");
    }

    fn clicking(p: &SkillProfile) -> &SkillStat {
        p.skills.iter().find(|s| s.skill == "static_clicking").unwrap()
    }

    /// Plan A7: a change is flagged as noise when zero is inside its range. Every rating
    /// has sd 0.05, so each day's tail has 0.05 and the change has 0.05·√2.
    #[test]
    fn a_change_inside_its_range_is_noise_and_one_outside_is_not() {
        let sd = 0.05 * 2f64.sqrt();
        for (gain, noise) in [(0.03, true), (0.15, false)] {
            let p = skill_profile(&two_days(gain, Some(0.05)), None, &TimeSettings::default()).unwrap();
            let s = clicking(&p);
            let m = (1.0f64 + gain).ln();
            let sp = s.spread.unwrap();
            assert!((s.change.unwrap() - gain).abs() < 1e-9);
            assert!((sp.low - ((m - sd).exp() - 1.0)).abs() < 1e-6, "{sp:?}");
            assert!((sp.high - ((m + sd).exp() - 1.0)).abs() < 1e-6, "{sp:?}");
            assert_eq!(sp.noise, noise, "{gain}: {sp:?}");
            assert!((sp.sd() - sd).abs() < 1e-9);
            assert!((s.latest_sd.unwrap() - 0.05).abs() < 1e-9);
            // One skill in its group and in the profile: they share its range.
            let group = p.groups.iter().find(|g| g.group == "clicking").unwrap();
            near(group.spread.unwrap(), sp);
            near(p.overall_spread.unwrap(), sp);
        }
    }

    /// Plan A7: runs saved without a range give no range, and nothing is flagged as noise.
    #[test]
    fn runs_without_a_stored_range_have_no_spread() {
        let p = progress(&db(), "Core Week", at(2026, 11, 10, 12, 0), &plan(), &TimeSettings::default()).unwrap();
        assert!(clicking(&p.profile).change.is_some());
        assert!(p.profile.skills.iter().all(|s| s.spread.is_none() && s.latest_sd.is_none()));
        assert!(p.profile.groups.iter().all(|g| g.spread.is_none()));
        assert_eq!(p.profile.overall_spread, None);
        assert!(p.week.iter().all(|d| d.focus_spread.is_none()));
        assert!(p.sessions.iter().all(|s| s.within_spread.is_none() && s.carryover_spread.is_none()));
        assert_eq!(p.summary.best_spread, None);
        assert!(p.summary.best.is_some(), "the old rule still names a gain");
        assert!(p.carry.iter().all(|c| c.spread.is_none()));
    }

    /// Plan A7: a run with a stored range but no sd gets its sd from the range ends, half
    /// their distance on the knob's log scale. A usable stored sd wins.
    #[test]
    fn a_missing_sd_comes_from_the_stored_range() {
        let mut db = Db::open_in_memory().unwrap();
        let run = |rep: i64| Run {
            at: at(2026, 11, 2, 18, rep as u32 * 2),
            scenario: "Timed Flicks",
            skill: "static_clicking",
            knob: "lifetime",
            rating: 0.7,
            day: "mon",
            rep,
            block: "main",
            hash: None,
        };
        save_with(&mut db, &run(1), &Stored { sd: None, low: Some(0.60), high: Some(0.66) });
        save_with(&mut db, &run(2), &Stored { sd: Some(0.05), low: Some(0.60), high: Some(0.66) });
        save_with(&mut db, &run(3), &Stored { sd: Some(0.0), low: Some(0.60), high: Some(0.66) });
        save_with(&mut db, &run(4), &Stored { sd: None, low: Some(0.60), high: None });
        let rows = load(&db, &TimeSettings::default()).unwrap();
        let from_range = (0.66f64 / 0.60).ln() / 2.0;
        assert!((rows[0].sd.unwrap() - from_range).abs() < 1e-9, "{:?}", rows[0].sd);
        assert_eq!(rows[1].sd, Some(0.05));
        assert!((rows[2].sd.unwrap() - from_range).abs() < 1e-9, "a zero sd isn't usable");
        assert_eq!(rows[3].sd, None, "one end alone says nothing");
    }

    /// A drill played on the days `days` after Nov 2, each day `gain` better than the one before.
    fn drill_days(db: &mut Db, drill: (&str, &str, &str), start: f64, gain: f64, days: std::ops::Range<u32>) {
        let better = |i: u32| {
            // A lifetime shrinks as it improves; a tracking speed grows.
            let f = (1.0 + gain).powi(i as i32);
            if drill.2 == "lifetime" { start / f } else { start * f }
        };
        for i in days {
            play(db, drill, (11, 2 + i), &[(better(i), None); 5]);
        }
    }

    fn lagging(db: &Db) -> (Option<String>, usize) {
        let p = progress(db, "Core Week", at(2026, 11, 20, 12, 0), &plan(), &TimeSettings::default()).unwrap();
        (p.summary.lagging, p.summary.lagging_needs)
    }

    /// Plan A7: a least improved skill is named only once two skills have three sessions each.
    #[test]
    fn nothing_is_least_improved_before_three_sessions() {
        let mut db = Db::open_in_memory().unwrap();
        drill_days(&mut db, FLICKS, 0.70, 0.10, 0..2);
        drill_days(&mut db, STRAFE, 20.0, 0.02, 0..2);
        assert_eq!(lagging(&db), (None, 2), "two sessions each: one more of each");
        drill_days(&mut db, FLICKS, 0.70, 0.10, 2..3);
        drill_days(&mut db, STRAFE, 20.0, 0.02, 2..3);
        // Reactive tracking gained 4% and static clicking 21%; the seven untrained skills aren't named.
        assert_eq!(lagging(&db), (Some("reactive_tracking".to_string()), 0));
        // One skill alone has nothing to be compared with.
        let mut alone = Db::open_in_memory().unwrap();
        drill_days(&mut alone, FLICKS, 0.70, 0.10, 0..3);
        assert_eq!(lagging(&alone), (None, 3));
    }

    fn stat(skill: &str, runs: usize, sessions: usize, change: Option<f64>, last: &str) -> SkillStat {
        SkillStat {
            skill: skill.into(),
            group: "clicking".into(),
            change,
            spread: None,
            scenarios: 1,
            runs,
            sessions,
            last_trained: (runs > 0).then(|| last.to_string()),
            latest_sd: None,
        }
    }

    /// Plan A7: Saturday's order is untrained skills, then skills with under three
    /// sessions (fewest first), then the rest by change.
    #[test]
    fn ranked_puts_untrained_then_few_sessions_then_smallest_change() {
        let profile = SkillProfile {
            skills: vec![
                stat("big", 40, 4, Some(0.30), "2026-11-05"),
                stat("two", 10, 2, Some(-0.20), "2026-11-05"),
                stat("small", 30, 3, Some(0.02), "2026-11-05"),
                stat("none", 0, 0, None, ""),
                stat("one", 5, 1, None, "2026-11-05"),
                stat("tie_old", 30, 3, Some(0.02), "2026-11-01"),
            ],
            groups: vec![],
            overall: None,
            overall_spread: None,
        };
        assert_eq!(profile.ranked(), ["none", "one", "two", "tie_old", "small", "big"]);
        assert_eq!(profile.least_improved().unwrap().skill, "tie_old");
    }

    /// Plan A7: within-session and carry-over ranges come from the reps' sds, and the
    /// day's focus range from the last reps of the two Mondays.
    #[test]
    fn sessions_and_focus_carry_ranges_from_the_reps() {
        let mut db = Db::open_in_memory().unwrap();
        let mut first = [(0.70, Some(0.05)); 5];
        first[4].1 = Some(0.04);
        play(&mut db, FLICKS, (11, 2), &first);
        let mut second = [(0.66, Some(0.05)); 5];
        second[0] = (0.68, Some(0.03));
        second[4] = (0.60, Some(0.02));
        play(&mut db, FLICKS, (11, 9), &second);
        let p = progress(&db, "Core Week", at(2026, 11, 10, 12, 0), &plan(), &TimeSettings::default()).unwrap();
        let s = p.sessions.iter().find(|s| s.date == "2026-11-09").unwrap();
        let sd = |a: f64, b: f64| (a * a + b * b).sqrt();
        near(s.within_spread.unwrap(), Spread::of((0.68f64 / 0.60).ln(), sd(0.03, 0.02)));
        near(s.carryover_spread.unwrap(), Spread::of((0.70f64 / 0.68).ln(), sd(0.03, 0.04)));
        assert_eq!((s.first_sd, s.last_sd), (Some(0.03), Some(0.02)));
        near(p.week[0].focus_spread.unwrap(), Spread::of((0.70f64 / 0.60).ln(), sd(0.02, 0.04)));
        assert!(!p.week[0].focus_spread.unwrap().noise);
        assert_eq!(p.sessions.iter().find(|s| s.date == "2026-11-02").unwrap().carryover_spread, None);
    }

    /// Plan A7: a drill's carry-over is named only after three sessions, and is the mean
    /// of its carry-overs (log units) with a range.
    #[test]
    fn carry_by_drill_needs_three_sessions() {
        let mut db = Db::open_in_memory().unwrap();
        steady(&mut db, (11, 2), 0.70, Some(0.05));
        steady(&mut db, (11, 9), 0.68, Some(0.05));
        let now = at(2026, 11, 17, 12, 0);
        let p = progress(&db, "Core Week", now, &plan(), &TimeSettings::default()).unwrap();
        let c = &p.carry[0];
        assert_eq!((c.scenario.as_str(), c.sessions, c.needed), ("Timed Flicks", 2, 1));
        assert_eq!((c.kept, c.spread), (None, None));
        steady(&mut db, (11, 16), 0.66, Some(0.05));
        let p = progress(&db, "Core Week", now, &plan(), &TimeSettings::default()).unwrap();
        let c = &p.carry[0];
        assert_eq!((c.sessions, c.needed), (3, 0));
        // 0.70 → 0.68 and 0.68 → 0.66: the mean of the two gaps in log units.
        let want = (0.70f64 / 0.66).sqrt() - 1.0;
        assert!((c.kept.unwrap() - want).abs() < 1e-9, "{:?}", c.kept);
        // Two gaps, each with sd 0.05·√2: their mean has sd 0.05.
        assert!((c.spread.unwrap().sd() - 0.05).abs() < 1e-9, "{:?}", c.spread);
        assert_eq!(p.carry.len(), 1);
    }

    /// Plan A7: "Biggest gain" is named only when its range excludes zero.
    #[test]
    fn biggest_gain_is_not_named_inside_its_noise() {
        let now = at(2026, 11, 10, 12, 0);
        let p = progress(&two_days(0.03, Some(0.05)), "Core Week", now, &plan(), &TimeSettings::default()).unwrap();
        assert_eq!((p.summary.best, p.summary.best_spread), (None, None), "3% against a ±7% range");
        let p = progress(&two_days(0.15, Some(0.05)), "Core Week", now, &plan(), &TimeSettings::default()).unwrap();
        let (skill, gain) = p.summary.best.clone().unwrap();
        assert_eq!(skill, "static_clicking");
        assert!((gain - 0.15).abs() < 1e-9);
        let sp = p.summary.best_spread.unwrap();
        assert!(!sp.noise && sp.low > 0.0, "{sp:?}");
    }
}
