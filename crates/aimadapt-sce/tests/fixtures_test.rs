//! Reads whole `.sce` files from disk. The fixtures are made up for these tests (an
//! original scenario in each of KovaaK's two map formats), never copies of anyone's
//! scenario. They keep the quirks real files have: CRLF line ends, a byte-order mark,
//! `//` comment lines, a stray line without `=`, blank lines inside a section,
//! extensions on references, and unused map objects.

use std::path::{Path, PathBuf};

use aimadapt_sce::map::{MapFormat, Solid, Teams};
use aimadapt_sce::{Sce, find_files, gaps, import};
use aimadapt_sim::scenario::{Shape, Trigger};
use aimadapt_sim::world::AccuracyMult;
use aimadapt_sim::{Input, Phase, Sim, Vec3};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

fn read(name: &str) -> Sce {
    Sce::read(&fixtures().join(format!("AimAdapt Test - {name}.sce"))).unwrap()
}

#[test]
fn finds_every_fixture() {
    let files = find_files(&[fixtures()]);
    assert_eq!(files.len(), 6);
    assert!(files.iter().all(|f| f.extension().unwrap() == "sce"));
}

#[test]
fn json_map_scenario_reads_resolves_and_is_covered() {
    let sce = read("Drifting Pair");
    // The byte-order mark doesn't end up in the first key.
    assert_eq!(sce.name(), "AimAdapt Test - Drifting Pair");
    assert_eq!(sce.settings.top.f32("Timelimit"), Some(60.0));
    assert_eq!(sce.settings.top.str("Description"), "Made up for AimAdapt's tests.[nl]Two drifting balls.");

    let r = sce.resolve();
    assert!(r.missing.is_empty(), "{:?}", r.missing);
    assert_eq!(r.player.unwrap().name(), "Shooter");
    let weapon = r.weapon.unwrap();
    assert_eq!(weapon.name(), "Steady Beam");
    // A blank line and a stray line inside the section don't end it.
    assert_eq!(weapon.bool("IsBurstWeapon"), Some(false));
    assert_eq!(r.bots.len(), 2);
    for slot in &r.bots {
        assert_eq!(slot.reference, "Pair Rot.rot");
        assert_eq!(slot.team, Some("2"));
        let picks: Vec<(&str, f32)> = slot.choices.iter().map(|c| (c.bot.name(), c.weight)).collect();
        assert_eq!(picks, vec![("Drifter", 3.0), ("Hopper", 1.0)]);
    }
    let drifter = &r.bots[0].choices[0];
    assert_eq!(drifter.character.unwrap().f32("MaxSpeed"), Some(600.0));
    assert_eq!(
        drifter.dodges.iter().map(|(d, w)| (d.name(), *w)).collect::<Vec<_>>(),
        vec![("Wander", 1.0), ("Dart", 0.5)]
    );
    assert_eq!(drifter.abilities[0].name(), "Nudge");
    assert_eq!(r.bots[0].choices[1].abilities[0].kind, "Movement Ability Profile");

    let map = sce.map.as_ref().unwrap();
    assert_eq!(map.format, MapFormat::Json);
    assert_eq!(map.solids.len(), 3);
    assert!(matches!(&map.solids[1], Solid::Primitive { mesh, .. } if mesh == "Cylinder"));
    let Solid::Mesh { triangles, .. } = &map.solids[2] else { panic!("procedural brush") };
    assert_eq!(triangles, &vec![[0, 1, 2], [2, 3, 0]]);
    assert_eq!(map.spawns.len(), 3);
    assert_eq!(map.spawns[0].teams, Teams::Mask(1));
    assert_eq!(map.spawns[1].characters, vec!["Drifter Ball".to_string()]);
    assert!(map.spawns[2].volume.is_some());
    assert!(map.other.is_empty());

    assert_eq!(gaps(&sce), vec![]);
}

#[test]
fn reflex_map_scenario_reads_and_lists_its_gaps() {
    let sce = read("Old Room");
    assert_eq!(sce.settings.top.f32("Timescale"), Some(0.9));
    let r = sce.resolve();
    assert!(r.missing.is_empty(), "{:?}", r.missing);
    assert_eq!(r.bots.len(), 2);
    assert_eq!(r.bots[0].choices[0].character.unwrap().str("MainBBType"), "Cuboid");

    let map = sce.map.as_ref().unwrap();
    assert_eq!(map.format, MapFormat::Reflex);
    assert_eq!(map.version, "8");
    let Solid::Polyhedron { vertices, faces, .. } = &map.solids[0] else { panic!("brush") };
    assert_eq!((vertices.len(), faces.len()), (8, 6));
    assert_eq!(map.spawns.len(), 3);
    assert!(map.spawns[0].initial);
    assert_eq!(map.spawns[0].teams, Teams::Flags { a: true, b: false });
    assert_eq!(map.spawns[1].teams, Teams::Flags { a: false, b: true });
    // The camera's look-at point is cosmetic; the teleporter isn't modelled.
    assert_eq!(map.other, vec!["Teleporter".to_string()]);

    let features: Vec<&str> = gaps(&sce).iter().map(|g| g.feature).collect();
    assert_eq!(features, vec!["map objects", "projectile weapon", "heads"]);
}

