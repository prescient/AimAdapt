//! Tests for helper bots: bots on the player's team that push the targets around.

use aimadapt_sim::{Input, Sim};

use super::super::*;
use super::period;

/// Made up for these tests: a helper `Push` on the player's team (a pinned orb that
/// kicks every 0.02 s), two still targets, a spawn point for the helper listed before
/// the player's own, and one for the targets.
const HELPER_SCE: &str = r#"Name=AimAdapt Test - Helper
AimTypeTag=Clicking
PlayerProfile=Gunner
AddedBots=Push.bot;Dot.bot;Dot.bot
BotTeams=1;2;2
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
[Bot Profile]
Name=Push
CharacterProfile=Push Orb
NoAiming=true
NoDodging=true
UseWeapons=false
UseAbilityFrequency=1.0
AbilityUseTimer=0.01
SeeThroughWalls=false
[Character Profile]
Name=Push Orb
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Cuboid
MainBBRadius=1.0
MainBBHeight=2.0
MainBBHasHead=true
SpawnOffsetMin=X=0.000 Y=10.000 Z=0.000
SpawnOffsetMax=X=0.000 Y=30.000 Z=0.000
AbilityProfileNames=Kick.abilmelee;;
[Melee Ability Profile]
Name=Kick
HurtboxRadius=150.0
HurtboxDamage=0.0
FlatKnockbackHorizontal=300.0
FlatKnockbackVertical=100.0
AbilityDuration=0.01
AIReuseTimer=0.01
ChargeTimer=0.01
DelayAfterUse=0.01
MaxCharges=1.0
AIUseInCombat=true
AIMaxTargFOV=360.0
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "gameObject", "name": "SpawnPoint", "location": "500, 0, 0", "rotation": "0, 0, 90", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "PermittedCharacterProfiles", "value": "Push Orb"}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, -200, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}]}
]}
"#;

fn import(text: &str) -> Result<Imported, Vec<Gap>> {
    convert(&Sce::parse(text), "helper.sce", "0")
}

fn reasons(text: &str) -> Vec<Gap> {
    import(text).err().unwrap_or_default()
}

/// Changes a line of the helper's own profiles.
fn with(old: &str, new: &str) -> String {
    assert!(HELPER_SCE.contains(old), "{old}");
    HELPER_SCE.replacen(old, new, 1)
}

/// Bug guard for the hit schedule measured in KovaaK's: the period is the longest of
/// the ability's reuse timer, its duration plus charge timer and its duration plus
/// delay, rounded up to a whole number of the bot's press timer (a tie counts).
#[test]
fn the_hit_period_follows_what_was_measured() {
    // (press timer, reuse, duration, charge, delay) -> seconds between hits
    for (inputs, want) in [
        ((0.01, 0.01, 0.01, 0.01, 0.01), 0.02),
        ((0.01, 0.01, 0.01, 3.0, 0.01), 3.01),
        ((0.01, 0.01, 0.01, 1.0, 0.01), 1.01),
        ((0.01, 1.0, 0.01, 0.01, 0.01), 1.0),
        ((1.0, 0.01, 0.01, 0.01, 0.01), 1.0),
        ((0.01, 0.01, 0.01, 0.01, 1.0), 1.01),
        ((0.02, 0.05, 0.02, 0.02, 0.02), 0.06),
        ((0.01, 0.05, 0.01, 0.01, 0.01), 0.05),
        ((0.0, 0.3, 0.1, 0.1, 0.1), 0.3),
        // No timers on the ability: every press hits.
        ((0.5, 0.0, 0.0, 0.0, 0.0), 0.5),
    ] {
        let (tick, reuse, duration, charge, delay) = inputs;
        let got = period(tick, reuse, duration, charge, delay);
        assert!((got - want).abs() < 1e-5, "{inputs:?}: {got}, not {want}");
    }
}

