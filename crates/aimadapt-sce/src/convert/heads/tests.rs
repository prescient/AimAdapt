//! Tests for bots with heads and the weapon's headshot damage (plan K9). The base text is the made-up Top Knot
//! fixture: three still capsules with heads (a radius of 0.25 m sunk 0.1 m into the body) shot by a weapon that
//! does 25 and four times that to a head, scored by damage efficiency. Every refusal changes one key of an accepted
//! file, and every number is made up.

use aimadapt_sim::scenario::Head;

use super::super::*;

const KNOT: &str = include_str!("../../../tests/fixtures/AimAdapt Test - Top Knot.sce");

fn convert_text(text: &str) -> Result<Imported, Vec<Gap>> {
    convert(&Sce::parse(text), "knot.sce", "0")
}

fn imported(text: &str) -> Imported {
    convert_text(text).unwrap_or_else(|e| panic!("{:?}", e.iter().map(ToString::to_string).collect::<Vec<_>>()))
}

/// The reasons the file is refused for, as they read.
fn refused(text: &str) -> Vec<String> {
    convert_text(text).err().unwrap_or_default().iter().map(ToString::to_string).collect()
}

/// Changes a line of a text; the line has to be there once.
fn change(text: &str, old: &str, new: &str) -> String {
    assert_eq!(text.matches(old).count(), 1, "{old}");
    text.replacen(old, new, 1)
}

fn with(old: &str, new: &str) -> String {
    change(KNOT, old, new)
}

/// The base file with no head on its characters.
fn headless() -> String {
    with("MainBBHasHead=true", "MainBBHasHead=false")
}

/// The file with a second kind of bot, a copy of the character with `extra` lines at the end of it (the last
/// of a key wins), filling the third slot.
fn two_kinds(text: &str, extra: &str) -> String {
    let from = text.find("[Character Profile]\nName=Knot Capsule").unwrap();
    let section = &text[from..text.find("[Map Data]").unwrap()];
    let second = format!(
        "[Bot Profile]\nName=Wide\nCharacterProfile=Knot Wide\nUseWeapons=false\n{}{extra}\n",
        section.replace("Name=Knot Capsule", "Name=Knot Wide")
    );
    let text = change(text, "AddedBots=Knot.bot;Knot.bot;Knot.bot", "AddedBots=Knot.bot;Knot.bot;Wide.bot");
    change(&text, "[Map Data]", &format!("{second}[Map Data]"))
}

/// The head the base file's character has, in the sim's units: 25 units of radius, 10 of sink, and the
/// colour of `X=0.900 Y=0.800 Z=0.200`.
fn knot_head() -> Head {
    Head { radius: 0.25, offset: -0.1, color: "#e6cc33".into(), headshots_only: false }
}

/// Bug guard (found in the review of plan K9): a head with a radius under 0.01 m (a made-up 0.02 units here,
/// floating above the body) got past the importer and was then refused as "a value the trainer doesn't
/// allow", because the trainer wanted a head of 0.01 m or more. Files give a bot such a speck where they want
/// no head worth hitting. Any radius above 0 plays; 0 and less keep their own reason.
#[test]
fn a_head_too_small_to_hit_still_imports() {
    let text = with("MainBBHeadRadius=25.0", "MainBBHeadRadius=0.02");
    let text = change(&text, "MainBBHeadOffset=-10.0", "MainBBHeadOffset=40.0");
    assert_eq!(refused(&text), Vec::<String>::new());
    let i = imported(&text);
    let head = i.scenario.targets.head.as_ref().unwrap();
    assert!((head.radius - 0.0002).abs() < 1e-7 && (head.offset - 0.4).abs() < 1e-6, "{head:?}");
    assert!(i.scenario.problems().is_empty(), "{:?}", i.scenario.problems());
}

/// A capsule bot's head comes through in the sim's units: the radius and offset in meters, the colour of
/// `EnemyHeadColor`. With one kind it is on the scenario's targets; there is no "heads" gap.
#[test]
fn a_capsule_bots_head_is_imported_on_the_targets_with_one_kind() {
    let i = imported(KNOT);
    assert_eq!(i.scenario.targets.head, Some(knot_head()));
    assert!(i.scenario.world.as_ref().unwrap().kinds.is_empty());
    assert!(i.scenario.problems().is_empty(), "{:?}", i.scenario.problems());
    // The only note is the score rule's, about damage efficiency with points for kills.
    assert!(i.notes.iter().all(|n| !n.contains("head")), "a plain head gets no note: {:?}", i.notes);
    assert!(crate::gaps(&Sce::parse(KNOT)).is_empty());
    // A ball (a spheroid) takes a head too.
    let ball = imported(&with("MainBBType=Cylindrical", "MainBBType=Spheroid"));
    assert_eq!(ball.scenario.targets.head, Some(knot_head()));
    // An unset head colour (black) leaves the default.
    let plain = imported(&with("EnemyHeadColor=X=0.900 Y=0.800 Z=0.200", "EnemyHeadColor=X=0.000 Y=0.000 Z=0.000"));
    assert_eq!(plain.scenario.targets.head, Some(Head { color: Head::default().color, ..knot_head() }));
}

