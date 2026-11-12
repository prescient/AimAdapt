//! Tests for movement abilities.

use glam::Vec3;

use super::tests::{DT, arena, strafer};
use super::*;
use crate::rng::Rng;

/// A bot that presses its key every 0.05 s and is always willing to.
fn abilities(list: Vec<Ability>) -> Abilities {
    Abilities { press_every_s: 0.05, chance: 1.0, window_s: [0.05, 0.05], list, ..Abilities::default() }
}

fn with(a: Ability) -> Motion {
    Motion { abilities: Some(abilities(vec![a])), ..strafer() }
}

/// 10 m in front of the player, resting on the arena's floor.
const START: Vec3 = Vec3::new(0.0, -1.5, -10.0);

/// Plays `seconds` with `seen` as the bot's line of sight; `each` sees the time, the
/// position and the state after every frame.
fn play(
    m: &Motion,
    start: Vec3,
    spawn_right: Vec3,
    seen: bool,
    seconds: f32,
    mut each: impl FnMut(f32, Vec3, &BotState),
) -> (Vec3, BotState) {
    let walls = arena();
    let mut rng = Rng::new(7);
    let mut s = BotState::new(m, &mut rng, start, spawn_right);
    let mut p = start;
    let mut t = 0.0;
    while t < seconds {
        s.step_seen(m, &mut rng, &walls, &mut p, 0.5, 0.0, t, DT, seen);
        t += DT;
        each(t, p, &s);
    }
    (p, s)
}

/// When the bot starts each use.
fn use_times(m: &Motion, seconds: f32) -> Vec<f32> {
    let (mut last, mut times) = (0, Vec::new());
    play(m, START, Vec3::X, true, seconds, |t, _, s| {
        if s.ability_uses() != last {
            last = s.ability_uses();
            times.push(t);
        }
    });
    times
}

fn uses(m: &Motion, spawn_right: Vec3, seen: bool) -> u32 {
    play(m, START, spawn_right, seen, 1.5, |_, _, _| {}).1.ability_uses()
}

fn flat_speed(s: &BotState) -> f32 {
    Vec3::new(s.vel.x, 0.0, s.vel.z).length()
}

fn gaps(t: &[f32]) -> Vec<f32> {
    t.windows(2).map(|w| w[1] - w[0]).collect()
}

#[test]
fn a_charge_comes_back_after_its_timer() {
    let one = Ability { charge_s: 1.0, duration_s: 0.1, speed: 3.0, ..Ability::default() };
    let t = use_times(&with(one), 6.0);
    assert!(t.len() >= 5, "{t:?}");
    assert!(t[0] < 0.2, "the first charge is there from the start: {t:?}");
    for g in gaps(&t) {
        assert!((0.98..1.25).contains(&g), "one charge, one second to regain it: {t:?}");
    }
}

#[test]
fn a_bot_that_spawns_without_charges_waits_for_the_first_one() {
    let none = Ability { spawn_charges: 0, charge_s: 1.0, speed: 3.0, ..Ability::default() };
    let t = use_times(&with(none), 3.0);
    assert!((0.98..1.2).contains(&t[0]), "{t:?}");
}

#[test]
fn the_block_after_spawning_holds_a_use_back() {
    let m = Motion {
        abilities: Some(Abilities {
            start_block_s: 0.7,
            ..abilities(vec![Ability { speed: 3.0, ..Ability::default() }])
        }),
        ..strafer()
    };
    let t = use_times(&m, 2.0);
    assert!((0.7..0.85).contains(&t[0]), "{t:?}");
}

#[test]
fn the_delay_or_the_reuse_timer_after_a_use_holds_the_next_one_off() {
    // Charges to spare, so only the delay counts: the larger of the two, from the end.
    let many =
        Ability { max_charges: 9, spawn_charges: 9, charge_s: 0.05, duration_s: 0.1, speed: 3.0, ..Ability::default() };
    for (delay_s, reuse_s, least) in [(0.5, 0.3, 0.6), (0.2, 0.8, 0.9)] {
        let t = use_times(&with(Ability { delay_s, reuse_s, ..many.clone() }), 4.0);
        assert!(t.len() >= 4, "{t:?}");
        for g in gaps(&t) {
            assert!(g >= least - 0.02 && g < least + 0.2, "delay {delay_s}, reuse {reuse_s}: {t:?}");
        }
    }
    // The shared cooldown counts from the end of any ability.
    let m = Motion {
        abilities: Some(Abilities { global_cooldown_s: 0.7, ..abilities(vec![Ability { duration_s: 0.1, ..many }]) }),
        ..strafer()
    };
    let t = use_times(&m, 4.0);
    assert!(gaps(&t).iter().all(|g| (0.78..1.0).contains(g)), "{t:?}");
}

