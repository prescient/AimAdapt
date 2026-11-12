//! Tests for turning `.sce` files into scenarios.

use super::*;

/// Bug: `TeamMask` was read with `1 << (team - 1)`, which panics (or wraps in a
/// release build) for a player team above 32.
#[test]
fn a_team_above_32_has_no_bit() {
    assert!(!team_can_use(Teams::Mask(1), 40));
    assert!(team_can_use(Teams::Mask(1 << 31), 32));
    assert!(team_can_use(Teams::Mask(0), 40), "no mask lets everyone in");
}

/// Bug: `MapScale=0` imported, with the whole map squashed onto the player.
#[test]
fn a_zero_map_scale_is_refused() {
    let text = VOLUME_SCE.replace("MapScale=1.0", "MapScale=0");
    let err = convert(&Sce::parse(&text), "x.sce", "0").unwrap_err();
    assert!(err.iter().any(|r| r.feature == "a map scale of 0 or below"), "{err:?}");
}

#[test]
fn unreal_axes_become_the_sims() {
    // Forward, right, up.
    assert_eq!(axes([1.0, 0.0, 0.0]), Vec3::NEG_Z);
    assert_eq!(axes([0.0, 1.0, 0.0]), Vec3::X);
    assert_eq!(axes([0.0, 0.0, 1.0]), Vec3::Y);
    assert_eq!(map_point([200.0, 0.0, 0.0], 4.0), Vec3::new(0.0, 0.0, -8.0));
}

#[test]
fn rotations_turn_forward_right_and_up() {
    let fwd = glam::Vec3::X;
    // Yaw 90: facing right. Pitch -15: looking down.
    assert!((ue_rotation([0.0, 0.0, 90.0]) * fwd - glam::Vec3::Y).length() < 1e-5);
    assert!((ue_rotation([0.0, -15.0, 0.0]) * fwd).z < 0.0);
}

/// Bug: roll turned the wrong way, so rolled walls stood in the wrong place: VT
/// Pulseshot's and horizontal map's rooms were all below the player, and DriftTS's
/// view-limiting ramps crossed the view. A rotation is Unreal's `FRotator`, whose
/// matrix has these rows (where forward, right and up end up).
#[test]
fn rotations_follow_unreals_rotator() {
    for [roll, pitch, yaw] in [[30.0, 0.0, 0.0], [-45.0, -90.0, 0.0], [144.5, 90.0, 9.5], [20.0, 35.0, -70.0]] {
        let [r, p, y] = [roll, pitch, yaw].map(|a: f32| a.to_radians());
        let (cr, sr, cp, sp, cy, sy) = (r.cos(), r.sin(), p.cos(), p.sin(), y.cos(), y.sin());
        let rows = [
            glam::Vec3::new(cp * cy, cp * sy, sp),
            glam::Vec3::new(sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp),
            glam::Vec3::new(-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp),
        ];
        let q = ue_rotation([roll, pitch, yaw]);
        for (axis, row) in [glam::Vec3::X, glam::Vec3::Y, glam::Vec3::Z].into_iter().zip(rows) {
            assert!((q * axis - row).length() < 1e-4, "{roll} {pitch} {yaw}: {axis} went to {}, not {row}", q * axis);
        }
    }
}

#[test]
fn team_masks_are_one_bit_per_team() {
    assert!(team_can_use(Teams::Mask(1), 1));
    assert!(!team_can_use(Teams::Mask(1), 2));
    assert!(team_can_use(Teams::Mask(2), 2));
    assert!(team_can_use(Teams::Mask(3), 2));
    assert!(team_can_use(Teams::Flags { a: false, b: true }, 2));
}

#[test]
fn colors_in_either_scale() {
    assert_eq!(hex_color([1.0, 0.0, 0.0]), "#ff0000");
    assert_eq!(hex_color([255.0, 128.0, 0.0]), "#ff8000");
}

#[test]
fn a_cube_spans_100_units_per_scale_step_from_its_corner() {
    let p = Placement { location: [100.0, -50.0, 0.0], rotation: [0.0; 3], scale: [2.0, 1.0, 1.0] };
    let m = cube(&p, 1.0, &|v| v);
    let xs: Vec<f32> = m.positions.iter().map(|q| q[2]).collect();
    // Unreal X 100..300 is sim Z -1..-3 m.
    assert!(xs.iter().all(|z| (*z + 1.0).abs() < 1e-5 || (*z + 3.0).abs() < 1e-5), "{xs:?}");
    assert_eq!(m.triangles.len(), 12);
}

/// Made up for this test: three still bots on a map whose only bot spawn is a
/// `SpawnVolume`, 1 m deep, 8 m wide and 2 m tall, 10 m ahead.
const VOLUME_SCE: &str = r#"Name=AimAdapt Test - Box
AimTypeTag=Clicking
PlayerProfile=Gunner
AddedBots=Dot.bot;Dot.bot;Dot.bot
BotTeams=2;2;2
ScorePerKill=1.0
MapScale=1.0
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
WeaponProfileNames=Tapper
[Weapon Profile]
Name=Tapper
Type=Hitscan
Category=SemiAuto
DamagePerShot=1.0
TimeBetweenShots=0.2
[Bot Profile]
Name=Dot
CharacterProfile=Dot Ball
[Character Profile]
Name=Dot Ball
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=20.0
MainBBHeight=40.0
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}]},
 {"type": "gameObject", "name": "SpawnVolume", "location": "1000, -400, -100", "rotation": "0, 0, 0", "scale": "1, 8, 2", "properties": [{"name": "TeamMask", "value": 0}]}
]}
"#;

/// Bug: every bot on a spawn volume appeared at one spot, the volume's corner,
/// so on "4BK - Micro Horizontal Easy Reflex" the balls sat on top of each other
/// and came back in the same place. A volume is a box placed from its lowest
/// corner, 100 units per scale step like a `Cube`, and bots appear anywhere in it.
#[test]
fn bots_appear_anywhere_in_a_spawn_volume() {
    let imported = convert(&Sce::parse(VOLUME_SCE), "box.sce", "0").unwrap();
    let world = imported.scenario.world.unwrap();
    let mut rng = aimadapt_sim::sim::Rng::new(5);
    let points: Vec<Vec3> = (0..200).map(|_| world.spawn_pos(&mut rng, &[], 0.2, Vec3::NEG_Z)).collect();
    // Unreal x 1000–1100, y −400–400, z −100–100 is sim z −10 to −11, x −4 to 4, y −1
    // to 1, and the 0.4 m balls' middles are their height above that: y −0.6 to 1.4.
    for p in &points {
        assert!((-4.0..=4.0).contains(&p.x) && (-0.6..=1.4).contains(&p.y) && (-11.0..=-10.0).contains(&p.z), "{p}");
    }
    let spread = |f: fn(&Vec3) -> f32| {
        let v: Vec<f32> = points.iter().map(f).collect();
        v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
    };
    assert!(spread(|p| p.x) > 7.0 && spread(|p| p.y) > 1.5 && spread(|p| p.z) > 0.8, "{:?}", &points[..5]);
    assert!(imported.notes.is_empty(), "{:?}", imported.notes);
}

/// Bug: a brush saved as its own triangles (KovaaK's editor does this for some
/// cubes) was drawn at 1 / MapScale of its size: the editor divides those
/// triangles by the map scale (4BK - SYW Truly FIXED's floor was 8 times too small).
#[test]
fn a_brushs_own_triangles_are_scaled_like_a_cube() {
    let tri = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
        let v = |p: [f32; 3]| format!("{{\"location\": \"{}, {}, {}\"}}", p[0], p[1], p[2]);
        format!("{{\"indices\": [0, 1, 2], \"vertices\": [{}, {}, {}]}}", v(a), v(b), v(c))
    };
    // One face of a cube saved at MapScale 4: 0 to 25 instead of 0 to 100.
    let part = tri([0.0, 0.0, 0.0], [25.0, 0.0, 0.0], [25.0, 25.0, 25.0]);
    let text = VOLUME_SCE.replace("MapScale=1.0", "MapScale=4.0").replace(
            "
]}",
            &format!(",
 {{\"type\": \"brush\", \"name\": \"Default\", \"mesh\": \"Cube\", \"location\": \"500, 0, 0\", \"rotation\": \"0, 0, 0\", \"scale\": \"2, 1, 1\", \"procedural\": [{part}]}}
]}}"),
        );
    let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
    let m = w.meshes.last().unwrap();
    let p: Vec<Vec3> = m.positions.iter().map(|p| Vec3::from_array(*p)).collect();
    // Unreal x 500 to 700 (scale 2), times MapScale 4, is 8 m deep.
    assert!(((p[1] - p[0]).length() - 8.0).abs() < 1e-3, "{p:?}");
    assert!(((p[2] - p[1]).length() - 4.0 * 2f32.sqrt()).abs() < 1e-3, "{p:?}");
}

fn reasons(text: &str) -> Vec<String> {
    convert(&Sce::parse(text), "box.sce", "0").err().unwrap_or_default().iter().map(ToString::to_string).collect()
}

/// Bug: "4BK - SYV Easy Slow Regen Edit" played as a still ball. Its bot has no
/// `MaxSpeed` but flies up and down; flying and jumping bots move.
#[test]
fn flying_and_jumping_bots_move() {
    let motion = |text: &str| convert(&Sce::parse(text), "box.sce", "0").unwrap().scenario.world.unwrap().motion;
    let flyer = VOLUME_SCE.replace("MainBBType=Spheroid", "MainBBType=Spheroid\nIsFlyer=true\nFlightVelocityUp=800.0");
    let f = motion(&flyer).expect("a flier moves").flight.expect("and flies");
    assert_eq!(f.up_speed, 8.0);
    let jumper = VOLUME_SCE
        .replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nDodgeProfileNames=Hop")
        .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nJumpVelocityMax=256.0\nJumpVelocityMin=200.0")
        .replace(
            "[Map Data]",
            "[Dodge Profile]\nName=Hop\nJumpFrequency=1.0\nMinJumpTime=0.5\nMaxJumpTime=0.7\n[Map Data]",
        );
    let m = motion(&jumper).expect("a jumper moves");
    assert_eq!((m.jump_speed, m.dodges[0].jump_chance, m.dodges[0].jump_s), ([2.0, 2.56], 1.0, [0.5, 0.7]));
    // NoDodging keeps both still.
    let still = flyer.replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nNoDodging=true");
    assert_eq!(motion(&still), None);
}

#[test]
fn a_dodge_profile_and_character_physics_come_through_in_meters() {
    let text = VOLUME_SCE
            .replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nDodgeProfileNames=Weave\nNoAiming=true")
            .replace(
                "MaxSpeed=0.0\nMainBBType",
                "MaxSpeed=600.0\nAcceleration=3000.0\nGravity=2.0\nFriction=4.0\nDisableCharacterCollision=true\nMainBBType",
            )
            .replace(
                "[Map Data]",
                "[Dodge Profile]\nName=Weave\nMinLRTimeChange=0.4\nMaxLRTimeChange=0.9\nMinTargetDistance=500\nMaxTargetDistance=2000\n\
                 InitialRightMovementState=Left\nToggleForwardBack=true\nDamageReactionChangesDirection=true\n\
                 DamageReactionChanceToIgnore=0.25\nBlockedMovementPercent=0.5\n[Map Data]",
            );
    let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
    let m = w.motion.unwrap();
    assert_eq!((m.max_speed, m.accel, m.friction), (6.0, 30.0, 4.0));
    assert!((m.gravity - 16.0).abs() < 1e-4);
    assert!(!m.bump_bots && !m.faces_player);
    let d = &m.dodges[0];
    assert_eq!((d.strafe_s, d.min_distance, d.max_distance), ([0.4, 0.9], 5.0, 20.0));
    assert_eq!((d.start_side, d.advance), (aimadapt_sim::motion::Start::Minus, true));
    assert_eq!(d.hit_turn.as_ref().unwrap().chance, 0.75);
    assert_eq!(d.blocked.as_ref().unwrap().fraction, 0.5);
}

/// Bug: a bot's gravity was read as 9.8 m/s² per unit of `Gravity`. KovaaK's uses
/// 800 units/s² (8 m/s²), measured there from how high and how long bots hop, so
/// imported bots fell about a fifth too fast and jumped too low.
#[test]
fn a_bots_gravity_is_800_units_a_second_squared() {
    let gravity = |setting: &str| {
        let text = VOLUME_SCE
            .replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nDodgeProfileNames=Hop")
            .replace("MainBBType=Spheroid", &format!("MainBBType=Spheroid\n{setting}JumpVelocityMax=256.0"))
            .replace("[Map Data]", "[Dodge Profile]\nName=Hop\nJumpFrequency=1.0\n[Map Data]");
        let world = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
        world.motion.map(|m| m.gravity)
    };
    assert!((gravity("Gravity=1.0\n").unwrap() - 8.0).abs() < 1e-4);
    assert!((gravity("Gravity=0.3\n").unwrap() - 2.4).abs() < 1e-4);
    // A character without the setting has a gravity of 1.
    assert!((gravity("").unwrap() - 8.0).abs() < 1e-4);
    // With none a jump would never end, so a bot whose only movement is jumping doesn't move.
    assert_eq!(gravity("Gravity=0.0\n"), None);
}

