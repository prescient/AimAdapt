//! Tests for the bots that blow themselves up (plan K8a). The base text is the made-up Short Fuse fixture: a
//! still ball of 30 health with one weapon ability whose weapon is a slow projectile that goes off 0.04 s after
//! the shot inside the ball and takes 20 off it. Each refusal changes one key of it, and says why.

use aimadapt_sim::motion::Fuse;

use super::super::*;
use crate::settings::Section;

const FUSE: &str = include_str!("../../../tests/fixtures/AimAdapt Test - Short Fuse.sce");

fn convert_text(text: &str) -> Result<Imported, Vec<Gap>> {
    convert(&Sce::parse(text), "fuse.sce", "0")
}

/// The reasons the file is refused for, as they read.
fn refused(text: &str) -> Vec<String> {
    convert_text(text).err().unwrap_or_default().iter().map(ToString::to_string).collect()
}

/// Changes a line of the base text; the line has to be there once.
fn with(old: &str, new: &str) -> String {
    assert_eq!(FUSE.matches(old).count(), 1, "{old}");
    FUSE.replacen(old, new, 1)
}

const WHY: &str = "weapon abilities (Short Fuse: ";

/// The refusal that one change to the base text gives, in full.
#[track_caller]
fn assert_refused(text: &str, why: &str) {
    assert_eq!(refused(text), vec![format!("{WHY}{why})")], "{why}");
}

/// The accepted shape: a still bot (it has a motion, which only runs its abilities) with the one weapon
/// ability as a fuse. Its tick, window, block and charge are read as a movement ability's are, with the key
/// defaults measured in KovaaK's (a tick every 0.1 s, always pressing, a block of 1 s).
#[test]
fn the_accepted_shape_imports_as_a_bot_that_stays_put_with_a_fuse() {
    let i = convert_text(FUSE).unwrap();
    assert!(i.scenario.problems().is_empty(), "{:?}", i.scenario.problems());
    let w = i.scenario.world.as_ref().unwrap();
    assert!(w.kinds.is_empty());
    let m = w.motion.as_ref().expect("a fuse bot has a motion");
    assert!(m.stays_put);
    assert_eq!((m.max_speed, m.gravity > 0.0, m.dodges.len(), m.still_until_hurt), (0.0, true, 0, false));
    assert!(m.flight.is_none() && m.jetpack.is_none());
    let ab = m.abilities.as_ref().unwrap();
    assert_eq!((ab.press_every_s, ab.chance, ab.start_block_s, ab.global_cooldown_s), (0.1, 1.0, 1.0, 0.0));
    assert!(ab.sees_through_walls && ab.aim_lift == 0.0);
    assert_eq!(ab.list.len(), 1);
    let a = &ab.list[0];
    assert_eq!(a.fuse, Some(Fuse { shot_every_s: 0.7, burn_s: 0.04, damage: 20.0 }));
    assert_eq!((a.max_charges, a.spawn_charges, a.charge_s, a.delay_s, a.reuse_s), (1, 1, 0.05, 0.0, 0.02));
    assert_eq!((a.speed, a.up_speed, a.duration_s, a.health_cost), (0.0, 0.0, 0.0, 0.0));
    assert!(a.reaction.is_none() && !a.blocks_movement && !a.vertical);
    assert_eq!((a.in_combat, a.out_of_combat, a.on_ground, a.in_air), (true, true, true, true));
    assert_eq!(i.scenario.targets.health, 30.0);
    assert!((i.scenario.targets.respawn_delay_s - 1.5).abs() < 1e-6);
}

