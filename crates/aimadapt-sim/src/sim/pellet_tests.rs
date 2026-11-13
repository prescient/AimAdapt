//! Tests for several pellets per shot (plan K11a). The numbers come from test maps played in KovaaK's 3.9.13 on
//! 12 November 2026, with its units divided by 100 (a ball of radius 40 is 0.4 m here).

use super::*;
use crate::scenario::{Head, Scenario, Scoring, Shape};
use crate::world::{World, WorldKind, WorldSpawn};

const FRAME: f32 = 1.0 / 240.0;

fn aimed(at: Vec3) -> Input {
    Input { aim: at.normalize(), fire_pressed: true, fire_held: true }
}

/// An imported scenario with one still ball of radius 0.4 and the given health at (0, 0, -10), a weapon of
/// `pellets` pellets of `damage` each, a score of 1 per damage and nothing else, and a respawn too late to matter.
fn yard(pellets: u32, damage: f32, health: f32) -> Scenario {
    let e = crate::library::builtins().into_iter().find(|e| e.id == "builtin:static-grid").unwrap();
    let mut s = e.scenario.unwrap();
    s.timing.countdown_s = 0.0;
    s.timing.duration_s = 600.0;
    s.targets.count = 1;
    s.scoring = Scoring { per_kill: 0.0, per_hit: 0.0, per_miss: 0.0, per_damage: 1.0, per_timeout: 0.0 };
    s.weapon.pellets = pellets;
    s.weapon.damage = damage;
    s.world = Some(World {
        spawns: vec![WorldSpawn { pos: [0.0, 0.0, -10.0], ..Default::default() }],
        kinds: vec![WorldKind { radius: 0.4, health, respawn_delay_s: 1000.0, ..Default::default() }],
        slots: vec![vec![(0, 1.0)]],
        ..Default::default()
    });
    s
}

/// One click at the first target.
fn click(sim: &mut Sim) {
    let at = sim.targets()[0].pos;
    sim.step(FRAME, aimed(at));
    sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
}

/// Ten pellets of 1 at a big ball: each click read +1 shot, +1 hit, and damage +10 done of +10 possible (KovaaK's
/// counts a click as one shot and, if it lands, one hit, whatever the pellet count).
#[test]
fn a_click_of_ten_pellets_is_one_shot_and_one_hit() {
    let mut sim = Sim::new(yard(10, 1.0, 1000.0), 1);
    for n in 1..=3u32 {
        click(&mut sim);
        let st = sim.stats();
        let want = 10.0 * n as f32;
        assert_eq!((st.shots, st.hits, st.misses), (n, n, 0), "after click {n}");
        assert_eq!((st.damage_done, st.damage_possible), (want, want), "after click {n}");
    }
    assert_eq!((sim.targets()[0].shots, sim.targets()[0].hits), (3, 3));
}

/// A click that misses adds pellets times damage to the possible damage and nothing to the damage done.
#[test]
fn a_missed_click_adds_the_whole_volley_to_possible_damage_only() {
    let mut sim = Sim::new(yard(10, 1.0, 1000.0), 1);
    sim.step(FRAME, aimed(Vec3::Y));
    let st = sim.stats();
    assert_eq!((st.shots, st.hits, st.misses), (1, 0, 1));
    assert_eq!((st.damage_done, st.damage_possible), (0.0, 10.0));
}

/// Ten pellets of 1 at a ball of health 3 in front of a big ball: all ten land on the small one (kill count 1,
/// damage 10 done of 10 possible) and the big ball behind was untouched and took no hit.
#[test]
fn all_the_pellets_land_on_the_first_body_and_none_carries_on() {
    let mut sim = Sim::new(yard(10, 1.0, 3.0), 1);
    let mut big = sim.targets()[0].clone();
    big.id += 1000;
    big.pos = Vec3::new(0.0, 0.0, -15.0);
    big.radius = 5.0;
    big.base_radius = 5.0;
    big.health = 100.0;
    big.max_health = 100.0;
    let big_id = big.id;
    sim.targets.push(big);
    click(&mut sim);
    let st = sim.stats();
    assert_eq!((st.kills, st.hits, st.shots), (1, 1, 1));
    assert_eq!((st.damage_done, st.damage_possible), (10.0, 10.0));
    let behind = sim.targets().iter().find(|t| t.id == big_id).unwrap();
    assert_eq!((behind.health, behind.hits, behind.damage), (100.0, 0, 0.0));
}