/// Bots of several kinds: each keeps its own size, health, color and movement,
/// and appears only where its character may.
#[test]
fn several_kinds_of_bot_play_side_by_side() {
    let text = VOLUME_SCE
            .replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Dot.bot;Big.bot")
            .replace("BotTeams=2;2;2", "BotTeams=2;2")
            .replace(
                "[Map Data]",
                "[Bot Profile]\nName=Big\nCharacterProfile=Big Ball\nDodgeProfileNames=Weave\n\
                 [Character Profile]\nName=Big Ball\nMaxHealth=50.0\nMaxSpeed=500.0\nMainBBType=Spheroid\n\
                 MainBBRadius=60.0\nMainBBHeight=120.0\nEnemyBodyColor=X=0.000 Y=0.000 Z=1.000\n\
                 [Dodge Profile]\nName=Weave\n[Map Data]",
            )
            .replace(
                "\"properties\": [{\"name\": \"TeamMask\", \"value\": 0}]}",
                "\"properties\": [{\"name\": \"TeamMask\", \"value\": 0}]},\n {\"type\": \"gameObject\", \"name\": \"SpawnPoint\", \"location\": \"1500, 0, 0\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 1, 1\", \"properties\": [{\"name\": \"TeamMask\", \"value\": 2}, {\"name\": \"PermittedCharacterProfiles\", \"value\": \"Big Ball\"}]}",
            );
    let s = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario;
    let w = s.world.as_ref().unwrap();
    assert_eq!(w.kinds.len(), 2);
    assert_eq!(w.slots, vec![vec![(0, 1.0)], vec![(1, 1.0)]]);
    let (dot, big) = (&w.kinds[0], &w.kinds[1]);
    assert_eq!((dot.radius, dot.health, dot.motion.is_some()), (0.2, 1.0, false));
    assert_eq!((big.radius, big.health, big.color.as_str()), (0.6, 50.0, "#0000ff"));
    assert!(big.motion.is_some() && w.motion.is_none());
    // The volume is for anyone; the far point only for the big ball.
    assert_eq!((dot.spawns.clone(), big.spawns.clone()), (vec![0], vec![]));
    // In play: one of each.
    let sim = aimadapt_sim::Sim::new(s.clone(), 4);
    let mut kinds: Vec<Option<u32>> = sim.targets().iter().map(|t| t.kind()).collect();
    kinds.sort();
    assert_eq!(kinds, vec![Some(0), Some(1)]);
    let big_t = sim.targets().iter().find(|t| t.kind() == Some(1)).unwrap();
    assert_eq!((big_t.radius, big_t.health), (0.6, 50.0));
}

/// Bots whose `InvertBlockedSpawn` radius puts them in clusters.
#[test]
fn inverted_bot_spawn_radius_makes_clusters() {
    let text = VOLUME_SCE
        .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nBlockedSpawnRadius=100.0\nInvertBlockedSpawn=true");
    let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
    assert!(w.cluster && (w.blocked_radius - 1.0).abs() < 1e-6);
}

#[test]
fn a_targets_melee_ability_cant_play() {
    let text = VOLUME_SCE
        .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nAbilityProfileNames=Shove.abilmelee;;")
        .replace("[Map Data]", "[Melee Ability Profile]\nName=Shove\nHurtboxRadius=80.0\n[Map Data]");
    // Only helper bots, on the player's team, push; a target's melee ability isn't played.
    assert_eq!(reasons(&text), vec!["bots that use abilities (a target's melee ability: Shove)".to_string()]);
}

/// Made up for these tests: the bots' character has one movement ability, and the
/// bot profile and character say when it is used.
fn dasher(ability: &str) -> String {
    VOLUME_SCE
        .replace(
            "CharacterProfile=Dot Ball",
            "CharacterProfile=Dot Ball\nUseAbilityFrequency=0.5\nUseAbilityFreqMinTime=0.4\nUseAbilityFreqMaxTime=1.2\n\
             AbilityUseTimer=0.3\nSeeThroughWalls=true",
        )
        .replace(
            "MainBBType=Spheroid",
            "MainBBType=Spheroid\nAbilityProfileNames=Zip.abilmov;;\nBlockAbilityOnStartDuration=1.5\nAbilityGlobalCooldown=0.6",
        )
        .replace("[Map Data]", &format!("[Movement Ability Profile]\nName=Zip\n{ability}\n[Map Data]"))
}

const ZIP: &str = "MaxCharges=2.0\nChargeTimer=1.5\nDelayAfterUse=0.25\nFullyAuto=true\nAbilityDuration=0.3\n\
    LockDirectionForDuration=true\nNegateGravityForDuration=true\nMainVelocity=1800.0\nMainVelocityCanGoVertical=false\n\
    MainVelocitySetToMovementKeys=true\nUpVelocity=150.0\nEndVelocityFactor=0.5\nAbilityBlocksMovement=true\n\
    AIUseInCombat=true\nAIUseOutOfCombat=false\nAIUseOnGround=true\nAIUseInAir=false\nAIReuseTimer=0.4\n\
    AIMinTargDist=500.0\nAIMaxTargDist=3000.0\nAIMaxTargFOV=90.0\nChargesOnSpawn=1.0";

#[test]
fn a_movement_ability_comes_through_in_meters_and_seconds() {
    let i = convert(&Sce::parse(&dasher(ZIP)), "box.sce", "0").unwrap();
    let m = i.scenario.world.unwrap().motion.expect("a bot with an ability moves, though its MaxSpeed is 0");
    assert_eq!(m.max_speed, 0.0);
    let ab = m.abilities.expect("and has abilities");
    assert_eq!((ab.press_every_s, ab.chance, ab.window_s), (0.3, 0.5, [0.4, 1.2]));
    assert_eq!((ab.start_block_s, ab.global_cooldown_s, ab.sees_through_walls), (1.5, 0.6, true));
    assert_eq!(ab.list.len(), 1);
    let a = &ab.list[0];
    assert_eq!((a.max_charges, a.spawn_charges, a.charge_s), (2, 1, 1.5));
    assert_eq!((a.delay_s, a.reuse_s, a.duration_s), (0.25, 0.4, 0.3));
    assert_eq!((a.speed, a.up_speed, a.end_factor), (18.0, 1.5, 0.5));
    assert!(a.along_input && !a.vertical && a.lock_direction && a.no_gravity && a.blocks_movement);
    assert_eq!((a.in_combat, a.out_of_combat, a.on_ground, a.in_air), (true, false, true, false));
    assert_eq!((a.min_distance, a.max_distance, a.max_angle_deg), (5.0, 30.0, 90.0));
    let note = "bots use their movement abilities (dashes and jumps), timed as measured in KovaaK's; there a dash now \
                and then comes later than its settings say";
    assert!(i.notes.iter().any(|n| n == note), "{:?}", i.notes);
}

/// Bug: a character without a `BlockAbilityOnStartDuration` key played as having no block;
/// KovaaK's waits 1 s. An explicit 0 stays 0.
#[test]
fn a_character_without_a_start_block_key_waits_one_second_and_an_explicit_zero_stays_zero() {
    let block = |text: &str| {
        let m = convert(&Sce::parse(text), "box.sce", "0").unwrap().scenario.world.unwrap().motion.unwrap();
        m.abilities.unwrap().start_block_s
    };
    let with_key = dasher(ZIP);
    assert_eq!(block(&with_key), 1.5);
    assert_eq!(block(&with_key.replace("BlockAbilityOnStartDuration=1.5", "BlockAbilityOnStartDuration=0.0")), 0.0);
    let without = with_key.replace("\nBlockAbilityOnStartDuration=1.5", "");
    assert!(!without.contains("BlockAbilityOnStartDuration"));
    assert_eq!(block(&without), 1.0);
}

#[test]
fn a_movement_ability_with_only_a_launch_takes_the_defaults() {
    let m = convert(&Sce::parse(&dasher("UpVelocity=400.0")), "box.sce", "0").unwrap().scenario.world.unwrap().motion;
    let a = m.unwrap().abilities.unwrap().list.remove(0);
    assert_eq!((a.max_charges, a.spawn_charges, a.end_factor, a.max_angle_deg), (1, 1, 1.0, 360.0));
    assert_eq!((a.min_distance, a.max_distance, a.up_speed), (0.0, 0.0, 4.0));
    assert!(a.in_combat && a.out_of_combat && a.on_ground && a.in_air);
    assert!(!a.lock_direction && !a.no_gravity);
    // An ability that moves nothing leaves a still bot still.
    let still = convert(&Sce::parse(&dasher("AbilityDuration=0.2")), "box.sce", "0").unwrap().scenario.world.unwrap();
    assert_eq!(still.motion, None);
    // Never usable (neither on the ground nor in the air) still imports.
    let never = dasher("UpVelocity=400.0\nAIUseOnGround=false\nAIUseInAir=false");
    assert!(convert(&Sce::parse(&never), "box.sce", "0").is_ok());
}

#[test]
fn movement_abilities_with_settings_the_trainer_doesnt_play_say_which() {
    for (setting, value) in [
        ("HealthRestore", "20.0"),
        ("Hurtbox", "true"),
        ("AIMinSelfHealth", "10.0"),
        ("AIMaxSelfHealth", "90.0"),
        ("AIMinTargHealth", "5.0"),
        ("AIMaxTargHealth", "50.0"),
        ("CancelDashOnGreaterThan45Collisions", "true"),
    ] {
        let text = dasher(&format!("MainVelocity=1800.0\n{setting}={value}"));
        assert_eq!(
            reasons(&text),
            vec![format!("a movement ability the trainer can't play yet (Zip: {setting}={value})")],
            "{setting}"
        );
    }
    // Their harmless values play.
    let fine =
        dasher("MainVelocity=1800.0\nHealthRestore=0.0\nHurtbox=false\nAIMinSelfHealth=0.0\nAIMaxSelfHealth=100.0");
    assert_eq!(reasons(&fine), Vec::<String>::new());
}

#[test]
fn weapon_abilities_are_refused_once_and_a_melee_one_beside_a_dash_still_is() {
    let weapon = VOLUME_SCE
        .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nAbilityProfileNames=Zap.abilwep;;")
        .replace("[Map Data]", "[Weapon Ability Profile]\nName=Zap\n[Map Data]");
    assert_eq!(reasons(&weapon), vec!["weapon abilities (Zap)".to_string()]);
    let both = dasher("MainVelocity=1800.0")
        .replace("AbilityProfileNames=Zip.abilmov;;", "AbilityProfileNames=Zip.abilmov;Shove.abilmelee;")
        .replace("[Map Data]", "[Melee Ability Profile]\nName=Shove\n[Map Data]");
    assert_eq!(reasons(&both), vec!["bots that use abilities (a target's melee ability: Shove)".to_string()]);
}

/// Bug: Pasu's player is on team 2, so the player and the bots swapped spawn
/// points: the bots appeared around the player.
#[test]
fn the_player_can_be_on_any_team_and_team_0_bots_are_their_opponents() {
    // Teams swapped: the player starts at the point TeamMask 2 allows, 10 m from
    // the volume the bots (team 0, "the other side") use.
    let text = VOLUME_SCE
        .replace("ScorePerKill=1.0", "ScorePerKill=1.0\nPlayerTeam=2")
        .replace("BotTeams=2;2;2", "BotTeams=0;0;0")
        .replace("{\"name\": \"TeamMask\", \"value\": 1}", "{\"name\": \"TeamMask\", \"value\": 2}")
        .replace("{\"name\": \"TeamMask\", \"value\": 0}", "{\"name\": \"TeamMask\", \"value\": 1}");
    let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
    assert_eq!(w.spawns.len(), 1);
    assert!((w.spawns[0].pos[2] - -10.0).abs() < 1e-4, "{:?}", w.spawns[0].pos);
}

