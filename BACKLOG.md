# RustyAim Backlog

RustyAim is an aim trainer for Windows. It rates you in real units, keeps each drill at the difficulty you hit about 85% of the time, runs a weekly training program, and records every target and shot for analytics. Importing KovaaK's stats files is a secondary feature.

**Priority:** P1 = next up · P2 = planned · P3 = later or optional

**Status:** `todo` · `doing` · `done` · `spike` (research before committing)

Last reviewed: Nov 4, 2026 (after the first playtest session).

---

## 1. Trainer engine

`crates/rustyaim-engine` (Bevy window) on top of `crates/rustyaim-sim` (rules, no graphics).

### Done
- [x] Full-screen window with raw mouse input and no frame cap. Sensitivity in cm/360 and DPI; FOV horizontal at 16:9.
- [x] Display modes: borderless; exclusive at any resolution and refresh rate the monitor offers (e.g. stretched 4:3); windowed. Optional centered play area, with a helper that sizes it from screen diagonals.
- [x] Gridded room, sphere and capsule targets, hit flash, and a HUD showing timer, score or rating, hit rate, kills and FPS.
- [x] Countdown, results screen, and Esc, R and Space controls.
- [x] Simulation: angle-space movement with bounces at the spawn edge, ray hits, 60 Hz beam ticks, and a seeded RNG, so a seed replays the same run.
- [x] Workouts and program days play back to back in one session, with a summary at the end.
- [x] Runs saved to the shared database with KovaaK's field names plus `source = 'rustyaim'`.
- [x] The app's exe runs the trainer with `--play`, `--workout` or `--program`, so the release is a single file.
- [x] `--smoke` bot that plays like a person, for end-to-end checks and demo data.
- [x] Moving targets bounce off the room's floor, ceiling and walls, not just their spawn area (Float Track sank through the floor). From [playtesting](PLAYTEST.md).
- [x] Time-left bars on timed targets, and timed targets change color by wave (orange-red, then blue, by default), so older and newer targets are easy to tell apart. Health bars for switching targets that heal. All scenario options. From [playtesting](PLAYTEST.md).
- [x] `--dump-snapshot` and `--smoke` only run against a test database (`--db`).
- [x] The game window opens over the app window (centered across it, tops level), so it lands where you keep the app.
- [x] Movement like players move, from the FAR comparison: acceleration (slow down and speed back up at turns, curved random turns), movement toward and away from the player, and jumps with gravity.
- [x] Regen with a delay, and "finished in time" ratings for regen targets, so smoothness drills reward unbroken tracking. Insights shows unbroken stretches on target.
- [x] At least two scenarios per sub-skill (29 built-ins), with Core Week alternating each drill and its variant week by week.

### To do
- [ ] `todo` P1 Pause menu and pausing on focus loss (Esc quits straight away today).
- [ ] `todo` P1 Hit and kill sounds (generated, no asset files), with volume in settings.
- [ ] `todo` P2 Measure input-to-photon latency and frame pacing; optional frame cap.
- [ ] `todo` P2 List the monitor's real video modes in Settings instead of free-typed resolutions (the engine can already read them).
- [ ] `todo` P2 More crosshair options: outline, and a color change on hit.
- [ ] `todo` P2 In-game results: a small chart of recent ratings for the scenario.
- [ ] `todo` P3 Game font with a degree sign; speeds read "deg/s" in game for now.

## 2. Scenarios

### Done
- [x] TOML format covering timing, arena, weapon (hitscan or beam; click, hold or always), targets (count, shape, size, health, regen, lifetime, shrink), spawn area, movement (static, strafe, random), scoring, adaptive settings and sub-skill. Validation with plain-language messages. Documented in [SCENARIOS.md](SCENARIOS.md).
- [x] Field list (`spec.rs`) that drives the editor. Tests keep it in step with the format and the docs.
- [x] Editor in the app with live validation; built-ins are copied before editing.
- [x] 19 built-in scenarios tagged with nine sub-skills:
  - **Clicking:** static, dynamic, linear.
  - **Tracking:** precise, reactive, control.
  - **Switching:** speed, evasive, stability.

### To do
- [ ] `todo` P3 Noise-based drift for control tracking (curve movement and acceleration already make smooth paths).
- [ ] `todo` P3 Player movement (WASD strafing).
- [ ] `todo` P3 Projectile weapons with travel time; spread and recoil.
- [ ] `todo` P3 Visual themes: room colors, target outlines, high contrast.

## 3. Adaptive rating

### Done
- [x] Each scenario rates one knob in real units: target lifetime (s), tracking speed (°/s), size (m) or kill pace (s between kills).
- [x] A Bayesian, Glicko-like estimate of the 85% point and how steeply success falls off.
  - It carries over between runs of the same version of a scenario, and its uncertainty widens with days away. Changing a scenario's settings starts it over with a calibration run.
