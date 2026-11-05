# Scenario files

AimAdapt's built-in trainer plays scenarios written as TOML files. The built-in scenarios live in [`crates/aimadapt-sim/scenarios`](crates/aimadapt-sim/scenarios). Your own scenarios go in `%APPDATA%\AimAdapt\scenarios`. The Play screen's editor writes them there, and you can also edit the files by hand.

Every field is optional. A missing field takes the default shown below, so a file only needs the values it changes. Unknown fields are rejected, which catches typos like `radiuss`. The app's editor is built from the same field list ([`crates/aimadapt-sim/src/spec.rs`](crates/aimadapt-sim/src/spec.rs)), and a test checks that this page covers every field.

## Example

```toml
name = "Micro Flicks"
description = "Tiny targets that appear just next to the last one."
category = "clicking"
tags = ["flick", "micro"]

[timing]
duration_s = 60.0

[targets]
count = 1
radius = 0.12

[targets.spawn]
distance = [10.0, 10.0]
yaw = [-12.0, 12.0]
pitch = [-6.0, 10.0]
min_separation = 1.5
max_from_last = 6.0

[scoring]
per_kill = 100.0
per_miss = -30.0
```

## How space works

- The player stands still in the middle of the room, facing the far wall. Their eye is the origin.
- Targets are placed by angle and distance. `yaw` is degrees left (−) or right (+) of straight ahead, and `pitch` is degrees below (−) or above (+) eye level.
- Moving targets travel in angle space and bounce off the edges of the spawn area and the room. They keep one distance (and on-screen size) unless they move forward and back, and they can jump straight up from where they are.
- A sphere of radius `r` at distance `d` is `2 × atan(r / d)` degrees wide. For example, a 0.3 m target at 10 m is about 3.4° wide.
- Your sensitivity (cm/360 and DPI), FOV and crosshair are player settings, not part of the scenario. They're stored in `%APPDATA%\AimAdapt\engine.toml`.

## Fields

`[min, max]` ranges pick a new value in between for each spawn or direction change. Use the same number twice for a fixed value.

### About

| Field | Default | Meaning |
|---|---|---|
| `name` | `"Untitled"` | Shown in lists. Your history is grouped by name, plus a fingerprint of the settings, so harder or easier variants are kept apart. |
| `description` | `""` | What the scenario trains and how to play it. |
| `author` | `""` | Who made it. |
| `category` | `"clicking"` | `clicking`, `tracking` or `switching`. |
| `skill` | `"none"` | The sub-skill it trains, for your skill profile and programs: `static_clicking`, `dynamic_clicking`, `linear_clicking`, `precise_tracking`, `reactive_tracking`, `control_tracking`, `speed_switching`, `evasive_switching` or `stability_switching`. It must match the category. |
| `tags` | `[]` | Short words for searching. |

### `[timing]`

| Field | Default | Meaning |
|---|---|---|
| `timing.duration_s` | `60.0` | Length of the run in seconds (5–3600). |
| `timing.countdown_s` | `3.0` | Countdown before the clock starts. Targets are visible but can't be hit. |
| `timing.kill_limit` | `0` | End early after this many kills. 0 means no limit. |

### `[weapon]`

| Field | Default | Meaning |
|---|---|---|
| `weapon.kind` | `"hitscan"` | `hitscan`: instant shots along the crosshair. `beam`: continuous damage while on target, sampled 60 times a second. |
| `weapon.trigger` | `"click"` | `click`: one shot per click. `hold`: fires while held. `always`: always firing, so only aim matters. |
| `weapon.fire_rate` | `0.0` | Hitscan: most shots per second. Required for `hold`. 0 means no cap with `click`. |
| `weapon.damage` | `100.0` | Hitscan damage per hit. |
| `weapon.damage_per_s` | `100.0` | Beam damage per second on target. |

### `[targets]`

