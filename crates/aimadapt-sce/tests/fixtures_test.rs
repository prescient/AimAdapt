//! Reads whole `.sce` files from disk. The fixtures are made up for these tests (an
//! original scenario in each of KovaaK's two map formats), never copies of anyone's
//! scenario. They keep the quirks real files have: CRLF line ends, a byte-order mark,
//! `//` comment lines, a stray line without `=`, blank lines inside a section,
//! extensions on references, and unused map objects.

use std::path::{Path, PathBuf};

use aimadapt_sce::map::{MapFormat, Solid, Teams};
use aimadapt_sce::{Sce, find_files, gaps};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

fn read(name: &str) -> Sce {
    Sce::read(&fixtures().join(format!("AimAdapt Test - {name}.sce"))).unwrap()
}

#[test]
fn finds_every_fixture() {
    let files = find_files(&[fixtures()]);
    assert_eq!(files.len(), 2);
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
    let Solid::Polyhedron { vertices, faces } = &map.solids[0] else { panic!("brush") };
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
