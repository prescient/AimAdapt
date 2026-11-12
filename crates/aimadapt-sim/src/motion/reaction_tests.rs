//! Tests for a bot that uses an ability because it was hurt (plan K15d). Each guards the same bug: a
//! bot's reaction to damage wasn't played. The rules are the ones measured in KovaaK's, see
//! [`BotState::react_to_hit`].
//!
//! A test drives a bot by hand: it steps frames and calls `on_hit` between them, as the sim does. Every
//! timeline runs on even frames of 1/120 s and on uneven ones between 1/240 and 1/60 s. A hit lands at a
//! frame boundary, so on uneven frames the expected times are worked out from the clock at which each hit
//! was delivered. A use shows in the first frame that starts at or after its time.

use glam::Vec3;

use super::tests::{DT, arena, strafer};
use super::*;
use crate::collide::Collider;
use crate::rng::Rng;

const START: Vec3 = Vec3::new(0.0, -1.5, -10.0);

/// A bot stepped by hand, with what it did.
struct Rig {
    m: Motion,
    walls: Collider,
    rng: Rng,
    s: BotState,
    p: Vec3,
    seen: bool,
    /// The bot's clock at the start of the frame in which each use began.
    uses: Vec<f32>,
    /// The bot's clock when each hit was delivered.
    hits: Vec<f32>,
}

impl Rig {
    fn new(m: Motion, seed: u64, seen: bool) -> Rig {
        let mut rng = Rng::new(seed);
        let s = BotState::new(&m, &mut rng, START, Vec3::X);
        Rig { m, walls: arena(), rng, s, p: START, seen, uses: Vec::new(), hits: Vec::new() }
    }

    fn clock(&self) -> f32 {
        self.s.ab.clock
    }

    fn step(&mut self, dt: f32) {
        let (before, t) = (self.s.ability_uses(), self.clock());
        self.s.step_seen(&self.m, &mut self.rng, &self.walls, &mut self.p, 0.5, 0.0, t, dt, self.seen);
        for _ in before..self.s.ability_uses() {
            self.uses.push(t);
        }
    }

    fn hit(&mut self, damage: f32) {
        let t = self.clock();
        self.hits.push(t);
        self.s.on_hit(&self.m, &mut self.rng, damage, t);
    }
}

/// When a hit lands: at a time on the bot's clock, or that long after the one before (the first, after 0).
#[derive(Clone, Copy)]
enum When {
    At(f32),
    After(f32),
}

fn at(times: &[f32], damage: f32) -> Vec<(When, f32)> {
    times.iter().map(|t| (When::At(*t), damage)).collect()
}

/// Frames of 1/120 s with `None`, or between 1/240 and 1/60 s drawn with the seed.
fn frames(kind: Option<u64>) -> Box<dyn FnMut() -> f32> {
    match kind {
        None => Box::new(|| DT),
        Some(seed) => {
            let mut rng = Rng::new(seed);
            Box::new(move || 1.0 / (60.0 + 180.0 * rng.next_f32()))
        }
    }
}

const KINDS: [Option<u64>; 4] = [None, Some(1), Some(2), Some(3)];

fn longest_frame(kind: Option<u64>) -> f32 {
    if kind.is_none() { DT } else { 1.0 / 60.0 }
}

/// Plays until the clock reaches `until`, delivering each hit of the script at the first frame start at
/// or after its time.
fn play(m: &Motion, script: &[(When, f32)], kind: Option<u64>, seen: bool, until: f32) -> Rig {
    let mut rig = Rig::new(m.clone(), 7, seen);
    let mut frame = frames(kind);
    let mut next = 0;
    while rig.clock() < until {
        while let Some((when, damage)) = script.get(next) {
            let time = match when {
                When::At(t) => *t,
                When::After(d) => rig.hits.last().copied().unwrap_or(0.0) + d,
            };
            if rig.clock() + 1e-4 < time {
                break;
            }
            rig.hit(*damage);
            next += 1;
        }
        rig.step(frame());
    }
    rig
}

