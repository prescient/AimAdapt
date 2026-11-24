# Adaptive play for imported KovaaK's scenarios (BACKLOG 9, Phase 5)

Status: approved Nov 24, 2026; building. Step 1 (the sim) is done.

Today a `.sce` file the trainer can play runs only in standard mode: `aimadapt-engine` refuses `--sce` with `--adaptive`, and the app offers only Play in AimAdapt (standard). This plan lets every playable file run adaptively, with a rating kept per file, so Play on an imported Viscose Easier scenario gives an adaptive run with a rating.

## What the 32 playable Viscose Easier files are

Read on this machine with the importer (Nov 24, 2026):

| Kind | Files | Examples |
|---|---|---|
| Click weapon | 9 | Pasu Angelic 20% Larger 80% Speed, 1w3ts reload Larger, B180 Voltaic Easy 92%, Controlsphere Click Easy |
| Held weapon, bots that stand still | 2 | devTS Goated NR Static 5Bot, Pokeball Frenzy Auto TE Wide |
| Held weapon, bots that move | 21 | Air Voltaic Easy Invincible 4 80%, Controlsphere rAim Easy 90%, Air Pure Intermediate Slower No UFO (ends after 5 kills), tamTargetSwitch Smooth Easy, waldoTS Novice |

None has a bot that leaves on its own (a lifetime, a drain or its own blast), but other playable files do.

The "80%" and "Slower" versions are KovaaK's `Timescale`: Pasu Angelic 80% Speed has `Timescale=0.8` and a 48 s timer, and B180 Voltaic Easy 92% has 0.782 and 46.92 s. In both the timer is the original 60 s times the scale, so the run still takes 60 real seconds. The importer reads `Timescale` already, and the sim plays it as the rate of the whole game clock (`World::time_scale`).

## The knob

Two knobs, picked from what the file plays. Nothing is added to the imported scenario (its fingerprint, and so every standard best, stays as it is): the choice is made when the run starts, from `Knob::Auto` on a scenario with a world.

**Speed** (new `Knob::TimeScale`, stored as `time_scale`, shown as `×0.85`, higher is harder). 1.0 is the file as written, its own `Timescale` included. A run at ×0.85 runs the game clock at the file's scale times 0.85, so everything on the game clock slows together, as KovaaK's `Timescale` does: bot movement (speeds, acceleration, gravity, jumps, fliers), abilities and their timers, helper pushes, the weapon's fire rate, regen and drains, respawn delays, lifetimes and fuses. The run's game length is scaled with it, so a run takes the file's real length at any speed, as the community's 80% files do. A kill limit still ends a run. Range ×0.25 (easy) to ×2.5 (hard), start ×1.0, narrowed where needed so the file's scale times the value stays in the sim's 0.1–10.

**Kill pace** (the existing `Knob::KillPace`, seconds between kills, lower is harder) for files where a faster clock wouldn't make the drill harder. With still bots it only speeds up the weapon and respawns, which makes a held weapon easier. With a click weapon and no timer there is no bar a single kill has to clear, and the pace is what KovaaK's scores (kills a minute). The file plays at its own speed and the windows are real seconds. Range 4.0 s (easy) to 0.2 s (hard), start 1.2 s.

How each maps onto `aimadapt-sim::adaptive`: `Adaptive::resolve` picks the knob and its range for a scenario with a world; `Rating`, the mix, calibration, breakthroughs and the guard rails are used unchanged.

## What counts as a success

The goal stays 85%. Rules in this order, the first that fits:

1. **Timed** (some bot can leave on its own: a lifetime, a drain or its own blast). Speed. Each bot is one outcome: a kill is a success, a bot that times out is a failure. With a click weapon a kill after more than one miss on that bot fails, as in Timed Flicks.
2. **Kill pace** (a click weapon, or no bot that moves by itself or is pushed). Kill pace. A kill inside the window is a success and every window that runs out is a failure, as in Static Grid. With a click weapon a kill after more than one miss since the last kill fails. With a held weapon there's no miss rule, because a held trigger fires between targets.
3. **Tracking** (a held or always-firing weapon and bots that move: tracking, kill-limit tracking and switching). Speed. Each 2-second segment of real time is one outcome, the share of 60 Hz ticks with the aim on a part of a bot that counts, firing or not, as with the beam. Ticks count only while a bot is up, so a respawn gap isn't the player's. Segments weigh 3, as in built-in tracking. In switching the flick to the next bot is part of the share, as it is in KovaaK's damage score.

