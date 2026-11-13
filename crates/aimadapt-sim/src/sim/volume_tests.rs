//! Tests for hurt volumes and deadly teleporters (plan K12a). The numbers come from test maps played in
//! KovaaK's 3.9.13, with its units divided by 100 (a ball of radius 40 is 0.4 m here).

use super::*;
use crate::scenario::Scenario;
use crate::world::{World, WorldKind, WorldSpawn, WorldVolume};

const FRAME: f32 = 1.0 / 240.0;

fn idle() -> Input {
    Input { aim: Vec3::NEG_Z, fire_pressed: false, fire_held: false }
}

/// A box from `lo` to `hi` along the sim's axes.
fn volume(lo: [f32; 3], hi: [f32; 3], kill: bool, damage: f32, cooldown_s: f32) -> WorldVolume {
    let d = |k: usize| hi[k] - lo[k];
    WorldVolume {
        corner: lo,
        edges: [[d(0), 0.0, 0.0], [0.0, d(1), 0.0], [0.0, 0.0, d(2)]],
        kill,
        damage,
        cooldown_s,
        shown: true,
    }
}

/// A box around the ball's place, big enough to hold it on every side.
fn around(kill: bool, damage: f32, cooldown_s: f32) -> WorldVolume {
    volume([-2.0, -2.0, -12.0], [2.0, 2.0, -8.0], kill, damage, cooldown_s)
}

/// One bot slot filled by a still ball of radius 0.4 and the given health that stands at (0, 0, -10) and
/// comes back `respawn_s` after it goes, on a map with `volumes`.
fn pen(volumes: Vec<WorldVolume>, health: f32, respawn_s: f32) -> Scenario {
    let e = crate::library::builtins().into_iter().find(|e| e.id == "builtin:static-grid").unwrap();
    let mut s = e.scenario.unwrap();
    s.timing.countdown_s = 0.0;
    s.timing.duration_s = 600.0;
    s.targets.count = 1;
    s.world = Some(World {
        spawns: vec![WorldSpawn { pos: [0.0, 0.0, -10.0], ..Default::default() }],
        kinds: vec![WorldKind { radius: 0.4, health, respawn_delay_s: respawn_s, ..Default::default() }],
        slots: vec![vec![(0, 1.0)]],
        volumes,
        ..Default::default()
    });
    s
}

/// When the ball goes (seconds into the run) and when it loses health, over `secs` of play.
fn timeline(s: Scenario, secs: f32) -> (Vec<f32>, Vec<f32>) {
    let mut sim = Sim::new(s, 1);
    let (mut gone, mut hurt) = (Vec::new(), Vec::new());
    let mut last = sim.targets().first().map(|t| (t.id, t.health));
    while sim.elapsed() < secs {
        let events = sim.step(FRAME, idle());
        gone.extend(events.iter().filter(|e| matches!(e, Event::TimedOut { .. })).map(|_| sim.elapsed()));
        let now = sim.targets().first().map(|t| (t.id, t.health));
        if let (Some((a, before)), Some((b, after))) = (last, now)
            && a == b
            && after < before - 1e-3
        {
            hurt.push(sim.elapsed());
        }
        last = now;
    }
    (gone, hurt)
}

fn assert_times(got: &[f32], want: &[f32], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: {got:?}, wanted about {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 0.05, "{what}: {got:?}, wanted about {want:?}");
    }
}

/// A volume with its near face 0.3 from the middle of a ball of radius 0.4 kills it on every side; with the
/// face 0.5 away the ball lives (measured: 30 against 50 with a ball of 40).
#[test]
fn a_body_only_has_to_touch_a_volume() {
    let sides = |gap: f32| {
        [
            volume([gap, -1.0, -12.0], [gap + 1.0, 1.0, -8.0], true, 0.0, 0.0),
            volume([-1.0, gap, -12.0], [1.0, gap + 1.0, -8.0], true, 0.0, 0.0),
            volume([-1.0, -gap - 1.0, -12.0], [1.0, -gap, -8.0], true, 0.0, 0.0),
        ]
    };
    for (gap, dies) in [(0.3, true), (0.5, false)] {
        for (side, v) in ["side", "above", "below"].into_iter().zip(sides(gap)) {
            let (gone, _) = timeline(pen(vec![v], 100.0, 600.0), 1.0);
            assert_eq!(!gone.is_empty(), dies, "{side} at {gap}");
        }
    }
}