- [x] Adapt between runs, not during them (from [playtesting](PLAYTEST.md): changing difficulty target by target felt chaotic).
  - The first adaptive run on a scenario is a calibration run that adapts as you play.
  - Every later run plays at one value, shown up front ("This run: 0.62 s"), picked from the mix: at your level (80–90% chance), stretch (55–72%) and easy (93–97%).
  - Steady, Balanced and Push mixes are set per profile.
  - Guard rails: no stretch run after a stretch run, or after a run more than 10 points below the goal.
  - A breakthrough is holding the goal for a whole run at a value past your best, announced at the end of the run.
- [x] Clicking success needs a kill before the timeout with at most one miss on it. Tracking is scored per 2-second segment by the share of time on target.
- [x] Switching is rated on what it trains (from [playtesting](PLAYTEST.md): both ratings were mostly target acquisition time).
  - Speed Switch: kill pace. A kill within the pace of the one before is a success; each empty window is a failure. No per-target timers.
  - Stability Switch and Switch Track: target speed, counting only time while a target is engaged (damaged and not yet healed). Their targets heal when you leave them and show health bars.
  - The HUD and results show kills a minute; Insights splits acquisition time from time on target while killing.
- [x] Every knob matches what its scenario trains (from a scoring review of every built-in).
  - Kill pace for Static Grid, Timed Grid and Wide Flicks, so untimed drills aren't copies of their timed versions and grid ratings don't measure the queue. With a hitscan weapon, a kill after a spray of misses isn't a success.
  - Target size for Micro Flicks (fixed 1.5 s lifetime) and Precise Track (fixed slow speed), so the rating is precision rather than reaction time or speed.
  - Target speed for Strafe Clicks (fixed 2 s lifetime), so it and Timed Strafe Clicks rate speed and time separately.
  - Control Track moves in real curves (`curve` movement: steady speed, turning heading).
  - Insights adds centering while on target and, on lifetime ratings, the share that's reaction time.
- [x] Replaced the first version, which used 0–100 levels and a staircase.

### To do
- [ ] `spike` P1 Tune against real play. The default easy/hard ranges, first guesses and the "consistent vs. fades gradually" cut-off are educated guesses that have only been checked with the bot.
- [ ] `todo` P2 Show the stretch/near/easy split and breakthroughs per run in Insights.
- [ ] `todo` P2 Offer (not force) another calibration run when a rating has become very uncertain, e.g. after a long break. From [playtesting](PLAYTEST.md).

## 4. Programs and workouts

### Done
- [x] Workouts: TOML lists of scenarios, each with a run count, mode and block. There are built-ins and an editor.
- [x] Programs: weekdays mapped to workouts, or to a weak-spots day built from the skill profile.
- [x] Core Week, Monday to Saturday, about 29 minutes a day:
  - Five drills per day: warm-up, main ×5, secondary ×4, maintenance ×3.
  - Saturday covers your weak spots; Sunday is rest.
- [x] Program page:
  - Week grid that allows catching up a missed day later in the week.
  - Skill profile measured from the end of your first day on each drill.
  - Weekly summary and streak.
  - Rep and carry-over gains for each drill.
- [x] Today card on the Play screen.

### To do
- [ ] `todo` P1 Start Core Week from the real app window and play it by hand for a full week; fix whatever comes up.
- [ ] `todo` P2 Pick the active program, and add more programs (e.g. a clicking-heavy or tracking-heavy week).
- [ ] `todo` P2 Program editor.
- [ ] `todo` P2 Weak-spots day from Insights detail, not just sub-skills: e.g. your weakest flick direction, distance band or tracking speed.
- [ ] `todo` P3 Hide Core Week's day workouts from the general Workouts list, or group them under the program.

## 5. Insights and analytics

### Done
- [x] Telemetry per run:
  - Every target: flick distance and direction, reaction, time to reach, overshoot, corrections, outcome, knob value and pick type.
  - Every shot: offset relative to the target, along and across the flick.
  - Tracking samples at 30 Hz.
  - A 240 Hz aim trace.
- [x] Insights per scenario:
  - Rating by run with its likely range, and the rating during the last run.
  - Speed vs. hit-rate curve with your 95%, 85% and 50% points.
  - Fitts' law flick speed.
  - Hit rate and time by direction and by distance.
  - Shot placement, reaction histogram and 10-second segments.
  - Tracking: time on target by speed, recovery after direction changes, and lead or lag.
  - Coaching notes.
- [x] Skill profile across the nine sub-skills, three groups and overall (on the Program page).

### To do
- [ ] `todo` P2 Replay a run from its aim trace, with targets, at any speed.
- [ ] `todo` P2 Flick shape from the trace: peak speed, deceleration, and where corrections start.
- [ ] `todo` P3 Rate of improvement per sub-skill (gain per week), with a projection.

## 6. Settings and profiles

### Done
- [x] Settings tab with profiles, usually one per game. Each profile holds sensitivity in that game's units (Overwatch 2, Valorant, CS2, Apex, Call of Duty, or cm/360), DPI, FOV with the game's usual value, adaptive mix, crosshair with a preview, and display settings.
- [x] Switch profiles from the Play screen; `--profile <name>` in the engine. The first profile is matched to your latest KovaaK's run.

