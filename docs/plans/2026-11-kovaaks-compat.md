# Plan: closing the gaps in KovaaK's scenario import

AimAdapt plays KovaaK's `.sce` files in its own trainer (BACKLOG section 9). On 11 November 2026 it played 98 of the 225 scenario files on the development machine, and 30 of the 34 Voltaic S5 Novice benchmarks. This plan lists what stops the other 127 and turns each cause into a work package.

A scenario is refused when it uses something the trainer doesn't model, so every package here removes one or more reasons for refusal. Refusing is deliberate. A scenario that plays differently from KovaaK's gives scores that mean nothing next to the player's KovaaK's history.

## Status

| Package | Commit | Files that play |
|---|---|---|
| K1 Every reason on a file | dc4732c | 98 of 225 |
| K2 Dashes | fdf08f9 | 103 of 225, 31 of the 34 Novice benchmarks |
| K3 Helper bots and knockback | | 107 of 225, all 34 Novice benchmarks |
| K4 Lives, time score and efficiency | | |
| K5 Hitbox shapes | | |
| K6 Standing and spawning details | | |
| K15 Ability and helper setups K2 and K3 refuse | | |
| K7 Armed bots that never fire, invincible characters | | |
| K8 Bots that destroy themselves | | |
| K9 Heads | | |
| K10 Waypoint paths | | |
| K11 Several pellets per shot | | |
| K12 Teleporters, hurt volumes and jump pads | | |
| K13 Solid props | | |
| K14 Player movement | | |

## Where the files stand

Counted with a scratch survey over the same 225 files. A file can have several reasons, and it plays only when all of them are gone.

| Reason for refusal | Files | Covered by |
|---|---|---|
| Bots that use abilities (dashes, knockback) | 47 | K2, K3 |
| Waypoint paths | 28 | K10 |
| The player can move | 25 | K14 |
| Heads, headshots only | 21, 3 | K9 |
| Bots' weapon abilities | 19 | K8 |
| Map objects (teleporter, hurt volume, jump pad) | 17 | K12 |
| Armed bots while the player can be hurt | 16 | K7, K8 |
| Solid props | 15 | K13 |
| Aim assist | 11 | Later |
| Invincibility set on a character | 11 | K7 |
| Aim down sights | 9 | Later |
| Bot size (zero-size helper bots) | 9 | K3 |
| Player abilities | 8 | K14 |
| Several pellets per shot | 8 | K11 |
| Hitbox shape | 6 | K5 |
| Score multiplied by damage or kill efficiency | 5, 1 | K4 |
| Recoil | 5 | Later |
| KovaaK's own dynamic difficulty | 4 | Later |
| Bot lives, a score for time | 3, 3 | K4 |
| Distance scoring, MBS scoring | 3, 1 | Later |
| Projectile and explosive weapons | 3, 1 | Later |
| Third-person camera | 1 | Later |

Expected count of playable files after each step, if the packages land in order: K2 to K5 reach 118 and all 34 Voltaic S5 Novice benchmarks. K7 reaches 133, K8 143, K9 152, K10 159, K11 166, K12 175, K13 184 and K14 about 200. The rest need the "Later" list.

Recounted after K3, with 107 playing. That estimate took K2 and K3 to clear every ability file, and they didn't: 19 files still have a movement ability setting K2 refuses (most often a dash that can go up or down) and 6 have a helper setup K3 refuses. Those are now K15, which runs after K6. With it the expected counts are: K4 111, K5 111, K15 118, K7 and K8 144, K9 153, K10 160, K11 167, K12 175, K13 184 and K14 201. The other 24 need the "Later" list, or use the kill efficiency score, which has no result file to check a formula against.

Of the 118 files that don't play after K3, 58 wait on a single package, 30 on two and 30 on three or more. Files each package is needed for, and the files it would unlock by itself: K7 and K8 44 and 22, K14 31 and 5, K10 28 and 3, K15 23 and 7, K9 21 and 5, K12 17 and 1, K13 15 and 5, K4 8 and 4, K11 8 and 3, K5 6 and none, the "Later" list 24 and 3.

## What is out of scope

- The adaptive knob for imported scenarios (BACKLOG section 9, phase 5) and the app work around them (phase 6).
- Copying anything from KovaaK's or from scenario authors: no assets, no prop meshes, no scenario files in this repo.
- Editing a player's own KovaaK's files. Test maps made for measuring are new files with the `AimAdapt -` prefix.