#[test]
fn a_dash_holds_its_speed_then_keeps_the_end_share_of_it_and_slows_down() {
    let dash = Ability { charge_s: 100.0, duration_s: 0.2, speed: 12.0, end_factor: 0.5, ..Ability::default() };
    let m = with(dash);
    let (mut during, mut after, mut seen_use) = (Vec::new(), None, false);
    let (_, s) = play(&m, START, Vec3::X, true, 2.5, |_, _, s| {
        if s.using_ability() {
            seen_use = true;
            during.push(flat_speed(s));
        } else if seen_use && after.is_none() {
            after = Some(flat_speed(s));
        }
    });
    assert!(during.len() >= 20, "{}", during.len());
    assert!(during.iter().all(|v| (v - 12.0).abs() < 0.01), "{during:?}");
    let after = after.unwrap();
    assert!((after - 6.0).abs() < 0.7, "half the dash speed right after it: {after}");
    assert!(flat_speed(&s) <= 4.01, "back to its top speed: {}", flat_speed(&s));
    assert_eq!(s.ability_uses(), 1);
}

#[test]
fn a_dash_goes_along_the_bots_movement_or_the_way_it_faces() {
    let at_first_use = |a: Ability| {
        let mut v = None;
        play(&with(a), START, Vec3::X, true, 0.3, |_, _, s| {
            if s.ability_uses() == 1 && v.is_none() {
                v = Some(s.vel);
            }
        });
        v.unwrap()
    };
    let base = Ability { charge_s: 100.0, speed: 6.0, ..Ability::default() };
    // The frame's braking has taken a little off the 6 m/s by the time it's seen.
    // Facing the player (+Z from here), the bot's strafe to its right is toward -X.
    let along = at_first_use(Ability { along_input: true, ..base.clone() });
    assert!((along.x + 5.5).abs() < 0.7 && along.z.abs() < 0.1, "along its strafe: {along}");
    let facing = at_first_use(base.clone());
    assert!((facing.z - 5.5).abs() < 0.7 && facing.x.abs() < 0.5, "toward the player: {facing}");
    let back = at_first_use(Ability { speed: -6.0, ..base });
    assert!((back.z + 5.5).abs() < 0.7, "a negative speed goes backwards: {back}");
}

#[test]
fn a_bot_with_no_movement_input_dashes_the_way_it_faces() {
    let still = Dodge { strafe: false, ..Dodge::default() };
    let m = Motion {
        dodges: vec![still],
        abilities: Some(abilities(vec![Ability {
            charge_s: 100.0,
            speed: 6.0,
            along_input: true,
            ..Ability::default()
        }])),
        ..strafer()
    };
    let mut v = None;
    play(&m, START, Vec3::X, true, 0.3, |_, _, s| {
        if s.ability_uses() == 1 && v.is_none() {
            v = Some(s.vel);
        }
    });
    assert!((v.unwrap().z - 5.5).abs() < 0.7, "{:?}", v);
}