### To do
- [ ] `todo` P2 More games (Fortnite, Rainbow Six Siege, Marvel Rivals, The Finals) once their sensitivity formulas are checked.
- [ ] `todo` P3 Editable training-time rules (session gap, day start, daily goal), which are fixed defaults today.

## 7. App screens

### Done
- [x] Navigation: Dashboard, Play, Progress, Program, Insights, Training time, Benchmarks, Library & import, Settings.
- [x] Dashboard, Progress, Training time and Benchmarks cover KovaaK's runs and standard trainer runs. Adaptive runs are charted in Insights instead, because their result is a time or speed.

### To do
- [ ] `todo` P1 Rework the Dashboard around the trainer: today's program day, this week, skill profile and recent ratings. It's still KovaaK's-centric.
- [ ] `todo` P2 Date range control on the Dashboard (7 days, 30 days, all time).
- [ ] `todo` P3 Folder picker for the KovaaK's stats folder.

## 8. KovaaK's import (secondary)

`crates/rustyaim-core` and `rustyaim-cli`.

### Done
- [x] Finds the install across Steam libraries and parses stats files (kills, weapons, summary, settings). Parsing is tolerant, duplicates are skipped, and imports are incremental. The app imports on startup and watches the folder.
- [x] Sessions, active time, the 4 AM day boundary, streaks, daily goal, weekday and hour breakdowns, and restarted-run detection.
- [x] Progress: personal bests, improvement, trend, plateaus, cold-start dip, consistency, fatigue and sensitivity history (cm/360).

### To do
- [ ] `spike` P2 Confirm the units of `Pause Duration` (assumed to be seconds) by pausing a run once.
- [ ] `spike` P2 Confirm KovaaK's sensitivity-scale names for Valorant, CS2 and Apex; only Overwatch is verified.
- [ ] `todo` P2 Voltaic benchmark ranks: load rank thresholds for the scenarios you play and show a rank per scenario and overall. The Benchmarks screen lists Voltaic S5 bests without ranks today.
- [ ] `todo` P3 Tag KovaaK's scenarios with the nine sub-skills (from names, or from `.sce` settings) so they can feed the skill profile.
- [ ] `todo` P3 Compare scores before and after a sensitivity change.
- [ ] `todo` P3 Test the live folder watcher during a real KovaaK's session.

### Superseded by the built-in trainer
These were the original plan for adaptive training through KovaaK's. The trainer's adaptive rating and Core Week now cover them, so they're parked:
- Writing generated plans into KovaaK's as `RustyAim -` playlists, and the spike on whether KovaaK's reloads playlist files.
- Difficulty ladders of existing KovaaK's scenarios, with step-up and step-down rules.
- Generated `.sce` difficulty variants.
- Detecting the KovaaK's playlist in progress.

KovaaK's file locations, checked against a real install:

| What | Where (default Steam library) |
|---|---|
| Run results | `FPSAimTrainer\FPSAimTrainer\stats\*.csv` |
| Playlists | `FPSAimTrainer\Saved\SaveGames\Playlists\*.json` (UTF-16) |
| Active playlist | `Saved\SaveGames\PlaylistInProgress.json`, `PlaylistOrder.json` |
| Local and workshop scenarios | `Saved\SaveGames\Scenarios\*.sce`, `steamapps\workshop\content\824270\<id>\` |

## 9. Testing and releases

The plan is in [TESTING.md](TESTING.md).

### Done
- [x] Pre-commit hook: formatting, clippy with warnings as errors, tests, and a release build to `dist/RustyAim.exe`.
- [x] CI on every push, with the exe as an artifact.
- [x] Release workflow on version tags: portable exe plus installers. v0.1.0 is published; it predates the trainer.
- [x] App options `--db <file>` and `--no-import` for test and demo databases.

### To do
- [ ] `todo` P1 Release v0.2.0 with the trainer, adaptive ratings, Core Week and Insights.
- [ ] `todo` P2 Playwright smoke test of every screen on CI, against a bot-generated demo database.
- [ ] `todo` P2 Backend tests for the Tauri commands against an in-memory database.
- [ ] `todo` P2 Treat KovaaK's files missing `Score:` or `Challenge Start:` as incomplete and retry them (the half-written file risk), with a fixture test.
- [ ] `todo` P3 Golden snapshot tests of the reports (`insta`); property tests for the parser and session grouping (`proptest`).
- [ ] `todo` P3 Daylight-saving handling: cap run length and add fixtures that span a clock change.
- [ ] `todo` P3 Ignored real-data test over the local stats folder; an import timing test with 10k files.

## 10. Later

- [ ] `todo` P3 Session notes (sleep, energy, hardware changes) linked to sessions.
- [ ] `todo` P3 Export to CSV, and database backup and restore.
- [ ] `todo` P3 Auto-update, and code signing so SmartScreen stops warning.