/// The uses come at the times `expect` works out from the clocks at which the hits were delivered, each
/// within one frame after its time, on every kind of frames.
#[track_caller]
fn check(name: &str, m: &Motion, script: &[(When, f32)], seen: bool, until: f32, expect: impl Fn(&[f32]) -> Vec<f32>) {
    for kind in KINDS {
        let rig = play(m, script, kind, seen, until);
        let want = expect(&rig.hits);
        let what = format!("{name}, frames {kind:?}: hits {:?}, uses {:?}, wanted {want:?}", rig.hits, rig.uses);
        assert_eq!(rig.uses.len(), want.len(), "{what}");
        for (got, want) in rig.uses.iter().zip(&want) {
            let late = got - want;
            assert!((-0.002..=longest_frame(kind) + 0.002).contains(&late), "{what}");
        }
    }
}

/// A reaction with a reset timer of 1 s, no chance to ignore it and a fixed delay.
fn react(threshold: f32, delay: f32) -> Reaction {
    Reaction { threshold, reset_s: 1.0, ignore_chance: 0.0, delay_s: [delay, delay] }
}

/// A level blink of 0.1 s with charges that come back at once, that reacts to damage.
fn reacting(r: Reaction) -> Ability {
    Ability {
        max_charges: 99,
        spawn_charges: 99,
        charge_s: 0.0,
        duration_s: 0.1,
        speed: 2.0,
        reaction: Some(r),
        ..Ability::default()
    }
}

/// A bot whose key ticks every `tick` seconds, with `chance` of pressing, no start block.
fn keyed(tick: f32, chance: f32, a: Ability) -> Motion {
    let abilities =
        Abilities { press_every_s: tick, chance, window_s: [1.0, 1.0], list: vec![a], ..Default::default() };
    Motion { abilities: Some(abilities), ..strafer() }
}

/// A bot that never presses its key.
fn quiet(a: Ability) -> Motion {
    keyed(0.05, 0.0, a)
}

/// A bot whose key ticks every `tick` seconds and always presses.
fn pressing(tick: f32, a: Ability) -> Motion {
    keyed(tick, 1.0, a)
}

fn plus(h: &[f32], delay: f32) -> Vec<f32> {
    h.iter().map(|c| c + delay).collect()
}

fn picks(h: &[f32], which: &[usize], delay: f32) -> Vec<f32> {
    which.iter().map(|i| h[*i] + delay).collect()
}

#[test]
fn a_threshold_of_zero_makes_every_hit_a_trigger() {
    let m = quiet(reacting(react(0.0, 0.2)));
    check("hits at 1, 2, 3", &m, &at(&[1.0, 2.0, 3.0], 1.0), true, 4.0, |h| plus(h, 0.2));
}

#[test]
fn a_trigger_comes_each_time_the_damage_since_the_last_one_reaches_the_threshold() {
    let m = quiet(reacting(react(50.0, 0.2)));
    let times: Vec<f32> = (0..15).map(|k| 1.0 + 0.1 * k as f32).collect();
    check("a trigger at every fifth hit", &m, &at(&times, 10.0), true, 3.5, |h| picks(h, &[4, 9, 14], 0.2));
}

/// The sum of a beam's per-tick damage in `f32` can fall a hair short of the threshold, and the slack makes
/// it reach it. 30 hits of 100/60 add up to a hair over 50 in `f32`, which needs no slack; 60 hits of
/// 50/60 add up to 49.99998, which does (without it the fourth trigger would come at hit 244, not 240).
#[test]
fn a_sum_a_hair_short_of_the_threshold_still_triggers() {
    let m = quiet(reacting(react(50.0, 0.05)));
    for (per_second, damage, hits) in [(60.0, 100.0 / 60.0, 120), (120.0, 50.0 / 60.0, 240)] {
        let times: Vec<f32> = (1..=hits).map(|k| 0.5 + k as f32 / per_second).collect();
        for kind in KINDS {
            let rig = play(&m, &at(&times, damage), kind, true, 3.5);
            assert_eq!((rig.hits.len(), rig.uses.len()), (hits, 4), "{per_second} hits a second, frames {kind:?}");
        }
    }
}

