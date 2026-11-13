//! Tests for several pellets per shot (plan K11a). The base text is the made-up Pellet Yard fixture: three still
//! balls of health 50 shot by a weapon of six pellets of 5, whose pattern for standing still (`SpreadSSH`) ends in 0
//! and whose other seven patterns end in 3.5. Every refusal changes one key of an accepted file.

use super::super::*;

const YARD: &str = include_str!("../../../tests/fixtures/AimAdapt Test - Pellet Yard.sce");

fn convert_text(text: &str) -> Result<Imported, Vec<Gap>> {
    convert(&Sce::parse(text), "yard.sce", "0")
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
    change(YARD, old, new)
}

/// The base file with its weapon's shots-per-click line replaced.
fn clicks(line: &str) -> String {
    with("ShotsPerClick=6\n", line)
}

const SSH: &str = "SpreadSSH=0.4,0.6,0.2,0.0";

#[test]
fn the_pellet_count_is_stored_on_the_weapon() {
    let i = imported(YARD);
    assert_eq!((i.scenario.weapon.pellets, i.scenario.weapon.damage), (6, 5.0));
    assert!(i.scenario.problems().is_empty(), "{:?}", i.scenario.problems());
    // The largest count the trainer allows plays.
    assert_eq!(imported(&with("ShotsPerClick=6", "ShotsPerClick=1000")).scenario.weapon.pellets, 1000);
    assert_eq!(imported(&with("ShotsPerClick=6", "ShotsPerClick=2")).scenario.weapon.pellets, 2);
}

/// A count of 1, no line at all and a value below 1 are the plain weapon of one pellet, as before.
#[test]
fn a_count_of_one_a_missing_line_and_a_value_below_one_give_one_pellet() {
    for line in ["ShotsPerClick=1\n", "", "ShotsPerClick=0.5\n", "ShotsPerClick=0\n", "ShotsPerClick=-3\n"] {
        let i = imported(&clicks(line));
        assert_eq!(i.scenario.weapon.pellets, 1, "{line:?}");
        assert!(!i.scenario.to_toml().contains("pellets ="), "{line:?}");
    }
}

#[test]
fn a_count_that_is_not_whole_or_is_above_1000_is_refused_by_its_detail() {
    let text = clicks("ShotsPerClick=2.5\n");
    assert_eq!(refused(&text), ["several pellets per shot (ShotsPerClick=2.5 isn't a whole number)"]);
    let text = clicks("ShotsPerClick=1000.5\n");
    assert_eq!(refused(&text), ["several pellets per shot (ShotsPerClick=1000.5 isn't a whole number)"]);
    let text = clicks("ShotsPerClick=1001\n");
    assert_eq!(refused(&text), ["several pellets per shot (ShotsPerClick=1001 is above 1000)"]);
    // A count between 1 and 2 is looked at too: it isn't the plain weapon of one pellet.
    let text = clicks("ShotsPerClick=1.5\n");
    assert_eq!(refused(&text), ["several pellets per shot (ShotsPerClick=1.5 isn't a whole number)"]);
}

#[test]
fn a_pattern_that_spreads_when_standing_still_is_refused() {
    let spreading = |v: &str| refused(&with(SSH, &format!("SpreadSSH={v}")));
    let want = |v: &str| {
        let why = "has a fourth number other than 0, so shots spread when the player stands still";
        vec![format!("weapon spread (SpreadSSH={v} {why})")]
    };
    assert_eq!(spreading("0.4,0.6,0.2,3.5"), want("0.4,0.6,0.2,3.5"));
    assert_eq!(spreading("0.4,0.6,0.2,-1.0"), want("0.4,0.6,0.2,-1.0"));
    // Spaces around the numbers are allowed.
    assert_eq!(spreading(" 0.4, 0.6 ,0.2 , 0.5 "), want("0.4, 0.6 ,0.2 , 0.5"));
}

#[test]
fn a_pattern_that_cant_be_read_is_refused() {
    let unreadable = |v: &str| refused(&with(SSH, &format!("SpreadSSH={v}")));
    let want = |v: &str| vec![format!("weapon spread (SpreadSSH={v} can't be read as four numbers)")];
    assert_eq!(unreadable("0.4,0.6,0.0"), want("0.4,0.6,0.0"));
    assert_eq!(unreadable("0.4,0.6,0.2,0.0,1.0"), want("0.4,0.6,0.2,0.0,1.0"));
    assert_eq!(unreadable("0.4,wide,0.2,0.0"), want("0.4,wide,0.2,0.0"));
    assert_eq!(unreadable(""), want(""));
    // Words that Rust reads as numbers aren't numbers here, in the fourth place or another.
    assert_eq!(unreadable("0.4,0.6,inf,0.0"), want("0.4,0.6,inf,0.0"));
    assert_eq!(unreadable("0.4,0.6,0.2,NaN"), want("0.4,0.6,0.2,NaN"));
}

