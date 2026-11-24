//! Adaptive runs of imported files (BACKLOG 9, Phase 5; docs/plans/2026-11-imported-adaptive.md), played on
//! made-up fixtures: Slow Taps (a click weapon at half speed), Still Hose (a held weapon on still balls), Slow
//! Strafe (a held weapon on a strafing ball at 80%) and Short Fuse (balls that blow up by themselves).

use std::path::{Path, PathBuf};

use aimadapt_sce::import;
use aimadapt_sim::adaptive::{ImportedRule, Knob, Mix, Pick, Prior, imported_rule, logit_with_slips};
use aimadapt_sim::{Input, Phase, RunResult, Scenario, Sim, Vec3};

const FRAME: f32 = 1.0 / 240.0;

fn scenario(name: &str) -> Scenario {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(format!("AimAdapt Test - {name}.sce"));
    import(&path).unwrap_or_else(|g| panic!("{name}: {g:?}")).scenario
}

/// A rating of `value` from an earlier run, so the next run plays at one value near it.
fn prior_at(knob: Knob, value: f32) -> Prior {
    Prior {
        theta: knob.h(value) + 0.12 * logit_with_slips(0.85),
        slope: 0.12,
        sd: 0.08,
        slope_sd: None,
        days: 1.0,
        fastest: None,
        last_run: Some((Pick::Near, 0.85)),
    }
}

fn away() -> Input {
    Input { aim: Vec3::Y, fire_pressed: false, fire_held: false }
}

/// Plays the run to its end (at most `secs` real seconds), with `input` deciding each frame.
fn play(mut sim: Sim, secs: f32, mut input: impl FnMut(&Sim) -> Input) -> RunResult {
    for _ in 0..(secs * 240.0).round() as usize {
        if sim.phase() == Phase::Finished {
            break;
        }
        let i = input(&sim);
        sim.step(FRAME, i);
    }
    assert_eq!(sim.phase(), Phase::Finished, "the run ended within {secs} s");
    sim.result()
}

/// Waits out the countdown, which is the same at any speed.
fn after_countdown(sim: &Sim, then: impl FnOnce() -> Input) -> Input {
    if sim.phase() == Phase::Countdown { away() } else { then() }
}

#[test]
fn each_fixture_gets_the_rule_and_knob_of_its_kind() {
    for (name, rule, knob) in [
        ("Slow Taps", ImportedRule::Pace, Knob::KillPace),
        ("Still Hose", ImportedRule::Pace, Knob::KillPace),
        ("Still Wall", ImportedRule::Pace, Knob::KillPace),
        ("Floor Keeper", ImportedRule::Pace, Knob::KillPace),
        ("Slow Strafe", ImportedRule::Tracking, Knob::TimeScale),
        ("Dash Lane", ImportedRule::Tracking, Knob::TimeScale),
        ("Short Fuse", ImportedRule::Timed, Knob::TimeScale),
        ("Holding Pens", ImportedRule::Timed, Knob::TimeScale),
    ] {
        let s = scenario(name);
        assert_eq!(imported_rule(&s), Some(rule), "{name}");
        assert_eq!(s.adaptive.resolve(&s).knob, knob, "{name}");
    }
}

/// Slow Taps runs at half speed with a 10 s timer: 20 real seconds. Its kill pace windows are real seconds, so
/// a click on a ball every 0.6 windows is a success each time, and every 1.7 windows a failure.
#[test]
fn slow_taps_is_rated_on_kill_pace_in_real_seconds() {
    let s = scenario("Slow Taps");
    assert_eq!(s.world.as_ref().unwrap().time_scale, 0.5);
    let prior = prior_at(Knob::KillPace, 1.0);
    let value = Sim::adaptive(s.clone(), 1, Some(prior), Mix::Balanced).run_pick().unwrap().0;
    let tap_every = |every: f32| {
        let mut next = every;
        play(Sim::adaptive(s.clone(), 1, Some(prior), Mix::Balanced), 30.0, |sim| {
            after_countdown(sim, || {
                let Some(t) = sim.targets().first() else { return away() };
                if sim.real_elapsed() < next {
                    return away();
                }
                next += every;
                Input { aim: t.pos.normalize(), fire_pressed: true, fire_held: true }
            })
        })
    };
    let fast = tap_every(value * 0.6);
    assert!((fast.duration_s - 20.0).abs() < 0.01, "{} real seconds", fast.duration_s);
    let a = fast.adaptive.unwrap();
    assert_eq!(a.knob, Knob::KillPace);
    assert_eq!(a.success_rate, Some(1.0), "{a:?}");
    assert!(a.outcomes >= 25, "{}", a.outcomes);
    // The kill times the run keeps are real seconds: one every 0.6 windows.
    let gaps: Vec<f32> = fast.kills.windows(2).map(|k| k[1].at_s - k[0].at_s).collect();
    assert!(gaps.iter().all(|g| (g - value * 0.6).abs() < 0.02), "{gaps:?}");
    let slow = tap_every(value * 1.7);
    assert_eq!(slow.adaptive.unwrap().success_rate, Some(0.0));
}