/// With several kinds of bot each kind has its own head.
#[test]
fn heads_are_imported_on_each_kind_with_several() {
    let i = imported(&two_kinds(KNOT, "MainBBHeadRadius=30.0\nMainBBHeadOffset=0.0"));
    let w = i.scenario.world.as_ref().unwrap();
    assert_eq!(w.kinds.len(), 2);
    assert_eq!(w.kinds[0].head, Some(knot_head()));
    assert_eq!(w.kinds[1].head, Some(Head { radius: 0.3, offset: 0.0, ..knot_head() }));
    assert!(i.scenario.targets.head.is_none(), "the kinds hold the heads");
    assert!(i.scenario.problems().is_empty());
}

/// `HeadshotOnly=true` on a character with a head, with a weapon that can headshot, imports as a head that is
/// all that counts.
#[test]
fn headshots_only_is_imported_with_a_weapon_that_can_headshot() {
    let i = imported(&with("HeadshotOnly=false", "HeadshotOnly=true"));
    assert_eq!(i.scenario.targets.head, Some(Head { headshots_only: true, ..knot_head() }));
    assert_eq!(i.scenario.weapon.headshot_multiplier, 4.0);
}

/// What stays refused, one changed key of an accepted file each, with the reason that says why.
#[test]
fn what_wasnt_measured_stays_refused_with_the_reason() {
    // A head on a cuboid body.
    assert_eq!(
        refused(&with("MainBBType=Cylindrical", "MainBBType=Cuboid")),
        ["heads (Knot Capsule: a head on a cuboid body wasn't measured)"]
    );
    // A head with a radius of 0, or none.
    assert_eq!(
        refused(&with("MainBBHeadRadius=25.0", "MainBBHeadRadius=0.0")),
        ["heads (Knot Capsule: MainBBHeadRadius is 0, a head needs a radius above 0)"]
    );
    assert_eq!(
        refused(&with("MainBBHeadRadius=25.0", "MainBBHeadRadius=-5.0")),
        ["heads (Knot Capsule: MainBBHeadRadius is -5, a head needs a radius above 0)"]
    );
    // Headshots only on a character with no head.
    let no_head = headless();
    assert!(convert_text(&no_head).is_ok());
    assert_eq!(
        refused(&change(&no_head, "HeadshotOnly=false", "HeadshotOnly=true")),
        ["headshots only (Knot Capsule: HeadshotOnly=true on a character with no head)"]
    );
    // Headshots only with a weapon that can't headshot.
    let only = with("HeadshotOnly=false", "HeadshotOnly=true");
    assert!(convert_text(&only).is_ok());
    assert_eq!(
        refused(&change(&only, "HeadshotCapable=true", "HeadshotCapable=false")),
        [
            "headshots only (Knot Capsule: HeadshotOnly=true, and the player's weapon can't headshot (HeadshotCapable isn't true))"
        ]
    );
    // A multiplier of 0 on a weapon that can headshot, in a file where it would be set.
    assert_eq!(
        refused(&with("HeadshotMultiplier=4.0", "HeadshotMultiplier=0.0")),
        ["heads (Knot Pistol: HeadshotMultiplier is 0, it must be above 0)"]
    );
    assert_eq!(
        refused(&with("HeadshotMultiplier=4.0", "HeadshotMultiplier=-2.0")),
        ["heads (Knot Pistol: HeadshotMultiplier is -2, it must be above 0)"]
    );
    // The multiplier's reasons need the weapon to count: where it isn't set, a 0 is no reason.
    let unused = change(
        &with("HeadshotMultiplier=4.0", "HeadshotMultiplier=0.0"),
        "HeadshotCapable=true",
        "HeadshotCapable=false",
    );
    assert!(convert_text(&unused).is_ok());
    let quiet = change(&headless(), "ScoreMultDamageEfficiency=true\n", "");
    assert!(convert_text(&change(&quiet, "HeadshotMultiplier=4.0", "HeadshotMultiplier=0.0")).is_ok());
    // A multiplier the sim can't use is a plain problem with the value.
    let huge = refused(&with("HeadshotMultiplier=4.0", "HeadshotMultiplier=5000.0"));
    assert_eq!(huge.len(), 1);
    assert!(huge[0].contains("headshot multiplier"), "{huge:?}");
}

