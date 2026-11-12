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
    assert_eq!(files.len(), 12);
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

/// Still box bots import as boxes (plan K5), start their height above their spawn points,
/// and a shot that passes a box's upper front corner hits it, where one at the same place on
/// a capsule of the same size would go by.
#[test]
fn box_bots_import_as_boxes_and_take_a_shot_at_their_corner() {
    let i = import(&path("Box Row")).unwrap();
    let s = i.scenario;
    assert_eq!((s.targets.count, s.targets.shape), (3, Shape::Box));
    assert!((s.targets.radius - 0.35).abs() < 1e-6 && (s.targets.height - 1.5).abs() < 1e-6);
    assert!(i.notes.is_empty(), "{:?}", i.notes);
    assert!(s.problems().is_empty());
    let w = s.world.as_ref().unwrap();
    // Unreal (800, 0, 0) is 8 m ahead, and a 1.5 m tall box's middle starts 1.5 m up.
    let spawns: Vec<Vec3> = w.spawns.iter().map(|p| Vec3::from_array(p.pos)).collect();
    assert!(spawns.iter().any(|p| (*p - Vec3::new(0.0, 1.5, -8.0)).length() < 1e-4), "{spawns:?}");

    // Aim 0.3 m right of a bot's middle and 0.7 m above it, at its front face's depth: inside
    // the box's top edge (half width 0.35, half height 0.75), outside a capsule's rounded top.
    let corner_shot = |s: aimadapt_sim::Scenario| {
        let mut sim = started(s);
        let p = sim.targets()[0].pos;
        let aim = (p + Vec3::new(0.3, 0.7, 0.35)).normalize();
        sim.step(0.01, Input { aim, fire_pressed: true, fire_held: true });
        sim.stats().kills
    };
    assert_eq!(corner_shot(s.clone()), 1, "the box takes the corner shot");
    let mut capsule = s;
    capsule.targets.shape = Shape::Capsule;
    assert_eq!(corner_shot(capsule), 0, "a capsule of the same size misses it");
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
    // The character's made-up flight: 30000 units/s² and 20000 units/s, in meters, and a drag of 7.
    let flight = p.flight.as_ref().expect("a chaser has a flight");
    assert_eq!((flight.accel, flight.max_speed, flight.drag), (300.0, 200.0, 7.0));
    assert_eq!(p.start_s, 0.0);
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

/// The Floor Keeper file with its helper's spawn point moved 12 m along the slab, away
/// from the spot under the ball, and its chaser given a slow acceleration.
fn far_chaser() -> aimadapt_sim::Scenario {
    let text = std::fs::read_to_string(path("Floor Keeper")).unwrap();
    // Drop the fixture's own flight settings, if it has them, and set slow ones.
    let mut text: String = text
        .lines()
        .filter(|l| !l.starts_with("Acceleration=") && !l.starts_with("DragCoefficient="))
        .map(|l| format!("{l}\n"))
        .collect();
    for (old, new) in [
        ("MaxSpeed=20000.0\nIsFlyer=true", "MaxSpeed=20000.0\nAcceleration=500.0\nDragCoefficient=1.0\nIsFlyer=true"),
        ("\"500, 0, -50\"", "\"500, 1200, -50\""),
    ] {
        assert!(text.contains(old), "{old}");
        text = text.replacen(old, new, 1);
    }
    aimadapt_sce::convert::convert(&Sce::parse(&text), "far.sce", "0").unwrap().scenario
}

/// Bug: K3 played a chasing helper as arriving at once, always at the point of its
/// slab nearest the ball, whatever its settings and wherever it spawned. A chaser
/// has an acceleration and takes time to cross to a ball that appears somewhere
/// else, so one that spawns 12 m along the slab and accelerates at 5 m/s² (it needs
/// over 2 s) can't hold the ball up in the first 1.5 s: the ball lands. Before the
/// fix it was held up from the first kick.
#[test]
fn a_chaser_far_from_its_ball_takes_time_to_arrive() {
    let mut sim = started(far_chaser());
    let mut lowest = f32::MAX;
    for _ in 0..(1.5 * 120.0) as usize {
        sim.step(1.0 / 120.0, idle());
        lowest = lowest.min(sim.targets()[0].pos.y);
    }
    assert!((lowest - 0.3).abs() < 0.02, "the ball lands before the chaser arrives: lowest {lowest}");
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
/// all. These were taken before helper bots (plan K3) were added; Dash Lane's bot dashes, so
/// its pin was taken again when abilities were timed as measured (it had been b4157023853bb7c5;
/// the file sets its start block, so only the new rule mark changed it). A fingerprint also
/// covers a hash of the file's bytes, which is why git must not touch the fixtures
/// (see `git_keeps_the_fixtures_bytes`).
#[test]
fn imported_fixtures_keep_their_fingerprints() {
    assert_eq!(import(&path("Dash Lane")).unwrap().scenario.fingerprint(), "7de71ff815020d9e");
    assert_eq!(import(&path("Still Wall")).unwrap().scenario.fingerprint(), "dae40cef9b1e60b7");
}

/// Regression test: CI failed on every push from plan K3 on. Git on Windows checked the
/// fixtures out with CRLF line ends, so their bytes, and with them the fingerprints
/// pinned above, differed from the copies the fingerprints were taken from.
/// `.gitattributes` now keeps the fixtures byte for byte, and each has the line ends it
/// was written with: the two oldest CRLF, as KovaaK's writes them, the rest LF.
#[test]
fn git_keeps_the_fixtures_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let attributes = std::fs::read_to_string(root.join(".gitattributes")).unwrap();
    let rule = "crates/aimadapt-sce/tests/fixtures/*.sce -text";
    assert!(attributes.lines().any(|l| l.trim() == rule), ".gitattributes needs the line: {rule}");
    let crlf = |name: &str| std::fs::read(path(name)).unwrap().windows(2).any(|w| w == b"\r\n");
    for name in ["Drifting Pair", "Old Room"] {
        assert!(crlf(name), "{name} lost its CRLF line ends");
    }
    for name in [
        "Dash Lane",
        "Still Wall",
        "Floor Keeper",
        "Ledge Kicker",
        "Last Lives",
        "Box Row",
        "Stair Court",
        "Jump Court",
        "Costly Hop",
        "Quiet Guns",
    ] {
        assert!(!crlf(name), "{name} was given CRLF line ends");
    }
}

/// One slot that plays three balls in turn, `BotMaxLives=3` and `ScorePerTime=1` with no
/// points for kills: the run ends on the third kill, with each ball killed once in
/// rotation order, and the score is the game seconds left.
#[test]
fn a_slot_with_lives_ends_on_its_last_kill_and_pays_for_the_time_left() {
    let i = import(&path("Last Lives")).unwrap();
    let s = &i.scenario;
    assert_eq!(s.timing.kill_limit, 3);
    assert_eq!(s.timing.duration_s, 100.0);
    assert_eq!(s.scoring.per_kill, 0.0);
    let w = s.world.as_ref().unwrap();
    assert_eq!(w.time_score, 1.0);
    assert_eq!((w.kinds.len(), w.slots.len(), w.rotations.clone()), (3, 1, vec![Some(0)]));
    assert!(i.notes.is_empty(), "{:?}", i.notes);
    assert!(s.problems().is_empty());

    let mut sim = Sim::new(i.scenario, 3);
    let mut killed = Vec::new();
    let dt = 1.0 / 120.0;
    for step in 0..(100.0 / dt) as usize {
        if sim.phase() == Phase::Finished {
            break;
        }
        // Aim at the ball when there is one and click every 0.1 s.
        let input = match sim.targets().first() {
            Some(t) if step % 12 == 0 => Input { aim: t.pos.normalize(), fire_pressed: true, fire_held: true },
            Some(t) => Input { aim: t.pos.normalize(), fire_pressed: false, fire_held: false },
            None => Input { aim: Vec3::Y, fire_pressed: false, fire_held: false },
        };
        let kind = sim.targets().first().and_then(|t| t.kind());
        let before = sim.stats().kills;
        sim.step(dt, input);
        if sim.stats().kills > before {
            killed.extend(kind);
        }
    }
    assert_eq!(sim.phase(), Phase::Finished);
    assert_eq!(sim.stats().kills, 3);
    assert_eq!(killed, vec![0, 1, 2], "each ball once, in the order the rotation lists them");
    assert!(sim.elapsed() < 10.0, "it ended on the kills, not the clock: {}", sim.elapsed());
    assert!((sim.score() - (100.0 - sim.elapsed())).abs() < 1e-3, "{} at {} s", sim.score(), sim.elapsed());
    assert!(sim.score() > 90.0);
}

/// Plan K6a on one made-up file: a small and a large ball, each with its own start height and
/// spawn offsets, drop from above a floor and come to rest on it (they don't move by themselves,
/// but their character has gravity), and a walker whose feet appear 0.5 m above the floor, within
/// its step-up height, is on the floor in its first frame.
#[test]
fn kinds_start_at_their_own_height_and_bots_fall_and_step_onto_the_floor() {
    let i = import(&path("Stair Court")).unwrap();
    let s = i.scenario;
    assert!(s.problems().is_empty());
    assert_eq!(i.notes.len(), 1, "{:?}", i.notes);
    let w = s.world.as_ref().unwrap();
    assert_eq!((s.targets.count, w.kinds.len()), (3, 3));
    let (pebble, boulder, strider) = (&w.kinds[0], &w.kinds[1], &w.kinds[2]);
    // The first kind's start height is the spawn point's; Boulder (R 60, H 120) starts 0.8 m
    // above a pebble (R 20, H 40), and a 1.7 m walker 1.3 m above it.
    assert_eq!((pebble.rise, pebble.offsets), (0.0, None));
    assert!((boulder.rise - 0.8).abs() < 1e-4 && (strider.rise - 1.3).abs() < 1e-4);
    // The world's offsets are the pebble's (1 m wide, to the side); the others say their own.
    assert_eq!((w.offset_min, w.offset_max), ([-0.5, 0.0, 0.0], [0.5, 0.0, 0.0]));
    assert_eq!(boulder.offsets, Some([[-3.0, 0.0, 0.0], [3.0, 0.5, -1.0]]));
    assert_eq!(strider.offsets, Some([[0.0; 3]; 2]));
    // Both balls fall with no speed of their own; the walker has a step-up height.
    for ball in [pebble, boulder] {
        let m = ball.motion.as_ref().expect("a still ball with gravity falls");
        assert_eq!((m.max_speed, m.gravity, m.step_up), (0.0, 8.0, 0.0));
    }
    let walker = strider.motion.as_ref().unwrap();
    assert_eq!((walker.max_speed, walker.step_up), (3.0, 0.75));
    assert_eq!((pebble.spawns.clone(), boulder.spawns.clone(), strider.spawns.clone()), (vec![0], vec![2], vec![1]));

    // The floor's top, 1 m below the player's eye.
    let floor = w.meshes[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!((floor + 1.0).abs() < 1e-4, "{floor}");
    let mut sim = Sim::new(s, 5);
    let of = |sim: &Sim, kind: u32| sim.targets().iter().find(|t| t.kind() == Some(kind)).unwrap().clone();
    // At the start: the balls are in the air (spawn point 3 m above the floor, plus their rise,
    // and the pebble's 0.2 m radius), the walker already on the floor.
    let (a, b, c) = (of(&sim, 0), of(&sim, 1), of(&sim, 2));
    assert!((a.pos.y - (floor + 3.0 + 0.4)).abs() < 1e-3, "{}", a.pos.y);
    // The boulder's own offset lifts it by up to 0.5 m more, never lowers it.
    let lifted = b.pos.y - (floor + 3.0 + 1.2);
    assert!((-1e-3..=0.5 + 1e-3).contains(&lifted), "{}", b.pos.y);
    assert!((c.pos.y - (floor + 0.85)).abs() < 0.03, "{}", c.pos.y);
    // The pebble's offset runs along the spawn point's own axes, up to 0.5 m each way.
    assert!(a.pos.x.abs() <= 0.5 + 1e-4 && (a.pos.z + 8.0).abs() < 1e-4, "{}", a.pos);
    // After the countdown they fall and come to rest on the floor, a radius above it.
    while sim.phase() == Phase::Countdown {
        sim.step(0.05, idle());
    }
    for _ in 0..(4.0 * 120.0) as usize {
        sim.step(1.0 / 120.0, idle());
    }
    for (kind, radius) in [(0, 0.2), (1, 0.6), (2, 0.85)] {
        let t = of(&sim, kind);
        assert!((t.pos.y - (floor + radius)).abs() < 0.03, "kind {kind}: {}", t.pos.y);
    }
}

/// Plan K15a on one made-up file: a ball with 90 health hops straight up and pays 30 health for each
/// hop. It imports with the cost, and with no shots fired its balls leave by themselves, on their third
/// hop: timeouts, no kills and no score. A shot of 30 at a ball after its first hop kills it for good.
#[test]
fn a_ball_that_pays_health_to_hop_leaves_by_itself() {
    let i = import(&path("Costly Hop")).unwrap();
    let s = i.scenario;
    assert!(s.problems().is_empty());
    assert!(i.notes.iter().all(|n| !n.contains("pays health")), "{:?}", i.notes);
    let w = s.world.as_ref().unwrap();
    assert_eq!(s.targets.health, 90.0);
    assert!((s.targets.respawn_delay_s - 1.0).abs() < 1e-6);
    let ab = w.motion.as_ref().unwrap().abilities.as_ref().unwrap();
    assert_eq!((ab.list.len(), ab.list[0].health_cost, ab.list[0].up_speed, ab.list[0].speed), (1, 30.0, 4.0, 0.0));
    // Imported scenarios give nothing for a timeout.
    assert_eq!(s.scoring.per_timeout, 0.0);

    let mut sim = started(s.clone());
    let dt = 1.0 / 120.0;
    for _ in 0..(15.0 / dt) as usize {
        sim.step(dt, idle());
    }
    let stats = sim.stats();
    assert!(stats.timeouts >= 3, "balls went away by themselves: {stats:?}");
    assert_eq!((stats.kills, stats.hits, stats.damage_done), (0, 0, 0.0));
    assert_eq!(sim.score(), 0.0);

    // Shot once its first hop has taken 30 of its health, a ball dies of a shot of 60: a kill with
    // its damage counted, where a ball that leaves by itself gives none.
    let mut strong = s;
    strong.weapon.damage = 60.0;
    let mut sim = started(strong);
    for _ in 0..(6.0 / dt) as usize {
        let weak = sim.targets().first().filter(|t| t.health <= 60.0).map(|t| t.pos);
        let input =
            Input { aim: weak.unwrap_or(Vec3::Y).normalize(), fire_pressed: weak.is_some(), fire_held: weak.is_some() };
        sim.step(dt, input);
        if sim.stats().kills > 0 {
            break;
        }
    }
    assert_eq!((sim.stats().kills, sim.stats().damage_done), (1, 60.0), "{:?}", sim.stats());
    assert!(sim.score() > 0.0);
}

/// Plan K7 on one made-up file: two kinds of ball carry a gun but are told not to aim, and the player's
/// own character is invincible while the scenario's switch is off. It imports with no note about weapons.
/// The same text with the balls aiming is refused for "bots shoot back" (they would fire at a player the
/// scenario doesn't protect), and imports again once the scenario makes the player invincible.
#[test]
fn armed_bots_that_do_not_aim_play_and_ones_that_aim_do_not() {
    let i = import(&path("Quiet Guns")).unwrap();
    assert!(i.scenario.problems().is_empty());
    assert!(i.notes.iter().all(|n| !n.to_lowercase().contains("weapon")), "{:?}", i.notes);
    let w = i.scenario.world.as_ref().unwrap();
    assert_eq!(w.kinds.len(), 2);
    assert_eq!(w.slots.len(), 2);

    // In play it is plain target switching: two balls are up, and one goes down to the player's shots.
    let mut sim = started(i.scenario.clone());
    assert_eq!(sim.targets().len(), 2);
    let aim = sim.targets()[0].pos.normalize();
    for _ in 0..10 {
        sim.step(0.2, Input { aim, fire_pressed: true, fire_held: true });
        if sim.stats().kills > 0 {
            break;
        }
    }
    assert_eq!(sim.stats().kills, 1, "{:?}", sim.stats());
    assert!(sim.score() > 0.0);

    let text = std::fs::read_to_string(path("Quiet Guns")).unwrap();
    assert_eq!(text.matches("NoAiming=true").count(), 2);
    let convert = |text: &str| aimadapt_sce::convert::convert(&Sce::parse(text), "quiet.sce", "0");
    let aiming = text.replace("NoAiming=true", "NoAiming=false");
    // The bots' weapons are the only reason: the player's own character being invincible isn't one.
    let refused = convert(&aiming).unwrap_err();
    assert!(!refused.is_empty() && refused.iter().all(|g| g.feature == "bots shoot back"), "{refused:?}");
    let protected = aiming.replace("InvinciblePlayer=false", "InvinciblePlayer=true");
    assert!(convert(&protected).is_ok());
}

/// Plan K6b on one made-up file: a hopping ball with an air jump, a pogo and buffered jumps, a ball
/// with a jetpack (a delay, a tank and a refill that works only on the ground) and a flier flown by
/// its jump and crouch keys that rises on both. Each import comes through with its settings, and in
/// play the hopper and the jetpack bot leave the floor and come back to it while the flier stays up.
#[test]
fn bots_jump_use_jetpacks_and_fly_by_keys() {
    let i = import(&path("Jump Court")).unwrap();
    let s = i.scenario;
    assert!(s.problems().is_empty());
    assert_eq!(i.notes.len(), 1, "{:?}", i.notes);
    let w = s.world.as_ref().unwrap();
    assert_eq!(w.kinds.len(), 3);
    let m = |k: usize| w.kinds[k].motion.as_ref().expect("every kind moves");
    let hopper = m(0);
    assert!(hopper.pogo && hopper.buffered_jumps);
    assert_eq!((hopper.jump_speed, hopper.air_jumps, hopper.air_jump_speed), ([3.0, 3.0], 1, 3.5));
    assert_eq!((hopper.step_up, hopper.dodges[0].jump_chance, hopper.dodges[0].jump_s), (0.75, 0.6, [0.6, 1.2]));
    let j = m(1).jetpack.as_ref().expect("a jetpack");
    assert_eq!(
        (j.thrust, j.max_up, j.delay_s, j.fuel_s, j.refill_per_s, j.refill_in_air),
        (15.0, 2.5, 0.4, 2.5, 0.5, false)
    );
    let glider = m(2);
    assert!(glider.flight.as_ref().unwrap().up_on_both && glider.jetpack.is_none());
    let d = &glider.dodges[0];
    assert_eq!((d.fly_by_keys, d.jump_chance, d.crouch_chance, d.crouch_s), (true, 0.5, 0.5, [0.4, 0.8]));

    let mut sim = Sim::new(s, 9);
    while sim.phase() == Phase::Countdown {
        sim.step(0.05, idle());
    }
    let mut range = [(f32::MAX, f32::MIN); 3];
    for _ in 0..(20.0 * 120.0) as usize {
        sim.step(1.0 / 120.0, idle());
        for t in sim.targets() {
            let r = &mut range[t.kind().unwrap() as usize];
            *r = (r.0.min(t.pos.y), r.1.max(t.pos.y));
        }
    }
    for (kind, (lo, hi)) in range.into_iter().enumerate() {
        assert!(hi - lo > 0.4, "kind {kind} moved up and down: {lo}..{hi}");
    }
}