#[test]
fn a_helper_is_a_pusher_and_not_a_target() {
    let i = import(HELPER_SCE).unwrap();
    let s = &i.scenario;
    let w = s.world.as_ref().unwrap();
    // Two targets, one kind, no helper in the slots.
    assert_eq!(s.targets.count, 2);
    assert_eq!((w.kinds.len(), w.slots.len(), w.rotations.len()), (0, 0, 0));
    // The helper's spawn point, listed first, is neither the player's start nor a
    // target's: the targets use the one point of their own.
    assert_eq!(w.spawns.len(), 1);
    assert!((w.spawns[0].pos[2] - -10.0).abs() < 1e-4 && (w.spawns[0].pos[0] - -2.0).abs() < 1e-4);
    // A pusher: 5 m ahead of the player's own start (not of the helper's point), 0.02 m
    // up (its height) and 0.2 m along its own right (the middle of its offset), which
    // for a spawn point turned to face right is back toward the player. It faces right.
    assert_eq!(w.pushers.len(), 1);
    let p = &w.pushers[0];
    assert!(Vec3::from_array(p.pos).distance(Vec3::new(0.0, 0.02, -4.8)) < 1e-4, "{:?}", p.pos);
    assert!(Vec3::from_array(p.facing).distance(Vec3::X) < 1e-4, "{:?}", p.facing);
    assert_eq!((p.chases, p.needs_sight), (false, true));
    assert_eq!((p.radius, p.push, p.lift), (1.5, 3.0, 1.0));
    assert!((p.every_s - 0.02).abs() < 1e-6);
    assert!(
        i.notes.iter().any(|n| n == "helper bots push the targets around (1 of them); they aren't drawn or scored")
    );
    // The targets can be pushed, and do nothing by themselves.
    let m = w.motion.as_ref().expect("a target needs a motion to be pushed");
    assert!(m.dodges.is_empty() && m.abilities.is_none() && m.jetpack.is_none() && m.max_speed == 0.0);
    // Nothing about the helper shows without it: bots move only when they say so.
    assert!(!i.notes.iter().any(|n| n.contains("bots move by their dodge profiles")), "{:?}", i.notes);
}

/// A file with only helpers has no bots; one with a helper and no melee ability just
/// leaves it out.
#[test]
fn a_file_with_only_helpers_has_no_bots() {
    let text = with("AddedBots=Push.bot;Dot.bot;Dot.bot\nBotTeams=1;2;2", "AddedBots=Push.bot\nBotTeams=1");
    let r: Vec<String> = reasons(&text).iter().map(ToString::to_string).collect();
    assert_eq!(r, vec!["no bots".to_string()]);
    let ten = (0..51).map(|_| "Dot.bot").collect::<Vec<_>>().join(";");
    let many = with("AddedBots=Push.bot;Dot.bot;Dot.bot", &format!("AddedBots=Push.bot;{ten}"))
        .replace("BotTeams=1;2;2", &format!("BotTeams=1;{}", vec!["2"; 51].join(";")));
    assert!(reasons(&many).iter().any(|g| g.feature == "too many bots"));
    // 50 targets and a helper are fine: the helper isn't counted.
    let ok = many.replacen(";Dot.bot", "", 1).replacen(";2", "", 1);
    assert_eq!(import(&ok).unwrap().scenario.targets.count, 50);
}

/// The helper is left out of the kinds, the slots and the rotations, and the rotation
/// indices still match the slots that are left.
#[test]
fn a_helper_is_left_out_of_kinds_slots_and_rotations() {
    let text = with(
        "AddedBots=Push.bot;Dot.bot;Dot.bot",
        "AddedBots=Pair.rot;Push.bot;Pair.rot;Pair.rot",
    )
    .replace("BotTeams=1;2;2", "BotTeams=2;1;2;2")
    .replace(
        "[Map Data]",
        "[Bot Rotation Profile]\nName=Pair\nProfileNames=Dot;Big\nProfileWeights=1.0;1.0\nRandomized=false\n\
         [Bot Profile]\nName=Big\nCharacterProfile=Big Ball\nUseWeapons=false\n\
         [Character Profile]\nName=Big Ball\nMaxHealth=3.0\nMaxSpeed=0.0\nMainBBType=Spheroid\nMainBBRadius=50.0\nMainBBHeight=100.0\n\
         [Map Data]",
    );
    let s = import(&text).unwrap().scenario;
    let w = s.world.as_ref().unwrap();
    assert_eq!(s.targets.count, 3);
    assert_eq!(w.kinds.len(), 2);
    assert_eq!(w.slots, vec![vec![(0, 1.0), (1, 1.0)]; 3]);
    assert_eq!(w.rotations, vec![Some(0), Some(0), Some(0)]);
    assert_eq!(w.pushers.len(), 1);
    // Every kind can be pushed.
    assert!(w.kinds.iter().all(|k| k.motion.is_some()));
    assert!(w.problems().is_empty(), "{:?}", w.problems());
}