/// A kill volume removes a touching body at once, ignores its cooldown, and does it again each time the
/// body comes back.
#[test]
fn kill_removes_a_body_at_once_and_again_after_each_respawn() {
    let (gone, hurt) = timeline(pen(vec![around(true, 0.0, 99.0)], 100.0, 1.0), 3.5);
    assert_times(&gone, &[0.0, 1.0, 2.0, 3.0], "deaths");
    assert!(hurt.is_empty(), "{hurt:?}");
}

/// First-frame rule, damage 100 and cooldown 5 with the body inside from the start: health 200 takes no
/// damage on its first frame, is hurt at 5 s and goes at 10 s.
#[test]
fn a_hit_on_a_bodys_first_frame_starts_the_cooldown_but_does_no_damage() {
    let (gone, hurt) = timeline(pen(vec![around(false, 100.0, 5.0)], 200.0, 600.0), 11.0);
    assert_times(&hurt, &[5.0], "hurt");
    assert_times(&gone, &[10.0], "gone");
}

/// Health 100, cooldown 5, back after 3 s: it dies at 0, and each time it comes back the pair's cooldown has
/// not run out, so it waits for the next multiple of 5.
#[test]
fn a_bot_that_returns_before_the_cooldown_ends_waits_for_it() {
    let (gone, _) = timeline(pen(vec![around(false, 100.0, 5.0)], 100.0, 3.0), 16.0);
    assert_times(&gone, &[0.0, 5.0, 10.0, 15.0], "deaths");
}

/// Health 100, cooldown 5, back after 7 s: the cooldown has run out each time, so it dies the moment it
/// comes back.
#[test]
fn a_bot_that_returns_after_the_cooldown_dies_at_once() {
    let (gone, _) = timeline(pen(vec![around(false, 100.0, 5.0)], 100.0, 7.0), 15.0);
    assert_times(&gone, &[0.0, 7.0, 14.0], "deaths");
}

/// Health 200, cooldown 2, back after 3 s: stamp at 0, hurt at 2, gone at 4, back at 7 with no damage, and
/// so on in a cycle of 7 s.
#[test]
fn a_cycle_of_stamp_hit_and_death_repeats_every_seven_seconds() {
    let (gone, hurt) = timeline(pen(vec![around(false, 100.0, 2.0)], 200.0, 3.0), 18.5);
    assert_times(&hurt, &[2.0, 9.0, 16.0], "hurt");
    assert_times(&gone, &[4.0, 11.0, 18.0], "gone");
}

/// Health 200, cooldown 5, back after 6 s: hurt at 5, gone at 10, back at 16 with no damage, hurt at 21.
#[test]
fn a_bot_back_after_a_longer_wait_gets_no_damage_on_its_first_frame() {
    let (gone, hurt) = timeline(pen(vec![around(false, 100.0, 5.0)], 200.0, 6.0), 22.0);
    assert_times(&hurt, &[5.0, 21.0], "hurt");
    assert_times(&gone, &[10.0], "gone");
}

