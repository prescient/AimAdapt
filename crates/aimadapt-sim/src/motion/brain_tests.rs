//! Tests for what a bot's abilities keep when it respawns (see [`Brain`]). They all guard against one
//! bug: a bot's ability timers started again when it respawned. Measured in KovaaK's, the start block, each
//! ability's reuse clock and the frequency windows carry over from one life of a bot to the next, while the
//! tick grid of the key and the charges start again.
//!
//! A test plays a list of lives, a "life" being one bot from its spawn to its death (see
//! [`play_lives`]). Each runs on even frames of 1/120 s and on uneven ones between 1/240 and 1/60 s. A life
//! ends at the first frame at or after its length, so where the expected times depend on when it ended they
//! are worked out from the clocks the test saw.

use glam::Vec3;

use super::ability_tests::{FRAME_KINDS, frame_lengths};
use super::tests::{DT, arena, strafer};
use super::*;
use crate::collide::Collider;
use crate::rng::Rng;

const START: Vec3 = Vec3::new(0.0, -1.5, -10.0);

/// The bot's key ticks this often unless a test says otherwise.
const TICK: f32 = 0.1;

/// An ability with plenty of charges that come back at once, no duration, no delay and no reuse time.
fn plenty() -> Ability {
    Ability { max_charges: 99, spawn_charges: 99, charge_s: 0.0, ..Ability::default() }
}

/// A bot that always presses its key every `TICK`, with no block, with these abilities.
fn bot(list: Vec<Ability>, tweak: impl FnOnce(&mut Abilities)) -> Motion {
    let mut ab = Abilities { press_every_s: TICK, chance: 1.0, window_s: [1.0, 1.0], list, ..Abilities::default() };
    tweak(&mut ab);
    Motion { abilities: Some(ab), ..strafer() }
}

/// One life of a bot, as [`play_lives`] saw it.
struct Life {
    /// The sim time at which its clock was 0.
    began: f32,
    /// Its clock when it left.
    left: f32,
    /// Its clock at the start of the frame in which each use began.
    uses: Vec<f32>,
}

/// Plays lives one after the other, each a length (it ends at the first frame at or after it) and the time
/// away before the next bot appears. The brain of each bot goes to the next, as the sim does.
fn play_lives(m: &Motion, lives: &[(f32, f32)], seed: Option<u64>) -> Vec<Life> {
    let walls = arena();
    let mut rng = Rng::new(7);
    let mut frame = frame_lengths(seed);
    let (mut brain, mut away, mut began) = (None, 0.0, 0.0);
    let mut out = Vec::new();
    for &(length, gap) in lives {
        let mut s = BotState::reborn(m, &mut rng, START, Vec3::X, brain.take(), away);
        let (mut p, mut seen_uses, mut uses) = (START, 0, Vec::new());
        while s.ab.clock < length {
            let t = s.ab.clock;
            s.step_seen(m, &mut rng, &walls, &mut p, 0.5, 0.0, t, frame(), true);
            while seen_uses < s.ability_uses() {
                seen_uses += 1;
                uses.push(t);
            }
        }
        let left = s.ab.clock;
        out.push(Life { began, left, uses });
        began += left + gap;
        away = gap;
        brain = s.take_brain(m);
    }
    out
}

/// The longest frame of this kind of timeline.
fn longest(seed: Option<u64>) -> f32 {
    if seed.is_none() { DT } else { 1.0 / 60.0 }
}

/// Whether a use began in the frame of the tick at `tick`: at its time or up to a frame later.
fn on_tick(t: f32, tick: f32, seed: Option<u64>) -> bool {
    (-0.002..=longest(seed) + 0.002).contains(&(t - tick))
}

#[track_caller]
fn assert_uses(what: &str, got: &[f32], want: &[f32], seed: Option<u64>) {
    let fits = got.len() == want.len() && got.iter().zip(want).all(|(g, w)| on_tick(*g, *w, seed));
    assert!(fits, "{what}, frames {seed:?}: uses at {got:?}, wanted {want:?}");
}

