//! Tests for a bot whose one ability is a weapon that blows the bot itself up (plan K8a, see [`Fuse`]). All
//! numbers are made up; the rules they check were measured in KovaaK's 3.9.13 on 12 November 2026.
//!
//! Each timeline runs on even frames of 1/120 s and on several runs of uneven frames between 1/240 and 1/60 s.
//! The sim steps once a frame with the real frame time, so a timer that only works on even frames is a bug.
//! The times of the shots are the schedule's own (a tick of the key, or the time the weapon is ready), so they
//! must come out the same on every kind of frame; a blast lands in the first frame that starts at or after its
//! time.

use glam::Vec3;

use super::ability_tests::{FRAME_KINDS, frame_lengths};
use super::tests::arena;
use super::*;
use crate::rng::Rng;

const START: Vec3 = Vec3::new(0.0, -1.5, -10.0);

/// A still bot that presses its key every `tick` s from the start (no block, always willing) with one weapon
/// ability, whose weapon fires every `shot_every` s and whose blast lands `burn` s after the shot and takes
/// `damage`.
fn fuse_bot(tick: f32, shot_every: f32, burn: f32, damage: f32, charge_s: f32) -> Motion {
    let ability = Ability {
        max_charges: 1,
        spawn_charges: 1,
        charge_s,
        fuse: Some(Fuse { shot_every_s: shot_every, burn_s: burn, damage }),
        ..Ability::default()
    };
    let abilities = Abilities {
        press_every_s: tick,
        chance: 1.0,
        window_s: [1.0, 1.0],
        start_block_s: 0.0,
        list: vec![ability],
        ..Abilities::default()
    };
    Motion { stays_put: true, abilities: Some(abilities), ..Motion::default() }
}

/// One life of a bot as the test saw it.
struct Life {
    bot: BotState,
    /// When each shot was scheduled, on the bot's clock.
    shots: Vec<f32>,
    /// The clock at the start of each frame in which health was taken off, and how much.
    landed: Vec<(f32, f32)>,
    /// How many times the bot used its ability.
    uses: u32,
}

impl Life {
    fn left(&self) -> f32 {
        self.bot.ab.clock
    }
}

/// Plays one bot from its appearance until its clock reaches `until`, `away_s` after the bot whose brain it
/// takes (see [`BotState::reborn`]). Checks that it never moves.
fn live(
    m: &Motion,
    rng: &mut Rng,
    brain: Option<Brain>,
    away_s: f32,
    until: f32,
    frame: &mut dyn FnMut() -> f32,
) -> Life {
    let walls = arena();
    let mut bot = BotState::reborn(m, rng, START, Vec3::X, brain, away_s);
    let mut pos = START;
    let (mut shots, mut landed) = (Vec::new(), Vec::new());
    let mut last_shot = bot.ab.slots[0].last_shot;
    while bot.ab.clock < until {
        let before = bot.ab.clock;
        bot.step_seen(m, rng, &walls, &mut pos, 0.5, 0.0, before, frame(), true);
        assert_eq!(pos, START, "a bot that stays put moved");
        if bot.ab.slots[0].last_shot != last_shot {
            last_shot = bot.ab.slots[0].last_shot;
            shots.extend(last_shot);
        }
        let owed = bot.take_owed_health();
        if owed > 0.0 {
            landed.push((before, owed));
        }
    }
    let uses = bot.ability_uses();
    Life { bot, shots, landed, uses }
}

/// The longest frame of this kind of timeline.
fn longest(seed: Option<u64>) -> f32 {
    if seed.is_none() { 1.0 / 120.0 } else { 1.0 / 60.0 }
}

#[track_caller]
fn assert_shots(what: &str, got: &[f32], want: &[f32], seed: Option<u64>) {
    let fits = got.len() == want.len() && got.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.002);
    assert!(fits, "{what}, frames {seed:?}: shots at {got:?}, wanted {want:?}");
}

/// The blasts landed in the frame that starts at or after each time (up to a frame later), each for `damage`.
#[track_caller]
fn assert_landed(what: &str, got: &[(f32, f32)], want: &[f32], damage: f32, seed: Option<u64>) {
    let fits = got.len() == want.len()
        && got.iter().zip(want).all(|((at, d), w)| (-0.002..longest(seed) + 0.002).contains(&(at - w)) && *d == damage);
    assert!(fits, "{what}, frames {seed:?}: health taken at {got:?}, wanted {want:?} for {damage}");
}