fn path(name: &str) -> PathBuf {
    fixtures().join(format!("AimAdapt Test - {name}.sce"))
}

#[test]
fn still_bots_on_a_json_map_import_with_kovaaks_rules() {
    let i = import(&path("Still Wall")).unwrap();
    let s = &i.scenario;
    assert_eq!(s.name, "AimAdapt Test - Still Wall");
    assert_eq!(s.timing.duration_s, 30.0);
    assert_eq!((s.targets.count, s.targets.shape, s.targets.health), (3, Shape::Sphere, 1.0));
    assert!((s.targets.radius - 0.3).abs() < 1e-6);
    assert!((s.targets.respawn_delay_s - 0.2).abs() < 1e-6);
    assert_eq!(s.targets.color, "#ff8000");
    assert_eq!((s.weapon.trigger, s.weapon.fire_rate), (Trigger::Click, 5.0));
    assert_eq!(s.scoring.per_kill, 10.0);
    let w = s.world.as_ref().unwrap();
    assert_eq!(w.source, "AimAdapt Test - Still Wall.sce");
    assert_eq!(w.source_hash.len(), 16);
    assert_eq!(w.accuracy_mult, AccuracyMult::Sqrt);
    assert_eq!(w.fov_range, Some([100.0, 120.0]));
    assert!((w.blocked_radius - 1.5).abs() < 1e-6);
    // Unreal (250, -100, 0) × MapScale 4 is 10 m ahead and 4 m left, and a 0.6 m
    // ball's middle starts its height above the spawn point, as in KovaaK's.
    let spawns: Vec<Vec3> = w.spawns.iter().map(|p| Vec3::from_array(p.pos)).collect();
    assert!(spawns.iter().any(|p| (*p - Vec3::new(-4.0, 0.6, -10.0)).length() < 1e-4), "{spawns:?}");
    // The wall is drawn: one cube, 12 triangles.
    assert_eq!(w.meshes.len(), 1);
    assert_eq!(w.meshes[0].triangles.len(), 12);
    assert!(i.notes.is_empty(), "{:?}", i.notes);
    assert!(s.problems().is_empty());
}

#[test]
fn an_imported_scenario_plays_in_the_sim() {
    let s = import(&path("Still Wall")).unwrap().scenario;
    let mut sim = Sim::new(s, 11);
    let idle = Input { aim: Vec3::NEG_Z, fire_pressed: false, fire_held: false };
    while sim.phase() == Phase::Countdown {
        sim.step(0.05, idle);
    }
    // Three bots, each near a spawn point on the wall (0.6 m up, ± 0.5 m) and apart
    // from the others.
    let pos: Vec<Vec3> = sim.targets().iter().map(|t| t.pos).collect();
    assert_eq!(pos.len(), 3);
    for p in &pos {
        assert!((p.z + 10.0).abs() < 1e-4 && (p.y - 0.6).abs() <= 0.5 + 1e-4, "{p}");
    }
    // A hit kills (1 damage, 1 health); then a miss. Sqrt accuracy: 10 × √0.5.
    let aim = sim.targets()[0].pos.normalize();
    sim.step(0.01, Input { aim, fire_pressed: true, fire_held: true });
    assert_eq!(sim.stats().kills, 1);
    sim.step(0.3, idle);
    sim.step(0.01, Input { aim: Vec3::Y, fire_pressed: true, fire_held: true });
    assert_eq!(sim.stats().misses, 1);
    assert!((sim.score() - 10.0 * 0.5f32.sqrt()).abs() < 1e-4, "{}", sim.score());
}