#[test]
fn the_damage_counted_is_forgotten_after_the_reset_timer() {
    let times: Vec<f32> = (0..12).map(|k| 1.0 + 0.3 * k as f32).collect();
    let long = quiet(reacting(react(50.0, 0.2)));
    check("a reset timer of 1 s", &long, &at(&times, 10.0), true, 5.5, |h| picks(h, &[4, 9], 0.2));
    let short = quiet(reacting(Reaction { reset_s: 0.25, ..react(50.0, 0.2) }));
    check("a reset timer of 0.25 s", &short, &at(&times, 10.0), true, 5.5, |_| Vec::new());
}

#[test]
fn a_trigger_is_ignored_by_chance_each_time_by_itself() {
    let sure = quiet(reacting(Reaction { ignore_chance: 1.0, ..react(0.0, 0.2) }));
    for kind in KINDS {
        let rig = play(&sure, &at(&[1.0, 2.0, 3.0], 1.0), kind, true, 4.0);
        assert!(rig.uses.is_empty(), "frames {kind:?}: {:?}", rig.uses);
    }
    let half = quiet(reacting(Reaction { ignore_chance: 0.5, ..react(0.0, 0.2) }));
    let times: Vec<f32> = (1..=400).map(|k| k as f32).collect();
    let mut runs = Vec::new();
    for kind in [None, Some(1)] {
        let rig = play(&half, &at(&times, 1.0), kind, true, 402.0);
        println!("ignore chance 0.5, frames {kind:?}: {} uses of 400 triggers", rig.uses.len());
        assert!((160..=240).contains(&rig.uses.len()), "frames {kind:?}: {}", rig.uses.len());
        runs.push(rig.uses);
    }
    let again = play(&half, &at(&times, 1.0), None, true, 402.0);
    assert_eq!(again.uses, runs[0], "the same seed, the same uses");
}

#[test]
fn the_delay_is_drawn_evenly_between_the_two_ends() {
    let m = quiet(reacting(Reaction { delay_s: [0.1, 0.9], ..react(0.0, 0.0) }));
    let times: Vec<f32> = (1..=200).map(|k| 2.0 * k as f32).collect();
    for kind in [None, Some(1)] {
        let rig = play(&m, &at(&times, 1.0), kind, true, 404.0);
        assert_eq!(rig.uses.len(), 200, "frames {kind:?}");
        let waits: Vec<f32> = rig.uses.iter().zip(&rig.hits).map(|(u, h)| u - h).collect();
        let (lo, hi) = waits.iter().fold((f32::MAX, f32::MIN), |(lo, hi), w| (lo.min(*w), hi.max(*w)));
        let mean = waits.iter().sum::<f32>() / waits.len() as f32;
        println!("delay 0.1 to 0.9, frames {kind:?}: shortest {lo:.3}, longest {hi:.3}, mean {mean:.3}");
        assert!(lo >= 0.1 - 0.002 && hi <= 0.9 + longest_frame(kind) + 0.002, "{lo} {hi}");
        assert!(lo < 0.2 && hi > 0.8 && (0.45..0.55).contains(&mean), "{lo} {hi} {mean}");
    }
}

#[test]
fn a_new_trigger_starts_the_wait_again_and_a_hit_below_the_threshold_doesnt() {
    let m = quiet(reacting(react(0.0, 0.5)));
    check("a second trigger", &m, &at(&[1.0, 1.3], 1.0), true, 3.0, |h| vec![h[1] + 0.5]);
    let m = quiet(reacting(react(20.0, 0.5)));
    let script = [(When::At(1.0), 20.0), (When::At(1.3), 5.0)];
    check("a hit below the threshold", &m, &script, true, 3.0, |h| vec![h[0] + 0.5]);
}

