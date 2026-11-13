//! Tests for where a bot with a head starts when its stack is inside a wall. Every file is made up here: a ball body
//! 0.3 units across (radius) under a head of radius 40 at an offset of 200, which makes a stack 280.6 units tall
//! standing on the bottom of its body, with the stack's bottom 139.7 units under the spawn point. Positions are in
//! Unreal units (X forward, Y right, Z up) as the map has them, and the player starts at the origin.

use aimadapt_sim::{Sim, Vec3};

use super::super::*;
use super::FEATURE;

const HEADER: &str = "Name=AimAdapt Test - Mote Test
AimTypeTag=Clicking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Mote.bot
BotTeams=2
ScorePerKill=100.0
MapScale=1.0
InvinciblePlayer=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
WeaponProfileNames=Mote Pistol;;;
[Weapon Profile]
Name=Mote Pistol
Type=Hitscan
Category=SemiAuto
DamagePerShot=100.0
TimeBetweenShots=0.2
[Bot Profile]
Name=Mote
CharacterProfile=Mote Ball
UseWeapons=false
[Character Profile]
Name=Mote Ball
MaxHealth=100.0
MaxSpeed=0.0
Gravity=1.0
MainBBType=Spheroid
MainBBRadius=0.3
MainBBHeight=0.6
MainBBHasHead=true
MainBBHeadRadius=40.0
MainBBHeadOffset=200.0
";

/// A `Cube` brush from its lowest corner, with a scale in steps of 100 units.
fn cube(at: [f32; 3], scale: [f32; 3]) -> String {
    format!(
        r#"{{"type": "brush", "name": "Default", "mesh": "Cube", "location": "{}, {}, {}", "rotation": "0, 0, 0", "scale": "{}, {}, {}"}}"#,
        at[0], at[1], at[2], scale[0], scale[1], scale[2]
    )
}

fn spawn_point(at: [f32; 3], team: u32, yaw: f32) -> String {
    format!(
        r#"{{"type": "gameObject", "name": "SpawnPoint", "location": "{}, {}, {}", "rotation": "0, 0, {yaw}", "scale": "1, 1, 1", "properties": [{{"name": "TeamMask", "value": {team}}}, {{"name": "Weight", "value": 1.0}}]}}"#,
        at[0], at[1], at[2]
    )
}

/// The file with a map of these objects, the player starting at the origin turned `yaw` degrees and one bot spawn
/// point at `bot`.
fn file(objects: &[String], bot: [f32; 3], yaw: f32) -> String {
    let mut all = vec![spawn_point([0.0; 3], 1, yaw), spawn_point(bot, 2, 0.0)];
    all.extend_from_slice(objects);
    format!("{HEADER}[Map Data]\n{{\"version\": \"1.0.0\", \"objects\": [\n{}\n]}}\n", all.join(",\n"))
}

/// A block 600 units square and 300 thick, its top at z = -100: x from 1000 to 1600, y from -300 to 300.
fn block() -> String {
    cube([1000.0, -300.0, -400.0], [6.0, 6.0, 3.0])
}

/// A wall 100 thick and 700 tall (from -400 to 300) against the block's far edge, at x = 1600.
fn wall_behind() -> String {
    cube([1600.0, -300.0, -400.0], [1.0, 6.0, 7.0])
}

/// A bot spawn point 90 units from the block's far edge.
const NEAR_FAR_EDGE: [f32; 3] = [1510.0, 0.0, -100.0];

fn convert_text(text: &str) -> Result<Imported, Vec<Gap>> {
    convert(&Sce::parse(text), "mote.sce", "0")
}

fn imported(text: &str) -> Imported {
    convert_text(text).unwrap_or_else(|e| panic!("{:?}", e.iter().map(ToString::to_string).collect::<Vec<_>>()))
}

/// The reasons the file is refused for, as they read.
fn refused(text: &str) -> Vec<String> {
    convert_text(text).err().unwrap_or_default().iter().map(ToString::to_string).collect()
}

/// The one reason a file is refused for, which has to be the feature this module owns.
#[track_caller]
fn reason(text: &str) -> String {
    let r = refused(text);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with(FEATURE), "{r:?}");
    r[0].clone()
}

fn first_spawn(text: &str) -> Vec3 {
    Vec3::from_array(imported(text).scenario.world.unwrap().spawns[0].pos)
}