/// A strafing bot with a movement ability: it imports, says so in a note, and in play
/// it moves faster than its top speed at some point (it dashed), and stays in its lane.
#[test]
fn a_bot_with_a_dash_imports_and_dashes_in_play() {
    let i = import(&path("Dash Lane")).unwrap();
    assert!(i.notes.iter().any(|n| n.contains("movement abilities")), "{:?}", i.notes);
    let w = i.scenario.world.as_ref().unwrap();
    let motion = w.motion.as_ref().unwrap();
    assert_eq!(motion.max_speed, 6.0);
    let ab = motion.abilities.as_ref().unwrap();
    assert_eq!((ab.list.len(), ab.list[0].speed, ab.list[0].along_input), (1, 24.0, true));
    assert_eq!((ab.press_every_s, ab.chance, ab.window_s), (0.2, 0.8, [1.0, 2.0]));
    let mut sim = Sim::new(i.scenario, 11);
    let idle = Input { aim: Vec3::Y, fire_pressed: false, fire_held: false };
    while sim.phase() == Phase::Countdown {
        sim.step(0.05, idle);
    }
    let dt = 1.0 / 120.0;
    let (mut last, mut fastest) = (sim.targets()[0].pos, 0.0f32);
    for _ in 0..(8.0 / dt) as usize {
        sim.step(dt, idle);
        let p = sim.targets()[0].pos;
        fastest = fastest.max(Vec3::new(p.x - last.x, 0.0, p.z - last.z).length() / dt);
        assert!(p.x.abs() < 8.0 && p.y > -2.5, "stays in its lane: {p}");
        last = p;
    }
    assert!(fastest > 6.5, "it dashed past its top speed of 6 m/s: {fastest}");
}

#[test]
fn scenarios_the_trainer_cant_play_yet_say_why() {
    // The drifting pair move and dash, which plays, but one of them also pushes with
    // a melee ability (a knockback). Only helper bots on the player's team push.
    let moving = import(&path("Drifting Pair")).unwrap_err();
    let text: Vec<String> = moving.iter().map(ToString::to_string).collect();
    assert_eq!(text, vec!["bots that use abilities (a target's melee ability: Nudge)".to_string()]);
    assert!(!moving[0].beyond_plan, "abilities are in the plan");
    // Reflex maps play; this one's projectile weapon doesn't, and isn't planned.
    let old = import(&path("Old Room")).unwrap_err();
    assert!(!old.iter().any(|r| r.to_string().contains("Reflex")), "{old:?}");
    assert!(old.iter().any(|r| r.feature == "projectile weapon" && r.beyond_plan), "{old:?}");
}

fn idle() -> Input {
    Input { aim: Vec3::Y, fire_pressed: false, fire_held: false }
}

/// A sim started on the run (past the countdown).
fn started(s: aimadapt_sim::Scenario) -> Sim {
    let mut sim = Sim::new(s, 7);
    while sim.phase() == Phase::Countdown {
        sim.step(0.05, idle());
    }
    sim
}

fn near(a: Vec3, b: [f32; 3]) -> bool {
    (a - Vec3::from_array(b)).length() < 1e-3
}

/// A chasing helper sits under an invisible floor, facing up, and lifts a ball that
/// falls toward the floor: the ball never reaches it. Without the helper it lands.
/// The ball's character is invincible, which the scenario doesn't say.
#[test]
fn a_chasing_helper_under_a_clip_floor_keeps_a_ball_up() {
    let i = import(&path("Floor Keeper")).unwrap();
    assert!(i.notes.iter().any(|n| n.contains("helper bots push the targets around (1 of them)")), "{:?}", i.notes);
    let s = i.scenario;
    let w = s.world.as_ref().unwrap();
    // The helper is not a target, and its spawn point is not the ball's.
    assert_eq!(s.targets.count, 1);
    assert_eq!((w.spawns.len(), w.kinds.len(), w.slots.len()), (1, 0, 0));
    assert_eq!(w.pushers.len(), 1);
    let p = &w.pushers[0];
    assert!(p.chases && p.needs_sight);
    // 5 m ahead, on the underside of a 0.5 m slab below the player's feet (its height
    // above that is its own: 2 units).
    assert!(near(Vec3::from_array(p.pos), [0.0, -0.48, -5.0]), "{:?}", p.pos);
    assert!(near(Vec3::from_array(p.facing), [0.0, 1.0, 0.0]), "{:?}", p.facing);
    assert_eq!((p.radius, p.every_s, p.push, p.lift), (0.6, 0.02, 3.0, 0.6));
    assert!(w.invincible, "the ball's character is invincible");
    let motion = w.motion.as_ref().expect("a ball needs a motion to be lifted");
    assert!(motion.dodges.is_empty() && motion.abilities.is_none() && motion.max_speed == 0.0);

    let play = |s: aimadapt_sim::Scenario| {
        let mut sim = started(s);
        let mut lowest = f32::MAX;
        for _ in 0..(10.0 * 120.0) as usize {
            sim.step(1.0 / 120.0, idle());
            lowest = lowest.min(sim.targets()[0].pos.y);
        }
        lowest
    };
    // The ball's middle is 0.3 m above the floor when it rests on it.
    let lowest = play(s.clone());
    assert!(lowest > 0.5, "the helper holds it up: lowest {lowest}");
    let mut without = s.clone();
    without.world.as_mut().unwrap().pushers.clear();
    let lowest = play(without);
    assert!((lowest - 0.3).abs() < 0.02, "without it the ball lands: lowest {lowest}");
    // It takes hits and never dies, though one shot would kill it.
    let mut strong = s;
    strong.weapon.damage = 100.0;
    let mut sim = started(strong);
    for _ in 0..40 {
        let aim = sim.targets()[0].pos.normalize();
        sim.step(0.05, Input { aim, fire_pressed: true, fire_held: true });
    }
    assert!(sim.stats().hits > 0 && sim.stats().kills == 0, "{:?}", sim.stats());
}