For Viscose Easier that is 11 files on kill pace and 21 on speed. At 85% on target the speed rating will often start under ×1.0 on the tracking files: it reads as "you stay on target 85% of the time at 0.8 times the file's speed".

**Calibration.** The first run is a calibration run, as for built-ins. Kill pace already sets each window as it goes. For speed, the clock rate changes at each segment (every 2 s), and for timed files each bot's outcome is at the rate when it appeared. The sim keeps each rate change, so the saved run, its kill times and its replay stay in real seconds. Every later run plays at one speed.

The sim also needs tracking ticks for held hitscan weapons, which every imported tracking file uses: today only the beam feeds tracking segments.

## How ratings are keyed

Adaptive runs of an imported file are stored as built-in adaptive runs are: mode `RustyAim Adaptive`, history name `<name> (Adaptive)`, `engine_runs.base_scenario` the KovaaK's name, `runs.scenario_hash` the imported scenario's fingerprint (which holds the file's hash). No schema change.

- The next run starts from the last rating with the same name, fingerprint and knob. A changed file, or a sim rule change that changes the fingerprint, starts over with a calibration run, as an edited built-in does.
- Standard runs stay `RustyAim Imported` under the plain name, so standard bests never see a rating. The best held is per fingerprint.
- KovaaK's history (Progress, Benchmarks, bests) already leaves out both modes, and no KovaaK's run feeds the rating.
- Imported scenarios have no sub-skill, so their ratings stay out of Skills and the program figures. To check while building: every query that reads adaptive runs (plan check, check-ins, Insights overview, program recaps).

## What the app shows

- **A KovaaK's scenario's page** (`sceCard`): Play (adaptive, the main button) and Standard run, as on an AimAdapt scenario's page. The card shows the rating with its knob (`Speed ×0.82` or `Kill pace 0.91 s`), what it means in one line, the adaptive runs, and the standard runs line it shows now.
- **Progress › Benchmarks › Viscose Easier** (`viscoseCard`): Play (adaptive) and a smaller Standard on each playable row, and a Rating column beside the KovaaK's best.
- **Scenarios › Browse**: its Play on a playable file starts an adaptive run, since Play always means adaptive (plan L1).
- **The trainer**: `--sce <file> --adaptive` plays as a built-in adaptive run does, with the HUD, the kind of run ("At your level: ×0.82") and the results screen.
- The user guide (KovaaK's and Adaptive mode sections, What's new), README, SCENARIOS (knobs), TESTING and BACKLOG change with it.

## Steps

Each is one commit through the hook, pushed, with Windows CI checked.

1. **Sim.** `Knob::TimeScale`; the knob and range for a world; a game clock whose rate can change in a run, ending on real time, with results in real seconds; tracking ticks for held hitscan weapons; the three success rules. Unit tests on made-up worlds, and fixture tests on new `AimAdapt Test - <name>.sce` files: a click file, a held weapon with still bots, a held weapon with a strafing ball, and a timed file (Short Fuse already is one).
2. **Trainer.** `--sce --adaptive`; the knob in core's copy, the stored id and the HUD; the bot test holds fire for held weapons so it kills. Checked with `--sce <fixture> --adaptive --smoke <dir> --db <scratch>` and on Viscose Easier files read in place.
3. **App.** `play_sce` with an adaptive flag, the rating on `sce_info`, the buttons and the Rating column; UI tests; guide, README and the rest.

## Left out

- Starting a file's first run from its KovaaK's best (a kill pace from the best score, say). The calibration run finds the level in about 20 outcomes.
- Imported scenarios in workouts, programs and Skills (Phase 6 and later).
- Kill limits in adaptive runs stay as written. Air Pure at ×0.5 is a long run.
