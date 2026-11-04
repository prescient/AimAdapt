//! What the engine allows: one entry per scenario field with its label, help text,
//! limits and choices. The app's scenario editor builds its form from this list and
//! SCENARIOS.md documents the same fields; a test keeps it in step with
//! [`crate::scenario::Scenario`].

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    /// Dotted path into the scenario, like `targets.spawn.yaw`.
    pub path: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    #[serde(flatten)]
    pub kind: Kind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Text,
    LongText,
    /// A list of short words.
    Tags,
    Color,
    Number {
        min: f32,
        max: f32,
        step: f32,
        unit: &'static str,
    },
    Integer {
        min: u32,
        max: u32,
    },
    /// `[min, max]`; the engine picks a value in between each time.
    Range {
        min: f32,
        max: f32,
        step: f32,
        unit: &'static str,
    },
    /// Width, height, depth.
    Size3 {
        min: f32,
        max: f32,
        step: f32,
        unit: &'static str,
    },
    Choice {
        options: &'static [Choice],
    },
    /// The adaptive knobs: a list of `{ knob, easy, hard }` rows.
    Knobs {
        options: &'static [Choice],
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
    pub help: &'static str,
}

const fn num(min: f32, max: f32, step: f32, unit: &'static str) -> Kind {
    Kind::Number { min, max, step, unit }
}

const fn range(min: f32, max: f32, step: f32, unit: &'static str) -> Kind {
    Kind::Range { min, max, step, unit }
}

const fn f(path: &'static str, group: &'static str, label: &'static str, help: &'static str, kind: Kind) -> Field {
    Field { path, group, label, help, kind }
}

const CATEGORY: &[Choice] = &[
    Choice { value: "clicking", label: "Clicking", help: "Flicks and precise clicks on targets." },
    Choice { value: "tracking", label: "Tracking", help: "Keeping the crosshair on moving targets." },
    Choice { value: "switching", label: "Switching", help: "Moving quickly between several targets." },
];

const WEAPON_KIND: &[Choice] = &[
    Choice { value: "hitscan", label: "Hitscan", help: "Instant shots along the crosshair." },
    Choice { value: "beam", label: "Beam", help: "Continuous damage while on target, sampled 60 times a second." },
];

const TRIGGER: &[Choice] = &[
    Choice { value: "click", label: "Click", help: "One shot per click." },
    Choice { value: "hold", label: "Hold", help: "Fires while the button is held." },
    Choice { value: "always", label: "Always", help: "Always firing; only aim matters." },
];

const SHAPE: &[Choice] = &[
    Choice { value: "sphere", label: "Sphere", help: "A ball." },
    Choice { value: "capsule", label: "Capsule", help: "An upright pill, roughly a body." },
];

const MOVEMENT: &[Choice] = &[
    Choice { value: "static", label: "Static", help: "Targets stay put." },
    Choice { value: "strafe", label: "Strafe", help: "Side to side, reversing at random intervals." },
    Choice { value: "random", label: "Random", help: "A new random direction at each change." },
];

const KNOBS: &[Choice] = &[
    Choice {
        value: "lifetime",
        label: "Target lifetime (s)",
        help: "Seconds before a target times out. Shorter is harder.",
    },
    Choice { value: "radius", label: "Target radius (m)", help: "Smaller is harder." },
    Choice { value: "speed", label: "Target speed (m/s)", help: "Top movement speed; the bottom keeps its ratio." },
    Choice {
        value: "change_every",
        label: "Direction change (s)",
        help: "Longest time between direction changes. Shorter is harder.",
    },
    Choice { value: "spread", label: "Spawn spread (×)", help: "Multiplier on the spawn area's width and height." },
    Choice { value: "health", label: "Target health", help: "More health means longer to destroy." },
    Choice { value: "respawn_delay", label: "Respawn delay (s)", help: "Wait before the next target." },
];

