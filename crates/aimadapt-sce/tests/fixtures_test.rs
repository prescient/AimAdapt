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
    assert_eq!(files.len(), 3);
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

#[test]
fn scenarios_the_trainer_cant_play_yet_say_why() {
    // The drifting pair move, which plays, but they nudge and dash, which doesn't yet.
    let moving = import(&path("Drifting Pair")).unwrap_err();
    assert_eq!(moving, vec!["bots that use abilities (dashes and knockbacks)".to_string()]);
    // Reflex maps play; this one's projectile weapon doesn't.
    let old = import(&path("Old Room")).unwrap_err();
    assert!(!old.iter().any(|r| r.contains("Reflex")), "{old:?}");
    assert!(old.iter().any(|r| r.starts_with("projectile weapon")), "{old:?}");
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