/// The damage of a blast is the explosion's damage times `SelfDamageMultiplier` (missing reads as 1), the times
/// and the gates and timers of the ability are read as a movement ability's, and times are cut to an hour.
#[test]
fn a_fuses_damage_times_and_the_abilitys_own_settings_come_through() {
    let fuse = |text: &str| {
        let m = convert_text(text).unwrap().scenario.world.unwrap().motion.unwrap();
        m.abilities.unwrap().list[0].clone()
    };
    let two = with("SelfDamageMultiplier=1.0", "SelfDamageMultiplier=0.5");
    assert_eq!(fuse(&two).fuse.unwrap().damage, 10.0);
    let missing = with("SelfDamageMultiplier=1.0\n", "");
    assert_eq!(fuse(&missing).fuse.unwrap().damage, 20.0);
    let slow = with("TimeBetweenShots=0.7", "TimeBetweenShots=90000.0");
    assert_eq!(fuse(&slow).fuse.unwrap().shot_every_s, 3600.0);
    let own = with("MaxCharges=1.0", "MaxCharges=3.0")
        .replace("DelayAfterUse=0.0", "DelayAfterUse=0.25")
        .replace("AIMaxTargFOV=360.0", "AIMaxTargFOV=90.0")
        .replace("AIMaxTargDist=100000.0", "AIMaxTargDist=5000.0");
    let a = fuse(&own);
    assert_eq!((a.max_charges, a.delay_s, a.max_angle_deg, a.max_distance), (3, 0.25, 90.0, 50.0));
    // The bot's key and the character's block and cooldown come through too.
    let keys = with("SeeThroughWalls=true", "SeeThroughWalls=false\nAbilityUseTimer=0.5\nUseAbilityFrequency=0.5")
        .replace(
            "BlockSelfDamage=false",
            "BlockSelfDamage=false\nBlockAbilityOnStartDuration=2.5\nAbilityGlobalCooldown=0.3",
        );
    let m = convert_text(&keys).unwrap().scenario.world.unwrap().motion.unwrap();
    let ab = m.abilities.unwrap();
    assert_eq!((ab.press_every_s, ab.chance, ab.start_block_s, ab.global_cooldown_s), (0.5, 0.5, 2.5, 0.3));
    assert!(!ab.sees_through_walls);
}

/// Every refusal for a bot or its character, one changed key each, names the ability, the key and its value.
#[test]
fn a_bot_that_isnt_a_fuse_is_refused_with_the_reason() {
    assert_refused(&with("UseWeapons=true", "UseWeapons=false"), "its bot has UseWeapons=false");
    assert_refused(&with("UseWeapons=true\n", ""), "its bot doesn't set UseWeapons=true");
    assert_refused(&with("NoAiming=false", "NoAiming=true"), "its bot has NoAiming=true");
    // A weapon of its own (and a player the bot's gun can't hurt, so only this reason shows).
    let armed = with("NoAiming=false", "NoAiming=false\nWeaponsProfileNames=Tapper;;;").replacen(
        "MapScale=1.0",
        "MapScale=1.0\nInvinciblePlayer=true",
        1,
    );
    assert_refused(&armed, "its bot carries a weapon of its own");
    let on_character = with("MinRespawnDelay=1.5", "MinRespawnDelay=1.5\nWeaponProfileNames=Tapper;;;").replacen(
        "MapScale=1.0",
        "MapScale=1.0\nInvinciblePlayer=true",
        1,
    );
    assert_refused(&on_character, "its bot carries a weapon of its own");
    assert_refused(&with("MaxSpeed=0.0\nMainBBType", "MaxSpeed=300.0\nMainBBType"), "its bot moves by itself");
    assert_refused(&with("MainBBHasHead=false", "MainBBHasHead=false\nGravity=1.0"), "its character has Gravity=1.0");
    assert_refused(&with("BlockSelfDamage=false", "BlockSelfDamage=true"), "its character has BlockSelfDamage=true");
    // Plan K9: a head on a bot that blows itself up wasn't measured.
    assert_refused(
        &with(
            "MainBBHasHead=false",
            "MainBBHasHead=true
MainBBHeadRadius=10.0",
        ),
        "its character has a head",
    );
    assert_refused(
        &with("BlockSelfDamage=false", "BlockSelfDamage=false\nHealthRegenPerSec=5.0"),
        "its character has HealthRegenPerSec=5.0",
    );
    assert_refused(
        &with("BlockSelfDamage=false", "BlockSelfDamage=false\nInvincibleBots=true"),
        "its bot can't die (InvincibleBots=true)",
    );
    assert_refused(
        &with("MapScale=1.0", "MapScale=1.0\nInvincibleBots=true"),
        "its bot can't die (InvincibleBots=true)",
    );
    // `NoDodging` keeps a bot that has a top speed still, and so it plays.
    let still = with("MaxSpeed=0.0\nMainBBType", "MaxSpeed=300.0\nMainBBType").replacen(
        "NoAiming=false",
        "NoAiming=false\nNoDodging=true",
        1,
    );
    assert!(convert_text(&still).is_ok());
    // A flier with no gravity key hangs there, so it is a bot that stays put.
    let fliers = with("MainBBHasHead=false", "MainBBHasHead=false\nIsFlyer=true\nGravity=1.0");
    assert!(convert_text(&fliers).is_ok(), "{:?}", refused(&fliers));
}

