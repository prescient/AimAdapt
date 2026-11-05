//! The in-app user guide (`ui/guide.md`, shown on the Help page) has to keep up
//! with the app. These checks fail when something users can see is added without
//! a word in the guide: a page, a built-in scenario or workout, a rating, a key in
//! the trainer, or a new version.

const GUIDE: &str = include_str!("../ui/guide.md");
const APP_JS: &str = include_str!("../ui/app.js");
const INDEX: &str = include_str!("../ui/index.html");
const GAME: &str = include_str!("../../aimadapt-engine/src/game.rs");

/// Section ids, from `## Title {#id}` headings.
fn sections() -> Vec<&'static str> {
    GUIDE.lines().filter(|l| l.starts_with("## ")).filter_map(|l| l.split("{#").nth(1)?.strip_suffix('}')).collect()
}

#[test]
fn the_guide_is_for_this_version() {
    let line = format!("This guide is for AimAdapt {}.", env!("CARGO_PKG_VERSION"));
    assert!(
        GUIDE.contains(&line),
        "ui/guide.md should say \"{line}\": update the guide (and What's new) for this release"
    );
    assert!(sections().contains(&"whats-new"), "the guide has a What's new section");
}

#[test]
fn every_page_has_a_section() {
    let ids = sections();
    // Every view the app can show, from `const views = { … }` in app.js.
    let views = APP_JS.split("const views = {").nth(1).and_then(|s| s.split('}').next()).expect("views in app.js");
    let views: Vec<&str> =
        views.split(',').map(|v| v.split(':').next().unwrap().trim()).filter(|v| !v.is_empty()).collect();
    assert!(views.len() >= 10, "{views:?}");
    for v in views {
        if v != "help" {
            assert!(ids.contains(&v), "ui/guide.md needs a section with {{#{v}}} for the {v} page");
        }
    }
    // And every sidebar entry.
    for v in INDEX.split("data-view=\"").skip(1).map(|s| s.split('"').next().unwrap()) {
        assert!(v == "help" || ids.contains(&v), "ui/guide.md needs a section for the {v} page");
    }
}

#[test]
fn links_inside_the_guide_go_somewhere() {
    let ids = sections();
    for link in GUIDE.split("](#help/").skip(1).map(|s| s.split(')').next().unwrap()) {
        assert!(ids.contains(&link), "ui/guide.md links to #help/{link}, which isn't a section");
    }
}

#[test]
fn every_builtin_scenario_and_workout_is_in_the_guide() {
    for e in aimadapt_sim::library::builtins() {
        let name = e.scenario.expect("built-ins load").name;
        assert!(GUIDE.contains(&format!("| {name} |")), "ui/guide.md's scenario tables need a row for {name}");
    }
    let n = aimadapt_sim::library::builtins().len();
    assert!(
        GUIDE.contains(&format!("There are {n} built-ins")),
        "ui/guide.md should give the number of built-ins ({n})"
    );
    for e in aimadapt_sim::workout::builtins() {
        let name = e.workout.expect("built-ins load").name;
        // The program's days are covered as a range ("Core Week: Monday to … Friday").
        if !name.starts_with("Core Week: ") {
            assert!(GUIDE.contains(&format!("**{name}:**")), "ui/guide.md's workout list needs {name}");
        }
    }
}

#[test]
fn every_rating_is_explained() {
    let knob = aimadapt_sim::spec::FIELDS.iter().find(|f| f.path == "adaptive.knob").expect("knob field");
    let aimadapt_sim::spec::Kind::Choice { options } = knob.kind else { panic!("the knob is a choice") };
    for o in options.iter().filter(|o| o.value != "auto") {
        let label = o.label.split(" (").next().unwrap();
        assert!(GUIDE.contains(&format!("| {label} |")), "ui/guide.md's rating table needs {label}");
    }
}

#[test]
fn every_trainer_key_is_listed() {
    let keys = GAME.split("fn keys(").nth(1).and_then(|s| s.split("\n}\n").next()).expect("fn keys in game.rs");
    let mut found = 0;
    for code in keys.split("KeyCode::").skip(1).map(|s| s.split(')').next().unwrap()) {
        let name = match code {
            "Escape" => "Esc",
            c => c.strip_prefix("Key").unwrap_or(c),
        };
        assert!(GUIDE.contains(&format!("| {name} |")), "ui/guide.md's key table needs {name} (KeyCode::{code})");
        found += 1;
    }
    assert!(found >= 4, "found {found} keys");
}