/// The first use of a life, as a list of at most one.
fn first(life: &Life) -> &[f32] {
    &life.uses[..life.uses.len().min(1)]
}

/// The first tick of the key, at 1, 2, ... times `TICK`, that is at or after `end`, a tie counting as at.
fn first_tick_at_or_after(end: f32) -> f32 {
    (1..).map(|k| k as f32 * TICK).find(|t| *t + 1e-3 >= end).unwrap()
}

/// The first tick that is a try when the last one was at `last`: strictly more than `reuse` after it.
fn first_try_after(last: f32, reuse: f32) -> f32 {
    (1..).map(|k| k as f32 * TICK).find(|t| t - last > reuse + 1e-3).unwrap()
}

/// A: the start block counts once, from the first bot. Block 3.0: the first bot's first use is at 3.0, and
/// the next ones use at their first tick.
#[test]
fn the_start_block_counts_once_from_the_first_bot() {
    let m = bot(vec![plenty()], |a| a.start_block_s = 3.0);
    for seed in FRAME_KINDS {
        let lives = play_lives(&m, &[(4.0, 0.5), (1.0, 0.5), (1.0, 0.5)], seed);
        assert_uses("life 1", first(&lives[0]), &[3.0], seed);
        assert_uses("life 2", first(&lives[1]), &[0.1], seed);
        assert_uses("life 3", first(&lives[2]), &[0.1], seed);
    }
}

/// B: a block still running at a respawn. Block 3.0, the first bot leaves at 1.0 and is away 0.45, so the
/// block ends 1.55 into the next life and its first use is at its tick at 1.6.
#[test]
fn a_block_still_running_when_a_bot_comes_back_ends_on_the_first_bots_clock() {
    let m = bot(vec![plenty()], |a| a.start_block_s = 3.0);
    for seed in FRAME_KINDS {
        let lives = play_lives(&m, &[(1.0, 0.45), (2.0, 0.0)], seed);
        assert!(lives[0].uses.is_empty(), "{seed:?}: {:?}", lives[0].uses);
        // Worked out from the clock the first bot left at: the block ends 3.0 - (that + 0.45) into the life.
        let end = 3.0 - (lives[0].left + 0.45);
        let want = first_tick_at_or_after(end);
        assert!((want - 1.6).abs() < 1e-3, "{seed:?}: the first tick after {end} is {want}");
        assert_uses("life 2", first(&lives[1]), &[want], seed);
    }
}

/// C: each ability's reuse clock is carried. Reuse 2.0: tries in life one at 0.1 and 2.2. It leaves at 2.45
/// (the brief says 2.5: 2.45 keeps the expected times off the tie that frames on a tie's edge would decide),
/// away 0.5. The last try was 0.8 before life two began, so its first use is at its tick at 1.3 and the next
/// one 2.1 later.
#[test]
fn the_reuse_clock_runs_on_through_a_death_and_a_respawn() {
    let m = bot(vec![Ability { reuse_s: 2.0, ..plenty() }], |_| {});
    for seed in FRAME_KINDS {
        let lives = play_lives(&m, &[(2.45, 0.5), (3.7, 0.0)], seed);
        assert_uses("life 1", &lives[0].uses, &[0.1, 2.2], seed);
        // The last try, 2.2, was this long before life two began.
        let last = 2.2 - (lives[0].left + 0.5);
        let first = first_try_after(last, 2.0);
        let second = first_try_after(first, 2.0);
        assert!((first - 1.3).abs() < 1e-3 && (second - 3.4).abs() < 1e-3, "{seed:?}: {first}, {second}");
        assert_uses("life 2", &lives[1].uses, &[first, second], seed);
    }
}