/// A second ability on the character keeps it from being a fuse bot, whatever the other one is, and the
/// refusal says how many it has.
#[test]
fn a_character_with_other_abilities_is_refused() {
    let two = with("AbilityProfileNames=Short Fuse.abilwep;;;", "AbilityProfileNames=Short Fuse.abilwep;Hop.abilmov;;")
        .replace("[Map Data]", "[Movement Ability Profile]\nName=Hop\nMaxCharges=1.0\n[Map Data]");
    assert_refused(&two, "its character has 2 abilities; a fuse bot has this one alone");
    let twice = with(
        "AbilityProfileNames=Short Fuse.abilwep;;;",
        "AbilityProfileNames=Short Fuse.abilwep;Short Fuse.abilwep;;",
    );
    assert_refused(&twice, "its character has 2 abilities; a fuse bot has this one alone");
}

/// Helper bots that push the targets keep a fuse bot from staying put, so it is refused.
#[test]
fn a_fuse_bot_in_a_file_with_helper_bots_is_refused() {
    let helper = with("AddedBots=Fuse.bot\nBotTeams=2", "AddedBots=Push.bot;Fuse.bot\nBotTeams=1;2")
        .replace(
            "[Map Data]",
            "[Bot Profile]\nName=Push\nCharacterProfile=Push Orb\nNoAiming=true\nNoDodging=true\nUseWeapons=false\n\
             UseAbilityFrequency=1.0\nAbilityUseTimer=0.01\n\
             [Character Profile]\nName=Push Orb\nMaxHealth=1.0\nMaxSpeed=0.0\nMainBBType=Cuboid\nMainBBRadius=1.0\n\
             MainBBHeight=2.0\nAbilityProfileNames=Kick.abilmelee;;\n\
             [Melee Ability Profile]\nName=Kick\nHurtboxRadius=150.0\nHurtboxDamage=0.0\nFlatKnockbackHorizontal=300.0\n\
             FlatKnockbackVertical=100.0\nAbilityDuration=0.01\nAIReuseTimer=0.01\nChargeTimer=0.01\nDelayAfterUse=0.01\n\
             MaxCharges=1.0\nAIUseInCombat=true\nAIMaxTargFOV=360.0\n[Map Data]",
        );
    let why = refused(&helper);
    assert!(why.contains(&format!("{WHY}helper bots push the targets)")), "{why:?}");
}

/// Every refusal for the ability's own settings.
#[test]
fn an_ability_that_isnt_a_fuse_is_refused_with_the_reason() {
    assert_refused(&with("AIDamageReaction=false", "AIDamageReaction=true"), "AIDamageReaction=true");
    for key in ["AIUseInCombat", "AIUseOutOfCombat", "AIUseOnGround", "AIUseInAir"] {
        assert_refused(&with(&format!("{key}=true"), &format!("{key}=false")), &format!("{key}=false"));
    }
    for (from, to) in [
        ("AIMinSelfHealth=0.0", "AIMinSelfHealth=10.0"),
        ("AIMinTargHealth=0.0", "AIMinTargHealth=10.0"),
        ("AIMaxSelfHealth=100.0", "AIMaxSelfHealth=90.0"),
        ("AIMaxTargHealth=100.0", "AIMaxTargHealth=90.0"),
    ] {
        assert_refused(&with(from, to), to);
    }
    // Missing gates and missing `AIUse` keys read as open.
    let open = FUSE.lines().filter(|l| !l.starts_with("AIMinSelf") && !l.starts_with("AIUseInAir")).collect::<Vec<_>>();
    assert!(convert_text(&open.join("\n")).is_ok());
    assert_refused(
        &with("WeaponProfile=Slow Charge", "WeaponProfile=Nothing"),
        "its WeaponProfile=Nothing is no weapon profile in the file",
    );
    assert_refused(&with("WeaponProfile=Slow Charge\n", ""), "it has no WeaponProfile");
}