#[test]
fn a_ground_jump_leaves_the_floor_at_its_speed_and_isnt_used_again_in_the_air() {
    let jump = Ability {
        max_charges: 9,
        spawn_charges: 9,
        charge_s: 0.05,
        up_speed: 6.0,
        in_air: false,
        ..Ability::default()
    };
    let (mut peak, mut top_vy) = (f32::MIN, f32::MIN);
    let (mut was_grounded, mut last, mut mid_air_use, mut landed_after) = (true, 0, false, false);
    let (_, s) = play(&with(jump), START, Vec3::X, true, 3.0, |_, p, s| {
        peak = peak.max(p.y);
        top_vy = top_vy.max(s.vel.y);
        if !s.grounded && !was_grounded && s.ability_uses() != last {
            mid_air_use = true;
        }
        landed_after |= s.ability_uses() > 1 && s.grounded;
        was_grounded = s.grounded;
        last = s.ability_uses();
    });
    assert!(top_vy > 5.8 && top_vy < 6.01, "left the floor at its speed: {top_vy}");
    let rise = 6.0f32 * 6.0 / (2.0 * 9.8);
    assert!((peak - (-1.5 + rise)).abs() < 0.05, "peak {peak}, expected {}", -1.5 + rise);
    assert!(!mid_air_use, "never used while in the air, with charges left");
    assert!(s.ability_uses() >= 2 && landed_after, "but again after landing: {}", s.ability_uses());
}

#[test]
fn the_rules_stop_a_use() {
    let dash = Ability { charge_s: 0.2, speed: 5.0, ..Ability::default() };
    let on = |a: Ability| with(a);
    let base = uses(&on(dash.clone()), Vec3::X, true);
    assert!(base >= 3, "{base}");
    // 10 m away: too near for 20, too far for 5.
    assert_eq!(uses(&on(Ability { min_distance: 20.0, ..dash.clone() }), Vec3::X, true), 0, "too near");
    assert_eq!(uses(&on(Ability { max_distance: 5.0, ..dash.clone() }), Vec3::X, true), 0, "too far");
    assert!(uses(&on(Ability { min_distance: 5.0, max_distance: 30.0, ..dash.clone() }), Vec3::X, true) > 0);
    // Out of sight, only abilities for out of combat go, unless it sees through walls.
    let blind = Ability { out_of_combat: false, ..dash.clone() };
    assert_eq!(uses(&on(blind.clone()), Vec3::X, false), 0, "out of sight");
    assert!(uses(&on(blind.clone()), Vec3::X, true) > 0, "in sight");
    let x_ray =
        Motion { abilities: Some(Abilities { sees_through_walls: true, ..abilities(vec![blind]) }), ..strafer() };
    assert!(uses(&x_ray, Vec3::X, false) > 0, "sees through walls");
    let hidden_only = Ability { in_combat: false, ..dash.clone() };
    assert_eq!(uses(&on(hidden_only.clone()), Vec3::X, true), 0, "in combat");
    assert!(uses(&on(hidden_only), Vec3::X, false) > 0, "out of combat");
    // A narrow view angle needs the player in front of a bot that doesn't face them.
    let narrow = Motion { faces_player: false, ..on(Ability { max_angle_deg: 90.0, ..dash.clone() }) };
    assert_eq!(uses(&narrow, Vec3::X, true), 0, "facing away (-Z from a spawn whose right is +X)");
    assert!(uses(&narrow, Vec3::NEG_X, true) > 0, "facing the player");
    assert!(
        uses(&on(Ability { max_angle_deg: 10.0, ..dash }), Vec3::X, true) > 0,
        "a bot that faces them always passes"
    );
}

#[test]
fn ground_and_air_rules_use_the_bots_state() {
    let dash = Ability { charge_s: 0.2, speed: 5.0, ..Ability::default() };
    let high = Vec3::new(0.0, 3.0, -10.0);
    let first_use_grounded = |a: Ability| {
        let mut grounded = None;
        play(&with(a), high, Vec3::X, true, 2.0, |_, _, s| {
            if s.ability_uses() == 1 && grounded.is_none() {
                grounded = Some(s.grounded);
            }
        });
        grounded
    };
    // From 3 m up, the first press comes while it is still falling.
    assert_eq!(first_use_grounded(dash.clone()), Some(false));
    // Only on the ground: it waits for the landing (about 0.9 s).
    let (mut t_use, mut landed) = (None, None);
    play(&with(Ability { in_air: false, ..dash }), high, Vec3::X, true, 2.0, |t, _, s| {
        landed = landed.or(s.grounded.then_some(t));
        if s.ability_uses() == 1 && t_use.is_none() {
            t_use = Some(t);
        }
    });
    assert!(t_use.unwrap() >= landed.unwrap() - 0.02, "{t_use:?} after landing at {landed:?}");
}

