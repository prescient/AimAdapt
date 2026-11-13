//! Tests for hurt volumes and deadly teleporters (plan K12a). The base text is the made-up Kill Gate fixture:
//! a map of version 0.0.7 at map scale 2 with a thin teleporter across a lane of strafing balls (health 20) that
//! leads to a waypoint inside a far away hurt box that does 100000 damage every frame. Every refusal changes one
//! thing in an accepted file, and every number is made up.

use aimadapt_sim::world::WorldVolume;

use super::super::*;

const GATE: &str = include_str!("../../../tests/fixtures/AimAdapt Test - Kill Gate.sce");
const PENS: &str = include_str!("../../../tests/fixtures/AimAdapt Test - Holding Pens.sce");
const KNOT: &str = include_str!("../../../tests/fixtures/AimAdapt Test - Top Knot.sce");

fn convert_text(text: &str) -> Result<Imported, Vec<Gap>> {
    convert(&Sce::parse(text), "gate.sce", "0")
}

fn imported(text: &str) -> Imported {
    convert_text(text).unwrap_or_else(|e| panic!("{:?}", e.iter().map(ToString::to_string).collect::<Vec<_>>()))
}

/// The details of the "map volumes" gaps the inventory gives, in order.
fn volume_gaps(text: &str) -> Vec<String> {
    crate::inventory::gaps(&Sce::parse(text))
        .into_iter()
        .filter(|g| g.feature == "map volumes")
        .map(|g| g.detail)
        .collect()
}

/// Every reason the converter gives, as it reads.
fn refused(text: &str) -> Vec<String> {
    convert_text(text).err().unwrap_or_default().iter().map(ToString::to_string).collect()
}

/// Changes a piece of a text; it has to be there once.
fn change(text: &str, old: &str, new: &str) -> String {
    assert_eq!(text.matches(old).count(), 1, "{old}");
    text.replacen(old, new, 1)
}

/// The text with one more map object, as the first one.
fn with_object(text: &str, object: &str) -> String {
    change(text, "\"objects\": [", &format!("\"objects\": [\n {object},"))
}

/// A `Hurt` object at `location` (unscaled) with `scale`, and its properties as written.
fn hurt(location: &str, scale: &str, kill: bool, damage: f32, cooldown: f32) -> String {
    format!(
        "{{\"type\": \"gameObject\", \"name\": \"Hurt\", \"location\": \"{location}\", \"rotation\": \"0, 0, 0\", \
         \"scale\": \"{scale}\", \"properties\": [{{\"name\": \"Kill\", \"value\": {kill}}}, {{\"name\": \"Damage\", \
         \"value\": {damage:?}}}, {{\"name\": \"Cooldown\", \"value\": {cooldown:?}}}]}}"
    )
}

/// The gate's hurt box, far away, with other numbers.
fn pit(text: &str, kill: bool, damage: f32, cooldown: f32) -> String {
    let old = "\"properties\": [{\"name\": \"Kill\", \"value\": false}, {\"name\": \"Damage\", \"value\": 100000.0}, {\"name\": \"Cooldown\", \"value\": 0.0}]";
    let new = format!(
        "\"properties\": [{{\"name\": \"Kill\", \"value\": {kill}}}, {{\"name\": \"Damage\", \"value\": {damage:?}}}, \
         {{\"name\": \"Cooldown\", \"value\": {cooldown:?}}}]"
    );
    change(text, old, &new)
}

fn volumes_of(text: &str) -> Vec<WorldVolume> {
    imported(text).scenario.world.unwrap().volumes
}

fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

/// The base file plays: its teleporter has no delay and its target kills at once.
#[test]
fn a_gate_with_a_deadly_target_plays() {
    assert_eq!(volume_gaps(GATE), Vec::<String>::new());
    assert_eq!(refused(GATE), Vec::<String>::new());
}

/// A hurt volume is stored shown with its numbers. A teleporter whose target kills at once is stored unseen, as a
/// volume that kills what touches it (the player sees the body vanish).
#[test]
fn a_hurt_is_stored_with_its_numbers_and_a_deadly_teleporter_as_an_unseen_kill() {
    let v = volumes_of(GATE);
    assert_eq!(v.len(), 2);
    let (gate, pit) = (&v[0], &v[1]);
    assert_eq!((gate.kill, gate.shown, gate.damage, gate.cooldown_s), (true, false, 0.0, 0.0));
    assert_eq!((pit.kill, pit.shown, pit.damage, pit.cooldown_s), (false, true, 100000.0, 0.0));
}

