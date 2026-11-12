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

/// The delay counts from the press (not the end of the ability) and a try exactly when it
/// is over isn't used; the reuse time doesn't hold a use back, it spaces the tries. (This
/// pinned "the larger of the two, from the end" before the timing was measured again.)
#[test]
fn the_delay_after_a_use_holds_the_next_one_off_and_the_reuse_time_spaces_the_tries() {
    // Charges to spare, so only the delay and the tries count. Ticks come every 0.05 s.
    let many =
        Ability { max_charges: 9, spawn_charges: 9, charge_s: 0.05, duration_s: 0.1, speed: 3.0, ..Ability::default() };
    // A delay of 0.5 from the press is over at 0.5 (a tie), so the next tick, 0.55 s on,
    // is used. A reuse time of 0.8 puts the tries 0.85 s apart, and the delay is over by then.
    // With a reuse time of 0.3 the tries are 0.35 s apart. The second try finds the delay
    // (over at 0.55 s) still on and uses up the reuse time, so the use is at the third, 0.7 s on.
    for (delay_s, reuse_s, gap) in [(0.5, 0.0, 0.55), (0.2, 0.8, 0.85), (0.5, 0.3, 0.7)] {
        let t = use_times(&with(Ability { delay_s, reuse_s, ..many.clone() }), 4.0);
        assert!(t.len() >= 4, "{t:?}");
        for g in gaps(&t) {
            assert!((g - gap).abs() < 0.02, "delay {delay_s}, reuse {reuse_s}, gaps of {gap}: {t:?}");
        }
    }
    // The shared cooldown counts from the scheduled end of any ability (the tick plus 0.1 s).
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

/// A charge starts to refill at the press, whatever the length of the dash. A 4.0 s charge,
/// a tick of 0.1 s and no reuse time: it is back at 4.1 s after the press, which is a tie,
/// so the use comes at the next tick, 4.1 s after the one before, for a dash of 0.1 s and of
/// 1.0 s alike. (This was pinned as "4.2 s, starting when the dash ends", the timing the
/// refill rule explained wrongly.)
#[test]
fn a_charge_starts_refilling_at_the_press_not_when_its_dash_ends() {
    for duration_s in [0.1, 1.0] {
        let dash =
            Ability { max_charges: 1, spawn_charges: 1, charge_s: 4.0, duration_s, speed: 3.0, ..Ability::default() };
        let t = use_times(&pressing(0.1, 1.0, [1.0, 1.0], vec![dash]), 20.0);
        assert!(t.len() >= 4, "{t:?}");
        for g in gaps(&t) {
            assert!((4.07..=4.13).contains(&g), "a {duration_s} s dash, 4.0 s charge and a tie: {t:?}");
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

// When a bot uses an ability, as measured in KovaaK's on made-up test maps (120 frames
// a second): the key ticks on a grid, a tick is a try only when more than the reuse time
// has passed since the ability's last try, and charges, delays and the ability itself
// are on the bot's scheduled times, never on the frame lengths.

/// A bot whose key ticks every `tick` seconds, always willing, with one ability and a
/// start block of `block` seconds.
fn ticking(tick: f32, block: f32, a: Ability) -> Motion {
    Motion {
        abilities: Some(Abilities {
            press_every_s: tick,
            chance: 1.0,
            window_s: [1.0, 1.0],
            start_block_s: block,
            list: vec![a],
            ..Abilities::default()
        }),
        ..strafer()
    }
}

/// A dash that goes nowhere much: `max` charges (`spawn` of them at the start) of
/// `charge_s` seconds each.
fn dashing(duration_s: f32, charge_s: f32, max: u32, spawn: u32, delay_s: f32, reuse_s: f32) -> Ability {
    Ability {
        max_charges: max,
        spawn_charges: spawn,
        charge_s,
        delay_s,
        reuse_s,
        duration_s,
        speed: 2.0,
        ..Ability::default()
    }
}

/// A dash with 99 charges that come back at once, so only the ticks and the reuse time
/// count.
fn plenty(duration_s: f32, reuse_s: f32) -> Ability {
    dashing(duration_s, 0.0, 99, 99, 0.0, reuse_s)
}

/// The bot's clock at the start of the frame in which each use begins, on frames of the
/// lengths `frame` gives, until `seconds`. (The clock runs as the frames add up, as the
/// bot's own does.)
fn use_clocks(m: &Motion, seconds: f32, mut frame: impl FnMut() -> f32) -> Vec<f32> {
    let walls = arena();
    let mut rng = Rng::new(7);
    let mut s = BotState::new(m, &mut rng, START, Vec3::X);
    let (mut p, mut t, mut last, mut times) = (START, 0.0f32, 0, Vec::new());
    while t < seconds {
        let dt = frame();
        s.step_seen(m, &mut rng, &walls, &mut p, 0.5, 0.0, t, dt, true);
        while last < s.ability_uses() {
            last += 1;
            times.push(t);
        }
        t += dt;
    }
    times
}

/// The uses come at `expect`, each within one frame after its time: on frames of 1/120 s
/// and on frames between 1/240 and 1/60 s for 8 seeds, with the same number of uses every
/// time. Nothing is used in the 0.05 s after the last one listed (less than a tick).
#[track_caller]
fn assert_timeline(name: &str, m: &Motion, expect: &[f32]) {
    let until = expect.last().unwrap() + 0.05;
    let check = |what: String, times: &[f32], frame: f32| {
        assert_eq!(times.len(), expect.len(), "{name}, {what}: {times:?}, wanted {expect:?}");
        for (got, want) in times.iter().zip(expect) {
            let late = got - want;
            assert!((-0.002..=frame + 0.002).contains(&late), "{name}, {what}: {times:?}, wanted {expect:?}");
        }
    };
    check("even frames".into(), &use_clocks(m, until, || DT), DT);
    for seed in 1..=8 {
        let mut frames = Rng::new(seed);
        let times = use_clocks(m, until, || 1.0 / (60.0 + 180.0 * frames.next_f32()));
        check(format!("seed {seed}"), &times, 1.0 / 60.0);
    }
}

/// Bug: every tick of the key was a try, so the reuse time was a hold counted from the
/// end of the ability. Now a tick is a try only when strictly more than the reuse time
/// has passed since the last try: tries are 0.2 s apart for tick 0.1 and reuse 0.1, 0.3
/// for 0.25, 1.1 for 1.0; a reuse time shorter than the tick makes no difference.
#[test]
fn the_reuse_time_spaces_the_tries_of_the_key() {
    let cases: [(f32, f32, &[f32]); 4] = [
        (0.1, 0.1, &[0.1, 0.3, 0.5, 0.7]),
        (0.1, 0.25, &[0.1, 0.4, 0.7, 1.0]),
        (0.1, 1.0, &[0.1, 1.2, 2.3]),
        (0.25, 0.1, &[0.25, 0.5, 0.75, 1.0]),
    ];
    for (tick, reuse, want) in cases {
        let m = ticking(tick, 0.0, plenty(0.0, reuse));
        assert_timeline(&format!("tick {tick}, reuse {reuse}"), &m, want);
    }
}

/// Bug: a charge only started to refill when its dash had ended, so a 1 s dash with a 3 s
/// charge came back 4 s after the press. It refills from the press.
#[test]
fn a_charge_refills_from_the_press_whatever_the_length_of_the_dash() {
    let long = |delay_s, reuse_s| ticking(0.1, 1.0, dashing(1.0, 3.0, 1, 0, delay_s, reuse_s));
    assert_timeline("1 s dash, 3 s charge", &long(0.1, 0.1), &[3.0, 6.2, 9.4]);
    assert_timeline("the same, reuse 1.0", &long(0.1, 1.0), &[3.2, 6.5, 9.8]);
}

/// Bug: the delay after a use counted from the end of the ability. It counts from the
/// press, and a try exactly when it is over isn't used.
#[test]
fn the_delay_counts_from_the_press_not_from_the_end_of_the_dash() {
    let long = |delay_s| ticking(0.1, 1.0, dashing(1.0, 0.1, 1, 0, delay_s, 0.1));
    assert_timeline("1 s dash, delay 0.1", &long(0.1), &[1.0, 2.2, 3.4]);
    // From the end the 0.6 s would put the second use at 2.6.
    assert_timeline("1 s dash, delay 0.6", &long(0.6), &[1.0, 2.2, 3.4]);
    let slow = ticking(0.1, 1.0, dashing(1.0, 3.0, 1, 0, 0.6, 0.1));
    assert_timeline("1 s dash, 3 s charge, delay 0.6", &slow, &[3.0, 6.2, 9.4]);
    // A short dash with a long delay: over 0.5 s after the press, and a try then isn't used.
    let late = ticking(0.1, 0.0, dashing(0.1, 0.0, 99, 99, 0.5, 0.0));
    assert_timeline("0.1 s dash, delay 0.5", &late, &[0.1, 0.7, 1.3]);
}

/// Bug: a bot with no `BlockAbilityOnStartDuration` key used abilities at once (the
/// importer's side is tested there). The block ends at the key's time, and a tick exactly
/// there is a try that goes through; the ticks before aren't tries, so they use up no
/// reuse time.
#[test]
fn a_try_exactly_at_the_end_of_the_start_block_goes_through_and_earlier_ticks_dont_count() {
    assert_timeline("block 0.7", &ticking(0.1, 0.7, plenty(0.0, 0.0)), &[0.7, 0.8, 0.9]);
    // Ticks at 0.1 to 0.5 would have used up the reuse time of 0.5 if they were tries.
    assert_timeline("block 0.55", &ticking(0.1, 0.55, plenty(0.0, 0.5)), &[0.6, 1.2]);
    assert_timeline("block 1.0", &ticking(0.1, 1.0, plenty(0.0, 0.0)), &[1.0, 1.1, 1.2]);
}

/// Bug: a charge that came back exactly at a try was used at it. It isn't there for that
/// try, only for the next. Three charges of 2.0 s from 3 at the start: back at 3.0, 5.0
/// and 7.0, which are tries, so the uses are at 3.2, 5.2 and 7.2.
#[test]
fn a_charge_back_exactly_at_a_try_isnt_there_for_it() {
    let m = ticking(0.1, 1.0, dashing(0.1, 2.0, 3, 3, 0.1, 0.1));
    assert_timeline("3 charges of 2.0 s", &m, &[1.0, 1.2, 1.4, 3.2, 5.2, 7.2]);
}

/// The exception: a refill that began when the bot appeared, because it has fewer charges
/// than its maximum, is used on a tie, and so is every charge of that chain until the
/// slot is full. A refill that a press started is not.
#[test]
fn a_refill_chain_from_the_spawn_is_used_on_a_tie_and_one_from_a_press_isnt() {
    // 0 of 1 at the start: back at 2.0, a try. The next, from the press at 2.0, is back
    // at 4.0, a try too, and isn't used then.
    let one = ticking(0.1, 1.0, dashing(0.1, 2.0, 1, 0, 0.1, 0.1));
    assert_timeline("0 of 1", &one, &[2.0, 4.2, 6.4]);
    // 1 of 3: used at 1.0; the chain from the spawn comes back at 2.0, 4.0 and 6.0, each
    // used as it comes, and the chain goes on while the bot keeps spending them.
    let three = ticking(0.1, 1.0, dashing(0.1, 2.0, 3, 1, 0.1, 0.1));
    assert_timeline("1 of 3", &three, &[1.0, 2.0, 4.0, 6.0]);
}

/// Bug: a try at the very moment a dash ends was used. A try then isn't used: with a 1 s
/// dash from 0.1 the next use is at 1.2, not 1.1.
#[test]
fn a_try_at_the_moment_a_dash_ends_isnt_used() {
    let m = ticking(0.1, 0.0, plenty(1.0, 0.0));
    assert_timeline("1 s dash, tick 0.1", &m, &[0.1, 1.2, 2.3]);
    let half = ticking(0.1, 1.0, dashing(0.5, 0.1, 1, 0, 0.1, 1.0));
    assert_timeline("0.5 s dash", &half, &[1.0, 2.1, 3.2]);
}

/// Bug: charges that were missing together were refilled from the end of each dash. They
/// come back one after another, each a charge time after the one before, starting at the
/// press that spent the first.
#[test]
fn missing_charges_come_back_one_after_another_from_the_first_press() {
    let m = ticking(0.1, 1.0, dashing(1.0, 3.0, 3, 3, 0.1, 0.1));
    assert_timeline("1 s dash, 3 charges of 3 s", &m, &[1.0, 2.2, 3.4, 4.6, 7.2, 10.2]);
}

/// The measured rows that have no bug of their own.
#[test]
fn the_other_measured_timelines_hold_on_any_frame_times() {
    assert_timeline("tick 0.25", &ticking(0.25, 1.0, dashing(0.1, 0.1, 1, 0, 0.1, 0.1)), &[1.0, 1.25, 1.5]);
    assert_timeline("reuse 0.25", &ticking(0.1, 0.0, dashing(0.1, 0.1, 1, 1, 0.1, 0.25)), &[0.1, 0.4, 0.7]);
    assert_timeline("block 2.5", &ticking(0.1, 2.5, dashing(0.1, 2.0, 1, 1, 0.1, 0.1)), &[2.5, 4.7, 6.9]);
}

/// Measured: a dash with a 4.0 s charge, 0.1 s long, a tick of 0.1 and reuse 0.1 came
/// every 4.20 s, never 4.10 s. Tries are 0.2 s apart, and the one at 4.1 is the moment
/// the charge comes back, so the dash comes at the try at 4.3. (The old rule got this
/// by starting the charge at the end of the dash.) Frames of uneven length, as in a real
/// run, make no difference.
#[test]
fn a_dash_every_4_2_s_is_the_tie_at_4_1_then_the_try_at_4_3_on_any_frames() {
    let m = ticking(0.1, 0.0, dashing(0.1, 4.0, 1, 1, 0.0, 0.1));
    assert_timeline("4.0 s charge", &m, &[0.1, 4.3, 8.5, 12.7]);
}

/// At a tick the abilities are gone through in list order; each one for which the tick
/// is a try gets its last try set, and the first ready one is used; those after it aren't
/// tried at that tick. A has one charge that never comes back and B has a reuse time of
/// 0.5: A is used at 0.1, B is first tried at 0.2 and used, and its next try is at 0.8.
#[test]
fn abilities_after_the_one_used_arent_tried_at_that_tick() {
    let a = Ability { max_charges: 1, spawn_charges: 1, charge_s: 100.0, speed: 2.0, ..Ability::default() };
    let m = Motion {
        abilities: Some(Abilities {
            press_every_s: 0.1,
            chance: 1.0,
            window_s: [1.0, 1.0],
            list: vec![a, plenty(0.0, 0.5)],
            ..Abilities::default()
        }),
        ..strafer()
    };
    assert_timeline("two abilities", &m, &[0.1, 0.2, 0.8]);
}

/// A tick is a try whether or not the ability's rules allow the use: it uses up the
/// reuse time. In sight (in combat), an ability for out of combat only fails at 0.1 and
/// that was a try, so at 0.2, out of sight, it isn't one and the use is at 0.7.
#[test]
fn a_try_the_rules_turn_down_still_uses_up_the_reuse_time() {
    let hidden_only = Ability { in_combat: false, ..plenty(0.0, 0.5) };
    let m = ticking(0.1, 0.0, hidden_only);
    let walls = arena();
    let mut rng = Rng::new(7);
    let mut s = BotState::new(&m, &mut rng, START, Vec3::X);
    let (mut p, mut t, mut first) = (START, 0.0f32, None);
    while t < 1.0 {
        let seen = t < 0.15;
        s.step_seen(&m, &mut rng, &walls, &mut p, 0.5, 0.0, t, DT, seen);
        if s.ability_uses() == 1 && first.is_none() {
            first = Some(t);
        }
        t += DT;
    }
    let first = first.expect("used");
    assert!((0.69..0.72).contains(&first), "{first}");
}

/// A bot that waits to be hurt uses nothing, and the ticks that pass meanwhile aren't
/// tries: when it wakes it doesn't make up for the ones it missed. At 20 ticks a second
/// over 3 s it would have 60 to catch up on.
#[test]
fn a_bot_that_waits_to_be_hurt_doesnt_burst_through_the_ticks_it_missed() {
    let m = Motion { still_until_hurt: true, ..ticking(0.05, 0.0, plenty(0.0, 0.0)) };
    let walls = arena();
    let mut rng = Rng::new(3);
    let mut s = BotState::new(&m, &mut rng, START, Vec3::X);
    let mut p = START;
    for i in 0..360 {
        s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
    }
    assert_eq!(s.ability_uses(), 0);
    s.on_hit(&m, &mut rng, 1.0, 3.0);
    s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, 3.0, DT);
    assert!(s.ability_uses() <= 1, "{}", s.ability_uses());
    // And on from there it goes at the ticks: 0.2 s more is 4 ticks or so, not a burst.
    for i in 361..385 {
        s.step(&m, &mut rng, &walls, &mut p, 0.5, 0.0, i as f32 * DT, DT);
    }
    assert!((3..=5).contains(&s.ability_uses()), "{}", s.ability_uses());
}

/// A key that ticks every 0 s ticks once a frame, at the frame's clock, and doesn't hang
/// or divide by zero. With a reuse time of 0.25 the tries are more than 0.25 s apart.
#[test]
fn a_tick_of_zero_is_one_try_a_frame() {
    let frames = 120;
    let t = use_clocks(&ticking(0.0, 0.0, plenty(0.0, 0.0)), (frames as f32 - 0.5) * DT, || DT);
    assert_eq!(t.len(), frames, "one use a frame");
    let t = use_clocks(&ticking(0.0, 0.0, plenty(0.0, 0.25)), 3.0, || DT);
    assert!(t.len() >= 8, "{t:?}");
    assert!(gaps(&t).iter().all(|g| (0.2505..0.27).contains(g)), "{t:?}");
}

/// Every tick that came due in a frame is a try, in order: a frame of a second with a
/// tick of 0.1 s has ten of them. Past 64 ticks in a frame the bot skips to the first
/// tick after the clock without trying the skipped ones, and stays on the grid.
#[test]
fn a_long_frame_tries_each_tick_that_came_due_up_to_64_then_skips_to_the_grid() {
    let m = ticking(0.1, 0.0, plenty(0.0, 0.0));
    let walls = arena();
    let go = |gap: f32| {
        let mut rng = Rng::new(7);
        let mut s = BotState::new(&m, &mut rng, START, Vec3::X);
        let mut p = START;
        s.ability_tick(&m, gap);
        s.step_seen(&m, &mut rng, &walls, &mut p, 0.5, 0.0, gap, 0.01, true);
        (s, rng, p)
    };
    let (s, ..) = go(1.0);
    assert_eq!(s.ability_uses(), 10);
    let (mut s, mut rng, mut p) = go(100.0);
    assert_eq!(s.ability_uses(), 64, "no more than 64 ticks in a frame");
    // The next tick after 100.0 is at 100.1: nothing until then, one use then.
    let mut t = 100.01;
    while t < 100.095 {
        s.step_seen(&m, &mut rng, &walls, &mut p, 0.5, 0.0, t, 0.01, true);
        t += 0.01;
    }
    assert_eq!(s.ability_uses(), 64, "the skipped ticks weren't tries");
    s.step_seen(&m, &mut rng, &walls, &mut p, 0.5, 0.0, t, 0.01, true);
    s.step_seen(&m, &mut rng, &walls, &mut p, 0.5, 0.0, t + 0.01, 0.01, true);
    assert_eq!(s.ability_uses(), 65, "on the grid, one tick");
}