/// Where a spawn point at map position (`forward`, `right`, `up`) is in the sim's frame, with the bot's middle
/// 0.6 units above it, for a player starting at the origin turned by `yaw` degrees.
fn frame(forward: f32, right: f32, up: f32, yaw: f32) -> Vec3 {
    let turn = glam::Quat::from_rotation_y(yaw.to_radians());
    turn * Vec3::new(right / 100.0, (up + 0.6) / 100.0, -forward / 100.0)
}

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-3
}

/// Bug: a bot with a head whose stack started inside a block on the map was lifted straight up out of it by the
/// sim, wherever the block's edge was. KovaaK's moved it sideways to a wall flush against the edge and stood it
/// on the block there (measured, 12 November 2026, with a wall on each of three sides). The spawn point moves
/// toward that edge until the body's middle is its radius and 0.1 of a unit short of the wall.
#[test]
fn a_bot_beside_a_flush_wall_is_moved_to_the_wall_whichever_side_it_is_on() {
    // The wall behind the block, 90 units from the spawn point: it ends 0.4 short of the wall's face.
    let behind = first_spawn(&file(&[block(), wall_behind()], NEAR_FAR_EDGE, 0.0));
    assert!(close(behind, frame(1599.6, 0.0, -100.0, 0.0)), "{behind}");
    // On the left, and on the right.
    let left = cube([1000.0, -400.0, -400.0], [6.0, 1.0, 7.0]);
    let at = first_spawn(&file(&[block(), left], [1300.0, -210.0, -100.0], 0.0));
    assert!(close(at, frame(1300.0, -299.6, -100.0, 0.0)), "{at}");
    let right = cube([1000.0, 300.0, -400.0], [6.0, 1.0, 7.0]);
    let at = first_spawn(&file(&[block(), right], [1300.0, 210.0, -100.0], 0.0));
    assert!(close(at, frame(1300.0, 299.6, -100.0, 0.0)), "{at}");
    // The player's start is turned by an angle that isn't a quarter turn: the block is not lined up with the
    // frame any more, and the bot still ends against the wall.
    for yaw in [30.0, -77.0, 135.0] {
        let at = first_spawn(&file(&[block(), wall_behind()], NEAR_FAR_EDGE, yaw));
        assert!(close(at, frame(1599.6, 0.0, -100.0, yaw)), "yaw {yaw}: {at}");
    }
}

/// A bot with a head that falls and starts inside a block whose top is the shortest way out is lifted onto it by
/// the sim, as before (measured: a block 300 thick and 360 wide, every side 180 away): no move and no reason.
#[test]
fn a_bot_whose_shortest_way_out_is_up_is_left_where_it_is() {
    let wide = cube([1000.0, -180.0, -400.0], [3.6, 3.6, 3.0]);
    let text = file(&[wide], [1180.0, 0.0, -100.0], 0.0);
    assert_eq!(refused(&text), Vec::<String>::new());
    assert!(close(first_spawn(&text), frame(1180.0, 0.0, -100.0, 0.0)));
    // A wall beside the block doesn't matter then.
    let text = file(&[block(), wall_behind()], [1300.0, 0.0, -100.0], 0.0);
    assert!(close(first_spawn(&text), frame(1300.0, 0.0, -100.0, 0.0)));
}

/// A headed bot whose stack starts clear of everything gets no move and no reason: the spawn point is 160 units over
/// the block, so the stack's bottom is 20 units over it.
#[test]
fn a_stack_that_starts_clear_of_the_floor_is_left_alone() {
    let text = file(&[block(), wall_behind()], [1510.0, 0.0, 60.0], 0.0);
    assert_eq!(refused(&text), Vec::<String>::new());
    assert!(close(first_spawn(&text), frame(1510.0, 0.0, 60.0, 0.0)));
    // And one in a map with no walls at all.
    let text = file(&[], [1510.0, 0.0, -100.0], 0.0);
    assert!(close(first_spawn(&text), frame(1510.0, 0.0, -100.0, 0.0)));
}