## How the work is run

- Each package is built by a smaller model from the package text below and a short brief. The result is reviewed before it is committed: the diff is read, the tests are run, the survey is run again over the real files, and the pre-commit hook runs on the whole tree.
- One commit per package, through the pre-commit hook. Nothing is pushed.
- Packages run one at a time, because nearly all of them change the same three crates (`aimadapt-sce`, `aimadapt-sim`, `aimadapt-engine`).
- Tests use made-up scenarios only. Fixture files are hand-written `AimAdapt Test - <name>.sce` files in `crates/aimadapt-sce/tests/fixtures/`, with invented names and numbers that reproduce the mechanic. A builder works from the brief and the fixtures, and doesn't open other people's scenario files.
- Every package follows [AGENTS.md](../../AGENTS.md): the README, the user guide (with a What's new line), BACKLOG, TESTING and SCENARIOS kept in step, and a failing-first test for every bug.
- New fields on `World`, `WorldKind` and `Motion` are optional and left out of saved scenarios when unused, so scenarios that already play keep their fingerprint and their bests.
- A setting whose meaning is a guess gets a note on the import ("how closely this matches KovaaK's is still to be checked") and a row in the table at the end of this plan.

## Measuring progress

`aimadapt-cli sce --summary` counts the files that play. Since K1 it also counts files per reason and how many files each reason blocks alone. Each package's commit message gives the count before and after.

The listing includes the test maps written for measuring in KovaaK's (the `AimAdapt -` prefix: 32 on the development machine at the start, all playable, 36 after K2, 34 of them playable, and 50 after K3, 47 of them playable). The counts in this plan leave them out: 130 of 257 in the listing was 98 of 225 here, 137 of 261 after K2 is 103 of 225, and 154 of 275 after K3 is 107 of 225.

After K1, 50 of the 127 refused files had a single reason: armed bots 11, abilities 9, the player moving 5, heads 5, invincibility set on a character 5, solid props 5, waypoint paths 3, several pellets 3, and one each for map objects, aim assist, aim down sights and the damage efficiency score. The other 77 need two or more packages.

Playing a file isn't the same as matching KovaaK's. KovaaK's records no target positions, so the checks are: side by side by eye, a test map where one number can be read off the screen, and scores from real play. The last one needs a person at the mouse (BACKLOG section 9, phase 4).

## Wave 1: already in the backlog's plan

### K1. Every reason on a file

The listing shows only the beyond-plan reasons on a file that has both kinds, so it understates what a package unlocks.

- `aimadapt-sce`: one call that returns every reason for a file, in-plan and beyond, without stopping at the first kind.
- `aimadapt-cli sce`: a refused file lists all of its reasons. `--summary` adds a count per reason and a count of files that reason blocks alone.
- The app's Browse page shows the same full list for a file it can't play.
- Tests: a made-up file with one reason of each kind reports both.

### K2. Dashes

Bots with a movement ability dash: a burst of speed along the way they are moving, on a charge that refills. One Voltaic benchmark (Ground) needs it.

- `aimadapt-sim::motion`: a bot can carry dashes. A dash has charges (the most it holds, how many it spawns with, seconds to regain one), a delay after use, a duration, a speed, an upward speed, the share of speed kept when it ends, whether the direction is locked, whether gravity is off for the duration, and whether the bot's own movement is blocked meanwhile.
- When a bot uses one: how often it considers it, the chance each time, a block after spawning, a cooldown shared by all its abilities, and the ability's own rules (distance to the player, the player within a view angle, on the ground or in the air, own health).
- `aimadapt-sce::convert`: `[Movement Ability Profile]` sections on a bot's character become dashes. A movement ability with a setting outside this list is still refused, and names the setting.
- Tests: charge and reuse timing, the speed during and after a dash, the rules that stop a use, a fixed-seed run that stays the same, and a fixture file.

Done. The timing was measured in KovaaK's first, on four `AimAdapt - Use Test` maps with hopping and dashing bots filmed off the screen:

- `AbilityUseTimer` is the time between presses of the ability key.
- `UseAbilityFrequency` is a chance rolled once per window of `UseAbilityFreqMinTime` to `UseAbilityFreqMaxTime` seconds. For that whole window the bot presses every time or not at all. With a frequency of 1 the window times change nothing.
- `FullyAuto` changes nothing for a bot.
- `AIReuseTimer` and `ChargeTimer` hold a use back until the first press after them. A charge starts to refill when the ability ends, and a charge that comes back at the moment of a press isn't used for that press: a 4 s charge after a 0.1 s dash, pressed every 0.1 s, went every 4.2 s.
- A dash with `MainVelocitySetToMovementKeys` goes along the bot's movement input and stays on the ground. It covers its speed times its duration, and the bot then carries on at about its usual speed.
- A line of sight to the player counts as in combat. Nobody has to shoot.
- A dash with `MainVelocityCanGoVertical` and a main velocity throws the bot high into the air.
- Gravity is 800 units/s² per unit of `Gravity`. The importer had 980, so every imported bot fell a fifth too fast. Fixed in its own commit, with a regression test.

Refused with a reason that names the setting: dashes that can go vertical, abilities used in reaction to damage (`AIDamageReaction`), a movement ability on a bot with `NoDodging`, health restore, hurtboxes and the health limits. Times over an hour and distances over 10 km are cut to those limits. After K2, 20 files in the listing still have a movement ability the trainer refuses.

### K3. Helper bots and knockback

Three Voltaic benchmarks (Pasu, Controlsphere, Raw Control) and most of the 47 ability files move their targets with helper bots. A helper is on the player's team, is too small to see, and hits enemy bots near it with a melee ability that does no damage and pushes them. Some helpers sit still. Others chase the target along the far side of an invisible wall, so the target is pushed back whenever it comes close to that wall.

- `aimadapt-sce::convert` (`convert/helpers.rs`): a bot on the player's team is a helper. It isn't a target, isn't drawn, doesn't count toward the score or the number of bots on screen, and isn't held to the size limits. A helper with a melee ability becomes a pusher in the world. A helper without one is dropped with a note.
- `aimadapt-sim` (`sim/pushing.rs`): `World` carries pushers (where each starts, which way it faces, whether it chases, its reach, its hit period, its level and vertical push, and whether it needs to see its target). Each step, every pusher that is due adds its push to the bots it reaches.
- Invincible bots by character: a bot is invincible when the scenario says so or its own character does. `WorldKind` carries the flag, so a mixed scenario can have both.
- Tests: the hit period table, a ball held up by a chasing pusher under an invisible floor, a pinned pusher's reach and sight, a helper left out of the score and the bot count, the per-character flag, each refusal, and two fixture files (a chasing helper and a pinned one).

Done. Knockback was measured in KovaaK's first, on fourteen `AimAdapt - Knock Test` maps filmed off the screen:

- `FlatKnockbackHorizontal` and `FlatKnockbackVertical` are speeds in units per second, added to the speed the target already has. The `Hurtbox…KnockbackFactor` values don't scale them, and the push doesn't fade toward the edge of the hurtbox.
- The hurtbox is a sphere of radius `HurtboxRadius` whose middle is that far ahead of the helper along its facing. A target is hit when its body overlaps the sphere, so the reach along the facing is twice the radius plus the target's own. `MapScale` doesn't scale it.
- The level push points from the helper to the target. The vertical push keeps its sign.
- A helper hits on a fixed period: the smallest whole number of `AbilityUseTimer` presses (one at the least) that is as long as the longest of `AIReuseTimer`, `AbilityDuration + ChargeTimer` and `AbilityDuration + DelayAfterUse`. A press that lands exactly on that time is used. The helper hits on that schedule whether or not anything is in reach.
- A long swing (`AbilityDuration` of 1 s) hits a target once when it enters the hurtbox, which is a different rule, so swings over 0.05 s are refused.
- With `SeeThroughWalls=false` a helper hits only the targets it has a clear line to, middle to middle, past visible solid blocks. Clip blocks never stop a push.
- A chasing helper (a fast flier whose dodge profile always goes forward) presses against the far face of the clip slab between it and its target, at the point nearest the target.

Refused with a reason: pushes that do damage or vary between a smallest and a largest value, swings over 0.05 s, more than one charge, helpers that don't always use the ability or that see a narrower view, helpers that carry weapons, a target's own melee ability, helpers that are neither pinned nor a fast chaser, a hit period under 0.005 s, and a flying target that doesn't move by itself in a scenario with helpers (whether it hovers or falls when pushed wasn't measured).