/// Bug: JSON maps' invisible walls (`Clip`, `FullClip`) were drawn and stopped
/// shots, and a player who spawns sunk into the floor saw from under it
/// (Midrange Long Strafes): characters are lifted onto the floor, as in Unreal.
#[test]
fn clip_brushes_are_invisible_and_the_player_stands_on_the_floor() {
    let brush = |name: &str, loc: &str, scale: &str| {
        format!(
            ",\n {{\"type\": \"brush\", \"name\": \"{name}\", \"mesh\": \"Cube\", \"location\": \"{loc}\", \"rotation\": \"0, 0, 0\", \"scale\": \"{scale}\"}}"
        )
    };
    let walls = [
        brush("Default", "-1000, -1000, -50", "20, 20, 0.5"), // a floor whose top is at z = 0
        brush("Clip", "500, -1000, 0", "0.1, 20, 10"),
        brush("DefaultNoCollision", "600, -1000, 0", "0.1, 20, 10"),
        brush("WeaponClip", "700, -1000, 0", "0.1, 20, 10"),
    ]
    .concat();
    let text = VOLUME_SCE
            .replace("MaxSpeed=0.0\nWeaponProfileNames", "MaxSpeed=0.0\nMainBBRadius=30.0\nMainBBHeight=200.0\nCameraOffset=X=0.000 Y=0.000 Z=60.000\nSpawnOffsetMin=X=0.000 Y=0.000 Z=-100.000\nSpawnOffsetMax=X=0.000 Y=0.000 Z=-100.000\nWeaponProfileNames")
            .replace("\n]}", &format!("{walls}\n]}}"));
    let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
    assert_eq!(w.meshes.len(), 3, "the weapon clip is left out");
    assert!(w.meshes[1].clip && !w.meshes[1].decor);
    assert!(w.meshes[2].decor && !w.meshes[2].clip);
    // The floor's top is 1.62 m below the eye: the 2 m player stands 2 cm above it
    // (its middle 1.02 m up), and the camera is 0.6 m above its middle.
    let floor_top = w.meshes[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!((floor_top - -1.62).abs() < 0.005, "{floor_top}");
    // Shots pass the clip wall but not the floor.
    let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
    assert_eq!(walls.ray(Vec3::ZERO, Vec3::NEG_Z, 20.0), None);
    assert!(walls.ray(Vec3::ZERO, Vec3::NEG_Y, 20.0).is_some());
}

/// [`VOLUME_SCE`] with a 2.3 m player (plus `player` settings) appearing `spawn_z`
/// units above a floor whose top is at z = 0, and how far below the eye that floor
/// and the bots' spawn volume's lowest corner (100 units under the floor) are.
fn standing(player: &str, spawn_z: f32) -> (f32, f32) {
    let text = VOLUME_SCE
        .replace(
            "MaxSpeed=0.0\nWeaponProfileNames",
            &format!("MaxSpeed=0.0\nMainBBRadius=55.0\nMainBBHeight=230.0\n{player}WeaponProfileNames"),
        )
        .replace("\"location\": \"0, 0, 0\"", &format!("\"location\": \"0, 0, {spawn_z}\""))
        .replace("\n]}", &format!("{}\n]}}", brush_json("Cube", "-2000, -2000, -16", "0, 0, 0", "40, 40, 0.16")));
    let w = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap();
    let floor = w.meshes[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    (-floor, -w.spawns[0].pos[1])
}

/// Bug: characters stood lower than in KovaaK's, by up to their whole height.
/// Measured there with test maps (`AimAdapt - Eye Test …`, 23 of them): on both map
/// formats a character's middle starts its whole height (`MainBBHeight`) above
/// the point it appears at, players, bots and spawn volumes alike, and the eye is
/// the camera offset above that. With a head (`MainBBHasHead`) the eye is at the
/// top of the body instead (and half the head's offset higher), and the capsule
/// Unreal stands on floors reaches the head's radius further each way. A
/// character whose feet are within `StepUpHeight` (and 2.4 units) above a floor is
/// pulled down onto it, and one on a floor stands 2 units above it.
#[test]
fn characters_start_their_height_above_where_they_appear() {
    let near = |got: f32, want: f32| assert!((got - want).abs() < 0.005, "{got} != {want}");
    // Floating 5 m up: the eye is 2.3 m above the spawn point (KovaaK's: 729.7
    // units over a floor 500 below), and the 0.4 m ball's middle 0.4 m above its
    // volume's corner, 1 m below the floor.
    let (floor, volume) = standing("", 500.0);
    near(floor, 7.3);
    near(volume, 7.3 + 1.0 - 0.4);
    // A camera offset adds to it.
    near(standing("CameraOffset=X=0.000 Y=0.000 Z=50.000\n", 500.0).0, 7.8);
    // With a head the eye is the top of the body: 1.15 m higher (844.9 units).
    let head = "MainBBHasHead=true\nMainBBHeadRadius=45.0\nMainBBHeadOffset=0.0\n";
    near(standing(head, 500.0).0, 8.45);
    // Standing on the floor with a 75-unit step-up: the head's capsule (1.6 m
    // each way from the middle) has its feet 0.7 m up, within the step, so it's
    // pulled down to 2 cm above the floor and the eye is at 1.62 + 1.15 m (276.6
    // units in KovaaK's). With a 30-unit step it stays up: 3.45 m (344.7).
    near(standing(&format!("{head}StepUpHeight=75.0\n"), 0.0).0, 2.77);
    near(standing(&format!("{head}StepUpHeight=30.0\n"), 0.0).0, 3.45);
    // No head and a 120-unit step: the feet are 1.15 m up, so it's pulled down
    // too, and sees from 1.17 m (116.5 units).
    near(standing("StepUpHeight=120.0\n", 0.0).0, 1.17);
    near(standing("StepUpHeight=75.0\n", 0.0).0, 2.3);
}

/// [`VOLUME_SCE`] with its bot's hitbox changed: type, radius and height in Unreal units.
fn with_hitbox(kind: &str, radius: f32, height: f32) -> String {
    VOLUME_SCE
        .replace("MainBBType=Spheroid", &format!("MainBBType={kind}"))
        .replace("MainBBRadius=20.0", &format!("MainBBRadius={radius:.1}"))
        .replace("MainBBHeight=40.0", &format!("MainBBHeight={height:.1}"))
}

fn import_ok(text: &str) -> crate::convert::Imported {
    convert(&Sce::parse(text), "x.sce", "0").unwrap_or_else(|e| panic!("{e:?}"))
}

fn first_spawn_height(text: &str) -> f32 {
    import_ok(text).scenario.world.unwrap().spawns[0].pos[1]
}

/// Bug: a tall round bot started too low. Measured in KovaaK's 3.9.13 on 11 November
/// 2026 (`AimAdapt - Shape Test …` maps): a spheroid's middle starts the larger of its
/// height and twice its radius above its spawn point, and it is a ball of radius R
/// whatever its height is. The importer used twice the radius for every spheroid,
/// and called the tall ones "stretched spheres" in a note.
#[test]
fn a_tall_spheroid_starts_its_height_above_its_spawn_point() {
    let near = |got: f32, want: f32| assert!((got - want).abs() < 0.005, "{got} != {want}");
    let base = first_spawn_height(VOLUME_SCE); // R 20, H 40
    let tall = import_ok(&with_hitbox("Spheroid", 20.0, 120.0));
    let w = tall.scenario.world.clone().unwrap();
    near(w.spawns[0].pos[1], base + 0.8);
    assert_eq!((tall.scenario.targets.shape, tall.scenario.targets.radius), (Shape::Sphere, 0.2));
    assert!(!tall.notes.iter().any(|n| n.contains("stretched")), "{:?}", tall.notes);
    // A flat one starts twice its radius up, like the round one.
    let flat = import_ok(&with_hitbox("Spheroid", 20.0, 20.0));
    near(flat.scenario.world.unwrap().spawns[0].pos[1], base);
    assert!(!flat.notes.iter().any(|n| n.contains("stretched")), "{:?}", flat.notes);
}

#[test]
fn cylinders_play_as_capsules_without_a_note() {
    let i = import_ok(&with_hitbox("Cylindrical", 20.0, 120.0));
    let t = &i.scenario.targets;
    assert_eq!((t.shape, t.radius, t.height), (Shape::Capsule, 0.2, 1.2));
    assert!(!i.notes.iter().any(|n| n.contains("cylinder")), "{:?}", i.notes);
}

#[test]
fn cuboids_play_as_boxes_that_start_their_height_up() {
    let i = import_ok(&with_hitbox("Cuboid", 20.0, 120.0));
    let t = &i.scenario.targets;
    assert_eq!((t.shape, t.radius, t.height), (Shape::Box, 0.2, 1.2));
    assert!(
        (i.scenario.world.unwrap().spawns[0].pos[1] - first_spawn_height(&with_hitbox("Spheroid", 20.0, 120.0))).abs()
            < 1e-4
    );
    // R 20 with H exactly 2R is allowed too.
    assert_eq!(import_ok(&with_hitbox("Cuboid", 20.0, 40.0)).scenario.targets.shape, Shape::Box);
}

/// A box stays lined up with the map's axes, so the importer can play it only when its
/// turn of the map (the player's start yaw) is a multiple of 90 degrees.
#[test]
fn boxes_need_the_player_to_start_along_an_axis() {
    let turned = |yaw: &str| {
        with_hitbox("Cuboid", 20.0, 120.0).replace(
            r#""rotation": "0, 0, 0", "scale": "1, 1, 1""#,
            &format!(r#""rotation": "0, 0, {yaw}", "scale": "1, 1, 1""#),
        )
    };
    for ok in ["0", "90", "180", "-90", "270", "90.3"] {
        assert_eq!(import_ok(&turned(ok)).scenario.targets.shape, Shape::Box, "yaw {ok}");
    }
    for bad in ["45", "30", "-45", "91"] {
        let err = convert(&Sce::parse(&turned(bad)), "x.sce", "0").unwrap_err();
        assert!(err.iter().any(|r| r.feature == "hitbox shape" && r.detail.contains("box bots")), "yaw {bad}: {err:?}");
    }
    // Round bots don't mind the turn.
    let round = VOLUME_SCE
        .replace(r#""rotation": "0, 0, 0", "scale": "1, 1, 1""#, r#""rotation": "0, 0, 45", "scale": "1, 1, 1""#);
    import_ok(&round);
}

#[test]
fn cylinders_and_cuboids_flatter_than_they_are_wide_are_refused() {
    for (kind, word) in [("Cylindrical", "cylinder"), ("Cuboid", "cuboid")] {
        let err = convert(&Sce::parse(&with_hitbox(kind, 20.0, 30.0)), "x.sce", "0").unwrap_err();
        let gap = err.iter().find(|r| r.feature == "hitbox shape").unwrap_or_else(|| panic!("{kind}: {err:?}"));
        assert!(gap.detail.contains(word) && gap.detail.contains("0.30") && gap.detail.contains("0.20"), "{gap:?}");
    }
}

/// A made-up brush for [`VOLUME_SCE`]'s map, which has `MapScale=1`: 100 units a meter.
fn brush_json(mesh: &str, loc: &str, rot: &str, scale: &str) -> String {
    format!(
        ",\n {{\"type\": \"brush\", \"name\": \"Default\", \"mesh\": \"{mesh}\", \"location\": \"{loc}\", \"rotation\": \"{rot}\", \"scale\": \"{scale}\"}}"
    )
}

fn world_with(brushes: &[String]) -> World {
    let text = VOLUME_SCE.replace("\n]}", &format!("{}\n]}}", brushes.concat()));
    convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap()
}

/// A level ray from the eye, `deg` degrees right of straight ahead.
fn level(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    Vec3::new(a.sin(), 0.0, -a.cos())
}

/// Bug: DriftTS's two ramps that limit the view to 90° were drawn as slanted planes
/// across it, and passed shots through. A `Ramp` is placed from its corner like a
/// `Cube`, its slope runs across it (Y to Z), and roll turns as in Unreal: rolled
/// -45° and pitched -90°, this one stands with its side on the line 45° left of
/// straight ahead, its near end 6 m out.
#[test]
fn a_ramp_turned_to_limit_the_view_has_its_side_on_the_limit() {
    let w = world_with(&[brush_json("Ramp", "282.843, -565.685, 200", "-45, -90, 0", "4, 2, 6")]);
    assert!(!w.meshes[0].decor, "a ramp is solid");
    let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
    // Just past the limit, the ramp's near end stops the shot about 6 m out.
    let hit = walls.ray(Vec3::ZERO, level(-50.0), 20.0).expect("the ramp stops a shot past the limit");
    assert!((hit - 6.0 / 5f32.to_radians().cos()).abs() < 0.02, "{hit}");
    // Inside the limit, nothing does.
    assert_eq!(walls.ray(Vec3::ZERO, level(-40.0), 20.0), None);
}

/// Bug: the curved wall of VT ww5t's map (and Aether's, PGT's, Snake Track's) was
/// drawn as four skate ramps off its corners, and passed shots. A `Concave` is a
/// block placed from its corner with a quarter circle cut from the far corner, seen
/// from above: four of them, turned to a square room's corners, make it round.
#[test]
fn four_concave_corners_make_a_round_room() {
    // 15 m across, around the bots' spawn volume 10 m ahead.
    let w = world_with(&[
        brush_json("Concave", "-1500, -1500, -300", "0, 0, 0", "15, 15, 6"),
        brush_json("Concave", "1500, -1500, -300", "0, 0, 90", "15, 15, 6"),
        brush_json("Concave", "1500, 1500, -300", "0, 0, 180", "15, 15, 6"),
        brush_json("Concave", "-1500, 1500, -300", "0, 0, -90", "15, 15, 6"),
    ]);
    let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
    for deg in [0.0, 20.0, 45.0, 70.0, 90.0, 135.0, 180.0, -100.0, -45.0] {
        let hit = walls.ray(Vec3::ZERO, level(deg), 40.0).unwrap_or(f32::MAX);
        assert!((hit - 15.0).abs() < 0.05, "{deg}°: {hit}");
    }
}

/// Bug: VT ControlTS's floor, a `WedgeB`, was drawn as a narrow ramp to one side and
/// passed shots. Like a `Ramp`, its slope runs across it from its high corner: turned
/// 90°, this floor is 1 m higher at the far end, as ControlTS's is in KovaaK's.
#[test]
fn a_wedge_floor_rises_away_from_its_low_end() {
    let w = world_with(&[brush_json("WedgeB", "800, -500, -300", "0, 0, 90", "10, 8, 1")]);
    let walls = aimadapt_sim::collide::Collider::new(&w.meshes);
    // Straight down from 1 m and 7 m ahead: the top is 3 m - x / 8 below the eye.
    for x in [1.0, 7.0] {
        let drop = walls.ray(Vec3::new(0.0, 0.0, -x), Vec3::NEG_Y, 10.0).expect("the wedge is solid");
        assert!((drop - (3.0 - x / 8.0)).abs() < 0.01, "{x} m ahead: {drop}");
    }
}

/// Bug: VT Aether's three targets (a rotation with `Randomized=false`, each losing
/// 100 health a second) were picked at random and never drained. Made up here: a
/// rotation of two bots that differ in health, taken in turn, both draining.
#[test]
fn rotations_in_order_and_draining_health_come_through() {
    let text = VOLUME_SCE
            .replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Turns.rot")
            .replace("BotTeams=2;2;2", "BotTeams=2")
            .replace(
                "[Bot Profile]",
                "[Bot Rotation Profile]\nName=Turns\nProfileNames=Dot;Big Dot\nProfileWeights=1.0;1.0\nRandomized=false\n\
                 [Bot Profile]\nName=Big Dot\nCharacterProfile=Big Ball\n\
                 [Character Profile]\nName=Big Ball\nMaxHealth=50.0\nHealthRegenPerSec=-10.0\nMaxSpeed=0.0\nMainBBType=Spheroid\nMainBBRadius=20.0\nMainBBHeight=40.0\n\
                 [Bot Profile]",
            )
            .replace("MaxHealth=1.0\nMaxSpeed=0.0\nMainBBType", "MaxHealth=1.0\nHealthRegenPerSec=-10.0\nMaxSpeed=0.0\nMainBBType");
    let i = convert(&Sce::parse(&text), "box.sce", "0").unwrap();
    let w = i.scenario.world.unwrap();
    assert_eq!(w.kinds.len(), 2);
    assert_eq!(w.slots, vec![vec![(0, 1.0), (1, 1.0)]]);
    assert_eq!(w.rotations, vec![Some(0)]);
    assert!(w.kinds.iter().all(|k| k.regen_per_s == -10.0), "{:?}", w.kinds);
    assert!(!i.notes.iter().any(|n| n.contains("negative")), "{:?}", i.notes);
    // Picked by weight when the rotation is randomized.
    let random = text.replace("Randomized=false", "Randomized=true");
    let w = convert(&Sce::parse(&random), "box.sce", "0").unwrap().scenario.world.unwrap();
    assert!(w.rotations.is_empty(), "{:?}", w.rotations);
}

/// Skeet Tracking's targets stand behind its walls until something launches them,
/// which isn't played: still targets that are all out of sight can't be played.
#[test]
fn still_bots_that_all_start_behind_walls_cant_be_played() {
    let wall = ",\n {\"type\": \"brush\", \"name\": \"Default\", \"mesh\": \"Cube\", \"location\": \"500, -2000, -1000\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 40, 20\"}";
    let text = VOLUME_SCE.replace("\n]}", &format!("{wall}\n]}}"));
    assert_eq!(reasons(&text), vec!["every place a bot appears is behind a wall".to_string()]);
}

const REFLEX_SCE: &str = "Name=AimAdapt Test - Old Box\nPlayerProfile=Gunner\nAddedBots=Dot.bot\nBotTeams=2\nMapScale=2.0\n\
        [Character Profile]\nName=Gunner\nMaxSpeed=0.0\nMainBBRadius=10.0\nMainBBHeight=100.0\nWeaponProfileNames=Tapper\n\
        [Weapon Profile]\nName=Tapper\nType=Hitscan\nCategory=SemiAuto\n\
        [Bot Profile]\nName=Dot\nCharacterProfile=Dot Ball\n\
        [Character Profile]\nName=Dot Ball\nMaxSpeed=0.0\nMainBBType=Spheroid\nMainBBRadius=20.0\nMainBBHeight=40.0\n\
        [Map Data]\nreflex map version 8\nglobal\n\
        \tentity\n\t\ttype PlayerSpawn\n\t\tVector3 position 0.000000 0.000000 0.000000\n\t\tVector3 angles 90.000000 0.000000 0.000000\n\t\tBool8 teamB 0\n\
        \tentity\n\t\ttype PlayerSpawn\n\t\tVector3 position 500.000000 0.000000 0.000000\n\t\tVector3 angles 270.000000 0.000000 0.000000\n\t\tBool8 teamA 0\n\
        \tbrush\n\t\tvertices\n\
        \t\t\t-1000.000000 -16.000000 -1000.000000\n\t\t\t1000.000000 -16.000000 -1000.000000\n\t\t\t1000.000000 -16.000000 1000.000000\n\t\t\t-1000.000000 -16.000000 1000.000000\n\
        \t\t\t-1000.000000 0.000000 -1000.000000\n\t\t\t1000.000000 0.000000 -1000.000000\n\t\t\t1000.000000 0.000000 1000.000000\n\t\t\t-1000.000000 0.000000 1000.000000\n\
        \t\tfaces\n\
        \t\t\t0.000000 0.000000 1.000000 1.000000 0.000000 4 5 6 7 0x00000000\n\
        \t\t\t0.000000 0.000000 1.000000 1.000000 0.000000 0 3 2 1 0x00000000\n\
        \tbrush\n\t\tvertices\n\
        \t\t\t200.000000 0.000000 -100.000000\n\t\t\t210.000000 0.000000 -100.000000\n\t\t\t210.000000 300.000000 -100.000000\n\t\t\t200.000000 300.000000 100.000000\n\
        \t\tfaces\n\
        \t\t\t0.000000 0.000000 1.000000 1.000000 0.000000 0 1 2 3 0x00000000 internal/editor/textures/editor_clip\n";

/// Reflex maps: centimeters with X right, Y up and Z forward, and yaw first.
///
/// Bug: characters stood half their height too low on these maps, so tamTS's
/// 300-unit player saw from inside the pit KovaaK's has them look down into.
/// Measured in KovaaK's with test maps, a character's middle is its whole
/// height (`MainBBHeight`) above its spawn point, whatever its radius or the
/// map scale, standing or floating: the eye is the spawn plus the height plus
/// the camera offset, and a still 200-unit ball's middle is 200 units up.
#[test]
fn reflex_maps_play() {
    let floor_top = |w: &World| w.meshes[0].positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    let imported = convert(&Sce::parse(REFLEX_SCE), "old.sce", "0").unwrap();
    let w = imported.scenario.world.unwrap();
    // The player faces +X at yaw 90, and the bot's spawn is 5 m that way, times
    // MapScale 2: 10 m straight ahead. Sizes aren't scaled: the 1 m player's eye
    // is 1 m above the floor, and the 0.4 m ball's middle 0.4 m above it.
    let bot = Vec3::from_array(w.spawns[0].pos);
    assert!((bot - Vec3::new(0.0, -0.6, -10.0)).length() < 1e-3, "{bot}");
    // The floor's top face, and the clip wall, invisible.
    assert_eq!(w.meshes.len(), 2);
    assert!((floor_top(&w) - -1.0).abs() < 1e-3, "{}", floor_top(&w));
    assert_eq!(w.meshes[0].triangles.len(), 4, "two quads, two triangles each");
    assert!(w.meshes[1].clip);
    assert!(!imported.notes.iter().any(|n| n.contains("Reflex")), "{:?}", imported.notes);
    // A camera offset adds to the height: 3 m tall with the camera 0.8 m up sees
    // from 3.8 m (the test map in KovaaK's: 379.8 units).
    let tall = REFLEX_SCE.replace(
        "MainBBHeight=100.0\nWeaponProfileNames",
        "MainBBHeight=300.0\nCameraOffset=X=0.000 Y=0.000 Z=80.000\nWeaponProfileNames",
    );
    let imported = convert(&Sce::parse(&tall), "old.sce", "0").unwrap();
    let w = imported.scenario.world.unwrap();
    assert!((floor_top(&w) - -3.8).abs() < 1e-3, "{}", floor_top(&w));
    assert!(!imported.notes.iter().any(|n| n.contains("Reflex")), "{:?}", imported.notes);
}

/// Bugs: health regen was left out (Pokeball Small heals 4 hp/s), the player's
/// spawn rules were ignored (1w4ts, ww5t), and walls didn't stop bots appearing
/// inside them (Amare Microcluster).
#[test]
fn regen_the_players_spawn_rules_and_walls_come_through() {
    let text = VOLUME_SCE
            .replace("MainBBType=Spheroid", "MainBBType=Spheroid\nHealthRegenPerSec=4.0\nHealthRegenDelay=0.25")
            .replace("WeaponProfileNames=Tapper", "WeaponProfileNames=Tapper\nBlockSpawnFOV=40.0\nBlockSpawnDistance=9999.0\nInvertBlockedSpawn=true")
            .replace(
                "\n]}",
                ",\n {\"type\": \"brush\", \"name\": \"Default\", \"mesh\": \"Cube\", \"location\": \"1050, -500, -200\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 10, 4\"}\n]}",
            );
    let s = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario;
    assert_eq!((s.targets.regen_per_s, s.targets.regen_delay_s), (4.0, 0.25));
    let w = s.world.unwrap();
    let b = w.player_block.clone().unwrap();
    assert_eq!((b.fov_deg, b.invert), (40.0, true));
    assert!((b.distance - 99.99).abs() < 1e-3);
    // The wall: Unreal x 1050–1150 is sim z -10.5 to -11.5, its front face inside
    // the spawn volume (z -10 to -11), so every bot ends up in front of it.
    assert_eq!(w.walls.len(), 1);
    let mut rng = aimadapt_sim::sim::Rng::new(9);
    for _ in 0..100 {
        let p = w.spawn_pos(&mut rng, &[], 0.2, Vec3::NEG_Z);
        assert!(p.z >= -10.5 + 0.2 - 1e-4, "{p}");
    }
}

/// Bug: the map step ran only when nothing else was wrong, so a file with an early
/// reason (a hitbox shape) and a map reason (no spawn point for the player) listed
/// only the first, and fixing it revealed the second.
#[test]
fn early_and_map_reasons_are_reported_together() {
    let text = VOLUME_SCE
        .replace("MainBBType=Spheroid", "MainBBType=Cuboid")
        .replace("MainBBHeight=40.0", "MainBBHeight=20.0") // flatter than it is wide: still refused
        .replace(r#""TeamMask", "value": 1}"#, r#""TeamMask", "value": 2}"#)
        .replace(r#""TeamMask", "value": 0}"#, r#""TeamMask", "value": 2}"#);
    let err = convert(&Sce::parse(&text), "x.sce", "0").unwrap_err();
    let has = |f: &str| err.iter().filter(|r| r.feature == f).count();
    assert_eq!(has("hitbox shape"), 1, "{err:?}");
    assert_eq!(has("the map has no spawn point for the player"), 1, "{err:?}");
}

/// A bot radius already refused as out of range still goes through the map step
/// without a panic, and the map's reason is added.
#[test]
fn a_bad_radius_and_a_map_reason_do_not_panic() {
    let text = VOLUME_SCE
        .replace("MainBBRadius=20.0", "MainBBRadius=0.0")
        .replace(r#""TeamMask", "value": 1}"#, r#""TeamMask", "value": 2}"#)
        .replace(r#""TeamMask", "value": 0}"#, r#""TeamMask", "value": 2}"#);
    let err = convert(&Sce::parse(&text), "x.sce", "0").unwrap_err();
    assert!(err.iter().any(|r| r.feature == "bot size"), "{err:?}");
    assert!(err.iter().any(|r| r.feature == "the map has no spawn point for the player"), "{err:?}");
}

/// `FullyAuto` makes no difference for a bot in KovaaK's, so it doesn't change what
/// comes through.
#[test]
fn full_auto_makes_no_difference_to_a_bots_ability() {
    let get = |auto: &str| {
        let text = dasher(&format!("UpVelocity=400.0\nFullyAuto={auto}"));
        convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap().motion
    };
    assert_eq!(get("true"), get("false"));
}

/// A dash that can go up or down with a main velocity does something else in KovaaK's
/// (it launches the bot high into the air), which the trainer can't play yet.
#[test]
fn a_vertical_dash_is_refused_but_a_vertical_jump_imports() {
    let refused = |ability: &str| convert(&Sce::parse(&dasher(ability)), "box.sce", "0").map(|_| ()).unwrap_err();
    let gaps = refused("MainVelocity=2000.0\nMainVelocityCanGoVertical=true");
    assert_eq!(gaps.len(), 1, "{gaps:?}");
    assert_eq!(
        gaps[0].to_string(),
        "a movement ability the trainer can't play yet (Zip: MainVelocityCanGoVertical=true)"
    );
    // A horizontal dash, and a jump that is vertical with no main velocity, play.
    assert!(
        convert(&Sce::parse(&dasher("MainVelocity=2000.0\nMainVelocityCanGoVertical=false")), "box.sce", "0").is_ok()
    );
    let jump = dasher("MainVelocity=0.0\nMainVelocityCanGoVertical=true\nUpVelocity=600.0");
    let m = convert(&Sce::parse(&jump), "box.sce", "0").unwrap().scenario.world.unwrap().motion.unwrap();
    let a = &m.abilities.unwrap().list[0];
    assert_eq!((a.speed, a.up_speed, a.vertical), (0.0, 6.0, true));
}

/// Helper files set "anywhere" and "never" as huge numbers (an `AIMaxTargDist` of a
/// million). Those are cut to what the trainer allows; the file isn't refused for them.
#[test]
fn huge_ability_distances_and_times_are_cut_down_not_refused() {
    let text = dasher("UpVelocity=400.0\nAIMaxTargDist=100000000.0\nChargeTimer=99999.0\nAIReuseTimer=1e9");
    let m = convert(&Sce::parse(&text), "box.sce", "0").unwrap().scenario.world.unwrap().motion.unwrap();
    let a = &m.abilities.unwrap().list[0];
    assert_eq!((a.max_distance, a.charge_s, a.reuse_s), (10_000.0, 3600.0, 3600.0));
    // A dash faster than the trainer allows is refused, with the reason.
    let fast = dasher("MainVelocity=90000.0");
    assert_eq!(reasons(&fast), vec!["a value the trainer doesn't allow (a bot's ability has a speed out of range)"]);
}

/// Whether a bot with `NoDodging` still uses its movement abilities wasn't measured in
/// KovaaK's. Before this was refused, such a bot was imported standing still and its
/// ability was dropped without a word.
#[test]
fn a_dash_on_a_bot_that_doesnt_dodge_is_refused() {
    let text = dasher("MainVelocity=1800.0").replace("AbilityUseTimer=0.3", "AbilityUseTimer=0.3\nNoDodging=true");
    assert_eq!(
        reasons(&text),
        vec!["a movement ability the trainer can't play yet (Zip: on a bot with NoDodging)".to_string()]
    );
    // An ability that moves nothing changes nothing, so the bot stays as it was.
    let idle = dasher("AbilityDuration=0.2").replace("AbilityUseTimer=0.3", "AbilityUseTimer=0.3\nNoDodging=true");
    assert_eq!(reasons(&idle), Vec::<String>::new());
}

/// Damage reactions weren't measured in KovaaK's, so the trainer doesn't play them.
#[test]
fn an_ability_that_reacts_to_damage_is_refused() {
    let text = dasher("UpVelocity=400.0\nAIDamageReaction=true");
    let gaps = convert(&Sce::parse(&text), "box.sce", "0").map(|_| ()).unwrap_err();
    assert_eq!(gaps.len(), 1, "{gaps:?}");
    assert_eq!(gaps[0].to_string(), "a movement ability the trainer can't play yet (Zip: AIDamageReaction=true)");
}

/// Bug: the importer ignored `Timescale`, so a scenario saved at 0.8 played at normal
/// speed and for the wrong length. It goes into the world; without it the clock is 1.0.
#[test]
fn the_time_scale_is_read_into_the_world() {
    let scale = |text: &str| convert(&Sce::parse(text), "box.sce", "0").unwrap().scenario.world.unwrap().time_scale;
    assert_eq!(scale(&VOLUME_SCE.replace("MapScale=1.0", "MapScale=1.0\nTimescale=0.8")), 0.8);
    assert_eq!(scale(&VOLUME_SCE.replace("MapScale=1.0", "MapScale=1.0\nTimescale=2.0")), 2.0);
    assert_eq!(scale(&VOLUME_SCE.replace("MapScale=1.0", "MapScale=1.0\nTimescale=1.0")), 1.0);
    assert_eq!(scale(VOLUME_SCE), 1.0);
}

/// Bug: the importer ignored `Timescale`, so `Timescale=0` (which freezes the game) and
/// absurd values imported as normal scenarios. Values outside 0.1 to 10 are refused.
#[test]
fn a_time_scale_outside_the_range_is_refused() {
    for bad in ["0.0", "0.05", "50", "-1.0"] {
        let text = VOLUME_SCE.replace("MapScale=1.0", &format!("MapScale=1.0\nTimescale={bad}"));
        let err = convert(&Sce::parse(&text), "x.sce", "0").unwrap_err();
        let gap = err.iter().find(|r| r.feature == "time scale").unwrap_or_else(|| panic!("{bad}: {err:?}"));
        assert!(gap.beyond_plan);
        let value = bad.parse::<f32>().unwrap().to_string();
        assert!(gap.detail.contains(&value) && gap.detail.contains("0.1") && gap.detail.contains("10"), "{gap}");
    }
    for ok in ["0.1", "10.0"] {
        let text = VOLUME_SCE.replace("MapScale=1.0", &format!("MapScale=1.0\nTimescale={ok}"));
        assert!(convert(&Sce::parse(&text), "x.sce", "0").is_ok(), "{ok}");
    }
}

/// Two bot slots, so `BotMaxLives` is a list.
fn two_slots(extra: &str) -> String {
    VOLUME_SCE
        .replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Dot.bot;Dot.bot")
        .replace("BotTeams=2;2;2", "BotTeams=2;2")
        .replace("ScorePerKill=1.0", &format!("ScorePerKill=1.0\n{extra}"))
}

/// Bug: `BotMaxLives` is a list with one number per bot slot, and the importer read it as
/// one number, so `1;1` parsed as nothing and a scenario with lives on two slots wasn't
/// refused for lives at all.
#[test]
fn a_list_of_lives_is_read_one_number_per_slot() {
    let r = reasons(&two_slots("BotMaxLives=1;1"));
    assert!(r.iter().any(|x| x.starts_with("bots with a set number of lives")), "{r:?}");
}

fn imported(text: &str) -> Imported {
    convert(&Sce::parse(text), "box.sce", "0").unwrap_or_else(|e| panic!("{e:?}"))
}

fn gaps_of(text: &str) -> Vec<Gap> {
    convert(&Sce::parse(text), "box.sce", "0").err().unwrap_or_default()
}

/// One slot that plays three bots in order, as in the files `BotMaxLives` was measured on.
fn one_slot(extra: &str) -> String {
    VOLUME_SCE
        .replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Turns.rot")
        .replace("BotTeams=2;2;2", "BotTeams=2")
        .replace("ScorePerKill=1.0", &format!("ScorePerKill=1.0\n{extra}"))
        .replace(
            "[Bot Profile]",
            "[Bot Rotation Profile]\nName=Turns\nProfileNames=Dot;Dot;Dot\nProfileWeights=1.0;1.0;1.0\nRandomized=false\n\
             [Bot Profile]",
        )
}

/// Every refusal of lives is beyond the plan and names the setting and what wasn't measured.
fn lives_gap(text: &str, what: &str) {
    let g = gaps_of(text);
    let gap = g.iter().find(|x| x.feature == "bots with a set number of lives").unwrap_or_else(|| panic!("{g:?}"));
    assert!(gap.beyond_plan, "{gap}");
    assert!(gap.detail.contains("BotMaxLives") && gap.detail.contains(what), "{gap}");
    assert!(gap.detail.contains("hasn't been measured in KovaaK's"), "{gap}");
}

/// A slot with lives ends the run on its last kill: `BotMaxLives=N` is N kills.
#[test]
fn one_slot_with_lives_sets_the_kill_limit() {
    for text in [one_slot("BotMaxLives=3"), one_slot("BotMaxLives=3.0"), one_slot("BotMaxLives= 3 ")] {
        assert_eq!(imported(&text).scenario.timing.kill_limit, 3, "{text}");
    }
    // A plain bot with one slot, not a rotation, too.
    let plain = VOLUME_SCE
        .replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Dot.bot")
        .replace("BotTeams=2;2;2", "BotTeams=2\nBotMaxLives=7");
    assert_eq!(imported(&plain).scenario.timing.kill_limit, 7);
    // No lives, or 0, is no limit.
    assert_eq!(imported(&one_slot("BotMaxLives=0")).scenario.timing.kill_limit, 0);
    assert_eq!(imported(&one_slot("")).scenario.timing.kill_limit, 0);
    // Lives that are 0 everywhere leave a list of any length alone.
    assert_eq!(imported(&two_slots("BotMaxLives=0")).scenario.timing.kill_limit, 0);
    assert_eq!(imported(&two_slots("BotMaxLives=0;0")).scenario.timing.kill_limit, 0);
}

/// Lives on several bots, on some and not others, and a list that doesn't match the slots
/// were never measured in KovaaK's.
#[test]
fn lives_beyond_what_was_measured_are_refused() {
    lives_gap(&two_slots("BotMaxLives=1;1"), "several bots");
    lives_gap(&two_slots("BotMaxLives=2;5"), "several bots");
    lives_gap(&two_slots("BotMaxLives=0;2"), "some bots and not others");
    lives_gap(&two_slots("BotMaxLives=2;0"), "some bots and not others");
    // One number for two slots, and three for two.
    lives_gap(&two_slots("BotMaxLives=2"), "1 number(s) for 2 bot slot(s)");
    lives_gap(&two_slots("BotMaxLives=2;0;0"), "3 number(s) for 2 bot slot(s)");
    // The default three slots with a single number.
    let three = VOLUME_SCE.replace("ScorePerKill=1.0", "ScorePerKill=1.0\nBotMaxLives=5");
    lives_gap(&three, "1 number(s) for 3 bot slot(s)");
    // A slot whose bots are only sometimes invincible has no clear number of kills.
    let mixed = one_slot("BotMaxLives=2").replace("ProfileNames=Dot;Dot;Dot", "ProfileNames=Dot;Tough;Dot").replace(
        "[Bot Profile]",
        "[Bot Profile]\nName=Tough\nCharacterProfile=Tough Ball\n\
             [Character Profile]\nName=Tough Ball\nInvincibleBots=true\nMaxHealth=1.0\nMaxSpeed=0.0\n\
             MainBBType=Spheroid\nMainBBRadius=20.0\nMainBBHeight=40.0\n[Bot Profile]",
    );
    lives_gap(&mixed, "only sometimes invincible");
}

/// Bots that can't die have no lives to use up, whether the scenario or their character says so.
#[test]
fn invincible_bots_ignore_their_lives() {
    // Bug: `InvincibleBots` was ignored (VT Snake Track), so its bot died.
    let own = one_slot("BotMaxLives=2\nInvincibleBots=true");
    let i = imported(&own);
    assert_eq!(i.scenario.timing.kill_limit, 0);
    assert!(i.scenario.world.unwrap().invincible);
    // Even a list that would be refused.
    assert!(gaps_of(&two_slots("BotMaxLives=1;1\nInvincibleBots=true")).is_empty());
    // The slot's character, with the scenario's own flag off.
    let by_character = one_slot("BotMaxLives=2").replace(
        "MaxHealth=1.0\nMaxSpeed=0.0\nMainBBType",
        "MaxHealth=1.0\nMaxSpeed=0.0\nInvincibleBots=true\nMainBBType",
    );
    let i = imported(&by_character);
    assert_eq!(i.scenario.timing.kill_limit, 0);
    assert!(i.scenario.world.unwrap().invincible, "the character's flag comes through too");
}

/// `ScorePerTime` comes through to the world as points per game second left.
#[test]
fn a_score_for_time_reaches_the_world() {
    let w = |extra: &str| imported(&one_slot(extra)).scenario.world.unwrap();
    assert_eq!(w("ScorePerTime=2.5").time_score, 2.5);
    assert_eq!(w("ScorePerTime=-1.0").time_score, -1.0);
    assert_eq!(w("").time_score, 0.0);
    assert_eq!(w("ScorePerTime=0.0").time_score, 0.0);
    // By itself it has nothing unverified to say.
    assert!(imported(&one_slot("ScorePerTime=2.5")).notes.is_empty());
}

/// Damage efficiency comes through to the world; kill efficiency can't be played, and no
/// formula for it was ever checked.
#[test]
fn damage_efficiency_plays_and_kill_efficiency_is_beyond_the_plan() {
    let w = |extra: &str| imported(&one_slot(extra)).scenario.world.unwrap();
    assert!(w("ScoreMultDamageEfficiency=true\nScorePerKill=0.0\nScorePerDamage=1.0").damage_efficiency);
    assert!(!w("ScoreMultDamageEfficiency=false").damage_efficiency);
    assert!(!w("").damage_efficiency);
    let g = gaps_of(&one_slot("ScoreMultKillEfficiency=true"));
    assert_eq!(g.len(), 1, "{g:?}");
    assert_eq!(g[0].feature, "a score multiplied by kill efficiency");
    assert!(g[0].beyond_plan);
    assert!(g[0].detail.contains("ScoreMultKillEfficiency") && g[0].detail.contains("no result file"), "{}", g[0]);
}

/// Combinations nobody checked against KovaaK's say so in a note, and checked ones don't.
#[test]
fn unverified_score_combinations_get_a_note() {
    let notes = |extra: &str| imported(&one_slot(&format!("ScorePerKill=0.0\n{extra}"))).notes;
    let has = |n: &[String], s: &str| n.iter().any(|x| x.contains(s) && x.contains("hasn't been checked"));
    // The time score with a multiplier.
    let n = notes("ScorePerTime=1.0\nScoreMultAccuracy=true");
    assert!(has(&n, "points for time left"), "{n:?}");
    let n = notes("ScorePerTime=1.0\nScoreMultDamageEfficiency=true");
    assert!(has(&n, "points for time left"), "{n:?}");
    // Both multipliers.
    let n = notes("ScoreMultAccuracy=true\nScoreMultDamageEfficiency=true\nScorePerDamage=1.0");
    assert!(has(&n, "both accuracy and damage efficiency"), "{n:?}");
    // Damage efficiency on a score that has kills, hits or misses in it.
    for extra in ["ScorePerKill=1.0", "ScorePerHit=1.0", "ScoreLossPerMiss=1.0"] {
        let n = notes(&format!("ScoreMultDamageEfficiency=true\n{extra}"));
        assert!(has(&n, "kills, hits and misses"), "{extra}: {n:?}");
    }
    // What was measured is quiet: damage only with the efficiency, the accuracy alone, time alone.
    let quiet = notes("ScoreMultDamageEfficiency=true\nScorePerDamage=1.0");
    assert!(quiet.is_empty(), "{quiet:?}");
    assert!(notes("ScoreMultAccuracy=true").is_empty());
    assert!(notes("ScorePerTime=1.0").is_empty());
}

// Plan K6a: where bots appear and stand.

/// [`VOLUME_SCE`] with its bot's character given `dot` extra settings and a second kind of bot,
/// `Big`, whose character is a ball with `big` as its hitbox and extra settings (health 1 and
/// no speed, like the first).
fn with_big_kind(dot: &str, big: &str) -> String {
    VOLUME_SCE
        .replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Dot.bot;Big.bot")
        .replace("BotTeams=2;2;2", "BotTeams=2;2")
        .replace("MainBBType=Spheroid", &format!("{dot}MainBBType=Spheroid"))
        .replace(
            "[Map Data]",
            &format!(
                "[Bot Profile]\nName=Big\nCharacterProfile=Big Ball\n[Character Profile]\nName=Big Ball\n\
                 MaxHealth=1.0\nMaxSpeed=0.0\n{big}[Map Data]"
            ),
        )
}

const BIG_HITBOX: &str = "MainBBType=Spheroid\nMainBBRadius=60.0\nMainBBHeight=120.0\n";

/// Bug (plan K6): every kind of bot started at the first kind's height. Each kind starts
/// its own K5 height above its spawn point, so a kind's `rise` is how much higher (or
/// lower) it starts than the first kind.
#[test]
fn each_kind_of_bot_starts_at_its_own_height() {
    let near = |got: f32, want: f32| assert!((got - want).abs() < 0.005, "{got} != {want}");
    // Dot starts 0.4 m above its spawn point (R 20, H 40); Big 1.2 m, so 0.8 m higher.
    let w = import_ok(&with_big_kind("", BIG_HITBOX)).scenario.world.unwrap();
    assert_eq!(w.kinds.len(), 2);
    near(w.spawns[0].pos[1], first_spawn_height(VOLUME_SCE));
    assert_eq!(w.kinds[0].rise, 0.0);
    near(w.kinds[1].rise, 0.8);
    // A smaller second kind starts lower.
    let small = "MainBBType=Spheroid\nMainBBRadius=10.0\nMainBBHeight=20.0\n";
    near(import_ok(&with_big_kind("", small)).scenario.world.unwrap().kinds[1].rise, -0.2);
    // A flat spheroid starts twice its radius up (K5), a tall capsule its height.
    let flat = "MainBBType=Spheroid\nMainBBRadius=60.0\nMainBBHeight=30.0\n";
    near(import_ok(&with_big_kind("", flat)).scenario.world.unwrap().kinds[1].rise, 0.8);
    let capsule = "MainBBType=Cylindrical\nMainBBRadius=20.0\nMainBBHeight=180.0\n";
    near(import_ok(&with_big_kind("", capsule)).scenario.world.unwrap().kinds[1].rise, 1.4);
}

/// Bug (plan K6): every kind of bot used the first kind's `SpawnOffsetMin` and
/// `SpawnOffsetMax`. A kind keeps its own pair only when it differs from the world's.
#[test]
fn a_kind_has_its_own_spawn_offsets_only_when_they_differ() {
    let offsets = |min: &str, max: &str| format!("SpawnOffsetMin={min}\nSpawnOffsetMax={max}\n");
    let dot = offsets("X=0.000 Y=-50.000 Z=0.000", "X=0.000 Y=50.000 Z=0.000");
    let same = import_ok(&with_big_kind(&dot, &format!("{BIG_HITBOX}{dot}"))).scenario.world.unwrap();
    assert_eq!((same.kinds[0].offsets, same.kinds[1].offsets), (None, None));
    // The world's are the first kind's, in the sim's axes and meters.
    assert_eq!((same.offset_min, same.offset_max), ([-0.5, 0.0, 0.0], [0.5, 0.0, 0.0]));
    // Big's own: Unreal x is forward (sim -z), y right (sim x), z up (sim y).
    let own = offsets("X=100.000 Y=0.000 Z=0.000", "X=200.000 Y=0.000 Z=50.000");
    let w = import_ok(&with_big_kind(&dot, &format!("{BIG_HITBOX}{own}"))).scenario.world.unwrap();
    assert_eq!(w.kinds[0].offsets, None);
    assert_eq!(w.kinds[1].offsets, Some([[0.0, 0.0, -1.0], [0.0, 0.5, -2.0]]));
    assert_eq!((w.offset_min, w.offset_max), (same.offset_min, same.offset_max));
    // A first kind with none, and a second with some.
    let w = import_ok(&with_big_kind("", &format!("{BIG_HITBOX}{own}"))).scenario.world.unwrap();
    assert_eq!(w.kinds[1].offsets, Some([[0.0, 0.0, -1.0], [0.0, 0.5, -2.0]]));
}

/// Bug (plan K6): two characters that differ only in their spawn offsets were merged into
/// one kind, so the second's offsets were lost.
#[test]
fn characters_that_differ_only_in_spawn_offsets_are_two_kinds() {
    let same_hitbox = "MainBBType=Spheroid\nMainBBRadius=20.0\nMainBBHeight=40.0\n";
    let none = import_ok(&with_big_kind("", same_hitbox)).scenario.world.unwrap();
    assert!(none.kinds.is_empty(), "alike in everything: one kind, so the world has no list");
    for (key, value) in
        [("SpawnOffsetMin", "X=0.000 Y=-30.000 Z=0.000"), ("SpawnOffsetMax", "X=0.000 Y=0.000 Z=30.000")]
    {
        let w = import_ok(&with_big_kind("", &format!("{same_hitbox}{key}={value}\n"))).scenario.world.unwrap();
        assert_eq!(w.kinds.len(), 2, "{key}");
        assert_eq!(w.slots, vec![vec![(0, 1.0)], vec![(1, 1.0)]]);
    }
}

/// [`VOLUME_SCE`] with `extra` character settings on its bot, which doesn't move on its own.
fn still_bot(extra: &str) -> Option<aimadapt_sim::motion::Motion> {
    let text = VOLUME_SCE.replace("MainBBType=Spheroid", &format!("{extra}MainBBType=Spheroid"));
    import_ok(&text).scenario.world.unwrap().motion
}

/// Bug (plan K6): a bot that never moves hung in the air where it appeared, though its
/// character has gravity and KovaaK's lets it fall. It gets the physics of a pushed bot,
/// with no speed, when the character isn't a flyer and has a `Gravity` above 0. The key
/// has to be there: a character without it stays as it was.
#[test]
fn a_still_bot_with_gravity_falls() {
    let m = still_bot("Gravity=1.0\n").expect("a still bot with gravity falls");
    assert_eq!((m.max_speed, m.gravity), (0.0, 8.0));
    assert!(m.dodges.is_empty() && m.flight.is_none() && m.abilities.is_none());
    assert!((still_bot("Gravity=0.5\n").unwrap().gravity - 4.0).abs() < 1e-4);
    assert_eq!(still_bot("Gravity=0.0\n"), None, "nothing pulls it down");
    assert_eq!(still_bot(""), None, "no Gravity key: as before");
    assert_eq!(still_bot("Gravity=1.0\nIsFlyer=true\n"), None, "a still flyer stays still");
    // Falling isn't moving by a dodge profile: no note for it.
    let text = VOLUME_SCE.replace("MainBBType=Spheroid", "Gravity=1.0\nMainBBType=Spheroid");
    assert!(import_ok(&text).notes.is_empty());
}

/// `StepUpHeight` is read for bots with gravity that don't fly, in meters.
#[test]
fn step_up_height_is_read_for_walkers_with_gravity() {
    let walker = |extra: &str| {
        still_bot(&format!("MaxSpeed=300.0\nStepUpHeight=75.0\n{extra}")).expect("a walker has a motion").step_up
    };
    assert_eq!(walker("Gravity=1.0\n"), 0.75);
    assert_eq!(walker(""), 0.75, "gravity is 1 when the key is missing");
    assert_eq!(walker("Gravity=0.0\n"), 0.0, "no gravity, nothing to step down");
    assert_eq!(walker("IsFlyer=true\nFlightVelocityUp=800.0\nGravity=1.0\n"), 0.0, "a flyer");
    // A still bot that falls carries it too; a negative one is none.
    assert_eq!(still_bot("Gravity=1.0\nStepUpHeight=75.0\n").unwrap().step_up, 0.75);
    assert_eq!(still_bot("Gravity=1.0\nStepUpHeight=-30.0\n").unwrap().step_up, 0.0);
    assert_eq!(still_bot("Gravity=1.0\n").unwrap().step_up, 0.0, "no key, no step");
}

/// Made up for the jump key's tests: the bots' character, with `speed` as its `MaxSpeed`
/// and these other settings, has one dodge profile with these settings.
fn dotted(speed: f32, character: &str, dodge: &str) -> String {
    VOLUME_SCE
        .replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nDodgeProfileNames=Dodge")
        .replace("MaxSpeed=0.0\nMainBBType", &format!("MaxSpeed={speed}\n{character}\nMainBBType"))
        .replace("[Map Data]", &format!("[Dodge Profile]\nName=Dodge\n{dodge}\n[Map Data]"))
}

fn imported_motion(text: &str) -> Option<Motion> {
    convert(&Sce::parse(text), "box.sce", "0").unwrap().scenario.world.unwrap().motion
}

/// Whether the file's bots move by themselves, which is what the note about dodge
/// profiles says.
fn moves_by_itself(text: &str) -> bool {
    let notes = convert(&Sce::parse(text), "box.sce", "0").unwrap().notes;
    notes.iter().any(|n| n.starts_with("bots move by their dodge profiles"))
}

const HOP: &str = "JumpFrequency=0.5\nMinJumpTime=0.4\nMaxJumpTime=0.9";

#[test]
fn the_jump_settings_come_through() {
    let m =
        imported_motion(&dotted(0.0, "JumpVelocity=300.0\nCanPogoJump=true\nAllowBufferedJumps=true", HOP)).unwrap();
    assert!(m.pogo && m.buffered_jumps);
    assert_eq!(m.jump_speed, [3.0, 3.0]);
    let m = imported_motion(&dotted(0.0, "JumpVelocity=300.0", HOP)).unwrap();
    assert!(!m.pogo && !m.buffered_jumps, "off unless the file says so");
    // A real range keeps both ends: each jump picks between them.
    let m = imported_motion(&dotted(0.0, "JumpVelocityMin=200.0\nJumpVelocityMax=800.0", HOP)).unwrap();
    assert_eq!(m.jump_speed, [2.0, 8.0]);
}

/// A bot jumps when its character has a jump speed (either form), gravity to bring it
/// down and a dodge profile that jumps; a jetpack the bot uses counts too.
#[test]
fn a_bot_that_can_jump_or_use_a_jetpack_moves() {
    assert!(moves_by_itself(&dotted(0.0, "JumpVelocity=300.0", HOP)), "the single speed counts");
    assert!(moves_by_itself(&dotted(0.0, "JumpVelocityMax=300.0\nJumpVelocityMin=300.0", HOP)));
    assert!(!moves_by_itself(&dotted(0.0, "JumpVelocity=300.0\nGravity=0.0", HOP)), "no gravity: a jump never ends");
    assert!(!moves_by_itself(&dotted(0.0, "JumpVelocity=300.0", "JumpFrequency=0.0")), "no jump chance");
    assert!(!moves_by_itself(&dotted(0.0, "JumpVelocity=0.0", HOP)), "no jump speed");
    let pack = "HasJetpack=true\nJetpackThrust=1500.0";
    assert!(moves_by_itself(&dotted(0.0, pack, HOP)), "a jetpack with the delay key missing thrusts at once");
    assert!(moves_by_itself(&dotted(0.0, &format!("{pack}\nJetpackActivationDelay=0.3"), HOP)));
    assert!(!moves_by_itself(&dotted(0.0, &format!("{pack}\nJetpackActivationDelay=0.3"), "JumpFrequency=0.0")));
    // KovaaK's never thrusts with a delay of exactly 0, so it isn't used and the bot stays still.
    assert!(!moves_by_itself(&dotted(0.0, &format!("{pack}\nJetpackActivationDelay=0.0"), HOP)));
    assert!(moves_by_itself(&dotted(0.0, &format!("{pack}\nJetpackActivationDelay=0.001"), HOP)));
}

#[test]
fn a_jetpacks_delay_tank_and_refill_come_through() {
    let pack = "HasJetpack=true\nJetpackThrust=1500.0\nJetpackMaxZVelocity=250.0\nJetpackAirControlWithThrust=0.5";
    let all = format!(
        "{pack}\nJetpackActivationDelay=0.4\nJetpackFullFuelTime=2.5\nJetpackFuelIncPerSec=0.5\nJetpackFuelRegensInAir=false"
    );
    let j = imported_motion(&dotted(0.0, &all, HOP)).unwrap().jetpack.expect("a jetpack");
    assert_eq!((j.thrust, j.max_up, j.air_control), (15.0, 2.5, 0.5));
    assert_eq!((j.delay_s, j.fuel_s, j.refill_per_s, j.refill_in_air), (0.4, 2.5, 0.5, false));
    // The keys missing: no delay, an endless tank and a refill in the air.
    let j = imported_motion(&dotted(0.0, pack, HOP)).unwrap().jetpack.expect("a jetpack");
    assert_eq!((j.delay_s, j.fuel_s, j.refill_per_s, j.refill_in_air), (0.0, 0.0, 0.0, true));
    // A delay of exactly 0 leaves the jetpack out; the bot only hops.
    let zero = format!("{pack}\nJetpackActivationDelay=0.0\nJumpVelocity=300.0");
    let m = imported_motion(&dotted(0.0, &zero, HOP)).unwrap();
    assert!(m.jetpack.is_none() && m.jump_speed == [3.0, 3.0]);
}

#[test]
fn a_fliers_key_settings_come_through() {
    let fly = "IsFlyer=true\nFlightVelocityUp=800.0\nFlightVelocityDown=600.0\nGravity=0.0";
    let by_keys = "AlterateJumpCrouchInput=false\nJumpFrequency=0.5\nCrouchInAirFrequency=0.25\nMinCrouchTime=0.4\nMaxCrouchTime=0.9";
    let m =
        imported_motion(&dotted(0.0, &format!("{fly}\nCanCrouch=true\nIsFlyUpOnJumpAndCrouch=true"), by_keys)).unwrap();
    assert!(m.flight.as_ref().unwrap().up_on_both);
    let d = &m.dodges[0];
    assert!(d.fly_by_keys);
    assert_eq!((d.crouch_chance, d.crouch_s), (0.25, [0.4, 0.9]));
    // A dodge profile with the key on, or without it, alternates, as every flier did before.
    for dodge in ["AlterateJumpCrouchInput=true", "JumpFrequency=0.5"] {
        let m = imported_motion(&dotted(0.0, &format!("{fly}\nCanCrouch=true"), dodge)).unwrap();
        assert!(!m.dodges[0].fly_by_keys && !m.flight.unwrap().up_on_both, "{dodge}");
    }
    // The crouch needs a character that can crouch.
    let m = imported_motion(&dotted(0.0, fly, by_keys)).unwrap();
    assert_eq!((m.dodges[0].crouch_chance, m.dodges[0].crouch_s), (0.0, [0.0, 0.0]));
}

/// Bug (found in review of plan K6b): the settings for a landing were stored for every
/// bot whose character has them, and a flier's key settings for every dodge profile that
/// has them, though the first act only on a bot that jumps off the ground and the second
/// only on a flier. A scenario they can't touch got a new fingerprint, and its bests
/// started again for nothing.
#[test]
fn settings_that_cannot_act_are_left_out() {
    let landing = "JumpVelocity=300.0|CanPogoJump=true|AllowBufferedJumps=true".replace('|', "\n");
    let by_keys = "AlterateJumpCrouchInput=false|CrouchInAirFrequency=0.25".replace('|', "\n");
    // A walker that never presses the jump key, one with no jump speed, one with nothing
    // to bring it down, and a flier, which doesn't land.
    let fly = "IsFlyer=true|FlightVelocityUp=800.0|Gravity=0.0".replace('|', "\n");
    for (character, dodge) in [
        (landing.clone(), "JumpFrequency=0.0"),
        (landing.replace("300.0", "0.0"), HOP),
        (format!("{landing}\nGravity=0.0"), HOP),
        (format!("{fly}\n{landing}"), HOP),
    ] {
        let m = imported_motion(&dotted(600.0, &character, dodge)).unwrap();
        assert!(!m.pogo && !m.buffered_jumps, "{character} with {dodge}");
    }
    // A walker's dodge profile isn't flown by keys and has no crouch key.
    let m =
        imported_motion(&dotted(600.0, &format!("{landing}\nCanCrouch=true"), &format!("{by_keys}\n{HOP}"))).unwrap();
    assert!(m.pogo && m.buffered_jumps, "this one hops");
    let d = &m.dodges[0];
    assert!(!d.fly_by_keys);
    assert_eq!((d.crouch_chance, d.crouch_s), (0.0, [0.0, 0.0]));
}

/// Bug: "bots don't crouch yet" was given to every file whose character could crouch
/// and whose dodge profile crouched, though crouching changes nothing for a bot whose
/// crouched height and speed are its standing ones. It stays for a walking bot that would
/// change, and the note about a jetpack's guessed hold time is gone.
#[test]
fn the_crouch_note_is_for_a_walker_that_would_change_when_it_crouches() {
    let notes = |speed: f32, character: &str, dodge: &str| {
        convert(&Sce::parse(&dotted(speed, character, dodge)), "box.sce", "0").unwrap().notes
    };
    let note = "bots don't crouch yet";
    let crouch = "CrouchOnGroundFrequency=0.5";
    let can = "CanCrouch=true";
    assert!(notes(600.0, &format!("{can}\nCrouchHeightModifier=0.5"), crouch).iter().any(|n| n == note), "lower");
    assert!(notes(600.0, &format!("{can}\nMaxCrouchSpeed=300.0"), crouch).iter().any(|n| n == note), "slower");
    assert!(
        notes(600.0, &format!("{can}\nCrouchHeightModifier=0.5"), "CrouchInAirFrequency=0.5").iter().any(|n| n == note)
    );
    // Crouching changes nothing here.
    assert!(
        !notes(600.0, &format!("{can}\nCrouchHeightModifier=1.0\nMaxCrouchSpeed=600.0"), crouch).contains(&note.into())
    );
    assert!(!notes(600.0, can, crouch).contains(&note.into()), "no crouch height or speed given");
    // It would change, but the bot can't crouch, doesn't crouch, or flies.
    assert!(!notes(600.0, "CrouchHeightModifier=0.5", crouch).contains(&note.into()), "can't crouch");
    assert!(
        !notes(600.0, &format!("{can}\nCrouchHeightModifier=0.5"), "CrouchOnGroundFrequency=0.0")
            .contains(&note.into())
    );
    let flier = format!("{can}\nCrouchHeightModifier=0.5\nIsFlyer=true\nFlightVelocityUp=300.0");
    assert!(!notes(600.0, &flier, crouch).contains(&note.into()), "a flier");
    // A jetpack gets no note of its own now.
    let pack = "HasJetpack=true\nJetpackThrust=1500.0";
    assert!(notes(0.0, pack, HOP).iter().all(|n| !n.contains("jetpack")), "{:?}", notes(0.0, pack, HOP));
}

// Plan K15a: a negative `HealthRestore` on a movement ability is a health cost.

/// A hop that costs health.
fn hop(restore: &str) -> String {
    dasher(&format!("UpVelocity=400.0\nHealthRestore={restore}"))
}

fn cost_of(text: &str) -> f32 {
    let w = imported(text).scenario.world.unwrap();
    w.motion.unwrap().abilities.unwrap().list[0].health_cost
}

/// A negative `HealthRestore` imports as the cost, in health points like `MaxHealth`. Healing and
/// no restore are no cost, and healing is still refused with the setting named.
#[test]
fn a_negative_health_restore_is_the_abilitys_cost() {
    assert_eq!(cost_of(&hop("-30.0")), 30.0);
    assert_eq!(cost_of(&hop("-12.5")), 12.5);
    assert_eq!(cost_of(&hop("0.0")), 0.0);
    assert_eq!(cost_of(&dasher("UpVelocity=400.0")), 0.0);
    assert_eq!(reasons(&hop("20.0")), vec!["a movement ability the trainer can't play yet (Zip: HealthRestore=20.0)"]);
}

/// A cost on a bot that can't die was never measured, so the file is refused: the scenario's
/// `InvincibleBots` or the character's own.
#[test]
fn a_cost_on_a_bot_that_cant_die_is_refused() {
    let why = "a movement ability the trainer can't play yet (Zip: HealthRestore=-30.0 on a bot that can't die)";
    let scenario = hop("-30.0").replace("ScorePerKill=1.0", "ScorePerKill=1.0\nInvincibleBots=true");
    assert_eq!(reasons(&scenario), vec![why]);
    let character = hop("-30.0").replace(
        "MaxHealth=1.0\nMaxSpeed=0.0\nMainBBType",
        "InvincibleBots=true\nMaxHealth=1.0\nMaxSpeed=0.0\nMainBBType",
    );
    assert_eq!(reasons(&character), vec![why]);
    // Invincible bots with no cost still play, and a cost on a bot that can die does.
    let free = hop("0.0").replace("ScorePerKill=1.0", "ScorePerKill=1.0\nInvincibleBots=true");
    assert!(reasons(&free).is_empty());
    assert!(reasons(&hop("-30.0")).is_empty());
}

/// A cost on an ability that doesn't move the bot is refused: such an ability isn't played, so the
/// cost would be silently left out.
#[test]
fn a_cost_on_an_ability_that_moves_nothing_is_refused() {
    let text = dasher("AbilityDuration=0.2\nHealthRestore=-30.0");
    assert_eq!(
        reasons(&text),
        vec![
            "a movement ability the trainer can't play yet (Zip: HealthRestore=-30.0 on an ability that doesn't move the bot)"
        ]
    );
}

/// The two combinations with a cost that nobody checked in KovaaK's each get a note, and only they do.
#[test]
fn a_cost_with_regen_or_with_bot_lives_gets_a_note() {
    let regen_note = "a bot that pays health for an ability and also regenerates or drains health";
    let lives_note = "a bot that pays health for an ability, with bot lives";
    let notes = |text: &str| {
        let n = imported(text).notes;
        (n.iter().any(|n| n.contains(regen_note)), n.iter().any(|n| n.contains(lives_note)))
    };
    let regen = |text: String| {
        text.replace(
            "MaxHealth=1.0\nMaxSpeed=0.0\nMainBBType",
            "MaxHealth=1.0\nHealthRegenPerSec=4.0\nMaxSpeed=0.0\nMainBBType",
        )
    };
    let lives = |text: String| {
        text.replace("AddedBots=Dot.bot;Dot.bot;Dot.bot", "AddedBots=Dot.bot")
            .replace("BotTeams=2;2;2", "BotTeams=2\nBotMaxLives=3")
    };
    assert_eq!(notes(&hop("-30.0")), (false, false));
    assert_eq!(notes(&regen(hop("-30.0"))), (true, false));
    assert_eq!(notes(&lives(hop("-30.0"))), (false, true));
    assert_eq!(notes(&lives(regen(hop("-30.0")))), (true, true));
    // The same settings with no cost give neither.
    assert_eq!(notes(&regen(hop("0.0"))), (false, false));
    assert_eq!(notes(&lives(hop("0.0"))), (false, false));
    // A drain counts as regen too.
    let drain = hop("-30.0").replace(
        "MaxHealth=1.0\nMaxSpeed=0.0\nMainBBType",
        "MaxHealth=1.0\nHealthRegenPerSec=-4.0\nMaxSpeed=0.0\nMainBBType",
    );
    assert_eq!(notes(&drain), (true, false));
    // Each note is one line of plain text. Both of these once had a run of spaces in the middle,
    // where the string's line break in the source wasn't escaped.
    let all = imported(&lives(regen(hop("-30.0")))).notes;
    assert!(all.iter().all(|n| !n.contains("  ") && !n.contains('\n')), "{all:?}");
}

/// A scenario with a cost plays with no score for a timeout, as every imported scenario does.
#[test]
fn an_imported_timeout_scores_nothing() {
    assert_eq!(imported(&hop("-30.0")).scenario.scoring.per_timeout, 0.0);
}

// Spawn offsets run along the map's axes, never the spawn point's (measured in KovaaK's 3.9.13).

/// A made-up map for the offset tests: the player's start at the origin turned
/// `player_yaw` degrees, and one bot spawn point 10 m ahead for each of `points`: its
/// `rotation` (roll, pitch, yaw) and the characters it permits.
fn offset_map(player_yaw: f32, points: &[(&str, &str)]) -> String {
    let mut objects = vec![format!(
        r#"{{"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, {player_yaw}", "scale": "1, 1, 1", "properties": [{{"name": "TeamMask", "value": 1}}]}}"#
    )];
    for (rotation, permitted) in points {
        objects.push(format!(
            r#"{{"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, 0", "rotation": "{rotation}", "scale": "1, 1, 1", "properties": [{{"name": "TeamMask", "value": 2}}, {{"name": "PermittedCharacterProfiles", "value": "{permitted}"}}]}}"#
        ));
    }
    format!("[Map Data]\n{{\"version\": \"1.0.0\", \"objects\": [\n {}\n]}}\n", objects.join(",\n "))
}

/// [`VOLUME_SCE`]'s bots and player with `top` lines at the top of the file, `character`
/// settings on the bots' character and the map of [`offset_map`].
fn offset_sce(top: &str, character: &str, player_yaw: f32, points: &[(&str, &str)]) -> String {
    let head = VOLUME_SCE.split("[Map Data]").next().unwrap();
    let head = head.replace("MainBBType=Spheroid", &format!("{character}MainBBType=Spheroid"));
    format!("{top}{head}{}", offset_map(player_yaw, points))
}

/// `GameVersion` lines for the top of a file, none for an empty version.
fn game_version(version: &str) -> String {
    if version.is_empty() { String::new() } else { format!("GameVersion={version}\n") }
}

/// A character's `SpawnOffsetMin` and `SpawnOffsetMax`, both `v` (written as `X=… Y=… Z=…`).
fn fixed_offset(v: &str) -> String {
    format!("SpawnOffsetMin={v}\nSpawnOffsetMax={v}\n")
}

/// Where the targets of the file appear, from the first spawn point, over a number of draws.
fn target_offsets(text: &str) -> Vec<Vec3> {
    let world = import_ok(text).scenario.world.unwrap();
    let at = Vec3::from_array(world.spawns[0].pos);
    let mut rng = aimadapt_sim::sim::Rng::new(5);
    (0..30).map(|_| world.spawn_pos(&mut rng, &[], 0.2, Vec3::NEG_Z) - at).collect()
}

/// Every target appears `want` (meters, sim axes) from the spawn point.
fn assert_all_at(text: &str, want: Vec3) {
    for got in target_offsets(text) {
        assert!(got.distance(want) < 1e-4, "{got} is not {want}");
    }
}

const THREE_M_RIGHT: &str = "X=0.000 Y=300.000 Z=0.000";

/// Bug: the importer stored each spawn point's own axes and ran the offsets along them,
/// so a bot spawn point turned 90 degrees put its targets in the wrong place. Measured
/// in KovaaK's 3.9.13 (JSON and Reflex maps, points turned by yaw and by pitch): a spawn
/// point's rotation never turns an offset. An offset of Y=300 is 3 m to the map's right,
/// the sim's +x for a player who isn't turned. The old code put it on the sim's z.
#[test]
fn an_offset_is_the_same_on_a_turned_spawn_point() {
    let text = offset_sce("", &fixed_offset(THREE_M_RIGHT), 0.0, &[("0, 0, 90", "")]);
    assert_all_at(&text, Vec3::new(3.0, 0.0, 0.0));
    let world = import_ok(&text).scenario.world.unwrap();
    assert!(world.offset_axes.is_some(), "the turned point's own axes would move the target");
}

/// A spawn point pitched up keeps its offsets level: they run along the map's axes.
#[test]
fn an_offset_stays_level_on_a_pitched_spawn_point() {
    let text = offset_sce("", &fixed_offset("X=300.000 Y=0.000 Z=0.000"), 0.0, &[("0, 30, 0", "")]);
    assert_all_at(&text, Vec3::new(0.0, 0.0, -3.0));
}

/// A point turned half round with offsets that mirror each other fills the same box, so
/// nothing is stored for it (and its scenario keeps its fingerprint).
#[test]
fn a_mirrored_box_needs_no_offset_axes() {
    let character = "SpawnOffsetMin=X=0.000 Y=-50.000 Z=0.000\nSpawnOffsetMax=X=0.000 Y=50.000 Z=0.000\n";
    let world = import_ok(&offset_sce("", character, 0.0, &[("0, 0, 180", "")])).scenario.world.unwrap();
    assert_eq!(world.offset_axes, None);
}

/// The player's start turned 90 degrees with a bot spawn point that isn't turned: the
/// point's own axes already are the map's, so nothing is stored. The offset runs along
/// the map's right, which the player now faces (the sim's -z).
#[test]
fn a_turned_player_with_an_unturned_spawn_point_needs_no_offset_axes() {
    let text = offset_sce("", &fixed_offset(THREE_M_RIGHT), 90.0, &[("0, 0, 0", "")]);
    assert_eq!(import_ok(&text).scenario.world.unwrap().offset_axes, None);
    assert_all_at(&text, Vec3::new(0.0, 0.0, -3.0));
}

/// The player's start turned 90 degrees and the bot's spawn point turned another way:
/// the world keeps the map's frame as the turned player sees it.
#[test]
fn a_turned_player_with_a_turned_spawn_point_keeps_the_maps_frame() {
    let text = offset_sce("", &fixed_offset(THREE_M_RIGHT), 90.0, &[("0, 0, 30", "")]);
    let world = import_ok(&text).scenario.world.unwrap();
    let frame = world.offset_axes.expect("the point's own axes would move the target");
    for (got, want) in frame.iter().zip([Vec3::NEG_Z, Vec3::Y, Vec3::X]) {
        assert!(Vec3::from_array(*got).distance(want) < 1e-5, "{frame:?}");
    }
    assert_all_at(&text, Vec3::new(0.0, 0.0, -3.0));
}

/// Two kinds of bot, only the second (Big) with offsets, and two spawn points: the first
/// turned 90 degrees, the second not. Big appears on the turned one when `big_turned`.
fn kinds_on_points(big_turned: bool) -> World {
    let text = with_big_kind("", &format!("{BIG_HITBOX}{}", fixed_offset(THREE_M_RIGHT)));
    let (big, dot) = (("0, 0, 90", "Big Ball"), ("0, 0, 0", "Dot Ball"));
    let points = if big_turned { [big, dot] } else { [("0, 0, 90", "Dot Ball"), ("0, 0, 0", "Big Ball")] };
    let text = format!("{}{}", text.split("[Map Data]").next().unwrap(), offset_map(0.0, &points));
    import_ok(&text).scenario.world.unwrap()
}

/// The world needs the map's frame where the kind that has offsets is used on a turned
/// point; kept to a point that isn't turned, it doesn't.
#[test]
fn offset_axes_follow_the_spawn_points_a_kind_uses() {
    let turned = kinds_on_points(true);
    assert_eq!(turned.kinds.len(), 2);
    assert!(turned.offset_axes.is_some());
    let level = kinds_on_points(false);
    assert_eq!(level.kinds.len(), 2);
    assert_eq!(level.offset_axes, None);
}

/// Bug guard: a file with no offsets keeps exactly the numbers it had, signs of zeros
/// included (the sign is in its fingerprint), however its spawn points are turned.
#[test]
fn no_offsets_stores_what_it_always_did() {
    // An older file too: turning a zero vector would flip a sign.
    for version in ["", "2.0.4.9"] {
        let text = offset_sce(&game_version(version), "", 0.0, &[("0, 0, 90", ""), ("0, 20, 45", "")]);
        let world = import_ok(&text).scenario.world.unwrap();
        assert_eq!(world.offset_axes, None, "{version:?}");
        assert_eq!(world.offset_min.map(f32::to_bits), [0.0f32, 0.0, -0.0].map(f32::to_bits), "{version:?}");
        assert_eq!(world.offset_max.map(f32::to_bits), [0.0f32, 0.0, -0.0].map(f32::to_bits), "{version:?}");
    }
}

/// Bug: a file saved by KovaaK's before 2.0.5.0 has its offsets a quarter turned: X runs
/// along the map's right and Y along its backward. From 2.0.5.0 (and with no version)
/// X runs forward and Y right. The switch is exactly at 2.0.5.0.
#[test]
fn offsets_in_older_files_are_turned_a_quarter() {
    let at = |version: &str, v: &str| offset_sce(&game_version(version), &fixed_offset(v), 0.0, &[("0, 0, 0", "")]);
    let (x, y) = ("X=300.000 Y=0.000 Z=0.000", "X=0.000 Y=300.000 Z=0.000");
    assert_all_at(&at("2.0.4.9", x), Vec3::new(3.0, 0.0, 0.0));
    assert_all_at(&at("2.0.4.9", y), Vec3::new(0.0, 0.0, 3.0));
    for version in ["2.0.5.0", "3.9.13.0", ""] {
        assert_all_at(&at(version, x), Vec3::new(0.0, 0.0, -3.0));
        assert_all_at(&at(version, y), Vec3::new(3.0, 0.0, 0.0));
    }
}

/// Which `GameVersion`s turn the offsets.
#[test]
fn the_version_that_turns_offsets_is_read_by_parts() {
    let turned = |text: &str| offsets::offsets_turned(&Sce::parse(text).settings.top);
    let with_version = |value: &str| turned(&format!("GameVersion={value}\nName=x\n"));
    for old in ["2.0.4.9", "2.0.4.0", "1.0.8.0", "2.0"] {
        assert!(with_version(old), "{old}");
    }
    for new in ["2.0.5.0", "2.0.5", "v2.6.3", "3.7.1", "", "abc"] {
        assert!(!with_version(new), "{new:?}");
    }
    assert!(!turned("Name=x\n"), "no key");
}

/// Bug: `SpawnXOffset`, `SpawnYOffset` and `VerticalSpawnOffset` were ignored. Each is a
/// fixed offset, in the same frame as the newer keys and with the same version rule.
#[test]
fn the_older_offset_keys_are_a_fixed_offset() {
    let at =
        |version: &str, key: &str| offset_sce(&game_version(version), &format!("{key}\n"), 0.0, &[("0, 0, 90", "")]);
    assert_all_at(&at("", "SpawnXOffset=300.0"), Vec3::new(0.0, 0.0, -3.0));
    assert_all_at(&at("", "SpawnYOffset=300.0"), Vec3::new(3.0, 0.0, 0.0));
    assert_all_at(&at("", "VerticalSpawnOffset=50.0"), Vec3::new(0.0, 0.5, 0.0));
    assert_all_at(&at("2.0.4.9", "SpawnXOffset=300.0"), Vec3::new(3.0, 0.0, 0.0));
    assert_all_at(&at("2.0.4.9", "SpawnYOffset=300.0"), Vec3::new(0.0, 0.0, 3.0));
}

/// A leftover under a millimeter in an older key can't be seen, so it stores the same
/// scenario as the file without the key.
#[test]
fn a_tiny_older_offset_is_no_offset() {
    let fingerprint =
        |character: &str| import_ok(&offset_sce("", character, 0.0, &[("0, 0, 90", "")])).scenario.fingerprint();
    assert_eq!(fingerprint("SpawnYOffset=0.05\n"), fingerprint(""));
    assert_ne!(fingerprint("SpawnYOffset=0.5\n"), fingerprint(""));
}

/// A character with the older keys and the newer ones both set isn't read: how KovaaK's
/// adds them wasn't measured. The older keys with newer ones that are all zero are fine.
#[test]
fn older_and_newer_offsets_together_are_refused() {
    let both = format!("SpawnXOffset=300.0\n{}", fixed_offset(THREE_M_RIGHT));
    let err = convert(&Sce::parse(&offset_sce("", &both, 0.0, &[("0, 0, 0", "")])), "x.sce", "0").unwrap_err();
    assert!(err.iter().any(|g| g.feature == "spawn offsets" && g.detail.starts_with("Dot Ball:")), "{err:?}");
    // The second kind of bot too.
    let big = format!("{BIG_HITBOX}{both}");
    let err = convert(&Sce::parse(&with_big_kind("", &big)), "x.sce", "0").unwrap_err();
    assert!(err.iter().any(|g| g.feature == "spawn offsets" && g.detail.starts_with("Big Ball:")), "{err:?}");
    let zeros = format!("SpawnXOffset=300.0\n{}", fixed_offset("X=0.000 Y=0.000 Z=0.000"));
    assert_all_at(&offset_sce("", &zeros, 0.0, &[("0, 0, 0", "")]), Vec3::new(0.0, 0.0, -3.0));
}