/// Every volume the sim gets is one it can use: finite, with one vertical edge.
#[test]
fn the_converted_volumes_pass_the_worlds_checks() {
    for text in [GATE, PENS] {
        let i = imported(text);
        assert_eq!(i.scenario.world.as_ref().unwrap().problems(), Vec::<String>::new());
    }
}

/// In a map of version 0.0.7 a volume's location is its middle, in 1.0.0 its lowest corner: the same text gives
/// boxes half the size apart on each axis (measured in KovaaK's 3.9.13).
#[test]
fn the_pivot_depends_on_the_maps_version() {
    let corner = volumes_of(PENS);
    let middle = volumes_of(&change(PENS, "\"version\": \"1.0.0\"", "\"version\": \"0.0.7\""));
    assert_eq!(corner.len(), 2);
    for (c, m) in corner.iter().zip(&middle) {
        assert_eq!(c.edges, m.edges);
        let half: [f32; 3] = std::array::from_fn(|k| (0..3).map(|e| c.edges[e][k]).sum::<f32>() / 2.0);
        let want: [f32; 3] = std::array::from_fn(|k| c.corner[k] - half[k]);
        assert!(near(m.corner, want), "{:?} from {:?} by {half:?}", m.corner, c.corner);
    }
    // The pens are 200 units of the file's, 1.5 times that at map scale 1.5: 3 m a side.
    let sizes: Vec<f32> = corner[0].edges.iter().map(|e| e.iter().map(|v| v * v).sum::<f32>().sqrt()).collect();
    assert!(sizes.iter().all(|s| (s - 3.0).abs() < 1e-4), "{sizes:?}");
}

/// `MapScale` multiplies a volume's place and its size, like a brush's.
#[test]
fn map_scale_multiplies_a_volumes_place_and_size() {
    let one = volumes_of(&change(PENS, "MapScale=1.5", "MapScale=1.0"));
    let two = volumes_of(&change(PENS, "MapScale=1.5", "MapScale=2.0"));
    for (a, b) in one.iter().zip(&two) {
        for e in 0..3 {
            assert!(near(b.edges[e], a.edges[e].map(|v| v * 2.0)), "{:?} {:?}", a.edges, b.edges);
        }
        assert!(near(b.corner, a.corner.map(|v| v * 2.0)), "{:?} {:?}", a.corner, b.corner);
    }
}

/// The two pens are stored shown, with the file's damage and cooldown, and no kill.
#[test]
fn small_hurt_cubes_come_through_with_their_numbers() {
    let v = volumes_of(PENS);
    assert!(v.iter().all(|v| v.shown && !v.kill && v.damage == 40.0 && v.cooldown_s == 1.0));
}

#[test]
fn a_turned_volume_is_refused() {
    let text = change(
        GATE,
        "\"location\": \"1000, 100, -50\", \"rotation\": \"0, 0, 0\"",
        "\"location\": \"1000, 100, -50\", \"rotation\": \"0, 0, 30\"",
    );
    assert_eq!(volume_gaps(&text), vec!["a turned volume"]);
    assert!(refused(&text).iter().any(|r| r.contains("a turned volume")));
}

#[test]
fn a_teleporter_with_a_delay_is_refused() {
    let text =
        change(GATE, "{\"name\": \"TeleportDelay\", \"value\": 0.0}", "{\"name\": \"TeleportDelay\", \"value\": 2.5}");
    assert_eq!(volume_gaps(&text), vec!["a teleporter with a delay"]);
}

#[test]
fn a_teleporter_whose_target_is_missing_is_refused() {
    let text =
        change(GATE, "{\"name\": \"Target\", \"value\": \"pit\"}", "{\"name\": \"Target\", \"value\": \"nowhere\"}");
    assert_eq!(volume_gaps(&text), vec!["a teleporter whose target is missing"]);
}