/// Every refusal for the weapon of the ability.
#[test]
fn a_weapon_that_isnt_a_fuse_is_refused_with_the_reason() {
    let weapon = |old: &str, new: &str, why: &str| assert_refused(&with(old, new), &format!("its weapon has {why}"));
    weapon("Type=Projectile", "Type=Hitscan", "Type=Hitscan");
    weapon("Explosive=true", "Explosive=false", "Explosive=false");
    weapon("Category=SemiAuto\nIsBurstWeapon", "Category=FullyAuto\nIsBurstWeapon", "Category=FullyAuto");
    weapon("IsBurstWeapon=false", "IsBurstWeapon=true", "IsBurstWeapon=true");
    for key in ["ExplodesOnContactWithEnemy", "ExplodesOnContactWithWorld", "ExplodesOnNextAttack"] {
        weapon(&format!("{key}=false"), &format!("{key}=true"), &format!("{key}=true"));
    }
    weapon("ShotsPerClick=1", "ShotsPerClick=2", "ShotsPerClick=2");
    weapon("DelayBeforeShot=0.0", "DelayBeforeShot=0.1", "DelayBeforeShot=0.1");
    weapon("MagazineMax=0", "MagazineMax=6", "MagazineMax=6");
    weapon("GravityScale=0.0", "GravityScale=1.0", "GravityScale=1.0; it must be 0");
    weapon("GravityScale=0.0\n", "", "no GravityScale; it must be 0");
    weapon("TimeBetweenShots=0.7", "TimeBetweenShots=0.0", "TimeBetweenShots=0.0");
    let delay = |to: &str| {
        let both =
            format!("DelayAfterSpawn={to} and MaxTravelTime=0.5; the delay must be above 0 and below the travel time");
        weapon("DelayAfterSpawn=0.04", &format!("DelayAfterSpawn={to}"), &both);
    };
    delay("0.0");
    delay("0.5");
    delay("0.7");
    weapon("DamageAtEdge=20.0", "DamageAtEdge=10.0", "DamageAtCenter=20.0 and DamageAtEdge=10.0, which differ");
    weapon(
        "SelfDamageMultiplier=1.0",
        "SelfDamageMultiplier=0.0",
        "DamageAtCenter=20.0 and SelfDamageMultiplier=0.0, so a blast does its bot no damage",
    );
}

/// The blast has to stay inside the bot: how far the projectile gets before it goes off, plus the blast's
/// radius, is at most the body's nearest side (a ball's radius, 60 units here).
#[test]
fn a_blast_that_could_reach_outside_the_bot_is_refused() {
    let out = |reach: &str| {
        format!("its weapon's blast reaches {reach} units from the bot's middle, outside its body (60.0 units)")
    };
    // The base reaches 0.2 * 0.04 + 0 + 5 = 5.008 units.
    assert!(convert_text(FUSE).is_ok());
    // A bigger blast, 65.0 at most 60: refused; exactly the body's size plays.
    assert_refused(&with("Radius=5.0", "Radius=70.0"), &out("70.0"));
    assert!(convert_text(&with("Radius=5.0", "Radius=59.9")).is_ok());
    // A faster projectile: 1000 units/s for 0.04 s is 40, plus 25: 65.
    let fast = with("MuzzleVelocityMax=X=0.200 Y=0.000 Z=0.000", "MuzzleVelocityMax=X=0.000 Y=1000.000 Z=0.000")
        .replace("Radius=5.0", "Radius=25.0");
    assert_refused(&fast, &out("65.0"));
    // Starting away from the middle counts too: 50 units out and a blast of 5.
    assert!(
        convert_text(&with("OriginOffset=X=0.000 Y=0.000 Z=0.000", "OriginOffset=X=50.000 Y=0.000 Z=0.000")).is_ok()
    );
    assert_refused(
        &with("OriginOffset=X=0.000 Y=0.000 Z=0.000", "OriginOffset=X=0.000 Y=0.000 Z=57.000"),
        &out("62.0"),
    );
    // A weapon that doesn't say how fast it goes or where it starts can't be worked out.
    let unknown = |old: &str, key: &str| {
        let text = with(old, "");
        let want = format!("its weapon has no {key}; the blast's reach can't be worked out");
        assert_eq!(refused(&text), vec![format!("{WHY}{want})")]);
    };
    unknown("MuzzleVelocityMin=X=0.200 Y=0.000 Z=0.000\n", "MuzzleVelocityMin");
    unknown("MuzzleVelocityMax=X=0.200 Y=0.000 Z=0.000\n", "MuzzleVelocityMax");
    unknown("OriginOffset=X=0.000 Y=0.000 Z=0.000\n", "OriginOffset");
    unknown("Radius=5.0\n", "Radius");
}