#[test]
fn a_reaction_with_no_charge_when_it_comes_due_is_dropped_and_doesnt_wait() {
    let one = Ability { max_charges: 1, spawn_charges: 1, charge_s: 3.0, ..reacting(react(0.0, 0.2)) };
    let m = quiet(one);
    // Used at 1.2 (the charge is back at 4.2); due at 2.2 with none: dropped; used at 5.2.
    check("one charge", &m, &at(&[1.0, 2.0, 5.0], 1.0), true, 6.5, |h| picks(h, &[0, 2], 0.2));
}

#[test]
fn the_delay_after_a_use_holds_a_reaction_back_and_a_tie_isnt_enough() {
    let m = quiet(Ability { delay_s: 1.0, ..reacting(react(0.0, 0.2)) });
    let first = (When::At(1.0), 1.0);
    check("due 1.02 s after", &m, &[first, (When::After(1.02), 1.0)], true, 4.0, |h| plus(h, 0.2));
    check("due 0.98 s after", &m, &[first, (When::After(0.98), 1.0)], true, 4.0, |h| vec![h[0] + 0.2]);
    // Exactly 1 s: only on even frames can a time be exact.
    let rig = play(&m, &[first, (When::After(1.0), 1.0)], None, true, 4.0);
    assert_eq!(rig.uses.len(), 1, "a tie is dropped: {:?}", rig.uses);
}

#[test]
fn a_reaction_due_while_an_ability_goes_on_or_as_it_ends_is_dropped() {
    let m = quiet(Ability { duration_s: 0.5, ..reacting(react(0.0, 0.2)) });
    let first = (When::At(1.0), 1.0);
    // Due 0.3 s into the blink: dropped. Due 0.8 s after the first, 0.3 s after it ended: used.
    let script = [first, (When::After(0.3), 1.0), (When::After(0.5), 1.0)];
    check("in the middle", &m, &script, true, 4.0, |h| vec![h[0] + 0.2, h[2] + 0.2]);
    // Due exactly at the scheduled end, 0.5 s after the first (even frames only).
    let rig = play(&m, &[first, (When::After(0.5), 1.0)], None, true, 4.0);
    assert_eq!(rig.uses.len(), 1, "a tie at the end is dropped: {:?}", rig.uses);
}

#[test]
fn the_reuse_timer_plays_no_part_in_a_reaction() {
    let m = quiet(Ability { reuse_s: 10.0, ..reacting(react(0.0, 0.2)) });
    check("reuse 10 s", &m, &at(&[1.0, 2.0], 1.0), true, 4.0, |h| plus(h, 0.2));
}

#[test]
fn a_reaction_and_the_key_draw_on_the_same_charges() {
    let two = Ability { max_charges: 2, spawn_charges: 2, charge_s: 10.0, ..reacting(react(0.0, 0.3)) };
    let m = pressing(1.0, two);
    // The tick at 1.0 uses one, the reaction at 1.5 the other, the tick at 2.0 finds none.
    check("shared charges", &m, &at(&[1.2], 1.0), true, 5.0, |h| vec![1.0, h[0] + 0.3]);
}

#[test]
fn a_reaction_due_before_the_start_block_is_over_is_dropped() {
    let mut m = quiet(reacting(react(0.0, 0.2)));
    m.abilities.as_mut().unwrap().start_block_s = 2.0;
    check("block of 2 s", &m, &at(&[1.0, 1.9], 1.0), true, 4.0, |h| vec![h[1] + 0.2]);
}

#[test]
fn the_abilitys_own_rules_hold_a_reaction_back() {
    let m = quiet(Ability { out_of_combat: false, ..reacting(react(0.0, 0.2)) });
    check("no line of sight", &m, &at(&[1.0], 1.0), false, 3.0, |_| Vec::new());
    check("line of sight", &m, &at(&[1.0], 1.0), true, 3.0, |h| plus(h, 0.2));
}