/// The weapon's multiplier is set when the weapon can headshot, it isn't 1 and a shootable bot has a head
/// or the scenario scores by damage efficiency. In every other file it is left at 1, and the converted scenario is
/// exactly the one without the two weapon keys.
#[test]
fn the_multiplier_is_set_only_where_it_changes_a_score() {
    let multiplier = |text: &str| imported(text).scenario.weapon.headshot_multiplier;
    assert_eq!(multiplier(KNOT), 4.0, "a head and an efficiency score");
    let no_efficiency = with("ScoreMultDamageEfficiency=true\n", "");
    assert_eq!(multiplier(&no_efficiency), 4.0, "a head alone");
    assert_eq!(multiplier(&headless()), 4.0, "an efficiency score alone");
    // Left out.
    assert_eq!(multiplier(&with("HeadshotCapable=true", "HeadshotCapable=false")), 1.0, "a weapon that can't headshot");
    assert_eq!(multiplier(&with("HeadshotMultiplier=4.0", "HeadshotMultiplier=1.0")), 1.0, "a multiplier of 1");
    let without_keys = |text: &str| change(&change(text, "HeadshotCapable=true\n", ""), "HeadshotMultiplier=4.0\n", "");
    let nothing_to_headshot = change(&headless(), "ScoreMultDamageEfficiency=true\n", "");
    let (with_keys, without) = (imported(&nothing_to_headshot), imported(&without_keys(&nothing_to_headshot)));
    assert_eq!(with_keys.scenario.weapon.headshot_multiplier, 1.0);
    assert_eq!(with_keys.scenario, without.scenario, "no head and no efficiency score: nothing changes");
    assert_eq!(with_keys.notes, without.notes);
}

/// Bug guard: two characters that differ only in their head were merged into one kind, so one of them lost its
/// head. They stay two kinds. Two headless characters that differ only in a head setting nothing uses merge as
/// they did, and the colour of a head counts only for a character that has one.
#[test]
fn kinds_differ_by_their_heads_only_where_there_are_heads() {
    let kinds = |text: &str| imported(text).scenario.world.unwrap().kinds.len();
    assert_eq!(kinds(&two_kinds(KNOT, "MainBBHeadRadius=30.0")), 2);
    assert_eq!(kinds(&two_kinds(KNOT, "MainBBHeadOffset=0.0")), 2);
    assert_eq!(kinds(&two_kinds(KNOT, "EnemyHeadColor=X=0.100 Y=0.800 Z=0.200")), 2);
    assert_eq!(kinds(&two_kinds(KNOT, "HeadshotOnly=true")), 2);
    assert_eq!(kinds(&two_kinds(KNOT, "MainBBHasHead=false")), 2, "a head and none");
    // The same head twice is one kind.
    assert_eq!(kinds(&two_kinds(KNOT, "")), 0, "alike, so one kind and no list of kinds");
    // Headless characters differ in nothing that plays.
    let bare = headless();
    assert_eq!(kinds(&two_kinds(&bare, "MainBBHeadRadius=30.0\nMainBBHeadOffset=5.0")), 0);
    assert_eq!(kinds(&two_kinds(&bare, "EnemyHeadColor=X=0.100 Y=0.800 Z=0.200")), 0);
    // A different body still makes a second kind.
    assert_eq!(kinds(&two_kinds(&bare, "MainBBRadius=45.0")), 2);
}