/// Still Hose's weapon fires while the button is held, so sweeping from one ball to the next with the button
/// down misses in between. Kill pace has no miss rule for a held weapon: the kills in time are successes.
#[test]
fn still_hose_rates_kills_in_time_whatever_the_held_fire_misses() {
    let s = scenario("Still Hose");
    let r = play(Sim::adaptive(s, 2, Some(prior_at(Knob::KillPace, 1.5)), Mix::Balanced), 30.0, |sim| {
        after_countdown(sim, || {
            let aim = sim.targets().first().map_or(Vec3::NEG_Z, |t| t.pos.normalize());
            // The first 0.1 s of each second sprays the wall.
            let aim = if sim.real_elapsed().fract() < 0.1 { (aim + Vec3::Y * 0.2).normalize() } else { aim };
            Input { aim, fire_pressed: false, fire_held: true }
        })
    });
    assert!(r.stats.misses > 20 && r.stats.kills > 20, "{:?}", r.stats);
    let a = r.adaptive.unwrap();
    assert_eq!(a.knob, Knob::KillPace);
    assert_eq!(a.success_rate, Some(1.0), "{a:?}");
}

/// Slow Strafe runs at 80% with a 16 s timer: 20 real seconds. A first run calibrates the speed segment by
/// segment; on target all the way, it rates the player above the file's own speed. A later run plays at one
/// speed, its game clock at 0.8 times that, and still takes 20 real seconds.
#[test]
fn slow_strafe_is_rated_on_speed_by_the_share_of_time_on_the_ball() {
    let s = scenario("Slow Strafe");
    let on_ball = |sim: &Sim| {
        after_countdown(sim, || {
            let aim = sim.targets().first().map_or(Vec3::NEG_Z, |t| t.pos.normalize());
            Input { aim, fire_pressed: false, fire_held: true }
        })
    };
    let first = play(Sim::adaptive(s.clone(), 3, None, Mix::Balanced), 30.0, on_ball);
    assert!((first.duration_s - 20.0).abs() < 0.01, "{} real seconds", first.duration_s);
    let a = first.adaptive.unwrap();
    assert!(a.calibration());
    assert_eq!(a.knob, Knob::TimeScale);
    assert_eq!(a.success_rate, Some(1.0), "{a:?}");
    assert!(a.value > 1.5, "×{}", a.value);
    assert!(first.stats.damage_done > 100.0, "the held weapon hit the ball: {:?}", first.stats);

    // The ball moves: off it, the share is nothing.
    let still_aim = play(Sim::adaptive(s.clone(), 3, None, Mix::Balanced), 30.0, |sim| {
        after_countdown(sim, || Input { aim: Vec3::new(0.3, 0.0, -1.0).normalize(), ..away() })
    });
    assert_eq!(still_aim.adaptive.unwrap().success_rate, Some(0.0));

    let mut sim = Sim::adaptive(s.clone(), 4, Some(prior_at(Knob::TimeScale, 0.6)), Mix::Balanced);
    let (value, _) = sim.run_pick().unwrap();
    assert!((sim.time_scale() - 0.8 * value).abs() < 1e-6);
    let mut game = 0.0;
    for _ in 0..(30.0 * 240.0) as usize {
        if sim.phase() == Phase::Finished {
            break;
        }
        let i = on_ball(&sim);
        sim.step(FRAME, i);
        game = sim.elapsed();
    }
    assert!((game - 16.0 * value).abs() < 0.01, "{game} game seconds at ×{value}");
    assert!((sim.result().duration_s - 20.0).abs() < 0.01);
}

/// Short Fuse's balls blow up by themselves: each is one outcome, a kill a success and a blast a failure.
#[test]
fn short_fuse_rates_each_ball() {
    let s = scenario("Short Fuse");
    let prior = prior_at(Knob::TimeScale, 1.0);
    let gone = play(Sim::adaptive(s.clone(), 5, Some(prior), Mix::Balanced), 60.0, |_| away());
    let a = gone.adaptive.unwrap();
    assert!(gone.stats.timeouts >= 5, "{:?}", gone.stats);
    assert_eq!(a.outcomes as u32, gone.stats.timeouts);
    assert_eq!(a.success_rate, Some(0.0));

    // With the file's own shots of 15 only the first ball can be killed (its blast comes before a second shot),
    // so the player here has shots of 30. A click on a ball every 0.25 s from when it is first seen; the ball,
    // the clicks on it so far and when it was first seen are kept between frames.
    let mut strong = s;
    strong.weapon.damage = 30.0;
    let pressed = std::cell::Cell::new((0, 0, 0.0));
    let shot = play(Sim::adaptive(strong, 5, Some(prior), Mix::Balanced), 60.0, |sim| {
        after_countdown(sim, || {
            let Some(t) = sim.targets().first() else { return away() };
            let (no, n, seen) = pressed.get();
            let (n, seen) = if no == t.no() { (n, seen) } else { (0, sim.elapsed()) };
            let press = sim.elapsed() - seen >= n as f32 * 0.25;
            pressed.set((t.no(), n + u32::from(press), seen));
            Input { aim: t.pos.normalize(), fire_pressed: press, fire_held: press }
        })
    });
    assert!(shot.stats.kills >= 10 && shot.stats.timeouts == 0, "{:?}", shot.stats);
    assert_eq!(shot.adaptive.unwrap().success_rate, Some(1.0));
}