After K3, 107 of the 225 files play, and all 34 Voltaic S5 Novice benchmarks do. Four local files import with pushers: two kinds of chaser under an invisible floor or behind a wall, and one set of pinned helpers. None of them was played in the engine by hand. Six files still have a helper setup K3 refuses (see K15).

### K4. Lives, time score and efficiency

- Bots with a set number of lives: the run ends when the lives are used up.
- A score for time, paid when the run ends that way.
- A score multiplied by damage efficiency or kill efficiency.
- Before building: check each formula against real KovaaK's result files (kills, shots, damage done and possible, and the score are all in them). The formulas go in SCENARIOS.md.
- Checked on 11 November 2026 against result files:
  - Time score: `ScorePerTime` times the game seconds left (`Timelimit` minus the game seconds played) when the run ends. Four runs on three scenarios, each within 3 ms.
  - Lives: `BotMaxLives` holds one number per bot slot. The three local files with lives have one slot each, a rotation played in order with as many lives as it has bots. Their result files show exactly that many kills, each of the next bot in the rotation, and the run ends on the last kill. No local file has lives on several slots, so that isn't measured and is refused.
  - Damage efficiency: the score times damage done over damage possible, where damage possible is shots times the damage of one shot. One run, exact. That file scores by damage only. A file that scores kills too is unverified; the multiplier is taken to apply to the whole score.
  - Kill efficiency: no result file has a score to check, so it stays refused.
