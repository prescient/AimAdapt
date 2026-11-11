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
        world.motion.expect("a jumper moves").gravity
    };
    assert!((gravity("Gravity=1.0\n") - 8.0).abs() < 1e-4);
    assert!((gravity("Gravity=0.3\n") - 2.4).abs() < 1e-4);
    // A character without the setting has a gravity of 1.
    assert!((gravity("") - 8.0).abs() < 1e-4);
    assert_eq!(gravity("Gravity=0.0\n"), 0.0);
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

/// Bug: Air Pure and Air CELESTIAL end when the bot runs out of lives and score
/// the time left, which isn't played: they played 1000 s scoring nothing. Rules
/// that decide the score or when a run ends say they can't be played yet.
#[test]
fn bot_lives_and_unplayed_score_rules_cant_play_yet() {
    let lives = VOLUME_SCE.replace("ScorePerKill=1.0", "ScorePerKill=0.0\nBotMaxLives=5\nScorePerTime=1.0");
    let r = reasons(&lives);
    assert!(r.iter().any(|x| x.starts_with("bots with a set number of lives")), "{r:?}");
    assert!(r.iter().any(|x| x.starts_with("a score for time")), "{r:?}");
    // Bots that can't die never run out of lives.
    let invincible = VOLUME_SCE.replace("ScorePerKill=1.0", "ScorePerKill=1.0\nBotMaxLives=1\nInvincibleBots=true");
    assert!(!reasons(&invincible).iter().any(|x| x.contains("lives")), "{:?}", reasons(&invincible));
    // Bug: `InvincibleBots` was ignored (VT Snake Track), so its bot died.
    assert!(convert(&Sce::parse(&invincible), "box.sce", "0").unwrap().scenario.world.unwrap().invincible);
    let eff = VOLUME_SCE.replace("ScorePerKill=1.0", "ScorePerKill=1.0\nScoreMultDamageEfficiency=true");
    assert!(
        reasons(&eff).iter().any(|x| x.starts_with("a score multiplied by damage efficiency")),
        "{:?}",
        reasons(&eff)
    );
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
