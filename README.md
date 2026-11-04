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

The **Play** screen lists the built-in and your own scenarios, with your best for each. Play opens the trainer full screen (Esc quits, R restarts, Space plays again). Copy & edit opens a form with every setting the engine supports. Your sensitivity and FOV start from your latest KovaaK's run. RustyAim runs show up in Progress and Training time next to your KovaaK's runs.

To run the trainer on its own while developing:

```bash
cargo run -p rustyaim-engine -- builtin:static-grid --windowed
```

`--smoke <folder>` makes a bot play a 4-second version, saves `play.png` and `results.png` there, and exits. It's a quick check that rendering, input and saving work on a machine; add `--db <file>` to keep the test run out of your history.

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