/// A teleporter imports only when its target is deadly at once: a kill box, or a box with no cooldown whose damage
/// is at least the largest health of the target bots (20 here).
#[test]
fn a_teleporter_that_leads_into_the_open_is_refused_unless_the_target_kills_at_once() {
    let open = "\"location\": \"1000, 5000, 5000\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 1, 1\"";
    let text =
        change(GATE, open, "\"location\": \"1000, 300, -150\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 1, 1\"");
    assert_eq!(volume_gaps(&text), vec!["a teleporter that leads into the open"]);
    // Cases of the hurt box the target is in.
    assert_eq!(volume_gaps(&pit(GATE, true, 0.0, 3.0)), Vec::<String>::new(), "a kill box, whatever its cooldown");
    assert_eq!(volume_gaps(&pit(GATE, false, 20.0, 0.0)), Vec::<String>::new(), "damage equal to the health");
    assert_eq!(
        volume_gaps(&pit(GATE, false, 19.0, 0.0)),
        vec!["a teleporter that leads into the open", "damage every frame that doesn't kill at once"]
    );
    assert_eq!(
        volume_gaps(&pit(GATE, false, 100000.0, 2.0)),
        vec!["a teleporter that leads into the open"],
        "it waits for its cooldown"
    );
}

/// The middle of the body ends a spawn rise (80 units) above the waypoint, so a box whose top is between the two
/// holds the waypoint and not the body. The pit is 800 units tall at map scale 2, and its middle is at 2 times
/// the file's 4850 or 4820, with the waypoint at 10000 and the body at 10080.
#[test]
fn the_body_has_to_be_inside_the_deadly_box_a_spawn_rise_above_the_waypoint() {
    let at = |z: i32| {
        let old = "\"location\": \"1000, 5000, 5000\", \"rotation\": \"0, 0, 0\", \"scale\": \"4, 4, 4\"";
        let new = format!("\"location\": \"1000, 5000, {z}\", \"rotation\": \"0, 0, 0\", \"scale\": \"4, 4, 4\"");
        change(GATE, old, &new)
    };
    assert_eq!(volume_gaps(&at(4850)), Vec::<String>::new(), "the box's top is at 10100, over the body");
    assert_eq!(volume_gaps(&at(4820)), vec!["a teleporter that leads into the open"], "its top, 10040, is under it");
}

#[test]
fn damage_every_frame_that_does_not_kill_at_once_is_refused() {
    for (damage, refused_for) in [(5.0, true), (0.0, false), (20.0, false), (100000.0, false)] {
        let text = with_object(GATE, &hurt("0, 4000, 0", "1, 1, 1", false, damage, 0.0));
        let want: Vec<String> =
            refused_for.then(|| "damage every frame that doesn't kill at once".to_string()).into_iter().collect();
        assert_eq!(volume_gaps(&text), want, "damage {damage}");
    }
    let slow = with_object(GATE, &hurt("0, 4000, 0", "1, 1, 1", false, 5.0, 0.5));
    assert_eq!(volume_gaps(&slow), Vec::<String>::new(), "with a cooldown the damage doesn't depend on the frame rate");
    let kill = with_object(GATE, &hurt("0, 4000, 0", "1, 1, 1", true, 5.0, 0.0));
    assert_eq!(volume_gaps(&kill), Vec::<String>::new(), "a kill volume's damage is never used");
}

/// The gate with its second ball swapped for a smaller, weaker kind: radius 10 (a spawn rise of 20) and health 5.
fn with_a_mite(text: &str) -> String {
    let text = change(text, "AddedBots=Strider.bot;Strider.bot", "AddedBots=Strider.bot;Mite.bot");
    let mite = "[Bot Profile]\nName=Mite\nCharacterProfile=Mite Ball\nDodgeProfileNames=Side Weave\n\
                DodgeProfileWeights=1.0\nUseWeapons=false\nSeeThroughWalls=false\n[Character Profile]\n\
                Name=Mite Ball\nMaxHealth=5.0\nMaxSpeed=500.0\nAcceleration=3000.0\nFriction=8.0\n\
                MainBBType=Spheroid\nMainBBRadius=10.0\nMainBBHeight=20.0\nMainBBHasHead=false\n[Dodge Profile]";
    change(&text, "[Dodge Profile]", mite)
}

/// Every kind of target bot has to land inside the deadly box. With the box's top at 10040 a mite's middle
/// (10020) is inside and a strider's (10080) is over it, so a file with both leads into the open.
#[test]
fn every_kind_of_bot_has_to_land_inside_the_deadly_box() {
    let low = |text: &str| {
        let old = "\"location\": \"1000, 5000, 5000\", \"rotation\": \"0, 0, 0\", \"scale\": \"4, 4, 4\"";
        change(text, old, "\"location\": \"1000, 5000, 4820\", \"rotation\": \"0, 0, 0\", \"scale\": \"4, 4, 4\"")
    };
    let mixed = with_a_mite(GATE);
    assert_eq!(volume_gaps(&mixed), Vec::<String>::new(), "both kinds land inside the tall box");
    assert_eq!(volume_gaps(&low(&mixed)), vec!["a teleporter that leads into the open"]);
    let mites = change(&low(&mixed), "AddedBots=Strider.bot;Mite.bot", "AddedBots=Mite.bot;Mite.bot");
    assert_eq!(volume_gaps(&mites), Vec::<String>::new(), "mites alone all land inside");
}

