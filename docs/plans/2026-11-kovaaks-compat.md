# Plan: closing the gaps in KovaaK's scenario import

AimAdapt plays KovaaK's `.sce` files in its own trainer (BACKLOG section 9). On 11 November 2026 it played 98 of the 225 scenario files on the development machine, and 30 of the 34 Voltaic S5 Novice benchmarks. This plan lists what stops the other 127 and turns each cause into a work package.

A scenario is refused when it uses something the trainer doesn't model, so every package here removes one or more reasons for refusal. Refusing is deliberate. A scenario that plays differently from KovaaK's gives scores that mean nothing next to the player's KovaaK's history.

## Status

| Package | Commit | Files that play |
|---|---|---|
| K1 Every reason on a file | dc4732c | 98 of 225 |
| K2 Dashes | fdf08f9 | 103 of 225, 31 of the 34 Novice benchmarks |
| K3 Helper bots and knockback | 32d5397 | 107 of 225, all 34 Novice benchmarks |
| K4 Lives, time score and efficiency | 783b0d7 | 111 of 225 |
| K5 Hitbox shapes | cfb7fd1 | 111 of 225 |
| K6a Where bots appear and stand | 7736860 | 111 of 225 |
| K6b The jump key | 13f11d4 | 111 of 225 |
| K15a An ability that costs health | e3c7715 | 113 of 225 |
| K15b Chasers that take time to arrive | 19f0673 | 116 of 225 |
| K7 Armed bots that never fire, invincible characters | 399d081 | 132 of 225 |
| Fix: spawn offsets follow the map's axes | 4e0c441 | 132 of 225 |
| Fix: regeneration between shots | cf9d8ca | 132 of 225 |
| Fix: when a bot uses an ability | a94eb94 | 132 of 225 |
| K15c Dashes that go up or down | 65512d7 | 132 of 225 |
| K15d Abilities used when hurt | 8ddb138 | 133 of 225 |
| K15e The smaller refusals | 3ddc7f7 | 133 of 225 |
| Fix: a bot comes back after its least respawn delay | f6b14a1 | 133 of 225 |
| Fix: what a bot's abilities keep over a respawn | c118ce9 | 133 of 225 |
| K8a Still bots that blow themselves up | 1aa3a85 | 139 of 225 |
| K8b The other weapon abilities, bots that fire | | |
| K9 Heads | 4c4db67 | 143 of 225 |
| Fix: the player's own older spawn offsets, and where a small player stands | | 144 of 225 |
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

Recounted after K7, with 132 playing. K7 ran before the rest of K15 and made 16 files play. Of the 93 that don't play, 33 have one reason, 32 have two and 28 have three or more. Files each reason is on, and the files it blocks by itself: waypoint paths 28 and 4, player movement 25 and 5, heads 21 and 5, weapon abilities 19 and 6, map objects 17 and 1, a movement ability the rest of K15 covers 16 and 1, solid props 15 and 5, aim assist 11 and 1, aim down sights 9 and 1, several pellets per shot 8 and 3, player abilities 8 and none, bots that aim and fire 4 and none, `InvincibleBots` on the player's character 2 and none. K8 reaches 138 if it plays the six files that wait on weapon abilities alone.

Recounted after K8a, with 139 playing. Of the 86 that don't play, 32 have one reason, 26 have two and 28 have three or more. Files each reason is on, and the files it blocks by itself: waypoint paths 28 and 5, player movement 25 and 5, heads 21 and 5, map objects 17 and 2, solid props 15 and 5, weapon abilities 13 and 4, aim assist 11 and 1, a movement ability that is still refused 9 and none, aim down sights 9 and 1, several pellets per shot 8 and 3, player abilities 8 and none, recoil 5 and none, KovaaK's own dynamic difficulty 4 and 1, bots that aim and fire 4 and none.

Recounted after the fix for the player's own spawn offsets, with 144 playing. Of the 81 that don't play, 34 have one reason, 20 have two and 27 have three or more. Files each reason is on, and the files it blocks by itself: waypoint paths 28 and 5, player movement 25 and 6, map objects 17 and 2, solid props 15 and 5, weapon abilities 13 and 6, aim assist 11 and 2, a movement ability that is still refused 9 and 1, aim down sights 9 and 1, several pellets per shot 8 and 3, player abilities 8 and none, recoil 5 and 1, KovaaK's own dynamic difficulty 4 and 1, bots that aim and fire 4 and none, and a bot size outside what the trainer allows 3 and 1. No file is refused for still bots hidden behind walls any more.

## What is out of scope

- The adaptive knob for imported scenarios (BACKLOG section 9, phase 5) and the app work around them (phase 6).
- Copying anything from KovaaK's or from scenario authors: no assets, no prop meshes, no scenario files in this repo.
- Editing a player's own KovaaK's files. Test maps made for measuring are new files with the `AimAdapt -` prefix.

## How the work is run

- Each package is built by a smaller model from the package text below and a short brief. The result is reviewed before it is committed: the diff is read, the tests are run, the survey is run again over the real files, and the pre-commit hook runs on the whole tree.
- One commit per package, through the pre-commit hook. Each commit is pushed to `main`, and the next package waits until the CI run for that push has passed.
- Packages run one at a time, because nearly all of them change the same three crates (`aimadapt-sce`, `aimadapt-sim`, `aimadapt-engine`).
- Tests use made-up scenarios only. Fixture files are hand-written `AimAdapt Test - <name>.sce` files in `crates/aimadapt-sce/tests/fixtures/`, with invented names and numbers that reproduce the mechanic. A builder works from the brief and the fixtures, and doesn't open other people's scenario files.
- Every package follows [AGENTS.md](../../AGENTS.md): the README, the user guide (with a What's new line), BACKLOG, TESTING and SCENARIOS kept in step, and a failing-first test for every bug.
- New fields on `World`, `WorldKind` and `Motion` are optional and left out of saved scenarios when unused, so scenarios that already play keep their fingerprint and their bests.
- A setting whose meaning is a guess gets a note on the import ("how closely this matches KovaaK's is still to be checked") and a row in the table at the end of this plan.

## Measuring progress

`aimadapt-cli sce --summary` counts the files that play. Since K1 it also counts files per reason and how many files each reason blocks alone. Each package's commit message gives the count before and after.

The listing includes the test maps written for measuring in KovaaK's (the `AimAdapt -` prefix: 32 on the development machine at the start, all playable, 36 after K2, 34 of them playable, and 50 after K3, 47 of them playable). The counts in this plan leave them out: 130 of 257 in the listing was 98 of 225 here, 137 of 261 after K2 is 103 of 225, 154 of 275 after K3 is 107 of 225, and 158 of 275 after K4 is 111 of 225.

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
- `AIReuseTimer` and `ChargeTimer` hold a use back until the first press after them. A charge starts to refill when the ability ends, and a charge that comes back at the moment of a press isn't used for that press: a 4 s charge after a 0.1 s dash, pressed every 0.1 s, went every 4.2 s. (K15 measured this again and found two of these wrong: the charge refills from the press, and the reuse timer spaces the tries. See "When a bot uses an ability" under K15.)
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
- A chasing helper (a fast flier whose dodge profile always goes forward) presses against the far face of the clip slab between it and its target, at the point nearest the target. This was read off one map by eye, and K15b measured it properly: a chaser trails a moving target, and how far depends on its drag and its acceleration.

Refused with a reason: pushes that do damage or vary between a smallest and a largest value, swings over 0.05 s, more than one charge, helpers that don't always use the ability or that see a narrower view, helpers that carry weapons, a target's own melee ability, helpers that are neither pinned nor a fast chaser, a hit period under 0.005 s, and a flying target that doesn't move by itself in a scenario with helpers (whether it hovers or falls when pushed wasn't measured).

After K3, 107 of the 225 files play, and all 34 Voltaic S5 Novice benchmarks do. Four local files import with pushers: two kinds of chaser under an invisible floor or behind a wall, and one set of pinned helpers. None of them was played in the engine by hand. Six files still have a helper setup K3 refuses (see K15).

### K4. Lives, time score and efficiency

- Bots with a set number of lives: the run ends when the lives are used up.
- A score for time, paid when the run ends that way.
- A score multiplied by damage efficiency or kill efficiency.
- Before building: check each formula against real KovaaK's result files (kills, shots, damage done and possible, and the score are all in them). The formulas are in the user guide (SCENARIOS.md documents the built-in scenario file format, which this doesn't touch).
- Checked on 11 November 2026 against result files:
  - Time score: `ScorePerTime` times the game seconds left (`Timelimit` minus the game seconds played) when the run ends. Four runs on three scenarios, each within 3 ms.
  - Lives: `BotMaxLives` holds one number per bot slot. The three local files with lives have one slot each, a rotation played in order with as many lives as it has bots. Their result files show exactly that many kills, each of the next bot in the rotation, and the run ends on the last kill. No local file has lives on several slots, so that isn't measured and is refused.
  - Damage efficiency: the score times damage done over damage possible, where damage possible is shots times the damage of one shot. One run, exact. That file scores by damage only. A file that scores kills too is unverified; the multiplier is taken to apply to the whole score.
  - Kill efficiency: no result file has a score to check, so it stays refused.
- The same result files showed that `Timescale` is the rate of KovaaK's whole game clock, and the importer ignored it. A challenge lasts `Timelimit / Timescale` real seconds (185 runs), shots per real second are the weapon's rate times the time scale, and the time score counts game seconds. 27 of the files that played after K3 set a time scale other than 1 and so ran at the wrong speed and length. Fixed in its own commit before K4, with a regression test.
- `aimadapt-sim`: the world gets the lives, the time score and the two multipliers; `run_step` ends the run, `score()` applies them.
- Tests: one per rule on a made-up scenario, and that old scenarios score as before.