/// The edges of a stretch of pressing: for each life the ticks it pressed at, as sim times.
fn pressing(lives: &[Life], seed: Option<u64>) -> Vec<Vec<(f32, bool)>> {
    lives
        .iter()
        .map(|l| {
            let ticks = (1..).take_while(|k| (*k as f32) * TICK <= 3.2);
            ticks
                .map(|k| {
                    let at = k as f32 * TICK;
                    (l.began + at, l.uses.iter().any(|u| on_tick(*u, at, seed)))
                })
                .collect()
        })
        .collect()
}

/// D: the frequency windows are carried. Chance 0.5 with windows of exactly 2.0 s, lives of 3.3 s with 0.5 s
/// away, for at least 60 s: every edge at which the pressing stops or starts inside a life falls within a
/// tick of a multiple of 2.0 s of sim time from the first bot's spawn, in every life, and where a window
/// runs across a respawn the state is the same on both sides.
#[test]
fn the_frequency_windows_run_on_through_deaths() {
    let m = bot(vec![plenty()], |a| {
        a.chance = 0.5;
        a.window_s = [2.0, 2.0];
    });
    for seed in FRAME_KINDS {
        let lives = play_lives(&m, &[(3.3, 0.5); 17], seed);
        assert!(lives.last().unwrap().began > 60.0 - 3.8, "at least 60 s of sim time");
        let marks = pressing(&lives, seed);
        let (mut edges, mut on, mut off, mut across) = (0, 0, 0, 0);
        for life in &marks {
            for w in life.windows(2) {
                let (_, before) = w[0];
                let (at, now) = w[1];
                if before != now {
                    edges += 1;
                    let into = (at + 0.002).rem_euclid(2.0);
                    assert!(into < TICK + 0.004, "{seed:?}: an edge at {at}, {into} after a multiple of 2.0");
                }
            }
            on += life.iter().filter(|m| m.1).count();
            off += life.iter().filter(|m| !m.1).count();
        }
        // A window that runs across a respawn: the last tick of one life and the first of the next are in
        // the same window, and the state is as it was.
        for pair in marks.windows(2) {
            let (last, first) = (pair[0].last().unwrap(), pair[1][0]);
            let window = |t: f32| ((t + 1e-3) / 2.0).floor();
            if window(last.0) == window(first.0) {
                across += 1;
                assert_eq!(last.1, first.1, "{seed:?}: a window across a respawn, at {} and {}", last.0, first.0);
            }
        }
        assert!(edges >= 6, "{seed:?}: {edges} edges");
        assert!(on > 50 && off > 50, "{seed:?}: both kinds of stretch: {on} pressed, {off} not");
        assert!(across >= 3, "{seed:?}: {across} windows across a respawn");
        // The same seed gives the same uses.
        let again = play_lives(&m, &[(3.3, 0.5); 17], seed);
        let uses = |l: &[Life]| l.iter().map(|l| l.uses.clone()).collect::<Vec<_>>();
        assert_eq!(uses(&lives), uses(&again), "{seed:?}");
    }
}

/// E: the tick grid starts again with each bot. A tick of 1.0 gives uses at 1.0 and 2.0 in each life (this
/// holds on the old code too: it guards against the grid being carried).
#[test]
fn the_tick_grid_starts_again_with_each_bot() {
    let m = bot(vec![plenty()], |a| a.press_every_s = 1.0);
    for seed in FRAME_KINDS {
        for (i, life) in play_lives(&m, &[(2.5, 0.7); 3], seed).iter().enumerate() {
            assert_uses(&format!("life {}", i + 1), &life.uses, &[1.0, 2.0], seed);
        }
    }
}