/// A: the base life. Health 3 and a blast of 2 are the sim's; here the weapon is 0.6 s between shots and 0.05 s
/// from shot to blast, with a tick of 0.1 s and no block. It fires at its first tick, 0.1, and again as soon
/// as it is ready, 0.7 (the tick at 0.6 holds the trigger, which is pressed for 0.1 s). Its blasts are at 0.15
/// and 0.75. The blast's damage is owed to the sim, which kills the bot of 3 on the second.
#[test]
fn the_base_life_fires_at_0_1_and_0_7_and_blasts_at_0_15_and_0_75() {
    for seed in FRAME_KINDS {
        let m = fuse_bot(0.1, 0.6, 0.05, 2.0, 0.0);
        let life = live(&m, &mut Rng::new(7), None, 0.0, 1.0, &mut frame_lengths(seed));
        assert_shots("life", &life.shots, &[0.1, 0.7], seed);
        assert_landed("life", &life.landed, &[0.15, 0.75], 2.0, seed);
    }
}

/// B: the trigger. A key that ticks every 0.5 s with no block. After the first shot at 0.5 the press at 1.0
/// fires a weapon that is ready within 0.1 s of it (0.55, 0.58 and 0.60 s between shots: at 1.05, 1.08 and
/// 1.10) and loses it for one that isn't (0.62 and 0.65), which fires at the tick at 1.5. A lost press still
/// uses the ability: by 1.55 the key has ticked three times and each tick was a use, whatever the shots.
#[test]
fn a_press_fires_a_weapon_that_is_ready_within_a_tenth_of_a_second_and_loses_it_otherwise() {
    for seed in FRAME_KINDS {
        for (every, second) in [(0.55, 1.05), (0.58, 1.08), (0.60, 1.10), (0.62, 1.5), (0.65, 1.5)] {
            let m = fuse_bot(0.5, every, 5.0, 1.0, 0.0);
            let life = live(&m, &mut Rng::new(7), None, 0.0, 1.55, &mut frame_lengths(seed));
            // The tick at 1.5 can press a weapon that is ready at 1.6 (the ones that fired at 1.05 and
            // 1.10 are): that shot is the third and not part of this timeline.
            let first_two: Vec<f32> = life.shots.iter().copied().filter(|t| *t < 1.55).collect();
            assert_shots(&format!("{every} s between shots"), &first_two, &[0.5, second], seed);
            assert_eq!(life.uses, 3, "{every} s between shots, frames {seed:?}: every press is a use");
        }
    }
}

/// C: the weapon's own pace. A tick of 0.1 s and 0.65 s between shots: the presses join up and the weapon
/// fires at its own pace, 0.65 s apart.
#[test]
fn with_a_fast_key_the_weapon_fires_at_its_own_pace() {
    for seed in FRAME_KINDS {
        let m = fuse_bot(0.1, 0.65, 5.0, 1.0, 0.0);
        let life = live(&m, &mut Rng::new(7), None, 0.0, 2.1, &mut frame_lengths(seed));
        assert_shots("life", &life.shots, &[0.1, 0.75, 1.4, 2.05], seed);
    }
}

/// D: the charge. A charge timer of 2.0 s with 0.6 s between shots and a tick of 0.1 s: shots at 0.1, 2.2 and
/// 4.3. The charge sets the pace (the weapon alone would fire every 0.6 s). A weapon ability shares the tie rule
/// measured for movement abilities: a charge that comes back exactly at a try isn't there for that try (see
/// `Slot::settle`), so the charge spent at 0.1 is back at 2.1 and the tick at 2.2 is the first that can use it.
/// KovaaK's wasn't measured on this tie for a weapon ability.
#[test]
fn a_slow_charge_sets_the_pace_of_the_shots() {
    for seed in FRAME_KINDS {
        let m = fuse_bot(0.1, 0.6, 5.0, 1.0, 2.0);
        let life = live(&m, &mut Rng::new(7), None, 0.0, 4.4, &mut frame_lengths(seed));
        assert_shots("life", &life.shots, &[0.1, 2.2, 4.3], seed);
    }
}

/// E: the weapon's cooldown outlives the bot, through `take_brain` and `reborn`. A weapon of 1.5 s fired at
/// 1.6 on the first bot's clock and the bot is gone at 1.65, away 1.0 s: the next bot's weapon is ready 0.45
/// s into its life and fires then (its tick at 0.4 holds the trigger). Without the carry it fires at 0.1.
/// With frames that don't end the life at exactly 1.65 the time is worked out from the clock it left at.
#[test]
fn the_weapons_cooldown_runs_on_through_a_death_and_a_respawn() {
    for seed in FRAME_KINDS {
        let m = fuse_bot(0.1, 1.5, 5.0, 0.1, 0.0);
        let (mut rng, mut frame) = (Rng::new(7), frame_lengths(seed));
        let mut first = live(&m, &mut rng, None, 0.0, 1.65, &mut frame);
        assert_shots("life 1", &first.shots, &[0.1, 1.6], seed);
        let ready = 1.6 + 1.5 - (first.left() + 1.0);
        assert!((ready - 0.45).abs() < longest(seed) + 1e-3, "{seed:?}: ready {ready} into the next life");
        let second = live(&m, &mut rng, first.bot.take_brain(&m), 1.0, 1.0, &mut frame);
        assert_shots("life 2", &second.shots[..1], &[ready], seed);
    }
}

