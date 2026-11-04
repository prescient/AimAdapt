# RustyAim Backlog

A Rust desktop app that imports KovaaK's stats files into a local database, tracks progress and training time, and builds daily training plans that get harder as you improve.

UI mockup: https://claude.ai/artifact/Bv4XCqQQwy3aRVuU6wEgP1

**Priority:** P0 = needed for a first usable build · P1 = core features · P2 = the adaptive training feature · P3 = later

**Status:** `todo` · `doing` · `done` · `spike` (research before committing)

---

## Known data sources (checked against a real install, Nov 2026)

| What | Where (default Steam library) | Format |
|---|---|---|
| Run results | `FPSAimTrainer\FPSAimTrainer\stats\*.csv` | One file per run. Kill rows, weapon totals, summary `Key:,value` lines, and the settings used for the run |
| Playlists | `FPSAimTrainer\Saved\SaveGames\Playlists\*.json` | `playlistName`, `scenarioList[{scenario_name, play_Count}]`, `shareCode`, … |
| Active playlist | `Saved\SaveGames\PlaylistInProgress.json`, `PlaylistOrder.json` | Current playlist and which scenario you're on |
| Local scenarios | `Saved\SaveGames\Scenarios\*.sce` | INI-like text: `Timescale=`, bot profiles, speeds, map scale |
| Workshop scenarios | `steamapps\workshop\content\824270\<id>\` | Same `.sce` format (196 items installed) |

The stats file name holds the scenario name, the mode (`Challenge`) and the finish time: `<Scenario> - Challenge - 2026.11.03-06.31.39 Stats.csv`. The run length is the finish time minus `Challenge Start:`.

---

## Epic 0: Foundations (P0)

- [x] `done` **Pick the UI stack:** Tauri 2 (`crates/rustyaim-app`). The frontend is plain HTML/CSS/JS in `crates/rustyaim-app/ui` with no Node or bundler; it talks to Rust through `window.__TAURI__`.
- [x] `done` Set up a Cargo workspace: `crates/rustyaim-core` (parsing, database, stats logic) and `crates/rustyaim-cli` (command line for now; the UI app comes later). All logic lives in core so it can be tested without a UI.
- [x] `done` SQLite database (`rusqlite`, migrations tracked in `PRAGMA user_version`). Schema v1 has `runs`, `run_fields`, `kills`, `weapons` and `import_errors`. `sessions`, `tags`, `plans` and `plan_items` get added in later migrations.
- [ ] `todo` Config file: stats folder path, session gap, daily goal, time a training day starts. (For now the CLI takes `--stats` and `--db` options.)
- [ ] `doing` Logging plus a "parse errors" list shown in the UI. Errors are stored in `import_errors` and listed by `rustyaim errors`; there's no UI or logging yet.

## Epic 1: Importing files (P0)

- [x] `done` Find the KovaaK's install automatically, including Steam libraries on other drives (reads `libraryfolders.vdf`). The CLI's `--stats` option is the fallback until the UI has a folder picker.
- [x] `done` Parse the file name: scenario, mode, finish time (local time). Handles scenario names that contain ` - `.
- [x] `done` Parse the CSV sections: kill rows, weapon totals, summary values (score, hits, misses, TTK, damage, `Challenge Start`, pause count and duration, `Avg Target Scale`, `Avg Time Dilation`), and settings (sens, sens scale, DPI, FOV, FPS cap, average FPS, resolution, game version).
- [x] `done` Make parsing tolerant: columns are looked up by header name, bad or missing values become null, and every `Key:,value` line is kept in `run_fields`.
- [x] `done` Skip duplicates: the file name is unique per run (scenario plus finish time to the second), so it's the key.
- [x] `done` Import incrementally: files already in the database are skipped by name, so a rescan is cheap.
- [ ] `doing` Watch the folder (`notify` crate) and import new runs live. `rustyaim watch` is built but hasn't been tested against a live KovaaK's session yet.
- [x] `done` Tests that use real sample files as fixtures (`crates/rustyaim-core/tests/fixtures`: a clicking run, a tracking run and a 1-second run).

## Epic 2: Sessions and training time (P0)

All in `crates/rustyaim-core/src/training.rs`. Reported by `rustyaim-cli time`.

- [x] `done` Run length = finish time − `Challenge Start`, handling runs that cross midnight. Pause duration is subtracted.
- [ ] `spike` Confirm the units of `Pause Duration`. None of the first 213 runs has a pause, so it's assumed to be seconds, and a pause as long as the run itself is ignored. Pause a run once and check the file.
- [x] `done` Group runs into sessions: a break of more than N minutes (default 15) starts a new one.
- [x] `done` Active time (inside runs) vs. session time (first run to last).
- [x] `done` Training-day boundary (default 4 AM) so late-night sessions count toward the day they started on.
- [x] `done` Streaks (current and best; today stays open until the day ends), daily goal met or not, weekly totals.
- [x] `done` Detect restarted runs: shorter than 50% of that scenario's median length, for scenarios with 3+ runs. 1-second scenarios aren't flagged.
- [x] `done` Averages by day of week (missed days count as 0) and runs by hour of day.
- [ ] `todo` Store sessions in the database once the UI needs them faster. They're cheap to recompute for now.

## Epic 3: Progress tracking (P1)

All in `crates/rustyaim-core/src/progress.rs`. Reported by `rustyaim-cli progress [scenario]` and `rustyaim-cli sens`. Restarted runs stay in the history but never count as bests and are left out of averages.

- [x] `done` Per-scenario history: score, accuracy, average TTK, personal-best flags.
- [x] `done` Improvement measures: first run → best, average, trend (last 5 runs vs. the 5 before).
- [x] `done` Sensitivity history from each run's settings (sens, scale, DPI, FOV), with cm/360. Changes are marked in scenario histories, ready for chart markers.
- [ ] `spike` cm/360 is only verified for the "Overwatch" scale. Valorant/CS/Apex use standard values but KovaaK's names for those scales are unconfirmed.
- [x] `done` Plateau detection: 10+ runs without a new best and a trend within ±3%.
- [x] `done` Cold-start dip: the day's first run vs. the best from earlier days, and vs. the rest of that day's runs (the better warm-up measure).
- [x] `done` Consistency: variation (stdev / mean) of the last 10 runs.
- [x] `done` Fatigue curve: score relative to each scenario's average, in 10-minute buckets into the session. It's a rough signal because it mixes in which scenarios you play when.
- [ ] `todo` Compare scores before and after a sensitivity change per scenario (moved here from Epic 7 now that periods exist).

## Epic 4: Scenario classification and benchmarks (P1)

- [ ] `todo` Tag each scenario's skill from its name (static clicking / dynamic clicking / tracking / switching), with manual overrides saved in the database.
- [ ] `spike` Read the `.sce` files (local and workshop) to tag scenarios from their settings, which is more reliable than names.
- [ ] `todo` Benchmark packs: import scenario lists and rank thresholds (e.g. Voltaic S5 Novice/Intermediate/Advanced) from a file.
- [ ] `todo` Rank per scenario, overall rank, and score needed for the next rank.

## Epic 5: UI screens (P1)

The app imports on startup, watches the stats folder, and refreshes when new runs arrive (`runs-imported` event). Commands: `snapshot`, `scenario`, `rescan`.

- [x] `done` Dashboard: totals, streak banner, 7-day chart against the daily goal, most played, recent sessions, most improved, coach notes. The skill split waits on tags (Epic 4).
- [x] `done` Progress: searchable, sortable scenario list and a detail view with score chart (PBs, 3-run average, day and settings-change markers), runs table and insights.
- [x] `done` Training time: calendar heatmap with month navigation, weekday averages, 7-day session timeline, runs by hour, active vs. idle.
- [ ] `doing` Benchmarks: lists your Voltaic S5 bests for now. Ranks need Epic 4.
- [ ] `doing` Library & import: folder, counts, errors, rescan, rules (read-only), sensitivity history, FPS/resolution. Still to do: editable rules (needs the settings file), a folder picker, and the tags table (Epic 4).
- [ ] `todo` Today's plan screen (Epic 6)
- [x] `done` Coach notes: repeat-before-switching, warm-up, plateau and settings-change notes, from the stats.
- [x] `done` Packaging: portable single exe plus MSI and NSIS installers, built by the release workflow.
- [ ] `todo` Date range control on the dashboard (7 days / 30 days / all time) from the mockup.

## Epic 6: Adaptive daily training plans (P2, headline feature)

Goal: each day the app builds a plan (which scenarios, how many runs, at what difficulty), puts it into KovaaK's as a playlist, then reads the results back and adjusts the next plan.

### 6a. Model your skill level (read only)
- [ ] `todo` Skill estimate per category from recent scores, normalized per scenario (percentile of your own history, or benchmark rank once thresholds are loaded).
- [ ] `todo` Find the weakest category and recent plateaus. These drive what each plan focuses on.
- [ ] `todo` Detect the playlist you're currently running (`PlaylistInProgress.json`) and which day of a program you're on (e.g. *FAR 4 AimGod Day 1–7*).

### 6b. Build the plan inside the app (no game files touched)
- [ ] `todo` Plan inputs: time budget (default 30 min), focus categories, rest days.
- [ ] `todo` Plan shape: warm-up → main block on the weak skill → maintenance on the other skills → optional benchmark run.
- [ ] `todo` 3–5 runs per scenario, chosen from scenarios you've already played.
- [ ] `todo` Mark each plan item done as matching runs arrive from the stats folder.

### 6c. Difficulty ladders using existing scenarios
- [ ] `todo` Ladder format: an ordered list of scenarios for one skill, from easiest to hardest (e.g. `Easy` → `Novice` → `Intermediate`, or `80%` → `90%` → `100%` speed variants).
- [ ] `todo` Build starter ladders from scenario names, plus user-edited ladders.
- [ ] `todo` Step-up rule: move up a rung when your last K runs clear a target (e.g. 3 runs at or above your 80th percentile, or a rank threshold) with accuracy above a floor.
- [ ] `todo` Step-down rule: move down after two sessions below a floor, so a bad day doesn't drop you.
- [ ] `todo` Show why: every change comes with a reason ("Moved to Intermediate: 3 runs above 900 at 88% accuracy").

### 6d. Write plans into KovaaK's as playlists
- [ ] `spike` Does KovaaK's pick up a new playlist file while running, or only after a restart? Does it need `playlistId` and `shareCode`?
- [ ] `todo` Write `RustyAim - <date> - <focus>.json` into the Playlists folder. Only create files with the `RustyAim -` prefix and never edit the user's own playlists.
- [ ] `todo` Back up the Playlists folder before the first write, and clean up old generated plans.

### 6e. Generated difficulty variants (stretch)
- [ ] `spike` Copy a scenario's `.sce` file with `Timescale` (speed), target size or bot speed changed to make finer difficulty steps (e.g. `RustyAim 1w4ts 85%`). Check that KovaaK's loads it and that stats files are written normally.
- [ ] `spike` Downsides: generated scenarios aren't on public leaderboards, and scores aren't comparable to the original scenario. Decide whether to keep a "speed %" field linking each variant back to its original.
- [ ] `todo` If it works: continuous difficulty (e.g. +5% speed per step) instead of fixed rungs.

## Epic E: Built-in aim trainer (P1)

Goal: RustyAim runs its own scenarios instead of relying only on KovaaK's. The engine is Bevy, in its own window with raw mouse input and no frame cap. Scenarios are TOML files with a published schema (SCENARIOS.md), and the app has a form-based editor built from that schema. KovaaK's import keeps working; engine runs go into the same database tagged `source = 'rustyaim'`.

Layout: `rustyaim-sim` (scenario format, field descriptions, simulation; no graphics, fully tested) → `rustyaim-engine` (Bevy window, HUD, saving runs) → the app, which starts the trainer by running its own exe with `--play <id>` so it stays a single file.

### E1. First playable version
- [x] `done` Scenario format: timing, arena, weapon (hitscan or beam; click, hold or always), targets (count, sphere or capsule, size, health, regen, lifetime, shrink), spawn area (distance, yaw, pitch, spacing, max distance from the last kill), movement (static, strafe, random) and scoring. Validation with plain-language messages.
- [x] `done` Field list (`spec.rs`) with labels, help, limits and choices; tests keep it in step with the format.
- [x] `done` Simulation: spawning, angle-space movement with bouncing at the area edge, ray hits on spheres and capsules, 60 Hz beam ticks, kill/TTK records, seeded RNG so a seed replays the same run.
- [x] `done` Engine: full-screen Bevy window, raw mouse look in cm/360 and DPI, FOV in horizontal 16:9 degrees, gridded room, hit flash, HUD (timer, score, accuracy, kills, FPS), countdown, results with personal best, Esc / R / Space.
- [x] `done` Runs saved with KovaaK's field names (score, hits, misses, TTK, sens, FOV, FPS), so progress and training screens include them. A scenario fingerprint keeps easier and harder variants apart.
- [x] `done` Settings start from your latest KovaaK's sensitivity and FOV.
- [x] `done` Play screen: scenario cards with bests, filters, settings, play button. Editor: every field from the spec, live validation, copy-and-edit for built-ins.
- [x] `done` Eight original starter scenarios: Static Grid, Wide Flicks, Micro Flicks, Shrink Reflex, Strafe Clicks, Strafe Track, Float Track, Switch Track.

### E2. Feel and feedback
- [ ] `todo` Hit and kill sounds (generated, no asset files), with volume in settings.
- [ ] `todo` Measure input-to-photon latency and frame pacing; option for a frame cap.
- [ ] `todo` Pause on focus loss, and a pause menu instead of Esc quitting straight away.
- [ ] `todo` Crosshair editor preview in the app; more crosshair options (outline, color on hit).
- [ ] `todo` In-game results: score chart for this scenario and accuracy by target.

### E3. More of what the engine allows
- [ ] `todo` Player movement (WASD strafing) and targets that move toward or away from the player.
- [ ] `todo` Target paths beyond strafe and random: circles, smooth noise, jumps with gravity.
- [ ] `todo` Projectile weapons with travel time; spread and recoil.
- [ ] `todo` Time dilation (play at 80% speed) and a target-scale setting, like KovaaK's difficulty variants.
- [ ] `todo` Visual themes: room colors, target outlines, high-contrast mode.

### E1b. Adaptive training, workouts, display and analytics
- [x] `done` Adaptive mode for every scenario (first version: 0–100 levels and a staircase; replaced below).
- [x] `done` Ratings in real units instead of levels: a Bayesian (Glicko-like) estimate of the knob value you hit 85% of the time (target lifetime in s, tracking speed in °/s, or size), with its uncertainty. It finds you in ~15–25 targets, carries over between runs and widens with days away. Targets come from a mix (near / stretch / easy, by chance of success; Steady, Balanced or Push per profile) with guard rails, breakthroughs for beating your fastest hit, and a speed vs. hit-rate curve in Insights. A kill with more than one miss on it counts as a failure, so spraying doesn't fake a rating.
- [x] `done` Timed scenarios where difficulty is speed: Timed Flicks, Timed Wide Flicks, Timed Micro Flicks, Timed Grid, Timed Strafe Clicks. Every built-in has its own adaptive knobs; user scenarios get sensible defaults.
- [x] `done` Workouts: TOML lists of scenarios with runs and mode, played back to back in one trainer session with a summary at the end. Four built-ins, editor in the app.
- [x] `done` Display: borderless, exclusive (any resolution and refresh rate the monitor offers, e.g. stretched 4:3) or windowed; a centered play area of any size with a helper to match a smaller screen (e.g. 24" on a 32" monitor).
- [x] `done` Telemetry per run: every target (flick distance and direction, reaction, time to reach, overshoot, corrections, outcome, level), every shot (offset in target radii along/across the flick), tracking samples at 30 Hz (on target, error, target speed, lead/lag, time since direction change) and a 240 Hz aim trace.
- [x] `done` Insights screen: level by run and within the last run, speed vs. difficulty (Fitts' law fit and throughput in bits/s), success and time by flick direction and distance, shot placement map (overshoot vs. stopping short, vertical bias), reaction histogram, tracking by target speed and after direction changes (recovery time), lead/lag, success by 10-second segment, and coaching notes generated from all of it.
- [x] `done` Settings tab with profiles (one per game): sensitivity in the game's own units (Overwatch 2, Valorant, CS2, Apex, Call of Duty, or cm/360), DPI, FOV with each game's usual value, crosshair with preview, and display options. Switch profiles from the Play screen; `--profile <name>` in the engine.
- [ ] `todo` More games in the profile list (Fortnite, Rainbow Six Siege, Marvel Rivals, The Finals) once their sensitivity formulas are checked.
- [x] `done` `--smoke` bot that plays like a person (reaction delay, aim error, corrections), for end-to-end checks and preview data.

### E4. Plans with the engine
- [ ] `todo` Daily plan from Insights: pick the scenarios that train your weakest direction, distance or tracking speed, as a generated workout.
- [ ] `todo` Replay a run from its aim trace, with targets, at any speed.
- [ ] `todo` Flick shape analysis from the trace: peak speed, deceleration, and where corrections start.
- [ ] `todo` Compare levels across scenarios on one skill profile (clicking, tracking, switching), and show how fast each is improving.
- [ ] `todo` Per-run adaptive knobs in Insights (e.g. "you're reliable at 0.62 s lifetimes").
- [ ] `todo` List the monitor's real video modes in the app (the engine can read them) instead of free-typed resolutions.

## Epic T: Testing and releases (P1)

The plan is in [TESTING.md](TESTING.md).

- [x] `done` Pre-commit hook (`.githooks/pre-commit`): formatting, clippy with warnings as errors, tests, release build to `dist/RustyAim.exe`.
- [x] `done` CI on every push/PR (`.github/workflows/ci.yml`), with the exe uploaded as an artifact.
- [x] `done` Release workflow on version tags (`.github/workflows/release.yml`): portable exe plus installers. First run published v0.1.0.
- [ ] `todo` Treat files missing `Score:`/`Challenge Start:` as incomplete and retry them (half-written file risk), with a fixture test.
- [ ] `todo` Golden snapshot tests of the reports with `insta`.
- [ ] `todo` Ignored real-data test over the local stats folder (`cargo test -- --ignored`).
- [ ] `todo` Property tests for the parser and session grouping (`proptest`).
- [ ] `todo` Backend tests for the Tauri commands against an in-memory fixture database.
- [ ] `todo` Playwright smoke test of every screen on CI, using a fixture-only snapshot (needs a `--db` option on the app).
- [ ] `todo` Daylight-saving and very-long-run handling: cap run length and add fixtures that span a DST change.
- [ ] `todo` Import timing test with 10k generated files.

## Epic 7: Later (P3)

- [ ] `todo` Session notes: sleep, energy, mood, hardware changes, linked to sessions so you can see how they relate to scores.
- [ ] `todo` Export to CSV and database backup/restore.
- [ ] `todo` Auto-update, and code signing so SmartScreen stops warning.
- [ ] `todo` Import older history from other trackers' exports, if needed.
- [ ] `spike` Use the per-kill data (TTK spread, overshots) for richer insights, e.g. "you overflick on small targets".