/// Each pair of a body and a volume has its own cooldown. A mover of health 400 that passes through a narrow
/// volume into a long one is hit at 1.2 s (narrow), 2.2 s (long) and 7.2 s (long again) with cooldown 5.
#[test]
fn cooldowns_are_kept_for_each_volume_and_each_body() {
    let narrow = volume([1.6, -1.0, -12.0], [1.8, 1.0, -8.0], false, 100.0, 5.0);
    let long = volume([2.6, -1.0, -12.0], [40.0, 1.0, -8.0], false, 100.0, 5.0);
    let mut sim = Sim::new(pen(vec![narrow, long], 400.0, 600.0), 1);
    let mut hurt = Vec::new();
    let mut health = 400.0;
    for k in 1..=(8.0 / FRAME) as u32 {
        sim.targets[0].pos = Vec3::new(k as f32 * FRAME, 0.0, -10.0);
        sim.step(FRAME, idle());
        let now = sim.targets()[0].health;
        if now < health - 1e-3 {
            hurt.push(sim.elapsed());
        }
        health = now;
    }
    assert_times(&hurt, &[1.2, 2.2, 7.2], "hurt");
}

/// A volume that does 0 damage with no cooldown still counts as a hit each frame, so a touching bot's regen
/// delay keeps restarting and it does not heal, while the same bot outside heals.
#[test]
fn a_volume_with_no_damage_keeps_a_touching_bot_from_healing() {
    let healed = |volumes: Vec<WorldVolume>| {
        let mut s = pen(volumes, 100.0, 600.0);
        s.world.as_mut().unwrap().kinds[0].regen_per_s = 100.0;
        s.world.as_mut().unwrap().kinds[0].regen_delay_s = 0.5;
        let mut sim = Sim::new(s, 1);
        sim.targets[0].health = 10.0;
        for _ in 0..(2.0 / FRAME) as u32 {
            sim.step(FRAME, idle());
        }
        sim.targets()[0].health
    };
    assert!(healed(Vec::new()) > 90.0, "the control heals");
    assert!((healed(vec![around(false, 0.0, 0.0)]) - 10.0).abs() < 1e-3, "a touching bot does not");
}

/// A death by a volume is a timeout, not a kill: no hit, shot, damage or score is counted.
#[test]
fn a_death_by_a_volume_counts_a_timeout_and_nothing_else() {
    for (v, at_least) in
        [(around(true, 0.0, 0.0), 3), (around(false, 100000.0, 0.0), 3), (around(false, 100.0, 5.0), 1)]
    {
        let mut sim = Sim::new(pen(vec![v], 100.0, 0.5), 1);
        for _ in 0..(3.0 / FRAME) as u32 {
            sim.step(FRAME, idle());
        }
        let st = sim.stats();
        assert!(st.timeouts >= at_least, "{st:?}");
        assert_eq!((st.kills, st.hits, st.shots), (0, 0, 0), "{st:?}");
        assert_eq!(st.damage_done, 0.0);
        assert_eq!(sim.score(), sim.scenario().scoring.per_timeout * st.timeouts as f32);
    }
}

/// A wall 0.02 thick kills a ball that crosses it at 24 m/s, 0.1 m a frame: a body that is only tested where
/// it is each frame would skip it.
#[test]
fn a_fast_body_cannot_skip_a_thin_kill_wall() {
    let wall = volume([4.04, -1.0, -12.0], [4.06, 1.0, -8.0], true, 0.0, 0.0);
    let mut s = pen(vec![wall], 100.0, 600.0);
    s.world.as_mut().unwrap().kinds[0].radius = 0.01;
    let mut sim = Sim::new(s, 1);
    for k in 1..=(0.5 / FRAME) as u32 {
        sim.targets[0].pos = Vec3::new(24.0 * k as f32 * FRAME, 0.0, -10.0);
        let x = sim.targets[0].pos.x;
        assert!(!(4.03..=4.07).contains(&x), "the frames must straddle the wall, not land in it: {x}");
        sim.step(FRAME, idle());
        if sim.stats().timeouts > 0 {
            assert!(x >= 4.04, "went at {x}");
            return;
        }
    }
    panic!("the ball crossed the wall alive");
}

