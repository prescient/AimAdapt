# Plan: changes from the November 2026 reviews

Two reviews were written on 10 November 2026. One looked at how the app is laid out and how a player moves through it. The other looked at the adaptive rating, how runs are picked and what the skill measures claim. This plan turns their findings into work packages.

Each package is small enough to build, check and commit on its own. The papers behind the adaptive decisions are in [docs/research/papers](../research/papers/README.md).

## Status

Finished on 10 November 2026. Each package has its own commit, and so does the final pass.

| Package | Commit |
|---|---|
| L1 One meaning for Play | 179e294 |
| A1 Steadier ratings | a5ac46a |
| L2 Scenario page tabs | 61ce281 |
| A3 Scenario versions kept apart | 13bb26b |
| L4 Program tab, Progress trimmed | 705ec9b |
| A8 Stricter breakthroughs | e0943ce |
| L6a Browse takes in Recent and Yours | 540c900 |
| A4 Calibration after a break | 2604f35 |
| L6b Help, narrow sidebar, editor folds | cef17bc |
| A5 Planned order inside a block | b952977 |
| A2 Name the run and judge it against its plan | 5e44a30 |
| A6 Planned against actual success | 3ac8e6c |
| A7 Ranges and noise on derived measures | 17239c1 |
| L5 Settings › Training, with an editable daily goal | 6d38a7b |
| L3a Results screen in the trainer | b6c9791 |
| L3b Session summary and "Since your last session" | e245ade |

Still to check by hand, because the bot and the browser preview can't: closing the trainer in the built app and landing on the session summary, and how long the summary takes to build on a large history.

## What is out of scope

Decided earlier and not part of this plan:

- A variation or "technique training" mode.
- Fixed-settings assessments with an overall rank. Voltaic and Viscose benchmarks stay the yardstick.
- Share codes and leaderboards.

Left as it is on purpose: the separate trainer window, the size of the Today page, hash routes, the no-build UI and the page names.

## How the work is run

