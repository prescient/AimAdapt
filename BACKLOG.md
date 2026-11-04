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

- [ ] `todo` **Pick the UI stack.** Recommendation: **Tauri** (Rust backend, web frontend), so the HTML/CSS mockup carries straight over. Alternative: `egui`, which is pure Rust and simpler to ship but means rebuilding the UI.
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

- [ ] `todo` Per-scenario history: score, accuracy, average TTK, personal-best flags.
- [ ] `todo` Improvement measures: first run → best, rolling average, recent trend.
- [ ] `todo` Sensitivity history built from the settings in each run, with markers on charts.
- [ ] `todo` Plateau detection: N runs without a new best and a flat rolling average.
- [ ] `todo` Cold-start dip: first run of the day vs. previous best.
- [ ] `todo` Consistency score: how much scores vary from run to run.
- [ ] `todo` Fatigue curve: score vs. minutes into the session.

## Epic 4: Scenario classification and benchmarks (P1)

- [ ] `todo` Tag each scenario's skill from its name (static clicking / dynamic clicking / tracking / switching), with manual overrides saved in the database.
- [ ] `spike` Read the `.sce` files (local and workshop) to tag scenarios from their settings, which is more reliable than names.
- [ ] `todo` Benchmark packs: import scenario lists and rank thresholds (e.g. Voltaic S5 Novice/Intermediate/Advanced) from a file.
- [ ] `todo` Rank per scenario, overall rank, and score needed for the next rank.

## Epic 5: UI screens (P1)

- [ ] `todo` Dashboard
- [ ] `todo` Progress (scenario list + detail chart)
- [ ] `todo` Training time (calendar, weekday averages, session timeline, runs by hour, active vs. idle)
- [ ] `todo` Benchmarks
- [ ] `todo` Library & import (folder, rules, sensitivity history, tags)
- [ ] `todo` Today's plan screen (Epic 6)
- [ ] `todo` Coach notes: rule-based insights generated from the stats

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

## Epic 7: Later (P3)

- [ ] `todo` Session notes: sleep, energy, mood, hardware changes, linked to sessions so you can see how they relate to scores.
- [ ] `todo` Before/after sensitivity comparison per scenario.
- [ ] `todo` Export to CSV and database backup/restore.
- [ ] `todo` Windows installer and auto-update.
- [ ] `todo` Import older history from other trackers' exports, if needed.
- [ ] `spike` Use the per-kill data (TTK spread, overshots) for richer insights, e.g. "you overflick on small targets".