/// F: the charges start again with each bot (this holds on the old code too: it guards against the charges
/// being carried). With no charge on spawn and a charge time of 2.0 the first use of each life is 2.0 after
/// its spawn. With one charge on spawn and a charge time of 10 a charge spent in life one is there again at
/// the first tick of life two.
#[test]
fn the_charges_start_again_with_each_bot() {
    let none = Ability { max_charges: 1, spawn_charges: 0, charge_s: 2.0, ..Ability::default() };
    let m = bot(vec![none], |_| {});
    let one = Ability { max_charges: 1, spawn_charges: 1, charge_s: 10.0, ..Ability::default() };
    let n = bot(vec![one], |_| {});
    for seed in FRAME_KINDS {
        for (i, life) in play_lives(&m, &[(2.5, 0.5); 3], seed).iter().enumerate() {
            assert_uses(&format!("no charges, life {}", i + 1), &life.uses, &[2.0], seed);
        }
        for (i, life) in play_lives(&n, &[(1.0, 0.5); 2], seed).iter().enumerate() {
            assert_uses(&format!("one charge, life {}", i + 1), &life.uses, &[0.1], seed);
        }
    }
}

/// G: `DelayAfterUse` and the shared cooldown start again with each bot. Not measured in KovaaK's: that is a
/// decision. With a delay of 5.0, life one uses at 0.1 and leaves at 1.0, and life two uses at its first
/// tick; the same for a shared cooldown of 5.0.
#[test]
fn the_delay_after_a_use_and_the_shared_cooldown_start_again_with_each_bot() {
    let delayed = bot(vec![Ability { delay_s: 5.0, ..plenty() }], |_| {});
    let cooled = bot(vec![Ability { duration_s: 0.1, ..plenty() }], |a| a.global_cooldown_s = 5.0);
    for seed in FRAME_KINDS {
        for (name, m) in [("delay", &delayed), ("shared cooldown", &cooled)] {
            for (i, life) in play_lives(m, &[(1.0, 0.5); 2], seed).iter().enumerate() {
                assert_uses(&format!("{name}, life {}", i + 1), &life.uses, &[0.1], seed);
            }
        }
    }
}

/// A bot stepped by hand until its clock reaches `to`.
fn run_to(s: &mut BotState, m: &Motion, rng: &mut Rng, walls: &Collider, p: &mut Vec3, to: f32) {
    while s.ab.clock < to {
        let t = s.ab.clock;
        s.step_seen(m, rng, walls, p, 0.5, 0.0, t, DT, true);
    }
}

/// H: an ability going on when the bot leaves ends with the bot (not measured: a decision). The new bot
/// isn't using one, and isn't moved by one.
#[test]
fn an_ability_going_on_when_its_bot_leaves_doesnt_carry() {
    let dash = Ability { duration_s: 5.0, speed: 8.0, ..plenty() };
    let m = bot(vec![dash], |a| a.press_every_s = 1.0);
    let (walls, mut rng) = (arena(), Rng::new(7));
    let mut old = BotState::new(&m, &mut rng, START, Vec3::X);
    let mut p = START;
    run_to(&mut old, &m, &mut rng, &walls, &mut p, 2.0);
    assert!(old.using_ability(), "the dash from the tick at 1.0 is still going on");
    let brain = old.take_brain(&m);
    let mut new = BotState::reborn(&m, &mut rng, START, Vec3::X, brain, 0.5);
    assert!(!new.using_ability());
    let mut p = START;
    while new.ab.clock < 0.9 {
        let t = new.ab.clock;
        new.step_seen(&m, &mut rng, &walls, &mut p, 0.5, 0.0, t, DT, true);
        let flat = Vec3::new(new.vel.x, 0.0, new.vel.z).length();
        assert!(!new.using_ability() && flat <= 4.01, "at {t}: {flat} m/s");
    }
}

/// A bot that presses no key but uses its ability when hurt, after `delay` seconds.
fn flincher(threshold: f32, delay: f32) -> Motion {
    let reaction = Reaction { threshold, reset_s: 100.0, ignore_chance: 0.0, delay_s: [delay, delay] };
    bot(vec![Ability { reaction: Some(reaction), ..plenty() }], |a| a.chance = 0.0)
}