/// A pinned helper beside two balls kicks the one it can see, sideways and away from
/// itself, and leaves the one behind a solid ledge alone. Without needing sight it
/// kicks both.
#[test]
fn a_pinned_helper_that_needs_sight_leaves_a_ball_behind_a_ledge_alone() {
    let i = import(&path("Ledge Kicker")).unwrap();
    let s = i.scenario;
    let w = s.world.as_ref().unwrap();
    assert_eq!((s.targets.count, w.spawns.len(), w.pushers.len()), (2, 2, 1));
    let p = &w.pushers[0];
    assert!(!p.chases && p.needs_sight);
    // 10 m ahead, facing away from the player; a reach of 1.5 m, 4 m/s every 0.1 s.
    assert!(near(Vec3::from_array(p.pos), [0.0, 0.02, -10.0]), "{:?}", p.pos);
    assert!(near(Vec3::from_array(p.facing), [0.0, 0.0, -1.0]), "{:?}", p.facing);
    assert_eq!((p.radius, p.every_s, p.push, p.lift), (1.5, 0.1, 4.0, 0.0));

    // The ball on the left is in plain view, the one on the right behind the ledge.
    let moved = |s: aimadapt_sim::Scenario| {
        let mut sim = started(s);
        let at = |sim: &Sim| {
            let mut t: Vec<Vec3> = sim.targets().iter().map(|t| t.pos).collect();
            t.sort_by(|a, b| a.x.total_cmp(&b.x));
            t
        };
        let start = at(&sim);
        for _ in 0..(3.0 * 120.0) as usize {
            sim.step(1.0 / 120.0, idle());
        }
        let end = at(&sim);
        let level = |a: Vec3, b: Vec3| Vec3::new(b.x - a.x, 0.0, b.z - a.z);
        (level(start[0], end[0]), level(start[1], end[1]))
    };
    let (left, right) = moved(s.clone());
    // Away from the helper: left and farther.
    assert!(left.x < -0.3 && left.z < -0.3, "kicked sideways and away: {left}");
    assert!(right.length() < 1e-3, "behind the ledge: {right}");
    let mut sees = s;
    sees.world.as_mut().unwrap().pushers[0].needs_sight = false;
    let (left, right) = moved(sees);
    assert!(left.x < -0.3 && right.x > 0.3 && right.z < -0.3, "{left} {right}");
}

/// Reads every `.sce` file in your KovaaK's folders. Run by hand with
/// `cargo test -p aimadapt-sce -- --ignored`; it only reads.
#[test]
#[ignore = "needs KovaaK's installed; run by hand"]
fn every_local_scenario_file_reads() {
    let files = find_files(&aimadapt_core::discover::find_scenario_dirs());
    assert!(!files.is_empty(), "no .sce files found");
    let mut problems = Vec::new();
    for f in &files {
        let sce = Sce::read(f).unwrap();
        if let Err(e) = &sce.map {
            problems.push(format!("{}: {e}", f.display()));
        }
        if sce.name().is_empty() {
            problems.push(format!("{}: no Name", f.display()));
        }
    }
    println!("{} files read", files.len());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// A scenario's fingerprint is what its saved ratings and best scores hang on, so a
/// new world field that isn't left out of the file at its default would restart them
/// all. These were taken before helper bots (plan K3) were added.
#[test]
fn imported_fixtures_keep_their_fingerprints() {
    assert_eq!(import(&path("Dash Lane")).unwrap().scenario.fingerprint(), "b4157023853bb7c5");
    assert_eq!(import(&path("Still Wall")).unwrap().scenario.fingerprint(), "dae40cef9b1e60b7");
}
