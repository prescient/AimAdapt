//! Checks on the plain-JS UI in `ui/`, which has no test runner of its own. They
//! read the source as text, so most look for a function and a key token in it
//! rather than a whole line, and survive reformatting.
//!
//! To see a check fail on older code, point them at a copy of that `ui/` folder:
//! `AIMADAPT_TEST_UI_DIR=<folder> cargo test -p aimadapt-app --test ui`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn ui_dir() -> PathBuf {
    std::env::var_os("AIMADAPT_TEST_UI_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"))
}

/// A file in `ui/`, read when the test runs.
fn ui(name: &str) -> String {
    std::fs::read_to_string(ui_dir().join(name)).unwrap_or_else(|e| panic!("ui/{name}: {e}"))
}

/// Every script in `ui/` by file name, read when the test runs, so new files are covered.
fn scripts() -> BTreeMap<String, String> {
    let all: BTreeMap<String, String> = std::fs::read_dir(ui_dir())
        .expect("ui folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "js"))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(&p).unwrap()))
        .collect();
    assert!(all.len() >= 9 && all.contains_key("app.js"), "{:?}", all.keys());
    all
}

/// A top-level declaration's text: from its `function name(`, `async function name(`,
/// `const name =` or `let name =` line up to the next line that starts something
/// else at column 0 (closing brackets there still belong to it).
fn decl<'a>(src: &'a str, name: &str) -> &'a str {
    let heads = [
        format!("function {name}("),
        format!("async function {name}("),
        format!("const {name} ="),
        format!("let {name} ="),
    ];
    let mut start = None;
    let mut at = 0;
    for line in src.split_inclusive('\n') {
        if let Some(s) = start {
            if line.starts_with(|c: char| !c.is_whitespace() && !matches!(c, '}' | ')' | ']')) {
                return &src[s..at];
            }
        } else if heads.iter().any(|h| line.starts_with(h.as_str())) {
            start = Some(at);
        }
        at += line.len();
    }
    &src[start.unwrap_or_else(|| panic!("no top-level {name}"))..]
}

/// The leading identifier of `s`.
fn ident(s: &str) -> &str {
    let end = s.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(s.len());
    &s[..end]
}

/// `s` split at commas outside brackets and quotes.
fn split_top_level(s: &str) -> Vec<&str> {
    let (mut parts, mut depth, mut quote, mut from) = (Vec::new(), 0i32, None, 0);
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'' | '`') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => {
                parts.push(&s[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[from..]);
    parts
}

/// Names a script declares at the top level (column 0): functions, classes, and
/// each name in a `const`, `let` or `var` statement's first line.
fn top_level_names(file: &str, src: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in src.lines() {
        if let Some(rest) = ["async function ", "function ", "class "].iter().find_map(|k| line.strip_prefix(k)) {
            names.push(ident(rest.trim_start_matches('*').trim_start()).to_string());
        } else if let Some(rest) = ["const ", "let ", "var "].iter().find_map(|k| line.strip_prefix(k)) {
            for part in split_top_level(rest).into_iter().map(str::trim) {
                assert!(
                    !part.starts_with(['{', '[']),
                    "{file}: top-level destructuring isn't checked here, declare the names one by one: {line}"
                );
                if !part.is_empty() {
                    names.push(ident(part).to_string());
                }
            }
        }
    }
    names
}

/// Commands the Rust side registers, from `tauri::generate_handler![…]` in main.rs.
fn registered_commands() -> BTreeSet<String> {
    let main = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/main.rs")).expect("main.rs");
    let list = main
        .split("generate_handler![")
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("generate_handler! in main.rs");
    list.split(',').map(|s| s.trim().rsplit("::").next().unwrap().to_string()).filter(|s| !s.is_empty()).collect()
}

// ---------- the page as a whole ----------

/// Every script shares one global scope. A name declared twice at the top level
/// breaks the page: a second `const`, `let` or `class` is a SyntaxError that drops
/// its whole script, and a second `function` silently replaces the first.
#[test]
fn no_top_level_name_is_declared_twice() {
    let mut first: BTreeMap<String, String> = BTreeMap::new();
    let mut twice = Vec::new();
    for (file, src) in scripts() {
        for name in top_level_names(&file, &src) {
            if let Some(other) = first.insert(name.clone(), file.clone()) {
                twice.push(format!("{name} ({other} and {file})"));
            }
        }
    }
    for name in ["ui", "esc", "render", "invoke", "loadTrainer", "settingsPage", "today", "helpPage"] {
        assert!(first.contains_key(name), "didn't find {name}: is the parsing still right?");
    }
    assert!(twice.is_empty(), "declared twice: {twice:?}");
}

/// A misspelled or renamed command only fails when its page is used. Every
/// `invoke("…")` must name a command main.rs registers, by name (not computed), and
/// the browser preview's answers must be for real commands too.
#[test]
fn every_invoke_names_a_registered_command() {
    let commands = registered_commands();
    assert!(commands.len() >= 10 && commands.contains("snapshot"), "{commands:?}");
    let mut called = BTreeSet::new();
    for (file, src) in scripts() {
        for (i, _) in src.match_indices("invoke(") {
            let line = src[..i].rsplit('\n').next().unwrap();
            // Tauri's own invoke, another function's name, or a comment.
            if line.ends_with(|c: char| c == '.' || c == '_' || c.is_alphanumeric())
                || line.trim_start().starts_with("//")
            {
                continue;
            }
            let rest = &src[i + "invoke(".len()..];
            let name = rest.strip_prefix('"').and_then(|r| r.split('"').next()).unwrap_or_else(|| {
                panic!("{file}: invoke( needs its command's name in quotes: invoke({}", &rest[..rest.len().min(40)])
            });
            assert!(commands.contains(name), "{file} invokes \"{name}\", which main.rs doesn't register");
            called.insert(name.to_string());
        }
    }
    assert!(called.len() >= 15, "{called:?}");
    let app = ui("app.js");
    let preview =
        app.split("const PREVIEW = {").nth(1).and_then(|s| s.split("\n};").next()).expect("PREVIEW in app.js");
    for line in preview.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("//")) {
        let name = ident(line);
        assert!(commands.contains(name), "the preview answers \"{name}\", which main.rs doesn't register");
    }
}