/// I: a waiting reaction and the damage counted toward one are dropped when the bot leaves (not measured: a
/// decision). A trigger in life one whose wait runs past its death gives no use in life two, and with a
/// threshold of 50, 40 damage in life one and 10 in life two don't add up to a trigger.
#[test]
fn a_waiting_reaction_and_its_damage_are_dropped_when_the_bot_leaves() {
    let (walls, mut rng) = (arena(), Rng::new(7));
    // A wait that runs past the death: hit at 0.5, due at 2.5, the bot leaves at 1.0, away 0.5.
    let m = flincher(0.0, 2.0);
    let mut old = BotState::new(&m, &mut rng, START, Vec3::X);
    let mut p = START;
    run_to(&mut old, &m, &mut rng, &walls, &mut p, 0.5);
    old.on_hit(&m, &mut rng, 10.0, 0.5);
    run_to(&mut old, &m, &mut rng, &walls, &mut p, 1.0);
    let mut new = BotState::reborn(&m, &mut rng, START, Vec3::X, old.take_brain(&m), 0.5);
    let mut p = START;
    run_to(&mut new, &m, &mut rng, &walls, &mut p, 3.0);
    assert_eq!(new.ability_uses(), 0, "the old bot's wait ended with it");
    // The reaction works in this motion: a hit now is answered 2.0 s later.
    new.on_hit(&m, &mut rng, 10.0, 3.0);
    run_to(&mut new, &m, &mut rng, &walls, &mut p, 5.1);
    assert_eq!(new.ability_uses(), 1);

    // Damage counted: 40 and then 10 against a threshold of 50.
    let m = flincher(50.0, 0.1);
    let mut old = BotState::new(&m, &mut rng, START, Vec3::X);
    let mut p = START;
    run_to(&mut old, &m, &mut rng, &walls, &mut p, 0.5);
    old.on_hit(&m, &mut rng, 40.0, 0.5);
    run_to(&mut old, &m, &mut rng, &walls, &mut p, 1.0);
    let mut new = BotState::reborn(&m, &mut rng, START, Vec3::X, old.take_brain(&m), 0.5);
    let mut p = START;
    run_to(&mut new, &m, &mut rng, &walls, &mut p, 0.3);
    new.on_hit(&m, &mut rng, 10.0, 0.3);
    run_to(&mut new, &m, &mut rng, &walls, &mut p, 1.5);
    assert_eq!(new.ability_uses(), 0, "40 in life one and 10 in life two aren't 50");
    // The threshold can be reached: 40 and 10 in the same life.
    let mut one = BotState::new(&m, &mut rng, START, Vec3::X);
    let mut p = START;
    run_to(&mut one, &m, &mut rng, &walls, &mut p, 0.3);
    one.on_hit(&m, &mut rng, 40.0, 0.3);
    one.on_hit(&m, &mut rng, 10.0, 0.3);
    run_to(&mut one, &m, &mut rng, &walls, &mut p, 1.5);
    assert_eq!(one.ability_uses(), 1);
}

/// The start block holds a reaction on the first bot's clock too. Block 3.0 and a bot that is away 0.5 s: a
/// hit 0.3 s into the life of a bot that came back after the block was over is answered, and one in a life
/// that began while the block was still running isn't (the answer is due 1.9 s into the run).
#[test]
fn a_reaction_is_held_by_the_block_on_the_first_bots_clock() {
    let mut m = flincher(0.0, 0.1);
    m.abilities.as_mut().unwrap().start_block_s = 3.0;
    let (walls, mut rng) = (arena(), Rng::new(7));
    for (first_life, uses) in [(4.0, 1), (1.0, 0)] {
        let mut old = BotState::new(&m, &mut rng, START, Vec3::X);
        let mut p = START;
        run_to(&mut old, &m, &mut rng, &walls, &mut p, first_life);
        let mut new = BotState::reborn(&m, &mut rng, START, Vec3::X, old.take_brain(&m), 0.5);
        let mut p = START;
        run_to(&mut new, &m, &mut rng, &walls, &mut p, 0.3);
        new.on_hit(&m, &mut rng, 10.0, 0.3);
        run_to(&mut new, &m, &mut rng, &walls, &mut p, 1.0);
        assert_eq!(new.ability_uses(), uses, "a first life of {first_life} s");
    }
}

