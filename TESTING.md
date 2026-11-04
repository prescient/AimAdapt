# Testing strategy

RustyAim turns files it doesn't control (KovaaK's stats CSVs) into numbers people use to judge their own progress. Two kinds of failure matter most:

1. **Wrong numbers that look right.** A miscounted session or a mis-parsed score silently misleads you. This is the biggest risk.
2. **Breaking on new input.** A game update changes the file format, or a file is caught half-written.

UI glitches matter less: they're visible, and easy to spot and fix.

So most testing effort goes into `rustyaim-core`, where all parsing and maths lives, and the UI is kept thin.

The built-in trainer follows the same pattern. All game rules live in `rustyaim-sim`, which has no graphics and is tested directly. `rustyaim-engine` only feeds it mouse input and draws what it reports.

## What runs when

| When | What | How |
|---|---|---|
| Every commit (pre-commit hook) | Formatting, clippy (warnings are errors), all tests, release build to `dist/RustyAim.exe` | `.githooks/pre-commit`. Enable once per clone with `git config core.hooksPath .githooks` |
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

### 1b. Trainer rules (`rustyaim-sim`)

These are 54 tests over the scenario format, simulation, adaptive ratings, telemetry, workouts, programs, settings and profiles. Each one steps the simulation at 240 Hz with scripted aim, so no window is needed:

- **Format:** an empty file gives the defaults, TOML round-trips, unknown fields are rejected, bad values get plain messages, and the fingerprint ignores the description but not the difficulty.
- **Field list:** every scenario field is described exactly once, every choice parses, every default is within its limits, and SCENARIOS.md mentions every field.
- **Simulation:** sphere and capsule hits, countdown blocks shots, kills respawn in the same frame, misses, fire rate on hold, time and kill limits, 60 Hz beam accuracy, moving targets staying inside their area at a fixed distance, lifetime timeouts and shrinking, spawn spacing, and the same seed replaying the same run.
- **Library:** built-ins are valid, and user scenarios save, load, rename on clash, delete, and reject `../` ids.
- **Adaptive ratings:** simulated players with a known 85% speed (0.35–1.6 s) are found within 25% after 25 targets and within 10% after 100, even from a wrong first guess. Each mix keeps success in its band: Steady 82–92%, Balanced 78–90%, Push 72–86%. A surprising result moves the rating more than an expected one: hitting a stretch target vs. one at your level, and missing one at your level vs. a stretch. Breakthroughs only fire past your previous best, and time away widens the uncertainty. In the simulation, perfect play gets faster and idling slower. Spraying clicks doesn't count as success, and tracking rates target speed with one outcome per 2 s segment.
- **Telemetry:** reaction, time to reach, overshoot and corrections for a scripted flick. Shots past the target read as positive "along", and aiming behind a strafing target reads as negative lead.
- **Workouts and settings:** built-in workouts only use real scenarios, steps expand in order, display settings round-trip, and the play area is centered and clamped. A 24" area on a 42" 4K screen comes out at 2194x1234.
- **Profiles:** game sensitivities convert to the known cm/360 (Overwatch 5.3 at 400 DPI is 65.35, Valorant 0.35 at 800 is 46.65, CS2 1.0 at 800 is 51.95). Profiles round-trip, bad ones are explained, and old single-settings files become a Default profile. The engine tests saving profiles and loading the active one or one by name.
- **Programs:** every Core Week day is 25–33 minutes, with at most 6 drills of 3+ reps (warm-ups aside). Sunday is rest, weak-spots days put the weakest skills first, and every sub-skill has a drill tagged with it. The engine resolves each day and builds Saturday from an empty profile. In `rustyaim-core`, the skill profile counts faster lifetimes and higher speeds as gains. The week grid marks days done, today, upcoming and rest, and compares with last week; rep and carry-over gains, the streak, and training days starting at 4 AM are checked on hand-built data.
- **Insights** (`rustyaim-core`): synthetic runs with a known weak direction, overshooting, low shots, tracking lag and a 200 ms recovery after direction changes produce those exact figures and coaching notes.
- **Engine record** (`rustyaim-engine`): an engine run saved through `insert_runs` gets the right score, accuracy, length and `source`, keeps separate bests for variants, and is ignored when matching KovaaK's settings.

**Engine record** also has an end-to-end test: two adaptive runs and a standard one are saved through `record::save`. The second adaptive run, a day later, must start from the first's rating with the day counted, the names must keep both modes apart, telemetry and traces must be stored, and Insights must read all three back.

**Engine smoke test:** `rustyaim-engine <scenario> --smoke <folder> --db <file>` (or `--workout <id>`) has a bot that plays like a person go through short runs, then saves screenshots mid-run and on each results screen. It needs a GPU, so it's run by hand, not on CI. Check the screenshots and that the runs, with telemetry, are in the test database. Opening the app with `--db <that file>` previews Insights on bot data.

### 2. Fixture tests (`crates/rustyaim-core/tests/`)

These run the real pipeline (file → parse → SQLite → reports) on real KovaaK's files in `tests/fixtures/`. Today there are 7 tests, using 3 fixtures: a clicking run, a tracking run and a 1-second run.

**Fixture policy:** whenever a file breaks parsing or gives odd numbers, copy it into `tests/fixtures` and add a test before fixing it. Files still needed:

- [ ] A run with a pause (also settles the units of `Pause Duration`)
- [ ] A run that crosses midnight
- [ ] A run from an older game version, if one turns up
- [ ] A scenario name with a comma or non-ASCII characters
- [ ] A different sensitivity scale (Valorant/CS), to confirm KovaaK's scale names for cm/360
- [ ] A half-written or truncated file (see Known risks)

### 3. Golden snapshot tests (planned)

Serialize the reports (`TrainingReport`, `ScenarioStats` and the rest) for the fixture set to JSON and compare them with a saved copy, using the `insta` crate. Any change to a metric then shows up as a diff to accept or reject. This catches "I changed the trend window and the dashboard numbers moved" without writing an assertion for every field.

### 4. Robustness tests (planned)

- **Property tests** (`proptest`): random and mangled CSV text never panics the parser; random run timings always give sessions that don't overlap, with active time no larger than session time.
- **Fuzzing** (`cargo-fuzz`, later): the parser over a corpus seeded from the fixtures.

### 5. Database tests

Already covered: a new database migrates to the current schema (version 2 adds `runs.source`), and reopening keeps it.

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

- **Now:** browser preview against a data dump (`--dump-snapshot`, see README). Every screen must render with no console errors.
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

Use `cargo llvm-cov --workspace` from time to time to find untested branches in `rustyaim-core`. There's no hard percentage target; the aim is that every metric shown in the UI is covered by a unit or golden test.