/// F: a blast still to go off is dropped by `reborn`, and the charges and the tick grid start again. A weapon
/// of 1.0 s whose blast takes 5.0 s and a charge that takes 100 s to come back: the first bot fires at 0.1,
/// spends its only charge and leaves at 0.3 with its blast pending. The next bot, away 0.5 s, has no blast
/// waiting, a charge of its own, and the first tick of its key at 0.1 (so it uses the ability then, though its
/// weapon isn't ready until 0.3 and the press is lost).
#[test]
fn a_respawn_drops_the_pending_blast_and_starts_the_charges_and_ticks_again() {
    for seed in FRAME_KINDS {
        let m = fuse_bot(0.1, 1.0, 5.0, 1.0, 100.0);
        let (mut rng, mut frame) = (Rng::new(7), frame_lengths(seed));
        let mut first = live(&m, &mut rng, None, 0.0, 0.3, &mut frame);
        assert_shots("life 1", &first.shots, &[0.1], seed);
        assert_eq!(first.bot.ab.blasts.len(), 1, "{seed:?}: the blast is pending");
        let brain = first.bot.take_brain(&m);
        let mut next = BotState::reborn(&m, &mut Rng::new(9), START, Vec3::X, brain, 0.5);
        assert!(next.ab.blasts.is_empty(), "{seed:?}: the old bot's blast was carried over");
        let (mut p, walls, mut r) = (START, arena(), Rng::new(9));
        while next.ab.clock < 0.15 {
            let t = next.ab.clock;
            next.step_seen(&m, &mut r, &walls, &mut p, 0.5, 0.0, t, 1.0 / 120.0, true);
        }
        assert_eq!(next.ability_uses(), 1, "{seed:?}: its first tick at 0.1 had a charge to use");
        assert!(next.ab.blasts.is_empty(), "{seed:?}: and the press was lost, so it has no blast of its own");
    }
}

/// A fuse bot that stays put ends a life of shots with no velocity, off the ground and with no ability going on.
#[test]
fn a_fuse_ability_does_not_touch_the_bots_velocity() {
    let m = fuse_bot(0.1, 0.6, 0.05, 2.0, 0.0);
    let life = live(&m, &mut Rng::new(7), None, 0.0, 1.0, &mut frame_lengths(None));
    assert_eq!(life.bot.vel, Vec3::ZERO);
    assert!(!life.bot.grounded);
    assert!(!life.bot.using_ability());
}

/// G: a blast that is due exactly when a frame starts lands in that frame, as a try does on its tick, and not
/// a frame later. On even frames of 1/120 s the base life's blasts, at 0.15 and 0.75, are each a frame's start.
#[test]
fn a_blast_due_exactly_at_a_frames_start_lands_in_that_frame() {
    let m = fuse_bot(0.1, 0.6, 0.05, 2.0, 0.0);
    let life = live(&m, &mut Rng::new(7), None, 0.0, 1.0, &mut frame_lengths(None));
    let at: Vec<f32> = life.landed.iter().map(|(at, _)| *at).collect();
    assert_shots("blasts", &at, &[0.15, 0.75], None);
}

/// H: a use of a weapon ability is a press of the trigger and nothing else. The ability's movement settings
/// (made up here: a push of 20 m/s that also throws the bot up, at once or held for 0.3 s) are never played.
/// Two bots that are free to move, alike but for those settings, are in the same place with the same velocity
/// frame after frame, and neither has an ability going on. The importer only takes bots that stay put, so this
/// guards the sim's own rule.
#[test]
fn a_fuse_abilitys_movement_settings_are_never_played() {
    let free = |speed: f32, up_speed: f32, duration_s: f32| {
        let mut m = fuse_bot(0.1, 0.6, 5.0, 1.0, 0.0);
        m.stays_put = false;
        let a = &mut m.abilities.as_mut().unwrap().list[0];
        (a.speed, a.up_speed, a.duration_s) = (speed, up_speed, duration_s);
        m
    };
    let walls = arena();
    let run = |m: &Motion| {
        let mut rng = Rng::new(7);
        let mut bot = BotState::reborn(m, &mut rng, START, Vec3::X, None, 0.0);
        let (mut pos, mut seen) = (START, Vec::new());
        while bot.ab.clock < 1.0 {
            let t = bot.ab.clock;
            bot.step_seen(m, &mut rng, &walls, &mut pos, 0.5, 0.0, t, 1.0 / 120.0, true);
            seen.push((pos, bot.vel, bot.using_ability()));
        }
        (seen, bot.ability_uses())
    };
    let (plain, _) = run(&free(0.0, 0.0, 0.0));
    assert!(plain.iter().all(|(_, _, using)| !using));
    for duration_s in [0.0, 0.3] {
        let (pushed, uses) = run(&free(20.0, 5.0, duration_s));
        assert!(uses >= 1, "held {duration_s} s: it did press the trigger");
        assert_eq!(pushed, plain, "held {duration_s} s: the movement settings were played");
    }
}