- The same result files showed that `Timescale` is the rate of KovaaK's whole game clock, and the importer ignored it. A challenge lasts `Timelimit / Timescale` real seconds (185 runs), shots per real second are the weapon's rate times the time scale, and the time score counts game seconds. 27 of the files that played after K3 set a time scale other than 1 and so ran at the wrong speed and length. Fixed in its own commit before K4, with a regression test.
- `aimadapt-sim`: the world gets the lives, the time score and the two multipliers; `run_step` ends the run, `score()` applies them.
- Tests: one per rule on a made-up scenario, and that old scenarios score as before.

### K5. Hitbox shapes

- True cylinders (flat ends; they are capsules now), stretched spheres (a height other than twice the radius; they are round now), and cuboids.
- The weapon's hit radius: a shot counts when it passes within that distance of the hitbox.
- `aimadapt-sim`: new `Shape` variants with their own ray tests, and the hit radius on the weapon. `aimadapt-engine` draws each shape as it is hit.
- The import notes "cylinder bots are played as capsules", "bots are stretched spheres in KovaaK's" and "the weapon's hit radius is left out" go away.
- Tests: ray hits and misses at the edges of each shape, with and without a hit radius, and the fingerprints of scenarios that already play.

### K6. Standing and spawning details

Small differences on files that already play.