- One package at a time per lane. The Rust lane (sim, engine, core) and the UI lane (`crates/aimadapt-app/ui`) can each have one package open, because they touch different files. Packages that need both run alone.
- Each package is built by a smaller model from the package text below plus a short brief. The result is reviewed before it is committed: the diff is read, a bug fix must show its test failing on the old code, the pre-commit hook runs on the whole tree, and UI changes are looked at in the browser preview against bot data.
- One commit per package, through the pre-commit hook. Nothing is pushed.
- Every package follows [AGENTS.md](../../AGENTS.md): a failing-first regression test for a bug, and the README, the user guide (with a What's new line), BACKLOG, TESTING and SCENARIOS kept in step.
- Test and demo data come from the bot (`--smoke`) and a scratch database (`--db`, `--no-import`). No real history is opened.

## Order

| Step | Rust lane | UI lane |
|---|---|---|
| 1 | A1 Steadier ratings | L1 One meaning for Play |
| 2 | A3 Scenario versions kept apart | L2 Scenario page tabs |
| 3 | A8 Stricter breakthroughs | L4 Program tab, Progress trimmed |
| 4 | A4 Calibration after a break | L6a Browse takes in Recent and Yours |
| 5 | A5 Planned order inside a block | L6b Help, narrow sidebar, editor folds |
| 6 | A2 Name the run and judge it against its plan | |
| 7 | A6 Planned against actual success (both lanes) | |
| 8 | A7 Ranges and noise on derived measures (both lanes) | |
| 9 | L5 Settings › Training, with an editable daily goal (both lanes) | |
| 10 | L3a Results screen in the trainer | |
| 11 | L3b Session summary and "Since your last session" (both lanes) | |
| 12 | Final pass: README screenshots, docs, backlog | |

## Adaptive packages

### A1. Steadier ratings

**Problem.** Between runs the model forgets how sure it was about the slope (how steeply success falls as a drill gets harder) and starts every run with the same wide guess. The carried uncertainty is also applied to the wrong point of the curve. A player whose skill has not changed sees the rating move about 6% from run to run.

**Change.**
- Save the slope's uncertainty with each run and start the next run from it. Runs saved before this change have none, so they fall back to today's value. No rating is reset.
- Apply the carried uncertainty to the goal point (the value the player holds at the goal rate), which is the number it was measured on.
- New nullable column `engine_runs.slope_sd`, added by a migration.

**Tests.** A regression test with a simulated player of fixed skill who plays a chain of runs: the rating's average run-to-run movement must stay under a bound that the old code breaks. A second test checks the rating still follows a real change: after a 10% improvement, most of the step shows within five runs.

**Expected effect** (from the review's simulation): run-to-run movement 5.9% → 2.8%, error against the true value 4.8% → 3.4%. The cost is a slower response to a real change: a 10% step shows 36%, 78% and 93% after one, three and five runs, where it showed 58%, 96% and 100%.

**Docs.** Guide: the adaptive section and What's new. README: the adaptive section if it describes how the rating moves.

### A2. Name the run and judge it against its plan

**Problem.** A stretch run is planned to succeed about 6 times in 10, and the results screen then says "Below the 85% goal this time". The run went as planned and reads as a failure.

**Change.**
- Before the run, name its kind with what to expect: "Stretch run: 0.52 s, about 6 in 10 expected", "Easy run: 0.71 s, about 19 in 20 expected". Runs at the player's level keep the 85% wording.
- After the run, judge it against its plan: "Stretch run: 58% at 0.52 s, as planned", or "above plan", or "below plan". "As planned" covers the luck of a run that length (binomial noise for the number of targets), with a floor of five points.
- A breakthrough still leads the results when there is one.
- The wording comes from one function with no graphics in it, so it can be unit tested.

**Tests.** Unit tests on the verdict for each kind of run, at, above and below plan, and for short runs. The screen itself is checked by eye with the bot.

**Docs.** Guide: trainer and adaptive sections, What's new.

### A3. Scenario versions kept apart (bug)

**Problem.** A rating belongs to one version of a scenario: change the scenario's settings and the rating starts over, because the old number is on a different scale. The trainer respects that. The program figures and Insights group runs by scenario name and rated setting only, so a trend, a gain or a best can mix versions. Found by reading the queries. It has not been reproduced yet.

**Change.** Write the failing tests first: two versions of one scenario with different ratings, then check the trend, gains, bests and notes. If the tests show the bug, use only the scenario's current version (the version of its latest run) for anything that compares ratings. The Runs table keeps listing every run. If the tests pass on today's code, report that and stop.

**Tests.** Unit tests in `programs.rs` and `insights.rs`, named for the bug.

**Docs.** Guide: a line where versions are explained, and What's new.

### A4. Calibration after a break

**Problem.** After a long break the old rating is a poor guess, and a run played at one fixed value from that guess teaches the model little. BACKLOG had this as "offer, not force" from the first playtest.

**Change.** When a rating was last set three or more days ago and its uncertainty, widened for the time away, has reached the level a new scenario leaves calibration at (0.12 in log units), the next run is a calibration run. For a settled rating that takes about five and a half days. The run starts from the old rating and adapts target by target. The trainer says why: "Calibration run: 9 days since your last one, finding your level again."

The review recommended making this automatic with a clear line on screen, because an offer needs a prompt in the middle of a session. This replaces the backlog's "offer" item.

**Tests.** Unit tests on the rule (days and uncertainty on both sides of the line) and a sim test that a run after a long break is a calibration run that starts near the old rating.

**Docs.** Guide: adaptive section, What's new. PLAYTEST.md keeps its history; BACKLOG records the change of approach.

### A5. Planned order inside a block

**Problem.** Each run's kind is drawn at random, so a block of five can open on a stretch run or close on one.

**Change.** Inside a workout or program block of two or more runs, the kinds follow a plan built from the mix:
- The first run is at the player's level.
- Stretch runs sit in the middle. They are never next to each other and never last.
- An easy run, when the mix gives one, goes last.
- The number of each kind comes from the mix's shares. Five runs on Balanced: at level, at level, stretch, at level, easy.
- The guard rails stay: a stretch run is swapped for one at level after a stretch run or a run well below the goal.
- Single runs outside a block keep the random draw.

**Tests.** Unit tests over every mix and block lengths of 2 to 8: the first is at level, the last is never a stretch, no two stretches touch, the counts match the shares.

**Docs.** Guide: adaptive and workout sections, What's new.

### A6. Planned against actual success

**Problem.** Nothing shows whether runs come out as planned. If stretch runs planned at 60% keep landing at 45%, the model is off and nobody can see it.

**Change.**
- A table of planned against actual success by kind of run (at level, stretch, easy): number of runs, planned rate, actual rate and the gap, marked when the gap is larger than luck explains. Shown for one scenario on its Runs tab and for everything on Progress › Skills.
- The Runs table shows each run's kind and marks breakthroughs.
- The same table from the command line, so it can be checked against any database.

This also closes the backlog item on showing the split of run kinds and breakthroughs.

**Tests.** Unit tests on the aggregation with made-up runs, including a kind with no runs and a gap inside the noise.

**Docs.** Guide: scenarios and progress sections, What's new. README: Insights.

### A7. Ranges and noise on derived measures

**Problem.** Ratings come with a range. The numbers built from them (gains per drill, carry-over, the skill profile, weak spots) are shown bare, and a change smaller than the noise is presented like a real one.

**Change.**
- Gains and changes carry a range built from the ratings' own ranges.
- A change inside the noise is greyed and labelled as such.
- Weak spots and carry-over are named only after at least three sessions on the drills involved. Before that the page says how many more are needed.
- Where a measure is about gain, the wording is "least improved". "Weakest" is kept for measures of level.

**Tests.** Unit tests: a change inside the noise is flagged, one outside it is not, and nothing is named a weak spot on fewer than three sessions.

**Docs.** Guide: progress and program sections, What's new.

### A8. Stricter breakthroughs

**Problem.** A breakthrough needs the goal rate held over a run of only ten targets. At ten targets, nine hits is 90% and mostly luck. A lucky short run also sets the "best held" value, which then hides a real breakthrough later.

**Change.** Holding the goal needs the goal rate over 30 or more targets. A shorter run (10 to 29) must beat the goal by a margin of one binomial standard deviation for its length, so 10 of 10, or 19 of 20. The same rule decides both the breakthrough and the best held value. Stored bests are left as they are.

**Tests.** Unit tests: 9 of 10 does not hold the goal, 10 of 10 does, 26 of 30 does. The first fails on the old code.

**Docs.** Guide: adaptive section (what a breakthrough is), What's new.

## Layout packages

### L1. One meaning for Play

**Problem.** "Play" starts an adaptive run in one place, a standard run in another and a whole session in a third.

**Change.**
- Play always means an adaptive run of one scenario. "Standard run" is always the secondary button.
- Workouts and program days use "Start session" and "Continue session".
- Small text under the button says what will happen ("adaptive · 60 s").
- A KovaaK's scenario that plays in AimAdapt has no adaptive mode yet, so its one button stays "Play in AimAdapt".

**Docs.** Guide: every place a button is named. README text where it names buttons. Screenshots are retaken in the final pass.

### L2. Scenario page tabs

**Problem.** The scenario page is four pages stacked in one scroll: the summary, the last run's charts, the full run table and the editor link.

**Change.**
- One header: name, rating with its range, best held, the play buttons.
- Tabs under it, replacing the jump links: **Overview** (rating trend, the last five runs with Replay links, the top three notes), **Insights** (every note, then the charts from the last ten runs) and **Runs** (the full table). The middle tab was first planned as "Last run"; its charts cover the last ten runs, so it keeps the name Insights.
- Addresses such as `#scenarios/<id>/runs`, so a tab can be linked and survives a reload. Old addresses still land somewhere sensible.
- A KovaaK's scenario's page uses the same header and tabs and hides the tabs it has no data for.
- The editor stays where it is.

**Docs.** Guide: scenarios section, What's new.

### L3a. Results screen in the trainer

**Problem.** The run ends in the trainer, and the feedback about it lives in the app. Most players close the results and never see it.

**Change.** The results screen gains three things:
- A small chart of the scenario's last ten or so ratings with this run marked.
- The single most relevant note for this run.
- Keys: next run, watch the replay, details in the app. The last two close the trainer and open the app on the replay or on the scenario's page.

**Tests.** The data behind the chart and the choice of note are unit tested. The drawing is checked by eye with the bot, and the commit says so.

**Docs.** Guide: trainer section (keys and results), What's new. README: Trainer, and its results screenshot.

### L3b. Session summary and "Since your last session"

**Change.**
- When the trainer closes, the app lands on a short summary of the session just played: each scenario's rating change with its range, breakthroughs, time played, and the top note.
- Today gains "Since your last session": rating changes and breakthroughs since the session before, and each program drill's change against the same day last week.

This closes two backlog items: the results chart (with L3a) and Today's "Since your last session".

**Tests.** Unit tests on the summary's figures. A UI test that the pages name the commands they call.

**Docs.** Guide: today and trainer sections, What's new. README: The app.

### L4. Program tab, Progress trimmed

**Problem.** The program has no page of its own. Its week sits on Progress › Skills between results, with start buttons, and its day workouts crowd the Workouts list.

**Change.**
- Scenarios gains a **Program** tab beside Workouts: the week as a row of days, each day's drills and run counts, and a start button on each day.
- Core Week's day workouts leave the general Workouts list and show under the program.
- Today keeps its one card for today.
- Progress › Skills drops the start buttons and leads with the skill profile and carry-over.
- Benchmarks is labelled "Benchmarks from KovaaK's" and explains itself when empty.
- Progress › Time keeps daily time, the month view and sessions. The other four cards go behind a "More about when you train" fold.
- Notes are ranked once. Today shows the top note and Progress shows the full list.

**Docs.** Guide: program, scenarios, progress and today sections, What's new. README: Program.

### L5. Settings › Training

**Problem.** The mix (Steady, Balanced, Push) sits in the Aim card, the goals sit in a tab that cannot be edited, and the guide gives half an hour as if it were a limit.

**Change.**
- A **Training** tab in Settings holds the mix, the daily goal and the advice on session length. It replaces "Goals and time".
- The daily goal can be edited (minutes a day). The other time rules stay fixed for now and stay listed.
- Today's card and the scenario header show the current mix and link to the tab.
- The guide's advice is reworded to what the evidence supports: half an hour is a sound default and there is no sign that it is a ceiling. In the largest study of aim training, gains on a single task kept growing up to about an hour in a day, with most of the benefit reached between 30 and 60 minutes, and practising on more days mattered more than the length of any one day.

**Tests.** Unit tests on saving and loading the goal. UI tests for the tab and its old address.

**Docs.** Guide: start, settings and today sections, What's new. README: Scenarios, workouts and settings. BACKLOG: the editable goals item is part done.

### L6a. Browse takes in Recent and Yours

**Change.**
- The Scenarios tabs become Browse, Workouts and Program. Browse opens sorted by most recently played and has filters for played and for your own scenarios. Old addresses for Recent and Yours open Browse with the matching filter.
- The "Best score" column shows the rating for AimAdapt scenarios, since that is the number the rest of the app uses.

**Docs.** Guide: scenarios section, What's new. README: Scenarios.

### L6b. Help, narrow sidebar, editor folds

**Change.**
- Help: the contents sit in a rail beside the guide with a filter box, and every page has a "?" link to its section. Whatever of this exists already is kept.
- Under about 1,100 px the sidebar shows icons only.
- In the scenario editor, sections whose fields all hold defaults start folded.

**Docs.** Guide: the sections for Help and the editor, What's new.

## Final pass

Done on 10 November 2026, in one commit.

- Seven small fixes left over from the packages, done by a cheaper model and reviewed: a rest day on Progress says so
  once, the Time page's last hour label stays inside its box, a workout's page finds its drills the way the Program
  tab does, the sidebar stays in view on a long page, the Accuracy tile names the runs it covers, a gap in Planned
  against actual reads "9 pts above plan" in the app and in `plan-check`, and the weak-spots day says where its drills
  come from.
- Every README screenshot was retaken from a history the bot played into a scratch database, spread over two weeks.
  Two pictures are new: a scenario's page and the session summary.
- Retaking the pictures turned up eight display bugs, each fixed with a test that failed first: a "-0.0" change shown
  in red on a key figure, rating ticks that read the same, a Runs count that left out older versions, a "run 5.5" on
  the rating chart, a dropped Overshoot column, a "40-180°" distance row, a speed label cut off at the chart's edge,
  and a "-0%" change on Progress.
- The first set of pictures was thrown away. Three runs from a real KovaaK's stats folder had been imported into the
  scratch database by a step that ran without `--no-import`. They were removed, the demo data was rebuilt, and every
  picture was taken again. Use `--no-import` with the app and `--stats <empty folder>` with the CLI on any scratch
  database.
- The README and the guide were reread against the app, BACKLOG's finished items moved to Done, and "Last reviewed"
  set.