/// A world with one kind of bot has no slots, but each bot up at once still keeps its own cooldown: with
/// two of them inside a volume that kills at full health, both must go on the first frame.
#[test]
fn bots_of_a_world_without_slots_have_cooldowns_of_their_own() {
    let mut s = pen(vec![around(false, 100.0, 5.0)], 100.0, 600.0);
    let w = s.world.as_mut().unwrap();
    (w.kinds, w.slots) = (Vec::new(), Vec::new());
    s.targets.count = 2;
    s.targets.health = 100.0;
    s.targets.radius = 0.4;
    s.targets.height = 0.8;
    let mut sim = Sim::new(s, 1);
    sim.step(FRAME, idle());
    assert_eq!(sim.stats().timeouts, 2, "{:?}", sim.stats());
}

/// A body's first pass tests where it is, not the way from where the last body of its slot died.
#[test]
fn a_new_bodys_first_pass_does_not_sweep_from_elsewhere() {
    // Nor from the middle of the world: a wall between it and the ball's place leaves a new ball alone.
    let between = volume([-1.0, -1.0, -6.0], [1.0, 1.0, -5.0], true, 0.0, 0.0);
    let mut sim = Sim::new(pen(vec![between], 100.0, 600.0), 1);
    sim.step(FRAME, idle());
    assert_eq!(sim.stats().timeouts, 0, "a new ball is tested where it is");
    let wall = volume([5.0, -1.0, -12.0], [6.0, 1.0, -8.0], true, 0.0, 0.0);
    let mut sim = Sim::new(pen(vec![wall], 100.0, 600.0), 1);
    sim.step(FRAME, idle());
    assert_eq!(sim.stats().timeouts, 0);
    sim.targets[0].volume_from = None;
    sim.targets[0].pos = Vec3::new(10.0, 0.0, -10.0);
    sim.step(FRAME, idle());
    assert_eq!(sim.stats().timeouts, 0, "it appeared past the wall, it didn't cross it");
}

/// A bot that can't die takes no damage from a volume and no hit removes it, on its first frame or later. (The
/// importer refuses such files, so this is the sim's own rule and was not measured.)
#[test]
fn a_volume_does_not_hurt_a_bot_that_cannot_die() {
    for cooldown in [0.0, 1.0] {
        let mut s = pen(vec![around(false, 100.0, cooldown)], 100.0, 600.0);
        s.world.as_mut().unwrap().kinds[0].invincible = true;
        let mut sim = Sim::new(s, 1);
        for _ in 0..(3.0 / FRAME) as u32 {
            sim.step(FRAME, idle());
        }
        assert_eq!(sim.stats().timeouts, 0, "cooldown {cooldown}");
        assert_eq!(sim.targets()[0].health, 100.0, "cooldown {cooldown}");
    }
}

/// Replacing the opening targets forgets the cooldowns: the new ball is on its first frame with nothing
/// stamped, so a hit that would kill it at full health removes it.
#[test]
fn replacing_the_opening_targets_forgets_the_cooldowns() {
    let mut sim = Sim::new(pen(vec![around(false, 100.0, 5.0)], 100.0, 600.0), 1);
    sim.step(FRAME, idle());
    assert_eq!(sim.stats().timeouts, 1);
    sim.respawn_all();
    sim.step(FRAME, idle());
    assert_eq!(sim.stats().timeouts, 2, "the stamp of the ball before must not protect the new one");
}

