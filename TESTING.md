# Testing strategy

AimAdapt turns files it doesn't control (KovaaK's stats CSVs) into numbers people use to judge their own progress. Two kinds of failure matter most:

1. **Wrong numbers that look right.** A miscounted session or a mis-parsed score silently misleads you. This is the biggest risk.
2. **Breaking on new input.** A game update changes the file format, or a file is caught half-written.

UI glitches matter less: they're visible, and easy to spot and fix.

So most testing effort goes into `aimadapt-core`, where all parsing and maths lives, and the UI is kept thin.

The built-in trainer follows the same pattern. All game rules live in `aimadapt-sim`, which has no graphics and is tested directly. `aimadapt-engine` only feeds it mouse input and draws what it reports.

## What runs when

| When | What | How |
|---|---|---|
| Every commit (pre-commit hook) | Formatting, clippy (warnings are errors), all tests, release build to `dist/AimAdapt.exe` | `.githooks/pre-commit`. Enable once per clone with `git config core.hooksPath .githooks` |
| Every push / PR | The same checks on a clean Windows machine, plus the exe uploaded as a build artifact | `.github/workflows/ci.yml` |
| Version tag (`v0.1.0`) | Windows installers (MSI + NSIS) attached to a GitHub release | `.github/workflows/release.yml` |
| Before a release | Real-data check and manual smoke test (below) | By hand |

## Test layers

### 1. Unit tests (in each module, `#[cfg(test)]`)

These cover pure logic with small, hand-built inputs, one behaviour per test. Today there are 17:

- **parse:** file names with ` - ` in the scenario, rejecting other files, runs crossing midnight, unknown and bad fields.
- **training:** session splitting, the 4 AM day boundary, per-scenario restart detection, streaks and goals, weekday averages, pause subtraction.
- **progress:** cm/360, improvement and best-score counting, plateaus, cold starts, restarted runs excluded, fatigue buckets.
- **discover:** Steam `libraryfolders.vdf` parsing.

**Rule:** every bug fix starts with a failing unit test.

### 1b. Trainer rules (`aimadapt-sim`)

These are 78 tests over the scenario format, simulation, adaptive ratings, telemetry, workouts, programs, settings and profiles. Each one steps the simulation at 240 Hz with scripted aim, so no window is needed:

- **Format:** an empty file gives the defaults, TOML round-trips, unknown fields are rejected, bad values get plain messages, and the fingerprint ignores the description but not the difficulty.
- **Field list:** every scenario field is described exactly once, every choice parses, every default is within its limits, and SCENARIOS.md mentions every field.
- **Simulation:** sphere and capsule hits, countdown blocks shots, kills respawn in the same frame, misses, fire rate on hold, time and kill limits, 60 Hz beam accuracy, moving targets staying inside their area at a fixed distance, lifetime timeouts and shrinking, spawn spacing, time to kill not counting time spent on other targets, and the same seed replaying the same run. Every built-in scenario, in both modes, is played for 60 s and checked every frame for targets leaving the room.
- **Library:** built-ins are valid, and user scenarios save, load, rename on clash, delete, and reject `../` ids.
- **Adaptive ratings:** simulated players with a known 85% speed (0.35–1.6 s) are found within 25% after 25 targets and within 10% after 100, even from a wrong first guess. Each mix keeps success in its band: Steady 82–92%, Balanced 78–90%, Push 72–86%. A surprising result moves the rating more than an expected one: hitting a stretch target vs. one at your level, and missing one at your level vs. a stretch. Time away widens the uncertainty. In the simulation, perfect play gets faster and idling slower. Spraying clicks doesn't count as success, and tracking rates target speed with one outcome per 2 s segment.
- **Runs, not targets:** with no earlier rating a run is a calibration run; with one, every target (or tracking segment) in the run gets the same value, and the rating still takes the run in. Run picks follow the mix (Balanced within 4 points of 65/20/15), stretch runs are harder and easy runs easier, and there's no stretch run after a stretch run or a run more than 10 points below the goal. A breakthrough needs the goal held for at least 10 outcomes at a value past your best, and is announced on the run's last step.
- **Movement:** with acceleration, velocity never changes faster than the limit allows and strafes slow through each reversal; forward/back movement stays inside the spawn distance range and keeps sideways speed in m/s; jumps reach their height with the set airtime; regen waits for its delay; and new fields at their "off" value leave existing scenarios' fingerprints (and ratings) unchanged.
- **Regen smoothness:** on a target needing 1.5 s of beam with fast regen, steady tracking finishes every target in time and a player who slips off for 0.3 s every 1.2 s finishes none.
- **Variants:** every sub-skill has at least two built-ins; each workout variant is a real built-in with the same sub-skill as the scenario it stands in for; items alternate by week with the same number of reps; weak-spots days rotate too.
- **Hitscan target speed:** each target gets its own speed and keeps it through reversals, perfect play raises the rating and idling lowers it, and validation requires a lifetime (for target size too).
- **Knobs that fit the drill:** Static Grid, Timed Grid and Wide Flicks are rated on kill pace (a clicker inside the pace succeeds every time, one at 1.7× never does; Timed Grid keeps its 2 s timers), and a kill after two misses doesn't count while one miss is allowed. Micro Flicks and Precise Track are rated on size: every target in a run has the run's size, Micro Flicks keeps its 1.5 s lifetime and Precise Track its slow speed, quick hits succeed and timeouts fail, and calibration shrinks the ball for a steady tracker. Strafe Clicks is rated on target speed with a fixed lifetime.
- **Curve movement:** on Control Track the heading never turns faster than the turn rate between frames (bounces aside), speed stays within 8%, and the target turns through more than a full circle in 20 s. Curve movement without a turn rate is rejected.
- **Switching:** Speed Switch has no target timers and is rated on kill pace: kills comfortably inside the pace all count, a player at 1.7× the pace gets one failure per empty window and none of the late kills, and calibration finds a 0.7 s player. Stability Switch sweeps at least 15° per direction on targets at most 3.5° across, and counts only engaged time, so a player who looks away 0.6 s after every kill still scores above 97% (below 85% with the old scoring), and a half-damaged target that's abandoned stays engaged for exactly its heal time.
- **Telemetry:** reaction, time to reach, overshoot and corrections for a scripted flick. Shots past the target read as positive "along", and aiming behind a strafing target reads as negative lead.
- **Workouts and settings:** built-in workouts only use real scenarios, steps expand in order, display settings round-trip, and the play area is centered and clamped, and can sit at the top, bottom or any position (out-of-range positions are rejected). A 24" area on a 42" 4K screen comes out at 2194x1234.
- **Profiles:** game sensitivities convert to the known cm/360 (Overwatch 5.3 at 400 DPI is 65.35, Valorant 0.35 at 800 is 46.65, CS2 1.0 at 800 is 51.95). Profiles round-trip, bad ones are explained, and old single-settings files become a Default profile. The engine tests saving profiles and loading the active one or one by name.
- **Programs:** every Core Week day is 25–33 minutes, with at most 6 drills of 3+ reps (warm-ups aside). Sunday is rest, weak-spots days put the weakest skills first, and every sub-skill has a drill tagged with it. The engine resolves each day and builds Saturday from an empty profile. In `aimadapt-core`, the skill profile counts faster lifetimes and higher speeds as gains. The week grid marks days done, today, upcoming and rest, and compares with last week; rep and carry-over gains, the streak, and training days starting at 4 AM are checked on hand-built data.
- **Insights** (`aimadapt-core`): synthetic runs with a known weak direction, overshooting, low shots, tracking lag and a 200 ms recovery after direction changes produce those exact figures and coaching notes. Centering is the median distance from center over on-target samples only. On a lifetime rating that went from 0.80 to 0.62 s with a 0.22 s median reaction, the reaction note says the part you control is 0.40 s, down 31% against the rating's 23%.
- **Insights for switching:** acquisition (kill to reaching the next target), on-target share while engaged and kills a minute come out exactly on a synthetic run, and old single-target "fastest hit" values aren't used as the best held.
- **HUD wording** (`aimadapt-engine`): size ratings on tracking show time on target, on clicking a hit rate.
- **Mouse lock** (`aimadapt-engine`): the cursor locks on start, releases when focus moves away, and locks again when focus comes back, on every click, and within half a second while focused, even when the game thinks it's already locked (after a quick alt-tab Windows drops the lock without the game noticing).
- **Strafe width:** drills with gradual turns (Hop Strafe Track, Unbroken Strafes, Hop Clicks) sweep at least as wide per strafe as their plain versions (Strafe Track, Timed Strafe Clicks), and Hop Switch at least 12°.
- **Bars and colors** (`aimadapt-engine`): bars start above the top of a capsule, not inside it; timed targets get a time-left bar that empties, untimed ones none; timed targets change color by wave (Timed Grid's first three share a color, the next three take the other) and untimed ones keep one; switching targets show health.
- **User guide** (`aimadapt-app`): the guide names this version, has a What's new section and a section for every page and sidebar entry, every internal link goes to a section, and it has a row for every built-in scenario, every rating and every trainer key, and an entry for every built-in workout.
- **UI start-up** (`aimadapt-app`): app.js waits for DOMContentLoaded before its first render, so views from later scripts are defined (opening on #help used to stay on "Loading").
- **KovaaK's files on Browse** (`aimadapt-app`): `sce_entries` lists each name once (the first file wins), reads tags and aim type, and marks which files the trainer can play, on the made-up fixtures.
- **Test databases only** (`aimadapt-app`, `aimadapt-engine`): `--dump-snapshot` and `--smoke` refuse to run without `--db`.
- **Ratings per scenario version** (`aimadapt-engine`): a rating carries over to the same scenario and to one with a new description, but not to one with different settings.
- **Engine record** (`aimadapt-engine`): an engine run saved through `insert_runs` gets the right score, accuracy, length and `source`, keeps separate bests for variants, and is ignored when matching KovaaK's settings.

**Engine record** also has an end-to-end test: two adaptive runs and a standard one are saved through `record::save`. The first adaptive run must be a calibration run that sets no best. The second, a day later, must start from the first's rating with the day counted, play at one value, and store its value and pick for the next run's guard rails, the names must keep both modes apart, telemetry, traces and replay frames must be stored, a replay must load with frames, trace and shots, and Insights must read all three back.

**Engine smoke test:** `aimadapt-engine <scenario> --smoke <folder> --db <file>` (or `--workout <id>`) has a bot that plays like a person go through short runs, then saves screenshots mid-run and on each results screen. It needs a GPU, so it's run by hand, not on CI. Check the screenshots and that the runs, with telemetry, are in the test database. Opening the app with `--db <that file>` previews Insights on bot data.

### 2. Fixture tests (`crates/aimadapt-core/tests/`)

These run the real pipeline (file → parse → SQLite → reports) on real KovaaK's files in `tests/fixtures/`. Today there are 7 tests, using 3 fixtures: a clicking run, a tracking run and a 1-second run.

**Fixture policy:** whenever a file breaks parsing or gives odd numbers, copy it into `tests/fixtures` and add a test before fixing it. Files still needed:

- [ ] A run with a pause (also settles the units of `Pause Duration`)
- [ ] A run that crosses midnight
- [ ] A run from an older game version, if one turns up
- [ ] A scenario name with a comma or non-ASCII characters
- [ ] A different sensitivity scale (Valorant/CS), to confirm KovaaK's scale names for cm/360
- [ ] A half-written or truncated file (see Known risks)

### 2b. Scenario file tests (`crates/aimadapt-sce`)

The `.sce` reader and converter have 32 unit tests and 6 fixture tests:
- **Settings:** sections and the map split, a byte-order mark, keys in any case (last one wins), positional lists with empty slots, and `X= Y= Z=` vectors.
- **Maps:** Reflex brushes and spawns (`teamA` and `TeamA`), faces that point at missing vertices, and JSON primitives, procedural triangles, spawn points and volumes, waypoints and props. Errors name the line or object.
- **References:** bots through `.bot` and `.rot` (with weights), characters, dodge profiles and abilities, and each missing name listed once.
- **Inventory:** what counts as covered by the plan and what's a gap.
- **Converter:** Unreal axes and units to the sim's, team masks, colors, `Cube` brushes (100 units per scale step from a corner) and rotations. Bots on a spawn volume appear anywhere in its box, which is placed like a `Cube` (regression test: they all appeared at its corner, on top of each other). Regression tests: bots that fly or jump count as moving (SYV played as a still ball), an inverted bot spawn radius isn't playable, and health regen, the player's spawn rules and `Cube` walls come through to the world. The Still Wall fixture imports with KovaaK's rules (spawn points, offsets, blocked radius, accuracy multiplier, FOV lock) and plays in the sim: three bots on the wall, and a hit then a miss scores 10 × √0.5. Files the trainer can't play yet return their reasons.

`aimadapt-sim` tests a world's spawning (weighted points, offsets along each point's own axes and in either order, as ww5t writes them, kept apart; pushed out of walls but not out of a room around the player; the player's view rule, inverted or not, giving way to keeping targets apart when crowded) and the accuracy multipliers, and that hitscan targets heal after the regen delay (regression test: regen worked only with a beam). `aimadapt-engine` has a regression test that imported runs stay out of KovaaK's history (Progress and benchmarks) while built-in standard runs don't. The smoke test plays a file too: `aimadapt-engine --sce "<file>.sce" --smoke <folder> --db <file>`.

The fixtures in `tests/fixtures/` are made up for the tests: one per map format, and Still Wall, which the trainer can play. Unlike the stats fixtures they're never copies of real files, because a `.sce` file is someone's scenario design. Copy the quirk into a made-up file instead. They keep the quirks real files have: CRLF, a byte-order mark, comments, stray lines, extensions on references, cosmetic map objects.

Regression tests from the first run over 225 real files:
- ability references carry a kind extension (`Approach.abilmov`);
- Reflex `Target` entities are only the end-of-game camera's look-at point;
- some Workshop files sit in a binary container (magic number, version, length, text, trailer).

**Real-data check:** `cargo test -p aimadapt-sce -- --ignored` reads every `.sce` file in your KovaaK's and Workshop folders and fails on any map that doesn't parse or file without a name. `aimadapt-cli sce --summary` gives the inventory totals.

### 3. Golden snapshot tests (planned)

Serialize the reports (`TrainingReport`, `ScenarioStats` and the rest) for the fixture set to JSON and compare them with a saved copy, using the `insta` crate. Any change to a metric then shows up as a diff to accept or reject. This catches "I changed the trend window and the dashboard numbers moved" without writing an assertion for every field.

### 4. Robustness tests (planned)

- **Property tests** (`proptest`): random and mangled CSV text never panics the parser; random run timings always give sessions that don't overlap, with active time no larger than session time.
- **Fuzzing** (`cargo-fuzz`, later): the parser over a corpus seeded from the fixtures.

### 5. Database tests

Already covered: a new database migrates to the current schema (version 7, which adds replay frames), and reopening keeps it.

**Rule:** each new migration gets a test that opens a database from the previous version (saved as a fixture) and checks the data survived.

### 6. Real-data check (planned, `#[ignore]`d)

An ignored test that imports your own KovaaK's stats folder into a temporary database and asserts:
- there are zero parse errors
- every run has a score and a run length
- no session overlaps another

Run it before releases with `cargo test -- --ignored`. It catches format changes after game updates, using the largest and most varied dataset available. It stays ignored on CI because CI has no KovaaK's install.

### 7. App backend tests (planned)

The Tauri commands (`snapshot`, `scenario`, `rescan`) build their payloads from an in-memory database with fixture runs, with no window involved. They check that the JSON has the fields the UI reads. This guards the contract between Rust and `app.js`.

### 8. UI tests

- **Now:** browser preview against a data dump (`--dump-snapshot`, see README). Every screen must render with no console errors: Today, Scenarios (a trainer scenario, a workout and a KovaaK's scenario), each tab of Progress and Settings, both editors, Replay and Help. Old addresses (`#dashboard`, `#play`, `#program`, `#insights/<name>`, `#progress/<name>`, `#training`, `#benchmarks`, `#library`) must land on their new page.
- **Static checks** in `crates/aimadapt-app/tests/ui.rs` read the JS as text, for bugs a browser can't catch in CI yet: the first render waits for every script, old page addresses still redirect, chart ticks are formatted from their exact value, target speeds show one decimal, and Favorite and Hide act on the item they name (the lists and each item have their own addresses). `tests/guide.rs` keeps the user guide in step with the pages.
- **Screenshots** in the README come from a bot-played demo database (with the KovaaK's fixtures imported) spread over a week, taken with headless Edge against the preview.
- **Planned:** a Playwright smoke test on CI. It loads each screen against a snapshot built from the fixture database, not personal data, and checks key numbers and zero console errors. GitHub runners have Node, so this doesn't add Node to local development.
- **Later:** full app tests through `tauri-driver` (WebDriver), if the UI grows enough to need them.

### Manual smoke test (before a release)

1. Install from the release installer on a clean user profile.
2. Launch the app. It should find the stats folder and import existing runs.
3. Play one scenario. The run should appear without restarting the app.
4. Click each screen and check that today, the streak and the latest run look right.
5. Click Rescan. The counts should not change.
6. On Play, start Static Grid and one tracking scenario. Check that mouse sensitivity matches KovaaK's (a full turn should take the same distance), the FPS is well above your monitor's refresh rate, and the run appears on Progress after the trainer closes.

## Known risks to test for

- **Half-written files.** If the watcher reads a file while KovaaK's is still writing it, the file could parse with missing summary fields and be stored for good. Planned fix and test: treat a file without `Score:` and `Challenge Start:` as incomplete, and retry it on the next scan instead of storing it.
- **Daylight saving and time zones.** KovaaK's writes local times with no zone. A run spanning a clock change (e.g. 2 AM on Nov 1, 2026 in the US) gets a length that's off by an hour. Travelling also shifts days. Planned tests: fixtures spanning a DST change, with run length capped at a sane maximum.
- **Format changes after game updates.** The real-data check (layer 6) and tolerant parsing cover this, and new fixtures get added per the fixture policy.
- **Large histories.** Years of training means 10k+ files. Planned: a timing test that imports 10k generated files and builds the snapshot, with a budget (e.g. under 5 s) to catch accidental slow paths.
- **Rust ↔ UI drift.** A renamed field breaks a screen silently. Layers 7 and 8 cover this.

## Coverage

Use `cargo llvm-cov --workspace` from time to time to find untested branches in `aimadapt-core`. There's no hard percentage target; the aim is that every metric shown in the UI is covered by a unit or golden test.