/// J: no random number is drawn for the abilities at a respawn. After `reborn` with a motion that has
/// abilities the random stream stands where it stands after `new` with the same motion without them. A bot
/// without abilities draws what it drew before and has no brain.
#[test]
fn a_respawn_draws_no_random_numbers_for_the_abilities() {
    let with = bot(vec![plenty()], |a| a.start_block_s = 1.0);
    let without = Motion { abilities: None, ..with.clone() };
    let mut old = BotState::new(&with, &mut Rng::new(3), START, Vec3::X);
    let (walls, mut rng) = (arena(), Rng::new(3));
    let mut p = START;
    run_to(&mut old, &with, &mut rng, &walls, &mut p, 1.0);
    let brain = old.take_brain(&with);
    assert!(brain.is_some());

    let (mut a, mut b) = (Rng::new(5), Rng::new(5));
    let _ = BotState::reborn(&with, &mut a, START, Vec3::X, brain.clone(), 0.5);
    let _ = BotState::new(&without, &mut b, START, Vec3::X);
    assert_eq!((a.next_f32(), a.next_f32()), (b.next_f32(), b.next_f32()));
    // `new` does draw for abilities, so the test can tell.
    let mut c = Rng::new(5);
    let _ = BotState::new(&with, &mut c, START, Vec3::X);
    assert_ne!(c.next_f32(), Rng::new(5).next_f32());

    // A bot without abilities: the same bot and the same draws as `new`, and no brain.
    let (mut a, mut b) = (Rng::new(9), Rng::new(9));
    let born = BotState::reborn(&without, &mut a, START, Vec3::X, brain, 0.5);
    assert_eq!(born, BotState::new(&without, &mut b, START, Vec3::X));
    assert_eq!(a.next_f32(), b.next_f32());
    assert!(born.clone().take_brain(&without).is_none());
}

/// K: a brain that doesn't fit the motion (another number of abilities, or none) gives a fresh state, as
/// `new` does.
#[test]
fn a_brain_that_doesnt_fit_gives_a_fresh_state() {
    let two = bot(vec![plenty(), plenty()], |a| a.start_block_s = 2.0);
    let one = bot(vec![plenty()], |a| a.start_block_s = 2.0);
    let none = Motion { abilities: None, ..one.clone() };
    let (walls, mut rng) = (arena(), Rng::new(7));
    let brain_of = |rng: &mut Rng| {
        let mut old = BotState::new(&two, rng, START, Vec3::X);
        let mut p = START;
        run_to(&mut old, &two, rng, &walls, &mut p, 1.5);
        old.take_brain(&two)
    };
    for m in [&one, &none] {
        let brain = brain_of(&mut rng);
        let (mut a, mut b) = (Rng::new(11), Rng::new(11));
        let born = BotState::reborn(m, &mut a, START, Vec3::X, brain, 4.0);
        assert_eq!(born, BotState::new(m, &mut b, START, Vec3::X));
        assert_eq!(a.next_f32(), b.next_f32());
    }
    // One that fits is carried: it isn't the fresh state.
    let brain = brain_of(&mut rng);
    let (mut a, mut b) = (Rng::new(11), Rng::new(11));
    let born = BotState::reborn(&two, &mut a, START, Vec3::X, brain, 4.0);
    assert_ne!(born, BotState::new(&two, &mut b, START, Vec3::X));
}