/// Every setting the trainer can't play on a helper refuses the file and names it.
#[test]
fn helpers_the_trainer_cant_play_name_the_setting() {
    let refusals = [
        (
            with("AbilityProfileNames=Kick.abilmelee;;", "AbilityProfileNames=Kick.abilmelee;Dash.abilmov;")
                .replace("[Map Data]", "[Movement Ability Profile]\nName=Dash\n[Map Data]"),
            "Dash is not a melee ability",
        ),
        (
            with("AbilityProfileNames=Kick.abilmelee;;", "AbilityProfileNames=Kick.abilmelee;Zap.abilwep;")
                .replace("[Map Data]", "[Weapon Ability Profile]\nName=Zap\n[Map Data]"),
            "Zap is not a melee ability",
        ),
        (
            with("AbilityProfileNames=Kick.abilmelee;;", "AbilityProfileNames=Kick.abilmelee;Kick2.abilmelee;")
                .replace("[Map Data]", "[Melee Ability Profile]\nName=Kick2\nHurtboxRadius=10.0\n[Map Data]"),
            "2 melee abilities",
        ),
        (with("HurtboxDamage=0.0", "HurtboxDamage=5.0"), "HurtboxDamage=5.0"),
        (
            with("FlatKnockbackVertical=100.0", "FlatKnockbackVertical=100.0\nFlatKnockbackVerticalMin=50.0"),
            "FlatKnockbackVerticalMin=50.0",
        ),
        (
            with("FlatKnockbackVertical=100.0", "FlatKnockbackVertical=100.0\nFlatKnockbackHorizontalMin=0.0"),
            "FlatKnockbackHorizontalMin=0.0",
        ),
        (with("AbilityDuration=0.01", "AbilityDuration=0.06"), "AbilityDuration=0.06"),
        (with("MaxCharges=1.0", "MaxCharges=2.0"), "MaxCharges=2.0"),
        (with("UseAbilityFrequency=1.0", "UseAbilityFrequency=0.9"), "UseAbilityFrequency=0.9"),
        (with("AIUseInCombat=true", "AIUseInCombat=false"), "AIUseInCombat=false"),
        (with("AIMaxTargFOV=360.0", "AIMaxTargFOV=90.0"), "AIMaxTargFOV=90.0"),
        (with("AIMaxTargFOV=360.0", "AIMaxTargFOV=360.0\nAIMaxTargDist=299.0"), "AIMaxTargDist=299.0"),
        (with("HurtboxRadius=150.0", "HurtboxRadius=0.0"), "HurtboxRadius=0.0"),
        // A period under 0.005 s: no timers, so it would hit on every step.
        (
            with("AIReuseTimer=0.01", "AIReuseTimer=0.0")
                .replace("AbilityUseTimer=0.01", "AbilityUseTimer=0.0")
                .replace("ChargeTimer=0.01", "ChargeTimer=0.0")
                .replace("DelayAfterUse=0.01", "DelayAfterUse=0.0")
                .replace("AbilityDuration=0.01", "AbilityDuration=0.0"),
            "hits every",
        ),
        (
            with(
                "UseWeapons=false\nUseAbilityFrequency",
                "UseWeapons=true\nWeaponsProfileNames=Tapper\nUseAbilityFrequency",
            ),
            "UseWeapons=true",
        ),
        // Pinned means no aiming; chasing means a fast flier that always goes forward.
        (with("NoAiming=true", "NoAiming=false"), "neither pinned"),
        (
            with("NoAiming=true\nNoDodging=true", "NoAiming=false\nDodgeProfileNames=Rise")
                .replace("MaxSpeed=0.0\nMainBBType=Cuboid", "MaxSpeed=5000.0\nIsFlyer=true\nMainBBType=Cuboid")
                .replace("[Map Data]", "[Dodge Profile]\nName=Rise\nInitialForwardMovementState=Forward\n[Map Data]"),
            "neither pinned",
        ),
        // Not measured: whether a flier that doesn't move by itself hovers or falls when pushed.
        (
            with("MaxSpeed=0.0\nMainBBType=Spheroid", "MaxSpeed=0.0\nIsFlyer=true\nMainBBType=Spheroid"),
            "a flying target that doesn't move by itself: Dot Ball",
        ),
        // Two bots to choose from in the helper's slot.
        (
            with("AddedBots=Push.bot;Dot.bot;Dot.bot", "AddedBots=Either.rot;Dot.bot;Dot.bot")
                .replace("[Map Data]", "[Bot Rotation Profile]\nName=Either\nProfileNames=Push;Push\n[Map Data]"),
            "2 bots to choose from",
        ),
    ];
    for (text, word) in refusals {
        let r = reasons(&text);
        let helper: Vec<&Gap> = r.iter().filter(|g| g.feature == "helper bots").collect();
        assert!(helper.iter().any(|g| g.detail.contains(word) && g.beyond_plan), "{word}: {r:?}");
    }
    // The edges are fine: equal minimums, a distance of exactly twice the radius, a
    // press timer that gives a whole period, a flier that goes forward, and an
    // empty weapon list.
    for fine in [
        with(
            "FlatKnockbackVertical=100.0",
            "FlatKnockbackVertical=100.0\nFlatKnockbackVerticalMin=100.0\nFlatKnockbackHorizontalMin=300.0",
        ),
        with("AIMaxTargFOV=360.0", "AIMaxTargFOV=360.0\nAIMaxTargDist=300.0"),
        with("AbilityDuration=0.01", "AbilityDuration=0.05"),
        with("UseWeapons=false\nUseAbilityFrequency", "UseWeapons=true\nWeaponsProfileNames=;;;\nUseAbilityFrequency"),
    ] {
        assert_eq!(reasons(&fine), vec![], "{fine}");
    }
}

