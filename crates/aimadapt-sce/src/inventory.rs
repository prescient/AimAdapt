//! Which features a scenario uses that the trainer won't model in its first version
//! (BACKLOG section 9, phases 2 and 3). A scenario with no gaps is "covered": once
//! those phases land it should play faithfully. Checks are strict on purpose; a
//! feature leaves this list only when it's in those phases' plan.
//!
//! Covered by the plan, so not listed: hitscan (including a small `HitscanRadius`),
//! sphere, cylinder and box hitboxes, health, regen, bot movement and dodging,
//! rotation profiles, and bots' melee (knockback) and movement (dash) abilities.

use std::collections::BTreeMap;

use crate::convert::fuse;
use crate::scenario::{BotChoice, Sce};
use crate::settings::Section;

const WEAPON_ABILITY: &str = crate::scenario::ABILITY_KINDS[0];
const MELEE_ABILITY: &str = crate::scenario::ABILITY_KINDS[1];

/// Hitbox shapes the world sim will have.
const SHAPES: &[&str] = &["Spheroid", "Cylindrical", "Cuboid"];

/// Why the trainer can't play a scenario: a feature, with what in the file uses
/// it. [`gaps`] lists the ones beyond the plan; the converter adds the ones the
/// plan covers but the trainer doesn't play yet, and problems with the file, so
/// every reason is one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    pub feature: &'static str,
    pub detail: String,
    /// Outside the plan (BACKLOG section 9), so it won't play soon.
    pub beyond_plan: bool,
}

impl Gap {
    /// A reason the plan covers (or a problem with the file).
    pub fn planned(feature: &'static str, detail: impl Into<String>) -> Gap {
        Gap { feature, detail: detail.into(), beyond_plan: false }
    }
}

impl std::fmt::Display for Gap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.detail.is_empty() { f.write_str(self.feature) } else { write!(f, "{} ({})", self.feature, self.detail) }
    }
}

/// A scenario's gaps; empty means covered.
pub fn gaps(sce: &Sce) -> Vec<Gap> {
    let mut out: Vec<Gap> = Vec::new();
    let mut gap = |feature: &'static str, detail: String| {
        if !out.iter().any(|g| g.feature == feature && g.detail == detail) {
            out.push(Gap { feature, detail, beyond_plan: true });
        }
    };
    let top = &sce.settings.top;
    let r = sce.resolve();

    match &sce.map {
        Err(e) => gap("map", e.to_string()),
        Ok(m) => {
            if !m.other.is_empty() {
                gap("map objects", counted(m.other.iter().map(String::as_str)));
            }
            let solid_props: Vec<&str> = m.props.iter().filter(|p| p.collides).map(|p| p.name.as_str()).collect();
            if !solid_props.is_empty() {
                gap("solid props", counted(solid_props.into_iter()));
            }
            if m.spawns.iter().any(|s| !s.path.is_empty()) || !m.waypoints.is_empty() {
                gap("waypoint paths", format!("{} waypoints", m.waypoints.len()));
            }
            if m.solids.is_empty() && m.spawns.is_empty() {
                gap("map", "the map has no geometry or spawns".into());
            }
        }
    }
    for m in &r.missing {
        gap("missing profile", m.clone());
    }

    // Scoring and rules.
    for key in ["ScorePerDistance", "MaxDistanceTraveledScore"] {
        if top.f32_or(key, 0.0) != 0.0 {
            gap("distance scoring", key.into());
        }
    }
    if top.is("MBSEnable") {
        gap("MBS scoring", "MBSEnable".into());
    }
    if !matches!(top.str("DistanceScoreCondition"), "" | "None") {
        gap("distance scoring", format!("DistanceScoreCondition={}", top.str("DistanceScoreCondition")));
    }
    for key in ["IsTimeDilationActive", "IsTargetSizeActive"] {
        if top.is(key) {
            gap("KovaaK's dynamic difficulty", key.into());
        }
    }
    // Only the scenario's own switch counts here. Whether the player character's
    // `InvinciblePlayer` also protects the player from a bot that fires wasn't measured,
    // so a file with such a bot and only the character's switch on stays refused.
    let player_can_die = !top.is("InvinciblePlayer");

    // The player and their weapon.
    match r.weapon {
        None => gap("weapon", "the player has no weapon".into()),
        Some(w) => weapon_gaps(w, &mut gap),
    }
    if let Some(p) = r.player {
        if p.f32_or("MaxSpeed", 0.0) > 0.0 {
            gap("player movement", format!("MaxSpeed={}", p.str("MaxSpeed")));
        }
        if p.is("ThirdPersonCamera") {
            gap("third-person camera", p.name().into());
        }
        // `InvinciblePlayer` on the player's character is no gap: nothing in the trainer
        // hurts the player. `InvincibleBots` there is still refused, because what it does
        // on the player's character wasn't measured. On a bot's own character it plays.
        if p.is("InvincibleBots") {
            gap("per-character invincibility", format!("{}: InvincibleBots=true on the player's character", p.name()));
        }
    }
    if !r.player_abilities.is_empty() {
        gap("player abilities", counted(r.player_abilities.iter().map(|a| a.kind.as_str())));
    }

    // The bots. A helper (on the player's team) can't be shot, so how it can be hit
    // doesn't matter.
    let player_team = crate::scenario::player_team(top);
    let is_melee = |a: &&Section| a.kind.eq_ignore_ascii_case(MELEE_ABILITY);
    let scene = fuse::Scene {
        settings: &sce.settings,
        invincible: top.is("InvincibleBots"),
        pushed: (r.bots.iter().filter(|s| s.is_helper(player_team)))
            .any(|s| s.choices.iter().any(|c| c.abilities.iter().any(is_melee))),
    };
    for slot in &r.bots {
        let helper = slot.is_helper(player_team);
        for c in &slot.choices {
            if let Some(ch) = c.character {
                let shape = ch.str("MainBBType");
                if !helper && !SHAPES.contains(&shape) {
                    gap("hitbox shape", shape.into());
                }
                if !helper && ch.is("MainBBHasHead") {
                    gap("heads", ch.name().into());
                }
                if !helper && ch.is("HeadshotOnly") {
                    gap("headshots only", ch.name().into());
                }
                if !matches!(ch.str("MovementType"), "" | "Base") {
                    gap("movement type", ch.str("MovementType").into());
                }
            }
            if fires(c) && player_can_die {
                gap("bots shoot back", c.bot.name().into());
            }
            // Melee and movement abilities are how bots push each other apart and
            // dash (Pasu-style and ground benchmarks), so phase 3 covers them. Weapon
            // abilities aren't planned, except the one shape that blows its own bot up.
            weapon_ability_gaps(c, helper, &scene, &mut gap);
            for (d, _) in &c.dodges {
                if !matches!(d.str("WaypointLogic"), "" | "Ignore") {
                    gap("waypoint paths", format!("WaypointLogic={}", d.str("WaypointLogic")));
                }
                if !matches!(d.str("LOSReactType"), "" | "None") {
                    gap("line-of-sight reactions", format!("LOSReactType={}", d.str("LOSReactType")));
                }
            }
        }
    }
    out
}