/// What plays by a rule nobody checked in KovaaK's gets a note, once, and a file without those cases gets none
/// of them.
#[test]
fn heads_played_by_an_unchecked_rule_get_a_note() {
    let notes_of = |text: &str| -> Vec<String> {
        imported(text).notes.into_iter().filter(|n| n.contains("wasn't checked against KovaaK's")).collect()
    };
    // A head sunk more than twice its radius (the radius is 25 units, the offset -60 sinks it 2.4 times).
    let deep = with("MainBBHeadOffset=-10.0", "MainBBHeadOffset=-60.0");
    let n = notes_of(&deep);
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].starts_with("a head sunk more than twice its radius"), "{n:?}");
    // Two kinds with the same case give the note once.
    assert_eq!(notes_of(&two_kinds(&deep, "MainBBHeadRadius=24.0")).len(), 1);
    assert!(
        notes_of(&with("MainBBHeadOffset=-10.0", "MainBBHeadOffset=-50.0")).is_empty(),
        "exactly twice is not more"
    );
    // A ball with a head that falls (its character has gravity) stands on the bottom of its body, as
    // measured in KovaaK's, so it gets no note (it did, until a falling ball was measured).
    let ball = with("MainBBType=Cylindrical", "MainBBType=Spheroid\nGravity=1.0");
    assert!(notes_of(&ball).is_empty(), "{:?}", imported(&ball).notes);
    // A head wider than its body.
    let wide = with("MainBBHeadRadius=25.0", "MainBBHeadRadius=50.0");
    let n = notes_of(&wide);
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].starts_with("a head wider than its body"), "{n:?}");
    // None of them in the plain file, or in one with no head.
    assert!(notes_of(KNOT).is_empty());
    assert!(notes_of(&change(&headless(), "MainBBType=Cylindrical", "MainBBType=Spheroid\nGravity=1.0")).is_empty());
}

/// `MainBBHide` changes nothing about what a shot hits (measured), so it changes nothing in the converted
/// scenario.
#[test]
fn a_hidden_body_changes_nothing() {
    let hidden = imported(&with("HeadshotOnly=false", "HeadshotOnly=false\nMainBBHide=true"));
    let plain = imported(KNOT);
    assert_eq!(hidden.scenario, plain.scenario);
    assert_eq!(hidden.notes, plain.notes);
}

/// Guard (mutation check of plan K9): a head exactly as wide as its body isn't wider, so it gets no note. (A
/// note can't come twice: each text is added from one `any` over all kinds, so no file shows it.)
#[test]
fn a_head_as_wide_as_its_body_gets_no_note() {
    let notes_of = |text: &str| -> Vec<String> {
        imported(text).notes.into_iter().filter(|n| n.contains("wasn't checked against KovaaK's")).collect()
    };
    // The body's radius is 40 units.
    assert!(notes_of(&with("MainBBHeadRadius=25.0", "MainBBHeadRadius=40.0")).is_empty());
    assert_eq!(notes_of(&with("MainBBHeadRadius=25.0", "MainBBHeadRadius=41.0")).len(), 1);
}

/// A body can be a speck under its head (KovaaK's bots can be a ball a few hundredths of a unit across under a
/// far head). The importer refused any body radius under 0.01 m ("bot size"), with or without a head. With a head
/// anything above 0 and up to 10 m plays; without one the old limits stay, and a radius of 0 is refused either way.
#[test]
fn a_speck_of_a_body_imports_with_a_head_and_is_refused_without_one() {
    let size_reasons =
        |text: &str| -> Vec<String> { refused(text).into_iter().filter(|r| r.starts_with("bot size")).collect() };
    let speck = with("MainBBRadius=40.0", "MainBBRadius=0.3");
    assert_eq!(size_reasons(&speck), Vec::<String>::new());
    let i = imported(&speck);
    assert!((i.scenario.targets.radius - 0.003).abs() < 1e-7, "{}", i.scenario.targets.radius);
    assert!(i.scenario.problems().is_empty(), "{:?}", i.scenario.problems());
    // A second kind of bot is checked the same way.
    let two = two_kinds(&KNOT.replace("MainBBRadius=40.0", "MainBBRadius=0.3"), "MainBBRadius=0.2");
    assert_eq!(size_reasons(&two), Vec::<String>::new());
    // No head, no speck.
    let bare = change(&speck, "MainBBHasHead=true", "MainBBHasHead=false");
    let r = size_reasons(&bare);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].contains("0.01"), "{r:?}");
    let bare_second = two_kinds(&change(KNOT, "MainBBHasHead=true", "MainBBHasHead=false"), "MainBBRadius=0.2");
    assert_eq!(size_reasons(&bare_second).len(), 1, "{:?}", refused(&bare_second));
    // A radius of 0 or less is refused with a head too, as one past 10 m is.
    for radius in ["0.0", "-3.0", "1001.0"] {
        let text = with("MainBBRadius=40.0", &format!("MainBBRadius={radius}"));
        assert_eq!(size_reasons(&text).len(), 1, "{radius}: {:?}", refused(&text));
    }
    assert_eq!(size_reasons(&with("MainBBRadius=40.0", "MainBBRadius=1000.0")), Vec::<String>::new());
}
