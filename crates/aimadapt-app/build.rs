//! Everything in `ui/` is built into the exe, so the build refuses files there that
//! aren't part of the UI: a `dev-snapshot.json` (a player's whole history, for the
//! browser preview) once ended up inside a local build that way.

const UI_FILES: &[&str] = &["html", "js", "css", "md", "svg", "png", "ico", "woff2"];

fn main() {
    println!("cargo:rerun-if-changed=ui");
    let stray: Vec<String> = std::fs::read_dir("ui")
        .expect("ui folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() || !p.extension().and_then(|x| x.to_str()).is_some_and(|x| UI_FILES.contains(&x)))
        .map(|p| p.display().to_string())
        .collect();
    assert!(
        stray.is_empty(),
        "these would be built into the exe, so they can't be in ui/: {}. Browser-preview data goes in dev/.",
        stray.join(", ")
    );
    tauri_build::build()
}