Done. `BotMaxLives` is read as a list with one number per bot slot (it was read as a single number before, so `1;1` wasn't refused for lives; regression test). A scenario with one target slot whose number is N, and 0 on every helper slot, sets `timing.kill_limit` to N, and the existing early end finishes the run on the Nth kill. A slot holds one bot at a time, so a kill is a life. Lives are ignored when the scenario or the slot's characters are invincible. Lives on several target slots, on some and not others, on a helper, on a slot whose bots are only sometimes invincible, and in a list whose length doesn't match the bot slots are refused as beyond the plan, with the setting named and a note that they weren't measured. The world gets `time_score` (points per game second left, added to the points in `Sim::score()` at every moment of the run) and `damage_efficiency` (the score times damage done over damage possible, the same two numbers the run file writes); both stay out of the file at their defaults, so fingerprints don't change. Notes on the page say when a multiplier meets a score rule that wasn't checked with it. Still refused: the lives cases above, and kill efficiency (beyond the plan, no result file to check a formula against). The made-up fixture Last Lives plays through lives and the time score.

After K4, 111 of the 225 files play. The four new ones are the three local files with lives and a time score (one slot each, with 5, 7 and 8 lives) and one file with the damage efficiency multiplier, which also scores kills and so carries the unchecked note. None of them was played in the engine by hand. Four more files with damage efficiency wait on other packages. One file has lives on several slots and one uses kill efficiency, and both stay refused. No file that played before K4 changes, so no best starts again.

### K5. Hitbox shapes

The plan first read the three `MainBBType` values by their names: a cylinder with flat ends, a sphere stretched to its height, and a box. Two of the three were wrong, so the shapes were measured before anything was built.

Measured on 11 November 2026 with five test maps in KovaaK's 3.9.13 (`AimAdapt - Shape Test …`). Bots stood around a fixed ray straight ahead, fire was held for 1.5 s, and hits were read from the kill count and from screen captures. Below, R is the character's `MainBBRadius` and H its `MainBBHeight`.

- `Cylindrical` with H of 2R or more is a capsule of radius R and total height H, rounded ends included. It is drawn that way and hit that way. The importer already played it as one, so its note "cylinder bots are played as capsules" described a difference that doesn't exist.
- `Spheroid` is a ball of radius R whatever H is, tall or flat. It is never stretched. The importer already played it round, so the note "bots are stretched spheres in KovaaK's" was wrong too.
- `Cuboid` with H of 2R or more is a box 2R wide, 2R deep and H tall. It stays lined up with the map's axes: neither the bot's aim nor its spawn point's yaw turns it.
- Start height: a character's middle is H above its spawn point for all three shapes when H is 2R or more. A flat `Spheroid` (H under 2R) starts 2R above it. So a spheroid starts the larger of H and 2R above its spawn point. The importer used 2R for every spheroid, which put the tall ones too low. That is a bug, fixed here.
- Standing, with gravity on a floor: a tall `Spheroid` (R 100 or 50, H 800) falls and rests as a ball of radius R, like a round one. A capsule and a box (R 100, H 800) stand H tall with their bottom on the floor.
- `Cylindrical` and `Cuboid` with H under 2R: one test (R 200, H 160) was drawn and hit as a shape about 100 units across and tall. The rule behind that isn't known, and no local file has one, so they are refused.
- No local file's weapon has a `HitscanRadius` above 1 unit. With nothing to check a hit radius against, it isn't built, and its note stays for the day a file has one.

What gets built:

- `aimadapt-sim`: a third `Shape`, `Box` (`"box"` in a scenario file): `radius` is half its width and half its depth, `height` its full height, and its faces lie along the sim's axes. A shot hits when the ray meets the box. For moving, standing on floors, bumping and being pushed, a box uses the same body as a capsule of that radius and height. `aimadapt-engine` draws it as a box, and the scenario editor offers it as a shape.
- `aimadapt-sce`: a `Cuboid` with H of 2R or more imports as a box when the player starts facing along one of the map's axes (a start yaw that is a multiple of 90 degrees). The importer turns the map by that yaw, so on any other map the box would sit turned against the sim's axes, and such a file is refused. All six local files with cuboids pass. `Cylindrical` and `Cuboid` flatter than 2R are refused. The two wrong notes go.
- The tall spheroid's start height: the larger of H and 2R. Importer only, and only the first kind of bot (every kind uses the first kind's height until K6). The hit shape and the standing body were already right. The player's own eye height isn't touched: it was measured on other shapes.
- Tests: ray hits and misses at a box's faces, edges and corners (where a capsule of the same size misses), a regression test for the tall spheroid's start height, the refusals, a made-up fixture with box bots, and the fingerprints of fixtures that already play.

Done. After K5, 111 of the 225 files still play: no file started or stopped playing. Twelve playing files have a tall round bot as their first kind (H from 1.04 to 5.45 times 2R), so their bots now start higher and bests set on them here start again. The stretched-sphere note left 11 playing files and the cylinder note 26, and no file gained a note. One playing file has tall round bots only as later kinds, and it plays as before until K6 gives each kind its own start height. None of the six local files with cuboids plays yet, because each waits on another package too. So the box is checked by the sim's tests and the made-up Box Row fixture, and it hasn't been played or looked at in the engine on a real file.

### K6. Standing and spawning details

Small differences on files that already play. The package began as six short items: each kind's own start height and spawn offset, a bot's own `BlockSpawnFOV`, standing on a floor within the step-up height, crouching, the head's offset, and how long a jetpack holds its thrust. Measuring the jetpack showed that jumps and flyers were played wrongly too, so the package grew. It lands in two commits: where bots appear and stand (K6a), then the jump key (K6b).

Measured on 11 November 2026 with six test maps in KovaaK's 3.9.13 (`AimAdapt - K6 Test …`). Each put a row of red bots 3000 units in front of the player, and a screen recording at 60 frames a second gave every bot's height over time, to about 14 units. The counts below are over the 111 files that play.

Start heights, offsets and standing:

- Each kind of bot starts at its own height (K5's rule, on its own shape and size) and with its own `SpawnOffsetMin` and `SpawnOffsetMax`. Every kind uses the first kind's today. Among the kinds a player shoots at, start heights differ in 5 files and offsets in 3. Nothing to measure: it is K5's rule applied to each kind.
- A bot that never moves still falls when its character has gravity. The trainer leaves it hanging where it appeared (1 file). A bug, fixed here.
- A bot that appears with a floor within its `StepUpHeight` below its feet is put on that floor in its first frame (a gap of 250 with a step-up of 300). With the floor further down than that it falls at its gravity (a gap of 80 with a step-up of 75). 33 files have walking bots with a step-up height.
- A bot's own `BlockSpawnFOV` doesn't keep it out of the player's view. With 30 degrees set on the bot and none on the player, 96 spawns in 67 s landed on every point of an arc in front of the player. The importer doesn't read the setting, and that stays.
- Crouching on the ground changes nothing in these files. `CrouchHeightModifier` is 1.0 on every bot whose dodge profile crouches, and the one walking bot that crouches is as fast crouched as standing. A dodge profile's crouch matters only for a flyer (below). The note "bots don't crouch yet" stays for a walking bot whose crouch would change its height or its speed, and no playing file has one.
- The head's offset on the player's eye height: no playing file sets `MainBBHeadOffset`, so it stays in the backlog.

A dodge profile's jump settings work like a key. When the bot appears, and then every `MinJumpTime` to `MaxJumpTime` seconds, the bot decides with the chance `JumpFrequency` whether the jump key is down until its next decision. Two decisions in a row to hold it are one long hold. What the key does depends on the bot.

Jumping (33 files):

- A jump needs a press: the key going down while the bot stands on the ground. A bot told to hold the key in every window hopped once in 147 s. The trainer jumps at every decision to hold, so with a chance of one half it jumps about twice as often as KovaaK's. A bug, fixed here.
- `CanPogoJump`: a bot that lands with the key held jumps again, at every landing.
- `AllowBufferedJumps`: a press made in the air jumps at the landing, if the key is still held then.
- With neither, a press in the air does nothing unless the bot has an air jump left. One exception: a bot that comes down from its spawn point with the key held jumps as it lands.
- Air jumps (20 files): a press in the air makes an air jump while the bot has one left, and its upward speed becomes `AirJumpVelocity`. `AirJumpCount` is the number of air jumps per flight, and a landing refills it (checked with counts of 1, 2 and 3). A held key makes none. With `AllowBufferedJumps`, a press after the air jumps are used up is buffered for the landing: on a bot with one air jump, 28 of 70 jumps came at a landing.
- The jump speed: a character has either `JumpVelocity` or, in its place, `JumpVelocityMin` and `JumpVelocityMax`. No installed profile has both, and the pair is always two equal numbers. Both forms give the same jump, and the importer already read both. A test bot with a real range (200 to 800) jumped to a different height each time.

Jetpacks (18 files):

- The jetpack thrusts for as long as the key is held. The trainer thrust for one window per decision, which was the guess the plan named.
- `JetpackActivationDelay` passes between the press and the thrust when the press is made on the ground. A press in the air thrusts at once. A bot that lands with the key held keeps thrusting, with no new delay.
- A delay of exactly 0 never thrusts. A delay of 0.001 thrusts at once. One playing kind has 0.
- A kind with a jump speed jumps at a press on the ground, and the thrust follows after the delay. A press in the air only thrusts.
- A climb is capped at `JetpackMaxZVelocity`. A fall isn't.
- Fuel: the tank holds `JetpackFullFuelTime` seconds. It drains one second per second while the key is held, from the press on, the delay included. It refills by `JetpackFuelIncPerSec` per second whenever the jetpack isn't thrusting. When the tank runs dry the thrust stops and can't start again for about 0.26 s. After that it thrusts whenever the key is held and the tank isn't empty.
- So a bot that keeps the key held after running dry sputters. With a refill of 1 or less it stays off until the key is let go. With 1.25 it thrusts in bursts of 0.065 s, with 1.5 in bursts of 0.13 s, and with 2 it is on and off for 0.26 s each. All four were seen.
- `JetpackFuelRegensInAir` off: one bot thrust for 1.2 s on a 1.2 s tank, then never again in 144 s on the floor with the key held. Every playing kind with it off has a tank of 999 s or more, so the exact rule doesn't matter yet.

Flyers (14 files):

- `AlterateJumpCrouchInput` on: the bot flies up for one `ToggleUpDown` time and down for the next, starting upward, and holds still for the `UpDownSwapPause` time between the two. Its jump and crouch frequencies change nothing. The trainer plays every flyer this way.
- Off: the toggle times do nothing. The jump key flies the bot up and a crouch key flies it down, each on its own windows, at the flight speeds. The crouch key is held with the chance `CrouchInAirFrequency` for `MinCrouchTime` to `MaxCrouchTime` at a time. It needs the character's `CanCrouch` and works without `CanCrouchInAir`. `CrouchOnGroundFrequency` does nothing to a flyer. Both keys together cancel, unless the character has `IsFlyUpOnJumpAndCrouch`, when it rises.
- With no key down, a flyer without gravity stops at once. So the 11 dodge rows in 4 files with alternate off, no jumps and no crouches hover at one height in KovaaK's, where the trainer bobs them up and down. A bug, fixed here.
- At a new press, and at a turnaround, the vertical speed starts from zero. `FlightAccelUp` applies while the bot gains speed upward and `FlightAccelDown` while it gains speed downward. The trainer carries the old speed through a turnaround.
- A flyer whose character has gravity (3 files): with no key down it keeps its speed and falls under its gravity, and it stands on a floor. While a key is held, gravity doesn't act. It never jumps, whatever its jump speed.

Not measured, and left as it is: `FlightObeysPitch` (15 playing kinds have it), whether a bot's own `BlockSpawnFOV` stops other bots from appearing in that bot's view, and kinds that differ in `BlockedSpawnRadius` (1 file, where every kind uses the first kind's).

What gets built in K6a, where bots appear and stand:

- `aimadapt-sim`: a kind of bot can have its own rise (how much higher or lower than the first kind it starts) and its own pair of spawn offsets. A spawn draws the same random numbers in the same order as before, so worlds without these keep their seeds' layouts.
- `aimadapt-sce`: each kind gets K5's start height on its own character, and its own offsets when they differ from the first kind's.
- A bot that never moves, isn't a flyer and has a `Gravity` above 0 in its character gets the physics of a pushed bot: it falls and stands. Before, only scenarios with helper bots did that.
- `StepUpHeight` is read for bots with gravity that don't fly. When such a bot appears with a floor within that height below its feet, it starts on the floor. Only the moment a bot appears is built: KovaaK's wasn't measured on a walking bot that meets a step.
- Tests: a regression test for each of the two bugs (every kind at the first kind's height and offsets, a still bot with gravity hanging in the air), the step-up on both sides of its limit, a made-up fixture, and the fingerprints of fixtures that already play.

K6a is done. After it, 111 of the 225 files still play: no file started or stopped playing, and no note changed. 38 playing files changed, so bests set on them here start again. 33 have a bot with a step-up height, kinds differ in start height in 5 and in offsets in 3, and some files are in two of those groups. One of the 33 is the file with the still bot that now falls: its bots have a top speed but are told never to dodge, and every one of its spawn points has a floor under it, so they land. In one file two characters that differ only in their offsets are two kinds now. All of this is checked by the tests and the made-up Stair Court fixture. No real file was played in the engine and looked at.

What gets built in K6b, the jump key:

- `aimadapt-sim`: each bot has a jump key, decided when it appears and at the end of every jump window. Only dodge profiles with a jump chance draw random numbers for it, so files without jumps keep their seeds' layouts.
- Jumpers: a press on the ground jumps. A press in the air makes an air jump while one is left, and is buffered otherwise when the character has buffered jumps. A landing with the key held jumps for a pogo jumper, for a buffered press, and for the first landing after the bot appeared.
- Jetpacks: the activation delay, thrust for as long as the key is held, and the tank with its 0.25 s wait after running dry. A jetpack with a delay of exactly 0 is left out by the importer, so that kind only hops.
- Flyers: a dodge profile with alternate off flies by the jump key and a crouch key. For every flyer the vertical speed starts from zero at a new input. With no input a flyer without gravity stops, and one with gravity falls.
- `aimadapt-sce`: reads `CanPogoJump`, `AllowBufferedJumps`, `JetpackActivationDelay`, `JetpackFullFuelTime`, `JetpackFuelIncPerSec`, `JetpackFuelRegensInAir`, `AlterateJumpCrouchInput`, `IsFlyUpOnJumpAndCrouch`, `CrouchInAirFrequency`, `MinCrouchTime` and `MaxCrouchTime`. A bot counts as moving when it can jump (either form of the jump speed, and gravity) or has a jetpack it uses. The note about the jetpack's guessed hold time goes, and the crouch note is kept only where a crouch would change a walking bot.
- Tests: a regression test for each of the three bugs (a jump at every decision to hold, a thrust of one window, flyers with alternate off bobbing), one test per rule above, a made-up fixture, and the fingerprints of fixtures that already play.

K6b is done. After it, 111 of the 225 files still play, and no file started or stopped playing. 24 files lost a note: the one about a jetpack's guessed hold time is gone (19 files), and no playing file keeps the crouch note (6 had it), because no walking bot in them changes when it crouches.

63 playing files changed, so bests set on them here start again. 61 have a bot that jumps, uses a jetpack or flies. The other 2 have a bot with a jump chance that can't jump (no jump speed, or no gravity): it plays as before, but the decision it makes as it appears draws a random number, so a seed lays the run out differently. The remaining 48 keep their fingerprints.

Two things were settled in review, each with a regression test:

- The importer leaves out settings that can't act. Pogo and buffered jumps are stored only for a bot that jumps off the ground, and a flier's key settings only for a flier. Stored wherever the file had them, they changed 20 files whose play is the same.
- A scenario's fingerprint covered only its stored settings. A change to the sim's rules leaves those as they were, so 3 files whose bots play differently now would have kept their bests. The fingerprint has a mark for such a change, given only to scenarios that use the rule: a scenario whose bots fly, or whose dodge profiles have a jump chance, carries the jump key's mark. A later rule change adds its own mark in the same place (`Scenario::rule_marks`).

All of this is checked by the tests and the made-up Jump Court fixture, and the three bug tests were also run against the code before K6b, where they fail. No real file was played in the engine and looked at.

### K15. Ability and helper setups K2 and K3 refuse

Added on 11 November 2026, after the recount that followed K3. It runs after K6, because 23 of the files that don't play need it and 7 of them need nothing else.

- A dash that can go up or down (`MainVelocityCanGoVertical` with a main velocity). The most common of the four. K2's test map showed such a bot thrown high into the air and nothing more exact, so measure the launch first.
- An ability that costs the bot health (a negative `HealthRestore`): bots that hurt themselves or remove themselves on a timer. Check in KovaaK's that the bot dies of it and whether that death counts as a kill.
- An ability used in reaction to damage (`AIDamageReaction`), with the health limits that go with it (`AIMaxSelfHealth`, `AIMaxTargHealth`), and a movement ability on a bot with `NoDodging`.
- Helpers that go freely at their target (neither pinned nor a fast chaser behind a wall), a target's own melee ability, and bots of size zero that K3 doesn't count as helpers.

Each one is measured, built and taken off the refusal list by itself, so this package is split into several commits.

Counted over the 23 files before anything was built, as the files that have an item and the files that would play with that item alone: the vertical dash 14 and none, the health cost 6 and 2, the damage reaction 4 and none, the helpers of the fourth item 3 and 3, bots of size zero 3 and none, a target's melee ability 2 and none, and one file each for `AIMaxSelfHealth`, `AIMaxTargHealth`, an ability on a bot with `NoDodging` and a movement ability with a hurtbox. The health cost (K15a) and the helpers (K15b) are the two items that unlock files alone, 5 between them, so they come first. The other 2 of the 7 files need the vertical dash, and neither it nor the damage reaction has been measured yet.

The health cost (K15a), measured on 11 November 2026 with two test maps in KovaaK's 3.9.13 (`AimAdapt - Cost Test A` and `B`). Four bots hopped with a movement ability whose `HealthRestore` is negative. They had no health regen and a respawn delay of 1.5 s, a kill was worth 1000 points and damage 1. Read from screenshots about 1.6 s apart, so the times are rough.

- Each use takes the cost off the bot's health at once. After the first use the health bars stood at 60%, 50% and 75%, for costs of 40 of 100, 40 of 80 and 25 of 100.
- The bot dies when a use takes it to exactly 0 (80 health with a cost of 40 on the second use, 100 with 25 on the fourth) or would take it below 0 (100 with 40 on the third use, with 20 left). A bot with 30 health and a cost of 40 dies on its first use in every life.
- That death is no kill. The kill count, the damage and the score stayed empty through nine such deaths.
- The bot comes back after its respawn delay at full health and starts over.

Six local files have such an ability and none of them plays. In all six the cost is two fifths to a half of the bot's health. None has health regen, invincible bots, bot lives or a cooldown shared between abilities, so none of those was measured together with a cost.

What gets built in K15a:

- `aimadapt-sim`: an ability can have a health cost, taken off the bot's health at each use. A bot whose health reaches 0 that way leaves like a bot that drains away: it counts as a timeout, gives no kill, no score and no damage, and its slot fills again after the respawn delay. The cost is left out of a stored scenario when it is 0, so nothing that plays today changes.
- `aimadapt-sce`: a negative `HealthRestore` on a movement ability becomes the cost. A positive one (healing) stays refused. So does a cost on a bot that can't die, which wasn't measured.
- Tests: the cost at each use, the death at exactly 0 and below it, no credit for that death, the respawn at full health, a shot that finishes a bot the cost has weakened, the importer's three cases, a made-up fixture, and the fingerprints of fixtures that already play.
- Expected: 2 files start playing, which makes 113 of 225, and no playing file changes.

K15a is done. After it, 113 of the 225 files play. The 2 expected files started, none stopped, and the 111 that played before keep their fingerprints, their notes and their stored settings, so no best starts again. The other 4 files with a cost are still refused for something else.

Three things were settled in review:

- A cost on an ability that doesn't move the bot refuses the file. The importer keeps a bot's abilities only when one of them moves it, so such a cost would have been dropped without a word. Neither of the 2 files is refused by this.
- The two combinations nobody measured play with a note on the scenario's page: a cost on a bot whose health regenerates or drains, and a cost in a scenario with bot lives. Neither of the 2 files has one.
- Both notes had a run of spaces in the middle, where the text's line break in the source wasn't escaped. A test now checks that every note is one line with no run of spaces, and it fails on the old text.

The new sim tests were run against the code with the cost taken out, where 7 of them and the fixture test fail. The 2 files were played in the sim without a window: left alone, their bots leave by themselves and the run scores 0 with no kills, and with shots that always hit they give kills and a score. No real file was played in the engine and looked at. There a bot that leaves by itself goes without a sound or a flash, and the results screen counts it under Timeouts.

The helpers (K15b). Reading the three files showed that their helpers aren't free. They are chasers behind a clip slab, like K3's, and K3 refuses them only because their top speed is under the 10000 units/s it takes a chaser to have (theirs is under a third of that). K3 plays a chaser as if it always sat at the point of the slab nearest its target. That holds only for a chaser so fast that it never falls behind, so the chase was measured.

Measured on 11 November 2026 in KovaaK's 3.9.13 at 120 frames a second (`AimAdapt - Knock Test S…` and `L…`). A visible chaser flew behind a slab 860 units from the lane of a target that ran back and forth at 600 units/s, 2.5 s each way. Six films of the game window gave both places in every frame, to about 6 units. The films differ in the chaser's top speed (300, 1000 and 3000 units/s), its acceleration (50000 and 100000 units/s²) and its `DragCoefficient` (4, 13 and 26).

- Each frame the chaser's velocity first loses the share `DragCoefficient` times the frame time. Then it gains `Acceleration` times the frame time along the line from the chaser to its target. Then its length is cut to `MaxSpeed`. Last, the slab takes away the part that points into it.
- So a chaser trails a moving target. Once it keeps pace, the line from it to the target leans away from the slab's normal by an angle whose sine is the drag times the target's speed over the acceleration. With an acceleration of 50000 the chaser was about 138 units behind at a drag of 13, 260 to 285 at a drag of 26 (the law gives 270 to 283) and 40 to 60 at a drag of 4 (50 to 56).
- One law fits all six films with the drag at 0.9 to 1.0 times `DragCoefficient`, to within 5 to 13 units.
- No other setting enters it. The films also differ in `Friction`, `AerialFriction`, `BrakingFrictionFactor`, `BrakingDeceleration`, the aim profile and `SeeThroughWalls`, and the same law fits them.
- The chaser aims a little late. The films fit best with the aim 0.008 to 0.033 s behind the target, which adds 5 to 20 units at 600 units/s.
- When the top speed binds, the result depends on the frame rate: the speed a frame adds into the slab counts toward the cut, and the slab removes it only afterwards. It bound in two films (top speeds of 300 and 1000). It doesn't bind in any local file, where every chaser is far faster than its target.

Three of the files that play have chasers, and K3 plays them as instant. Their chasers follow the same law, and at their targets' top speeds the lean reaches about 10 degrees. They are imported again with the flight, so bests set on them here start again.

What gets built in K15b:

- `aimadapt-sim`: a chasing pusher can have a flight (an acceleration, a top speed and a drag) and a start delay. With a flight it starts at its spawn point and flies by the law above, in steps of 1/120 s at most, held to the plane through its spawn point. Its reach lies along the line to its target, and the level push points from where it is to the target. A chasing pusher without a flight stays K3's instant one, for worlds written by hand.
- `aimadapt-sce`: a flier on the player's team whose dodge profile always goes forward is a chaser when its top speed, its acceleration and its drag are all above 0. The speed threshold goes. A chaser without a drag is refused, because that wasn't measured. A helper character's `BlockAbilityOnStartDuration` becomes the start delay (the three refused files set one, and every playing file has 0).
- Tests: a regression test for the instant chaser (a chaser trails a target that moves at a steady speed by the lag the law gives), the start delay, a top speed that binds, the refusals, the chasing fixture with made-up flight settings, and the fingerprints of fixtures that already play.
- Expected: the 3 files start playing, which makes 116 of 225, and the 3 playing files with chasers change.

K15b is done. After it, 116 of the 225 files play. The 3 expected files started and none stopped. Of the 113 that played before, 110 keep their fingerprints, their notes and their stored settings. The other 3 are the ones with chasers: their stored worlds gained the flight and nothing else, and bests on them start again. No local file is refused for its helper bots any more.

Four things were settled in review:

- A stored flight has no defaults. It names its acceleration, its top speed and its drag, and a flight with a key the trainer doesn't know is refused.
- A start delay gets one note for each delay, however many helpers share it. Each of the 3 new files has four helpers with the same delay and showed four notes, one naming each helper.
- The test that a chaser's path is the same at any frame length compared frames of 1/60 s and 1/240 s on gentle settings, and it still passed with the flight changed to one step per frame. It now compares 1/10 s and 1/240 s on a chaser with a high drag, to within 2%, and fails on that change.
- What `BlockAbilityOnStartDuration` does for a helper wasn't measured. It plays as no hits for that long from the start of the run, and the note says so.

The regression test was run against K15a's code, where it fails: the instant chaser holds the ball at 0.58 m from the first kick, and the test wants it to land. Ten changes were then made to the flight one at a time (no drag, a doubled drag, no top speed, no start delay, one step per frame, a start at the origin, not held to its plane, a kick from the nearest point of the plane, a reach along its facing, a reach behind its plane), and each fails a test.

The six local files with chasers were played in the sim without a window: a target is up in every frame and the ball keeps moving. No imported scenario with a chaser was watched in the game window, here or next to KovaaK's. Two things from the films aren't modelled: the chaser's aim being a little late, and the frame rate entering when the top speed binds.

The rest of K15 was measured on 11 and 12 November 2026 in KovaaK's 3.9.13 at 120 frames a second, with made-up test maps (`AimAdapt - Vert Test …` and `AimAdapt - React Test …`). Films of the game window at about 59 frames a second gave every bot's place in each frame, so times are good to about 0.02 s. Eighteen files that don't play still have a K15 refusal, and two of them need nothing else. The work comes in four commits: a fix to when a bot uses an ability, the dashes that go up or down (K15c), the abilities used when hurt (K15d) and the smaller refusals (K15e).

**When a bot uses an ability (the fix).** Flying bots that dash level and sideways under different timers (`Vert Test T1` and `T2`) showed that K2 had part of this wrong.

- Nothing is used until `BlockAbilityOnStartDuration` has passed since the bot appeared. A character without the key waits 1.0 s. A try that falls exactly on the end of the block goes through.
- The bot's key ticks every `AbilityUseTimer` from the moment the bot appears. A tick is a try only when it comes more than `AIReuseTimer` after the try before. So tries come at the smallest multiple of the use timer that is greater than the reuse timer: every 0.2 s for 0.1 and 0.1, every 1.1 s for 0.1 and 1.0, every 0.25 s for 0.25 and 0.1, every 0.3 s for 0.1 and 0.25. A try that finds the ability not ready still counts as one, and so does a try made during a dash. Ticks before the block ends don't count.
- A charge starts to refill at the press, whatever the ability's length. With several charges missing they come back one after another, each a `ChargeTimer` after the one before.
- A charge that comes back at the very moment of a try isn't there for it. Charges that are missing when the bot appears are the exception: they count from that moment and are there on a tie.
- `DelayAfterUse` counts from the press.
- No ability starts while one is going on, and a try at the very moment a dash ends isn't used.

So a bot with one charge uses it at the first try that is later than the charge timer, the delay and the ability's length, all counted from the press. K2 saw a dash every 4.2 s from a 4.0 s charge, a 0.1 s dash and a tick every 0.1 s, and took the charge to refill from the end of the dash. The films show the other cause: that bot's reuse timer was 0.1 s, so its tries were 0.2 s apart, and the try at 4.0 s was a tie.

Not measured: `AbilityGlobalCooldown`, two abilities on one bot (each is played with its own reuse timer, and a tick uses the first one that is ready), and whether the block starts again when a bot respawns (it was played as starting again with each bot until it was measured: it counts once, see the faults found while measuring K8). K3's rule for the time between a helper's hits was measured on helpers and doesn't follow these rules. It wasn't measured again and stays as it is. So does a helper whose character has no `BlockAbilityOnStartDuration`: it still starts at once.

What gets built in the fix:

- `aimadapt-sim`: tries, charges, the delay and the block follow the rules above, counted on the bot's own tick times so that the frame rate plays no part. A scenario whose bots have abilities carries a new rule mark in its fingerprint, so bests on those scenarios start again.
- `aimadapt-sce`: a target's character without `BlockAbilityOnStartDuration` gets a block of 1.0 s.
- Tests: one failing-first test for each rule the trainer had wrong (every tick a try, a charge that refills from the end of its ability, a delay counted from the end, no block without the key), the two tie rules, the same on frames of uneven length, the rule mark, and the fixtures' fingerprints.
- Expected: no file starts or stops playing. Every playing file whose bots have abilities changes.

The fix is done. No file starts or stops playing: 132 of 225. Eight of the playing files have a bot with an ability. They carry the new mark, so their bests start again, and one of them had no `BlockAbilityOnStartDuration` and now waits a second. No other file's scenario or fingerprint changed.

Sixteen tests failed on the old code: the importer's default, the rule mark and fourteen on the timing. One timing test passed there, the dash every 4.2 s that K2 had measured, which the old rules reached for the wrong reason. Seven changes were then made to the new code one at a time (a tie that counts as a try, a press that restarts the refill chain, only a use counting as a try, the delay counted from the end of the dash, a tick during the block counting as a try, no rule mark, the mark on a bot with an empty ability list), and each fails a test.

The eight files were played in the sim without a window on the code before and after. The runs without a shot fired score the same. Under held fire five are the same and three score differently, by 7% at most. None was watched in the game window. One edge is left as it is: a dash starts on the first frame at or after its tick, so below about 10 frames a second a frame can be longer than the gap between a dash's end and the next try, and that try is then lost.

**Dashes that go up or down (K15c)**, measured with `Vert Test K1` to `K5`, `A1` and `A2`: flying and walking bots above and below the player's eye that blink or dash under each combination of the settings.

- A dash set to the movement keys that negates gravity goes over the ground where the keys point: across the line to the player for a sideways key, along it for forward, away for back, and forward when no key is down. Its pitch is the pitch of the bot's aim line whichever key is down, and its speed along that slanted line is the whole `MainVelocity`. A flier's up and down key plays no part. A bot that doesn't aim (`NoAiming`) blinks level, along its own facing.
- A dash that isn't set to the movement keys goes along the aim line at `MainVelocity`, toward the player for a positive velocity and away for a negative one. An impulse (no duration) does the same.
- The aim line doesn't run from the bot's middle to the player's eye. It runs from one body height above the bot's middle to one body height above the middle of the player's character. A body height is twice the radius for a sphere or a spheroid, whatever its `MainBBHeight`, and `MainBBHeight` for a capsule. `CameraOffset` plays no part at either end. Flying bots of six sizes and shapes that blinked to and fro each settled at the height where that line is level. A cuboid wasn't measured.
- A walking bot with no gravity that is off the ground keeps the velocity a dash leaves it with (`EndVelocityFactor` 1.0) or an impulse gives it, whatever its drag and braking.
- A dash set to the movement keys on a walking bot with gravity, without `NegateGravityForDuration`, throws the bot upward by an amount no rule explained. No local file has that combination, and it stays refused.
- Two things in the films aren't copied. A blink goes about a tenth farther than its speed times its duration. Some bots' headings are off by about 3 degrees, which is their own aim error.

Fourteen of the eighteen files have such a dash. Nine have a long dash along the aim line that keeps its speed, four have blinks along the movement keys, and one has an impulse. None of them plays with the dash alone.

What gets built in K15c:

- `aimadapt-sim`: a bot's abilities carry an aim lift in meters, which is how far above the player's eye the bot's aim line is level. A dash that can go up or down keeps the heading it had (the movement keys, or the bot's facing) and takes the pitch of the aim line, with its whole speed along the slant. A bot that doesn't aim gets a level dash, and so does an aim line of no length. Level dashes are as they were.
- `aimadapt-sce`: the lift is the player's body height, less the bot's, less the height of the eye above the player's middle. Only a bot with a dash that goes up or down gets one. The refusal of every such dash goes, and narrower ones take its place, each with its own reason: a held dash that keeps gravity, an impulse along the movement keys, a dash not along the keys on a bot with `NoAiming`, a bot or player that is neither a spheroid nor a capsule, and a file with no player character.
- No rule mark. A lift of 0 isn't stored, so a scenario without such a dash keeps its stored settings and its fingerprint.
- A made-up fixture, High Blink: a flying ball that blinks along its strafe and drifts to the height where its aim line is level.

K15c is done. Still 132 of the 225 files play. None started and none stopped, and all 132 keep their fingerprints, their notes and their stored settings, so no best starts again. Of the 14 files, 9 no longer have a reason about the dash. The other 5 keep one because their player is shaped like a box, and each of them has other reasons as well. Five of the 14 now wait on one thing: one on an ability used when hurt (K15d), and four on weapon abilities that hurt the bot itself or the player (K8).

Twenty-five changes were made to the new code one at a time, and each fails at least one test. In the sim: the lift ignored, the lift turned over, a level dash, the climb added on top of the level speed, a pitched dash for a bot that doesn't aim, no guard for a line of no length, a level ability pitched, the keys ignored, an unlocked dash that keeps its first direction, no range check on the lift, and a lift that is always stored. In the importer: the camera left out of the lift, a spheroid's height read from `MainBBHeight`, a capsule's from its radius, a lift for every bot, each refusal dropped or widened, a box counted as a measured shape, and an ability jump counted as a dash.

No imported scenario with such a dash was watched in the game window, and none of the local files plays one yet.

**Abilities used when hurt (K15d)**, measured with `React Test R1` to `R14`, `R17` and `R18`: a tall capsule that hops, or a flying ball that blinks to and fro, as its reaction to damage, under a held automatic weapon or single shots.

- A hit is a trigger when the damage taken since the last trigger reaches `AIDamageReactionThreshold`. With a threshold of 0 every hit is one. The sum starts again at the trigger. A threshold of 50 under shots of 10 gave a trigger at every fifth hit, and under 1 damage 100 times a second a trigger every half second.
- The sum carried over 1.2 s of hits 0.3 s apart with an `AIDamageReactionResetTimer` of 1.0 s, so that timer starts again at each hit. That it clears the sum when it runs out is taken from its name.
- Hits on a bot that can't be hurt (`InvincibleBots`) count, with the weapon's damage.
- Each trigger is ignored with the chance `AIDamageReactionIgnoreChance`, by itself. At 0.5, 99 of 184 triggers were ignored, with up to five ignored in a row.
- A trigger that isn't ignored starts a delay drawn evenly between `AIDamageReactionMinDelay` and `AIDamageReactionMaxDelay` (21 delays between 0.1 and 0.9 s came out from 0.12 to 0.89 s, spread evenly). The ability is used when it runs out. A new trigger while one is waiting starts the delay again. A hit below the threshold doesn't.
- When the delay runs out the ability needs a charge, and `DelayAfterUse` must be over (a tie isn't enough). A reaction that fails either is dropped. It doesn't wait for the charge.
- `AIReuseTimer` and the ticks of the bot's key play no part in a reaction.
- `AIDamageReactionCooldown` did nothing in any test. Reactions came 0.25 s apart with a cooldown of 1.0 s, and the share of ignored triggers was the same at 0, 0.7 and 1.8 s. It isn't played.
- With a `UseAbilityFrequency` of 0 the bot uses the ability only as a reaction. Above 0 it also presses its key as before, and both draw on the same charges.
- The direction of a reaction's blink follows the rules for that ability.
- One case isn't understood. An ability that is a single upward impulse sometimes shows nothing when the hit lands while the bot is in the air: with half the triggers ignored and shots 0.25 s apart, hops never followed two shots in a row. A flying bot's blink doesn't have it. A reaction on an impulse stays refused.

Not measured: whether an ability's rules for combat, distance and view angle hold a reaction back (they are played as holding it back), and a reaction that comes due while another ability is going on (played as dropped). Four of the eighteen files have such a reaction on a movement ability. In three it is a dash or blink with a duration, which is what K15d builds. One of the three has a dash with rules of its own, and those are played as holding the reaction back. The fourth file has impulses, which stay refused, as does a reaction on a melee ability.

What gets built in K15d:

- `aimadapt-sim`: an ability can carry a reaction: the threshold, the reset time, the chance to ignore a trigger and the two ends of the wait. Every hit is counted for each ability that has one, on a bot that can't be hurt too, and before the dodge's own answer to the hit. A trigger that isn't ignored sets the time its use is due. Waiting reactions are handled between the key's ticks in the order of their times. Each is tried at its own time, so charges, `DelayAfterUse` and the end of the use count from that time and the length of a frame doesn't matter. One that can't go is dropped. The random numbers come only from abilities that have a reaction, so every other bot keeps its random stream.
- `aimadapt-sce`: the reaction's keys are read, with the editor's defaults for the missing ones, the two ends of the wait put in order and each value cut to the trainer's limits. The refusal of every reaction goes. A reaction on an ability with no duration is still refused, and so is a melee ability on a target.
- No rule mark. A scenario without a reaction stores nothing new, so it keeps its stored settings and its fingerprint.
- A made-up fixture, Flinch Kite: a flying ball that never presses its key and blinks once it has taken enough damage.

K15d is done. 133 of the 225 files play. One started and none stopped, and the 132 that played before keep their fingerprints, their notes and their stored settings, so no best starts again. Of the four files with a reaction on a movement ability, three no longer have a reason about it. One of them plays, and two are still refused for other things. The fourth keeps its reasons about impulses.

Thirty-six changes were made to the new code one at a time. Thirty-five fail at least one test. The last one, always storing the reaction, makes no difference, because the writer leaves out an empty value either way. Three of the thirty-five got through at first and showed gaps in the tests: the order of two reactions due in one frame, a tick and a reaction due at the same moment, and an ignored trigger while a reaction waits. Each now has a test that fails under its change.

The file that started was played without the game window. With no shots its bot never blinks, and under held fire it blinks every few seconds at uneven gaps, which fits the rules above. No imported scenario with a reaction was watched in the game window.

**The smaller refusals (K15e).** What was left after those three: a bot with a size of zero (3 files), a melee ability on a target (2), a health cost on an ability that doesn't move the bot (2), `AIMaxSelfHealth` (1), `AIMaxTargHealth` (1), an ability on a bot that doesn't dodge (1) and a hurtbox on a movement ability (1). The rule for them was set before they were read: a setting that only files with other refusals use stays refused, with its reason.

K15e builds nothing, and K15 ends with it. The local files were listed again after K15d. Of the eighteen files above, one plays, six have no K15 refusal left and wait on other packages, and eleven still have one. Those eleven have the seven kinds in the list, a dash that goes up or down in a scenario whose player is shaped like a box (5 files) and a reaction on an impulse (1). Ten of the eleven are refused for something outside K15 too: abilities for the player, bots that shoot back, solid props, heads, waypoint paths, map objects, aim assist or aiming down sights. Building their K15 setting would start none of them, so each keeps its refusal.

The eleventh is the second of the two files that K15 alone could start. It needs three of the kinds at once. Its targets have a melee ability to use near the player, who can be hurt and has one life. It also has bots too small to hit that stand still and pay health for an ability on a timer: a health cost on an ability that doesn't move the bot, on a bot with a size of zero. Most of its rotations start with one of them and go on with one kind of target, so the small bot holds a place until it dies and that kind of target arrives a set time into the run. The death by a cost was measured in K15a. Two things weren't: whether a bot uses an ability that doesn't move it, and what a target's melee ability does to the player. The second is a question about the player being hurt, and K8 has to answer that for its own files. So this file moves to K8, and its three settings stay refused until then.

## Wave 2: beyond the backlog's plan

Ordered by files unlocked for the work involved. Each starts by reading how the local files use the feature, so the package builds what scenarios need and leaves out what none of them use.

### K7. Armed bots that never fire, invincible characters

Runs before the rest of K15. That part needs a lot of measuring and would make 2 more files play, while 16 files are refused for nothing but the two reasons here.

What the local files have: 16 are refused for armed bots, 11 of them for that alone. In 12 of the 16 every armed bot is also told not to aim (`NoAiming=true`); they are target switching scenarios. In the other 4 the bots aim, fire explosive projectiles at a player with 10000 health, and have other reasons too. 11 files are refused for invincibility set on a character, 5 of them for that alone. In those 5 the player's own character has `InvinciblePlayer=true` and no bot is armed. 2 files have `InvincibleBots=true` on the player's character, both with other reasons.

Measured in KovaaK's 3.9.13 on 11 November 2026, with four test maps that copy a target switching scenario whose two bots carry a hitscan weapon:

- Bots told not to aim never fired. Two maps (one as the file is, one with a tough player and a weapon knockback of 50) showed no tracer in 11 still frames over 49 s, and the view never moved.
- The same bots told to aim fired all the time: a white tracer from each bot in all 10 frames over 36 s.
- Not established: whether a bot's shots hurt the player. The default HUD shows neither the player's health nor the score, and a player with 100 health showed no death in 18 s under fire. So a file whose bots aim stays refused unless the scenario's own `InvinciblePlayer` is on, as before.
- Not measured: `InvincibleBots` on the player's character. Its test map never showed a bot to shoot.

What gets built:

- A bot with a weapon that is told not to aim is no reason to refuse a file. It never fires, so it plays as a bot without a weapon. A helper bot with a weapon stays refused, aiming or not (no local file has one).
- `InvinciblePlayer=true` on the player's character is no reason to refuse a file, because nothing in the trainer hurts the player. It doesn't make a file with bots that aim and fire play: whether the character's switch protects the player the way the scenario's does wasn't measured.
- `InvincibleBots=true` on the player's character stays refused.

Result: 132 of 225. The 116 files that played before keep their fingerprints, notes and stored settings. Four files are still refused for bots that aim and fire and two for `InvincibleBots` on the player's character, all six with other reasons too.

### Two faults found by playing K7's files

The 16 files K7 made play were played in the sim without a window, by a bot that aims perfectly. Two older faults showed, and neither is K7's. Each gets its own commit with a regression test, before K8 starts.

#### Spawn offsets ran along each spawn point's facing

A character's `SpawnOffsetMin` and `SpawnOffsetMax` move its bots away from the spawn point they appear at. The importer ran that offset along the spawn point's own axes. The rule was read off one benchmark family (ww5t), whose spawn points face away from the player and whose offsets are written with the larger number first, and it was never measured.

Measured in KovaaK's 3.9.13 on 11 November 2026 with 40 test maps. Each has one still bot at a spawn point 10 m ahead and an offset of 5 m on one axis, and the ball's place is read off the screen as a tangent:

- A spawn point's rotation never turns an offset. Yaw and pitch were tried, on a Reflex map and on a JSON map.
- In a file whose `GameVersion` is 2.0.5.0 or later, X runs along the map's forward axis, Y to the map's right and Z up. The numbers are Unreal units and `MapScale` doesn't scale them.
- In a file whose `GameVersion` is below 2.0.5.0 the offset is a quarter turn round: X runs to the map's right and Y to the map's back. That was measured at 2.0.4.9 and at 2.0.4.0, and 2.0.5.0 is the first version without the turn.
- The older keys `SpawnXOffset`, `SpawnYOffset` and `VerticalSpawnOffset` are read at every version. They make one fixed offset, in the same frame and under the same version rule.
- The player's facing doesn't matter. With the player's start turned 90° the ball sat at a tangent of −0.509. The map's axes predict −0.5, and an offset that followed the player's view would have put it dead ahead.
- A copy of a ww5t file cut down to one spawn point showed its targets in an upright column at a tangent of 0.7375. The map's axes predict 0.7335 and the old rule 0.543.

Not measured: the old and the new keys both set on one character (no installed file has that), a helper bot's offset, the player's own character's offsets, a version below 2.0.4.0, and whether `MapScale` scales the older keys.

What gets built:

- `aimadapt-sim`: the sim's own rule stays, an offset along each spawn point's own axes. A world can now name one frame for every spawn point's offset (`offset_axes`), used in place of each point's axes. The field is optional and left out of a stored scenario when it isn't set, and nothing draws a new random number.
- `aimadapt-sce`: one reader for a character's offsets, used for targets and helpers. It reads the new keys and the older ones, turns the offset a quarter turn in a file older than 2.0.5.0, and refuses a character that sets both kinds of key.
- The importer writes the map's frame into the world only when it changes where a bot can appear: when, for some offset on some spawn point it is used at, the box of possible places along the map's axes differs from the box along the point's own axes. A spawn point turned half round with mirrored offsets, or a turned player start with unturned spawn points, gives the same box, so those worlds store nothing new and keep their fingerprints.
- A helper bot's offset runs along the map's axes too. Its facing stays its spawn point's.

Result: still 132 of 225, and 32 of the playing files changed, so their bests start again. 29 now store the map's frame and their targets appear in other places. 20 of those belong to the Voltaic S5 Novice set, the five ww5t files among them. Taken at the corners of each offset box, the places move by less than 2 m on average in 7 files, by 5 to 9 m in 14 and by 17 m or more in 8. One tracking scenario's bot used to start 16° to one side and now starts dead ahead. Two files gain a fixed vertical offset from the older key (0.1 m up, 0.5 m down). In one file a helper's four pushers moved 4 cm. The other 100 keep their fingerprints, notes and stored settings.

The 32 were played in the sim without a window. Each runs its full length with no problem reported, a target is up in at least 95% of frames, and the bot that aims perfectly hits. None was watched in the game window after the fix.

#### Bots that regenerate healed between shots

A character's `HealthRegenPerSec` and `HealthRegenDelay` make its bots heal. The sim healed a hitscan target continuously: on every frame where the delay had passed since the last hit, before that frame's shots. In three of the playing files the delay is close to the weapon's shot interval and the regen is fast, and there the bot that aims perfectly never got a kill.

Measured in KovaaK's 3.9.13 on 11 November 2026 with twelve test maps, at 120 frames a second. Each has one still ball dead ahead and a weapon that fires while the button is held and never misses. The ball comes back only after 60 s, so the hit count on screen at the kill is the hits it took.

| Health | Regen per second | Delay | Shot interval, damage | Hits to kill | What it shows |
|---|---|---|---|---|---|
| 100 | 1000 | 0.01 s | 0.01 s, 1 | 100 | With the delay equal to the interval, no healing while every shot lands |
| 100 | 50 | 0.01 s | 0.01 s, 1 | 100 | The same at a slower regen |
| 100 | 1000 | 0.02 s | 0.01 s, 1 | 100 | No healing with a longer delay |
| 100 | 1000 | 0.005 s | 0.01 s, 1 | Never (428 hits) | A full heal on each frame that has no shot |
| 50 | 100 | 0.005 s | 0.01 s, 1 | Never (483 hits) | 10 health on each frame with no shot beats 5 damage in six frames |
| 100 | 20 | 0.005 s | 0.01 s, 1 | 164 | 2 health on one frame in six: the rule below gives 164, other rules 103 to 125 |
| 100 | 50 | 0 | 0.01 s, 1 | 190 | With no delay, 5 health every 0.1 s whatever the hits (190 to 195 by phase) |
| 100 | 90 | 0 | 0.01 s, 1 | 913 | The steps are 0.1 s apart (910 to 919 by phase; 0.15 s would give 870, continuous healing 1000) |
| 100 | 20 | 0.03 s | 0.05 s, 5 | 33 | One step of 2 after every shot (continuous healing gives 22, none 20) |
| 100 | 20 | 0.045 s | 0.05 s, 5 | 20 | The delay ran out on the frame of the next shot, and the shot won |
| 100 | 40 | 0.05 s | 0.1 s, 5 | 96 | Exactly one step of 4 per shot (continuous healing gives 33) |
| 100 | 40 | 0.05 s | 0.5 s, 25 | 16 | Five steps of 4 between shots, 0.05, 0.15, 0.25, 0.35 and 0.45 s after the hit (four steps give 10, six give 76) |

The rule that fits all twelve:

- A bot heals in steps of a tenth of a second's worth, `HealthRegenPerSec` × 0.1 health at a time.
- With a delay of 0 a step lands every 0.1 s all the time. Hits don't pause it.
- With a delay above 0, each hit stops the healing and starts a countdown of the delay. When it runs out the bot gets one step at once, then one every 0.1 s until the next hit.
- The timers are looked at once a frame, after the frame's shots. A hit on the frame where the countdown runs out starts it again, and no step lands. So a weapon whose shots all land is never healed against when the delay is at least its shot interval.
- With a delay shorter than the shot interval, a step lands on the first frame at or after the countdown's end that has no hit. How often that happens depends on the frame rate.

Not measured: where the steps of a bot with no delay fall (the hit counts fit any phase), negative regen, a beam, a bot that was never hit and lost health another way, and any frame rate but 120.

What gets built:

- `aimadapt-sim`: in a scenario with a world (an imported one) and a hitscan weapon, each regenerating target has a timer for its next step. A hit on a bot with a delay sets it to the delay after the start of the hit's frame. After each frame's shots, every step that was due by the start of the frame lands and the timer moves on 0.1 s. The start of the frame is the moment the weapon's cooldown goes by, so a shot that is due always fires before a step that is due. The sim's frame is the game's frame, so a delay shorter than the shot interval depends on the frame rate here as it does in KovaaK's.
- Scenarios without a world keep the continuous rule, so the built-in scenarios play and score as before. The beam is unchanged (the importer makes none) and so is the drain.
- The stored settings don't change, so the scenarios that use the rule get a mark in their fingerprint (`heals in steps`) and their bests start again. The jump key's mark stays as it was.

Found in review of the build: the timers were first read at the end of a frame, while the weapon fires by the time left at its start. The game steps the sim with each frame's real length, and when a longer frame followed a hit's frame the countdown could run out one frame before the next shot. A bot with a delay of one shot interval then got a step it never gets in KovaaK's (with a regen of 1000, a full heal: 2999 hits and no kill in a test on frame lengths between 1/240 and 1/60 s). The timers are now set and read by the start of the frame, and the same test takes exactly 100 hits.

Result: still 132 of 225. None started and none stopped. The 47 playing files whose bots regenerate have a new fingerprint, so their bests start again. All 47 have a held hitscan weapon. Their stored settings and notes are as they were, and the other 85 keep their fingerprints too.

The 47 were played in the sim without a window, at 120 frames a second, by a bot that aims perfectly and holds fire, on the old rule and the new. Every file runs its full length with no problem reported. Idle runs are the same. With the bot shooting, 13 files give the same run, 30 give more kills, 3 the same kills with small changes in hits or score, and 1 fewer (299 to 149). That one has a delay shorter than its shot interval: it used to heal a little between shots and now gets a full step after each one, as the maps with one step per shot showed. The three files the bot could never get a kill in now give 149, 211 and 287, each at health over damage hits per kill. Five files give no kills on either rule. Their bots regenerate at or above the weapon's damage rate: four have no delay, and one gets a step after each shot that is larger than the shot's damage. The two playing files with a delay below the shot interval are not on a knife's edge: the step lands between shots at any frame rate above about 35 a second.

A delay of 0 with low health is sensitive to where the steps fall. In the play above, 0.1 s is exactly 12 frames, and reading the timers one frame earlier or later moved one file's kills between 154 and 178. Real frames don't line up like that, and KovaaK's has the same sensitivity. None of the 47 was watched in the game window.

### Faults found while measuring K8

K8's rules were measured in KovaaK's before anything was built. The films showed faults in what already plays. Each gets its own commit with a regression test, before K8 is built.

#### A bot came back after the middle of its respawn range

A character has `MinRespawnDelay` and `MaxRespawnDelay`. The importer stored the middle of the two as the bot's respawn delay, on the guess that KovaaK's draws a delay from the range. Nothing had measured it.

Measured in KovaaK's 3.9.13 on 12 November 2026 with three test maps, each with still balls that come back where they were (films at about 55 frames a second, times good to about 0.02 s):

- With 0.5 to 3.0, a ball that destroyed itself came back 0.53 to 0.57 s after it went, all 28 times.
- With the same range, a ball the player shot came back 0.53 to 0.57 s after it went, all 51 times.
- With 0.2 to 0.4, a ball that destroyed itself came back 0.23 to 0.26 s after it went, all 22 times.
- So a bot comes back `MinRespawnDelay` after it goes, however it went, and `MaxRespawnDelay` plays no part. A ball shows about 0.04 s later than the delay says, which is two frames of the film.

Not measured: a character with no `MinRespawnDelay` key (read as 0, as before). The 0.04 s isn't modelled.

What gets built:

- `aimadapt-sce`: a bot's respawn delay is its character's `MinRespawnDelay`, never below 0. `MaxRespawnDelay` is still compared when two characters are checked for being the same kind of bot, so two that differ only there stay two kinds and no scenario's slots change.

Result: still 133 of 225. None started and none stopped. Eight playing files have a range, so their stored delay is shorter and their fingerprint is new: bests set here on them start again. Nothing else in their stored settings changed, and their notes are as they were. The other 125 keep their fingerprints. Among the fixtures only Still Wall has a range (0.1 to 0.3), and its pin was taken again.

The eight were played in the sim without a window after the fix, idle and by a bot that aims perfectly and holds fire. Every file imports with no problem reported, and every run goes its full length except in one very long scenario, which the play tool cuts short. In five of the eight the shooting bot never kills a target inside the run, so the delay doesn't show there. In the other three targets come back sooner. None was watched in the game window.

#### A bot's ability timers started again with each respawn

The sim gave every new bot a fresh set of ability timers: the start block, the reuse clocks, the frequency windows, the charges and the key's ticks all began again when a bot came back. The fix to when a bot uses an ability had listed the block across a respawn as not measured.

Measured in KovaaK's 3.9.13 on 12 November 2026 with seven test maps of still balls that come back where they were (films at about 55 frames a second, times good to about 0.02 s). Something of a bot outlives it, called its brain here. A life is one bot from its spawn to its death.

- The start block counts once, from the run's start. With a block of 3.0 s and a key that ticks every 0.1 s, the first use came 3.0 s into the first life and 0.1 s into every later life.
- Each ability's reuse clock runs on. With a reuse time of 2.0 s and a tick of 0.1 s the uses were 2.1 s apart, counted straight through a death and a respawn of 0.5 s.
- The frequency windows run on. With a window of 2.0 s and a frequency of 0.5 the bot stopped and started pressing on edges 2.0 s apart counted from the run's start, through deaths. No window starts at a respawn.
- The key's ticks start again with each life: the first tick comes one interval after the bot appears. With a tick of 1.0 s the uses came 1.0 and 2.0 s into every life.
- The charges start again with each life. An ability with no charges at spawn and a charge time of 2.0 s first fired 2.0 s after each spawn.
- A bot profile with no `AbilityUseTimer` key plays as a tick of 0.1 s, and one with no `UseAbilityFrequency` key always presses.

Not measured, and played as follows: `DelayAfterUse` and `AbilityGlobalCooldown` start again with each life. An ability going on when its bot dies ends with it. A reaction to damage that is waiting, and the damage counted toward one, are dropped. A bot that first appears later in a run has its block counted from when it first appears. Where several bots are up, each place in the bot list keeps its own brain, and a place whose kind of bot changes between lives keeps each kind's brain for the next bot of that kind there.

What gets built:

- `aimadapt-sim`: when a bot leaves (killed, or gone by itself), the sim saves its brain with its slot and kind, and the next bot of that slot and kind takes it. A scenario without slots hands the oldest brain to the first bot back. The block's start, each reuse clock and the window carry over with the time the bot was away counted. The ticks, the charges, the delay after a use and the shared cooldown start fresh. A respawn draws no random numbers for any of it. Scenarios whose bots have abilities get a new rule mark in their fingerprint.
- `aimadapt-sce`: a bot profile with no `AbilityUseTimer` reads as a tick of 0.1 s (it was a tick every frame), and one with no `UseAbilityFrequency` as always pressing (it was never). An explicit 0 stays 0 for both.

Result: still 133 of 225. None started and none stopped. Nine playing files have bots with abilities, and their fingerprints are new: bests set here on them start again. Their stored settings and notes are as they were, since none of the nine leaves out either key. The other 124 keep their fingerprints. Among the fixtures Dash Lane, High Blink and Flinch Kite have abilities, and their pins were taken again.

The nine were played in the sim without a window after the fix, idle and by a bot that aims perfectly and holds fire. Every file imports with no problem reported, and every run goes its full length except in one very long scenario, which the play tool cuts short. None was watched in the game window.

### K8. Bots that destroy themselves

"Pressure" scenarios give each bot a weapon ability or a weapon that damages the bot itself after a delay, so a target that isn't shot in time goes away and counts against the player.

Of the 19 files with a bot's weapon ability, six waited on it alone. In all six the bot stands still and has one weapon ability, whose weapon is a slow explosive projectile that goes off inside the bot's own body a set time after the shot. That shape is K8a. The other 13 are K8b.

Measured in KovaaK's 3.9.13 on 12 November 2026 with seven test maps of still balls that come back where they were (films at about 55 frames a second, times good to about 0.02 s):

- A weapon ability's try needs what a movement ability's needs: a charge, a tick of the key later than the reuse time after the last try, the start block over and a willing window. It also needs `UseWeapons=true` on the bot. With `UseWeapons=false` or a frequency of 0 the bot never fired. `FullyAuto=false` on the ability changed nothing, and burst keys on a semi-auto weapon added no pause.
- A try presses the trigger for 0.1 s. The weapon fires at once if it is ready, or when its time between shots ends if that comes within the press. With a key every 0.5 s, weapons of 0.55, 0.58 and 0.60 s fired at 1.05, 1.08 and 1.10 s. Weapons of 0.62 and 0.65 s lost the press at 1.0 s and fired at 1.5 s. With a key every 0.1 s the presses join up, and a weapon of 0.65 s fired every 0.65 s.
- The blast comes `DelayAfterSpawn` after the shot (1.0 gave blasts 1.0 s after each shot) and takes the explosion's damage times `SelfDamageMultiplier` off the bot.
- A bot that dies of its own blast gives no kill, no shot and no damage on the HUD. After 40 s of such deaths the kill count was 0 and accuracy and damage were empty.
- The weapon's time between shots runs on through a death and a respawn, like the timers in the fault above: with 1.5 s between shots and a respawn delay of 1.0 s the shots stayed 1.5 s apart.
- A charge refills from the press. A test bot with a charge time of 2.0 s fired more slowly than its weapon alone would have.

Not measured, and played as follows. A press the weapon doesn't answer still uses up the ability's charge. A charge that is back exactly at a try isn't there for that try, the rule measured for movement abilities. A blast still waiting when its bot goes, killed or not, is dropped. A bot that dies of its own blast uses up no bot life. A missing `SelfDamageMultiplier` reads as 1. The ability's distance and view rules, `DelayAfterUse` and `AbilityGlobalCooldown` are read as a movement ability's are.

What K8a builds:

- `aimadapt-sim`: an ability can carry a fuse, which is the weapon's time between shots, the delay from the shot to the blast and the blast's damage to the bot. A use presses the trigger for 0.1 s. A shot queues a blast, and a blast that comes due takes its damage off the bot's health the way an ability's health cost does: when the health runs out the bot leaves as a timeout, with no kill, hit, shot or damage counted. The weapon's cooldown is part of the bot's brain and carries over a respawn. A motion can be marked as staying put. Such a bot runs its abilities and nothing else, starts exactly on its spawn point and isn't pushed apart from other bots.
- `aimadapt-sce`: a bot's weapon ability imports as a fuse when all of the following holds. Otherwise it is refused, with the ability's name and the first setting that fails.
  - The bot uses weapons, aims, carries no weapon of its own and has this one ability.
  - It doesn't move by itself, isn't pushed by helper bots, doesn't fall, can die, takes its own damage and doesn't regenerate.
  - The ability has no reaction to damage and no health rules, and none of its four `AIUse` flags is off.
  - Its weapon is in the file. It is a semi-auto explosive projectile with one shot a click, no burst, no wind-up, no magazine and no gravity, and it doesn't go off on contact or on the next attack.
  - The delay from the shot to the blast is above 0 and below the projectile's travel time. The blast does the same damage at its middle and its edge, and that damage times `SelfDamageMultiplier` is above 0.
  - The whole blast stays inside the bot's body: the projectile's travel in the delay, plus the weapon's origin offset, plus the blast's radius is no more than the body's radius (for a body that isn't a ball, the smaller of its radius and half its height). Then the blast always reaches its own bot and nothing else.
- A scenario with a fuse gets an import note that names what wasn't checked, and a second note when it also has bot lives.
- The made-up fixture `AimAdapt Test - Short Fuse.sce`.

Result: 139 of 225, up from 133. All six files that waited on weapon abilities alone play. No playing file's fingerprint or notes changed, and none stopped. The six were played in the sim without a window. Idle, every target goes by itself, and the run ends with timeouts only and a score of 0. With a bot that aims perfectly and holds fire, every shot is a kill and no target times out. None was watched in the game window.

K8b, what is left:

- 13 files still have a weapon ability that is refused. Four wait on it alone: in each, a bot carries a weapon of its own and two weapon abilities. Most of the others are refused first because the bot moves by itself, and two because the bot doesn't aim. Each of those needs its own measurements: a blast that leaves a moving bot behind, a fuse on a bot that falls, is pushed or regenerates, an explosion that reaches the player or another bot, and a bot that fires a weapon of its own.
- The four armed-bot files left over from K7 belong here: their bots aim and fire.
- One file left over from K15e belongs here too. Its targets have a melee ability to use near the player, and bots too small to hit pay health on a timer until they die, which brings in a new kind of target a set time into the run. Measure in KovaaK's what a melee hit does to a player who can be hurt, what the run and the score do when the player dies, and whether a bot uses an ability that doesn't move it. Then the file needs a melee ability on a target, a health cost on an ability that doesn't move the bot, and a bot with a size of zero, which nobody can see or hit.

### K9. Heads

Of the 21 files with a head on a bot the player can shoot, five waited on it alone. Three of the 21 are headshots only.

Measured in KovaaK's 3.9.13 on 12 November 2026 with 32 test maps: still bots placed around a ray straight ahead, and the HUD's hits, kills and damage read after each shot (100 uu is 1 m).

- The body keeps its own size: a capsule of radius R and height H, or a ball of radius R whatever its height setting says.
- The head is a ball of radius r. Its middle is r + offset above the body's top. Offset 0 rests the head on the top, a negative offset sinks it into the body, and a positive one leaves a gap that shots pass through.
- Body and head together are centred on the character's middle, which is where a headless bot's middle is. With B the body's height in the stack (H for a capsule, 2R for a ball), the body's middle is (2r + offset) / 2 below the character's middle, the head's middle is (B + offset) / 2 above it, and the stack is B + 2r + offset tall.
- A shot hits one bot, and the first surface on the ray decides between head and body.
- A weapon with `HeadshotCapable=true` does damage times `HeadshotMultiplier` to a head. A weapon that can't headshot does its plain damage to a head, and its multiplier counts for nothing.
- A shot's possible damage is damage times the multiplier whenever the weapon can headshot, whether or not any bot has a head.
- `HeadshotOnly=true`: a body hit does no damage, and a shot that hits only bodies is a miss.
- Under gravity a capsule with a head stands with its body's bottom on the floor, so it stands B + 2r + offset tall. Measured at offset 0 and at a negative offset.
- `MainBBHide` changes nothing about what a shot hits.
- A standing bot floats about 2 uu above the floor, with a head or without. The sim stands it on the floor.

What K9 builds:

- The sim: a target can have a head (`targets.head`, or `head` on a kind of bot in an imported world): a radius, an offset, a colour and whether only the head counts. Body and head are placed by the stacking rule above, a ray is tested against both and the nearer surface decides, and a target that shrinks scales its head and offset with it. The weapon has a `headshot_multiplier` (1 unless set). Both stay out of a scenario's file when unused, so no scenario that existed before has a new fingerprint.
- Shots: a head hit does damage times the multiplier and a body hit the plain damage. Possible damage is damage times the multiplier. A body hit on a bot that only takes headshots is a miss in every way: no damage, a miss in the stats and the shot record, no restart of the bot's regen delay, no reaction. Beams follow the same rules tick by tick.
- Standing and bumping: a bot with a head stands on the bottom of its body and bumps into walls and other bots as a capsule of its body's radius and the stack's height.
- The importer reads `MainBBHasHead`, `MainBBHeadRadius`, `MainBBHeadOffset`, `EnemyHeadColor` and `HeadshotOnly` from a character and `HeadshotCapable` and `HeadshotMultiplier` from the weapon. It sets the multiplier only where the weapon can headshot, the multiplier isn't 1, and either a bot the player can shoot has a head or the score uses damage efficiency. Two characters that differ only in their heads are two kinds of bot.
- Still refused, each with its own reason: a head on a cuboid, a head radius of 0 or less, headshots only without a head or with a weapon that can't headshot, a multiplier of 0 or less where it would be set, and a bot that blows itself up with a head. Three cases play with an import note: a head sunk more than twice its radius, a ball with a head that falls, and a head wider than its body.
- The engine draws the head as a ball in the head's colour and starts the health bar above the stack. The `--smoke` bot aims at the head of a bot that only takes headshots.
- A fixture, Top Knot: three still capsules with heads and a weapon that does four times its damage to a head, scored by damage efficiency.

Result: all 21 files lost the head reason, and 143 of 225 play. Four of the five that waited on heads alone play. The fifth has still bots that the importer finds hidden behind walls from where it puts the player, a planned reason that the head refusal came before. The importer didn't read the player's own older spawn offsets then. That was the cause, and the fix after K9 (the next section) reads them, so the file plays. The other 16 keep the reasons they had. No file that already played changed its fingerprint, its notes or its scenario.

### A fault found by K9's files

One file that K9 freed from the head refusal still didn't play. The importer found all its still bots hidden behind walls from where it put the player, and the player was in the wrong place.

#### The player's own older spawn offsets weren't read

A character can move where it appears with the older keys `SpawnXOffset`, `SpawnYOffset` and `VerticalSpawnOffset`, or with the newer ranges. For bots the importer read both. For the player's own character it read the newer ranges alone, so a player with an older key started on its spawn point.

Measured in KovaaK's 3.9.13 on 12 November 2026 with six test maps. Each has a floating player (gravity 0, not a flyer) and five still balls straight ahead, one on the aim line for each place the eye could be. The balls have healths of 1 to 5 and the gun does 1 damage and goes through bots, so the shot that brings the first kill tells where the eye is:

- With no offset the kill comes at shot 1.
- `VerticalSpawnOffset=300` in a file saved by 3.9.13 raises the eye 300 units.
- `SpawnYOffset=300` in such a file moves it 300 to the map's right. `SpawnXOffset=300` moves it 300 forward: the kill still comes at shot 1, and the other balls sit about 13% further from the crosshair.
- In a file saved by 2.0.4.9, `SpawnYOffset=300` moves it 300 back and `SpawnXOffset=300` moves it 300 to the right.
- So the player's older keys are one fixed offset in Unreal units along the map's axes, turned a quarter below 2.0.5.0, as a bot's are.

Not measured: a player that falls, `MapScale`, an offset below 0.1 of a unit, older and newer keys together, the newer keys in an old file, and a player whose spawn point is turned.

What gets built:

- `aimadapt-sce`: `offsets::player` reads the player's keys by the bots' rule. Older keys with newer ones that aren't zero are refused, as for a bot. An older vertical offset on a player that falls is refused, naming the character. The newer keys stay the middle of their range along the map's axes, and are turned with the file's version by inference.

#### A small player wasn't stood on its floor

Reading the offsets moved the player in files that already played, and looking at why showed a second fault. `Collider::stand` puts a character that appears just above a floor onto it. It tested the body at the far end of its drop (its step-up plus 2.4 units) and then searched for how far the body could drop. A map's walls are surfaces with nothing inside them, so a body shorter than its drop can be wholly under a floor's top face at the far end. The test then found nothing and the body stayed in the air. Over a thick floor the test touched the floor's lower face, and the search settled the body inside the floor. Only the player's start uses `stand`, and only a player character a few units tall is that short.

Measured in KovaaK's 3.9.13 on 12 November 2026 with three test maps. Each has a floating player 2 units tall above a platform and two still balls, one on the aim line if the eye stays where it appeared and one if the player stands 2 units above the platform:

- With no step-up and the platform 30 units under the player, the eye stays where it appeared.
- With a step-up of 60 and the platform 30 under, the player is pulled down and stands 2 units above the platform.
- With a step-up of 60 and the platform 70 under, past the reach of 62.4, the eye stays.
- So a body shorter than its step-up is pulled down onto a floor within its step-up, as a tall one is.

Not measured: a small player that falls. Within its reach it is stood on the floor's top face like the floating one.

What gets built:

- `aimadapt-sim`: for a body shorter than its reach, `stand` tests everything the body passes through on the way down, as one longer capsule. A taller body is tested as before, with the same arithmetic, so no other scenario moves.

Result: 144 of 225. The file that waited on its hidden bots plays, and none stopped. Four playing files have a new fingerprint, because the player's eye is stored in the scenario. In two a small player was stood inside a thick floor and now stands on top of it. In one a small floating player started above an invisible floor within its step-up and is now pulled down onto it. The fourth has an older vertical offset on a speck of a player, and its eye lands where it was to within a rounding error. Bests set here on those four start again. Their notes are as they were. The other 139 keep their fingerprints, and so do the six pinned fixtures and the built-in scenarios. One file that was already refused has one more reason: a player that falls and has an older vertical offset.

The four and the newly playing file were played in the sim without a window after the fix, idle and by a bot that aims perfectly and holds fire. Every file imports with no problem reported and every run goes its full length, with a target in view in at least 99% of frames. None was watched in the game window.

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
| The countdown before a run, on a scenario with a time scale | Not scaled: it takes the same real time at any time scale | No |
| The timer on screen, on a scenario with a time scale | Shows game seconds left, so it counts down at the time scale per real second | No |
| `AbilityGlobalCooldown` (character) | After any ability, none of the bot's abilities for that long | No |
| A bot with two abilities | Each has its own reuse timer. A tick uses the first one in the list that is ready, and the ones after it aren't tried at that tick | No |
| A try that the ability's own rules turn down (distance, view angle, ground or air) | Counts as a try, so the reuse timer runs from it | No |
| A tick of the key while the bot isn't willing to use abilities (`UseAbilityFrequency`) | Not a try | No |
| `DelayAfterUse` and `AbilityGlobalCooldown` on a bot that respawns | Start again with each new bot | No |
| An ability going on, or a reaction to damage that is waiting, when its bot dies | Ends with the bot | No |
| `BlockAbilityOnStartDuration` on a bot that first appears later in a run | Counts from when that bot first appears | No |
| A place in the bot list whose kind of bot changes between lives | Each kind's ability timers wait for the next bot of that kind there | No |
| A helper whose character has no `BlockAbilityOnStartDuration` | No wait before its first hit (a target waits 1 s) | No |
| `EndVelocityFactor` | The share of the dash speed the bot keeps when the dash ends | No |
| `LockDirectionForDuration`, `NegateGravityForDuration`, `AbilityBlocksMovement` | As named, for the ability's duration | No |
| `AIMinTargDist`, `AIMaxTargDist`, `AIMaxTargFOV`, `AIUseOnGround`, `AIUseInAir` | Rules that stop a use: distance to the player, the player within the view angle, on the ground or in the air | No |
| A new ability window | Starts when the last one ends, on its own timer | Partly: the windows and the presses ran on separate timers |
| A dash that comes later than its settings say | Not copied | Open: seen twice in KovaaK's on a 4.2 s dash (once 5.67 s), cause unknown |
| A dash that goes up or down, with a player that has a head | The aim line ends one body height above the player's middle, and the eye is where the importer puts it for a head | No: measured with a player without a head |
| An impulse along the aim line on a bot with gravity | Thrown along the aim line, and then it falls | No: measured on bots without gravity |
| A blink along the movement keys on a bot with `NoAiming`, with a key down | Level, where the key points | Partly: it blinked level in the films, and its heading under each key wasn't fitted |
| A dash that goes up or down on a cuboid bot, or with a cuboid player | Refused | No: a box's body height wasn't measured. Five local files have a box-shaped player, each with other reasons |
| A held dash that goes up or down and keeps gravity (no `NegateGravityForDuration`) | Refused, along the movement keys or not | Partly: along the keys it threw a walking bot upward by an amount no rule explained, and not along the keys it wasn't tried. No local file has either |
| An impulse (no duration) along the movement keys that can go up or down | Refused | No: no local file has one |
| A dash that goes up or down and isn't along the movement keys, on a bot with `NoAiming` | Refused | No: no local file has one |
| A dash that goes up or down in a file with no player character, or with a player that has no `MainBBType` | Refused | No: every local file with such a dash has both |
| A blink that goes farther than its speed times its duration | Not copied | Open: about a tenth farther in the films, cause unknown |
| Which targets a helper's hit reaches | Every target its hurtbox overlaps (and that it can see, when it needs sight) | No: it may hit only the target its AI has picked |
| Several helpers with the same name | Each takes the next spawn point its character is allowed on, in file order | No |
| A pinned helper's spawn delay | None: it hits from the start of the run | No |
| A chasing helper's aim | At where its target is now, with no delay | No: six films fit best with the aim 0.008 to 0.033 s late (see K15) |
| A chasing helper whose top speed binds | The same at any frame rate | No: in KovaaK's the result changes with the frame rate (see K15). It binds in no local file |
| A chasing helper in a stored world without a flight | K3's instant chaser, on the outer face of the clip slab at the point nearest its target | Wrong for a moving target (see K15). The importer no longer writes one |
| `BlockAbilityOnStartDuration` on a helper's character | No hits for that long from the start of the run | No |
| An ability's health cost on a bot that regenerates | The cost isn't a hit, so it doesn't restart the wait before the regen | No: no installed file has both |
| The healing steps of a bot with no regen delay | Counted from the moment the bot appears: the first step 0.1 s after that | No: the measured hit counts fit any phase |
| A regen delay shorter than the weapon's shot interval, at other frame rates | A step lands on the first frame at or after the countdown's end that has no hit, at any frame rate | No: measured at 120 frames a second only. Two playing files have such a delay, and in both the step lands between shots at any frame rate above about 35 a second |
| A regenerating bot that hasn't been hit | Its first step is due one delay after it appears, then every 0.1 s. At full health a step does nothing | No: it matters only for a bot that loses health another way |
| Negative regen (a drain) | Continuous, as before. Only healing goes in steps | No |
| Healing against steady fire on frames of uneven length | A shot that is due fires before a step that is due, whatever the frame lengths: both go by the start of the frame | No: the three maps with a delay of at least the shot interval took exactly 100 hits on real frames, which vary a little. Larger swings weren't tried |
| A bot that dies of its ability's cost, on a scenario with lives | It counts as a timeout and uses up no life, like a bot that drains away | No: no installed file has both |
| A moving cuboid bot's box | Stays lined up with the map's axes as the bot moves and turns | No: measured on still bots |
| `InvincibleBots` on a character | That character's bots can't die, whatever the scenario's own flag says | No |
| A bot with a weapon that is told not to aim | It never fires, so it plays as a bot without a weapon | Partly: no shot in 49 s on two test maps whose bots carry a hitscan weapon. Other kinds of weapon weren't tried |
| `InvinciblePlayer` on the player's character, with no bot that fires | Nothing to do: nothing in the trainer hurts the player | No |
| A spawn offset in a file whose `GameVersion` is below 2.0.4.0 | Turned a quarter, as measured at 2.0.4.0 and 2.0.4.9 | No: no playing file that old has an offset to the side or along the map (two have 0.05 of a unit, read as none) |
| A spawn offset in a file with no `GameVersion`, or one that can't be read | Not turned, as from 2.0.5.0 on | No: every installed file has the key |
| The older offset keys (`SpawnXOffset`, `SpawnYOffset`, `VerticalSpawnOffset`) on a map with a `MapScale` | Not scaled, like the newer keys | No |
| An older offset key below 0.1 of a unit | No offset: it would move a bot by less than a millimetre | No |
| A helper bot's spawn offset | Along the map's axes like a target's, at the middle of its range | No: measured on targets only. One playing file has one |
| The player's own character's newer spawn offset keys | The middle of the range, along the map's axes, turned a quarter in a file saved before 2.0.5.0 like a bot's | No: the older keys were measured for the player, the newer ones for bots only |
| `VerticalSpawnOffset` on a player's character that falls | Refused: where the player lands wasn't measured | No |
| The player's older offset keys on a map with a `MapScale`, below 0.1 of a unit, or on a spawn point that is turned | As for a bot: not scaled, no offset below 0.1, along the map's axes | No: measured at scale 1 on a point that isn't turned |
| The player's older offset keys together with newer ones that aren't zero | Refused, as for a bot | No |
| `StandStillUntilHurt` on a bot that is pushed | The pushes move it only once it has been hurt | No |
| A flier's vertical speed when it is lifted | The push is added to it like any other speed | No |
| A target bouncing on a push cushion | Not copied: in KovaaK's it lost about 4% of its height per cycle, cause unknown | Open |
| A bot with lives that drains away (negative regen) | It counts as a timeout and uses up no life: only kills do, so the run isn't ended by drained bots | No |
| Lives on a slot whose rotation is picked by weight | Still N kills, whichever bots come | No: measured on rotations played in order |
| A fractional `BotMaxLives` | Rounded to the nearest whole number, at least 1 | No |
| The score on screen with a score for time | One formula all run: the points so far plus the rate times the game seconds left now, so it counts down | No |
| A score multiplier (accuracy or damage efficiency) with a score for time | Applies to the whole score, the time points too (the note says it's unchecked) | No: the time score was measured with no multiplier |
| Accuracy and damage efficiency on one scenario | Both apply, one after the other (a note says it's unchecked) | No |
| Damage efficiency on a score that has kills, hits or misses in it | The multiplier applies to the whole score (a note says it's unchecked) | No: measured on a score by damage only |
| Overkill under damage efficiency | Damage done stops at the bot's health left, and the shot's full damage counts as possible | No |
| The beam's damage possible | Its damage for every tick it fires, on target or off, as the run file writes it | No |
| Damage efficiency with no shots fired | No cut, as with the accuracy multiplier | No |
| A character with no `Gravity` key | A moving bot gets gravity 1.0 (so a flyer without the key falls when no key is down), and a bot that never moves doesn't fall | No: every installed character profile has the key |
| A bot without gravity that appears within its `StepUpHeight` of a floor | Left where it appears | No: measured on bots with gravity |
| A walking bot that meets a step within its `StepUpHeight` | Not built: only the moment a bot appears is | No |
| Kinds that differ in `BlockedSpawnRadius` | Every kind uses the first kind's | No: 1 playing file |
| `JetpackFuelRegensInAir` off | The tank refills only while the bot is on the ground with the jump key up | Partly: one bot with the key held on the floor never refilled |
| The wait after a jetpack's tank runs dry | 0.25 s from the moment it runs dry, whatever the key does in that time | Partly: about 0.26 s on film with the key held throughout |
| `JetpackFullFuelTime` of 0 or less | An endless tank | No: no installed file has it |
| A jetpack with no `JetpackActivationDelay` key | No delay: it thrusts at once (only a delay written as 0 never thrusts) | No: every playing jetpack character has the key |
| A jetpack with no fuel keys | An endless tank when `JetpackFullFuelTime` is missing, no refill when `JetpackFuelIncPerSec` is, and a refill in the air when `JetpackFuelRegensInAir` is | No: every playing jetpack character has all three |
| A jetpack bot with air jumps, pressing in the air | The press makes an air jump and starts the thrust | No: no playing file has both |
| A bot with a jump chance that appears standing | It decides as it appears, and jumps at once if the key is down | Partly: measured on bots that came down from their spawn points |
| A jetpack bot's press in the air, with `AllowBufferedJumps` | Buffered like any other bot's: a jump at the landing if the key is still held | No |
| The press that made an air jump, with `AllowBufferedJumps` | Not buffered for the landing | No: the film fits both readings equally well |
| `JumpVelocityMin` below `JumpVelocityMax` | Each jump draws a speed evenly between the two | Partly: a 200 to 800 bot jumped to a different height each time, and the spread wasn't fitted |
| The jump key when a bot changes dodge profile | Keeps its state, and the new profile's first decision comes one of its windows later | No |
| A dodge profile with no `AlterateJumpCrouchInput` key | Alternate on, as the trainer played every flyer before K6 | No: no playing flyer's profile lacks it |
| A flyer with gravity and `AlterateJumpCrouchInput` on | Falls during the swap pause | No: no installed file has one |
| A flyer with gravity, crouch key held | Flies down at the flight speed, as without gravity | No |
| `FlightObeysPitch` | Not read | No: 15 playing kinds have it |
| `AIDamageReactionResetTimer` running out | Clears the damage counted so far | No: taken from the name. Measured: the timer starts again at each hit |
| An ability's own rules, `BlockAbilityOnStartDuration` and `AbilityGlobalCooldown` when a reaction comes due | They hold it back, and the reaction is dropped | No |
| A reaction that comes due while an ability is going on, or at the moment one ends | Dropped, like a try of the key | No |
| A trigger that is ignored while a reaction waits | The waiting reaction stays | No |
| A tick of the key and a reaction due at the same moment | The tick goes first | No |
| Two abilities' reactions due at the same moment | In the order of the abilities' list | No |
| A reaction's keys that a file leaves out | The values the editor writes for a new ability: a threshold of 0, a reset timer of 0.1 s, no chance to ignore, a wait of 0.125 to 0.25 s | No: every test map had them all |
| `AIDamageReactionThreshold` above a million | Cut to a million | No: no local file comes near |
| `AIDamageReactionCooldown` | Not read | Yes: it did nothing in any test |
| A reaction on an ability with no duration, or on a melee ability | Refused | Partly: an impulse's reaction sometimes showed nothing, cause unknown. A melee ability wasn't tried |
| A press of a weapon ability's key that the weapon doesn't answer | Still uses up the ability's charge and counts as a use | No |
| A weapon ability's charge that is back exactly at a try | Not there for that try, as measured for movement abilities | No |
| A weapon ability's distance and view rules, `DelayAfterUse` and `AbilityGlobalCooldown` | Read as a movement ability's are | No: the test maps had none |
| A blast still waiting when its bot is killed or dies | Dropped | No |
| A bot that dies of its own blast, on a scenario with lives | It counts as a timeout and uses up no life | No: measured without lives, where it gave no kill, shot or damage |
| A weapon with no `SelfDamageMultiplier` key | 1: the blast does its bot its full damage | No: every local fuse weapon has the key |
| A fuse bot's projectile | Never drawn, and it hits nothing on its way: the importer takes only a blast that stays inside its own bot | No |
| `MainBBHeadOffset` below minus twice the head's radius | Placed by the same stacking rule as a shallower head | Only at exactly minus twice the radius, where the head lay inside a ball and every hit was a body hit |
| A ball with a head under gravity | Stands on the ball's bottom, as tall as the stacking rule says | No: measured on capsules |
| A head much bigger than its body | Placed by the stacking rule | Partly: the bottom fit, the top came out 2 to 6 uu low |
| What a bot with a head bumps into | A capsule of its body's radius and the stack's height, even when the head is wider than the body | No |
| A headshots-only body in front of another bot | It blocks the shot, which is a miss | No: the sim's shots stop at the first surface |
| Possible damage for a weapon that can headshot, in a file where no shootable bot has a head and the score doesn't use damage efficiency | Left at the plain damage, so the files that already played keep their fingerprints and bests | Measured, and left out on purpose: KovaaK's counts damage times the multiplier |
| A head on a cuboid, a head radius of 0 or less, headshots only without a head or with a weapon that can't headshot, a headshot multiplier of 0 or less | Refused | No |
| `EnemyHeadColor` | The head's colour. Unset or black gives AimAdapt's own head colour | No |
| A head with a radius under 1 uu | Played at its size, a speck that can hardly be hit. Files use one where they want no head worth hitting | No |