| Field | Default | Meaning |
|---|---|---|
| `targets.count` | `1` | Targets alive at once (1–50). |
| `targets.shape` | `"sphere"` | `sphere`, or `capsule` (an upright pill, roughly a body). |
| `targets.radius` | `0.3` | Radius in meters. |
| `targets.height` | `1.8` | Capsule height in meters, including the rounded ends. |
| `targets.health` | `100.0` | Damage needed to destroy a target. Equal to `weapon.damage` means one shot. Use a huge value for pure tracking. |
| `targets.regen_per_s` | `0.0` | Beam only: health restored per second while the crosshair is off the target. |
| `targets.regen_delay_s` | `0.0` | Seconds after the last hit before a target starts healing. A short delay forgives a brief slip; regen close to the beam's damage rate means a kill needs an unbroken stretch on target. |
| `targets.color` | `"#ff5a4e"` | Target color as `#rrggbb`. |
| `targets.respawn_delay_s` | `0.0` | Seconds before a destroyed target is replaced. |
| `targets.lifetime_s` | `0.0` | A target disappears as a timeout after this many seconds. 0 means it stays until it's destroyed. |
| `targets.shrink_to` | `1.0` | Size at the end of its lifetime, as a share of the radius. Needs `lifetime_s`. |
| `targets.timer_bar` | `true` | Show a bar above each target that times out, emptying as its time runs out. |
| `targets.alternate_colors` | `true` | Targets that time out change color by wave: the first `count` targets use `color`, the next `count` use `alt_color`, and so on. With three targets up, the grid turns over from three of one color to three of the other, so a target's color says which wave it's from. |
| `targets.alt_color` | `"#4da3ff"` | The second color. The default blue against the default orange-red stays distinct for common kinds of color blindness. |
| `targets.health_bar` | `false` | Show each target's health in a bar above it. Useful with `regen_per_s`, so you can see a target healing. |

### `[targets.spawn]`

| Field | Default | Meaning |
|---|---|---|
| `targets.spawn.distance` | `[10.0, 10.0]` | Meters from the player. |
| `targets.spawn.yaw` | `[-25.0, 25.0]` | Degrees left/right. Moving targets stay inside this too. |
| `targets.spawn.pitch` | `[-5.0, 15.0]` | Degrees down/up, within ±85. |
| `targets.spawn.min_separation` | `4.0` | Smallest angle between a new target and any live one. Large values force wide flicks. |
| `targets.spawn.max_from_last` | `0.0` | Largest angle from the last destroyed target. Small values make micro-flicks. 0 means no limit. |

### `[targets.movement]`

| Field | Default | Meaning |
|---|---|---|
| `targets.movement.kind` | `"static"` | `static`, `strafe` (side to side, reversing at random intervals), `random` (a new direction at each change) or `curve` (steady speed along curved paths: the heading keeps turning, and each change picks a new turn rate and side). |
| `targets.movement.speed` | `[3.0, 5.0]` | Meters per second at the target's distance. |
| `targets.movement.change_every_s` | `[0.4, 1.2]` | Seconds between direction changes. |
| `targets.movement.vertical` | `0.5` | `random` only: 0 keeps movement level and 1 allows any direction. |
| `targets.movement.turn_deg_s` | `[0, 0]` | `curve` only, and required there: how fast the heading turns, in degrees per second. Each direction change picks a new rate from this range, turning left or right. |
| `targets.movement.accel` | `0.0` | How quickly a target changes velocity, in m/s². 0 reverses and turns instantly. With a value, a strafing target slows down, stops and speeds back up at each reversal, and `random` turns become curves. |
| `targets.movement.approach_speed` | `[0.0, 0.0]` | Speed toward and away from the player, within `spawn.distance` (which then needs min < max). Sideways speed stays the same in meters per second, so a closer target crosses your view faster and looks bigger. |
| `targets.movement.approach_every_s` | `[0.0, 0.0]` | Seconds between coming closer and backing away. `[0, 0]` uses `change_every_s`. |
| `targets.movement.jump_every_s` | `[0.0, 0.0]` | Seconds on the ground between jumps. |
| `targets.movement.jump_height` | `0.0` | Height of each jump in meters. 0 means no jumps. |
| `targets.movement.jump_air_s` | `0.0` | Seconds in the air per jump. Gravity is 8 × height ÷ airtime², so a 1.2 m jump lasting 0.7 s falls at about 20 m/s². |

### `[scoring]`

Score = kills × `per_kill` + hits × `per_hit` + misses × `per_miss` + damage × `per_damage` + timeouts × `per_timeout`.

