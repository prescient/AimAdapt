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
    /// Build the day from the skills the player has trained least and improved least instead.
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
    drills(skill)[0]
}

/// The drills for each sub-skill, rotated week by week on weak-spots days.
pub fn drills(skill: Skill) -> &'static [&'static str] {
    match skill {
        Skill::StaticClicking | Skill::None => &["builtin:timed-flicks", "builtin:timed-wide-flicks"],
        Skill::DynamicClicking => &["builtin:timed-strafe-clicks", "builtin:hop-clicks"],
        Skill::LinearClicking => &["builtin:linear-clicks", "builtin:angled-glides"],
        Skill::PreciseTracking => &["builtin:precise-track", "builtin:depth-drift"],
        Skill::ReactiveTracking => &["builtin:strafe-track", "builtin:hop-strafe-track"],
        Skill::ControlTracking => &["builtin:control-track", "builtin:arc-track"],
        Skill::SpeedSwitching => &["builtin:speed-switch", "builtin:wide-speed-switch"],
        Skill::EvasiveSwitching => &["builtin:switch-track", "builtin:hop-switch"],
        Skill::StabilitySwitching => &["builtin:stability-switch", "builtin:depth-stability-switch"],
    }
}

/// A weak-spots workout from skills ordered most in need first (least trained, then
/// least improved): a standard warm-up, the first two skills ×5, the next two ×4, and
/// the last, the skill least in need, ×3 so it doesn't fade. Each
/// skill's drill rotates with the week like the other days' variants.
pub fn weak_spots(name: &str, ranked: &[Skill]) -> Workout {
    let item = |s: Skill, runs: u32, block: Block| {
        let d = drills(s);
        Item {
            scenario: d[0].into(),
            runs,
            mode: Mode::Adaptive,
            block,
            variants: d[1..].iter().map(|v| v.to_string()).collect(),
        }
    };
    let mut items = vec![Item {
        scenario: "builtin:static-grid".into(),
        runs: 2,
        mode: Mode::Standard,
        block: Block::Warmup,
        variants: Vec::new(),
    }];
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
        author: "AimAdapt".into(),
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

    /// Built-in programs are listed by hand; see the scenarios' version of this test.
    #[test]
    fn every_built_in_program_file_is_listed_once() {
        crate::store::check_builtins("programs", BUILTIN);
    }
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
    fn weak_spots_drill_the_first_skills_most() {
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

    /// Variants rotate week by week, and each one trains the same sub-skill as the
    /// scenario it stands in for.
    #[test]
    fn variants_alternate_by_week_and_keep_the_skill() {
        let all = library::builtins();
        let skill = |id: &str| {
            let e = all.iter().find(|e| e.id == id).unwrap_or_else(|| panic!("{id} isn't a built-in"));
            e.scenario.as_ref().unwrap().skill
        };
        let mut checked = 0;
        for w in crate::workout::builtins() {
            let w = w.workout.unwrap();
            for it in &w.items {
                for v in &it.variants {
                    assert_eq!(skill(v), skill(&it.scenario), "{}: {v} stands in for {}", w.name, it.scenario);
                    checked += 1;
                }
                if !it.variants.is_empty() {
                    assert_eq!(it.for_week(44), it.scenario, "even weeks play the main drill");
                    assert_eq!(it.for_week(45), it.variants[0], "odd weeks play the variant");
                }
            }
            assert_eq!(w.for_week(45).total_runs(), w.total_runs(), "same reps either week");
        }
        assert!(checked >= 15, "{checked} variants");
        for s in Skill::ALL {
            for d in drills(s) {
                assert_eq!(skill(d), s, "{d} is listed for {s:?}");
            }
        }
        let w = weak_spots("Weak spots", &Skill::ALL);
        assert_ne!(w.for_week(1).items[1].scenario, w.for_week(2).items[1].scenario, "weak spots rotate too");
    }

    /// From the FAR comparison: one scenario per sub-skill risks fitting one exact
    /// setup, so every sub-skill has at least two built-ins to rotate between.
    #[test]
    fn every_skill_has_at_least_two_scenarios() {
        let all = library::builtins();
        for skill in Skill::ALL {
            let n = all.iter().filter(|e| e.scenario.as_ref().is_ok_and(|s| s.skill == skill)).count();
            assert!(n >= 2, "{skill:?} has {n} scenario(s)");
        }
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