#[test]
fn the_first_ready_ability_in_the_list_is_used() {
    let jump = Ability { up_speed: 5.0, spawn_charges: 0, charge_s: 100.0, ..Ability::default() };
    let dash = Ability { speed: 3.0, charge_s: 100.0, ..Ability::default() };
    let m = Motion { abilities: Some(abilities(vec![jump, dash])), ..strafer() };
    let (_, s) = play(&m, START, Vec3::X, true, 1.0, |_, _, _| {});
    // The first has no charge, so the second goes (once: one charge, none coming).
    assert_eq!(s.ability_uses(), 1);
}

#[test]
fn an_ability_that_blocks_movement_stops_the_bots_own() {
    let hold = Ability { charge_s: 100.0, duration_s: 0.5, ..Ability::default() };
    let speed_at_end = |blocks: bool| {
        let mut v = 0.0;
        play(&with(Ability { blocks_movement: blocks, ..hold.clone() }), START, Vec3::X, true, 0.5, |_, _, s| {
            v = flat_speed(s);
        });
        v
    };
    assert!(speed_at_end(true) < 0.3, "{}", speed_at_end(true));
    assert!(speed_at_end(false) > 3.5, "{}", speed_at_end(false));
}

#[test]
fn no_gravity_holds_a_bot_up_for_the_duration() {
    let float = |no_gravity: bool| {
        let a = Ability { charge_s: 100.0, duration_s: 0.3, speed: 3.0, no_gravity, ..Ability::default() };
        let mut falling = 0.0f32;
        play(&with(a), Vec3::new(0.0, 3.0, -10.0), Vec3::X, true, 0.34, |_, _, s| {
            if s.using_ability() {
                falling = falling.min(s.vel.y);
            }
        });
        falling
    };
    assert!(float(true) > -0.7, "only what it had before the dash began: {}", float(true));
    assert!(float(false) < -2.0, "{}", float(false));
}

/// Bots that wait to be hurt use nothing, but their charges and the block after
/// spawning count down meanwhile.
#[test]
fn a_bot_that_waits_to_be_hurt_uses_nothing_but_its_clock_runs() {
    let m = Motion {
        still_until_hurt: true,
        abilities: Some(Abilities {
            start_block_s: 1.0,
            ..abilities(vec![Ability { spawn_charges: 0, charge_s: 0.5, speed: 3.0, ..Ability::default() }])
        }),
        ..strafer()
    };
    let walls = arena();
    let mut rng = Rng::new(3);
    let mut s = BotState::new(&m, &mut rng, START, Vec3::X);
    let mut p = START;
    for i in 0..360 {
        s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
    }
    assert_eq!((s.ability_uses(), p), (0, START), "3 s waiting: nothing used, nowhere moved");
    s.on_hit(&m, &mut rng, 1.0, 3.0);
    for i in 360..400 {
        s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
    }
    assert!(s.ability_uses() >= 1, "hurt after the block and the charge were over: used at once");
}

#[test]
fn a_motion_without_abilities_saves_without_any_ability_key() {
    let text = toml::to_string(&strafer()).unwrap();
    assert!(!text.contains("abilit"), "{text}");
    let with_one = with(Ability { speed: 3.0, ..Ability::default() });
    let text = toml::to_string(&with_one).unwrap();
    assert!(text.contains("abilities"));
    let back: Motion = toml::from_str(&text).unwrap();
    assert_eq!(back, with_one);
}

#[test]
fn values_out_of_range_are_problems() {
    assert!(abilities(vec![Ability { speed: 12.0, ..Ability::default() }]).problems().is_empty());
    for bad in [
        Ability { speed: f32::NAN, ..Ability::default() },
        Ability { speed: 900.0, ..Ability::default() },
        Ability { up_speed: -900.0, ..Ability::default() },
        Ability { duration_s: 61.0, ..Ability::default() },
        Ability { max_charges: 5000, ..Ability::default() },
        Ability { charge_s: f32::INFINITY, ..Ability::default() },
        Ability { max_angle_deg: 400.0, ..Ability::default() },
        Ability { end_factor: -1.0, ..Ability::default() },
        Ability { health_cost: -1.0, ..Ability::default() },
        Ability { health_cost: f32::NAN, ..Ability::default() },
        Ability { health_cost: f32::INFINITY, ..Ability::default() },
    ] {
        assert!(!abilities(vec![bad.clone()]).problems().is_empty(), "{bad:?}");
    }
    assert!(!Abilities { chance: 1.5, ..abilities(vec![]) }.problems().is_empty());
    assert!(!Abilities { press_every_s: -1.0, ..abilities(vec![]) }.problems().is_empty());
    assert!(!Abilities { window_s: [0.5, f32::NAN], ..abilities(vec![]) }.problems().is_empty());
}