| Field | Default | Meaning |
|---|---|---|
| `scoring.per_kill` | `100.0` | Points per destroyed target. |
| `scoring.per_hit` | `0.0` | Points per hit. For beams, a hit is one 1/60 s tick on target. |
| `scoring.per_miss` | `0.0` | Usually 0 or negative. |
| `scoring.per_damage` | `0.0` | Points per point of damage. Tracking scenarios use 1. |
| `scoring.per_timeout` | `0.0` | Usually 0 or negative. |

### `[adaptive]`

Every scenario can be played in **Standard** mode, at the difficulty written in the file, or **Adaptive** mode. Adaptive mode rates you on one setting, the `knob`, in real units:

- **Target lifetime** (seconds) for timed clicking. Your rating might be "0.62 s": you hit 85% of targets that last 0.62 s.
- **Target speed** (degrees per second across your view) for tracking. "48 °/s" means you stay on target 85% of the time at that speed.
- **Target size** (radius in meters) where precision is the aim. Micro Flicks rates the smallest target you hit 85% of the time within a fixed 1.5 s, and Precise Track the smallest ball you stay on 85% of the time at a fixed slow speed.
- **Kill pace** (seconds between kills) for speed switching and for clicking with no timer or several targets up (Static Grid, Timed Grid, Wide Flicks). "1.50 s" means 85% of your kills come within 1.5 s of the one before, which is about 45 kills a minute. A lifetime rating with several targets up would measure the queue: each timer keeps running while you shoot the others.

**How the rating works.** The trainer models your chance of success at each value as an S-shaped curve. It keeps a probability estimate of where that curve sits and how steep it is, so it knows how sure it is, much like the Glicko version of Elo. Each outcome updates the estimate by how surprising it was. Succeeding at a value harder than your rating moves it more than succeeding at your level, and failing at a harder value costs less than failing at your level. The next run starts from your last rating on the same version of the scenario. Any change that affects play (its fingerprint) starts over with a calibration run, because a rating earned under other settings isn't on the same scale. The uncertainty grows with the days since you played, so after a break the rating moves more again.

- **Clicking:** a target is a success if you destroy it before it times out with at most one miss on it. Spraying clicks doesn't count. A timeout is a failure. With a hitscan weapon, lifetime, size and target speed are all set target by target, and size and target speed need `targets.lifetime_s` (without a timer a careful player would never fail). Strafe Clicks rates target speed this way.
- **Finished in time:** with a beam, `targets.lifetime_s` and the target-speed knob, each target is one outcome: a success if you finish it before it times out. Each target gets its own speed. With fast regen after a short `regen_delay_s`, that means unbroken tracking (Unbroken Strafes, Unbroken Float).
- **Tracking:** each `segment_s`-second segment is one outcome: the share of that time the crosshair was on target, whether or not you're firing. With `engaged_only`, only time while a target is damaged and not yet healed counts, so the trip to a fresh target after a kill isn't off target, but leaving a half-killed target is, until it heals.
- **Kill pace:** a kill within the pace of the one before is a success. Each full stretch of that length with no kill is a failure, and the late kill that ends it doesn't count as a success. With a hitscan weapon, a kill after more than one miss since the last kill doesn't count either. Speed Switch, Static Grid and Wide Flicks have no target timers. Timed Grid keeps its own 2 s timers, which don't change with the rating.

**Calibration run, then one difficulty per run.** The first adaptive run on a scenario is a calibration run. It sets each target's (or segment's) difficulty as it goes, between 65% and 90% chance of success, to find your level within the run. The game labels it "Calibration run". Every later run plays at one fixed difficulty for the whole run, shown as "This run: …" before you start, and the rating takes in the run as you play. Each run's difficulty is picked from a mix: first a chance of success, then the value with that chance on your current curve.

| Run | Chance of success | Balanced | Steady | Push |
|---|---|---|---|---|
| At your level | 80–90% | 65% | 85% | 50% |
| Stretch | 55–72% | 20% | 5% | 35% |
| Easy | 93–97% | 15% | 10% | 15% |

A stretch run never comes straight after another stretch run, or after a run more than 10 points below the goal. Holding the goal rate (at least 10 targets or segments) for a whole run at a value harder than your best counts as a breakthrough. The game announces it at the end of the run, and the value is saved as your best held. The mix is a setting in your profile.