/// Pellet damage adds as fractions: four pellets of 2.5 at a ball of health 25 read 10/10 after click 1, 20/20
/// after click 2, and click 3 kills it with 30/30.
#[test]
fn pellet_damage_adds_as_fractions() {
    let mut sim = Sim::new(yard(4, 2.5, 25.0), 1);
    for (n, want) in [(1, 10.0), (2, 20.0), (3, 30.0)] {
        click(&mut sim);
        let st = sim.stats();
        assert_eq!((st.damage_done, st.damage_possible), (want, want), "after click {n}");
        assert_eq!(st.kills, u32::from(n == 3), "after click {n}");
    }
}

/// With a score of 1 per damage, 0 per kill and a ball of health 3, one pellet of 10 scored 10.0 and ten pellets
/// of 1 scored 10.0: the pay is on the shot's whole damage, not on the health it took.
#[test]
fn the_score_per_damage_is_paid_on_the_whole_volley() {
    for (pellets, damage) in [(1, 10.0), (10, 1.0)] {
        let mut sim = Sim::new(yard(pellets, damage, 3.0), 1);
        click(&mut sim);
        assert_eq!(sim.stats().kills, 1);
        assert!((sim.score() - 10.0).abs() < 1e-4, "{pellets} pellets of {damage}: {}", sim.score());
    }
}

/// Not measured in KovaaK's, so inferred from the rules above: a capsule bot with a head of the radius 0.3 on top
/// of it, a weapon of 5 pellets of 2 with a multiplier of 4. A click on the head does pellets times damage times
/// the multiplier (40), a click on the body 10, and the possible damage is 40 for each click either way.
#[test]
fn a_head_takes_the_volley_times_the_multiplier_and_a_body_the_plain_volley() {
    let play = |head: bool| {
        let mut s = yard(5, 2.0, 1000.0);
        s.weapon.headshot_multiplier = 4.0;
        s.world.as_mut().unwrap().kinds = vec![WorldKind {
            shape: Shape::Capsule,
            radius: 0.5,
            height: 2.0,
            health: 1000.0,
            respawn_delay_s: 1000.0,
            head: Some(Head { radius: 0.3, offset: 0.0, ..Head::default() }),
            ..Default::default()
        }];
        let mut sim = Sim::new(s, 1);
        let at = if head { sim.targets()[0].head_middle().unwrap() } else { sim.targets()[0].body_middle() };
        sim.step(FRAME, aimed(at));
        let st = sim.stats();
        (st.hits, st.damage_done, st.damage_possible)
    };
    assert_eq!(play(true), (1, 40.0, 40.0));
    assert_eq!(play(false), (1, 10.0, 40.0));
}

/// A bot that can't be hurt still counts the volley on every click: 5 pellets of 2 for three clicks is 30 done
/// of 30 possible, three hits, no kill and the health unchanged.
#[test]
fn a_bot_that_cant_be_hurt_counts_the_volley_a_click() {
    let mut s = yard(5, 2.0, 10.0);
    s.world.as_mut().unwrap().invincible = true;
    let mut sim = Sim::new(s, 1);
    for _ in 0..3 {
        click(&mut sim);
    }
    let st = sim.stats();
    assert_eq!((st.hits, st.kills, st.damage_done, st.damage_possible), (3, 0, 30.0, 30.0));
    assert_eq!(sim.targets()[0].health, 10.0);
}

