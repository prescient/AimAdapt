//! Checks on the plain-JS UI in `ui/`, which has no test runner of its own.

const APP_JS: &str = include_str!("../ui/app.js");
const INDEX: &str = include_str!("../ui/index.html");

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