/// A chasing helper flies at its target, so it only plays when a wall that bots can't
/// pass is between it and every place a target appears.
#[test]
fn a_chaser_needs_a_wall_between_it_and_the_targets() {
    let chaser = |slab: &str| {
        with("NoAiming=true\nNoDodging=true", "NoAiming=false\nDodgeProfileNames=Rise")
            .replace("MaxSpeed=0.0\nMainBBType=Cuboid", "MaxSpeed=20000.0\nIsFlyer=true\nMainBBType=Cuboid")
            .replace(
                "[Map Data]",
                "[Dodge Profile]\nName=Rise\nInitialForwardMovementState=Forward\nToggleForwardBack=true\nBackTimeMult=0.0\n[Map Data]",
            )
            .replace("\n]}", &format!("{slab}\n]}}"))
    };
    let r = reasons(&chaser(""));
    assert!(r.iter().any(|g| g.feature == "helper bots" && g.detail.contains("no wall between")), "{r:?}");
    // A clip slab between the helper (500, 0, 0) and the targets (1000, -200, 0).
    let slab = ",\n {\"type\": \"brush\", \"name\": \"Clip\", \"mesh\": \"Cube\", \"location\": \"700, -500, -100\", \"rotation\": \"0, 0, 0\", \"scale\": \"0.5, 10, 2\"}";
    let p = import(&chaser(slab)).unwrap().scenario.world.unwrap().pushers;
    assert!(p[0].chases);
    // A slab the line doesn't cross doesn't count.
    let beside = slab.replace("-500, -100", "-500, 300");
    assert!(reasons(&chaser(&beside)).iter().any(|g| g.detail.contains("no wall between")));
    // A slow flier, or one that comes back, isn't a chaser.
    for text in [
        chaser(slab).replace("MaxSpeed=20000.0", "MaxSpeed=9000.0"),
        chaser(slab).replace("BackTimeMult=0.0", "BackTimeMult=0.5"),
        chaser(slab).replace("InitialForwardMovementState=Forward", "InitialForwardMovementState=Back"),
        chaser(slab).replace("IsFlyer=true", "IsFlyer=false"),
    ] {
        assert!(reasons(&text).iter().any(|g| g.detail.contains("neither pinned")), "{text}");
    }
}

/// A helper with no ability, or one that pushes by nothing, is left out with a note.
#[test]
fn a_helper_with_nothing_to_push_with_is_left_out() {
    let none = with("AbilityProfileNames=Kick.abilmelee;;", "AbilityProfileNames=;;");
    let i = import(&none).unwrap();
    assert!(i.scenario.world.as_ref().unwrap().pushers.is_empty());
    assert!(i.notes.iter().any(|n| n.contains("Push has no ability and is left out")), "{:?}", i.notes);
    // With no pusher the targets are as they were: still, with no motion.
    assert_eq!(i.scenario.world.as_ref().unwrap().motion, None);
    // Its spawn point is still the helper's, not the player's start.
    assert!((i.scenario.world.as_ref().unwrap().spawns[0].pos[2] - -10.0).abs() < 1e-4);

    let nothing = with("FlatKnockbackHorizontal=300.0\nFlatKnockbackVertical=100.0", "FlatKnockbackHorizontal=0.0");
    let i = import(&nothing).unwrap();
    assert!(i.scenario.world.as_ref().unwrap().pushers.is_empty());
    assert!(i.notes.iter().any(|n| n.contains("pushes by nothing and is left out")), "{:?}", i.notes);
    // A helper that is refused on other grounds isn't quietly dropped for pushing nothing.
    let both = nothing.replace("HurtboxDamage=0.0", "HurtboxDamage=2.0");
    assert!(reasons(&both).iter().any(|g| g.detail.contains("HurtboxDamage")));
}