/// A fourth number of 0 means nothing spreads, whatever the other three numbers and the other seven patterns are
/// (measured in KovaaK's 3.9.13: 31 of 31 shots hit under held fast fire). The base file's other seven end in 3.5.
#[test]
fn a_fourth_number_of_zero_plays_whatever_the_other_numbers_are() {
    assert_eq!(refused(YARD), Vec::<String>::new());
    for ssh in ["9.0,9.0,9.0,0", "0,0,0,0", "0.4,0.6,0.2,0.0", " 1 , 2 , 3 , 0 ", "0.4,0.6,0.2,-0.0"] {
        let text = with(SSH, &format!("SpreadSSH={ssh}"));
        assert_eq!(refused(&text), Vec::<String>::new(), "{ssh}");
    }
}

/// A weapon with no `SpreadSSH` line plays with no spread, and the lines that go with the patterns are not read.
#[test]
fn a_weapon_with_no_spread_line_plays_and_the_related_lines_are_not_read() {
    assert_eq!(refused(&with("SpreadSSH=0.4,0.6,0.2,0.0\n", "")), Vec::<String>::new());
    assert_eq!(refused(&with("UsePerBulletSpread=true", "UsePerBulletSpread=false")), Vec::<String>::new());
    assert_eq!(refused(&with("SpreadStationaryVelocity=25.0", "SpreadStationaryVelocity=-4")), Vec::<String>::new());
    assert_eq!(refused(&with("SpreadDecayDelay=0.2", "SpreadDecayDelay=abc")), Vec::<String>::new());
}

/// A weapon of one pellet whose pattern spreads is refused as well: the spread law wasn't measured.
#[test]
fn a_one_pellet_weapon_that_spreads_is_refused() {
    for line in ["ShotsPerClick=1\n", ""] {
        let text = change(&clicks(line), SSH, "SpreadSSH=0.4,0.6,0.2,3.5");
        let found = refused(&text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("weapon spread (SpreadSSH=0.4,0.6,0.2,3.5"), "{found:?}");
    }
}

/// Both reasons of the Loose Choke fixture's weapon come together.
#[test]
fn both_refusals_are_given_at_once() {
    let text = change(&clicks("ShotsPerClick=2.5\n"), SSH, "SpreadSSH=0.4,0.6,0.2,3.5");
    assert_eq!(refused(&text).len(), 2, "{:?}", refused(&text));
}

/// A bot's health bar shows when one click can't kill it, so with pellets the click is the whole volley: health 50
/// against 6 pellets of 5 (30) shows a bar and against 6 pellets of 10 (60) doesn't. The base file blocks health
/// bars, so the line that does that is taken out.
#[test]
fn the_health_bar_compares_health_with_a_whole_volley() {
    let open = with("BlockHealthbars=true\n", "");
    let bar = |text: &str| imported(text).scenario.targets.health_bar;
    assert!(bar(&open));
    assert!(!bar(&change(&open, "DamagePerShot=5.0", "DamagePerShot=10.0")));
    assert!(!bar(&change(&open, "DamagePerShot=5.0", "DamagePerShot=8.4")));
    assert!(bar(&change(&open, "DamagePerShot=5.0", "DamagePerShot=8.3")));
    // The same damage with one pellet is a bar: a click of 10 doesn't kill health 50.
    let single = change(&change(&open, "ShotsPerClick=6\n", ""), "DamagePerShot=5.0", "DamagePerShot=10.0");
    assert!(bar(&single));
}

/// The head note appears once, and only when the weapon fires several pellets and a shootable bot has a head.
#[test]
fn the_head_note_needs_both_pellets_and_a_head() {
    let note = |i: &Imported| i.notes.iter().filter(|n| n.contains("pellets against a head")).count();
    assert_eq!(note(&imported(YARD)), 0, "pellets and no head");
    let headed = with("MainBBHasHead=false", "MainBBHasHead=true\nMainBBHeadRadius=20.0\nMainBBHeadOffset=0.0");
    let i = imported(&headed);
    assert_eq!(note(&i), 1, "{:?}", i.notes);
    assert!(i.notes.iter().any(|n| n.contains("fires 6 pellets a click")), "{:?}", i.notes);
    let one = change(&headed, "ShotsPerClick=6\n", "");
    assert_eq!(note(&imported(&one)), 0, "a head and one pellet");
}
