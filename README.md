# RustyAim

Tracks your aim training. RustyAim imports KovaaK's per-run stats files into a local SQLite database and reports progress and training time. It also has its own built-in aim trainer, with scenarios you can edit. See [BACKLOG.md](BACKLOG.md) for the plan, [SCENARIOS.md](SCENARIOS.md) for the scenario format and [TESTING.md](TESTING.md) for how it's tested.

Windows only for now. Download `RustyAim_<version>_x64-portable.exe` from [releases](../../releases) and run it; there is nothing to install. Installers (setup .exe and .msi) are attached too if you prefer a Start menu entry and uninstaller. Needs the WebView2 runtime, which ships with Windows 11 and current Windows 10.

## Layout

- `crates/rustyaim-core` handles parsing, storage, import and summaries.
- `crates/rustyaim-sim` holds the trainer's rules: the scenario format, a description of every field (used to build the editor), and the simulation (spawning, movement, hits, scoring). It has no graphics, so it's fully unit-tested.
- `crates/rustyaim-engine` is the trainer itself: a Bevy window with raw mouse input and no frame cap. It saves finished runs to the same database.
- `crates/rustyaim-cli` is the `rustyaim-cli` command line front end.
- `crates/rustyaim-app` is the Tauri desktop app. Its UI is plain HTML/CSS/JS in `ui/`, with no build step. Run with `--play <scenario>`, the same exe runs the trainer instead, which keeps the download a single file.

## Desktop app

```bash
cargo run -p rustyaim-app
```

It imports new runs on startup, watches the stats folder while open, and refreshes the screens when new runs arrive. It uses the same database as the CLI.

The **Play** screen lists the built-in and your own scenarios and workouts.

- **Adaptive** (the default) rates you in real units: the target lifetime you hit 85% of the time on timed clicking scenarios (for example "0.62 s"), or the target speed you stay on 85% of the time when tracking (for example "48 °/s"). It finds your rating within the first 15–25 targets, then mixes targets near your rating with harder stretch targets and a few easy ones. Hitting a stretch target faster than ever is a breakthrough. The next run starts from your rating, and the trainer re-checks faster after time away. **Standard** plays the scenario exactly as written and scores points.
- **Workouts** play several scenarios back to back in one session. There are four built-ins, and you can build your own.
- **Copy & edit** opens a form with every setting the engine supports, including what adaptive mode makes harder.
- The Play screen shows which settings profile you're playing with and lets you switch.

**Settings** holds your profiles, usually one per game. A profile takes your sensitivity in that game's own units (Overwatch 2, Valorant, Counter-Strike 2, Apex Legends and Call of Duty, or cm/360 directly) and converts it. Each profile also has its own DPI, FOV (with each game's usual value one click away), crosshair (with a live preview), display mode (borderless, exclusive at any resolution and refresh rate, or windowed) and a centered play area. The play area lets a big monitor play like a smaller one, and a helper sizes it from screen diagonals. Your first profile is matched to your latest KovaaK's run. Adding a profile for another game converts your current sensitivity, so it feels the same.

**Insights** shows what the trainer measured: your rating over time, your speed vs. hit-rate curve (where you're reliable and where it's a coin flip), flick speed for size and distance (Fitts' law throughput), success by direction and distance, where shots land relative to the target, reaction time, tracking accuracy by target speed and after direction changes, whether you trail or lead targets, and coaching notes. RustyAim runs also show up in Progress and Training time next to your KovaaK's runs. Adaptive runs are listed as "<scenario> (Adaptive)" and charted in Insights, not on Progress, because their result is a time or speed rather than points.

To run the trainer on its own while developing:

```bash
cargo run -p rustyaim-engine -- builtin:static-grid --windowed
```

Add `--adaptive` for adaptive mode, or use `--workout builtin:daily-fundamentals` instead of a scenario. `--smoke <folder>` makes a bot that plays like a person run short versions (`--smoke-seconds`, default 4), saves screenshots there, and exits. It's a quick check that rendering, input and saving work on a machine; add `--db <file>` to keep test runs out of your history. The app also takes `--db <file>`, so you can open a test database.

To preview the UI in a browser against your real data, dump a snapshot next to the UI files and serve the folder (the dump is gitignored):

```bash
cargo run -p rustyaim-app -- --dump-snapshot crates/rustyaim-app/ui/dev-snapshot.json
```

```bash
python -m http.server 8765 --directory crates/rustyaim-app/ui
```


## Usage

```bash
cargo run -p rustyaim-cli -- import
```

```bash
cargo run -p rustyaim-cli -- summary
```

```bash
cargo run -p rustyaim-cli -- time
```

```bash
cargo run -p rustyaim-cli -- progress
```

```bash
cargo run -p rustyaim-cli -- progress "vt ww5t novice s5"
```

```bash
cargo run -p rustyaim-cli -- sens
```

```bash
cargo run -p rustyaim-cli -- watch
```

`progress` lists scenarios with best score, change since the first run, trend, and runs since the last personal best, plus cold-start and fatigue summaries. Sort with `--sort played|improved|recent|name`. Give a scenario name (or part of one) to see its full run history. `sens` lists your sensitivity and FOV changes with cm/360.

`time` shows training time: active vs. session time, sessions, days, streak, weekday averages and runs by hour. Options: `--gap 15` (break in minutes that starts a new session), `--day-start 4` (hour a training day begins), `--goal 30` (daily goal in minutes).

The stats folder is found in your Steam libraries automatically; override it with `--stats <dir>`. The database defaults to `%APPDATA%\RustyAim\rustyaim.db`; override it with `--db <file>`. `errors` lists files that couldn't be imported.

## Development

Needs Rust (stable, MSVC toolchain) and the Visual Studio C++ build tools. WebView2 ships with Windows 11.

Enable the pre-commit hook once per clone. It checks formatting, runs clippy and the tests, and builds `dist/RustyAim.exe`:

```bash
git config core.hooksPath .githooks
```

Build the release executable by hand:

```bash
sh scripts/release-exe.sh
```

Run the tests:

```bash
cargo test --workspace
```

If the repo lives in a synced folder like OneDrive, move build output out of it with a local, gitignored `.cargo/config.toml` containing `[build]` and `target-dir = "C:/some/other/folder"`.

To publish a release, push a version tag (for example `git tag v0.1.0` then `git push origin v0.1.0`). `.github/workflows/release.yml` builds the portable exe and the MSI and NSIS installers and attaches them to a GitHub release.