/// The n-th helper slot with a character takes the n-th spawn point that names it, in
/// file order; with fewer points than helpers the file is refused.
#[test]
fn helpers_take_their_spawn_points_in_order() {
    let two = |points: &str| {
        with("AddedBots=Push.bot;Dot.bot;Dot.bot", "AddedBots=Push.bot;Push.bot;Dot.bot")
            .replace("BotTeams=1;2;2", "BotTeams=1;1;2")
            .replace("\n]}", &format!("{points}\n]}}"))
    };
    let point = |x: i32, teams: u32| {
        format!(
            ",\n {{\"type\": \"gameObject\", \"name\": \"SpawnPoint\", \"location\": \"{x}, 0, 0\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 1, 1\", \"properties\": [{{\"name\": \"TeamMask\", \"value\": {teams}}}, {{\"name\": \"PermittedCharacterProfiles\", \"value\": \"Push Orb\"}}]}}"
        )
    };
    // One point isn't enough for two helpers.
    let r = reasons(&two(""));
    assert!(r.iter().any(|g| g.feature == "the map has no spawn point a helper bot can use"), "{r:?}");
    // A point only the opponents may use doesn't count.
    assert!(!reasons(&two(&point(900, 2))).is_empty());
    // Two do: the first helper takes the first point (500), the second the next (900).
    let w = import(&two(&point(900, 1))).unwrap().scenario.world.unwrap();
    let z: Vec<f32> = w.pushers.iter().map(|p| p.pos[2]).collect();
    assert!((z[0] - -4.8).abs() < 1e-4 && (z[1] - -9.0).abs() < 1e-4, "{z:?}");
    // Neither point is a target's, and neither is the player's start (it's at 0).
    assert_eq!(w.spawns.len(), 1);
}

/// The player's start skips a spawn point that names a helper's character and not the
/// player's, whatever the order in the file; one that names both is still usable.
#[test]
fn the_players_start_skips_a_helpers_spawn_point() {
    let w = import(HELPER_SCE).unwrap().scenario.world.unwrap();
    // The player's eye is at Unreal x = 0, so the helper (x = 500) is 5 m ahead.
    assert!((w.pushers[0].pos[2] - -4.8).abs() < 1e-4);
    // Name the player's character on the helper's point too, and it's the start: the
    // helper is then at the player's eye (and the map turns to face as the point does).
    let both = with("\"value\": \"Push Orb\"", "\"value\": \"Push Orb, Gunner\"");
    let w = import(&both).unwrap().scenario.world.unwrap();
    let pos = Vec3::from_array(w.pushers[0].pos);
    assert!(pos.distance(Vec3::new(0.2, 0.02, 0.0)) < 1e-3, "{pos}");
}