- Each kind of bot starts at its own height and spawn offset (all kinds use the first kind's now).
- A bot's own `BlockSpawnFOV`.
- Bots stand on a floor within their step-up height, as the player does.
- Crouching bots (dodge profiles that crouch on the ground).
- The head's offset on the player's eye height.
- How long a jetpack bot holds its thrust, which is a guess today.

Each item is its own small change with a test. The ones that need a number from KovaaK's get a test map first.

### K15. Ability and helper setups K2 and K3 refuse

Added on 11 November 2026, after the recount that followed K3. It runs after K6, because 23 of the files that don't play need it and 7 of them need nothing else.

- A dash that can go up or down (`MainVelocityCanGoVertical` with a main velocity). The most common of the four. K2's test map showed such a bot thrown high into the air and nothing more exact, so measure the launch first.
- An ability that costs the bot health (a negative `HealthRestore`): bots that hurt themselves or remove themselves on a timer. Check in KovaaK's that the bot dies of it and whether that death counts as a kill.
- An ability used in reaction to damage (`AIDamageReaction`), with the health limits that go with it (`AIMaxSelfHealth`, `AIMaxTargHealth`), and a movement ability on a bot with `NoDodging`.
- Helpers that go freely at their target (neither pinned nor a fast chaser behind a wall), a target's own melee ability, and bots of size zero that K3 doesn't count as helpers.

Each one is measured, built and taken off the refusal list by itself, so this package can be split into several commits.

## Wave 2: beyond the backlog's plan

Ordered by files unlocked for the work involved. Each starts by reading how the local files use the feature, so the package builds what scenarios need and leaves out what none of them use.

### K7. Armed bots that never fire, invincible characters

- 12 of the 16 "armed bots" files give their bots a weapon and also tell them not to aim. They are target switching scenarios, and nothing in them hurts the player. Check in KovaaK's that such a bot never fires, then stop refusing them.
- Invincibility set on a character: the player is invincible when the scenario or their own character says so. Bots are covered in K3.

### K8. Bots that destroy themselves

"Pressure" scenarios give each bot a weapon ability or a weapon that damages the bot itself after a delay, so a target that isn't shot in time goes away and counts against the player.

- Read the 19 files: the delay, what starts it (a timer from spawn, or the first hit), and what the score does when a bot dies that way.
- `aimadapt-sim`: a kind can have a fuse. The engine shows it only if KovaaK's does.
- The four armed-bot files left over from K7 belong here.

### K9. Heads

- A bot can have a head: a sphere on top of the body, with its own radius and offset. A hit on the head does the weapon's headshot damage.
- Headshots only: body hits do nothing.
- The engine draws the head in the head colour.

### K10. Waypoint paths

- 28 files move bots along waypoints placed in the map. The only logic in use is `FollowAimAtWaypoint`.
- The map reader keeps the waypoints and each spawn's path. Bot movement gets a mode that walks them.
- Measure in KovaaK's first: what happens at the last waypoint, and whether the dodge profile still applies on the way.

### K11. Several pellets per shot

- A shot fires several pellets with a spread. Read the spread settings in the 8 files, then model the pattern (fixed or random) and how hits and damage count per pellet.

### K12. Teleporters, hurt volumes and jump pads

- The map reader keeps only the names of these objects. Keep their properties too.
- Hurt volumes damage bots inside them (often a kill floor). Teleporters move a bot that touches them. One file has a jump pad.

### K13. Solid props

- 15 files have props that block shots or movement, among them three Voltaic 1w2ts benchmarks with a window frame.
- The trainer has no copies of KovaaK's prop meshes and won't get them. Measure each prop's solid box on a test map, and play it as a plain box of that size, drawn in a flat colour. A prop that hasn't been measured still refuses.

### K14. Player movement

- 25 files let the player walk, strafe or jump, and 8 give the player abilities.
- This is the largest package: the sim puts the eye at the origin, and bots measure their distance from it. It starts with a short design note: how the engine moves the eye, how the sim learns where it is, and how the bot replay stays the same from run to run.

## Later

Aim assist (11 files), aim down sights (9), recoil (5), KovaaK's own dynamic difficulty (4), distance and MBS scoring (4), projectile and explosive weapons (4), third-person camera (1). Each gets its own package when the files above play.

## Settings read by inference

These rows are filled in as packages land. A row leaves the table when a test map or a side by side confirms it.

| Setting | Read as | Checked |
|---|---|---|
| The first press of the ability key after a bot appears | One press interval after it appears | No |
| `BlockAbilityOnStartDuration` (character) | No ability for that long after the bot appears | No |
| `AbilityGlobalCooldown` (character) | After any ability, none of the bot's abilities for that long | No |
| `DelayAfterUse` (ability) | That ability can't be used again for that long after a use | No |
| `EndVelocityFactor` | The share of the dash speed the bot keeps when the dash ends | No |
| `LockDirectionForDuration`, `NegateGravityForDuration`, `AbilityBlocksMovement` | As named, for the ability's duration | No |
| `AIMinTargDist`, `AIMaxTargDist`, `AIMaxTargFOV`, `AIUseOnGround`, `AIUseInAir` | Rules that stop a use: distance to the player, the player within the view angle, on the ground or in the air | No |
| A new ability window | Starts when the last one ends, on its own timer | Partly: the windows and the presses ran on separate timers |
| A dash that comes later than its settings say | Not copied | Open: seen twice in KovaaK's on a 4.2 s dash (once 5.67 s), cause unknown |
| Which targets a helper's hit reaches | Every target its hurtbox overlaps (and that it can see, when it needs sight) | No: it may hit only the target its AI has picked |
| Several helpers with the same name | Each takes the next spawn point its character is allowed on, in file order | No |
| A pinned helper's spawn delay | None: it hits from the start of the run | No |
| A chasing helper's position | On the outer face of the clip slab, at the point nearest its target, with no travel time | Partly: one test map, by eye |
| `InvincibleBots` on a character | That character's bots can't die, whatever the scenario's own flag says | No |
| `StandStillUntilHurt` on a bot that is pushed | The pushes move it only once it has been hurt | No |
| A flier's vertical speed when it is lifted | The push is added to it like any other speed | No |
| A target bouncing on a push cushion | Not copied: in KovaaK's it lost about 4% of its height per cycle, cause unknown | Open |
