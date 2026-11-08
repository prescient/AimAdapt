# AimAdapt user guide

This guide is for AimAdapt 0.1.0. It covers every page of the app, the trainer window, how ratings work, and what each built-in scenario trains.

The app has four pages, in the sidebar: **Today** (what to play now), **Scenarios** (everything you can play, and how you're doing on each), **Progress** (how you're doing overall) and **Settings**. Help is at the bottom of the sidebar, and every page has a Help link to its own section.

## Getting started {#start}

1. **Set up your aim.** Open Settings. Pick the game you play, type your sensitivity exactly as it is in that game, and set your mouse DPI and field of view. If KovaaK's is installed, **Match my KovaaK's settings** copies your latest KovaaK's run instead.
2. **Choose a display mode.** Borderless full screen is the default. See [Settings](#help/settings) for exclusive full screen, a window, and a smaller play area on a big monitor.
3. **Play today's workout.** Today shows today's day of the Core Week program. Press **Start** and the trainer opens over the app.
4. **Expect a calibration run.** Your first adaptive run on each scenario finds your level as you play, so it gets harder and easier along the way. After that, every run plays at one difficulty.
5. **Check your progress.** Progress shows your week and your skill profile. Each scenario's page on Scenarios breaks it down: where you miss, how fast you react, how steady your tracking is, with a replay of every run.

Train most days, keep sessions to about half an hour, and take Sunday off. Day-to-day changes under about 5% are mostly luck. Look at the trend over a week or two.

## Today {#today}

Today is the home page: what to play now and how the week is going.

- **Streak and today's time** at the top: the days in a row you've trained, and today's active time against your daily goal.
- **Today's workout:** today's day of the Core Week program, with each drill's block (warm-up, main, secondary, maintenance), its runs and your current 85% rating. The bar shows how many of today's runs you've done. The button says **Start**, **Continue** if you've done part of it, or **Play again** once it's done. **Something else** opens Scenarios on Browse. A drill's name opens it on Scenarios. On a rest day it tells you what's next.
- **This week:** each day's minutes in the program, coloured done, partial or missed. **Week in detail** opens Progress › Skills.
- **Favorites:** the scenarios and workouts you've starred (up to six), with **Play** or **Start**.
- **Recently played:** your last five scenarios or workouts, with your current figure and **Play** or **Start**. Play starts an adaptive run. KovaaK's scenarios have no Play button, since you play those in KovaaK's.
- **Worth knowing:** up to two things: the skill furthest behind in your profile (with a link to that skill's scenarios on Browse), and a pattern from your history, such as cold starts, a plateau or a settings change.

## Scenarios {#scenarios}

Scenarios has four lists, each a tab across the page: **Recent**, **Browse**, **Workouts** and **Yours**. Each scenario and workout opens on its own page; **Scenarios ·** and the list's name, at the top of that page, take you back to the list where you left it. The Scenarios link in the sidebar opens the list you were last on.

- **Recent:** everything you've played, in the trainer or in KovaaK's, newest first and grouped by day (Today, Yesterday, then weekdays and dates). Filter by **Type** (AimAdapt, KovaaK's or Workouts) and **Skill**. KovaaK's rows are scenarios you played in KovaaK's: their page shows your scores, but you play them in KovaaK's.
- **Browse:** every scenario, played or not, grouped by skill: the trainer's, and your KovaaK's scenarios (every `.sce` file in your KovaaK's and Workshop folders, plus any you've played in KovaaK's). A KovaaK's scenario's skill is its aim type (Clicking, Tracking or Target switching) and its tags are the game and weapon tags in its file. Filter by **Type** (AimAdapt, KovaaK's, or KovaaK's that play here: the ones AimAdapt's trainer can play), **Skill**, **Played** (played or not played yet), or pick tags (for example *tracking* and *smooth*) to show only scenarios that have all of them. Each tag shows how many scenarios have it; **N more** shows the rest of the tags.
- **Workouts:** the built-in workouts and yours, with how many scenarios and runs each has and about how long it takes. Filter by **Made by** (built-in or you). The program's own days are on Today and Progress.
- **Yours:** the scenarios and workouts you made or copied with Copy & edit.

**Searching and sorting:** the search on each tab finds every word you type in a name, skill, tag or description, so *wide flick* finds Wide Flicks and anything tagged both. Click a column heading to sort by it, and again to reverse it. Sorting by Skill, Type, Made by, Rating or Last played puts a heading over each group. Ratings only compare within one kind (kill pace, speed, tracking speed or size), so Rating sorts by kind, then best first. Lists show 100 rows at a time: **Show more** or **Show all** at the bottom shows the rest. Each tab keeps its own search, filters and sort while the app is open. **Clear filters** shows the whole tab again.

Each row has the scenario's skill and tags, your 85% rating, your best standard score (or KovaaK's score), how many runs you've played, when you last played it, and **Play** (an adaptive run) or **Start** for a workout. A KovaaK's row marked *plays here* has **Play**, which plays its file in AimAdapt's trainer (see [Playing KovaaK's scenarios](#help/kovaaks-play)); other KovaaK's scenarios you play in KovaaK's. Reading your KovaaK's files takes a moment the first time Scenarios opens; the count line says so while it does.

**Favorites and hiding:** the star on a row, or at the top of a scenario's or workout's page, adds it to your favorites. **Favorites** next to the search shows only your favorites, and they show on Today. **Hide**, on an item's page, keeps something off the lists, for example an old KovaaK's scenario you don't play any more. Its runs aren't deleted. Once you've hidden something, a **Hidden** tab lists it, and **Show again** brings it back.

**Playing:** **Play** on a row starts an adaptive run. On a scenario's page, use **Play adaptive** or **Standard** at the top; **Copy & edit** (or **Edit** for your own) opens the [scenario editor](#help/edit). A workout's page lists its runs, with **Start** and **Copy & edit** (see [Workouts](#help/workout)). **New workout** and **New scenario** start empty ones. **Playing with** shows the settings profile the trainer will use; it opens Settings.

While the trainer is open, the page says so and the start buttons are disabled. Each run is saved when its timer ends, so quitting partway through a workout keeps the runs you finished.

**A scenario's page** is one scrolling page. The links under its name (**Overview**, **What to work on**, **Insights**, **Runs**) jump down it.
- **Overview:** up to four key figures (your 85% rating with your best and best held, targets hit, time on target, reaction, flick speed, or for switching kills a minute and acquire time), the rating by run with its likely range, and **Last run**: how the rating moved during your latest run.
- **What to work on:** plain-language notes from the figures below.
- **Insights:** what your runs in the trainer recorded. Every run stores each target, each shot, tracking samples 30 times a second and the aim trace, so these come from what you actually did:
  - **Speed vs. hit rate:** your chance of success at each difficulty. You're reliable (95%) at one end and it's a coin flip (50%) at the other. A steep drop means you're consistent up to your limit; a gentle one means misses creep in early.
  - **Speed vs. difficulty:** time to kill against how hard each flick was (distance and size), as a Fitts' law fit. Throughput, in bits per second, is your flick speed corrected for difficulty.
  - **By flick direction** and **By flick distance:** hit rate and median time in eight directions and five distance bands, to find a weak side.
  - **Where shots land:** your shots relative to the target, lined up with the flick, so you can see overshooting, stopping short, or a vertical bias.
  - **Reaction time:** how long before your crosshair starts moving toward a new target, as a histogram. It counts flicks of 3° or more, since on a tiny correction "started moving" hardly means anything. The Reaction column in the runs table counts the same flicks.
  - **Tracking:** time on target by target speed, how long you take to recover after the target changes direction, whether you trail or lead it, your unbroken stretches on target (half your time on target came in stretches at least this long), and centering (how far from the center you sit while on target).
  - **Within a run:** success in 10-second segments, which shows slow starts or fading late.
- **Runs:** your latest 30 runs with their rating, hit rate, flick speed, reaction and time on target, the workout they were part of, and a **Replay** link to watch each one again (see [Replay](#help/replay)).

**Reaction time and lifetime ratings.** A lifetime rating includes your reaction time, which practice barely changes. A note splits the rating into your reaction and the part you control. That part improves by more than the rating shows: going from 0.80 s to 0.62 s is 23% on the rating, but with a 0.22 s reaction the part you control fell 31%.

**KovaaK's scenarios** show their score history: personal best, average, accuracy and improvement, every score over time with your bests marked, a trend and any plateau, and every run. Changes to your sensitivity show as markers on the chart, so you can tell a settings change from a real gain. Restarted runs (much shorter than usual) never count as bests. KovaaK's stats files don't record shots, reaction or tracking, so these scenarios have no Insights or replays. Standard-mode runs of a trainer scenario are on its own page, with their score in the runs table. Some KovaaK's scenarios can also be played in AimAdapt's trainer: their page then has **Play in AimAdapt** (see [Playing KovaaK's scenarios](#help/kovaaks-play)).

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

## Built-in scenarios {#builtins}

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

## The Core Week program {#program}

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

Today shows the day's workout; Progress › Skills shows the week, your skill profile and how each drill moves across its reps.

## Workouts {#workout}

A workout is a list of scenarios, each with a number of runs and a mode (standard or adaptive), played back to back.

Built-in workouts:
- **Quick Warm-up:** five easy minutes: two standard runs to loosen up, then three adaptive ones.
- **Daily Fundamentals:** about eight minutes of clicking, tracking and switching, all adaptive.
- **Clicking Focus:** timed flicks at every distance, plus reaction and moving targets.
- **Tracking Focus:** reactive strafes, smooth tracking and switching.
- **Core Week: Monday** to **Core Week: Friday:** the program's days. They're played from Today and Progress › Skills, so they aren't in the Scenarios list.

Workouts have their own tab on Scenarios, and show on Recent once you've played them.

**Editing a workout:** add scenarios, set runs and mode for each, and reorder them with the arrows. **Save & start** saves it and plays it straight away. Built-ins are copied before editing, so the originals stay as they are. **Cancel** goes back to the workout's page.

## Scenario editor {#edit}

The editor is a form with every scenario setting, grouped as in the file format: about, timing, weapon, targets, spawn area, movement, scoring, adaptive and arena. Each field explains itself, and values are checked as you type. Anything that needs fixing is listed under **Fix before saving**.

- **Built-ins are copied before editing.** Your copy is saved in your own scenarios folder.
- **Save & play adaptive** saves and starts an adaptive run. **Save** and **Cancel** go back to the scenario's page.
- **Changing settings starts the rating over** with a calibration run (see [Adaptive mode](#help/adaptive)).
- Scenario files are TOML, so you can also edit them in a text editor. SCENARIOS.md in the project documents every field.

## Progress {#progress}

Progress shows how you're doing across all your scenarios, in three tabs.

**Skills**
- **Key figures:** days done this week in the program, minutes, your overall change since you started each drill, and your biggest gain over the last seven days.
- **Skill profile:** the change in each of the nine sub-skills, each group and overall, measured from the end of your first day on each drill.
- **Worth knowing:** the skill furthest behind, skills you haven't trained in a week, and what Saturday's weak-spots day will play.
- **This week:** each Core Week day as done, partial or missed, with runs and minutes, and each day's main drills compared with the same day last week. A day counts as done at 75% of its runs. A day you haven't finished has a **Play** (or **Continue**) button, so a missed day can be made up later in the week.
- **Reps and carry-over:** your first rep against your last rep of each drill, and how much of the previous session's rating carried over to the next.

**Benchmarks** lists your bests on Voltaic Season 5 scenarios from your KovaaK's stats files. Each opens the scenario on Scenarios.

**Time** shows when and how much you train, counting KovaaK's runs and trainer runs together: totals (active time, session time, days trained, days the daily goal was met), daily time for the last seven days, a calendar of daily time (use the arrows to change month), every session newest first, the last seven days as session blocks, time by day of the week, runs by hour of day, and active against idle time. How time is counted is in [Settings › Goals and time](#help/settings).

## Playing KovaaK's scenarios {#kovaaks-play}

AimAdapt is learning to play your own KovaaK's scenarios (`.sce` files) in its trainer, closely enough that scores compare with KovaaK's. It reads the file from your KovaaK's or Workshop folder and never changes it.

**What plays so far:** most scenarios with hitscan weapons, on KovaaK's newer (JSON) and older (Reflex) maps, with bots that stand still or move. That includes 30 of the 34 Voltaic S5 Novice benchmarks. Bots that use abilities (dashes and knockbacks: Pasu, Raw Control, Controlsphere and Ground), projectiles, heads, bots that shoot back and the rest come later.

**Playing one:** on Scenarios › Browse, pick **Type › KovaaK's that play here** and press **Play** on a row, or open a KovaaK's scenario. If AimAdapt can play it, its page has a **Play in AimAdapt** card with **Play**. If the file is there but uses something AimAdapt can't play yet, the page says what. Imported scenarios play in standard mode on the scenario's own map, with its bots, weapon, time limit and scoring, including the accuracy multiplier (some scenarios multiply the score by your accuracy or its square root). If the scenario locks the field of view to a range, your FOV is kept inside it for that run. Bots appear where the scenario's spawn points and areas put them, pushed out of any wall they'd overlap, as in KovaaK's. The scenario's rules on where bots may appear around your view apply too: on 1w4ts a new target never appears right under your crosshair, and on ww5t new targets appear near where you're looking. Bots heal if the scenario gives them health regen.

**How bots move:** as their dodge profiles say: they strafe left and right, come closer or back off to keep their distance from you, jump, fly up and down or use a jetpack, and change direction when hit or blocked, with their character's speed, acceleration, friction and gravity. Bots don't walk off ledges. Scenarios can mix several kinds of bot, each with its own size, health, color and movement, and bots can come in clusters. This is modelled on the settings and isn't checked against KovaaK's yet, so moving targets may feel a little different there.

**Walls:** your shots stop at walls, and bots bump into them. Invisible walls (clips) keep bots in but let your shots through, as in KovaaK's. Cylinders and tubes (round arenas) are solid; ramps, wedges, spheres and other shapes are drawn from their names, so they may look a little different from KovaaK's, and shots and bots pass through them. If you'd start inside the floor, you stand on it instead, as in KovaaK's.

**Your runs:** runs played in AimAdapt are kept apart from your KovaaK's runs of the same scenario. They don't change your KovaaK's bests, trends or benchmark ranks. The card shows how many you've played in AimAdapt and your best.

**What's still different:** the card lists what plays differently from KovaaK's, for example walls that are drawn from their names, or bots' jetpacks, whose timing is a guess. The sizes and distances still have to be checked against KovaaK's, so a score here may not match a score there yet.

## Replay {#replay}

Every run in the trainer can be watched again. On Scenarios, pick a scenario and click **Replay** next to a run in its **Runs**. **Back to** returns to the scenario.

The replay shows your own view: the crosshair stays in the middle and the view turns with it, as it did in the game, at the field of view you played at. The faint dots are a fixed grid, so you can see the view move even with no targets near.

- **Targets** are drawn where they were, at their size. A target lights up while your crosshair is on it, and a white ring shows its health once it's been damaged.
- **Trail:** a green line shows where the crosshair has been in the last 0.4 s, so you can see overshoot and corrections. Turn it off with **Trail**.
- **Shots:** a white ring for a hit and a red cross for a miss, where the crosshair was. The ticks under the bar mark every shot, red for misses, so you can jump to them.
- **Totals** in the corner count kills, shots and hit rate up to that moment. Tracking runs show kills and whether you're on target.
- **Controls:** **Play**/**Pause**, the bar to jump anywhere, and **Speed** (0.25× to 2×). Keys: Space plays or pauses, ← and → skip a second (with Shift, five), `,` and `.` step one frame, Home and End go to the start and the end.

A replay is redrawn from what the run recorded: your aim 240 times a second and the targets 60 times a second. It isn't a video, so it takes little space and records nothing outside the trainer. Runs from before replays were added, and KovaaK's runs, have none.

## Settings {#settings}

Settings has four tabs: **Aim and display**, **Goals and time**, **KovaaK's** and **Your data**.

**Aim and display** holds your profiles. Each profile has its own sensitivity, DPI, field of view, adaptive mix, crosshair and display settings, typically one per game. The trainer plays with the active one (Scenarios shows which); **Use this one** makes the selected profile active. **Add**, **Duplicate** and **Delete** manage them. Changes aren't used until you press **Save settings**; **Undo changes** throws them away.

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

**Goals and time** shows your daily goal (30 minutes active; the weekly goal is seven times it) and how time is counted:
- A new session starts after a 15-minute break.
- Session time runs from a session's first run to its last, so it includes menus and breaks. Active time is time inside runs.
- The training day starts at 4 AM.
- A run under half the usual length of its scenario counts as restarted, and never as a best.

**KovaaK's:** AimAdapt finds KovaaK's in any of your Steam libraries and imports its per-run stats files (FPSAimTrainer\stats). It only reads them and never changes your KovaaK's files. The tab shows the stats folder, how many files it found and imported, any files that couldn't be read, and **Rescan folder**. **Sensitivity history** lists each period with its sensitivity, DPI, FOV, cm/360 and run count. If your FPS sits at KovaaK's cap, it suggests raising the cap.

**Your data** shows where your database and your own scenarios and workouts are (see [Your data](#help/data)).

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
- **No KovaaK's runs.** Settings › KovaaK's shows where AimAdapt looked. It searches every Steam library for FPSAimTrainer\stats. Use **Rescan folder** after installing KovaaK's.
- **The trainer opens in the wrong place.** It opens over the app window, so move the app window where you want the trainer, or use full screen.

## What's new {#whats-new}

**November 2026**
- Many more KovaaK's scenarios play in AimAdapt: bots that move (strafe, keep their distance, jump, fly or use a jetpack), the older Reflex maps, several kinds of bot in one scenario, and bots that come in clusters. 30 of the 34 Voltaic S5 Novice benchmarks play now. Bots that use abilities (Pasu, Raw Control, Controlsphere, Ground) come next.
- In KovaaK's scenarios played here, walls stop your shots and the bots, invisible walls let shots through, and round walls, ramps and other shapes are drawn.
- Fixed: on some KovaaK's maps, walls were drawn far too small (a fraction of their size on maps with a map scale, like SYW's floor).
- Fixed: on KovaaK's maps where you start inside the floor (Midrange Long Strafes), you saw from under it. You now stand on the floor, as in KovaaK's.
- Fixed: KovaaK's scenarios that put you on team 2 (Pasu) had the bots appear around you instead of across the room.
- KovaaK's scenarios played in AimAdapt follow more of their rules: where bots may appear around your view (so on 1w4ts a new target never appears under your crosshair, and on ww5t new ones appear near where you look), and bots' health regen (the Pokeball variants). Your AimAdapt runs of those scenarios from before were easier than the real thing.
- Fixed: bots on some KovaaK's maps (Amare Microcluster) appeared buried in the wall behind them. They're now pushed out to the wall's surface, as in KovaaK's.
- Fixed: KovaaK's scenarios whose bots fly up and down or jump (like SYV) played as still targets. They now fly and jump.
- Fixed: KovaaK's scenarios whose map spawns bots in an area (a spawn volume) put every bot in one spot, so they sat on top of each other and came back in the same place. Bots now appear anywhere in the area, as in KovaaK's. Runs you played in AimAdapt before this fix were easier than the real scenario.
- Browse lists your KovaaK's scenarios too, played or not, with **Play** on the ones AimAdapt's trainer can play.
- Scenarios' lists use the whole page: a table on each tab you can sort by any column, with a search that also finds skills, tags and descriptions, more filters (played or not, made by, favorites only), and a heading for each group. Each scenario and workout opens on its own page, and its back link returns to the list where you left it.
- Play some KovaaK's scenarios in AimAdapt: scenarios with still bots on newer maps, including 11 Voltaic S5 Novice benchmarks. Their page on Scenarios has **Play in AimAdapt**, and those runs are kept apart from your KovaaK's runs.
- Scenarios has four tabs: **Recent**, **Browse** (every scenario you can play, by skill, with tag chips and a sort), **Workouts** and **Yours**. Star favorites, and hide what you don't want on the lists.
- Today shows your favorites, and its recently played list has Play buttons. The skill furthest behind links to its scenarios.
- A simpler layout: four pages instead of ten. **Today** has today's workout and the week; **Scenarios** lists everything you can play, newest first with filters, beside the selected scenario's page (Play, figures, notes, Insights and runs on one scrolling page); **Progress** has Skills, Benchmarks and Time; **Settings** gains Goals and time, KovaaK's (what was Library & import) and Your data. Old links still go to the right place.
- Replays: watch any new trainer run again from its scenario's page, from your own view, at any speed, with your shots and the crosshair's trail.
- The Reaction column in a scenario's runs table leaves out flicks under 3°, like the Reaction time chart, so the two agree in Micro Flicks.
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
