//! Checks on the plain-JS UI in `ui/`, which has no test runner of its own.

const APP_JS: &str = include_str!("../ui/app.js");
const INDEX: &str = include_str!("../ui/index.html");

/// Playtest: whole degrees per second were too coarse; the app shows one decimal,
/// like the trainer and Insights notes.
#[test]
fn target_speed_shows_one_decimal() {
    let trainer = include_str!("../ui/trainer.js");
    assert!(trainer.contains("if (knob === \"target_speed\") return `${v.toFixed(1)} °/s`;"), "fmtKnob in trainer.js");
}

/// Bug: the first render could run before the scripts after app.js had run, so
/// opening the app on #help failed with "helpPage is not defined" and the page
/// stayed on "Loading". app.js must wait for DOMContentLoaded before loading.
#[test]
fn the_first_render_waits_for_every_script() {
    assert!(
        APP_JS.contains("document.addEventListener(\"DOMContentLoaded\", () => load());"),
        "app.js should start loading on DOMContentLoaded"
    );
    assert!(
        !APP_JS.lines().any(|l| l.trim() == "load();"),
        "app.js shouldn't call load() straight away, before the other scripts have run"
    );
    // The scripts are plain, in-order <script> tags at the end of the body.
    let scripts: Vec<&str> = INDEX.split("<script src=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    assert_eq!(scripts.first(), Some(&"app.js"), "{scripts:?}");
    assert!(scripts.contains(&"help.js"), "{scripts:?}");
}

/// Ten pages became four (Today, Scenarios, Progress, Settings). Links to the old
/// pages, in the guide's history and anywhere else, must still land somewhere.
#[test]
fn old_page_addresses_still_go_somewhere() {
    let moved = APP_JS.split("const MOVED = {").nth(1).and_then(|s| s.split('}').next()).expect("MOVED in app.js");
    for old in ["dashboard", "play", "program", "training", "benchmarks", "library"] {
        assert!(moved.contains(&format!("{old}: \"")), "#{old} needs a new home in MOVED");
    }
    // Insights and a scenario's old Progress history go to the scenario on Scenarios.
    assert!(APP_JS.contains("if (view === \"insights\")"), "#insights/<name> needs a redirect");
    assert!(APP_JS.contains("if (view === \"progress\" && arg"), "#progress/<name> needs a redirect");
    for old in ["dashboard", "play", "program", "insights", "training", "benchmarks", "library"] {
        assert!(!INDEX.contains(&format!("data-view=\"{old}\"")), "the sidebar shouldn't link to #{old} any more");
    }
}

/// Bug: chart ticks were rounded to one decimal before their format ran, so a
/// rating axis from 0.50 to 0.56 s read "0.50" on every line.
#[test]
fn chart_ticks_are_formatted_from_their_exact_value() {
    let insights = include_str!("../ui/insights.js");
    assert!(
        !insights.contains("yfmt(Math.round(v * 10) / 10)"),
        "round inside the default format, not before every format"
    );
    assert!(insights.contains("${yfmt(v)}"), "axes() passes the exact tick value");
}