/// The same seed gives the same dashes, so a fixed-seed run stays the same.
#[test]
fn a_dashing_bot_is_reproducible() {
    let dash = Ability { charge_s: 0.5, duration_s: 0.15, speed: 9.0, along_input: true, ..Ability::default() };
    let m = Motion {
        abilities: Some(Abilities {
            press_every_s: 0.1,
            chance: 0.5,
            window_s: [0.1, 0.4],
            list: vec![dash],
            ..Abilities::default()
        }),
        ..strafer()
    };
    let run = || {
        let mut path = Vec::new();
        let (_, s) = play(&m, START, Vec3::X, true, 6.0, |_, p, _| path.push(p));
        (path, s.ability_uses())
    };
    let (a, b) = (run(), run());
    assert!(a.1 >= 2, "it dashed: {}", a.1);
    assert_eq!(a, b);
}

// What was measured in KovaaK's. Times are scaled down where the real ones were long,
// and each test names the measurement it follows.

/// Charges and delays that never get in the way, so only the key presses count.
fn instant() -> Ability {
    Ability { max_charges: 99, spawn_charges: 99, charge_s: 0.01, speed: 2.0, ..Ability::default() }
}

fn pressing(press_every_s: f32, chance: f32, window_s: [f32; 2], list: Vec<Ability>) -> Motion {
    Motion { abilities: Some(Abilities { press_every_s, chance, window_s, list, ..Abilities::default() }), ..strafer() }
}

/// Measured: timer 2.0 gave a jump every 2.00 s and 3.0 every 3.00 s; the timer is the
/// time between presses, not a hold time.
#[test]
fn the_use_timer_is_the_time_between_presses() {
    for timer in [0.6, 1.5] {
        let t = use_times(&pressing(timer, 1.0, [1.0, 1.0], vec![instant()]), 9.0);
        assert!(t.len() >= 5, "{t:?}");
        assert!((t[0] - timer).abs() < 0.02, "the first press is one interval after it appears: {t:?}");
        for g in gaps(&t) {
            assert!((g - timer).abs() < 0.02, "timer {timer}: {t:?}");
        }
    }
}

/// Measured: with timer 0.5, frequency 0.5 and a window of 4 s the bot jumped in runs
/// at 0.5 s spacing with silent gaps of 4, 8 and 4 s. A window rolls the chance once:
/// all presses or none.
#[test]
fn a_window_that_rolls_false_is_silent_and_one_that_rolls_true_presses_every_time() {
    let (press, window) = (0.25, 2.0);
    let t = use_times(&pressing(press, 0.5, [window, window], vec![instant()]), 80.0);
    let gaps = gaps(&t);
    assert!(gaps.iter().filter(|g| (**g - press).abs() < 0.02).count() > 20, "runs of presses: {gaps:?}");
    for g in &gaps {
        // The next press tick, or a whole silent window and more. A window that rolled
        // false and then true for a press or two would leave gaps in between.
        assert!((g - press).abs() < 0.02 || *g >= window - 0.02, "gap {g}: {gaps:?}");
    }
    assert!(gaps.iter().any(|g| *g >= window), "and silent windows: {gaps:?}");
    // Windows of one length and a rolled chance: about half of them press.
    let pressed = t.len() as f32 * press / 80.0;
    assert!((0.3..0.7).contains(&pressed), "{pressed}");
}

/// Measured: with a frequency of 1.0 the window times have no effect.
#[test]
fn a_chance_of_one_ignores_the_window_times() {
    let t = use_times(&pressing(0.25, 1.0, [3.0, 5.0], vec![instant()]), 20.0);
    assert!(t.len() >= 70, "{}", t.len());
    assert!(gaps(&t).iter().all(|g| (g - 0.25).abs() < 0.02), "{t:?}");
}