/// A bot with `NoDodging` (or one that doesn't move) must do nothing by itself once
/// the file has helpers: no jumps, no jetpack, no dodges and no abilities, whatever
/// its character allows. It only falls, and is kicked.
#[test]
fn a_no_dodging_target_only_falls_and_is_kicked() {
    let text = with("MaxSpeed=0.0\nMainBBType=Spheroid", "MaxSpeed=800.0\nJumpVelocityMax=600.0\nHasJetpack=true\nMainBBType=Spheroid")
        .replace("CharacterProfile=Dot Ball", "CharacterProfile=Dot Ball\nNoDodging=true\nDodgeProfileNames=Hop\nStandStillUntilHurt=true")
        .replace("AddedBots=Push.bot;Dot.bot;Dot.bot", "AddedBots=Push.bot;Dot.bot")
        .replace("BotTeams=1;2;2", "BotTeams=1;2")
        .replace(
            "[Map Data]",
            "[Dodge Profile]\nName=Hop\nJumpFrequency=1.0\nMinJumpTime=0.1\nMaxJumpTime=0.2\nToggleLeftRight=true\nToggleForwardBack=true\n[Map Data]",
        );
    let mut s = import(&text).unwrap().scenario;
    let w = s.world.as_mut().unwrap();
    let m = w.motion.clone().unwrap();
    assert!(m.dodges.is_empty() && m.jetpack.is_none() && m.abilities.is_none() && !m.still_until_hurt);
    assert_eq!(m.max_speed, 0.0);
    // The helper can't reach it; the floor is far below, so it just falls.
    w.pushers[0].radius = 0.01;
    s.timing.countdown_s = 0.0;
    let mut sim = Sim::new(s.clone(), 3);
    let (start, dt) = (sim.targets()[0].pos, 1.0 / 120.0);
    let idle = Input { aim: Vec3::Y, fire_pressed: false, fire_held: false };
    for _ in 0..240 {
        sim.step(dt, idle);
    }
    let p = sim.targets()[0].pos;
    assert!((p.x - start.x).abs() < 1e-4 && (p.z - start.z).abs() < 1e-4, "no sideways movement: {start} to {p}");
    // It fell: 2 s at 8 m/s² is 16 m, less the terminal speed's limit; at least well below.
    assert!(p.y < start.y - 5.0, "it falls: {start} to {p}");
    // And with the helper in reach it is kicked: a pusher right at the target.
    let mut kicked = s;
    let w = kicked.world.as_mut().unwrap();
    w.motion.as_mut().unwrap().gravity = 0.0;
    w.pushers[0].pos = [start.x - 0.5, start.y, start.z];
    w.pushers[0].facing = [1.0, 0.0, 0.0];
    w.pushers[0].radius = 1.0;
    w.pushers[0].lift = 0.0;
    w.pushers[0].needs_sight = false;
    let mut sim = Sim::new(kicked, 3);
    for _ in 0..120 {
        sim.step(dt, idle);
    }
    assert!(sim.targets()[0].pos.x - start.x > 0.5, "kicked away from the helper: {}", sim.targets()[0].pos);
}

/// A target character with `InvincibleBots` never dies, even when the scenario's own
/// switch is off: on the world with one kind of bot, and on each kind of several.
#[test]
fn a_characters_invincible_bots_makes_its_bots_invincible() {
    let one = with(
        "MaxHealth=1.0\nMaxSpeed=0.0\nMainBBType=Spheroid",
        "MaxHealth=1.0\nMaxSpeed=0.0\nInvincibleBots=true\nMainBBType=Spheroid",
    );
    let w = import(&one).unwrap().scenario.world.unwrap();
    assert!(w.invincible);
    assert!(!import(HELPER_SCE).unwrap().scenario.world.unwrap().invincible);
    // The scenario's own flag still applies to every bot.
    let own = with("ScorePerKill=1.0", "ScorePerKill=1.0\nInvincibleBots=true");
    assert!(import(&own).unwrap().scenario.world.unwrap().invincible);
    // Several kinds: only the kind whose character says so.
    let two = with("AddedBots=Push.bot;Dot.bot;Dot.bot", "AddedBots=Push.bot;Dot.bot;Big.bot")
        .replace(
            "[Map Data]",
            "[Bot Profile]\nName=Big\nCharacterProfile=Big Ball\nUseWeapons=false\n\
             [Character Profile]\nName=Big Ball\nMaxHealth=1.0\nMaxSpeed=0.0\nInvincibleBots=true\nMainBBType=Spheroid\nMainBBRadius=50.0\nMainBBHeight=100.0\n\
             [Map Data]",
        );
    let w = import(&two).unwrap().scenario.world.unwrap();
    assert!(!w.invincible, "the scenario's own flag is off");
    let flags: Vec<bool> = w.kinds.iter().map(|k| k.invincible).collect();
    assert_eq!(flags, vec![false, true]);
    // Two characters alike in everything but this are two kinds.
    let alike =
        two.replace("MainBBRadius=50.0", "MainBBRadius=20.0").replace("MainBBHeight=100.0", "MainBBHeight=40.0");
    assert_eq!(import(&alike).unwrap().scenario.world.unwrap().kinds.len(), 2);
}

/// Adding helper bots to a file changes nothing about how the file looks without
/// them: with the helper left out, the world is what it was before this feature.
#[test]
fn a_world_without_pushers_serialises_as_before() {
    let none = with("AbilityProfileNames=Kick.abilmelee;;", "AbilityProfileNames=;;");
    let s = import(&none).unwrap().scenario;
    let text = s.to_toml();
    assert!(!text.contains("pushers") && !text.contains("invincible"), "{text}");
}