/// A body that isn't a ball is limited by the smaller of its radius and half its height.
#[test]
fn a_bodys_nearest_side_is_its_radius_or_half_its_height() {
    let mut weapon = Section::new("Weapon Profile");
    for (k, v) in [
        ("MuzzleVelocityMin", "X=0.000 Y=0.000 Z=0.000"),
        ("MuzzleVelocityMax", "X=0.000 Y=0.000 Z=0.000"),
        ("OriginOffset", "X=0.000 Y=0.000 Z=0.000"),
        ("Radius", "55.0"),
    ] {
        weapon.push(k, v);
    }
    let body = |kind: &str, radius: &str, height: &str| {
        let mut ch = Section::new("Character Profile");
        for (k, v) in [("MainBBType", kind), ("MainBBRadius", radius), ("MainBBHeight", height)] {
            ch.push(k, v);
        }
        ch
    };
    // Whatever the height, a spheroid is a ball of its radius.
    assert_eq!(super::reach_refusal(&weapon, &body("Spheroid", "60.0", "10.0"), 0.04), None);
    assert!(super::reach_refusal(&weapon, &body("Spheroid", "50.0", "400.0"), 0.04).is_some());
    // A capsule or a box: half its height when that is less than its radius.
    assert_eq!(super::reach_refusal(&weapon, &body("Cylindrical", "100.0", "120.0"), 0.04), None);
    let why = super::reach_refusal(&weapon, &body("Cylindrical", "100.0", "100.0"), 0.04).unwrap();
    assert!(why.ends_with("(50.0 units)"), "{why}");
    let why = super::reach_refusal(&weapon, &body("Cuboid", "40.0", "300.0"), 0.04).unwrap();
    assert!(why.ends_with("(40.0 units)"), "{why}");
}

/// A weapon ability on a bot on the player's team is refused, with the reason that it is a helper's.
#[test]
fn a_helper_with_a_weapon_ability_is_refused_as_one_on_the_players_team() {
    let helper = with("BotTeams=2", "BotTeams=1");
    let why = refused(&helper);
    assert!(why.contains(&format!("{WHY}a bot on the player's team)")), "{why:?}");
}

/// A fuse bot gets one note about what wasn't checked, once, and a second when the run has bot lives. A
/// scenario with no fuse gets neither.
#[test]
fn a_fuse_bot_adds_its_two_notes() {
    let notes = |text: &str| convert_text(text).unwrap().notes;
    let plain = notes(FUSE);
    let about = |n: &&String| n.contains("blows itself up");
    assert_eq!(plain.iter().filter(about).count(), 1, "{plain:?}");
    let note = plain.iter().find(about).unwrap();
    assert!(note.contains("doesn't take") && note.contains("charge") && note.contains("dropped"), "{note}");
    assert!(note.contains("checked against KovaaK's") && !note.contains("  "), "{note}");
    assert!(plain.iter().all(|n| !n.contains("move by their dodge") && !n.contains("lives")), "{plain:?}");

    let lives = notes(&with("ScorePerKill=10.0", "ScorePerKill=10.0\nBotMaxLives=3"));
    assert_eq!(lives.iter().filter(about).count(), 2, "{lives:?}");
    let second = lives.iter().find(|n| n.contains("bot lives")).expect("a note about bot lives");
    assert!(second.contains("timeout") && second.contains("uses up no life"), "{second}");
    assert!(second.contains("checked against KovaaK's") && !second.contains("  "), "{second}");
    // Each once, however many fuse bots there are.
    let two = with("AddedBots=Fuse.bot", "AddedBots=Fuse.bot;Fuse.bot").replace("BotTeams=2", "BotTeams=2;2");
    let doubled = notes(&two);
    assert_eq!(doubled.iter().filter(about).count(), 1, "{doubled:?}");
}