/// A hit that is due every frame has to kill the healthiest kind of target bot at once: 10 damage kills a mite
/// (health 5) and not a strider (health 20).
#[test]
fn damage_every_frame_is_judged_by_the_healthiest_kind() {
    let mixed = with_a_mite(GATE);
    let weak = with_object(&mixed, &hurt("0, 4000, 0", "1, 1, 1", false, 10.0, 0.0));
    assert_eq!(volume_gaps(&weak), vec!["damage every frame that doesn't kill at once"]);
    let enough = with_object(&mixed, &hurt("0, 4000, 0", "1, 1, 1", false, 20.0, 0.0));
    assert_eq!(volume_gaps(&enough), Vec::<String>::new());
}

/// A volume that would heal, or one with a cooldown below zero, was never measured. It is refused by name,
/// before the world's own checks can refuse it as a value the trainer does not allow.
#[test]
fn a_volume_with_a_negative_number_is_refused() {
    for (damage, cooldown) in [(-5.0, 1.0), (100000.0, -1.0)] {
        let text = with_object(GATE, &hurt("0, 4000, 0", "1, 1, 1", false, damage, cooldown));
        assert_eq!(volume_gaps(&text), ["a volume with a negative number"], "damage {damage}, cooldown {cooldown}");
    }
}

/// The player's body, a box from the spawn point up to twice its rise and a little wider than its radius and
/// offsets, must not meet a volume. A hurt box whose middle is at the player's spawn point does, and the same
/// numbers in a 1.0.0 map (where the location is a corner) leave the spawn outside.
#[test]
fn a_volume_that_meets_the_players_start_is_refused() {
    let at_start = with_object(GATE, &hurt("0, 0, 0", "1, 1, 1", false, 0.0, 1.0));
    assert_eq!(volume_gaps(&at_start), vec!["the player starts inside one"]);
    let clear = with_object(GATE, &hurt("0, 600, 0", "1, 1, 1", false, 0.0, 1.0));
    assert_eq!(volume_gaps(&clear), Vec::<String>::new());
    // Corner at (-100, -50, -50): in a 0.0.7 map the box runs -150 to -50 in x, in a 1.0.0 map -100 to 0.
    let beside = hurt("-100, -50, -50", "1, 1, 1", false, 0.0, 1.0);
    assert_eq!(volume_gaps(&with_object(GATE, &beside)), Vec::<String>::new(), "centred, it is behind the player");
    let corner_map = change(GATE, "\"version\": \"0.0.7\"", "\"version\": \"1.0.0\"");
    assert!(volume_gaps(&with_object(&corner_map, &beside)).contains(&"the player starts inside one".to_string()));
}

/// The player's own size and spawn offsets count. The base file's player has neither, so a box whose face is
/// 0.6 m to the side of its spawn point, or 1.2 m over it, is clear. The same box meets a player of radius 60,
/// one whose offsets reach 70, and one 90 tall (which is taken to stand up to twice its rise).
#[test]
fn the_players_size_and_offsets_widen_its_start() {
    let hit = vec!["the player starts inside one".to_string()];
    // At map scale 2 a box of scale 1 is 2 m a side, around twice its location.
    let beside = hurt("0, 80, 0", "1, 1, 1", false, 0.0, 1.0);
    let over = hurt("0, 0, 110", "1, 1, 1", false, 0.0, 1.0);
    assert_eq!(volume_gaps(&with_object(GATE, &beside)), Vec::<String>::new());
    assert_eq!(volume_gaps(&with_object(GATE, &over)), Vec::<String>::new());
    let sized = |setting: &str| {
        change(GATE, "WeaponProfileNames=Tapper;;;", &format!("WeaponProfileNames=Tapper;;;\n{setting}"))
    };
    assert_eq!(volume_gaps(&with_object(&sized("MainBBRadius=60.0"), &beside)), hit, "its radius");
    assert_eq!(volume_gaps(&with_object(&sized("MainBBHeight=90.0"), &over)), hit, "its height");
    let offset = change(GATE, "SpawnOffsetMax=X=0.000 Y=0.000 Z=0.000", "SpawnOffsetMax=X=0.000 Y=70.000 Z=0.000");
    assert_eq!(volume_gaps(&with_object(&offset, &beside)), hit, "its spawn offsets");
}

