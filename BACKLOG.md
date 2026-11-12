# AimAdapt Backlog

AimAdapt is an aim trainer for Windows. It rates you in real units, keeps each drill at the difficulty you hit about 85% of the time, runs a weekly training program, and records every target and shot for analytics. Importing KovaaK's stats files is a secondary feature.

**Priority:** P1 = next up · P2 = planned · P3 = later or optional

**Status:** `todo` · `doing` · `done` · `spike` (research before committing)

Last reviewed: Nov 11, 2026 (after the layout review, the adaptive review and the change that makes the rating count slips; every package in [the review plan](docs/plans/2026-11-review-changes.md) is done, and finished items were moved to Done).

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
- [x] The results screen shows a chart of the scenario's last ten ratings (scores for a standard run) with the run marked, and one note from its recent runs, picked to be what a player can change next. When the app started the trainer, V closes it and opens the run's replay, and I opens the scenario's page (plan L3a). The list at the end of a workout has one line for each scenario, and the chart and note are left out when the window is too short for them. Ratings and scoring are unchanged and there is no migration.

### To do
- [ ] `todo` P1 Pause menu and pausing on focus loss (Esc quits straight away today).
- [ ] `todo` P1 Hit and kill sounds (generated, no asset files), with volume in settings.
- [ ] `todo` P2 Measure input-to-photon latency and frame pacing; optional frame cap.
- [ ] `todo` P2 List the monitor's real video modes in Settings instead of free-typed resolutions (the engine can already read them).
- [ ] `todo` P2 More crosshair options: outline, and a color change on hit.
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
- [x] Scenarios page tabs: Recent, Browse (every playable scenario by skill, tag chips you can combine, sort by skill, name, most played or longest since played), Workouts and Yours. Favorites (starred, first on the lists and on Today) and Hidden, saved in `list.json` in the data folder. Today's favorites and recently played have Play.
- [x] Scenarios tabs are Browse, Workouts and Program: Recent and Yours became Browse's Played filter and a Yours chip (the old addresses open Browse with the filter set), Browse opens by last played once anything is played, and one Rating or score column replaces Rating and Best score. Narrow windows drop Runs, Type and Skill so Play stays in view and the page doesn't scroll sideways (plan L6a).
- [x] Scenarios lists across the whole page, one tab each, with each scenario and workout on its own page: a table sortable by every column (headings over each skill, type, kind of rating or day), a search over names, skills, tags and descriptions, Type (including KovaaK's that play here), Played, Made by and Favorites filters, 100 rows at a time, and each tab remembering its filters, sort and scroll.

### To do
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
  - Steadier ratings (plan A1): the slope's uncertainty is carried between runs and the carried uncertainty applies to the goal point. A steady simulated player's rating moves about 2.2% from run to run, down from about 4.5%.
- [x] Program figures and Insights keep a scenario's versions apart, as the trainer does: bests, notes, the rating chart, skill gains and carry-over use only the version last played, and the Runs table greys older versions' ratings (plan A3, a bug found by reading).
- [x] Adapt between runs, not during them (from [playtesting](PLAYTEST.md): changing difficulty target by target felt chaotic).
  - The first adaptive run on a scenario is a calibration run that adapts as you play. So is the next run after a break: three days or more away, with the rating's uncertainty widened back to 0.12 or more (about five and a half days for a settled rating). It starts from the old rating and keeps the old best, and the HUD and results say how many days it has been. It's automatic, because an offer would need a prompt in the middle of a session (plan A4).
  - Every later run plays at one value, shown up front with its kind ("At your level: 0.62 s"), picked from the mix: at your level (80–90% chance), stretch (55–72%) and easy (93–97%).
  - One Steady, Balanced or Push mix for every profile, set on Settings › Training (plan L5). A mix saved in a profile before that carries over until a change is saved there.
  - Guard rails: no stretch run after a stretch run, or after a run more than 10 points below the goal.
  - Inside a block (a scenario played two or more times in a row in a workout or program day) the kind of each run is planned from the mix: the first at your level, stretch runs in the middle with none side by side, easy runs last. A run on its own still draws its kind (plan A5).
  - The trainer names the kind of run with what to expect before it starts ("Stretch run: 0.52 s" with "About 6 in 10 expected" under it), and the results judge a stretch or easy run against its plan (as planned, above or below), with room for the luck of a short run and at least five points. During a stretch or easy run the live success rate is shown beside the plan instead of the goal (plan A2).
  - Planned against actual (plan A6): each adaptive run's length is saved (schema v9, `engine_runs.evidence`). A card on a scenario's Runs tab and on Progress › Skills, and `aimadapt-cli plan-check [scenario]`, set the chance planned for runs at your level, stretch runs and easy runs beside how often you succeeded, weighted by run length, and mark a gap bigger than luck explains. Runs saved before v9 are left out and counted. The Runs table names each run's kind and tags breakthroughs, which are derived from the history the way the sim decides them.
  - A breakthrough is holding the goal for a whole run at a value past your best, announced at the end of the run. A run under 30 targets (a tracking segment counts as three) has to beat the goal by one binomial standard deviation to hold it, and the results screen says when a run was at the goal but too short to count (plan A8).
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
- [x] Ranges and noise on the figures derived from ratings (plan A7). Every change on Progress › Skills (the skill profile, each day's change against last week, reps and carry-over, the biggest gain) has a likely range built from the ratings' stored ranges (`rating_sd`, or half the distance between `rating_low` and `rating_high` on the knob's log scale), and one with zero inside its range is greyed as noise. A change from runs with no stored range shows no range and is never flagged. Carry-over has a By drill summary, and both it and a least improved skill need three sessions. Saturday's day ranks untrained skills first, then skills with under three sessions (fewest first), then the rest by change. "Weakest" is no longer used for a gain. Ratings and scoring are unchanged and there is no migration.
- [x] The rating now means the difficulty the player really hits 85% of the time, slips included (before, it was the 85% point of the curve before the 4% allowance for slips, so the true chance of success at the rating was about 83.6%). Every rating reads about 1 to 3% easier, once. The Insights chart's 85% mark is the same figure as the rating in the page header (the two differed by a percent or more because of the slips and because the chart drew its curve from numbers that were not quite the ones behind the rating). A run starts from the rating the last run ended on (it started up to a few percent harder while the slope was uncertain). Schema version 10 converts saved ratings the first time the database opens, so history, charts and changes stay on one scale: a rating held at a scenario's limit stays there, best held figures are played values and do not change, and the old figures are kept in `engine_ratings_v9`. The conversion has been run on bot data only.

### To do
- [ ] `spike` P1 Tune against real play. The default easy/hard ranges, first guesses and the "consistent vs. fades gradually" cut-off are educated guesses that have only been checked with the bot.
- [ ] `todo` P3 The Insights charts cover the last ten runs whatever their version, so for a few runs after an edit they mix old and new settings. Keep them to the current version, as the ratings are.
- [ ] `spike` P3 Targets within a run are picked from the curve's mean midpoint and typical steepness, which sits a little harder than the rating while the steepness is still uncertain. Check whether picks should use the same curve the rating is stored with.
- [ ] `todo` P3 Planned against actual works its gap out from the unrounded figures, so a row can read 86% planned, 86% actual and a gap of +1 pt. Work the gap out from the figures as shown.

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
- [x] A Program tab on Scenarios with the week, each day's drills and a start button; Core Week's day workouts are kept off the Workouts list, which points to it (plan L4).

### To do
- [ ] `todo` P1 Start Core Week from the real app window and play it by hand for a full week; fix whatever comes up. On the way, check that closing the trainer lands on the session summary in the built app, and time the summary on a long history (neither is covered by the bot or the browser preview).
- [ ] `todo` P2 Pick the active program, and add more programs (e.g. a clicking-heavy or tracking-heavy week).
- [ ] `todo` P2 Program editor and weekly calendar: edit the plan in the app (each day's workout, drills, runs and blocks) and set your week (which days you train, which are rest days, which day is weak spots), instead of Core Week's fixed Mon–Sat. Goes with the scenario and workout manager (section 2).
- [ ] `todo` P2 Weak-spots day from Insights detail, not just sub-skills: e.g. your weakest flick direction, distance band or tracking speed.

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
- [x] Settings tab with profiles, usually one per game. Each profile holds sensitivity in that game's units (Overwatch 2, Valorant, CS2, Apex, Call of Duty, or cm/360), DPI, FOV with the game's usual value, crosshair with a preview, and display settings.
- [x] Profiles switch in Settings (Scenarios shows the active one); `--profile <name>` in the engine. The first profile is matched to your latest KovaaK's run.
- [x] Settings › Training: the mix (one for every profile), a daily goal that can be changed, and advice on session length, in one tab that replaces Goals and time (plan L5). Both are saved in training.toml.

### To do
- [ ] `todo` P2 More games (Fortnite, Rainbow Six Siege, Marvel Rivals, The Finals) once their sensitivity formulas are checked.
- [ ] `todo` P2 Session gap, day start and the restart rule that can be changed. They are fixed and listed on Settings › Training.

## 7. App screens

### Done
- [x] Four pages instead of ten (mockups on the design canvas): Today, Scenarios, Progress (Skills, Benchmarks, Time) and Settings (Aim and display, Training, KovaaK's, Your data), with Help at the bottom of the sidebar. Old addresses redirect.
- [x] Scenarios: one list of trainer scenarios, workouts and KovaaK's scenarios, newest first by day, with name search and Skill, Tag and Trainer filters, beside the selected one as a single scrolling page (Play, figures, rating chart, what to work on, Insights, runs with replays).
- [x] Today: the program day with each drill's rating, the week, recently played and two notes.
- [x] Progress › Time lists every session.
- [x] Help: the user guide (`ui/guide.md`) with contents, search and What's new. Every page's header links to its section. A test keeps it in step with the pages, built-ins, ratings, trainer keys and version.
- [x] Play always means an adaptive run of one scenario; workouts and program days say Start session, Continue session and Repeat session; a KovaaK's scenario that plays here says Play in AimAdapt (plan L1).
- [x] A scenario's page has Overview, Insights and Runs tabs under one header, each with its own address, and a KovaaK's scenario's page has Overview and Runs the same way (plan L2).
- [x] Closing the trainer after a run opens a session summary: each scenario's rating before and after the session with the change and its likely range, breakthroughs, the time played and one note. Today has a Last session card with the same changes (planned as "Since your last session"; plan L3b). Ratings and scoring are unchanged and there is no migration.
- [x] Progress trimmed: Skills leads with the profile and carry-over and has no start buttons, Benchmarks says it comes from KovaaK's, Time folds its extra cards, and notes are ranked once, with Today showing the top one (plan L4).
- [x] Help has its filter at the top of the contents rail, which lists only the matching sections, marks the current one and scrolls on its own; the sidebar shows icons only at 1180 px and under; editor sections that hold only defaults start folded; every page's Help link lands on a guide section (plan L6b).
- [x] The sidebar stays in view on a long page, and scrolls on its own in a window too short for it, so the page links, Help and the status box don't leave the window.

### To do (second step of the simpler layout)
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

The plan for closing the gaps is [docs/plans/2026-11-kovaaks-compat.md](docs/plans/2026-11-kovaaks-compat.md): packages K1 to K15, one commit each, with the count of files that play before and after. On 11 November 2026, 98 of the 225 files played. K1 is done: `aimadapt-cli sce` lists every reason on a file, and `--summary` counts the files each reason is in and the files it blocks by itself. K2 is done: bots use their movement abilities, which brought the count to 103 of 225 and to 31 of the 34 Voltaic S5 Novice benchmarks. K3 is done: helper bots (bots on the player's team) push the targets with their melee abilities, pinned or chasing from behind a clip wall, and a character's `InvincibleBots` applies to its bots. The importer now also reads `Timescale` (the rate of the whole game clock), which result files showed while K4 was being prepared. K4 is done: a scenario with one bot slot and `BotMaxLives=N` ends on the Nth kill (`BotMaxLives` is a list with one number per bot slot, which the importer read as a single number before), `ScorePerTime` adds its rate times the game seconds left, and `ScoreMultDamageEfficiency` multiplies the score by damage done over damage possible. Notes on the page say when a multiplier meets a score rule it wasn't checked with. Still refused: lives on several bots, on some bots and not others, on a helper, on a bot that is only sometimes invincible, and a list that doesn't match the number of bot slots (none of these was measured in KovaaK's), and `ScoreMultKillEfficiency` (no result file to check a formula against). After K4, 111 of the 225 files play: the three with lives and a time score (Air Pure and Air CELESTIAL among them) and one with the damage efficiency multiplier. Still refused: pushes that do damage or vary between a smallest and a largest value (`HurtboxDamage`, the `…Min` knockbacks), long swings (`AbilityDuration` over 0.05 s hits once per entry, not on a schedule), more than one charge, helpers that don't always use the ability or that see a narrower view, helpers that carry weapons, a target's own melee ability, helpers that are neither pinned nor a chaser (a chaser is a flier that always goes forward), chasers whose character has no `Acceleration`, `DragCoefficient` or `MaxSpeed` above 0, a helper with a `BlockAbilityOnStartDuration` over an hour, and a flying target that doesn't move by itself in a scenario with helpers (whether it hovers or falls when pushed wasn't measured). After K3, 107 of the 225 files play, and all 34 Voltaic S5 Novice benchmarks do. The ability and helper setups that K2 and K3 still refuse are now their own package, K15. K5 is done: hitbox shapes were measured in KovaaK's first (a cylinder bot is a capsule, a spheroid is always a ball, a cuboid is a box lined up with the map), so the sim has a box shape, cuboid bots play as boxes when the player starts facing along one of the map's axes, tall spheroids start at the right height, and the two notes that described differences that don't exist are gone. The count stays at 111 of 225: the six local files with cuboids each wait on another package too. K15a is done: a negative `HealthRestore` on a movement ability is a health cost, taken off the bot's health at each use. A bot whose last health goes that way leaves as a timeout (no kill, score or damage; a timeout scores nothing in an imported scenario) and comes back at full health after its respawn delay. Measured in KovaaK's with two test maps. Still refused: healing (a positive `HealthRestore`) and a cost on a bot that can't die, which weren't measured. A cost on a bot with health regen or drain, or in a scenario with bot lives, plays with a note, since neither combination was measured. Two more local files play, which makes 113 of 225. K15b is done: a chasing helper has a flight (the character's `Acceleration` and `MaxSpeed` in meters, and its `DragCoefficient`) and flies by the law measured in KovaaK's with six films: each frame its velocity loses the drag's share, gains the acceleration along the line to its target, is cut to the top speed and loses the part that points into its slab. It starts at its spawn point, trails a target that moves (behind by the lane's distance times tan(asin(drag × speed ÷ acceleration))) and takes time to cross to one that appears elsewhere. Its reach and its level push point along the line from where it is to its target, and K3's instant chaser stays only for a stored world whose chaser has no flight. The speed threshold of 100 m/s is gone, since every chaser follows the same law. A character's `BlockAbilityOnStartDuration` is a helper's start delay, played as no hits for that long from the start of the run, with one note for each delay, because what it means for a helper wasn't measured. Not modelled: the chaser's aim being a little late (0.008 to 0.033 s), and the frame-rate dependence of a top speed that binds. No imported scenario with a chaser was watched in the game window. Three more local files play, which makes 116 of 225, and no local file is refused for its helper bots any more. The three that already played with a chaser have a different fingerprint now, so bests on them start again. K7 is done: a bot that carries a weapon but is told not to aim (`NoAiming=true`) never fires in KovaaK's (measured with four test maps), so a file with such bots is no longer refused. `InvinciblePlayer=true` on the player's own character profile is no reason to refuse a file either, because nothing in the trainer hurts the player. A helper with a weapon is still refused, aiming or not. Still refused: bots that aim and fire, unless the scenario's own `InvinciblePlayer` is on (whether the character's switch protects the player wasn't measured), and `InvincibleBots=true` on the player's character (not measured). Sixteen more local files play, which makes 132 of 225, and no file that already played changes. K15c and K15d are done: dashes and blinks that go up or down play, and a bot uses a dash or blink when it is hurt, both as measured in KovaaK's with test maps (the bot movement item below has the rules). K15c started no file by itself. With K15d one more local file plays, which makes 133 of 225, and no file that already played changes. K15 ends there. Its last part (K15e) builds nothing: every setting K15 leaves refused is in a file that is refused for something else too, except in one file whose targets have a melee ability and whose timer bots destroy themselves, and that file waits on K8. K8a is done: a still bot (no speed, no gravity, nothing pushes it) that may use weapons, carries no weapon of its own and has exactly one ability, a weapon ability whose weapon is a slow projectile that goes off inside the bot, now plays as the fuse. The bot fires on its ability key's ticks, held to the weapon's time between shots (a press the weapon can't answer within 0.1 s is lost), each blast takes health off it a set time after the shot, the weapon's cooldown outlives the bot, and a bot that dies of its own blast is a timeout, as measured in KovaaK's 3.9.13 on 12 November 2026. Every other weapon ability is still refused, now with the first reason that fails named in the detail (`weapon abilities (Zap: its weapon has ExplodesOnContactWithEnemy=true)`). With K8a 139 of the 225 local files play, up from 133. Not measured, and played with notes or as decisions: a press the weapon doesn't answer still uses the ability's charge, a blast waiting when its bot is killed is dropped, and a bot that dies of its own blast uses up no life. Measuring for K8 showed a fault in what already plays: a bot whose file gives a range of respawn delays came back after the middle of the range, where KovaaK's brings it back after the least (measured there with test maps). That is fixed. The count stays at 133 of 225, and the eight playing files with a range have their bests start again.

### To do
- [x] Phase 1, reader and inventory (`crates/aimadapt-sce`): settings, both map formats, and each scenario's player, weapon and bots resolved through `.bot`, `.rot` and ability references. `aimadapt-cli sce [name]` lists each file as covered by phases 2–3 or with the features it uses beyond them. On this machine: 118 of 225 files covered, including all 34 Voltaic S5 Novice benchmarks.
- [x] Phase 2a, still bots on JSON maps: a scenario can carry a world (`aimadapt-sim::world`: spawn points with offsets along the map's axes, or along each point's own when a world has no frame for them, blocked radius, accuracy multiplier, FOV lock, wall meshes), `aimadapt-sce::convert` builds one from a `.sce` (Unreal centimeters to meters, `MapScale` on map positions and walls only, `Cube` brushes), the trainer plays `--sce <file>` and draws the map, and a KovaaK's scenario's page has Play in AimAdapt. Runs are stored with their own mode and left out of Progress. 11 of the 34 Voltaic S5 Novice benchmarks played then (1w4ts ×5, ww5t ×5, DotTS); all 34 do now.
- [ ] `todo` P1 Phase 2b, the rest of the world sim: the weapon's hit radius (`HitscanRadius` above 1 unit; no local file has one), cylinder and cuboid bots flatter than they are wide (one test in KovaaK's, rule unknown; no local file has one), cuboid bots on a map where the player starts turned off the map's axes, bot lives on several bots (`BotMaxLives` with more than one number above 0), the kill efficiency score multiplier, crouching (only a walking bot whose crouch changes its height or its speed is left: a flier's crouch key is played, and no playing file has a walker that changes), a walking bot climbing a step within its step-up height (only the moment a bot appears is played; walking onto a step wasn't measured), the head's offset (one measurement: 20 units raise the eye 10), and check that `MapScale` doesn't apply to speeds (it doesn't to character heights, and not to spawn offsets either: measured in KovaaK's 3.9.13). The rarer shapes (`Sphere`, `Cone`, `Pipe`, `Dome`, `Torus`, capsules, pyramids, `HalfRamp`, `WedgeA`, `StairsB`) are drawn from their names and not solid: check them against KovaaK's when a scenario that matters uses them. Done: hitbox shapes, measured with five `AimAdapt - Shape Test …` maps in KovaaK's: a `Cylindrical` bot at least as tall as it is wide is a capsule with rounded ends, a `Spheroid` is a ball whatever its height, a `Cuboid` is a box that stays lined up with the map's axes, and a spheroid starts the larger of its height and twice its radius above its spawn point (tall ones started too low here); cuboids play as boxes. Done: where characters start, measured with 32 test maps in KovaaK's (`AimAdapt - Eye Test …`: flat Reflex and JSON floors, reading the eye height off the floor's far edge): a character's middle starts its whole `MainBBHeight` above its spawn point or the point in its spawn volume, on both formats, for players 2 to 300 tall, radius 1 to 55, map scale 1 and 4, and still bots; the eye is the camera offset above the middle, or with `MainBBHasHead` above the top of the body; the capsule that stands on floors reaches the head's radius further each way; a floor within `StepUpHeight` (plus 2.4 units) below the feet pulls the player down onto it, and one on a floor stands 2 units up (tamTS, Pasu Smooth Regen and B180 now match KovaaK's), spawn volumes, rotations as Unreal's `FRotator` (roll turned the wrong way), negative health regen (bots drain and go as timeouts: VT Aether, PGT), rotations with `Randomized=false` taken in turn (slots naming the same rotation share turns), `InvincibleBots`, `Ramp`, `WedgeB`, `Concave`, `RampConcave`, `RampConvex` and `DoubleRamp` shaped, placed from their corner and solid (checked against KovaaK's on DriftTS, ww5t, ControlTS and Frogtagon, and against the lettering on Clicking Static 5's wall), Reflex maps (centimeters, X right, Y up, Z forward, clip brushes), every JSON primitive drawn, walls as solid triangles (shots stop at them, bots bump into them; `Clip`/`FullClip` and Reflex clips are invisible and let shots through), a brush's own triangles scaled by `MapScale`, characters lifted onto the floor when they'd spawn in it, the player's team (`PlayerTeam`, team 0 bots as opponents), the player's spawn rules, health regen, several kinds of bot (per slot, by weight, with their own spawn points), clusters, bot lives on one slot (`BotMaxLives`), the score for the time left (`ScorePerTime`) and the damage efficiency multiplier (plan K4). Done (plan K6a): each kind of bot starts at its own height above its spawn point and uses its own spawn offsets (every kind used the first kind's), a bot that never moves by itself falls when its character has a `Gravity` above 0 (it hung in the air), and a bot with gravity that appears with a floor within its `StepUpHeight` below its feet starts on it (measured in KovaaK's: a gap of 250 with a step-up of 300 is put on the floor, a gap of 80 with 75 falls). A bot's own `BlockSpawnFOV` was measured too and does nothing to where bots appear (96 spawns landed on every point of an arc in front of the player), so it stays unread. Kinds that differ in `BlockedSpawnRadius` still use the first kind's. Done (bug fix after K7): spawn offsets run along the map's axes, never the spawn point's (measured in KovaaK's 3.9.13 with test maps, JSON and Reflex, points turned by yaw and by pitch), in Unreal units that `MapScale` doesn't scale. In a file saved by KovaaK's 2.0.5.0 or later X is the map's forward, Y its right and Z up; below 2.0.5.0 (measured at 2.0.4.9 and 2.0.4.0) X runs along the right and Y along the backward, and versions below 2.0.4.0 are read the same way without a measurement. The older keys `SpawnXOffset`, `SpawnYOffset` and `VerticalSpawnOffset` are one fixed offset under the same rule; a character with both the older and the newer keys is refused (not measured). A world stores the map's frame (`offset_axes`) only where running offsets along each point's own axes would move a target, so every other scenario keeps its fingerprint, and the imported scenarios that do move lose their bests. A helper's offset follows the bots' rule, which wasn't measured for helpers. The player's own older keys were left unread until the fix after K9, at the end of this item. Done (bug fix after that): imported bots heal in steps, as KovaaK's does (measured in 3.9.13 with twelve test maps, one still ball and a held weapon that never misses). A bot gets a lump of a tenth of a second's regen at a time. With a regen delay of 0 a lump lands every 0.1 s whatever the hits (the phase is read as counting from the moment the bot appears; it wasn't measured). With a delay, each hit starts the countdown again, and when it runs out the bot gets a lump at once and then one every 0.1 s. The timers are checked once a frame, after that frame's shots, so a hit on the frame a countdown ends wins, and a delay equal to the shot interval can't outheal steady fire (before, a bot healed between shots and could not be killed). The timers go by the game time at the start of the frame, as the weapon's cooldown does, so a shot that is due fires before a step that is due however uneven the frames are. With a delay shorter than the shot interval the count depends on the frame rate and was measured at 120 frames a second only. Built-in scenarios, the beam and the drain (negative regen) keep their rules, and a scenario with a world, a hitscan weapon and positive regen has a second mark in its fingerprint, so its bests start again. Done (plan K9): bots with heads, measured in KovaaK's 3.9.13 on 12 November 2026 with test maps for capsule and ball bodies. The head is a ball stacked on the body (`MainBBHeadRadius`, `MainBBHeadOffset`; the colour `EnemyHeadColor`), its middle `radius + offset` above the body's top, and body and head together are centred on the character's middle (so the body's middle is `(2r + offset) / 2` below it and the head's `(B + offset) / 2` above, with `B` the body's height, a ball's twice its radius). The first surface on the ray decides head or body, a head hit does the damage times `HeadshotMultiplier` for a weapon with `HeadshotCapable=true`, and a shot's possible damage (what damage efficiency divides by) is the damage times the multiplier whenever the weapon can headshot, with or without a head to hit. `HeadshotOnly` makes a body hit a miss in every way. A capsule with a head stands with the bottom of its body on the floor, so it is `B + 2r + offset` tall. `MainBBHide` changes nothing about what a shot hits. Played without a measurement: a head sunk past twice its radius (the same formulas), a ball with a head under gravity (stands as the capsule does), what such a bot bumps into (a capsule of the body's radius and the stack's height, even when the head is wider), a shot that stops at the nearest surface, so the body of a headshots-only bot hides a bot behind it, the beam's ticks by the same rules, and heads that scale with a shrinking target. The deep sink, the falling ball and the wider head are notes on the scenario's page. Still refused: a head on a box bot, a head with a radius of 0 or less, `HeadshotOnly` on a character with no head or with a weapon that can't headshot, a multiplier of 0 or less where the file would set it, and a bot that blows itself up and has a head. With K9 143 of the 225 local files play, up from 139: all 21 files that were refused for a head lost that reason, four of the five that waited on it alone play, and no file that already played changes. Not done: a standing bot in KovaaK's floats about 0.02 m above the floor, with or without a head (played on the floor), replays draw the body at the character's middle without the head, and a run's shot and tracking records measure the aim's error from the character's middle, which lies between body and head (the on-target flag follows the part that counts). Done (bug fix after K9): the player's own older offset keys are read (`offsets::player`). Measured in KovaaK's 3.9.13 on 12 November 2026 with six test maps (a floating player and five still balls straight ahead): they are one fixed offset in Unreal units along the map's axes, turned a quarter below 2.0.5.0, as a bot's are. An older vertical offset on a player that falls is refused, and so is a player with both kinds of key (neither was measured). The newer keys stay the middle of their range along the map's axes, and are turned with the file's version too, by inference. Reading them showed a flaw in `Collider::stand`, which only the player's start uses: a body shorter than its reach (its step-up plus 2.4 units) was tested only at the far end of its drop, so a speck of a player just above a floor stayed where it appeared, and one over a thick floor was stood inside it. Such a body is now tested over its whole drop. Three more test maps confirmed the rule: a speck of a floating player is pulled down onto a floor within its step-up, and stays where it is with no step-up or with the floor past its reach. 144 of the 225 local files play, up from 143, and four that already played start the player from a changed eye position, so bests on them start again.
- [ ] `todo` P1 Phase 3, bot movement: Unreal-style character movement and dodge profiles are done (`aimadapt-sim::motion`: speed, acceleration, friction, braking, gravity, air control, terminal velocity, jumps and air jumps, flight, a jetpack, wall bounce, strafes with pauses and time multipliers, keeping a distance, turning when hit or blocked, no walking off ledges, dodge profile changes, bots bumping each other). Movement abilities are done too (dashes, blinks and ability jumps as on Ground, with their charges, timers and AI rules for distance, view, ground and air). The ticks and tries of the ability key, the reuse time, the charge refill from the press, the delay, the wait after a bot appears (1 s when its character has no `BlockAbilityOnStartDuration`) and a dash along the ground were measured in KovaaK's with `AimAdapt - Use Test …` and `AimAdapt - Vert Test …` maps. Helper bots' knockbacks are done (K3, see section 9's plan paragraph). Abilities that cost health play (K15a). Dashes and blinks that go up or down play (K15c): they take their slope from the bot's aim line, which KovaaK's runs from one body height above the bot's middle to one body height above the middle of the player's character (measured; a dash along the movement keys heads where the keys point, and one that isn't goes along the line itself), so a flier that blinks to and fro drifts to the height where that line is level. Two cases stay refused because nobody measured them: a held dash that keeps gravity, and a bot or player shaped like a box (an impulse along the movement keys, and a dash not along them on a bot with `NoAiming`, are refused too). Abilities a bot uses in reaction to damage play (K15d: a trigger when the damage since the last one reaches the threshold, a chance to ignore it, a random wait, then a use if there is a charge, all measured in KovaaK's; one on an ability with no duration stays refused, since KovaaK's sometimes showed nothing for it). Left, and staying refused after K15e: healing abilities, a health cost on an ability that doesn't move the bot, abilities with a health limit or a hurtbox, dashes on bots with `NoDodging`, and bots with a size of zero; scenarios with those say they can't be played yet. No local file would start if one of them played, so none was built. Not measured: `AbilityGlobalCooldown`, a tick in a window where the bot isn't willing (played as no try), a bot with several abilities (played in list order, only the first ready one used), whether a try the ability's rules turn down counts (played as a try), a helper's `BlockAbilityOnStartDuration` (played as a start delay, with a note), and for a reaction to damage: whether the damage counted is cleared when the reset timer runs out (played as cleared), whether the ability's own rules, the wait after the bot appears or an ability that is going on hold it back (played as dropping it), and which goes first when a tick of the key and a reaction fall due together (the tick). What a bot's abilities keep when it respawns was measured in KovaaK's 3.9.13 on 12 November 2026 with seven test maps of still balls that come back where they were (films at about 55 frames a second, times good to about 0.02 s). The start block counts once, from the start of the run (with a block of 3.0 s and a key that ticks every 0.1 s the first use came 3.0 s in and every later life's 0.1 s in). Each ability's reuse clock runs on through a death and a respawn (a reuse time of 2.0 s gave uses 2.1 s apart across a respawn of 0.5 s). The frequency windows run on (with a window of 2.0 s and a frequency of 0.5 the pressing started and stopped on edges 2.0 s apart counted from the start of the run, through deaths). The tick grid of the key and the charges start again with each bot. A bot profile with no `AbilityUseTimer` plays as a tick of 0.1 s, and one with no `UseAbilityFrequency` always presses; the importer reads both. All played so since the fix (the sim keeps a brain per slot and kind that the next bot of that slot and kind takes). Not measured, and played as follows: `DelayAfterUse` and `AbilityGlobalCooldown` across a respawn (they start again with each bot), an ability going on when its bot dies (it ends with the bot), a reaction to damage that is waiting, and the damage counted toward one, when the bot dies (both dropped), a bot that first appears later in a run (its block counts from when it first appears), and a slot whose kind of bot changes between lives (the new kind starts with nothing carried, and its own state is kept for the next bot of that kind in that slot). Done (plan K6b): a bot's jump key, decided when it appears and every jump window (a jump needs a press, and a held key is one jump; pogo, buffered presses, the first landing and air jumps), jetpacks that thrust while the key is held after their activation delay and while the tank lasts (a quarter second wait after it runs dry), and fliers flown by a jump and a crouch key where their profile says so (the vertical speed starts from rest at a new input). Gravity is 800 units/s² per unit of `Gravity`, measured in KovaaK's from bots' hops (the importer had 980).
- [ ] `spike` P1 Phase 4, fidelity check: play each benchmark in both trainers, compare scores and check by eye, and tune. KovaaK's records no target positions, so scores are the measure. Dodge behavior is inferred from its setting names, so expect tuning here.
- [ ] `todo` P2 Phase 5, adaptive: a speed or time-scale multiplier as the knob, 1.0 being the original scenario.
- [ ] `todo` P2 Phase 6, app: both histories side by side on the scenario's page (now a run count and best), Insights and replays for imported runs, and Play from Today. The file's hash is part of the scenario's fingerprint, so a changed file starts new bests; the page doesn't show versions yet. Done: Play in AimAdapt on a KovaaK's scenario's page, runs kept out of KovaaK's history, and every local `.sce` file on Browse (`sce_library`) with Play on the playable ones.
- [ ] `todo` P3 Projectile and explosive weapons, aim assist, recoil and spread, third-person camera (used by other workshop scenarios, not the benchmarks).

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

## 10b. Code health

From the code review of 9 November 2026. Fixed then: two scenario-file panics, fire rate depending on frame rate, trainer and startup errors that never reached the screen, a settings file overwritten with defaults, files written in place, the dev snapshot inside a local exe, same-second runs a day long, the Fitts chart's oldest points, no guard against a newer database, a run saved in two halves, the CLI hiding refusals, several UI bugs, shared dependency versions, lint caps, a pinned toolchain, tests that tie hand-kept lists together, and SQL kept in aimadapt-core (`Db::conn()` is private to it outside tests).

### Done
- [x] Split the longest functions: insights `notes` (273 lines) into one function per section, convert `convert` (240) and `world` (207) into named steps, sim `move_targets` (183) with its free movement apart. Each import of the local KovaaK's files and each scripted run of every scenario came out identical before and after. Caps in `clippy.toml` lowered from 280 lines and complexity 40 to 165 and 36.
- [x] `Sim`'s jobs in their own files under `sim/` (firing, spawning, movement, rating, record; `sim.rs` went from 2,948 lines to about 820). The engine's `Game` keeps its session and its current run in their own structs, and a new run replaces the run state whole, so a per-run field can't be missed in the reset.
- [x] One list of what an imported scenario can't play: every reason is a `Gap` with its feature, its detail and whether it's beyond the plan; the CLI counts refusals by feature too.
- [x] Knob rules as one type in aimadapt-core (`knob::Knob`), checked against the sim's by a test.
- [x] The replay aim trace has a format marker and takes 6 bytes a sample instead of 12 (about 86 KB a minute); older traces still play.
- [x] The results screen's best rating is per scenario version, as the rating is.
- [x] Old replays are pruned: each scenario keeps the trace and frames of its latest 50 trainer runs and of its best run on each version (top score, or best rating for the knob, first to reach it). Older runs keep their stats, targets, shots and samples. The Runs table has Show all, so every kept replay can be opened.
- [x] Report bugs confirmed with tests and fixed: insights notes compared ratings across knobs, sensitivity periods split on the units at the same cm/360, tied best scores named different runs, and programs kept their own 4 AM day start. Insights reading every tracking sample was measured instead: 59 ms for 200 one-minute runs, so it stays (`insights_load_time`, run by hand).

### To do
- [ ] `todo` P3 Keep splitting long functions and lower the caps again: insights `scenario` (160 lines), engine `parse_and_run` (144), sim `spawn` (139), scenario `problems` (133), engine `rebuild_world` (122) and inventory `gaps` (120, complexity 35).

## 11. Later

- [ ] `todo` P3 Session notes (sleep, energy, hardware changes) linked to sessions.
- [ ] `todo` P3 Export to CSV, and database backup and restore.
- [ ] `todo` P3 Auto-update, and code signing so SmartScreen stops warning.