#[test]
fn a_bot_that_stands_still_until_hurt_counts_the_hit_that_wakes_it() {
    let m = Motion { still_until_hurt: true, ..quiet(reacting(react(10.0, 0.2))) };
    check("one hit of 10", &m, &at(&[1.0], 10.0), true, 3.0, |h| plus(h, 0.2));
    let m = Motion { still_until_hurt: true, ..quiet(reacting(react(20.0, 0.2))) };
    check("two hits of 10", &m, &at(&[1.0, 1.1], 10.0), true, 3.0, |h| vec![h[1] + 0.2]);
}

/// One charge that takes `charge_s` to come back, a key ticking every 1.0 s, a reaction hit at 0.7 with
/// a wait of `wait`, and one long frame from 0.85 to 1.35 that comes due for both the tick at 1.0 and the
/// reaction. Returns when the uses began.
fn one_long_frame(charge_s: f32, wait: f32) -> Vec<f32> {
    let one = Ability { max_charges: 1, spawn_charges: 1, charge_s, ..reacting(react(0.0, wait)) };
    let mut rig = Rig::new(pressing(1.0, one), 7, true);
    while rig.clock() + 1e-4 < 0.7 {
        rig.step(DT);
    }
    rig.hit(1.0);
    while rig.clock() + 1e-4 < 0.85 {
        rig.step(DT);
    }
    rig.step(0.5);
    while rig.clock() < 5.0 {
        rig.step(DT);
    }
    println!("one long frame, charge time {charge_s}, wait {wait}: uses began at {:?}", rig.uses);
    rig.uses
}

/// The ticks of the key and the reactions that came due in one frame go in the order of their times; on a
/// tie the tick goes first.
#[test]
fn ticks_and_reactions_due_in_one_frame_go_in_the_order_of_their_times() {
    let near = |got: &[f32], want: &[f32], what: &str| {
        assert_eq!(got.len(), want.len(), "{what}: {got:?}");
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 0.02, "{what}: {got:?}, wanted {want:?}");
        }
    };
    // Due 0.9: the reaction takes the charge, the tick at 1.0 finds none, the charge is back at 2.95 and
    // the tick at 3.0 uses it. (Tick first, the charge would be back at 3.05 and the next use at 4.0.)
    near(&one_long_frame(2.05, 0.2), &[1.35, 3.0], "reaction at 0.9");
    // Due 1.3: the tick at 1.0 takes the charge, the reaction finds none and is dropped, the charge is
    // back at 2.8 and the tick at 3.0 uses it. (Reaction first, back at 3.1 and the next use at 4.0.)
    near(&one_long_frame(1.8, 0.6), &[1.35, 3.0], "reaction at 1.3");
}

/// A key that ticks every 0 s ticks once a frame, at the frame's clock. A reaction that came due before
/// that clock goes before the tick. (In review the tick went first, and the reaction was dropped.)
#[test]
fn a_reaction_due_before_the_frames_clock_goes_before_the_tick_of_a_key_that_ticks_every_frame() {
    // One charge that takes 1.15 s to come back, and a tick is a try only when more than 1 s after the
    // last try. The first tick, at 0, uses the charge, so it is back at 1.15.
    let one = Ability { max_charges: 1, spawn_charges: 1, charge_s: 1.15, reuse_s: 1.0, ..reacting(react(0.0, 0.6)) };
    let mut rig = Rig::new(pressing(0.0, one), 7, true);
    while rig.clock() + 1e-4 < 0.7 {
        rig.step(DT);
    }
    rig.hit(1.0);
    while rig.clock() + 1e-4 < 0.95 {
        rig.step(DT);
    }
    // One long frame from 0.95 to 1.75. The frame that starts at 1.75 has the reaction, due at 1.3, and the
    // key's next try.
    rig.step(0.8);
    while rig.clock() < 3.5 {
        rig.step(DT);
    }
    // The reaction at 1.3 takes the charge (back at 2.45), the try at 1.75 finds none, and the next try,
    // the first frame after 2.75, uses it. (Tick first, the charge would be back at 2.9 and the next use
    // would come after 3.75.)
    let want = [0.0, 1.75, 2.76];
    assert_eq!(rig.uses.len(), want.len(), "{:?}", rig.uses);
    for (got, want) in rig.uses.iter().zip(want) {
        assert!((got - want).abs() < 0.02, "{:?}, wanted {want:?}", rig.uses);
    }
}