pub const FIELDS: &[Field] = &[
    f("name", "About", "Name", "Shown in lists and used to group your history.", Kind::Text),
    f("description", "About", "Description", "What the scenario trains and how to play it.", Kind::LongText),
    f("author", "About", "Author", "", Kind::Text),
    f("category", "About", "Category", "The skill it trains.", Kind::Choice { options: CATEGORY }),
    f("tags", "About", "Tags", "Short words for searching, like micro or wide.", Kind::Tags),
    f("timing.duration_s", "Timing", "Duration", "Length of the run.", num(5.0, 3600.0, 5.0, "s")),
    f(
        "timing.countdown_s",
        "Timing",
        "Countdown",
        "Targets show but can't be hit until it ends.",
        num(0.0, 30.0, 1.0, "s"),
    ),
    f(
        "timing.kill_limit",
        "Timing",
        "Kill limit",
        "End early after this many kills. 0 for no limit.",
        Kind::Integer { min: 0, max: 10_000 },
    ),
    f("weapon.kind", "Weapon", "Type", "", Kind::Choice { options: WEAPON_KIND }),
    f("weapon.trigger", "Weapon", "Trigger", "", Kind::Choice { options: TRIGGER }),
    f(
        "weapon.fire_rate",
        "Weapon",
        "Fire rate",
        "Hitscan: most shots per second. 0 means no cap for click trigger.",
        num(0.0, 100.0, 0.5, "shots/s"),
    ),
    f("weapon.damage", "Weapon", "Damage per hit", "Hitscan only.", num(0.1, 10_000.0, 1.0, "")),
    f(
        "weapon.damage_per_s",
        "Weapon",
        "Beam damage",
        "Beam only: damage per second on target.",
        num(0.1, 10_000.0, 5.0, "/s"),
    ),
    f("targets.count", "Targets", "Targets at once", "", Kind::Integer { min: 1, max: 50 }),
    f("targets.shape", "Targets", "Shape", "", Kind::Choice { options: SHAPE }),
    f(
        "targets.radius",
        "Targets",
        "Radius",
        "Size of the target. 0.3 m at 10 m is about 3.4° across.",
        num(0.01, 10.0, 0.01, "m"),
    ),
    f(
        "targets.height",
        "Targets",
        "Capsule height",
        "Capsule only, including the rounded ends.",
        num(0.1, 10.0, 0.05, "m"),
    ),
    f(
        "targets.health",
        "Targets",
        "Health",
        "Damage needed to destroy a target. Equal to weapon damage means one shot.",
        num(0.1, 100_000.0, 10.0, ""),
    ),
    f(
        "targets.regen_per_s",
        "Targets",
        "Regeneration",
        "Beam only: health restored per second while off target.",
        num(0.0, 10_000.0, 5.0, "/s"),
    ),
    f("targets.color", "Targets", "Color", "", Kind::Color),
    f(
        "targets.respawn_delay_s",
        "Targets",
        "Respawn delay",
        "Wait before a replacement appears.",
        num(0.0, 10.0, 0.05, "s"),
    ),
    f(
        "targets.lifetime_s",
        "Targets",
        "Lifetime",
        "A target disappears as a timeout after this long. 0 means forever.",
        num(0.0, 60.0, 0.1, "s"),
    ),
    f(
        "targets.shrink_to",
        "Targets",
        "Shrink to",
        "Size at the end of its lifetime, as a share of the radius. 1 means no shrinking.",
        num(0.05, 1.0, 0.05, "×"),
    ),
    f("targets.spawn.distance", "Spawn area", "Distance", "From the player.", range(1.0, 200.0, 0.5, "m")),
    f(
        "targets.spawn.yaw",
        "Spawn area",
        "Left / right",
        "Degrees from straight ahead; negative is left. Moving targets stay inside this too.",
        range(-180.0, 180.0, 1.0, "°"),
    ),
    f(
        "targets.spawn.pitch",
        "Spawn area",
        "Down / up",
        "Degrees from eye level; negative is down.",
        range(-85.0, 85.0, 1.0, "°"),
    ),
    f(
        "targets.spawn.min_separation",
        "Spawn area",
        "Min spacing",
        "Smallest angle between a new target and the live ones. Large values force wide flicks.",
        num(0.0, 179.0, 0.5, "°"),
    ),
    f(
        "targets.spawn.max_from_last",
        "Spawn area",
        "Max from last",
        "Largest angle from the last destroyed target. Small values make micro-flicks. 0 for no limit.",
        num(0.0, 360.0, 0.5, "°"),
    ),
    f("targets.movement.kind", "Movement", "Movement", "", Kind::Choice { options: MOVEMENT }),
    f("targets.movement.speed", "Movement", "Speed", "", range(0.0, 100.0, 0.25, "m/s")),
    f(
        "targets.movement.change_every_s",
        "Movement",
        "Direction change",
        "Time between changes of direction.",
        range(0.05, 30.0, 0.05, "s"),
    ),
    f(
        "targets.movement.vertical",
        "Movement",
        "Vertical share",
        "Random movement only: 0 keeps it level, 1 allows any direction.",
        num(0.0, 1.0, 0.05, ""),
    ),
    f("scoring.per_kill", "Scoring", "Per kill", "", num(-10_000.0, 10_000.0, 1.0, "pts")),
    f(
        "scoring.per_hit",
        "Scoring",
        "Per hit",
        "For beams a hit is one 1/60 s tick on target.",
        num(-10_000.0, 10_000.0, 1.0, "pts"),
    ),
    f("scoring.per_miss", "Scoring", "Per miss", "Usually 0 or negative.", num(-10_000.0, 10_000.0, 1.0, "pts")),
    f(
        "scoring.per_damage",
        "Scoring",
        "Per damage",
        "Points per point of damage dealt.",
        num(-100.0, 100.0, 0.1, "pts"),
    ),
    f("scoring.per_timeout", "Scoring", "Per timeout", "Usually 0 or negative.", num(-10_000.0, 10_000.0, 1.0, "pts")),
    f(
        "adaptive.goal",
        "Adaptive",
        "Success goal",
        "Share of targets (or time on target) adaptive mode aims for. 0.85 means it settles where you hit 85%.",
        num(0.5, 0.98, 0.01, ""),
    ),
    f(
        "adaptive.start_level",
        "Adaptive",
        "First-run level",
        "Where a first adaptive run starts, 0–100. Later runs start from your last level.",
        num(0.0, 100.0, 1.0, ""),
    ),
    f(
        "adaptive.window_s",
        "Adaptive",
        "Tracking window",
        "Beam scenarios: seconds of tracking judged as one outcome.",
        num(0.2, 5.0, 0.1, "s"),
    ),
    f(
        "adaptive.knobs",
        "Adaptive",
        "What gets harder",
        "Each setting runs from its easy value at level 0 to its hard value at level 100. Leave empty for a default that suits the scenario.",
        Kind::Knobs { options: KNOBS },
    ),
    f(
        "arena.size",
        "Arena",
        "Room size",
        "Width, height and depth.",
        Kind::Size3 { min: 2.0, max: 1000.0, step: 1.0, unit: "m" },
    ),
    f("arena.eye_height", "Arena", "Eye height", "Above the floor.", num(0.1, 100.0, 0.1, "m")),
    f("arena.wall_color", "Arena", "Wall color", "", Kind::Color),
    f("arena.floor_color", "Arena", "Floor color", "", Kind::Color),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Scenario;

    fn leaves(prefix: &str, v: &toml::Value, out: &mut Vec<String>) {
        match v {
            toml::Value::Table(t) => {
                for (k, v) in t {
                    let p = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                    leaves(&p, v, out);
                }
            }
            _ => out.push(prefix.to_string()),
        }
    }

    #[test]
    fn every_field_is_described_once() {
        let value: toml::Value = toml::from_str(&Scenario::default().to_toml()).unwrap();
        let mut paths = Vec::new();
        leaves("", &value, &mut paths);
        paths.retain(|p| p != "schema");
        let mut described: Vec<String> = FIELDS.iter().map(|f| f.path.to_string()).collect();
        // Empty lists aren't written out, so add the ones we know about.
        for list in ["tags", "adaptive.knobs"] {
            if !paths.contains(&list.to_string()) {
                paths.push(list.into());
            }
        }
        paths.sort();
        described.sort();
        assert_eq!(paths, described);
    }

    #[test]
    fn scenarios_md_documents_every_field() {
        let doc = include_str!("../../../SCENARIOS.md");
        for f in FIELDS {
            assert!(doc.contains(&format!("`{}`", f.path)), "SCENARIOS.md is missing `{}`", f.path);
        }
    }

    #[test]
    fn choices_match_the_file_format() {
        for field in FIELDS {
            if let Kind::Choice { options } = &field.kind {
                for o in options.iter() {
                    let mut value: toml::Value = toml::from_str(&Scenario::default().to_toml()).unwrap();
                    let mut slot = &mut value;
                    for part in field.path.split('.') {
                        slot = slot.get_mut(part).unwrap();
                    }
                    *slot = toml::Value::String(o.value.into());
                    let text = toml::to_string(&value).unwrap();
                    assert!(toml::from_str::<Scenario>(&text).is_ok(), "{} = {}", field.path, o.value);
                }
            }
        }
    }

    #[test]
    fn defaults_are_within_limits() {
        let value: toml::Value = toml::from_str(&Scenario::default().to_toml()).unwrap();
        for field in FIELDS {
            let mut v = &value;
            let mut found = true;
            for part in field.path.split('.') {
                match v.get(part) {
                    Some(next) => v = next,
                    None => found = false,
                }
            }
            if !found {
                continue;
            }
            let as_f = |v: &toml::Value| v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32));
            match field.kind {
                Kind::Number { min, max, .. } => {
                    let x = as_f(v).unwrap();
                    assert!(x >= min && x <= max, "{} = {x}", field.path);
                }
                Kind::Range { min, max, .. } | Kind::Size3 { min, max, .. } => {
                    for x in v.as_array().unwrap().iter().map(|x| as_f(x).unwrap()) {
                        assert!(x >= min && x <= max, "{} has {x}", field.path);
                    }
                }
                _ => {}
            }
        }
    }
}