| Field | Default | Meaning |
|---|---|---|
| `adaptive.goal` | `0.85` | Your rating is the value you succeed at this share of the time (0.6–0.95). |
| `adaptive.knob` | `"auto"` | `lifetime`, `target_speed`, `radius` or `kill_pace`. `auto` uses lifetime for hitscan, target speed for moving beam targets, and size otherwise. |
| `adaptive.easy` | `0` | Easiest value it will use, in the knob's units. 0 for a default (3 s lifetime, 3 °/s, 2× the radius, 3 s kill pace). |
| `adaptive.hard` | `0` | Hardest value it will use. 0 for a default (0.25 s, 180 °/s, 0.3× the radius, 0.3 s kill pace). |
| `adaptive.start` | `0` | First guess at your rating, for the calibration run. 0 for the scenario's own lifetime, speed or size (1.5 s for kill pace). |
| `adaptive.segment_s` | `2.0` | Tracking: seconds per outcome. |
| `adaptive.engaged_only` | `false` | Tracking: only count time while a target is damaged and not yet healed. For switching, with `targets.regen_per_s` so an abandoned target heals. |

```toml
[adaptive]
goal = 0.85
knob = "lifetime"
easy = 3.0
hard = 0.3
start = 1.1
```

### `[arena]`

| Field | Default | Meaning |
|---|---|---|
| `arena.size` | `[60.0, 24.0, 60.0]` | Room width, height and depth in meters. Spawn distance must fit inside it. |
| `arena.eye_height` | `1.7` | Eye height above the floor. |
| `arena.wall_color` | `"#3a3f4b"` | Wall and ceiling color. |
| `arena.floor_color` | `"#2a2e37"` | Floor color. |

## What gets recorded

Each finished run is saved to the same database as KovaaK's runs, with `source = 'rustyaim'` and KovaaK's field names. That includes score, hit and miss counts, kills with TTK, damage, sensitivity (as cm/360), DPI, FOV, average FPS and resolution. The `Hash` field holds the scenario's fingerprint, which changes when any gameplay setting changes. For beam weapons, hits and misses count 1/60 s ticks, which matches how KovaaK's reports tracking accuracy.

## Workouts

A workout is a TOML file listing scenarios to play back to back. Built-ins live in [`crates/aimadapt-sim/workouts`](crates/aimadapt-sim/workouts). Yours go in `%APPDATA%\AimAdapt\workouts`, and the app's workout editor writes them there.

```toml
name = "Morning"
description = "Clicking, then tracking."

[[items]]
scenario = "builtin:timed-flicks"   # or user:<file name>
runs = 2                            # 1–20, default 1
mode = "adaptive"                   # or "standard"; default adaptive
variants = ["builtin:timed-wide-flicks"]  # optional: rotate week by week

[[items]]
scenario = "builtin:strafe-track"
```

Each run is saved on its own, with the workout's name attached.

`variants` lists other scenarios that train the same thing. In weeks where the ISO week number is even the item plays `scenario`; in other weeks it plays the variants in turn. The training day starts at 4 AM, like the rest of the program.

## Programs

A program maps weekdays to workouts. Built-ins live in [`crates/aimadapt-sim/programs`](crates/aimadapt-sim/programs). Days that aren't listed are rest days. A day can be `weak_spots = true` instead of naming a workout. That day is then built from your skill profile: a standard warm-up, your two weakest skills ×5, the next two ×4, and your strongest ×3. Untrained skills count as weakest.

```toml
name = "Core Week"

[[days]]
day = "mon"
focus = "Static clicking"
workout = "builtin:core-mon"

[[days]]
day = "sat"
focus = "Weak spots"
weak_spots = true
```

Workout items can carry a `block` (`warmup`, `main`, `secondary` or `maintenance`). The Program page groups drills by block and compares each day's main drills with the same day last week.

## Playing from the command line

```bash
cargo run -p aimadapt-engine -- builtin:static-grid --windowed
```

The argument can be `builtin:<name>`, `user:<file name without .toml>`, or a path to any `.toml` file. Options: `--adaptive`, `--workout <id>` or `--program <id> [--day mon…sun]` (instead of a scenario), `--windowed`, `--no-save`, `--db <file>` and `--seed <n>`. The same seed with the same mouse input replays the same run.
