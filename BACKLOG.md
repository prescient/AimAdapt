# AimAdapt Backlog

AimAdapt is an aim trainer for Windows. It rates you in real units, keeps each drill at the difficulty you hit about 85% of the time, runs a weekly training program, and records every target and shot for analytics. Importing KovaaK's stats files is a secondary feature.

**Priority:** P1 = next up · P2 = planned · P3 = later or optional

**Status:** `todo` · `doing` · `done` · `spike` (research before committing)

Last reviewed: Nov 4, 2026 (after the first playtest session).

---

## 1. Trainer engine

`crates/aimadapt-engine` (Bevy window) on top of `crates/aimadapt-sim` (rules, no graphics).

### Done
- [x] Full-screen window with raw mouse input and no frame cap. Sensitivity in cm/360 and DPI; FOV horizontal at 16:9.
- [x] Display modes: borderless; exclusive at any resolution and refresh rate the monitor offers (e.g. stretched 4:3); windowed. Optional play area, centered or at any position (top, bottom or custom, from playtesting), with a helper that sizes it from screen diagonals.
- [x] The mouse locks again after alt-tab: on focus, on every click and every half second while focused (from playtesting).
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
- [x] Scenarios page tabs: Recent, Browse (every playable scenario by skill, tag chips you can combine, sort by skill, name, most played or longest since played), Workouts and Yours. Favorites (starred, first on the lists and on Today) and Hidden, saved in `list.json` in the data folder. Today's favorites and recently played have Play.
- [x] Scenarios lists across the whole page, one tab each, with each scenario and workout on its own page: a table sortable by every column (headings over each skill, type, kind of rating or day), a search over names, skills, tags and descriptions, Type (including KovaaK's that play here), Played, Made by and Favorites filters, 100 rows at a time, and each tab remembering its filters, sort and scroll.
- [ ] `todo` P2 Scenario and workout manager: one place to rename, edit, copy, delete (one or many) and archive your scenarios and workouts, imported KovaaK's scenarios included. Create, edit and delete already work one at a time in the editor, and hiding works from each item's page. Renaming needs care: history is stored by name, so a rename must carry the runs over (or scenarios get a stable id).
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
- [x] Program view (now Today and Progress › Skills):
  - Week grid that allows catching up a missed day later in the week.
  - Skill profile measured from the end of your first day on each drill.
  - Weekly summary and streak.
  - Rep and carry-over gains for each drill.
- [x] Today's workout card (now the Today page).

### To do
- [ ] `todo` P1 Start Core Week from the real app window and play it by hand for a full week; fix whatever comes up.
- [ ] `todo` P2 Pick the active program, and add more programs (e.g. a clicking-heavy or tracking-heavy week).
- [ ] `todo` P2 Program editor and weekly calendar: edit the plan in the app (each day's workout, drills, runs and blocks) and set your week (which days you train, which are rest days, which day is weak spots), instead of Core Week's fixed Mon–Sat. Goes with the scenario and workout manager (section 2).
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
- [x] Skill profile across the nine sub-skills, three groups and overall (on Progress › Skills).
- [x] Replay a run from its aim trace and recorded target positions (60 Hz), from the player's view, at any speed.

### To do
- [ ] `todo` P2 Flick shape from the trace: peak speed, deceleration, and where corrections start.
- [ ] `todo` P3 Rate of improvement per sub-skill (gain per week), with a projection.

## 6. Settings and profiles

### Done
- [x] Settings tab with profiles, usually one per game. Each profile holds sensitivity in that game's units (Overwatch 2, Valorant, CS2, Apex, Call of Duty, or cm/360), DPI, FOV with the game's usual value, adaptive mix, crosshair with a preview, and display settings.
- [x] Profiles switch in Settings (Scenarios shows the active one); `--profile <name>` in the engine. The first profile is matched to your latest KovaaK's run.

### To do
- [ ] `todo` P2 More games (Fortnite, Rainbow Six Siege, Marvel Rivals, The Finals) once their sensitivity formulas are checked.
- [ ] `todo` P2 Editable goals and training-time rules (daily and weekly goal, session gap, day start) in Settings › Goals and time, which shows fixed defaults today.

## 7. App screens

### Done
- [x] Four pages instead of ten (mockups on the design canvas): Today, Scenarios, Progress (Skills, Benchmarks, Time) and Settings (Aim and display, Goals and time, KovaaK's, Your data), with Help at the bottom of the sidebar. Old addresses redirect.
- [x] Scenarios: one list of trainer scenarios, workouts and KovaaK's scenarios, newest first by day, with name search and Skill, Tag and Trainer filters, beside the selected one as a single scrolling page (Play, figures, rating chart, what to work on, Insights, runs with replays).
- [x] Today: the program day with each drill's rating, the week, recently played and two notes.
- [x] Progress › Time lists every session.
- [x] Help: the user guide (`ui/guide.md`) with contents, search and What's new. Every page's header links to its section. A test keeps it in step with the pages, built-ins, ratings, trainer keys and version.

### To do (second step of the simpler layout)
- [ ] `todo` P1 Today: "Since your last session" (rating changes and breakthroughs since the session before) and each drill's change against the same day last week.
- [ ] `todo` P2 One streak: the program streak and the all-runs streak still differ (Today shows the all-runs one).
- [ ] `todo` P2 Settings save as you change them, instead of Save and Undo.
- [ ] `todo` P3 Folder picker for the KovaaK's stats folder.
- [ ] `todo` P3 Skill and tags for KovaaK's scenarios (Voltaic packs map to sub-skills), so the Skill filter covers them.

## 8. KovaaK's import (secondary)

`crates/aimadapt-core` and `aimadapt-cli`.

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
- Writing generated plans into KovaaK's as `AimAdapt -` playlists, and the spike on whether KovaaK's reloads playlist files.
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

## 9. KovaaK's scenarios in the trainer

Play your own `.sce` files in AimAdapt, faithfully enough that scores compare with KovaaK's, then train them adaptively. Voltaic S5 benchmarks first. Imported runs are kept apart from KovaaK's runs of the same name (shown side by side, never merged into bests or benchmarks). Your files are only read; tests use made-up `.sce` files, never other people's scenarios.

What a `.sce` holds (surveyed from 225 local and workshop files, Nov 2026): an INI-like settings part with repeated sections (`[Bot Profile]`, `[Character Profile]`, `[Dodge Profile]`, `[Weapon Profile]`, `[Bot Rotation Profile]`, ability profiles), then `[Map Data]`, which is either an older Reflex brush map (text) or a newer JSON map (boxes, procedural meshes, spawn points, waypoints). The 34 Voltaic S5 Novice files are hitscan only, with sphere or cylinder bots and JSON maps.

### To do
- [x] Phase 1, reader and inventory (`crates/aimadapt-sce`): settings, both map formats, and each scenario's player, weapon and bots resolved through `.bot`, `.rot` and ability references. `aimadapt-cli sce [name]` lists each file as covered by phases 2–3 or with the features it uses beyond them. On this machine: 118 of 225 files covered, including all 34 Voltaic S5 Novice benchmarks.
- [x] Phase 2a, still bots on JSON maps: a scenario can carry a world (`aimadapt-sim::world`: spawn points with offsets along each point's axes, blocked radius, accuracy multiplier, FOV lock, wall meshes), `aimadapt-sce::convert` builds one from a `.sce` (Unreal centimeters to meters, `MapScale` on map positions and walls only, `Cube` brushes), the trainer plays `--sce <file>` and draws the map, and a KovaaK's scenario's page has Play in AimAdapt. Runs are stored with their own mode and left out of Progress. 11 of the 34 Voltaic S5 Novice benchmarks played then (1w4ts ×5, ww5t ×5, DotTS); 30 do now.
- [ ] `todo` P1 Phase 2b, the rest of the world sim: cuboid hitboxes and stretched spheres, true cylinders (now capsules), negative health regen, hitscan radius, bots' own `BlockSpawnFOV`, per-kind spawn offsets, crouching, step-up height, and check that `MapScale` doesn't apply to character sizes. The shapes other than `Cube`, `Cylinder` and `Tube` are drawn from their names and not solid: check them against KovaaK's. Done: spawn volumes, Reflex maps (centimeters, X right, Y up, Z forward, spawns at the feet, clip brushes), every JSON primitive drawn, walls as solid triangles (shots stop at them, bots bump into them; `Clip`/`FullClip` and Reflex clips are invisible and let shots through), a brush's own triangles scaled by `MapScale`, characters lifted onto the floor when they'd spawn in it, the player's team (`PlayerTeam`, team 0 bots as opponents), the player's spawn rules, health regen, several kinds of bot (per slot, by weight, with their own spawn points), and clusters.
- [ ] `todo` P1 Phase 3, bot movement: Unreal-style character movement and dodge profiles are done (`aimadapt-sim::motion`: speed, acceleration, friction, braking, gravity, air control, terminal velocity, jumps and air jumps, flight, a jetpack, wall bounce, strafes with pauses and time multipliers, keeping a distance, turning when hit or blocked, no walking off ledges, dodge profile changes, bots bumping each other). Left: the bot abilities benchmarks use to move: melee knockback that helper bots use to push targets around (Pasu, Controlsphere, Raw Control) and dashes (Ground), with their AI rules (distance, view, reuse timer, charges); scenarios whose bots have abilities say they can't be played yet. Also how long a jetpack bot holds thrust, which is a guess.
- [ ] `spike` P1 Phase 4, fidelity check: play each benchmark in both trainers, compare scores and check by eye, and tune. KovaaK's records no target positions, so scores are the measure. Dodge behavior is inferred from its setting names, so expect tuning here.
- [ ] `todo` P2 Phase 5, adaptive: a speed or time-scale multiplier as the knob, 1.0 being the original scenario.
- [ ] `todo` P2 Phase 6, app: both histories side by side on the scenario's page (now a run count and best), Insights and replays for imported runs, and Play from Today. The file's hash is part of the scenario's fingerprint, so a changed file starts new bests; the page doesn't show versions yet. Done: Play in AimAdapt on a KovaaK's scenario's page, runs kept out of KovaaK's history, and every local `.sce` file on Browse (`sce_library`) with Play on the playable ones.
- [ ] `todo` P3 Projectile and explosive weapons, aim assist, recoil and spread, heads, third-person camera (used by other workshop scenarios, not the benchmarks).

## 10. Testing and releases

The plan is in [TESTING.md](TESTING.md).

### Done
- [x] Pre-commit hook: formatting, clippy with warnings as errors, tests, and a release build to `dist/AimAdapt.exe`.
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

## 11. Later

- [ ] `todo` P3 Session notes (sleep, energy, hardware changes) linked to sessions.
- [ ] `todo` P3 Export to CSV, and database backup and restore.
- [ ] `todo` P3 Auto-update, and code signing so SmartScreen stops warning.