/// Measured: reuse 1.5 with a press every 0.2 s gave a jump every 1.60 s.
#[test]
fn the_reuse_timer_holds_a_jump_back_until_the_next_press_after_it() {
    let jump = Ability { reuse_s: 1.5, up_speed: 3.0, ..instant() };
    let t = use_times(&pressing(0.2, 1.0, [1.0, 1.0], vec![jump]), 12.0);
    assert!(t.len() >= 5, "{t:?}");
    assert!(gaps(&t).iter().all(|g| (g - 1.6).abs() < 0.03), "{t:?}");
}

/// Measured: a zero-duration jump with a charge timer of 1.5 and a press every 0.2 s
/// came every 1.60 s.
#[test]
fn the_charge_timer_holds_a_jump_back_until_the_next_press_after_it() {
    let jump = Ability { max_charges: 1, spawn_charges: 1, charge_s: 1.5, up_speed: 3.0, ..Ability::default() };
    let t = use_times(&pressing(0.2, 1.0, [1.0, 1.0], vec![jump]), 12.0);
    assert!(t.len() >= 5, "{t:?}");
    assert!(gaps(&t).iter().all(|g| (g - 1.6).abs() < 0.03), "{t:?}");
}

/// Measured: a dash with a charge timer of 4.0, a duration of 0.1 s and a press every
/// 0.1 s came every 4.20 s, so a charge starts to refill when the ability ends.
#[test]
fn a_charge_starts_refilling_when_its_dash_ends() {
    let dash =
        Ability { max_charges: 1, spawn_charges: 1, charge_s: 4.0, duration_s: 0.1, speed: 3.0, ..Ability::default() };
    let t = use_times(&pressing(0.1, 1.0, [1.0, 1.0], vec![dash]), 20.0);
    assert!(t.len() >= 4, "{t:?}");
    for g in gaps(&t) {
        assert!((4.17..=4.23).contains(&g), "4.0 s of charging after the 0.1 s dash: {t:?}");
    }
}

/// The same measurement with frames of uneven length, as in a real run: still every
/// 4.2 s, never 4.1 s. The charge is back at the frame after the press at 4.1 s at the
/// earliest, whatever the frames are.
#[test]
fn the_dash_interval_doesnt_depend_on_the_frame_times() {
    let dash =
        Ability { max_charges: 1, spawn_charges: 1, charge_s: 4.0, duration_s: 0.1, speed: 3.0, ..Ability::default() };
    let m = pressing(0.1, 1.0, [1.0, 1.0], vec![dash]);
    for seed in 1..=8 {
        let walls = arena();
        let (mut rng, mut frames) = (Rng::new(7), Rng::new(seed));
        let mut s = BotState::new(&m, &mut rng, START, Vec3::X);
        let (mut p, mut t, mut last, mut times) = (START, 0.0f32, 0, Vec::new());
        while t < 30.0 {
            // Between 60 and 240 frames a second.
            let dt = 1.0 / (60.0 + 180.0 * frames.next_f32());
            s.step_seen(&m, &mut rng, &walls, &mut p, 0.5, 0.0, t, dt, true);
            if s.ability_uses() != last {
                last = s.ability_uses();
                times.push(t);
            }
            t += dt;
        }
        assert!(times.len() >= 6, "{times:?}");
        for g in gaps(&times) {
            assert!((4.17..=4.23).contains(&g), "seed {seed}: {times:?}");
        }
    }
}