/// Playtest: whole degrees per second were too coarse; the app shows one decimal,
/// like the trainer and Insights notes.
#[test]
fn target_speed_shows_one_decimal() {
    let trainer = ui("trainer.js");
    let fmt = decl(&trainer, "fmtKnob");
    assert!(fmt.contains("\"target_speed\"") && fmt.contains("toFixed(1)} °/s"), "{fmt}");
}

/// Bug: the first render could run before the scripts after app.js had run, so
/// opening the app on #help failed with "helpPage is not defined" and the page
/// stayed on "Loading". app.js must wait for DOMContentLoaded before loading.
#[test]
fn the_first_render_waits_for_every_script() {
    let app = ui("app.js");
    let start =
        app.lines().find(|l| l.contains("addEventListener(\"DOMContentLoaded\"")).expect("a DOMContentLoaded listener");
    assert!(start.contains("load()"), "app.js should start loading on DOMContentLoaded: {start}");
    assert!(
        !app.lines().any(|l| l.trim() == "load();"),
        "app.js shouldn't call load() straight away, before the other scripts have run"
    );
    // The scripts are plain, in-order <script> tags at the end of the body.
    let index = ui("index.html");
    let scripts: Vec<&str> = index.split("<script src=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    assert_eq!(scripts.first(), Some(&"app.js"), "{scripts:?}");
    assert!(scripts.contains(&"help.js"), "{scripts:?}");
}

/// Ten pages became four (Today, Scenarios, Progress, Settings). Links to the old
/// pages, in the guide's history and anywhere else, must still land somewhere.
#[test]
fn old_page_addresses_still_go_somewhere() {
    let app = ui("app.js");
    let index = ui("index.html");
    let moved = app.split("const MOVED = {").nth(1).and_then(|s| s.split('}').next()).expect("MOVED in app.js");
    for old in ["dashboard", "play", "program", "training", "benchmarks", "library"] {
        assert!(moved.contains(&format!("{old}: \"")), "#{old} needs a new home in MOVED");
    }
    // Insights and a scenario's old Progress history go to the scenario on Scenarios.
    let moved_to = decl(&app, "movedTo");
    assert!(moved_to.contains("view === \"insights\""), "#insights/<name> needs a redirect");
    assert!(moved_to.contains("view === \"progress\" && arg"), "#progress/<name> needs a redirect");
    for old in ["dashboard", "play", "program", "insights", "training", "benchmarks", "library"] {
        assert!(!index.contains(&format!("data-view=\"{old}\"")), "the sidebar shouldn't link to #{old} any more");
    }
}

/// Bug: chart ticks were rounded to one decimal before their format ran, so a
/// rating axis from 0.50 to 0.56 s read "0.50" on every line.
#[test]
fn chart_ticks_are_formatted_from_their_exact_value() {
    let insights = ui("insights.js");
    assert!(
        !insights.contains("yfmt(Math.round(v * 10) / 10)"),
        "round inside the default format, not before every format"
    );
    assert!(insights.contains("${yfmt(v)}"), "the tick labels get the exact tick value");
}

/// Bug: with no scenario in the address, Scenarios showed the list's first item,
/// so switching tabs or starring changed which scenario was open, and Favorite or
/// Hide could act on a different one. Now the lists and each item have their own
/// addresses (#scenarios/browse, #scenarios/builtin:static-grid), an item's page
/// shows only the item its address names, and every Favorite and Hide button names
/// its item.
#[test]
fn favorite_and_hide_act_on_the_item_they_name() {
    let scenarios = ui("scenarios.js");
    let page = decl(&scenarios, "scenariosPage");
    assert!(page.contains("itemPage(all.find((x) => x.key === arg)"), "{page}");
    // With no address, Scenarios goes to a list, never to an item.
    assert!(page.contains("location.replace(`#scenarios/${defaultTab(all)}`)"), "{page}");
    // Item keys have a colon (builtin:, user:, workout:, kovaaks:); list names don't.
    let tabs =
        scenarios.split("const SCENARIO_TABS = [").nth(1).and_then(|s| s.split(';').next()).expect("SCENARIO_TABS");
    assert!(tabs.contains("[\"browse\", \"Browse\"]") && !tabs.contains(':'), "{tabs}");
    // On the item's page and on each list row.
    assert!(scenarios.contains("data-action=\"favorite\" data-key=\"${esc(key)}\""));
    assert!(scenarios.contains("data-action=\"hide\" data-key=\"${esc(key)}\""));
    assert!(scenarios.contains("data-action=\"favorite\" data-key=\"${esc(x.key)}\""));
    assert!(scenarios.contains("data-action=\"hide\" data-key=\"${esc(x.key)}\""));
}

// ---------- regression tests ----------

/// Bug: settings profiles were picked by name. With "Valorant" and "Valorant 2",
/// renaming the second to "Valorant" sent its edits to the first, and Delete removed
/// both (and threw if they were the only two). The profile shown is now an index.
#[test]
fn settings_edits_go_to_the_profile_shown() {
    let s = ui("settings.js");
    let shown = decl(&s, "selectedProfile");
    assert!(!shown.contains(".name ==="), "the shown profile isn't found by name: {shown}");
    assert!(s.contains("data-action=\"select-profile\" data-index=\"${i}\""), "the list picks by position");
    assert!(!s.contains("btn.dataset.name"), "the list picks by position");
    let field = decl(&s, "setProfileField");
    assert!(!field.contains("selected ="), "renaming mustn't change which profile is shown: {field}");
    assert!(!s.contains("x.name !== name"), "Delete removes the profile shown, not every profile with its name");
}

/// Bug: Today, Scenarios, Progress › Skills and the editors asked for their data
/// again on every render while it was missing, without looking at the load's error.
/// A failed load rendered, which loaded again, forever: "Loading today…" stayed up
/// and the error never showed. They go through waitFor now, which starts each load
/// once and shows the error.
#[test]
fn a_failed_load_shows_its_error_instead_of_loading_forever() {
    let views = [
        ("today.js", "today"),
        ("program.js", "skillsTab"),
        ("program.js", "programTab"),
        ("trainer.js", "edit"),
        ("trainer.js", "workout"),
        ("scenarios.js", "scenariosPage"),
        ("settings.js", "settingsPage"),
    ];
    for (file, name) in views {
        let src = ui(file);
        let view = decl(&src, name);
        assert!(view.contains("waitFor("), "{name} in {file} should wait for its data with waitFor");
        for load in ["loadTrainer()", "loadProgram()", "loadInsightsOverview()"] {
            assert!(!view.contains(load), "{name} in {file} calls {load} itself");
        }
    }
    let app = ui("app.js");
    let wait = decl(&app, "waitFor");
    assert!(wait.contains(".error") && wait.contains("startOnce("), "{wait}");
    assert!(decl(&app, "startOnce").contains("running.has("), "a load isn't started twice at once");
    // A scenario's insights were asked for on every render while they loaded.
    assert!(decl(&ui("insights.js"), "insightsFor").contains("loadCached("));
}

/// Bug: a failed Rescan was never shown. Its error went in ui.error, which the reload
/// after it cleared, and which only shows when there's no data at all.
#[test]
fn a_failed_rescan_is_shown_next_to_the_button() {
    let app = ui("app.js");
    let rescan = decl(&app, "rescan");
    assert!(!rescan.contains("ui.error ="), "{rescan}");
    assert!(rescan.contains("ui.rescanError ="), "{rescan}");
    assert!(decl(&app, "rescanButton").contains("ui.rescanError"));
}

/// Bug: a KovaaK's scenario whose history failed to load stayed on "Loading <name>…",
/// and every render asked again. The failure is kept and shown, as sce_info's was.
#[test]
fn a_failed_scenario_history_is_shown() {
    let app = ui("app.js");
    assert!(decl(&app, "scenarioDetail").contains("loadCached("));
    assert!(decl(&app, "loadCached").contains("error: String("), "a failed load is kept as {{ error }}");
    let scenarios = ui("scenarios.js");
    let detail = decl(&scenarios, "kovaaksDetail");
    assert!(detail.contains("detail?.error"), "kovaaksDetail shows the error: {detail}");
}

/// Bug: emptying a "Custom (cm/360)" profile's cm/360 stored null, and sensText
/// called toFixed on it and threw. render() had no try/catch, so once you left
/// Settings that way every render threw and Settings couldn't be opened again.
#[test]
fn an_empty_sensitivity_field_cant_break_rendering() {
    for (file, src) in scripts() {
        for (i, _) in src.match_indices("profileCm(") {
            let mut depth = 0;
            let close = src[i..]
                .char_indices()
                .find(|&(_, c)| {
                    depth += match c {
                        '(' => 1,
                        ')' => -1,
                        _ => 0,
                    };
                    c == ')' && depth == 0
                })
                .map(|(j, _)| i + j)
                .expect("closing parenthesis");
            assert!(!src[close + 1..].starts_with(".toFixed"), "{file}: profileCm can be null; format it with cmText");
        }
    }
    let settings = ui("settings.js");
    assert!(!decl(&settings, "sensText").contains("cm.toFixed"));
    assert!(decl(&settings, "profileCm").contains("Number.isFinite"), "profileCm says null for a missing number");
    let app = ui("app.js");
    let render = decl(&app, "render");
    assert!(render.contains("try {") && render.contains("catch") && render.contains("console.error("), "{render}");
}

/// Bug: Cancel in the scenario and workout editors was a plain link, and the draft
/// was only cleared on Save or Delete, so a cancelled change came back when the
/// editor was opened again.
#[test]
fn cancel_in_the_editors_drops_the_draft() {
    let trainer = ui("trainer.js");
    assert!(!trainer.contains(">Cancel</a>"), "Cancel is an action, not a link");
    for (action, clears) in [("cancel-edit", "tr.edit = null"), ("cancel-workout", "tr.wedit = null")] {
        assert!(trainer.contains(&format!("data-action=\"{action}\"")), "{action} button");
        let handler = trainer.split(&format!("action === \"{action}\"")).nth(1).expect("its click handler");
        let handler = &handler[..handler.find("else if").unwrap_or(handler.len())];
        assert!(handler.contains(clears), "{action} should do {clears}: {handler}");
    }
}

/// Bug: the scenario editor's check answers 150 ms (and a call) after typing, and
/// wrote to the page without checking it was still the editor: it could retitle the
/// next page's heading, and a failed save after leaving threw on the missing
/// #edit-status.
#[test]
fn late_editor_checks_only_touch_their_own_editor() {
    let trainer = ui("trainer.js");
    let shows = decl(&trainer, "editorShows");
    assert!(shows.contains("route().view === \"edit\"") && shows.contains("ui.trainer.edit === draft"), "{shows}");
    assert!(decl(&trainer, "recheck").contains("if (!editorShows(e)) return;"));
    assert!(decl(&trainer, "saveScenario").contains("if (!editorShows(e)) return;"));
    assert!(!trainer.contains("getElementById(\"edit-status\").innerHTML"), "#edit-status can be missing");
}

// ---------- features and tidy-ups that should stay ----------

/// The trainer can close with an error (`trainer-closed` carries { error }), and a
/// settings file can be unusable (`profiles_note`). Both are shown, escaped, as
/// warnings: the first on the pages that start the trainer until dismissed or the
/// next start, the second on Settings › Aim and display.
#[test]
fn trainer_errors_and_unusable_settings_are_shown() {
    let trainer = ui("trainer.js");
    let closed = trainer.split("listen(\"trainer-closed\"").nth(1).expect("trainer-closed listener");
    assert!(closed.contains("payload?.error"), "the listener keeps the error");
    let banner = decl(&trainer, "trainerErrorBanner");
    assert!(banner.contains("banner warn") && banner.contains("esc(error)"), "{banner}");
    assert!(decl(&trainer, "launchTrainer").contains("closedError = null"), "a new start clears it");
    assert!(decl(&ui("scenarios.js"), "trainerBanners").contains("trainerErrorBanner()"), "Today and Scenarios");
    assert!(decl(&ui("scenarios.js"), "programPage").contains("trainerBanners()"), "Scenarios › Program");
    assert!(decl(&ui("program.js"), "skillsTab").contains("trainerErrorBanner()"), "Progress › Skills");
    let settings = ui("settings.js");
    let notice = decl(&settings, "settingsNotice");
    assert!(notice.contains("esc(d.profiles_note)") && notice.contains("banner warn"), "{notice}");
}

/// Views reach Tauri only through app.js's invoke() and listen(), whose preview
/// answers from crates/aimadapt-app/dev/ (never ui/, which is built into the exe);
/// and the score chart draws with the shared chart helpers (one place for ticks)
/// and the theme's colors.
#[test]
fn views_use_the_shared_helpers() {
    for (file, src) in scripts() {
        if file != "app.js" {
            assert!(!src.contains("tauri") && !src.contains("devData"), "{file} should only use invoke() and listen()");
        }
    }
    let app = ui("app.js");
    assert!(app.contains("fetch(\"../dev/dev-snapshot.json\")"));
    let chart = decl(&app, "scoreChart");
    assert!(chart.contains("yTicks(") && chart.contains("sx(") && chart.contains("sy("), "{chart}");
    assert!(!chart.contains("=\"#") && !chart.contains(":#"), "colors come from the theme's variables");
}

/// Every render replaces <main>, which dropped the keyboard focus, and fields showed
/// no focus at all. render() puts the focus back, and focus is drawn.
#[test]
fn keyboard_focus_shows_and_survives_a_render() {
    let app = ui("app.js");
    let render = decl(&app, "render");
    assert!(render.contains("focusedControl(") && render.contains("refocus("), "{render}");
    for (file, src) in scripts() {
        if file != "app.js" {
            assert!(!src.contains("setSelectionRange("), "{file}: render() keeps the caret");
        }
    }
    let css = ui("styles.css");
    assert!(css.contains(":focus-visible") && css.contains(".field:focus-within"));
}

/// Rule (plan L1): Play always means an adaptive run of one AimAdapt scenario.
/// Sessions (workouts and program days) say "session", never Play, and the
/// scenario page's secondary button is "Standard run". The replay page's
/// Play/Pause is a transport control and is left out.
#[test]
fn play_always_means_an_adaptive_run() {
    for (file, src) in scripts() {
        assert!(!src.contains("Play adaptive"), "{file} still says \"Play adaptive\"");
    }
    let scenarios = ui("scenarios.js");
    assert!(
        decl(&scenarios, "trainerDetail").contains(">Standard run</button>"),
        "the scenario header's secondary button"
    );
    assert!(decl(&scenarios, "trainerDetail").contains("adaptive · "), "the line under Play says what it does");
    // Every session button is labelled "session" and never Play.
    let mut seen = 0;
    for (file, src) in scripts() {
        for (at, _) in src.match_indices("<button") {
            let button = &src[at..];
            let Some(end) = button.find("</button>") else { continue };
            let button = &button[..end];
            let Some(action) =
                ["play-workout", "play-program"].into_iter().find(|a| button.contains(&format!("data-action=\"{a}\"")))
            else {
                continue;
            };
            let label = &button[button.find('>').unwrap() + 1..];
            assert!(label.to_lowercase().contains("session"), "{file}: {action} button says {label:?}");
            assert!(!label.contains("Play"), "{file}: {action} button says {label:?}");
            seen += 1;
        }
    }
    // The program's one sessionButton (Today and the Program tab share it) and the workout buttons.
    assert!(seen >= 3, "found {seen} session buttons");
    // A KovaaK's scenario that plays here has one button, "Play in AimAdapt".
    let play_sce = scenarios.matches("data-action=\"play-sce\"").count();
    assert!(play_sce >= 2, "{play_sce}");
    for (at, _) in scenarios.match_indices("data-action=\"play-sce\"") {
        let rest = &scenarios[at..];
        assert!(rest[..rest.find("</button>").unwrap()].ends_with("Play in AimAdapt"), "play-sce button");
    }
}

/// Found checking plan L1 in the browser, before it was committed: the longer
/// session labels stuck out of the week cards' buttons on windows under about
/// 1300 px wide, and the line under Play stretched the buttons beside it to its
/// height. A week card's button wraps its label, the week drops to four columns
/// before a word can't fit, and the scenario header keeps its buttons their own
/// height. (Layout can't be measured here, so this checks the rules that fix it.)
#[test]
fn longer_button_labels_fit_where_they_are_shown() {
    let css = ui("styles.css");
    let rule = |selector: &str| {
        let at = css.find(&format!("\n{selector} {{")).unwrap_or_else(|| panic!("no rule for {selector}"));
        let rest = &css[at..];
        rest[..rest.find('}').unwrap()].to_string()
    };
    assert!(!rule(".small-btn").contains("nowrap"), "a week card's button wraps its label");
    assert!(
        css.contains("@media (max-width: 1100px) { .week-grid { grid-template-columns: repeat(4,"),
        "the week has four columns on narrower windows"
    );
    assert!(
        rule(".detail-head .card-head > .chips").contains("align-items: flex-start"),
        "the scenario header's buttons keep their own height"
    );
}

/// Rule (plan L2): a scenario's page has Overview, Insights and Runs tabs, each with
/// its own address (`#scenarios/<key>`, `/insights`, `/runs`), instead of one long
/// scroll with jump buttons. Only `route()` for the scenarios view splits a tab off,
/// and only when there are two segments after the view and the last is a tab id, so
/// list tabs and every other view's addresses parse as before.
#[test]
fn a_scenario_pages_tab_comes_from_its_address() {
    let app = ui("app.js");
    let route = decl(&app, "route");
    assert!(route.contains("view === \"scenarios\""), "only Scenarios has tabs in its address: {route}");
    assert!(route.contains("rest.length >= 2"), "a bare key or list tab has no tab: {route}");
    assert!(route.contains("ITEM_TABS.some(") && route.contains("rest.at(-1)"), "the last segment is the tab: {route}");
    assert!(route.contains("decodeURIComponent(rest.join(\"/\"))"), "the old parse stays for everything else: {route}");
    let tabs = app.split("const ITEM_TABS = [").nth(1).and_then(|s| s.split("];").next()).expect("ITEM_TABS");
    for id in ["overview", "insights", "runs"] {
        assert!(tabs.contains(&format!("[\"{id}\", ")), "{id} is a tab: {tabs}");
    }
    // The tab reaches the page, and the Overview tab is the bare address.
    assert!(decl(&app, "render").contains("(arg, tab)"));
    assert!(decl(&app, "tabs").contains("bare"), "tabs() can leave one tab's address bare");
    let scenarios = ui("scenarios.js");
    assert!(decl(&scenarios, "itemTabs").contains("\"overview\")"), "Overview is the bare tab");
    let href = decl(&scenarios, "scenarioHref");
    assert!(href.contains("tab !== \"overview\""), "Overview adds no suffix: {href}");
    // A name: address swaps to the item's address and takes its tab along.
    assert!(decl(&scenarios, "scenariosPage").contains("scenarioHref(item.key, tab)"));
}

/// Rule (plan L2): both detail pages draw tabs under one header and have no jump
/// buttons or section ids left over from them. An AimAdapt page tabs only when it
/// has detailed records, and a KovaaK's page has no Insights tab.
#[test]
fn scenario_pages_draw_tabs_and_no_jump_buttons() {
    let scenarios = ui("scenarios.js");
    for (file, src) in scripts() {
        assert!(!src.contains("data-action=\"jump\"") && !src.contains("jumpLinks"), "{file} still has jump buttons");
    }
    assert!(!ui("styles.css").contains(".jump"), "the jump styles are gone");
    let trainer = decl(&scenarios, "trainerDetail");
    assert!(trainer.contains("itemTabs(e.id, [\"overview\", \"insights\", \"runs\"]"), "{trainer}");
    // The tabs come after the early returns for no records, so those pages have none.
    let tabs_at = trainer.find("itemTabs(").unwrap();
    for early in ["Not played yet", "Loading…", "No detailed records"] {
        assert!(trainer.find(early).unwrap() < tabs_at, "{early} shows without tabs");
    }
    assert!(trainer.contains("insightKpis(d, false)"), "the header has the rating now");
    assert!(trainer.contains("ratingLine("), "the header's figures line");
    let kovaaks = decl(&scenarios, "kovaaksDetail");
    assert!(kovaaks.contains("itemTabs(key, [\"overview\", \"runs\"]"), "{kovaaks}");
    assert!(!kovaaks.contains("insights\""), "a KovaaK's page has no Insights tab");
    assert!(kovaaks.contains("if (!cached) return"), "no history, no tabs");
    // The Overview tab is the bare address; the others add their id.
    assert!(trainer.contains("scenarioHref(e.id, \"runs\")") && trainer.contains("scenarioHref(e.id, \"insights\")"));
    // The header's figures line says what the latest rating and best held are.
    let line = decl(&ui("insights.js"), "ratingLine").to_string();
    assert!(line.contains("best held") && line.contains("likely"), "{line}");
}

/// Bug (found checking plan L2 in the browser): a tab too short to scroll had no
/// scrollbar, so the page was 15 px wider there and the header jumped sideways when
/// you switched tabs. The scrollbar's space is now always kept.
#[test]
fn switching_tabs_does_not_shift_the_page_sideways() {
    let css = ui("styles.css");
    let html = css.lines().find(|l| l.starts_with("html {")).expect("a rule for html alone");
    assert!(html.contains("scrollbar-gutter: stable"), "{html}");
}

/// Bug (plan A3): the scenario page took its rating line, best, best held and rating
/// chart from every rated run, so after you edited a scenario they mixed in ratings
/// from the old settings, which are on another scale. They now come from one list,
/// `ratedRuns`, that keeps only the current version's runs, and the run tables grey
/// the ratings that list leaves out.
#[test]
fn rating_figures_use_only_the_current_versions_runs() {
    let insights = ui("insights.js");
    let rated = decl(&insights, "ratedRuns");
    assert!(rated.contains("r.current") && rated.contains("r.knob === d.knob"), "{rated}");
    assert!(decl(&insights, "insightKpis").contains("ratedRuns(d)"));
    assert!(decl(&ui("scenarios.js"), "trainerDetail").contains("ratedRuns(d)"));
    // No page builds its own list of rated runs.
    for (file, src) in scripts() {
        assert_eq!(src.matches("r.rating !== null && r.knob").count(), usize::from(file == "insights.js"), "{file}");
    }
    // Both run tables draw ratings through one cell, which greys another version's.
    let cell = decl(&insights, "ratingCell");
    assert!(cell.contains("r.current") && cell.contains("muted"), "{cell}");
    for table in ["insightRunsTable", "latestRunsCard"] {
        assert!(decl(&insights, table).contains("ratingCell(r)"), "{table}");
    }
}

// ---------- plan L4: the Program tab, Progress trimmed ----------

/// Where `needle` first appears in `text`, or a failure naming both.
fn at(text: &str, needle: &str) -> usize {
    text.find(needle).unwrap_or_else(|| panic!("{needle:?} isn't in:\n{text}"))
}

/// The body of the CSS rule for `selector` (alone at the start of a line).
fn css_rule(css: &str, selector: &str) -> String {
    let start = css.find(&format!("\n{selector} {{")).unwrap_or_else(|| panic!("no rule for {selector}"));
    let rest = &css[start..];
    rest[..rest.find('}').unwrap()].to_string()
}

/// Rule (plan L4): Scenarios has a Program tab between Workouts and Yours. It sits in
/// the same row of tabs as the lists but isn't a list: it has no TAB_SETUP, search or
/// table, `scenariosPage` sends it to its own page, and that page has the lists' head
/// and banners. The back link from a drill opened there returns to it.
#[test]
fn scenarios_has_a_program_tab_that_is_not_a_list() {
    let scenarios = ui("scenarios.js");
    let tabs =
        scenarios.split("const SCENARIO_TABS = [").nth(1).and_then(|s| s.split(';').next()).expect("SCENARIO_TABS");
    assert!(at(tabs, "[\"workouts\"") < at(tabs, "[\"program\", \"Program\"]"), "{tabs}");
    assert!(at(tabs, "[\"program\"") < at(tabs, "[\"yours\""), "{tabs}");

    assert!(decl(&scenarios, "isListTab").contains("isScenarioTab(arg)"));
    assert!(decl(&scenarios, "scenarioTabList").contains("SCENARIO_TABS"));
    assert!(decl(&scenarios, "listPage").contains("scenarioTabList(all, tab)"));
    assert!(decl(&scenarios, "programPage").contains("scenarioTabList(all, \"program\")"));
    // Not a list: nothing indexes TAB_SETUP["program"], and isListTab says no.
    let setup = scenarios.split("const TAB_SETUP = {").nth(1).and_then(|s| s.split("\n};").next()).expect("TAB_SETUP");
    assert!(!setup.contains("program"), "Program has no list setup: {setup}");
    assert!(decl(&scenarios, "isListTab").contains("!== \"program\""));
    // scenariosPage sends it to the program page before the list page, and remembers it
    // as the tab to come back to.
    let page = decl(&scenarios, "scenariosPage");
    let program = at(page, "arg === \"program\"");
    let list = at(page, "isListTab(arg)");
    assert!(program < list, "{page}");
    let branch = &page[program..list];
    assert!(branch.contains("programPage(all)") && branch.contains("ui.list.tab = arg"), "{branch}");
    // The page has the lists' head and banners, the tab row with Program selected, and the body.
    let program_page = decl(&scenarios, "programPage");
    for part in ["scenariosHead()", "trainerBanners()", "tabs(\"scenarios\"", "programTab()"] {
        assert!(program_page.contains(part), "programPage should draw {part}: {program_page}");
    }
    assert!(!program_page.contains("listPage(") && !program_page.contains("tabItems("));
    assert!(decl(&scenarios, "listPage").contains("scenariosHead()"), "the lists draw the same head");
    // The back link and the sidebar go to the last tab, Program included, by its label.
    assert!(decl(&scenarios, "itemPage").contains("TAB_LABELS[listTab]"));
    assert!(decl(&scenarios, "defaultTab").contains("ui.list.tab"));
    assert!(decl(&scenarios, "TAB_LABELS").contains("SCENARIO_TABS"));
    // Core Week's days stay off the Workouts list, which points to the tab for them.
    assert!(decl(&scenarios, "isProgramDay").contains("builtin:core-"));
    assert!(decl(&scenarios, "listPage").contains("#scenarios/program"), "the Workouts tab points to Program");
    // Today's week links to it.
    let today_js = ui("today.js");
    let strip = decl(&today_js, "weekStrip");
    assert!(strip.contains("href=\"#scenarios/program\"") && !strip.contains("Week in detail"), "{strip}");
    // The program redraws the page when it arrives, on Scenarios too.
    assert!(decl(&ui("program.js"), "loadProgram").contains("\"scenarios\""));
}

/// Rule (plan L4): you start a program day from Today's card or the Program tab, and
/// nowhere else. Progress is for results. Both draw the button, the label and the
/// drill names through one helper each.
#[test]
fn program_start_buttons_are_only_on_today_and_the_program_tab() {
    let mut buttons = 0;
    let mut callers = Vec::new();
    for (file, src) in scripts() {
        buttons += src.matches("type=\"button\" data-action=\"play-program\"").count();
        for name in ["skillsTab", "weekCard", "notesCard", "profileCard", "sessionsCard"] {
            if src.contains(&format!("function {name}(")) {
                let text = decl(&src, name);
                assert!(
                    !text.contains("play-program") && !text.contains("sessionButton("),
                    "{file}: {name} is Progress and has no start button"
                );
            }
        }
        for func in ["todayWorkout", "dayCard"] {
            if src.contains(&format!("function {func}(")) && decl(&src, func).contains("sessionButton(") {
                callers.push(func);
            }
        }
    }
    assert_eq!(buttons, 1, "the one button is drawn by sessionButton");
    callers.sort_unstable();
    assert_eq!(callers, ["dayCard", "todayWorkout"]);
    let program = ui("program.js");
    let button = decl(&program, "sessionButton");
    assert!(button.contains("data-action=\"play-program\"") && button.contains("data-day="), "{button}");
    assert!(
        button.contains("ui.program.data.playing || ui.trainer.data.playing") && button.contains("disabled"),
        "{button}"
    );
    let label = decl(&program, "sessionLabel");
    for text in ["Start session", "Continue session", "Repeat session"] {
        assert!(label.contains(text), "{label}");
    }
    let today = ui("today.js");
    for text in ["Start session", "Continue session", "Repeat session", "scenarioName("] {
        assert!(!today.contains(text), "today.js builds its own {text}: use sessionLabel and drillLookup");
    }
    assert!(decl(&today, "todayWorkout").contains("drillLookup()"));
    assert!(decl(&program, "programTab").contains("drillLookup()"));
}

/// Rule (plan L4): the Program tab shows the plan, never a result. A day's card has
/// its drills (linked when the scenario exists), runs and mode, a rest day has no
/// button, and the cards wrap instead of running off the page.
#[test]
fn the_program_tab_shows_the_plan_and_no_results() {
    let program = ui("program.js");
    let tab = decl(&program, "programTab");
    let card = decl(&program, "dayCard");
    for part in ["#help/program", "#progress/skills", "d.description", "days_done", "75%", "made up"] {
        assert!(tab.contains(part), "programTab should have {part}");
    }
    assert!(
        !tab.contains("focus_change") && !card.contains("focus_change"),
        "focus_change is a result: it stays on Progress"
    );
    for part in [
        "BLOCK_LABELS",
        "STATUS_LABELS",
        "scenarioHref(",
        "it.mode === \"standard\"",
        "Rest day",
        "weak_spots",
        "bar-track",
        "isToday",
    ] {
        assert!(card.contains(part), "dayCard should have {part}");
    }
    assert!(at(card, "Rest day") < at(card, "sessionButton("), "a rest day returns before the button");
    // A rest day says so once: "Rest day" stands in for the focus, with no second line.
    assert_eq!(card.matches("Rest").count(), 1, "{card}");
    assert!(card.contains("esc(name)"), "drill names come from the user's files");
    let css = ui("styles.css");
    assert!(
        css_rule(&css, ".day-grid").contains("repeat(auto-fill, minmax("),
        "the days wrap into as many columns as fit"
    );
    let name = css_rule(&css, ".dd-name");
    assert!(name.contains("min-width: 0") && name.contains("overflow-wrap"), "long drill names wrap");
    assert!(css_rule(&css, ".day-card .btn").contains("align-self: flex-start"), "the button keeps its own width");
    for selector in [".day-grid", ".day-card", ".day-drills li", ".dd-name", ".dd-block", ".dd-runs"] {
        assert!(!css_rule(&css, selector).contains("nowrap"), "{selector} must not stop wrapping");
    }
}

/// Rule (plan L4): Progress › Skills leads with the profile and the notes, then
/// reps and carry-over, then the week, which is results only and links to the plan.
#[test]
fn skills_are_ordered_profile_then_carry_over_then_week() {
    let program = ui("program.js");
    let skills = decl(&program, "skillsTab");
    assert!(at(skills, "kpi(") < at(skills, "profileCard("));
    assert!(at(skills, "profileCard(") < at(skills, "notesCard()"));
    assert!(at(skills, "notesCard()") < at(skills, "sessionsCard("));
    assert!(at(skills, "sessionsCard(") < at(skills, "weekCard("), "{skills}");
    let week = decl(&program, "weekCard");
    assert!(week.contains("href=\"#scenarios/program\"") && week.contains("focus_change"), "{week}");
}

/// Rule (plan L4): Benchmarks says where its figures come from. Its label names
/// KovaaK's, it explains itself with or without scores, and an empty one points to
/// where the stats are set up.
#[test]
fn benchmarks_say_they_come_from_kovaaks() {
    let app = ui("app.js");
    let tabs = app.split("const PROGRESS_TABS = [").nth(1).and_then(|s| s.split("];").next()).expect("PROGRESS_TABS");
    assert!(tabs.contains("[\"benchmarks\", \"Benchmarks from KovaaK's\"]"), "{tabs}");
    let bench = decl(&app, "benchmarksTab");
    let intro = at(bench, "benchmark scenarios, read from your KovaaK's stats");
    assert!(bench[intro..].contains("own pages") && bench[intro..].contains("#progress/skills"), "{bench}");
    // The intro comes before the table, so it shows when there are scores. The empty
    // state is the table's else branch and says what to do.
    assert!(intro < at(bench, "<table>"));
    let empty = &bench[at(bench, "None of them has been played in KovaaK's")..];
    assert!(empty.contains("href=\"#settings/kovaaks\""), "{empty}");
    assert!(app.contains("[\"kovaaks\", \"KovaaK's\"]"), "Settings has a KovaaK's tab for that address");
}

/// Rule (plan L4): Progress › Time keeps the totals, the daily chart, the calendar
/// and the sessions up top, and folds the other four cards into "More about when you
/// train", closed to begin with. The fold is a native <details> (the keyboard works),
/// its open state is kept in `ui.folds` because every action redraws the page (Show
/// more sessions, a month change), and `toggle` doesn't bubble, so it is heard in the
/// capture phase.
#[test]
fn the_time_tab_folds_its_extra_cards() {
    let app = ui("app.js");
    let time = decl(&app, "timeTab");
    let fold = at(time, "fold(\"time-more\", \"More about when you train\"");
    assert!(at(time, "kpi(") < at(time, "dailyChart("));
    for up_top in ["dailyChart(", "calendar()", "sessionsList()"] {
        assert!(at(time, up_top) < fold, "{up_top} stays above the fold");
    }
    let inside = &time[fold..];
    for card in ["timeline()", "weekdays(", "hours()", "activeIdle("] {
        assert!(inside.contains(card), "{card} goes inside the fold");
        assert_eq!(time.matches(card).count(), 1, "{card} is drawn once");
    }
    let helper = decl(&app, "fold");
    assert!(helper.contains("<details") && helper.contains("<summary"), "{helper}");
    assert!(helper.contains("ui.folds[id] ? \"open\" : \"\""), "closed unless it was opened: {helper}");
    assert!(helper.contains("<summary id=\"fold-"), "the summary has an id, so render() puts the focus back: {helper}");
    assert!(app.contains("folds: {}"), "ui.folds starts empty");
    let listener = &app[at(&app, "addEventListener(\"toggle\"")..];
    let listener = &listener[..at(listener, "}, true);") + "}, true);".len()];
    assert!(
        listener.contains("ui.folds[id] = e.target.open"),
        "capture phase, since toggle doesn't bubble: {listener}"
    );
    // The summary shows the keyboard focus like other controls.
    assert!(ui("styles.css").contains("summary):focus-visible"));
}

/// Rule (plan L4): notes are ranked once. `rankedNotes` builds each one, in order of
/// importance; Today shows the first and Progress › Skills all of them; `coachNotes`
/// (a second list that was cut to three and never shown past its first) is gone.
#[test]
fn notes_are_ranked_once_and_both_pages_draw_from_the_list() {
    for (file, src) in scripts() {
        assert!(!src.contains("coachNotes"), "{file} still has coachNotes: use rankedNotes");
    }
    let program = ui("program.js");
    let ranked = decl(&program, "rankedNotes");
    let order = [
        "is furthest behind",
        "Plateau on",
        "Warm up first",
        "Repeat before you switch",
        "Not trained in a week",
        "Settings changed on",
        "weak spots",
    ];
    let mut last = 0;
    for title in order {
        let here = at(ranked, title);
        assert!(here > last, "{title} is out of order in rankedNotes");
        last = here;
    }
    assert!(
        !ranked.contains("Keep going") && !ranked.contains("slice(0"),
        "no placeholder and no cap in the shared list"
    );
    // Today: the top note only, no card without one. Progress: every note, with the
    // placeholder text only for an empty list.
    let today_js = ui("today.js");
    let worth = decl(&today_js, "worthKnowing");
    assert!(worth.contains("const [note] = rankedNotes()") && worth.contains("if (!note) return \"\""), "{worth}");
    assert!(worth.contains("#progress/skills"), "a link to the full list");
    let card = decl(&program, "notesCard");
    assert!(
        card.contains("rankedNotes()") && card.contains(".map(noteView)") && card.contains("Notes show up here"),
        "{card}"
    );
    // One list: nothing else builds notes or calls rankedNotes.
    let calls: usize = scripts()
        .values()
        .map(|s| s.matches("rankedNotes()").count() - s.matches("function rankedNotes()").count())
        .sum();
    assert_eq!(calls, 2, "Today and Progress call rankedNotes, and nothing else does");
    // A note is data: drawn escaped, with the one skill button the furthest-behind note has.
    let view = decl(&program, "noteView");
    assert!(view.contains("esc(n.title)") && view.contains("esc(n.body)"), "{view}");
    assert!(view.contains("data-action=\"browse-skill\"") && view.contains("esc(n.skill)"), "{view}");
}
