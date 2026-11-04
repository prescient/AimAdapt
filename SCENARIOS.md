# Scenario files

RustyAim's built-in trainer plays scenarios written as TOML files. The eight built-in scenarios live in [`crates/rustyaim-sim/scenarios`](crates/rustyaim-sim/scenarios). Your own scenarios go in `%APPDATA%\RustyAim\scenarios`. The Play screen's editor writes them there, and you can also edit the files by hand.

Every field is optional. A missing field takes the default shown below, so a file only needs the values it changes. Unknown fields are rejected, which catches typos like `radiuss`. The app's editor is built from the same field list ([`crates/rustyaim-sim/src/spec.rs`](crates/rustyaim-sim/src/spec.rs)), and a test checks that this page covers every field.

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
- Moving targets travel in angle space at a constant distance, so they keep the same on-screen size and bounce off the edges of the spawn area.
- A sphere of radius `r` at distance `d` is `2 × atan(r / d)` degrees wide. For example, a 0.3 m target at 10 m is about 3.4° wide.
- Your sensitivity (cm/360 and DPI), FOV and crosshair are player settings, not part of the scenario. They're stored in `%APPDATA%\RustyAim\engine.toml`.

## Fields

`[min, max]` ranges pick a new value in between for each spawn or direction change. Use the same number twice for a fixed value.

### About

| Field | Default | Meaning |
|---|---|---|
| `name` | `"Untitled"` | Shown in lists. Your history is grouped by name, plus a fingerprint of the settings, so harder or easier variants are kept apart. |
| `description` | `""` | What the scenario trains and how to play it. |
| `author` | `""` | Who made it. |
| `category` | `"clicking"` | `clicking`, `tracking` or `switching`. |
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
| `targets.color` | `"#ff5a4e"` | Target color as `#rrggbb`. |
| `targets.respawn_delay_s` | `0.0` | Seconds before a destroyed target is replaced. |
| `targets.lifetime_s` | `0.0` | A target disappears as a timeout after this many seconds. 0 means it stays until it's destroyed. |
| `targets.shrink_to` | `1.0` | Size at the end of its lifetime, as a share of the radius. Needs `lifetime_s`. |

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
| `targets.movement.kind` | `"static"` | `static`, `strafe` (side to side, reversing at random intervals) or `random` (a new direction at each change). |
| `targets.movement.speed` | `[3.0, 5.0]` | Meters per second at the target's distance. |
| `targets.movement.change_every_s` | `[0.4, 1.2]` | Seconds between direction changes. |
| `targets.movement.vertical` | `0.5` | `random` only: 0 keeps movement level and 1 allows any direction. |

### `[scoring]`

Score = kills × `per_kill` + hits × `per_hit` + misses × `per_miss` + damage × `per_damage` + timeouts × `per_timeout`.

| Field | Default | Meaning |
|---|---|---|
| `scoring.per_kill` | `100.0` | Points per destroyed target. |
| `scoring.per_hit` | `0.0` | Points per hit. For beams, a hit is one 1/60 s tick on target. |
| `scoring.per_miss` | `0.0` | Usually 0 or negative. |
| `scoring.per_damage` | `0.0` | Points per point of damage. Tracking scenarios use 1. |
| `scoring.per_timeout` | `0.0` | Usually 0 or negative. |

### `[arena]`

| Field | Default | Meaning |
|---|---|---|
| `arena.size` | `[60.0, 24.0, 60.0]` | Room width, height and depth in meters. Spawn distance must fit inside it. |
| `arena.eye_height` | `1.7` | Eye height above the floor. |
| `arena.wall_color` | `"#3a3f4b"` | Wall and ceiling color. |
| `arena.floor_color` | `"#2a2e37"` | Floor color. |

## What gets recorded

Each finished run is saved to the same database as KovaaK's runs, with `source = 'rustyaim'` and KovaaK's field names. That includes score, hit and miss counts, kills with TTK, damage, sensitivity (as cm/360), DPI, FOV, average FPS and resolution. The `Hash` field holds the scenario's fingerprint, which changes when any gameplay setting changes. For beam weapons, hits and misses count 1/60 s ticks, which matches how KovaaK's reports tracking accuracy.

## Playing from the command line

```bash
cargo run -p rustyaim-engine -- builtin:static-grid --windowed
```

The argument can be `builtin:<name>`, `user:<file name without .toml>`, or a path to any `.toml` file. Options: `--windowed`, `--no-save`, `--db <file>`, `--seed <n>`. The same seed with the same mouse input replays the same run.
