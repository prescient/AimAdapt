//! Programs: a weekly plan that maps each weekday to a workout. A day can instead
//! be "weak spots", built each time from the player's skill profile.
//!
//! ```toml
//! name = "Core Week"
//!
//! [[days]]
//! day = "mon"
//! focus = "Static clicking"
//! workout = "builtin:core-mon"
//!
//! [[days]]
//! day = "sat"
//! focus = "Weak spots"
//! weak_spots = true
//! ```
//!
//! Days that aren't listed are rest days.

use serde::{Deserialize, Serialize};

use crate::scenario::Skill;
use crate::workout::{Block, Item, Mode, Workout};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub const ALL: [Weekday; 7] =
        [Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri, Weekday::Sat, Weekday::Sun];

    pub fn id(self) -> &'static str {
        match self {
            Weekday::Mon => "mon",
            Weekday::Tue => "tue",
            Weekday::Wed => "wed",
            Weekday::Thu => "thu",
            Weekday::Fri => "fri",
            Weekday::Sat => "sat",
            Weekday::Sun => "sun",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Weekday::Mon => "Monday",
            Weekday::Tue => "Tuesday",
            Weekday::Wed => "Wednesday",
            Weekday::Thu => "Thursday",
            Weekday::Fri => "Friday",
            Weekday::Sat => "Saturday",
            Weekday::Sun => "Sunday",
        }
    }

    pub fn from_id(id: &str) -> Option<Weekday> {
        Weekday::ALL.into_iter().find(|d| d.id() == id)
    }

    /// 0 = Monday.
    pub fn index(self) -> usize {
        Weekday::ALL.iter().position(|d| *d == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Weekday {
        Weekday::ALL[i % 7]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Day {
    pub day: Weekday,
    /// What the day trains, shown in the app.
    pub focus: String,
    /// Workout id, unless this is a weak-spots day.
    #[serde(default)]
    pub workout: Option<String>,
    /// Build the day from the player's weakest skills instead.
    #[serde(default)]
    pub weak_spots: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Program {
    pub name: String,
    pub description: String,
    pub days: Vec<Day>,
}

impl Default for Program {
    fn default() -> Self {
        Program { name: "Untitled program".into(), description: String::new(), days: Vec::new() }
    }
}

impl Program {
    pub fn from_toml(text: &str) -> Result<Program, String> {
        let p: Program = toml::from_str(text).map_err(|e| e.to_string())?;
        let problems = p.problems();
        if problems.is_empty() { Ok(p) } else { Err(problems.join("; ")) }
    }

    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.name.trim().is_empty() {
            p.push("name is empty".into());
        }
        for (i, d) in self.days.iter().enumerate() {
            if self.days[..i].iter().any(|e| e.day == d.day) {
                p.push(format!("{} is listed twice", d.day.name()));
            }
            if d.workout.is_some() == d.weak_spots {
                p.push(format!("{}: give either a workout or weak_spots = true", d.day.name()));
            }
        }
        p
    }

    pub fn day(&self, day: Weekday) -> Option<&Day> {
        self.days.iter().find(|d| d.day == day)
    }
}

/// The scenario that drills each skill on a weak-spots day.
pub fn drill(skill: Skill) -> &'static str {
    match skill {
        Skill::StaticClicking | Skill::None => "builtin:timed-flicks",
        Skill::DynamicClicking => "builtin:timed-strafe-clicks",
        Skill::LinearClicking => "builtin:linear-clicks",
        Skill::PreciseTracking => "builtin:precise-track",
        Skill::ReactiveTracking => "builtin:strafe-track",
        Skill::ControlTracking => "builtin:control-track",
        Skill::SpeedSwitching => "builtin:speed-switch",
        Skill::EvasiveSwitching => "builtin:switch-track",
        Skill::StabilitySwitching => "builtin:stability-switch",
    }
}

/// A weak-spots workout from skills ordered weakest first: a standard warm-up, the
/// two weakest ×5, the next two ×4, and the strongest ×3 so it doesn't fade.
pub fn weak_spots(name: &str, ranked: &[Skill]) -> Workout {
    let item =
        |s: Skill, runs: u32, block: Block| Item { scenario: drill(s).into(), runs, mode: Mode::Adaptive, block };
    let mut items =
        vec![Item { scenario: "builtin:static-grid".into(), runs: 2, mode: Mode::Standard, block: Block::Warmup }];
    let n = ranked.len();
    for (i, s) in ranked.iter().take(4).enumerate() {
        items.push(if i < 2 { item(*s, 5, Block::Main) } else { item(*s, 4, Block::Secondary) });
    }
    if n > 4 {
        items.push(item(ranked[n - 1], 3, Block::Maintenance));
    }
    Workout {
        name: name.into(),
        description: format!(
            "Built from your skill profile: {}.",
            ranked.iter().take(4).map(|s| s.label().to_lowercase()).collect::<Vec<_>>().join(", ")
        ),
        author: "RustyAim".into(),
        items,
    }
}

const BUILTIN: &[(&str, &str)] = &[("core-week", include_str!("../programs/core-week.toml"))];

pub fn builtins() -> Vec<(String, Result<Program, String>)> {
    BUILTIN.iter().map(|(n, t)| (format!("builtin:{n}"), Program::from_toml(t))).collect()
}

pub fn resolve(id: &str) -> Result<Program, String> {
    let name = id.strip_prefix("builtin:").ok_or(format!("unknown program {id}"))?;
    let (_, text) = BUILTIN.iter().find(|(n, _)| *n == name).ok_or(format!("no built-in program {name}"))?;
    Program::from_toml(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library;

    #[test]
    fn core_week_is_valid_and_about_thirty_minutes_a_day() {
        let p = resolve("builtin:core-week").unwrap();
        let scenarios: Vec<(String, f32)> = library::builtins()
            .into_iter()
            .map(|e| {
                let s = e.scenario.unwrap();
                (e.id, s.timing.duration_s + s.timing.countdown_s)
            })
            .collect();
        assert!(p.day(Weekday::Sun).is_none(), "Sunday is rest");
        for d in &p.days {
            let w = match &d.workout {
                Some(id) => crate::workout::resolve(id, std::path::Path::new("")).unwrap(),
                None => weak_spots("x", &Skill::ALL),
            };
            assert!(w.items.len() <= 6, "{}: few exercises", d.day.name());
            assert!(
                w.items.iter().filter(|i| i.block != Block::Warmup).all(|i| i.runs >= 3),
                "{}: 3+ reps",
                d.day.name()
            );
            // About 75 s per run including the results screen.
            let secs: f32 = w
                .items
                .iter()
                .map(|i| {
                    let d = scenarios.iter().find(|s| s.0 == i.scenario).unwrap_or_else(|| panic!("{}", i.scenario)).1;
                    (d + 12.0) * i.runs as f32
                })
                .sum();
            assert!((25.0 * 60.0..=33.0 * 60.0).contains(&secs), "{}: {} min", d.day.name(), secs / 60.0);
        }
    }

    #[test]
    fn weak_spots_drill_the_weakest_skills_most() {
        let ranked = [
            Skill::LinearClicking,
            Skill::PreciseTracking,
            Skill::SpeedSwitching,
            Skill::DynamicClicking,
            Skill::StaticClicking,
        ];
        let w = weak_spots("Weak spots", &ranked);
        assert_eq!(w.items[0].mode, Mode::Standard);
        assert_eq!((w.items[1].scenario.as_str(), w.items[1].runs), ("builtin:linear-clicks", 5));
        assert_eq!((w.items[3].scenario.as_str(), w.items[3].runs), ("builtin:speed-switch", 4));
        assert_eq!((w.items[5].scenario.as_str(), w.items[5].runs), ("builtin:timed-flicks", 3));
        assert_eq!(w.total_runs(), 23);
        assert!(w.problems().is_empty());
    }

    #[test]
    fn every_skill_has_a_real_drill() {
        let ids: Vec<String> = library::builtins().into_iter().map(|e| e.id).collect();
        for s in Skill::ALL {
            let d = drill(s);
            assert!(ids.iter().any(|i| i == d), "{d}");
            let scen = library::resolve(d, std::path::Path::new("")).unwrap();
            assert_eq!(scen.skill, s, "{d} is tagged {:?}", scen.skill);
        }
    }

    #[test]
    fn days_must_be_unique_and_complete() {
        let bad = Program {
            name: "x".into(),
            description: String::new(),
            days: vec![
                Day { day: Weekday::Mon, focus: "a".into(), workout: None, weak_spots: false },
                Day { day: Weekday::Mon, focus: "b".into(), workout: Some("builtin:x".into()), weak_spots: false },
            ],
        };
        let p = bad.problems().join(" | ");
        assert!(p.contains("twice") && p.contains("either a workout"), "{p}");
    }
}