/// A volume near a spawn volume the player may start in counts, and so does one near a point of the player's team
/// only: a spawn only the bots' team can use is no concern.
#[test]
fn only_the_spawns_of_the_players_team_count_for_the_players_start() {
    let near_bots = with_object(GATE, &hurt("1000, -100, -150", "1, 1, 1", false, 0.0, 1.0));
    assert_eq!(volume_gaps(&near_bots), Vec::<String>::new());
    let player_volume = "{\"type\": \"gameObject\", \"name\": \"SpawnVolume\", \"location\": \"0, 300, -50\", \"rotation\": \"0, 0, 0\", \"scale\": \"1, 1, 1\", \"properties\": [{\"name\": \"TeamMask\", \"value\": 1}]}";
    let text = with_object(&with_object(GATE, player_volume), &hurt("0, 380, 0", "1, 1, 1", false, 0.0, 1.0));
    assert_eq!(volume_gaps(&text), vec!["the player starts inside one"], "the whole spawn volume is used");
}

#[test]
fn volumes_that_hurt_are_refused_when_the_bots_cannot_die() {
    let text = change(GATE, "MapScale=2.0", "MapScale=2.0\nInvincibleBots=true");
    assert!(volume_gaps(&text).contains(&"bots that can't die".to_string()), "{:?}", volume_gaps(&text));
    let own = change(GATE, "MaxHealth=20.0", "MaxHealth=20.0\nInvincibleBots=true");
    assert!(volume_gaps(&own).contains(&"bots that can't die".to_string()));
    // A hurt that does nothing to a bot that can't die is harmless: the pens, with their damage taken off.
    let pens = change(
        PENS,
        "MapScale=1.5",
        "MapScale=1.5
InvincibleBots=true",
    );
    assert_eq!(volume_gaps(&pens), vec!["bots that can't die"]);
    let harmless = pens.replace("\"value\": 40.0", "\"value\": 0.0");
    assert_eq!(volume_gaps(&harmless), Vec::<String>::new());
}

#[test]
fn volumes_are_refused_in_a_file_with_helper_bots() {
    let text = change(GATE, "BotTeams=2;2", "BotTeams=2;1");
    assert!(volume_gaps(&text).contains(&"a bot on the player's team".to_string()), "{:?}", volume_gaps(&text));
}

/// A `JumpPad` isn't a volume the trainer plays: it stays one of the "map objects", and no volume detail appears.
#[test]
fn a_jump_pad_is_still_a_map_object() {
    let text = with_object(
        GATE,
        "{\"type\": \"gameObject\", \"name\": \"JumpPad\", \"location\": \"0, 600, 0\", \"properties\": [{\"name\": \"Target\", \"value\": \"pit\"}]}",
    );
    assert_eq!(volume_gaps(&text), Vec::<String>::new());
    let gaps = crate::inventory::gaps(&Sce::parse(&text));
    assert_eq!(
        gaps.iter().map(|g| (g.feature, g.detail.as_str())).collect::<Vec<_>>(),
        vec![("map objects", "JumpPad")]
    );
}

/// The slow gate fixture: the teleporter waits 5 s and leads into the open.
#[test]
fn the_slow_gate_fixture_says_both_reasons() {
    let slow = include_str!("../../../tests/fixtures/AimAdapt Test - Slow Gate.sce");
    assert_eq!(volume_gaps(slow), vec!["a teleporter with a delay", "a teleporter that leads into the open"]);
}

/// Files without volumes get no notes about them. With volumes they get two, and a third when a bot has a head.
#[test]
fn volume_notes_come_only_with_volumes() {
    let has = |i: &Imported, word: &str| i.notes.iter().filter(|n| n.contains(word)).count();
    assert_eq!(has(&imported(KNOT), "volume"), 0);
    let i = imported(GATE);
    assert_eq!((has(&i, "bounding box"), has(&i, "damage reactions"), has(&i, "body and head")), (1, 1, 0));
    let headed = with_object(KNOT, &hurt("0, 4000, 0", "1, 1, 1", false, 0.0, 1.0));
    let i = imported(&headed);
    assert_eq!(
        (has(&i, "bounding box"), has(&i, "damage reactions"), has(&i, "body and head")),
        (1, 1, 1),
        "{:?}",
        i.notes
    );
}