/// KovaaK's pushes a bot standing near the edge of a platform off it, sideways, and it falls (measured: a block
/// 200 wide with sides 100 away, nothing beside it). That isn't built, so the file is refused with a reason that
/// says so.
#[test]
fn a_bot_pushed_off_a_ledge_with_nothing_beside_it_is_refused() {
    // 60 units from the left edge, 140 from the right, a long block.
    let platform = cube([1000.0, -100.0, -400.0], [6.0, 2.0, 3.0]);
    let r = reason(&file(&[platform], [1300.0, -40.0, -100.0], 0.0));
    assert!(r.contains("pushes it off"), "{r}");
    // A wall at the edge that doesn't reach the whole bot (50 units over the block's top, where the stack is 280)
    // is something else that wasn't measured.
    let low = cube([1000.0, -200.0, -400.0], [6.0, 1.0, 3.5]);
    let platform = cube([1000.0, -100.0, -400.0], [6.0, 2.0, 3.0]);
    let r = reason(&file(&[platform.clone(), low], [1300.0, -40.0, -100.0], 0.0));
    assert!(r.contains("doesn't cover"), "{r}");
    // A wall with a gap of 20 units to the edge is no wall flush against it.
    let apart = cube([1000.0, -320.0, -400.0], [6.0, 1.0, 7.0]);
    let r = reason(&file(&[platform.clone(), apart], [1300.0, -40.0, -100.0], 0.0));
    assert!(r.contains("pushes it off"), "{r}");
    // A wall flush against the edge that stops short of the bot along it (the bot is at x = 1300, the wall starts
    // at 1400), and one that floats 50 units over the block's top.
    let short = cube([1400.0, -200.0, -400.0], [2.0, 1.0, 7.0]);
    let r = reason(&file(&[platform.clone(), short], [1300.0, -40.0, -100.0], 0.0));
    assert!(r.contains("pushes it off"), "{r}");
    let floating = cube([1000.0, -200.0, -50.0], [6.0, 1.0, 4.0]);
    let r = reason(&file(&[platform, floating], [1300.0, -40.0, -100.0], 0.0));
    assert!(r.contains("doesn't cover"), "{r}");
}

/// Where two ways out are almost as short (within 5 percent), which one KovaaK's takes wasn't measured.
#[test]
fn a_near_tie_between_two_ways_out_is_refused() {
    // Up is 139.7 and each side 140.3 away.
    let narrow = cube([1000.0, -140.0, -400.0], [6.0, 2.8, 3.0]);
    let r = reason(&file(&[narrow], [1300.0, 0.0, -100.0], 0.0));
    assert!(r.contains("almost as short"), "{r}");
    // A block 200 wide, the spawn point in its middle: both sides are 100.3 away.
    let platform = cube([1000.0, -100.0, -400.0], [6.0, 2.0, 3.0]);
    let r = reason(&file(&[platform.clone(), wall_behind()], [1300.0, 0.0, -100.0], 0.0));
    assert!(r.contains("almost as short"), "{r}");
    // A little more than 5 percent apart is no tie: 60.3 and 140.3 (the earlier case) is, but 100.3 and 106 isn't.
    let r = reason(&file(&[platform], [1300.0, -6.0, -100.0], 0.0));
    assert!(r.contains("pushes it off"), "{r}");
}

/// Only a bot that falls is moved or left for the sim to lift: a floating one or one with no motion starts
/// exactly where its spawn point is, inside the wall, so the file is refused.
#[test]
fn a_bot_that_does_not_fall_is_refused() {
    let text = file(&[block(), wall_behind()], NEAR_FAR_EDGE, 0.0);
    assert!(refused(&text).is_empty());
    let floats = text.replace("MaxSpeed=0.0\nGravity=1.0", "MaxSpeed=0.0\nGravity=0.0");
    assert!(reason(&floats).contains("doesn't fall"));
    let no_key = text.replace("MaxSpeed=0.0\nGravity=1.0", "MaxSpeed=0.0");
    assert!(reason(&no_key).contains("doesn't fall"));
    // Moving by itself with no gravity floats too.
    let flies = text.replace("MaxSpeed=0.0\nGravity=1.0", "MaxSpeed=300.0\nGravity=0.0");
    assert!(reason(&flies).contains("doesn't fall"));
    // The same for the case where the sim would do the lifting.
    let wide = cube([1000.0, -180.0, -400.0], [3.6, 3.6, 3.0]);
    let up = file(&[wide], [1180.0, 0.0, -100.0], 0.0).replace("MaxSpeed=0.0\nGravity=1.0", "MaxSpeed=0.0");
    assert!(reason(&up).contains("doesn't fall"));
}