/// Two abilities' reactions that came due in one frame go in the order of their times, whatever their
/// order in the bot's list.
#[test]
fn two_abilities_reactions_due_in_one_frame_go_in_the_order_of_their_times() {
    // Each has one charge that doesn't come back in this test. The second in the list waits 0.2 s and lasts
    // 0.3 s; the first waits 0.3 s.
    let later = Ability { max_charges: 1, spawn_charges: 1, charge_s: 60.0, ..reacting(react(0.0, 0.3)) };
    let sooner = Ability { duration_s: 0.3, reaction: Some(react(0.0, 0.2)), ..later.clone() };
    // The key never presses and ticks every 10 s, so no tick falls between the two reactions. (A tick
    // between them would split them up, and each would then be handled by itself.)
    let mut m = keyed(10.0, 0.0, later);
    m.abilities.as_mut().unwrap().list.push(sooner);
    let mut rig = Rig::new(m, 7, true);
    while rig.clock() + 1e-4 < 0.7 {
        rig.step(DT);
    }
    rig.hit(1.0);
    while rig.clock() + 1e-4 < 0.85 {
        rig.step(DT);
    }
    // One long frame from 0.85 to 1.35: the second is due at 0.9 and is used, and the first, due at 1.0
    // while that goes on, is dropped.
    rig.step(0.5);
    while rig.clock() + 1e-4 < 3.0 {
        rig.step(DT);
    }
    // A second hit at 3.0: the second ability, due at 3.2, has no charge, and the first is used at 3.3.
    // (In list order the first would have gone at 1.0, and the second at 3.2.)
    rig.hit(1.0);
    while rig.clock() < 4.5 {
        rig.step(DT);
    }
    let want = [1.35, 3.3];
    assert_eq!(rig.uses.len(), want.len(), "{:?}", rig.uses);
    for (got, want) in rig.uses.iter().zip(want) {
        assert!((got - want).abs() < 0.02, "{:?}, wanted {want:?}", rig.uses);
    }
}

/// A tick of the key and a reaction that fall due at the same moment: the tick goes first. (Not measured
/// in KovaaK's; this pins what the trainer plays.)
#[test]
fn a_tick_goes_before_a_reaction_due_at_the_same_moment() {
    // The key's ability comes first in the list and lasts 0.1 s. The one that reacts lasts 1.5 s. Each has
    // one charge that doesn't come back in this test. The key ticks every 1.0 s and always presses.
    let keys =
        Ability { max_charges: 1, spawn_charges: 1, charge_s: 60.0, reaction: None, ..reacting(react(0.0, 0.3)) };
    let reacts = Ability { duration_s: 1.5, reaction: Some(react(0.0, 0.3)), ..keys.clone() };
    let mut m = pressing(1.0, keys);
    m.abilities.as_mut().unwrap().list.push(reacts);
    let mut rig = Rig::new(m, 7, true);
    while rig.clock() + 1e-4 < 0.7 {
        rig.step(DT);
    }
    // A hit at 0.7: the reaction is due at 1.0, with the key's tick.
    rig.hit(1.0);
    while rig.clock() + 1e-4 < 0.85 {
        rig.step(DT);
    }
    // One long frame from 0.85 to 1.35, so both fall due in the frame that starts at 1.35.
    rig.step(0.5);
    while rig.clock() < 4.5 {
        rig.step(DT);
    }
    // The tick uses the key's ability at 1.0 and the reaction is dropped. The tick at 2.0 finds the key's
    // ability empty and uses the other. (Reaction first, the long blink would go on until 2.5, and the
    // key's ability would wait for the tick at 3.0.)
    let want = [1.35, 2.0];
    assert_eq!(rig.uses.len(), want.len(), "{:?}", rig.uses);
    for (got, want) in rig.uses.iter().zip(want) {
        assert!((got - want).abs() < 0.02, "{:?}, wanted {want:?}", rig.uses);
    }
}