/// The weapon abilities of a bot that the trainer can't play, each with the first reason it can't: a helper's
/// never plays, and a target's does only in the shape of the fuse (see [`fuse`]).
fn weapon_ability_gaps(c: &BotChoice, helper: bool, scene: &fuse::Scene, gap: &mut impl FnMut(&'static str, String)) {
    for a in c.abilities.iter().filter(|a| a.kind.eq_ignore_ascii_case(WEAPON_ABILITY)) {
        let why = if helper { Some("a bot on the player's team".to_string()) } else { fuse::refusal(scene, c, a) };
        if let Some(why) = why {
            gap("weapon abilities", format!("{}: {why}", a.name()));
        }
    }
}

/// Whether a bot has a weapon it is set to use: `UseWeapons` is on and a weapon that
/// isn't empty is on the bot's `WeaponsProfileNames` or its character's
/// `WeaponProfileNames` (an empty `WeaponsProfileNames=;;;` list is no weapon).
pub(crate) fn armed(c: &crate::scenario::BotChoice) -> bool {
    c.bot.is("UseWeapons")
        && !c
            .bot
            .list("WeaponsProfileNames")
            .iter()
            .chain(c.character.map(|ch| ch.list("WeaponProfileNames")).unwrap_or_default().iter())
            .all(|w| w.is_empty())
}

/// Whether a bot shoots at the player. In KovaaK's 3.9.13 an armed bot that is told not
/// to aim (`NoAiming`) never fired, and one that aims fired all the time.
fn fires(c: &crate::scenario::BotChoice) -> bool {
    armed(c) && !c.bot.is("NoAiming")
}

fn weapon_gaps(w: &Section, gap: &mut impl FnMut(&'static str, String)) {
    if !matches!(w.str("Type"), "" | "Hitscan") {
        gap("projectile weapon", format!("Type={}", w.str("Type")));
    }
    if w.is("Explosive") {
        gap("explosive weapon", w.name().into());
    }
    if w.f32_or("AAMode", 0.0) != 0.0 || w.is("TriggerBotEnabled") {
        gap("aim assist", w.name().into());
    }
    let recoil = ["MaxRecoilUp", "MinRecoilUp", "MaxRecoilHoriz", "MinRecoilHoriz", "AimPunchAmount"]
        .iter()
        .any(|k| w.f32_or(k, 0.0) != 0.0);
    if recoil || w.is("UsePerShotRecoil") {
        gap("recoil", w.name().into());
    }
    if w.f32_or("ShotsPerClick", 1.0) > 1.0 {
        gap("several pellets per shot", format!("ShotsPerClick={}", w.str("ShotsPerClick")));
    }
    if w.is("IsBurstWeapon") {
        gap("burst fire", w.name().into());
    }
    if w.is("CanAimDownSight") {
        gap("aim down sights", w.name().into());
    }
    if !matches!(w.str("Category"), "" | "FullyAuto" | "SemiAuto") {
        gap("weapon category", w.str("Category").into());
    }
}

/// "Teleporter ×3, Hurt".
fn counted<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let mut n: BTreeMap<&str, usize> = BTreeMap::new();
    for i in items {
        *n.entry(i).or_default() += 1;
    }
    n.into_iter().map(|(k, c)| if c > 1 { format!("{k} ×{c}") } else { k.to_string() }).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "Name=Plain\nPlayerProfile=Hero\nAddedBots=Ball.bot\nInvinciblePlayer=true\n\
        [Character Profile]\nName=Hero\nWeaponProfileNames=Pin;;\nMaxSpeed=0.0\n\
        [Weapon Profile]\nName=Pin\nType=Hitscan\nCategory=SemiAuto\nAAMode=0\n\
        [Bot Profile]\nName=Ball\nCharacterProfile=Orb\nUseWeapons=false\n\
        [Character Profile]\nName=Orb\nMainBBType=Spheroid\nMainBBHasHead=false\n\
        [Map Data]\n{\"objects\": [{\"type\": \"gameObject\", \"name\": \"SpawnPoint\", \"location\": \"0, 0, 0\"}]}";

    fn features(text: &str) -> Vec<&'static str> {
        gaps(&Sce::parse(text)).into_iter().map(|g| g.feature).collect()
    }

    #[test]
    fn a_plain_hitscan_scenario_is_covered() {
        assert_eq!(gaps(&Sce::parse(BASE)), vec![]);
    }

    #[test]
    fn weapon_features_outside_the_plan_are_gaps() {
        assert_eq!(features(&BASE.replace("Type=Hitscan", "Type=Projectile")), vec!["projectile weapon"]);
        assert_eq!(features(&BASE.replace("AAMode=0", "AAMode=2")), vec!["aim assist"]);
        assert_eq!(features(&BASE.replace("Category=SemiAuto", "Category=SemiAuto\nMaxRecoilUp=2.0")), vec!["recoil"]);
        assert_eq!(features(&BASE.replace("WeaponProfileNames=Pin;;", "WeaponProfileNames=;;")), vec!["weapon"]);
    }

    #[test]
    fn bots_shoot_back_only_when_the_player_can_be_hurt() {
        let armed = BASE.replace("UseWeapons=false", "UseWeapons=true\nWeaponsProfileNames=Pin;;");
        assert_eq!(features(&armed), Vec::<&str>::new());
        assert_eq!(
            features(&armed.replace("InvinciblePlayer=true", "InvinciblePlayer=false")),
            vec!["bots shoot back"]
        );
    }

    /// K7: an armed bot told not to aim never fires in KovaaK's, so it is no gap. A bot
    /// that aims, or says nothing about aiming, still is.
    #[test]
    fn armed_bots_that_do_not_aim_never_fire() {
        let on_bot = BASE.replace(
            "UseWeapons=false",
            "UseWeapons=true
WeaponsProfileNames=Pin;;",
        );
        let on_character = BASE.replace("UseWeapons=false", "UseWeapons=true").replace(
            "Name=Orb
",
            "Name=Orb
WeaponProfileNames=Pin;;
",
        );
        for armed in [on_bot, on_character] {
            let hurt = armed.replace("InvinciblePlayer=true", "InvinciblePlayer=false");
            assert_eq!(features(&hurt), vec!["bots shoot back"], "{hurt}");
            let quiet = hurt.replace(
                "UseWeapons=true",
                "UseWeapons=true
NoAiming=true",
            );
            assert_eq!(features(&quiet), Vec::<&str>::new(), "{quiet}");
            let aiming = hurt.replace(
                "UseWeapons=true",
                "UseWeapons=true
NoAiming=false",
            );
            assert_eq!(features(&aiming), vec!["bots shoot back"], "{aiming}");
        }
    }

    /// K7: `InvinciblePlayer` on the player's character is no gap. It doesn't make bots
    /// that fire harmless (not measured), and `InvincibleBots` there is still a gap.
    #[test]
    fn the_players_character_may_be_invincible_but_not_make_bots_invincible() {
        let hurt = BASE.replace("InvinciblePlayer=true", "InvinciblePlayer=false");
        let own = hurt.replace(
            "WeaponProfileNames=Pin;;",
            "WeaponProfileNames=Pin;;
InvinciblePlayer=true",
        );
        assert_eq!(features(&own), Vec::<&str>::new());
        let armed = own.replace(
            "UseWeapons=false",
            "UseWeapons=true
WeaponsProfileNames=Pin;;",
        );
        assert_eq!(features(&armed), vec!["bots shoot back"]);
        let bots = hurt.replace(
            "WeaponProfileNames=Pin;;",
            "WeaponProfileNames=Pin;;
InvincibleBots=true",
        );
        assert_eq!(
            gaps(&Sce::parse(&bots)),
            vec![Gap {
                feature: "per-character invincibility",
                detail: "Hero: InvincibleBots=true on the player's character".into(),
                beyond_plan: true
            }]
        );
    }

    #[test]
    fn heads_player_movement_and_missing_profiles_are_gaps() {
        assert_eq!(features(&BASE.replace("MainBBHasHead=false", "MainBBHasHead=true")), vec!["heads"]);
        assert_eq!(features(&BASE.replace("MaxSpeed=0.0", "MaxSpeed=600")), vec!["player movement"]);
        assert_eq!(features(&BASE.replace("CharacterProfile=Orb", "CharacterProfile=Nobody")), vec!["missing profile"]);
    }

    #[test]
    fn bot_melee_and_movement_abilities_are_covered_but_weapon_ones_are_not() {
        let with = |refs: &str| {
            BASE.replace(
                "MainBBHasHead=false",
                &format!(
                    "MainBBHasHead=false
AbilityProfileNames={refs}"
                ),
            )
            .replace(
                "[Map Data]",
                "[Melee Ability Profile]
Name=Push
[Movement Ability Profile]
Name=Dash
[Weapon Ability Profile]
Name=Zap
[Map Data]",
            )
        };
        assert_eq!(features(&with("Push.abilmelee;Dash.abilmov;;")), Vec::<&str>::new());
        assert_eq!(
            gaps(&Sce::parse(&with("Push.abilmelee;Zap.abilwep;;"))),
            vec![Gap {
                feature: "weapon abilities",
                detail: "Zap: its bot has UseWeapons=false".into(),
                beyond_plan: true
            }]
        );
    }

    #[test]
    fn unmodelled_map_objects_are_listed_with_counts() {
        let text = BASE.replace(
            "\"location\": \"0, 0, 0\"}",
            "\"location\": \"0, 0, 0\"}, {\"type\": \"gameObject\", \"name\": \"Teleporter\"}, {\"type\": \"gameObject\", \"name\": \"Teleporter\"}, {\"type\": \"gameObject\", \"name\": \"Hurt\"}",
        );
        let g = gaps(&Sce::parse(&text));
        assert_eq!(g, vec![Gap { feature: "map objects", detail: "Hurt, Teleporter ×2".into(), beyond_plan: true }]);
    }

    #[test]
    fn kovaaks_own_dynamic_difficulty_is_a_gap() {
        assert_eq!(
            features(&BASE.replace("InvinciblePlayer=true", "InvinciblePlayer=true\nIsTimeDilationActive=true")),
            vec!["KovaaK's dynamic difficulty"]
        );
    }

    /// A helper bot (on the player's team) can't be shot, so its hitbox shape, head and
    /// headshot-only setting don't matter; its other settings still do.
    #[test]
    fn a_helpers_hitbox_is_not_a_gap() {
        let odd = BASE
            .replace(
                "MainBBType=Spheroid",
                "MainBBType=Cone
HeadshotOnly=true",
            )
            .replace("MainBBHasHead=false", "MainBBHasHead=true");
        assert_eq!(features(&odd), vec!["hitbox shape", "heads", "headshots only"]);
        let helper = odd.replace(
            "AddedBots=Ball.bot",
            "AddedBots=Ball.bot
BotTeams=1",
        );
        assert_eq!(features(&helper), Vec::<&str>::new());
        // The same bot on the opponents' team, and a helper that moves strangely.
        assert_eq!(
            features(&odd.replace(
                "AddedBots=Ball.bot",
                "AddedBots=Ball.bot
BotTeams=2"
            ))
            .len(),
            3
        );
        let flying = helper.replace(
            "MainBBType=Cone",
            "MainBBType=Cone
MovementType=Fly",
        );
        assert_eq!(features(&flying), vec!["movement type"]);
        // With the player on team 2 the helper is the one on team 2.
        let other = helper.replace(
            "BotTeams=1",
            "BotTeams=2
PlayerTeam=2",
        );
        assert_eq!(features(&other), Vec::<&str>::new());
    }
}
