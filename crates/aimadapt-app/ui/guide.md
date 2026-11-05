# AimAdapt user guide

This guide is for AimAdapt 0.1.0. It covers every page of the app, the trainer window, how ratings work, and what each built-in scenario trains.

## Getting started {#start}

1. **Set up your aim.** Open Settings. Pick the game you play, type your sensitivity exactly as it is in that game, and set your mouse DPI and field of view. If KovaaK's is installed, **Match my KovaaK's settings** copies your latest KovaaK's run instead.
2. **Choose a display mode.** Borderless full screen is the default. See [Settings](#help/settings) for exclusive full screen, a window, and a smaller play area on a big monitor.
3. **Play today's workout.** Open Play. The card at the top shows today's day of the Core Week program. Press **Start** and the trainer opens over the app.
4. **Expect a calibration run.** Your first adaptive run on each scenario finds your level as you play, so it gets harder and easier along the way. After that, every run plays at one difficulty.
5. **Check your progress.** The Program page shows your week and your skill profile. Insights breaks down each scenario: where you miss, how fast you react, how steady your tracking is.

Train most days, keep sessions to about half an hour, and take Sunday off. Day-to-day changes under about 5% are mostly luck. Look at the trend over a week or two.

## Dashboard {#dashboard}

The Dashboard summarizes all your training: trainer runs in standard mode and any KovaaK's runs AimAdapt has imported.

- **Banner:** how long you've trained today against your daily goal, or a reminder that your streak ends with the day.
- **Totals:** active training time (inside runs), session time (including menus and breaks), runs, your streak, and this week's time against the weekly goal.
- **Daily time:** the last seven days.
- **Most played** and **Most improved:** scenarios by runs, and by improvement from your first scores to your best.
- **Recent sessions:** your last five sessions with their active time.
- **Rescan folder** checks the KovaaK's stats folder for new runs. New runs are also picked up automatically while the app is open.

## Play {#play}

Play is where you start the trainer.

- **Today's workout:** today's Core Week day, with its blocks and drills, how many runs, and about how long it takes. The button says **Start**, **Continue** if you've done part of it today, or **Play again** once it's done. On a rest day it tells you what's next.
- **Workouts:** several scenarios back to back in one session. **Start** plays one; **Copy & edit** makes your own version of a built-in. **New workout** starts an empty one. See [Workouts](#help/workout).
- **Scenarios:** every built-in and your own, filtered by category (clicking, tracking, switching, or yours). Each card has **Standard** and **Adaptive** buttons. **New scenario** opens the editor.
- **Playing with:** the settings profile the trainer will use, with its sensitivity, FOV and display mode. Switch profiles here, or use **Edit settings**.

While the trainer is open, the page says so and the start buttons are disabled. Each run is saved when its timer ends, so quitting partway through a workout keeps the runs you finished.

## In the trainer {#trainer}

The trainer is a separate game window. It opens over the app window: centered across it with the tops level, or full screen on the app's monitor.

| Key | What it does |
|---|---|
| Mouse | Aim. The cursor is locked to the window while it has focus, and locks again by itself when you come back from another window. |
| Left click | Fire. Some scenarios fire while you hold it, or fire all the time. Clicking also locks the cursor again. |
| R | Restart the current run. |
| Space | After a run: play it again, or go to the next run in a workout. |
| Esc | Quit the trainer and return to the app. |
| F2 | Show or hide the FPS counter. |

**On screen:**
- **Top left:** "Calibration run" or "This run" with the run's difficulty, and your rating ("85%: 0.62 s").
- **Top center:** the time left.
- **Top right:** your success rate so far against the goal (named for what's counted: "Hit rate", "On target", "On pace" or "Finished in time"), kills and kills a minute. Kill-pace scenarios also show how long you have for the next kill.
- **Targets:** timed targets show a bar that empties as their time runs out, and change color by wave so newer ones stand out (three red, then three blue, then red again on Timed Grid). Switching targets show a health bar.

**After a run** the results show your rating and its likely range, your success rate, accuracy, kills, timeouts and average time to kill. A breakthrough gets a banner (see [Adaptive mode](#help/adaptive)).

## Adaptive mode {#adaptive}

Adaptive mode rates you on one setting per scenario, in real units, and keeps each run near the level you hit about 85% of the time. Standard mode plays the scenario exactly as written and scores points instead.

**What each scenario is rated on:**

| Rating | Units | Used for | Example |
|---|---|---|---|
| Target lifetime | seconds | Timed clicking | "0.62 s": you hit 85% of targets that last 0.62 s. Lower is better. |
| Kill pace | seconds between kills | Grids, untimed flicks, speed switching | "0.80 s": 85% of your kills come within 0.8 s of the one before. Lower is better. |
| Target size | radius in meters | Precision drills | "0.085 m": the smallest target you hit (or stay on) 85% of the time. Lower is better. |
| Target speed | degrees per second across your view | Tracking, strafe clicking, switching | "48.5 °/s": you stay on target 85% of the time at that speed. Higher is better. |

Target speed is in degrees per second because that's how fast your hand has to turn. A target moving at a fixed speed in meters per second crosses your view faster when it's close and slower when it's far, so on scenarios with a range of distances the figure is the speed at the average distance.

**What counts as a success:**
- **Clicking:** a target destroyed before it times out, with at most one miss on it. Spraying clicks doesn't count, and a timeout is a failure.
- **Kill pace:** a kill within the pace of the one before. Each full stretch of that length without a kill is a failure, and the late kill that ends it doesn't count either. When clicking, a kill after more than one miss doesn't count.
- **Tracking:** each 2-second segment is scored by the share of time your crosshair is on the target. On Stability Switch, Switch Track and their variants, only time while a target is damaged and not yet healed counts, so moving to the next target isn't held against you.
- **Finished in time:** on Unbroken Strafes and Unbroken Float, each target needs a stretch of beam and heals fast when you slip off. Finishing it before it times out is a success, so only unbroken tracking counts.

**Calibration run, then one difficulty per run.** Your first adaptive run on a scenario is a calibration run: it changes the difficulty as you play to find your level. Every run after that plays at one value for the whole run, shown when it starts ("This run: 0.62 s"), and your rating updates as you play. Each run is one of three kinds:

| Run | Chance of success | Balanced | Steady | Push |
|---|---|---|---|---|
| At your level | 80–90% | 65% | 85% | 50% |
| Stretch | 55–72% | 20% | 5% | 35% |
| Easy | 93–97% | 15% | 10% | 15% |

The mix (Steady, Balanced or Push) is a setting in your profile. A stretch run never follows another stretch run, or a run more than 10 points below the goal.

**Breakthroughs.** Holding 85% for a whole run (at least 10 targets or segments) at a value harder than your best is a breakthrough. The trainer announces it, and it becomes your new best held.

**How the rating moves.** The rating is an estimate with an uncertainty, much like a chess rating. A surprising result moves it more than an expected one: succeeding on a stretch run counts for a lot, failing on one costs little. The uncertainty grows with the days since you last played, so after a break your rating moves faster again.

**Ratings belong to one version of a scenario.** Changing anything that affects play (speed, size, timers, movement) starts the rating over with a calibration run, because the old rating isn't on the same scale. Changing only the name or description keeps it.

## Built-in scenarios {#scenarios}

There are 29 built-ins across nine sub-skills, at least two for each. In the Core Week program, most drills swap week by week with a variant that trains the same sub-skill.

**Static clicking**

| Scenario | What it is | Rated on |
|---|---|---|
| Timed Flicks | One target at a time that times out if you're too slow. | Target lifetime |
| Timed Wide Flicks | Timed targets far from the last one. Commit to big flicks. | Target lifetime |
| Timed Micro Flicks | Small timed targets close to the last one. | Target lifetime |
| Wide Flicks | One small untimed target at a time, always far from the last. | Kill pace |
| Micro Flicks | Tiny targets next to the last one, each up for 1.5 s. Precision, not reaction speed. | Target size |
| Static Grid | Three still targets; a new one appears as each goes. | Kill pace |
| Timed Grid | Three targets, each on its own 2 s timer. Clear the oldest first. | Kill pace |
| Shrink Reflex | Targets that shrink and vanish after 1.2 s. | Target lifetime |

**Dynamic and linear clicking**

| Scenario | What it is | Rated on |
|---|---|---|
| Timed Strafe Clicks | A strafing body that times out. Read the strafe and click. | Target lifetime |
| Strafe Clicks | A strafing body with a fixed 2 s timer. | Target speed |
| Hop Clicks | A strafing body that eases into turns and jumps. | Target lifetime |
| Linear Clicks | Targets gliding side to side in straight lines. Lead and click. | Target lifetime |
| Angled Glides | Targets gliding in straight lines at any angle. | Target lifetime |

**Tracking**

| Scenario | What it is | Rated on |
|---|---|---|
| Precise Track | A small ball drifting slowly. Stay centered on it. | Target size |
| Depth Drift | A small ball that drifts and comes closer and backs away. | Target speed |
| Strafe Track | A strafing body. The beam is always on, so only your aim counts. | Target speed |
| Close Strafe Track | A body strafing close to you with quick reversals. | Target speed |
| Hop Strafe Track | A body strafing with realistic turns and jumps. | Target speed |
| Unbroken Strafes | A strafing body that heals fast when you slip off. Finish each within 4 s. | Target speed (finished in time) |
| Control Track | A ball moving in long, smooth curves across a wide area. | Target speed |
| Arc Track | A ball that turns gradually, so it travels in curves. | Target speed |
| Float Track | A large ball drifting in every direction. Hold fire. | Target speed |
| Unbroken Float | A drifting ball that heals fast when you slip off. Finish each within 5 s. | Target speed (finished in time) |

**Switching**

| Scenario | What it is | Rated on |
|---|---|---|
| Speed Switch | Four nearly still targets that melt in a short burst. Kill one, snap to the next. | Kill pace |
| Wide Speed Switch | Five targets spread wide around you. | Kill pace |
| Switch Track | Three moving targets that each take half a second of beam and heal when you leave. | Target speed, while on a kill |
| Hop Switch | Three strafing, jumping bodies. | Target speed, while on a kill |
| Stability Switch | Three strafing targets that each need a long, steady beam. | Target speed, while on a kill |
| Depth Stability Switch | Like Stability Switch, with targets that come closer and back away. | Target speed, while on a kill |

## Program {#program}

Core Week is a six-day plan of about 29 minutes a day, built around reps rather than variety.

| Day | Focus |
|---|---|
| Monday | Static clicking |
| Tuesday | Reactive tracking |
| Wednesday | Switching |
| Thursday | Precise and dynamic clicking |
| Friday | Smooth and precise tracking |
| Saturday | Weak spots: drills for your weakest sub-skills |
| Sunday | Rest |

Each day has a warm-up (one drill, two standard runs), two main drills of five reps, two secondary drills of four, and a maintenance drill from another skill group (three reps). Most drills alternate week by week with a variant that trains the same sub-skill, for example Strafe Track one week and Hop Strafe Track the next. The training day starts at 4 AM, so a late-night session counts toward the day it started on.

The Program page shows:
- **This week:** each day as done, partial or missed, with minutes, and each day's main drills compared with the same day last week. A missed day can be made up later in the week with its **Play** button.
- **Skill profile:** the change in each of the nine sub-skills, each group and overall, measured from the end of your first day on each drill.
- **Weekly summary:** days done, minutes, streak, biggest gain, the skill furthest behind, and skills you haven't trained in a week.
- **Reps and carry-over:** your first rep against your last rep of each drill, and how much of the previous session's rating carried over to the next.

## Workouts {#workout}

A workout is a list of scenarios, each with a number of runs and a mode (standard or adaptive), played back to back.

Built-in workouts:
- **Quick Warm-up:** five easy minutes: two standard runs to loosen up, then three adaptive ones.
- **Daily Fundamentals:** about eight minutes of clicking, tracking and switching, all adaptive.
- **Clicking Focus:** timed flicks at every distance, plus reaction and moving targets.
- **Tracking Focus:** reactive strafes, smooth tracking and switching.
- **Core Week: Monday** to **Core Week: Friday:** the program's days, which you can also play on their own.

**Editing a workout:** add scenarios, set runs and mode for each, and reorder them with the arrows. **Save & start** saves it and plays it straight away. Built-ins are copied before editing, so the originals stay as they are.

## Scenario editor {#edit}

The editor is a form with every scenario setting, grouped as in the file format: about, timing, weapon, targets, spawn area, movement, scoring, adaptive and arena. Each field explains itself, and values are checked as you type. Anything that needs fixing is listed under **Fix before saving**.

- **Built-ins are copied before editing.** Your copy is saved in your own scenarios folder.
- **Save & play adaptive** saves and starts an adaptive run.
- **Changing settings starts the rating over** with a calibration run (see [Adaptive mode](#help/adaptive)).
- Scenario files are TOML, so you can also edit them in a text editor. SCENARIOS.md in the project documents every field.

## Progress {#progress}

Progress lists every scenario you've played, with search, sorting and a filter for scenarios with three or more runs. Open one to see its history: every score over time, your personal best, trend and any plateau. Changes to your sensitivity show as markers on the chart, so you can tell a settings change from a real gain. Restarted runs (much shorter than usual) never count as bests.

Progress covers KovaaK's runs and standard-mode trainer runs. Adaptive ratings are on Insights and Program.

## Insights {#insights}

Insights analyzes your runs in the built-in trainer, scenario by scenario. Every run stores each target, each shot, tracking samples 30 times a second and the aim trace, so these figures come from what you actually did.

- **Rating:** your 85% rating, your best and your best held, and the rating by run with its likely range. **Last run** shows how the rating moved during the run.
- **Speed vs. hit rate:** your chance of success at each difficulty. You're reliable (95%) at one end and it's a coin flip (50%) at the other. A steep drop means you're consistent up to your limit; a gentle one means misses creep in early.
- **Speed vs. difficulty:** time to kill against how hard each flick was (distance and size), as a Fitts' law fit. Throughput, in bits per second, is your flick speed corrected for difficulty.
- **By flick direction** and **By flick distance:** hit rate and median time in eight directions and five distance bands, to find a weak side.
- **Where shots land:** your shots relative to the target, lined up with the flick, so you can see overshooting, stopping short, or a vertical bias.
- **Reaction time:** how long before your crosshair starts moving toward a new target, as a histogram.
- **Tracking:** time on target by target speed, how long you take to recover after the target changes direction, whether you trail or lead it, your unbroken stretches on target (half your time on target came in stretches at least this long), and centering (how far from the center you sit while on target).
- **Switching:** how fast you reach the next target after a kill, and how well you stay on the one you're killing.
- **Within a run:** success in 10-second segments, which shows slow starts or fading late.
- **Notes:** plain-language takeaways from the figures above.

**Reaction time and lifetime ratings.** A lifetime rating includes your reaction time, which practice barely changes. A note splits the rating into your reaction and the part you control. That part improves by more than the rating shows: going from 0.80 s to 0.62 s is 23% on the rating, but with a 0.22 s reaction the part you control fell 31%.

## Training time {#training}

Training time shows when and how much you train: a calendar of daily time (use the arrows to change month), active against idle time, time by day of the week, runs by hour of day, and every session.

How time is counted:
- A new session starts after a 15-minute break.
- Session time runs from a session's first run to its last, so it includes menus and breaks. Active time is time inside runs.
- The training day starts at 4 AM.

## Benchmarks {#benchmarks}

Benchmarks lists your bests on Voltaic Season 5 scenarios from your KovaaK's stats files. Rank thresholds aren't loaded yet, so ranks show as a dash for now.

## Library & import {#library}

AimAdapt finds KovaaK's in any of your Steam libraries and imports its per-run stats files (FPSAimTrainer\stats). It only reads them and never changes your KovaaK's files.

This page shows the stats folder, how many runs and scenarios are in the database, any files that couldn't be read, the rules for counting time and restarts, and your sensitivity history: each period with its sensitivity, DPI, FOV, cm/360 and run count. If your FPS sits at KovaaK's cap, it suggests raising the cap.

## Settings {#settings}

Settings holds your profiles. Each profile has its own sensitivity, DPI, field of view, adaptive mix, crosshair and display settings, typically one per game. **Playing with** chooses the profile the trainer uses; **Use this one** switches to the selected one. **Add**, **Duplicate** and **Delete** manage them. Changes aren't used until you press **Save settings**; **Undo changes** throws them away.

**Aim**
- **Game:** pick the game, then type your sensitivity exactly as it is in that game. AimAdapt converts it to cm/360 at your DPI. Choose cm/360 to enter it directly.
- **Mouse DPI.**
- **Adaptive target mix:** Steady (mostly near your rating), Balanced (some stretch and easy runs) or Push (more stretch runs). See [Adaptive mode](#help/adaptive).
- **Field of view:** horizontal, at 16:9. Where a game uses another FOV measure, **Use** fills in the matching value.
- **Match my KovaaK's settings** copies the sensitivity, DPI and FOV of your latest KovaaK's run.

**Crosshair:** color, arm length (0 for a dot only), thickness, gap and a center dot, with a preview at actual size. **Show FPS** turns on the counter (F2 toggles it in game).

**Display**
- **Mode:** Borderless (full screen at your desktop resolution), Exclusive full screen (any resolution and refresh rate your monitor supports, for example stretched 4:3) or Window.
- **Resolution:** for a window, its size. For exclusive full screen, the screen mode.
- **Refresh rate:** exclusive full screen only. Leave it empty for the highest.
- **Play area:** draws the game in a smaller area with black around it, so a big monitor plays like a smaller one. **Set play area** works it out from the screen size you want and your monitor's size; **Use whole screen** turns it off.
- **Play area position:** centered, at the top or bottom of the screen, or a custom spot (how far from the left and from the top, as a percentage of the black space). The preview shows where it will be.

Sensitivity is in degrees per mouse count, so it stays the same at any resolution or play-area size.

## Your data {#data}

Everything is stored on your computer in `%APPDATA%\AimAdapt`:
- `aimadapt.db`: your runs and ratings (SQLite).
- `engine.toml`: your settings profiles.
- `scenarios\` and `workouts\`: your own scenario and workout files.

AimAdapt used to be called RustyAim. The first time the new version runs, it moves your old `%APPDATA%\RustyAim` folder to `%APPDATA%\AimAdapt`, so your history, ratings, profiles, scenarios and workouts carry over.

Nothing is sent anywhere. Deleting one of your scenarios or workouts removes the file; your past runs stay in your history.

## Troubleshooting {#troubleshooting}

- **The mouse isn't locked to the trainer.** It locks again by itself within half a second of the trainer getting focus back. If it doesn't, click inside the trainer window.
- **A rating started over.** The scenario's settings changed, so it calibrates again. This happens when you edit a scenario, and when an update changes a built-in.
- **No KovaaK's runs.** Library & import shows where AimAdapt looked. It searches every Steam library for FPSAimTrainer\stats. Use **Rescan folder** after installing KovaaK's.
- **The trainer opens in the wrong place.** It opens over the app window, so move the app window where you want the trainer, or use full screen.

## What's new {#whats-new}

**November 2026**
- The play area can sit anywhere on the screen, for example near the top, instead of always centered.
- The mouse locks to the trainer again by itself after alt-tabbing.
- Hop Strafe Track, Unbroken Strafes, Hop Switch and Hop Clicks strafe further side to side. Their ratings start with a new calibration run.
- Target speeds show one decimal (48.5 °/s), so small changes are visible.
- RustyAim is now AimAdapt. Your data moves to the new folder on the first run.
- A user guide in the app (this page).
- Each drill is rated on what it trains: kill pace for Static Grid, Timed Grid and Wide Flicks; target size for Micro Flicks and Precise Track; target speed for Strafe Clicks. These scenarios start with a new calibration run.
- Control Track moves in real curves.
- Insights adds centering while on target, and on lifetime ratings, the share that's reaction time.
- The trainer opens over the app window.
- Targets move more like players: they slow down and speed back up at turns, come closer and back away, and jump. Ten new scenarios use this, and most Core Week drills alternate with one of them week by week.
- Adaptive mode calibrates on your first run, then plays each run at one difficulty, with stretch and easy runs and breakthroughs.
- Switching is rated on what it trains: kill pace for Speed Switch, and time on target while you're on a kill for Stability Switch and Switch Track.