/// A kill shot's whole damage counts as done, but the bot's reaction to damage (`AIDamageReaction`) is given the
/// health the shot took, as before: a shot of 25 that takes a last 15 doesn't reach a threshold of 20. Not
/// measured in KovaaK's, where the bot is gone either way. A trigger draws random numbers, and once the bot is
/// dead that is its only trace, so the test looks at where the next bot appears in a spawn volume: the same
/// place as with a threshold no shot reaches, and another place with a threshold of 10, which the 15 does reach.
#[test]
fn a_kill_shot_gives_the_bots_reaction_the_health_it_took() {
    use crate::motion::{Abilities, Ability, Motion, Reaction};
    let comes_back_at = |threshold: f32| {
        let mut s = yard(1, 25.0, 15.0);
        let reaction = Reaction { threshold, reset_s: 1.0, ignore_chance: 0.0, delay_s: [0.1, 0.3] };
        let abilities = Abilities {
            press_every_s: 0.1,
            chance: 0.0,
            window_s: [1.0, 1.0],
            list: vec![Ability { reaction: Some(reaction), ..Ability::default() }],
            ..Default::default()
        };
        let w = s.world.as_mut().unwrap();
        w.spawns[0].edges = [[6.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 1.0]];
        w.kinds[0].respawn_delay_s = 0.25;
        w.kinds[0].motion = Some(Motion { stays_put: true, abilities: Some(abilities), ..Default::default() });
        let mut sim = Sim::new(s, 7);
        click(&mut sim);
        assert_eq!((sim.stats().kills, sim.stats().damage_done), (1, 25.0));
        for _ in 0..240 {
            if !sim.targets().is_empty() {
                break;
            }
            sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
        }
        sim.targets()[0].pos
    };
    assert_eq!(comes_back_at(20.0), comes_back_at(1000.0));
    assert_ne!(comes_back_at(10.0), comes_back_at(1000.0));
}

/// The pellet count is left out of a scenario's file at 1, so every stored fingerprint keeps its value, and a
/// file with no such key reads back as 1. A weapon with pellets stores them, reads them back and has a
/// fingerprint of its own.
#[test]
fn pellets_are_left_out_of_the_file_at_one_and_round_trip_above_it() {
    let keys = |s: &Scenario| -> Vec<String> {
        let doc: toml::Value = toml::from_str(&s.to_toml()).unwrap();
        let mut k: Vec<String> = doc["weapon"].as_table().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    let plain = Scenario::default();
    assert_eq!(keys(&plain), ["damage", "damage_per_s", "fire_rate", "kind", "trigger"]);
    assert_eq!(Scenario::from_toml("name = \"x\"").unwrap().weapon.pellets, 1);
    assert_eq!(Scenario::from_toml("name = \"x\"\n[weapon]\ndamage = 5.0").unwrap().weapon.pellets, 1);
    let mut shotgun = Scenario::default();
    shotgun.weapon.pellets = 6;
    let text = shotgun.to_toml();
    assert!(text.contains("pellets = 6"), "{text}");
    assert_eq!(Scenario::from_toml(&text).unwrap(), shotgun);
    assert_ne!(shotgun.fingerprint(), plain.fingerprint());
}

/// Pellets from 1 to 1000 are sound, 0 and 1001 are problems, and a beam can't fire pellets.
#[test]
fn pellets_are_validated() {
    for (n, ok) in [(0, false), (1, true), (1000, true), (1001, false)] {
        let mut s = Scenario::default();
        s.weapon.pellets = n;
        assert_eq!(s.problems().is_empty(), ok, "{n} pellets: {:?}", s.problems());
    }
    let mut beam = Scenario::default();
    beam.weapon.kind = crate::scenario::WeaponKind::Beam;
    beam.weapon.pellets = 2;
    assert!(beam.problems().iter().any(|p| p.contains("pellets")), "{:?}", beam.problems());
    beam.weapon.pellets = 1;
    assert!(beam.problems().is_empty(), "{:?}", beam.problems());
}