/// A way out down through the bottom of a thin block, a bot inside two blocks at once, a tilted block and a range
/// of spawn offsets inside a block are not measured.
#[test]
fn the_other_cases_inside_a_wall_are_refused() {
    // A slab 50 thick around the stack's top: down (60.9) is the shortest way out.
    let slab = cube([1000.0, -300.0, -20.0], [6.0, 6.0, 0.5]);
    let r = reason(&file(&[slab], [1300.0, 0.0, -100.0], 0.0));
    assert!(r.contains("down"), "{r}");
    // Inside two blocks at once.
    let r = reason(&file(&[block(), block()], NEAR_FAR_EDGE, 0.0));
    assert!(r.contains("more than one"), "{r}");
    // A tilted block, with the spawn point in its middle, 250 units up (the stack has to cross its top).
    let place = Placement { location: [1000.0, -300.0, -400.0], rotation: [0.0, 20.0, 0.0], scale: [6.0, 6.0, 3.0] };
    let middle = transform(&place, [50.0, 50.0, 250.0 / 3.0]);
    let tilted = cube([1000.0, -300.0, -400.0], [6.0, 6.0, 3.0])
        .replace(r#""rotation": "0, 0, 0""#, r#""rotation": "0, 20, 0""#);
    let r = reason(&file(&[tilted], middle, 0.0));
    assert!(r.contains("upright"), "{r}");
    // A ceiling: the block's top is above the whole bot, which crosses only its bottom.
    let ceiling = cube([1000.0, -300.0, -20.0], [6.0, 6.0, 3.0]);
    let r = reason(&file(&[ceiling], [1300.0, 0.0, -100.0], 0.0));
    assert!(r.contains("top"), "{r}");
    // A range of spawn offsets that stays inside the block.
    let text = file(&[block(), wall_behind()], NEAR_FAR_EDGE, 0.0);
    let offset =
        text.replace("MainBBHeadOffset=200.0", "MainBBHeadOffset=200.0\nSpawnOffsetMax=X=0.000 Y=50.000 Z=0.000");
    assert!(reason(&offset).contains("offset"));
    // A spawn point 1 unit under the block's top, beside the wall: the bot's middle (0.6 over the spawn point)
    // starts inside the block, where the sim would push it into the wall. Half a unit over the top it is moved.
    let r = reason(&file(&[block(), wall_behind()], [1510.0, 0.0, -101.0], 0.0));
    assert!(r.contains("its middle starts under the top"), "{r}");
    let over = first_spawn(&file(&[block(), wall_behind()], [1510.0, 0.0, -99.5], 0.0));
    assert!(close(over, frame(1599.6, 0.0, -99.5, 0.0)), "{over}");
    // The bot's axis nearer the block's edge than its radius (0.2 inside it), and just past the edge (0.1): the
    // stack only grazes the block's side.
    let r = reason(&file(&[block()], [1599.8, 0.0, -100.0], 0.0));
    assert!(r.contains("middle over it"), "{r}");
    let r = reason(&file(&[block()], [1600.1, 0.0, -100.0], 0.0));
    assert!(r.contains("middle over it"), "{r}");
}

/// The file with these settings added to the bot's character.
fn with_settings(text: &str, lines: &str) -> String {
    assert_eq!(text.matches("MainBBHeadOffset=200.0\n").count(), 1);
    text.replace("MainBBHeadOffset=200.0\n", &format!("MainBBHeadOffset=200.0\n{lines}\n"))
}

/// One fixed spawn offset, written with the newer settings: both ends the same.
fn fixed(x: f32, y: f32, z: f32) -> String {
    format!("SpawnOffsetMin=X={x:.3} Y={y:.3} Z={z:.3}\nSpawnOffsetMax=X={x:.3} Y={y:.3} Z={z:.3}")
}

/// Where the first bot is after `seconds` of play with nobody shooting: its middle, and the bottom of its stack.
fn settled(text: &str, seconds: f32) -> (Vec3, f32) {
    let i = imported(text);
    assert!(i.scenario.problems().is_empty(), "{:?}", i.scenario.problems());
    let mut sim = Sim::new(i.scenario, 3);
    let idle = aimadapt_sim::Input { aim: Vec3::NEG_Z, fire_pressed: false, fire_held: false };
    for _ in 0..(seconds / 0.01) as usize {
        sim.step(0.01, idle);
    }
    let t = &sim.targets()[0];
    (t.pos, t.pos.y - t.stack_height() / 2.0)
}

/// Bug: a speck under a head with a spawn offset was looked at where its spawn point is, so a file whose offset
/// lifts the stack clear of the floor was refused for a bot that starts inside a wall. KovaaK's adds a fixed
/// spawn offset first and then looks whether the bot is inside a block, and moves it out the shortest way from
/// the place the offset gives (measured, 12 November 2026: specks on blocks 300 thick with offsets of 150 and
/// 100 up and of 60 level).
#[test]
fn a_fixed_spawn_offset_counts_before_the_bot_is_looked_at() {
    // At the spawn point the far edge is the nearest way out and a wall stands there. 150 up, the stack is 10.3
    // clear of the block: no move and no reason, and the bot falls onto the block where it is.
    let beside = file(&[block(), wall_behind()], NEAR_FAR_EDGE, 0.0);
    for lines in [fixed(0.0, 0.0, 150.0), "VerticalSpawnOffset=150.0".to_string()] {
        let text = with_settings(&beside, &lines);
        assert_eq!(refused(&text), Vec::<String>::new(), "{lines}");
        assert!(close(first_spawn(&text), frame(1510.0, 0.0, -100.0, 0.0)), "{lines}");
        let (at, bottom) = settled(&text, 8.0);
        let want = frame(1510.0, 0.0, -100.0, 0.0);
        assert!((at.x - want.x).abs() < 2e-3 && (at.z - want.z).abs() < 2e-3, "{lines}: {at} not {want}");
        assert!((bottom + 1.0).abs() < 2e-3, "{lines}: the stack's bottom is at {bottom}");
    }
    // 100 up on a block 200 wide the stack is 39.7 inside it, with the sides 100.3 away: up is the shortest way
    // out, which the sim does. At the spawn point the two sides would have tied.
    let platform = cube([1000.0, -100.0, -400.0], [6.0, 2.0, 3.0]);
    let on_platform = file(&[platform], [1300.0, 0.0, -100.0], 0.0);
    assert!(reason(&on_platform).contains("almost as short"));
    let text = with_settings(&on_platform, &fixed(0.0, 0.0, 100.0));
    assert_eq!(refused(&text), Vec::<String>::new());
    assert!(close(first_spawn(&text), frame(1300.0, 0.0, -100.0, 0.0)));
    let (_, bottom) = settled(&text, 0.0);
    assert!((bottom + 1.0).abs() < 2e-3, "the stack's bottom is at {bottom}");
    // 60 to the right on a block 360 wide, where up was the shortest way out (139.7), a side is nearer (120.3):
    // KovaaK's pushes the bot off, which isn't played.
    let long = cube([1000.0, -180.0, -400.0], [6.0, 3.6, 3.0]);
    let on_long = file(&[long], [1300.0, 0.0, -100.0], 0.0);
    assert_eq!(refused(&on_long), Vec::<String>::new());
    let r = reason(&with_settings(&on_long, &fixed(0.0, 60.0, 0.0)));
    assert!(r.contains("pushes it off"), "{r}");
    // A file older than KovaaK's 2.0.5.0 turns a level offset a quarter: the same 60 runs along the block, where
    // up stays the shortest way out.
    let older = on_long.replace("MapScale=1.0\n", "MapScale=1.0\nGameVersion=2.0.4.0\n");
    assert_ne!(older, on_long);
    let text = with_settings(&older, &fixed(0.0, 60.0, 0.0));
    assert_eq!(refused(&text), Vec::<String>::new());
    assert!(close(first_spawn(&text), frame(1300.0, 0.0, -100.0, 0.0)));
    // The offsets run along the map's axes however the spawn point is turned: with the point turned a quarter
    // the 60 still runs across the block.
    let (was, now) = (r#""1300, 0, -100", "rotation": "0, 0, 0""#, r#""1300, 0, -100", "rotation": "0, 0, 90""#);
    assert_eq!(on_long.matches(was).count(), 1);
    let r = reason(&with_settings(&on_long.replace(was, now), &fixed(0.0, 60.0, 0.0)));
    assert!(r.contains("pushes it off"), "{r}");
    // An offset that leaves the bot inside the block beside its wall (210 forward from the middle of the block,
    // so 90 from the far edge) wasn't measured.
    let middle = file(&[block(), wall_behind()], [1300.0, 0.0, -100.0], 0.0);
    assert_eq!(refused(&middle), Vec::<String>::new());
    let r = reason(&with_settings(&middle, &fixed(210.0, 0.0, 0.0)));
    assert!(r.contains("spawn offset") && r.contains("beside a wall"), "{r}");
}

/// A range of spawn offsets puts each bot somewhere in it, and only fixed offsets were measured inside a block:
/// the file plays when the stack is clear of the walls everywhere in the range, and is refused otherwise.
#[test]
fn a_range_of_spawn_offsets_has_to_stay_clear_of_the_walls() {
    let beside = file(&[block(), wall_behind()], NEAR_FAR_EDGE, 0.0);
    let up = |lo: f32, hi: f32| {
        format!("SpawnOffsetMin=X=0.000 Y=0.000 Z={lo:.3}\nSpawnOffsetMax=X=0.000 Y=0.000 Z={hi:.3}")
    };
    // From 150 to 200 up, the stack is clear all the way.
    let text = with_settings(&beside, &up(150.0, 200.0));
    assert_eq!(refused(&text), Vec::<String>::new());
    assert!(close(first_spawn(&text), frame(1510.0, 0.0, -100.0, 0.0)));
    // A range that reaches into the block at one end, whichever end is written first.
    for (lo, hi) in [(100.0, 200.0), (200.0, 100.0), (0.0, 150.0)] {
        let r = reason(&with_settings(&beside, &up(lo, hi)));
        assert!(r.contains("range of spawn offsets"), "{lo} to {hi}: {r}");
    }
    // Both ends and the middle clear, with a post a quarter of the way along: the spawn point is 160 over the
    // block (the stack 20 clear of it), the range runs 200 to the left and 200 to the right, and the post
    // stands 100 to the left. The post ends inside the stack's height, so the stack crosses its top: a stack
    // wholly inside a wall touches none of its faces and counts as clear.
    let post = cube([1290.0, -110.0, -100.0], [0.2, 0.2, 2.0]);
    let over = file(&[block(), post], [1300.0, 0.0, 60.0], 0.0);
    let along = "SpawnOffsetMin=X=0.000 Y=-200.000 Z=0.000\nSpawnOffsetMax=X=0.000 Y=200.000 Z=0.000";
    assert_eq!(refused(&over), Vec::<String>::new());
    let r = reason(&with_settings(&over, along));
    assert!(r.contains("range of spawn offsets"), "{r}");
    // Without the post the same range plays.
    let open = file(&[block()], [1300.0, 0.0, 60.0], 0.0);
    assert_eq!(refused(&with_settings(&open, along)), Vec::<String>::new());
}

/// Bug: the first build of this looked at every bot with a head, and stopped a file that played, whose wide
/// bodies under heads start inside the map's floor. Only a speck of a body (under 1 unit across its radius) was
/// measured, so only a speck is looked at: a wider body starts where it did before.
#[test]
fn a_wider_body_under_a_head_starts_where_it_did() {
    let platform = cube([1000.0, -100.0, -400.0], [6.0, 2.0, 3.0]);
    let at = [1300.0, -40.0, -100.0];
    let on_ledge = file(&[platform], at, 0.0);
    assert!(reason(&on_ledge).contains("pushes it off"));
    // 0.9 of a unit is still a speck.
    assert!(reason(&on_ledge.replace("MainBBRadius=0.3", "MainBBRadius=0.9")).contains("pushes it off"));
    for radius in ["1.0", "2.0", "40.0"] {
        let body = format!("MainBBRadius={radius}");
        let wide = on_ledge.replace("MainBBRadius=0.3", &body);
        assert_eq!(refused(&wide), Vec::<String>::new(), "{radius}");
        let open = file(&[], at, 0.0).replace("MainBBRadius=0.3", &body);
        assert!(close(first_spawn(&wide), first_spawn(&open)), "{radius}");
    }
}

/// Bots of a spawn volume appear anywhere in it, so nothing measured at a point says where they go: a volume
/// stays where it is, even with its corner where a spawn point would be moved.
#[test]
fn a_spawn_volume_is_left_as_it_is() {
    let point = file(&[block(), wall_behind()], NEAR_FAR_EDGE, 0.0);
    let (was, now) = (r#""name": "SpawnPoint", "location": "1510,"#, r#""name": "SpawnVolume", "location": "1510,"#);
    assert_eq!(point.matches(was).count(), 1);
    let volume = point.replace(was, now);
    assert!(close(first_spawn(&point), frame(1599.6, 0.0, -100.0, 0.0)));
    assert!(close(first_spawn(&volume), frame(1510.0, 0.0, -100.0, 0.0)));
}

/// The file with a second kind of bot on the same spawn point: a copy of the first's bot and character, with the
/// pairs of text in `patch` changed in the copy.
fn with_second(base: &str, patch: &[(&str, &str)]) -> String {
    let from = base.find("[Bot Profile]").unwrap();
    let to = base.find("[Map Data]").unwrap();
    let mut copy = base[from..to].replace("Name=Mote\n", "Name=Mote Two\n").replace("Mote Ball", "Mote Two Ball");
    for (old, new) in patch {
        assert!(copy.contains(old), "{old}");
        copy = copy.replacen(old, new, 1);
    }
    let text =
        base.replace("AddedBots=Mote.bot", "AddedBots=Mote.bot;Mote Two.bot").replace("BotTeams=2", "BotTeams=2;2");
    text.replace("[Map Data]", &format!("{copy}[Map Data]"))
}

/// A second kind of bot on the same spawn point: each kind works out its own move, and the file plays only if they
/// all end in the same place. A body of another colour does. One with a wider body ends a little farther from the
/// wall, and one with no head stays where the spawn point is (inside the block, where the sim lifts it), so those
/// two are refused.
#[test]
fn characters_that_would_start_in_different_places_are_refused() {
    let base = file(&[block(), wall_behind()], NEAR_FAR_EDGE, 0.0);
    let twin = with_second(&base, &[("MaxHealth=100.0", "MaxHealth=100.0\nEnemyBodyColor=X=0.1 Y=0.2 Z=0.3")]);
    let w = imported(&twin).scenario.world.unwrap();
    assert_eq!(w.kinds.len(), 2);
    assert!(close(Vec3::from_array(w.spawns[0].pos), frame(1599.6, 0.0, -100.0, 0.0)), "{:?}", w.spawns[0].pos);
    let wider = with_second(&base, &[("MainBBRadius=0.3", "MainBBRadius=0.5")]);
    assert!(reason(&wider).contains("different places"));
    let bare =
        with_second(&base, &[("MainBBHasHead=true", "MainBBHasHead=false"), ("MainBBRadius=0.3", "MainBBRadius=40.0")]);
    assert!(reason(&bare).contains("different places"));
}

/// What a falling bot does in the sim after the move: it starts at the wall's edge with its body's bottom on the
/// block's top (a stack of 280.6 units, so its middle is 140.3 above it), where before it was lifted at the
/// spawn point and ended a block's width from the wall. The player's start is turned 30 degrees.
#[test]
fn the_sim_stands_the_moved_bot_on_the_block_at_the_wall() {
    let yaw = 30.0;
    let i = imported(&file(&[block(), wall_behind()], NEAR_FAR_EDGE, yaw));
    assert!(i.scenario.problems().is_empty(), "{:?}", i.scenario.problems());
    let sim = Sim::new(i.scenario, 3);
    let t = &sim.targets()[0];
    let want = frame(1599.6, 0.0, -100.0, yaw);
    assert!((t.pos.x - want.x).abs() < 2e-3 && (t.pos.z - want.z).abs() < 2e-3, "{} not {want}", t.pos);
    // The block's top is 1 m under the player's eye.
    let bottom = t.pos.y - t.stack_height() / 2.0;
    assert!((bottom + 1.0).abs() < 2e-3, "the stack's bottom is at {bottom}");
}

/// A body that is a speck under a head plays: the imported targets have the radius, with a head and no problems.
#[test]
fn a_speck_of_a_body_with_a_head_plays_here() {
    let i = imported(&file(&[], NEAR_FAR_EDGE, 0.0));
    assert!((i.scenario.targets.radius - 0.003).abs() < 1e-7);
    assert!(i.scenario.targets.head.is_some());
    assert!(i.scenario.problems().is_empty());
    assert!(
        refused(&file(&[], NEAR_FAR_EDGE, 0.0).replace("MainBBHasHead=true", "MainBBHasHead=false"))
            .iter()
            .any(|r| r.starts_with("bot size"))
    );
}
