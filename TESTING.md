# Testing strategy

RustyAim turns files it doesn't control (KovaaK's stats CSVs) into numbers people use to judge their own progress. Two kinds of failure matter most:

1. **Wrong numbers that look right.** A miscounted session or a mis-parsed score silently misleads you. This is the biggest risk.
2. **Breaking on new input.** A game update changes the file format, or a file is caught half-written.

UI glitches matter less: they're visible, and easy to spot and fix.

So most testing effort goes into `rustyaim-core`, where all parsing and maths lives, and the UI is kept thin.

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

Already covered: a new database migrates to the current schema, and reopening keeps it.

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

## Known risks to test for

- **Half-written files.** If the watcher reads a file while KovaaK's is still writing it, the file could parse with missing summary fields and be stored for good. Planned fix and test: treat a file without `Score:` and `Challenge Start:` as incomplete, and retry it on the next scan instead of storing it.
- **Daylight saving and time zones.** KovaaK's writes local times with no zone. A run spanning a clock change (e.g. 2 AM on Nov 1, 2026 in the US) gets a length that's off by an hour. Travelling also shifts days. Planned tests: fixtures spanning a DST change, with run length capped at a sane maximum.
- **Format changes after game updates.** The real-data check (layer 6) and tolerant parsing cover this, and new fixtures get added per the fixture policy.
- **Large histories.** Years of training means 10k+ files. Planned: a timing test that imports 10k generated files and builds the snapshot, with a budget (e.g. under 5 s) to catch accidental slow paths.
- **Rust ↔ UI drift.** A renamed field breaks a screen silently. Layers 7 and 8 cover this.

## Coverage

Use `cargo llvm-cov --workspace` from time to time to find untested branches in `rustyaim-core`. There's no hard percentage target; the aim is that every metric shown in the UI is covered by a unit or golden test.