/// Measured: a dash set to the movement keys, on the ground, 3500 uu/s for 0.1 s went
/// about 350 uu and stayed on the ground, the way the movement input pointed, which
/// right after a strafe change is opposite to the way the bot is still sliding.
#[test]
fn a_dash_goes_along_the_movement_input_at_its_speed_for_speed_times_duration() {
    let dash = Ability {
        charge_s: 100.0,
        duration_s: 0.1,
        speed: 35.0,
        end_factor: 0.28,
        along_input: true,
        ..Ability::default()
    };
    // The strafer turns around at 1.0 s; the first press comes just after.
    let m = pressing(1.02, 1.0, [1.0, 1.0], vec![dash]);
    let (mut prev_pos, mut prev_vel) = (START, Vec3::ZERO);
    let (mut from, mut slide, mut during, mut end) = (None, Vec3::ZERO, Vec::new(), None);
    play(&m, START, Vec3::X, true, 1.5, |_, p, s| {
        if s.using_ability() {
            if from.is_none() {
                (from, slide) = (Some(prev_pos), prev_vel);
            }
            during.push(s.vel);
        } else if from.is_some() && end.is_none() {
            end = Some(p);
        }
        (prev_pos, prev_vel) = (p, s.vel);
    });
    let (from, end) = (from.unwrap(), end.unwrap());
    assert!(slide.x < -1.0, "still sliding toward -X when it starts: {slide}");
    let flat = |v: &Vec3| Vec3::new(v.x, 0.0, v.z).length();
    assert!(during.iter().all(|v| (flat(v) - 35.0).abs() < 0.01), "at the dash speed: {during:?}");
    // The input points across the way the bot faces, opposite to its slide.
    assert!(during.iter().all(|v| v.x * slide.x < 0.0 && v.x.abs() > 25.0), "{during:?} against {slide}");
    let far = Vec3::new(end.x - from.x, 0.0, end.z - from.z).length();
    assert!((far - 3.5).abs() < 0.18, "35 m/s for 0.1 s: {far}");
    assert!((end.y - from.y).abs() < 0.05, "stays on the ground: {} to {}", from.y, end.y);
}

/// Measured: a bot with a line of sight to the player used an ability that is switched
/// off for out of combat, though nobody shot. So seeing the player is being in combat.
/// (`seen` is the line of sight.)
#[test]
fn a_line_of_sight_is_enough_to_be_in_combat() {
    let in_only = Ability { out_of_combat: false, ..instant() };
    assert!(uses(&with(in_only.clone()), Vec3::X, true) > 0);
    assert_eq!(uses(&with(in_only), Vec3::X, false), 0);
    let out_only = Ability { in_combat: false, ..instant() };
    assert_eq!(uses(&with(out_only.clone()), Vec3::X, true), 0);
    assert!(uses(&with(out_only), Vec3::X, false) > 0);
}

/// Plan K15a: an ability with no cost stores no `health_cost` key (a stored scenario, and its
/// fingerprint, stay as they were), and one with a cost keeps it through a save.
#[test]
fn a_health_cost_is_stored_only_when_there_is_one() {
    let text = toml::to_string(&with(Ability { speed: 3.0, ..Ability::default() })).unwrap();
    assert!(!text.contains("health_cost"), "{text}");
    let costly = with(Ability { speed: 3.0, health_cost: 30.0, ..Ability::default() });
    let text = toml::to_string(&costly).unwrap();
    assert!(text.contains("health_cost = 30.0"), "{text}");
    assert_eq!(toml::from_str::<Motion>(&text).unwrap(), costly);
}

/// Plan K15a: each use adds its cost to what the bot owes, and asking for it clears it.
#[test]
fn each_use_adds_its_cost_to_what_the_bot_owes() {
    let hop = Ability { charge_s: 0.3, duration_s: 0.0, speed: 2.0, health_cost: 25.0, ..Ability::default() };
    let free = Ability { charge_s: 0.3, speed: 2.0, ..Ability::default() };
    for (a, per_use) in [(hop, 25.0), (free, 0.0)] {
        let m = with(a);
        let mut seen = Vec::new();
        let mut last = 0;
        let mut owed = 0.0;
        // Ask after every frame, as the sim does: what is owed is the cost of the uses of that frame.
        let mut rng = Rng::new(7);
        let walls = arena();
        let mut s = BotState::new(&m, &mut rng, START, Vec3::X);
        let mut p = START;
        for i in 0..360 {
            s.step_seen(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT, true);
            let now = s.take_owed_health();
            owed += now;
            if s.ability_uses() != last {
                seen.push(now);
                last = s.ability_uses();
            }
            assert_eq!(s.take_owed_health(), 0.0, "asking clears it");
        }
        assert!(seen.len() >= 2 && seen.iter().all(|c| *c == per_use), "{seen:?}");
        assert_eq!(owed, per_use * s.ability_uses() as f32);
    }
}
