//! Tests for adaptive play on imported scenarios (BACKLOG 9, Phase 5; docs/plans/2026-11-imported-adaptive.md):
//! the rule each kind of file is rated by, the speed knob and its game clock, and kill pace in real seconds.
//! The worlds are made up: an empty floor with one bot slot.

use super::*;
use crate::adaptive::{Adaptive, ImportedRule, imported_rule};
use crate::motion::{Dodge, Motion};
use crate::scenario::Scenario;
use crate::world::{World, WorldMesh, WorldSpawn, WorldVolume};

const FRAME: f32 = 1.0 / 240.0;

fn away() -> Input {
    Input { aim: Vec3::Y, fire_pressed: false, fire_held: false }
}

/// Aimed at the first bot, firing as the weapon wants it (a held trigger held, a click clicked).
fn on_it(sim: &Sim, fire: bool) -> Input {
    let aim = sim.targets().first().map_or(Vec3::Y, |t| t.pos.normalize());
    Input { aim, fire_pressed: fire, fire_held: fire }
}

/// A bot that strafes left and right at 4 m/s.
fn strafer() -> Motion {
    let strafe = Dodge { strafe_s: [0.5, 0.5], ..Default::default() };
    Motion { max_speed: 4.0, accel: 40.0, dodges: vec![strafe], ..Default::default() }
}