/// A trigger that is ignored leaves a reaction that is already waiting alone. (Not measured in KovaaK's;
/// this pins what the trainer plays.)
#[test]
fn an_ignored_trigger_leaves_a_waiting_reaction_alone() {
    let m = quiet(reacting(Reaction { ignore_chance: 0.5, ..react(0.0, 0.5) }));
    // 200 pairs of hits 0.1 s apart, a pair every 2 s. When the first of a pair is kept and the second is
    // ignored, the use comes 0.5 s after the first: about a quarter of the pairs. When the second is kept
    // it comes 0.6 s after the first: about half. (If an ignored trigger called off the one waiting, no
    // use would come 0.5 s after a first hit.)
    let times: Vec<f32> = (0..200).flat_map(|k| [2.0 * k as f32 + 1.0, 2.0 * k as f32 + 1.1]).collect();
    for kind in [None, Some(1)] {
        let rig = play(&m, &at(&times, 1.0), kind, true, 402.0);
        let after_first = |u: &f32| u - rig.hits[2 * ((u - 1.0) / 2.0).floor() as usize];
        let early = rig.uses.iter().filter(|u| after_first(u) < 0.55).count();
        println!("pairs of hits, frames {kind:?}: {} uses, {early} of them 0.5 s after the first hit", rig.uses.len());
        assert!((30..=70).contains(&early), "frames {kind:?}: {early}");
        assert!((130..=170).contains(&rig.uses.len()), "frames {kind:?}: {}", rig.uses.len());
    }
}

/// An ability without a reaction takes no notice of hits.
#[test]
fn an_ability_without_a_reaction_ignores_hits() {
    let none = Ability { reaction: None, ..reacting(react(0.0, 0.2)) };
    let times: Vec<f32> = (1..=20).map(|k| k as f32 * 0.5).collect();
    for kind in KINDS {
        let rig = play(&quiet(none.clone()), &at(&times, 100.0), kind, true, 12.0);
        assert!(rig.uses.is_empty(), "frames {kind:?}: {:?}", rig.uses);
    }
}

#[test]
fn a_reaction_is_saved_only_when_there_is_one_and_its_values_are_checked() {
    let plain = quiet(Ability::default());
    let text = toml::to_string(&plain).unwrap();
    assert!(!text.contains("reaction"), "{text}");
    let some = Reaction { threshold: 30.0, reset_s: 0.4, ignore_chance: 0.25, delay_s: [0.1, 0.3] };
    let with_one = quiet(reacting(some));
    let text = toml::to_string(&with_one).unwrap();
    assert!(text.contains("reaction"), "{text}");
    let back: Motion = toml::from_str(&text).unwrap();
    assert_eq!(back, with_one);
    // Out of range values are problems; the ends are fine.
    let abilities = |r: Reaction| Abilities { list: vec![reacting(r)], ..Abilities::default() };
    assert!(abilities(react(30.0, 0.3)).problems().is_empty());
    let ends = Reaction { threshold: 1.0e6, reset_s: 3600.0, ignore_chance: 1.0, delay_s: [0.0, 3600.0] };
    assert!(abilities(ends).problems().is_empty());
    for bad in [
        Reaction { threshold: -1.0, ..react(0.0, 0.2) },
        Reaction { threshold: f32::NAN, ..react(0.0, 0.2) },
        Reaction { threshold: 2.0e6, ..react(0.0, 0.2) },
        Reaction { reset_s: -0.1, ..react(0.0, 0.2) },
        Reaction { reset_s: 4000.0, ..react(0.0, 0.2) },
        Reaction { ignore_chance: 1.5, ..react(0.0, 0.2) },
        Reaction { ignore_chance: -0.5, ..react(0.0, 0.2) },
        Reaction { delay_s: [-1.0, 0.2], ..react(0.0, 0.2) },
        Reaction { delay_s: [0.1, f32::INFINITY], ..react(0.0, 0.2) },
    ] {
        assert!(!abilities(bad).problems().is_empty(), "{bad:?}");
    }
}