/// A tall body reaches as far up and down as half its height, and across only as far as its radius: a box
/// 2 m tall and 0.4 m wide meets a volume 0.6 m over or under its middle, and not one 0.6 m to a side.
#[test]
fn a_tall_body_reaches_further_up_than_across() {
    let cases = [
        ("above", volume([-1.0, 0.6, -12.0], [1.0, 1.6, -8.0], true, 0.0, 0.0), true),
        ("below", volume([-1.0, -1.6, -12.0], [1.0, -0.6, -8.0], true, 0.0, 0.0), true),
        ("beside", volume([0.6, -3.0, -12.0], [1.6, 3.0, -8.0], true, 0.0, 0.0), false),
        ("in front", volume([-1.0, -3.0, -9.4], [1.0, 3.0, -8.4], true, 0.0, 0.0), false),
    ];
    for (side, v, dies) in cases {
        let mut s = pen(vec![v], 100.0, 600.0);
        let kind = &mut s.world.as_mut().unwrap().kinds[0];
        (kind.shape, kind.radius, kind.height) = (crate::scenario::Shape::Box, 0.2, 2.0);
        let mut sim = Sim::new(s, 1);
        let at = sim.targets()[0].pos;
        assert!((at - Vec3::new(0.0, 0.0, -10.0)).length() < 1e-4, "the box stands at the pen's point: {at}");
        sim.step(FRAME, idle());
        assert_eq!(sim.stats().timeouts > 0, dies, "{side}");
    }
}

/// Shots pass through volumes: the target in front of, behind or inside one is hit and killed.
#[test]
fn a_shot_passes_through_a_volume() {
    for v in [
        volume([-1.0, -1.0, -7.0], [1.0, 1.0, -5.0], false, 0.0, 99.0),
        volume([-1.0, -1.0, -14.0], [1.0, 1.0, -12.0], false, 0.0, 99.0),
        volume([-1.0, -1.0, -12.0], [1.0, 1.0, -8.0], false, 0.0, 99.0),
    ] {
        let mut s = pen(vec![v], 1.0, 600.0);
        s.weapon.damage = 100.0;
        let mut sim = Sim::new(s, 1);
        sim.step(FRAME, Input { aim: Vec3::NEG_Z, fire_pressed: true, fire_held: true });
        assert_eq!(sim.stats().kills, 1, "{:?}", sim.stats());
    }
}

#[test]
fn volume_problems_are_found() {
    let ok = pen(vec![around(false, 10.0, 1.0)], 100.0, 1.0);
    assert!(ok.world.as_ref().unwrap().problems().is_empty(), "{:?}", ok.world.as_ref().unwrap().problems());
    let problems = |edit: &dyn Fn(&mut World)| {
        let mut w = ok.world.clone().unwrap();
        edit(&mut w);
        w.problems()
    };
    assert!(problems(&|w| w.volumes[0].damage = -1.0).iter().any(|p| p.contains("damage")));
    assert!(problems(&|w| w.volumes[0].cooldown_s = -1.0).iter().any(|p| p.contains("cooldown")));
    assert!(problems(&|w| w.volumes[0].damage = f32::NAN).iter().any(|p| p.contains("isn't a number")));
    assert!(problems(&|w| w.volumes[0].corner[1] = f32::INFINITY).iter().any(|p| p.contains("isn't a number")));
    assert!(problems(&|w| w.volumes[0].edges[2] = [0.0; 3]).iter().any(|p| p.contains("no length")));
    // Two vertical edges, and an edge that leans.
    assert!(problems(&|w| w.volumes[0].edges[0] = [0.0, 1.0, 0.0]).iter().any(|p| p.contains("vertical edge")));
    assert!(problems(&|w| w.volumes[0].edges[0] = [1.0, 1.0, 0.0]).iter().any(|p| p.contains("vertical edge")));
    // A box turned about the vertical is fine.
    assert!(problems(&|w| w.volumes[0].edges = [[0.7, 0.0, 0.7], [0.0, 2.0, 0.0], [-0.7, 0.0, 0.7]]).is_empty());
}

/// Stored scenarios that have no volumes don't gain a key for them, so their fingerprints stay put.
#[test]
fn a_world_without_volumes_serializes_without_the_key() {
    let plain = toml::to_string(&World::default()).unwrap();
    assert!(!plain.contains("volumes"), "{plain}");
    let with = World { volumes: vec![around(true, 0.0, 0.0)], ..Default::default() };
    assert!(toml::to_string(&with).unwrap().contains("volumes"));
}