/// One bot slot on an empty floor, the bot 10 m ahead: an imported scenario of `secs` game seconds at
/// `time_scale`, with a hitscan weapon fired by `trigger` at 20 shots a second.
fn world(motion: Option<Motion>, trigger: Trigger, time_scale: f32, secs: f32) -> Scenario {
    let e = crate::library::builtins().into_iter().find(|e| e.id == "builtin:static-grid").unwrap();
    let mut s = e.scenario.unwrap();
    // An imported file names no knob: it gets the one its rule picks.
    s.adaptive = Adaptive::default();
    s.timing.countdown_s = 0.0;
    s.timing.duration_s = secs;
    s.targets.count = 1;
    s.targets.radius = 0.5;
    s.targets.height = 1.0;
    s.targets.health = 1e6;
    s.targets.respawn_delay_s = 0.0;
    s.weapon.trigger = trigger;
    s.weapon.fire_rate = 20.0;
    let floor = WorldMesh {
        positions: vec![[-30.0, -1.5, -30.0], [30.0, -1.5, -30.0], [30.0, -1.5, 5.0], [-30.0, -1.5, 5.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        ..Default::default()
    };
    s.world = Some(World {
        spawns: vec![WorldSpawn { pos: [0.0, -1.0, -10.0], ..Default::default() }],
        meshes: vec![floor],
        motion,
        time_scale,
        ..Default::default()
    });
    assert!(s.problems().is_empty(), "{:?}", s.problems());
    s
}

/// A rating of `value` on `knob` from an earlier run, so the next run plays at one value near it.
fn prior_at(knob: Knob, value: f32) -> Prior {
    Prior {
        theta: knob.h(value) + 0.12 * crate::adaptive::logit_with_slips(0.85),
        slope: 0.12,
        sd: 0.08,
        slope_sd: None,
        days: 1.0,
        fastest: None,
        last_run: Some((Pick::Near, 0.85)),
    }
}

/// Plays until the run ends (at most `secs` real seconds), with `input` deciding each frame.
fn play(sim: &mut Sim, secs: f32, mut input: impl FnMut(&Sim) -> Input) {
    for _ in 0..(secs * 240.0).round() as usize {
        if sim.phase() == Phase::Finished {
            return;
        }
        let i = input(sim);
        sim.step(FRAME, i);
    }
}

#[test]
fn speed_is_a_knob_where_higher_is_harder() {
    let k = Knob::TimeScale;
    assert!(k.higher_is_harder());
    assert!(k.h(1.2) > k.h(0.8));
    assert!((k.value(k.h(0.85)) - 0.85).abs() < 1e-5);
    assert_eq!(k.format(0.8512), "×0.85");
    assert_eq!(k.label(), "Speed");
    // Stored as `time_scale`, which the trainer saves as the run's knob.
    let text = toml::to_string(&Adaptive { knob: Knob::TimeScale, ..Default::default() }).unwrap();
    assert!(text.contains("knob = \"time_scale\""), "{text}");
}

/// Each kind of imported file is rated by the first rule that fits it: bots that can leave by themselves,
/// then a click weapon or bots that never move, then everything else (a held weapon on moving bots).
#[test]
fn an_imported_scenario_is_rated_by_the_first_rule_that_fits() {
    let rule = |s: &Scenario| (imported_rule(s), s.adaptive.resolve(s).knob);
    let builtin = world(None, Trigger::Click, 1.0, 10.0);
    let builtin = Scenario { world: None, ..builtin };
    assert_eq!(rule(&builtin), (None, Knob::Lifetime), "a built-in scenario keeps its knob");

    let pace = Some(ImportedRule::Pace);
    assert_eq!(rule(&world(None, Trigger::Click, 1.0, 10.0)), (pace, Knob::KillPace), "clicking still bots");
    assert_eq!(rule(&world(Some(strafer()), Trigger::Click, 1.0, 10.0)), (pace, Knob::KillPace), "clicking movers");
    assert_eq!(rule(&world(None, Trigger::Hold, 1.0, 10.0)), (pace, Knob::KillPace), "holding fire on still bots");
    let no_speed = Motion { max_speed: 0.0, ..Default::default() };
    assert_eq!(rule(&world(Some(no_speed), Trigger::Hold, 1.0, 10.0)), (pace, Knob::KillPace), "a bot with no speed");

    let tracking = Some(ImportedRule::Tracking);
    assert_eq!(rule(&world(Some(strafer()), Trigger::Hold, 1.0, 10.0)), (tracking, Knob::TimeScale));
    assert_eq!(rule(&world(Some(strafer()), Trigger::Always, 1.0, 10.0)), (tracking, Knob::TimeScale));

    let timed = Some(ImportedRule::Timed);
    let mut lifetime = world(None, Trigger::Click, 1.0, 10.0);
    lifetime.targets.lifetime_s = 1.5;
    assert_eq!(rule(&lifetime), (timed, Knob::TimeScale), "a lifetime");
    let mut drain = world(Some(strafer()), Trigger::Hold, 1.0, 10.0);
    drain.targets.regen_per_s = -10.0;
    assert_eq!(rule(&drain), (timed, Knob::TimeScale), "a drain");
    let mut volume = world(None, Trigger::Click, 1.0, 10.0);
    let edges = [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]];
    let kill = WorldVolume { corner: [5.0, -1.5, -11.0], edges, kill: true, damage: 0.0, cooldown_s: 0.0, shown: true };
    volume.world.as_mut().unwrap().volumes.push(kill);
    assert_eq!(rule(&volume), (timed, Knob::TimeScale), "a volume that kills");
    let costly = crate::motion::Ability { health_cost: 10.0, ..Default::default() };
    let abilities = crate::motion::Abilities { list: vec![costly], ..Default::default() };
    let blast = Motion { abilities: Some(abilities), ..strafer() };
    assert_eq!(
        rule(&world(Some(blast), Trigger::Hold, 1.0, 10.0)),
        (timed, Knob::TimeScale),
        "an ability that costs health"
    );
}

/// Speed scales an imported file's own clock: a built-in scenario can't name it.
#[test]
fn only_an_imported_scenario_can_be_rated_on_speed() {
    let mut s = world(None, Trigger::Click, 1.0, 10.0);
    s.adaptive.knob = Knob::TimeScale;
    assert!(s.problems().is_empty(), "{:?}", s.problems());
    s.world = None;
    assert!(s.problems().iter().any(|p| p.contains("speed is for imported")), "{:?}", s.problems());
}

/// Speed goes from ×0.25 (easy) to ×2.5 (hard) and starts at ×1.0, the file as written, narrowed so the
/// file's own rate times it stays in the sim's 0.1–10. Kill pace on an import starts at 1.2 s.
#[test]
fn an_imported_scenarios_knob_has_its_own_range() {
    let range = |s: &Scenario| {
        let r = s.adaptive.resolve(s);
        (r.easy, r.hard, r.start)
    };
    assert_eq!(range(&world(Some(strafer()), Trigger::Hold, 1.0, 10.0)), (0.25, 2.5, 1.0));
    let (easy, hard, _) = range(&world(Some(strafer()), Trigger::Hold, 0.2, 10.0));
    assert!((easy - 0.5).abs() < 1e-6 && hard == 2.5, "{easy}–{hard} at 0.2");
    let (easy, hard, _) = range(&world(Some(strafer()), Trigger::Hold, 5.0, 10.0));
    assert!(easy == 0.25 && (hard - 2.0).abs() < 1e-6, "{easy}–{hard} at 5");
    assert_eq!(range(&world(None, Trigger::Click, 0.8, 10.0)), (4.0, 0.2, 1.2));
}

#[test]
fn the_clock_turns_game_seconds_into_real_seconds_across_changes_of_rate() {
    let mut c = Rate::new(0.5);
    assert!(!c.is_real());
    assert!(Rate::new(1.0).is_real());
    assert_eq!(c.span(1.0, 3.0), 6.0);
    // 2 game seconds at 0.5 take 4 real seconds; from there the clock runs at 1.0.
    c.set(2.0, 1.0);
    assert_eq!(c.to_real(1.0), 2.0);
    assert_eq!(c.to_real(2.0), 4.0);
    assert_eq!(c.to_real(3.0), 5.0);
    assert_eq!(c.span(1.0, 2.0), 3.0);
    // A change at the same moment replaces the last.
    c.set(2.0, 2.0);
    assert_eq!(c.changes.len(), 2);
    assert_eq!(c.to_real(4.0), 5.0);
}

/// A speed run at a value runs the whole game clock at the file's rate times that value, and lasts the
/// file's real length: an 8 s file at 0.8 takes 10 real seconds at any speed, as KovaaK's 80% files do.
#[test]
fn a_speed_run_plays_at_the_files_rate_times_its_value_for_the_files_real_time() {
    let s = world(Some(strafer()), Trigger::Hold, 0.8, 8.0);
    for rating in [0.5, 1.6] {
        let mut sim = Sim::adaptive(s.clone(), 3, Some(prior_at(Knob::TimeScale, rating)), Mix::Balanced);
        let (value, _) = sim.run_pick().expect("a run at one value");
        assert!((sim.time_scale() - 0.8 * value).abs() < 1e-6, "{} at ×{value}", sim.time_scale());
        play(&mut sim, 9.9, |sim| on_it(sim, true));
        assert_eq!(sim.phase(), Phase::Running, "×{value}: still going after 9.9 real seconds");
        assert!((sim.real_time_left() - 0.1).abs() < 0.01, "{}", sim.real_time_left());
        play(&mut sim, 0.2, |sim| on_it(sim, true));
        assert_eq!(sim.phase(), Phase::Finished, "×{value}: over after 10.1 real seconds");
        assert!((sim.real_elapsed() - 10.0).abs() < 1e-3, "{}", sim.real_elapsed());
        assert!((sim.elapsed() - 8.0 * value).abs() < 1e-2, "{} game seconds at ×{value}", sim.elapsed());
        let r = sim.result();
        assert!((r.duration_s - 10.0).abs() < 1e-3, "the saved run counts real seconds: {}", r.duration_s);
        let a = r.adaptive.unwrap();
        assert_eq!(a.knob, Knob::TimeScale);
        assert_eq!(a.run_value, Some(value));
        assert!(a.timeline.last().unwrap().0 <= 10.0 + 1e-3, "the rating's timeline is real seconds too");
    }
}

/// A calibration run sets the speed for each 2 s segment: the clock's rate changes as it goes, the run still
/// ends after the file's real length, and every time saved with it is in real seconds.
#[test]
fn a_speed_calibration_run_changes_the_clock_each_segment_and_keeps_real_seconds() {
    let mut s = world(Some(strafer()), Trigger::Hold, 0.8, 16.0);
    s.targets.health = 20.0;
    s.weapon.damage = 1.0;
    let mut sim = Sim::adaptive(s, 4, None, Mix::Balanced);
    assert!(sim.run_pick().is_none(), "no rating yet: a calibration run");
    // On target half of each second, so the rating settles inside the range rather than at its end.
    play(&mut sim, 30.0, |sim| if sim.real_elapsed().fract() < 0.6 { on_it(sim, true) } else { away() });
    assert_eq!(sim.phase(), Phase::Finished);
    let changes = &sim.rate.changes;
    assert!(changes.len() >= 5, "the rate changed segment by segment: {changes:?}");
    for w in changes.windows(2) {
        assert!(w[1].0 > w[0].0 && w[1].1 > w[0].1 && w[1].2 != w[0].2, "{changes:?}");
        // Each segment is 2 real seconds (121 ticks of 1/60 s, as the beam's are).
        assert!((w[1].1 - w[0].1 - 2.0).abs() < 0.02, "{changes:?}");
    }
    let r = sim.result();
    assert!((r.duration_s - 20.0).abs() < 1e-3, "{} real seconds", r.duration_s);
    assert!(r.stats.kills > 3, "it kills: {}", r.stats.kills);
    let mut last = 0.0;
    for k in &r.kills {
        assert!(k.at_s >= last && k.at_s <= 20.0 + 1e-3 && k.ttk_s > 0.0, "{k:?}");
        last = k.at_s;
    }
    let trace_end = r.telemetry.trace.last().unwrap()[0];
    assert!(trace_end <= 20.0 + 1e-3 && trace_end > 19.9, "the aim trace ends with the run: {trace_end}");
    let a = r.adaptive.unwrap();
    assert!(a.calibration());
    assert!(a.outcomes >= 9, "a segment every 2 real seconds: {}", a.outcomes);
    assert!(a.value < 1.0, "on target 60% of the time rates under the file's speed: ×{}", a.value);
}

/// Held fire on moving bots is rated on each segment's share of ticks with the aim on the bot, firing or not.
/// Before, only a beam fed tracking segments, so a held hitscan weapon gave no outcomes at all.
#[test]
fn held_fire_tracking_rates_each_segment_by_the_share_of_time_on_target() {
    let s = world(Some(strafer()), Trigger::Hold, 1.0, 10.0);
    let run = |input: &dyn Fn(&Sim) -> Input| {
        let mut sim = Sim::adaptive(s.clone(), 5, Some(prior_at(Knob::TimeScale, 1.0)), Mix::Balanced);
        play(&mut sim, 11.0, input);
        sim.result().adaptive.unwrap()
    };
    let on = run(&|sim| on_it(sim, true));
    assert_eq!(on.success_rate, Some(1.0), "{on:?}");
    assert_eq!(on.outcomes, 4, "a segment every 2 s (121 ticks of 1/60 s, as the beam's are)");
    assert!((on.evidence - 12.0).abs() < 1e-3, "each segment weighs 3: {}", on.evidence);
    assert_eq!(run(&|sim| on_it(sim, false)).success_rate, Some(1.0), "aim is what counts, not firing");
    assert_eq!(run(&|_| away()).success_rate, Some(0.0));
    // On target the first half of every second.
    let half = run(&|sim| if sim.real_elapsed().fract() < 0.5 { on_it(sim, true) } else { away() });
    let rate = half.success_rate.unwrap();
    assert!((rate - 0.5).abs() < 0.05, "{rate}");
}

/// While no bot is up (its respawn delay), the ticks aren't the player's: they count neither way.
#[test]
fn tracking_ticks_with_no_bot_up_count_for_nothing() {
    let mut s = world(Some(strafer()), Trigger::Hold, 1.0, 10.0);
    s.targets.health = 10.0;
    s.weapon.damage = 2.0;
    s.targets.respawn_delay_s = 0.5;
    let mut sim = Sim::adaptive(s, 6, Some(prior_at(Knob::TimeScale, 1.0)), Mix::Balanced);
    // Waiting where the next bot appears, as a player would.
    let spawn = Vec3::new(0.0, -1.0, -10.0).normalize();
    play(&mut sim, 11.0, |sim| {
        if sim.targets().is_empty() { Input { aim: spawn, ..on_it(sim, true) } } else { on_it(sim, true) }
    });
    let r = sim.result();
    assert!(r.stats.kills >= 10, "it spends much of the run waiting for bots: {} kills", r.stats.kills);
    let a = r.adaptive.unwrap();
    assert_eq!(a.success_rate, Some(1.0), "{a:?}");
    assert!(a.outcomes < 5, "the waits don't fill segments: {}", a.outcomes);
}

/// A timed file rates each bot: a kill is a success and a bot that leaves by itself a failure, at the speed
/// the clock had when it appeared. A held weapon has no miss rule, a click weapon fails a kill after a spray.
#[test]
fn a_timed_import_rates_each_bot() {
    let timed = |trigger: Trigger| {
        let mut s = world(None, trigger, 1.0, 10.0);
        s.targets.lifetime_s = 1.0;
        s.targets.health = 1.0;
        s.weapon.damage = 1.0;
        s.weapon.fire_rate = 10.0;
        s.targets.respawn_delay_s = 0.2;
        s
    };
    let run = |s: &Scenario, input: &dyn Fn(&Sim) -> Input| {
        let mut sim = Sim::adaptive(s.clone(), 7, Some(prior_at(Knob::TimeScale, 1.0)), Mix::Balanced);
        assert!(sim.rated_per_target());
        play(&mut sim, 11.0, input);
        sim.result()
    };
    let held = timed(Trigger::Hold);
    let gone = run(&held, &|_| away());
    let a = gone.adaptive.unwrap();
    assert_eq!(a.success_rate, Some(0.0));
    assert_eq!(a.outcomes as u32, gone.stats.timeouts, "one failure for each bot that left");

    // Spraying for 0.4 s beside each bot, then onto it: many misses, still a success with a held weapon.
    let spray = |sim: &Sim| {
        let Some(t) = sim.targets().first() else { return away() };
        let age = sim.elapsed() - t.spawned_at;
        let aim = if age < 0.4 { (t.pos + Vec3::X * 2.0).normalize() } else { t.pos.normalize() };
        Input { aim, fire_pressed: true, fire_held: true }
    };
    let held_spray = run(&held, &spray);
    assert!(held_spray.stats.misses > 20 && held_spray.stats.kills > 5, "{:?}", held_spray.stats);
    assert_eq!(held_spray.adaptive.unwrap().success_rate, Some(1.0));

    // A click weapon: clicking twice beside a bot and then on it fails the kill.
    let click = timed(Trigger::Click);
    // A press every 0.15 s of a bot's life (longer than the weapon's 0.1 s between shots): two beside it,
    // then on it. The bot and the presses on it so far are kept between frames.
    let pressed = std::cell::Cell::new((0, 0));
    let taps = |sim: &Sim| {
        let Some(t) = sim.targets().first() else { return away() };
        let age = sim.elapsed() - t.spawned_at;
        let n = if pressed.get().0 == t.no() { pressed.get().1 } else { 0 };
        let press = age >= n as f32 * 0.15;
        pressed.set((t.no(), n + u32::from(press)));
        let aim = if n < 2 { (t.pos + Vec3::X * 2.0).normalize() } else { t.pos.normalize() };
        Input { aim, fire_pressed: press, fire_held: press }
    };
    let clicked = run(&click, &taps);
    assert!(clicked.stats.kills > 3, "{:?}", clicked.stats);
    assert_eq!(clicked.adaptive.unwrap().success_rate, Some(0.0), "kills after two misses fail");
}

/// Kill pace on an imported file: the windows are real seconds, whatever the file's time scale. At 0.5 a
/// 1 s window is half a second of the game clock.
#[test]
fn kill_pace_windows_on_an_import_are_real_seconds() {
    let mut s = world(None, Trigger::Click, 0.5, 10.0);
    s.targets.health = 1.0;
    s.weapon.damage = 1.0;
    let prior = prior_at(Knob::KillPace, 1.0);
    let mut sim = Sim::adaptive(s.clone(), 8, Some(prior), Mix::Balanced);
    let (value, _) = sim.run_pick().unwrap();
    play(&mut sim, 0.5, |_| away());
    let (window, left) = sim.pace().unwrap();
    assert_eq!(window, value);
    assert!((left - (value - 0.5)).abs() < 0.01, "half a real second gone: {left} of {value}");

    // A kill each `every` real seconds.
    let kill_every = |every: f32| {
        let mut sim = Sim::adaptive(s.clone(), 8, Some(prior), Mix::Balanced);
        let mut next = every;
        play(&mut sim, 21.0, |sim| {
            if sim.real_elapsed() >= next && !sim.targets().is_empty() {
                next += every;
                on_it(sim, true)
            } else {
                away()
            }
        });
        sim.result()
    };
    let fast = kill_every(value * 0.6);
    assert!((fast.duration_s - 20.0).abs() < 1e-2, "10 game seconds at 0.5: {}", fast.duration_s);
    let a = fast.adaptive.unwrap();
    assert_eq!(a.success_rate, Some(1.0), "{a:?}");
    assert!(a.outcomes > 20);
    let slow = kill_every(value * 1.7);
    assert_eq!(slow.adaptive.unwrap().success_rate, Some(0.0));
}

/// Kill pace with a held weapon on still bots: a held trigger fires between targets, so its misses don't
/// fail a kill. A click weapon keeps the rule.
#[test]
fn kill_pace_with_a_held_weapon_has_no_miss_rule() {
    let pace = |trigger: Trigger| {
        let mut s = world(None, trigger, 1.0, 10.0);
        s.targets.health = 1.0;
        s.weapon.damage = 1.0;
        s.targets.respawn_delay_s = 0.3;
        let mut sim = Sim::adaptive(s, 9, Some(prior_at(Knob::KillPace, 1.5)), Mix::Balanced);
        // Firing all the time: beside the bot until it has been up 0.2 s, then on it.
        play(&mut sim, 11.0, |sim| {
            let Some(t) = sim.targets().first() else { return Input { fire_pressed: true, ..away() } };
            let age = sim.elapsed() - t.spawned_at;
            let aim = if age < 0.2 { (t.pos + Vec3::X * 2.0).normalize() } else { t.pos.normalize() };
            let press = ((sim.elapsed() * 240.0).round() as u32).is_multiple_of(12);
            Input { aim, fire_pressed: press, fire_held: true }
        });
        sim.result()
    };
    let held = pace(Trigger::Hold);
    assert!(held.stats.misses > 10 && held.stats.kills > 10, "{:?}", held.stats);
    assert_eq!(held.adaptive.unwrap().success_rate, Some(1.0));
    let clicked = pace(Trigger::Click);
    assert!(clicked.stats.kills > 10, "{:?}", clicked.stats);
    assert_eq!(clicked.adaptive.unwrap().success_rate, Some(0.0));
}

/// A kill limit still ends a speed run, before its real time is up.
#[test]
fn a_speed_run_still_ends_on_its_kill_limit() {
    let mut s = world(Some(strafer()), Trigger::Hold, 0.8, 80.0);
    s.timing.kill_limit = 3;
    s.targets.health = 10.0;
    s.weapon.damage = 1.0;
    let mut sim = Sim::adaptive(s, 10, Some(prior_at(Knob::TimeScale, 1.0)), Mix::Balanced);
    play(&mut sim, 100.0, |sim| on_it(sim, true));
    assert_eq!(sim.phase(), Phase::Finished);
    let r = sim.result();
    assert_eq!(r.stats.kills, 3);
    assert!(r.duration_s < 10.0, "{} real seconds", r.duration_s);
}

/// A standard run of an import, and every built-in run, keeps its clock as before: no speed knob, the file's
/// rate throughout, and the scenario's length on the game clock.
#[test]
fn standard_and_built_in_runs_keep_their_clock() {
    let mut sim = Sim::new(world(Some(strafer()), Trigger::Hold, 0.8, 8.0), 11);
    play(&mut sim, 11.0, |sim| on_it(sim, true));
    assert_eq!(sim.rate.changes, vec![(0.0, 0.0, 0.8)]);
    assert!((sim.elapsed() - 8.0).abs() < 1e-6);
    assert!((sim.result().duration_s - 10.0).abs() < 1e-3);
    let builtin = Scenario { world: None, ..world(None, Trigger::Click, 1.0, 5.0) };
    let mut sim = Sim::adaptive(builtin, 12, None, Mix::Balanced);
    play(&mut sim, 6.0, |_| away());
    assert!(sim.rate.is_real());
    assert_eq!(sim.elapsed(), 5.0);
}
