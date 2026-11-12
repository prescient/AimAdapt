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
| Fix: regeneration between shots | | 132 of 225 |
| K15 The rest: vertical dashes, damage reactions | | |
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

Recounted after K7, with 132 playing. K7 ran before the rest of K15 and made 16 files play. Of the 93 that don't play, 33 have one reason, 32 have two and 28 have three or more. Files each reason is on, and the files it blocks by itself: waypoint paths 28 and 4, player movement 25 and 5, heads 21 and 5, weapon abilities 19 and 6, map objects 17 and 1, a movement ability the rest of K15 covers 16 and 1, solid props 15 and 5, aim assist 11 and 1, aim down sights 9 and 1, several pellets per shot 8 and 3, player abilities 8 and none, bots that aim and fire 4 and none, `InvincibleBots` on the player's character 2 and none. K8 reaches 138 if it plays the six files that wait on weapon abilities alone.

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

### K8. Bots that destroy themselves

"Pressure" scenarios give each bot a weapon ability or a weapon that damages the bot itself after a delay, so a target that isn't shot in time goes away and counts against the player.

- Read the 19 files: the delay, what starts it (a timer from spawn, or the first hit), and what the score does when a bot dies that way.
- `aimadapt-sim`: a kind can have a fuse. The engine shows it only if KovaaK's does.
- The four armed-bot files left over from K7 belong here: their bots aim and fire.

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
| The countdown before a run, on a scenario with a time scale | Not scaled: it takes the same real time at any time scale | No |
| The timer on screen, on a scenario with a time scale | Shows game seconds left, so it counts down at the time scale per real second | No |
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
| The player's own character's spawn offset | The newer keys alone, at the middle of the range, along the player's own facing. The older keys and the file's version aren't applied | No: one playing file sets an older key there, of 1 unit |
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
