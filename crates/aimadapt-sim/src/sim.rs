//! The simulation: target spawning, movement, hit detection, scoring, adaptive
//! difficulty and telemetry. It knows nothing about rendering or input devices. The
//! engine feeds it the aim direction and trigger state each frame and draws whatever
//! targets it reports, which keeps all the rules testable without a window.
//!
//! Coordinates: the player's eye is the origin, -Z is straight ahead, +Y is up and
//! +X is right (the same as a Bevy camera with no rotation).

use glam::Vec3;

use crate::adaptive::{Knob, Mix, Pick, Prior, Rating};
use crate::scenario::{Movement, MovementKind, Range, Scenario, Shape, Trigger, WeaponKind};
use crate::telemetry::{
    Outcome, ShotRecord, TargetFrame, TargetPose, TargetRecord, Telemetry, TrackSample, angle_deg, screen_offset,
};

/// Beam damage and accuracy are sampled at this rate, independent of frame rate.
pub const BEAM_TICK_HZ: f32 = 60.0;
/// Adaptive clicking: a kill after more misses than this counts as a failure, so
/// spraying clicks can't fake a fast rating.
pub const MAX_MISSES_PER_TARGET: u32 = 1;
/// Tracking telemetry sample rate.
const SAMPLE_HZ: f32 = 30.0;
/// Aim trace rate.
const TRACE_HZ: f32 = 240.0;
/// Replay frames: where every target is, this many times a second.
const FRAME_HZ: f32 = 60.0;

/// What the player is doing this frame.
#[derive(Debug, Clone, Copy)]
pub struct Input {
    /// Unit vector the crosshair points along.
    pub aim: Vec3,
    /// The fire button went down since the last step.
    pub fire_pressed: bool,
    /// The fire button is down.
    pub fire_held: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Countdown,
    Running,
    Finished,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub id: u64,
    pub pos: Vec3,
    /// Current radius (shrinks over the lifetime if the scenario says so).
    pub radius: f32,
    pub health: f32,
    pub max_health: f32,
    /// Position as angles (degrees) and distance from the player; movement happens
    /// in these terms so a target keeps its size and height as it strafes.
    yaw: f32,
    pitch: f32,
    distance: f32,
    /// Degrees per second of yaw and pitch, and the velocity it's turning toward
    /// (the same unless the scenario sets an acceleration).
    rate: [f32; 2],
    goal: [f32; 2],
    /// Forward/back: meters per second away from the player (negative = closer),
    /// the speed it's heading for, and seconds until it switches.
    radial: f32,
    radial_goal: f32,
    next_radial: f32,
    /// Jumps: meters above its base height, upward speed, seconds until the next.
    lift: f32,
    lift_v: f32,
    next_jump: f32,
    /// Seconds into the run of the last hit, for the regen delay.
    last_hit_at: f32,
    /// Curve movement: how fast the heading turns, degrees per second (negative
    /// turns the other way).
    turn: f32,
    next_change: f32,
    last_change: f32,
    age: f32,
    spawned_at: f32,
    /// Lifetime and size it was given at spawn (adaptive mode changes them over time).
    lifetime: f32,
    /// Adaptive hitscan target speed: this target's own speed range (m/s), kept
    /// through direction changes.
    speed: Option<Range>,
    base_radius: f32,
    height: f32,
    shots: u32,
    hits: u32,
    /// Misses while this was the nearest target.
    misses: u32,
    damage: f32,
    track: Track,
}

/// Telemetry for one target while it's alive.
#[derive(Debug, Clone, PartialEq)]
struct Track {
    no: u32,
    start_at: f32,
    origin: Vec3,
    distance_deg: f32,
    /// Unit flick direction on screen.
    flick: (f32, f32),
    radius_deg: f32,
    reaction_s: Option<f32>,
    reach_s: Option<f32>,
    inside: bool,
    overshoot_deg: f32,
    entries: u32,
    /// Adaptive mode: the knob value this target was given, its planned chance of
    /// success, and what kind of pick it was.
    value: Option<(f32, f32, Pick)>,
}

impl Track {
    fn begin(no: u32, now: f32, origin: Vec3, pos: Vec3, radius: f32, value: Option<(f32, f32, Pick)>) -> Track {
        let mut t = Track {
            no,
            start_at: now,
            origin,
            distance_deg: 0.0,
            flick: (1.0, 0.0),
            radius_deg: 0.0,
            reaction_s: None,
            reach_s: None,
            inside: false,
            overshoot_deg: 0.0,
            entries: 0,
            value,
        };
        t.restart(now, origin, pos, radius);
        t
    }

    /// Starts timing the flick again from where the crosshair is now.
    fn restart(&mut self, now: f32, origin: Vec3, pos: Vec3, radius: f32) {
        let (x, y) = screen_offset(origin, pos);
        let len = (x * x + y * y).sqrt();
        self.start_at = now;
        self.origin = origin;
        self.distance_deg = angle_deg(origin, pos);
        self.flick = if len > 1e-4 { (x / len, y / len) } else { (1.0, 0.0) };
        self.radius_deg = (radius / pos.length().max(0.1)).atan().to_degrees();
        self.reaction_s = None;
        self.reach_s = None;
        self.overshoot_deg = 0.0;
        self.entries = u32::from(self.inside);
    }
}

impl Target {
    /// Health left as a share of full health.
    pub fn health_fraction(&self) -> f32 {
        (self.health / self.max_health).clamp(0.0, 1.0)
    }

    /// Seconds of lifetime left, if it has one.
    pub fn time_left(&self) -> Option<f32> {
        (self.lifetime > 0.0).then(|| (self.lifetime - self.age).max(0.0))
    }

    /// Lifetime left as a share of the whole, if it has one.
    pub fn time_left_fraction(&self) -> Option<f32> {
        self.time_left().map(|t| (t / self.lifetime).clamp(0.0, 1.0))
    }

    /// Capsule height now (it scales with the radius when a target shrinks).
    pub fn height(&self) -> f32 {
        self.height * self.radius / self.base_radius.max(1e-6)
    }

    /// Spawn order, from 1.
    pub fn no(&self) -> u32 {
        self.track.no
    }

    /// Damaged and not yet healed back to full.
    pub fn engaged(&self) -> bool {
        self.health > 1e-4 && self.health < self.max_health - 1e-4
    }

    /// Where it is now, as the player sees it, for replays.
    fn pose(&self, shape: Shape) -> TargetPose {
        let (yaw_deg, pitch_deg) = angles(self.pos);
        let distance = self.pos.length().max(0.1);
        let height_deg = match shape {
            Shape::Sphere => 0.0,
            Shape::Capsule => 2.0 * (self.height() / 2.0 / distance).atan().to_degrees(),
        };
        TargetPose {
            no: self.track.no,
            yaw_deg,
            pitch_deg,
            radius_deg: (self.radius / distance).atan().to_degrees(),
            height_deg,
            health: self.health_fraction(),
        }
    }

    fn hit_by(&self, aim: Vec3, shape: Shape) -> Option<f32> {
        let h = self.height * self.radius / self.base_radius.max(1e-6);
        ray_hit(aim, self.pos, self.radius, shape, h)
    }

    fn record(&self, ended_at: f32, outcome: Outcome) -> TargetRecord {
        let k = &self.track;
        let direction_deg = k.flick.1.atan2(k.flick.0).to_degrees();
        TargetRecord {
            no: k.no,
            spawned_at: self.spawned_at,
            start_at: k.start_at,
            ended_at,
            outcome,
            distance_deg: k.distance_deg,
            direction_deg,
            radius_deg: k.radius_deg,
            lifetime_s: self.lifetime,
            reaction_s: k.reaction_s,
            reach_s: k.reach_s,
            overshoot_deg: k.overshoot_deg,
            entries: k.entries,
            shots: self.shots,
            hits: self.hits,
            value: k.value.map(|v| v.0),
            chance: k.value.map(|v| v.1),
            pick: k.value.map(|v| v.2.as_str()),
        }
    }
}

/// Things that happened during a step, for effects and sounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Spawned {
        id: u64,
    },
    Hit {
        id: u64,
    },
    Killed {
        id: u64,
    },
    Miss,
    TimedOut {
        id: u64,
    },
    /// Adaptive mode, at the end of a run: the goal rate held at a value harder
    /// than the player's best.
    Breakthrough {
        value: f32,
    },
    Finished,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    /// Hitscan shots fired, or beam ticks while firing.
    pub shots: u32,
    /// Shots or beam ticks that hit a target.
    pub hits: u32,
    pub misses: u32,
    pub kills: u32,
    pub timeouts: u32,
    pub damage_done: f32,
    /// Damage that would have been done if every shot or tick hit.
    pub damage_possible: f32,
}

impl Stats {
    pub fn accuracy(&self) -> Option<f32> {
        (self.shots > 0).then(|| self.hits as f32 / self.shots as f32)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KillRecord {
    /// Seconds into the run when the target was destroyed.
    pub at_s: f32,
    /// Seconds from when the player could start on the target (it appearing, the
    /// clock starting, or the previous kill, whichever is latest) to its destruction.
    pub ttk_s: f32,
    pub shots: u32,
    pub hits: u32,
    pub damage: f32,
}

/// How an adaptive run went.
#[derive(Debug, Clone, PartialEq)]
pub struct AdaptiveSummary {
    pub knob: Knob,
    /// The rating: the knob value hit at the goal rate, and a ±1 standard deviation range.
    pub value: f32,
    pub range: (f32, f32),
    /// The rating going in.
    pub start_value: f32,
    /// Model parameters, kept to start the next run (log scale).
    pub theta: f32,
    pub slope: f32,
    pub sd: f32,
    pub goal: f32,
    pub success_rate: Option<f32>,
    pub outcomes: usize,
    /// This run's value, if it held the goal rate (fixed-difficulty runs only).
    pub fastest: Option<f32>,
    /// (seconds, rating), thinned to at most 300 points.
    pub timeline: Vec<(f32, f32)>,
    /// The value this run was played at, or `None` for a calibration run.
    pub run_value: Option<f32>,
    /// What kind of run it was (`Calibrate` for a calibration run).
    pub pick: Pick,
    /// The chance of success the run was planned at.
    pub planned_chance: Option<f32>,
    pub breakthrough: bool,
}

impl AdaptiveSummary {
    pub fn calibration(&self) -> bool {
        self.run_value.is_none()
    }
}

/// The finished run, ready to save.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    /// Points, or the rating (in the knob's units) for adaptive runs.
    pub score: f32,
    pub stats: Stats,
    pub kills: Vec<KillRecord>,
    /// Seconds of play (excludes the countdown).
    pub duration_s: f32,
    pub adaptive: Option<AdaptiveSummary>,
    pub telemetry: Telemetry,
}

impl RunResult {
    pub fn avg_ttk_s(&self) -> Option<f32> {
        (!self.kills.is_empty()).then(|| self.kills.iter().map(|k| k.ttk_s).sum::<f32>() / self.kills.len() as f32)
    }
}

/// Adaptive state for a run.
struct AdaptiveRun {
    rating: Rating,
    mix: Mix,
    /// Tracking: the current segment's value, planned chance and pick.
    segment: Option<(f32, f32, Pick)>,
    /// The whole run's value, planned chance and pick. `None` for a calibration
    /// run, where every target or segment gets its own value.
    run: Option<(f32, f32, Pick)>,
    breakthrough: bool,
}

impl AdaptiveRun {
    /// The difficulty for the next target or segment.
    fn next(&mut self, rng: &mut Rng) -> (f32, f32, Pick) {
        match self.run {
            Some(run) => run,
            None => self.rating.pick(rng, self.mix),
        }
    }
}

/// The speed range (m/s) that moves targets at `deg_s` across the view at the
/// scenario's mean distance.
fn speed_for(s: &Scenario, deg_s: f32) -> Range {
    let sp = &s.targets.spawn;
    let d = (sp.distance.0 + sp.distance.1) / 2.0;
    let mps = deg_s.to_radians() * d;
    Range(mps * 0.85, mps)
}

/// Kill-pace rating: the current window between kills.
#[derive(Debug, Clone, Copy, Default)]
struct Pace {
    /// When the window started: the clock start, the last kill, or the end of the
    /// last window that ran out.
    since: f32,
    /// A window has run out since the last kill, so the next kill isn't a success.
    missed: bool,
    /// This window's value, planned chance and pick.
    window: Option<(f32, f32, Pick)>,
    /// Clicking misses since the last kill: a kill after a spray of them isn't a
    /// success, as with the lifetime knob.
    misses: u32,
}

pub struct Sim {
    /// The scenario as written.
    scenario: Scenario,
    /// What's being played: `scenario` with the adaptive knob applied.
    current: Scenario,
    adaptive: Option<AdaptiveRun>,
    rng: Rng,
    /// Seconds since the countdown began.
    clock: f32,
    phase: Phase,
    targets: Vec<Target>,
    next_id: u64,
    next_no: u32,
    /// Running-clock times at which a replacement target should appear.
    pending: Vec<f32>,
    last_killed_dir: Option<Vec3>,
    /// Seconds into the run of the last kill.
    last_kill_at: f32,
    last_aim: Vec3,
    stats: Stats,
    kills: Vec<KillRecord>,
    cooldown: f32,
    beam_acc: f32,
    /// Ticks, on-target ticks and seconds in the current adaptive tracking segment.
    window: (u32, u32, f32),
    pace: Pace,
    telemetry: Telemetry,
    next_sample: f32,
    next_trace: f32,
    next_frame: f32,
}

impl Sim {
    /// A standard run at the scenario's own difficulty.
    pub fn new(scenario: Scenario, seed: u64) -> Sim {
        Sim::build(scenario, seed, None)
    }

    /// An adaptive run. With no earlier rating it's a calibration run, which sets
    /// each target's (or segment's) difficulty as it goes to find the player's
    /// level. Otherwise the whole run plays at one difficulty picked from the mix
    /// around the rating, and the rating takes in the run as it's played.
    pub fn adaptive(scenario: Scenario, seed: u64, prior: Option<Prior>, mix: Mix) -> Sim {
        let cfg = scenario.adaptive.resolve(&scenario);
        let rating = Rating::new(cfg, prior);
        let run = prior.is_some().then(|| rating.pick_run(&mut Rng::new(seed ^ 0x9e37_79b9), mix));
        let mut sim =
            Sim::build(scenario, seed, Some(AdaptiveRun { rating, mix, segment: None, run, breakthrough: false }));
        if let Some((value, _, _)) = run {
            let t = &mut sim.current.targets;
            match cfg.knob {
                Knob::Lifetime => t.lifetime_s = value,
                Knob::Radius => t.radius = value,
                Knob::TargetSpeed => t.movement.speed = speed_for(&sim.scenario, value),
                Knob::KillPace | Knob::Auto => {}
            }
            sim.respawn_all();
        }
        sim
    }

    /// Replaces the opening targets, so they use the current difficulty.
    fn respawn_all(&mut self) {
        self.targets.clear();
        self.next_id = 1;
        self.next_no = 1;
        let mut events = Vec::new();
        for _ in 0..self.scenario.targets.count {
            self.spawn(&mut events);
        }
    }

    fn build(base: Scenario, seed: u64, adaptive: Option<AdaptiveRun>) -> Sim {
        let current = base.clone();
        let mut sim = Sim {
            scenario: base,
            current,
            adaptive,
            rng: Rng::new(seed),
            clock: 0.0,
            phase: Phase::Countdown,
            targets: Vec::new(),
            next_id: 1,
            next_no: 1,
            pending: Vec::new(),
            last_killed_dir: None,
            last_kill_at: 0.0,
            last_aim: Vec3::NEG_Z,
            stats: Stats::default(),
            kills: Vec::new(),
            cooldown: 0.0,
            beam_acc: 0.0,
            window: (0, 0, 0.0),
            pace: Pace::default(),
            telemetry: Telemetry::default(),
            next_sample: 0.0,
            next_trace: 0.0,
            next_frame: 0.0,
        };
        sim.start_segment();
        let mut events = Vec::new();
        for _ in 0..sim.scenario.targets.count {
            sim.spawn(&mut events);
        }
        if sim.scenario.timing.countdown_s <= 0.0 {
            sim.phase = Phase::Running;
        }
        sim
    }

    /// The scenario as written (name, arena, timing).
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    /// The scenario at the current difficulty.
    pub fn current(&self) -> &Scenario {
        &self.current
    }

    pub fn is_adaptive(&self) -> bool {
        self.adaptive.is_some()
    }

    /// Adaptive mode: the knob, current rating and success rate so far.
    pub fn rating(&self) -> Option<(Knob, f32, Option<f32>)> {
        self.adaptive.as_ref().map(|a| (a.rating.cfg.knob, a.rating.value(), a.rating.success_rate()))
    }

    /// Kill-pace rating: the seconds allowed for the next kill and how many are left
    /// (negative once the window has run out).
    pub fn pace(&self) -> Option<(f32, f32)> {
        if self.knob() != Some(Knob::KillPace) {
            return None;
        }
        let value = self.pace.window.map(|w| w.0).or(self.run_pick().map(|r| r.0))?;
        Some((value, value - (self.elapsed() - self.pace.since)))
    }

    /// Adaptive mode: whether each target is one outcome (a kill in time) rather
    /// than tracking segments or kill pace.
    pub fn rated_per_target(&self) -> bool {
        self.per_target()
    }

    /// Kills per minute so far.
    pub fn kills_per_min(&self) -> Option<f32> {
        let t = self.elapsed();
        (t >= 5.0).then(|| self.stats.kills as f32 * 60.0 / t)
    }

    fn knob(&self) -> Option<Knob> {
        self.adaptive.as_ref().map(|a| a.rating.cfg.knob)
    }

    /// Adaptive mode: the value this whole run plays at and what kind of run it is,
    /// or `None` for a calibration run (and standard runs).
    pub fn run_pick(&self) -> Option<(f32, Pick)> {
        self.adaptive.as_ref().and_then(|a| a.run).map(|(v, _, p)| (v, p))
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    /// Seconds of play so far (0 during the countdown).
    pub fn elapsed(&self) -> f32 {
        (self.clock - self.scenario.timing.countdown_s).clamp(0.0, self.scenario.timing.duration_s)
    }

    pub fn time_left(&self) -> f32 {
        self.scenario.timing.duration_s - self.elapsed()
    }

    pub fn countdown_left(&self) -> f32 {
        (self.scenario.timing.countdown_s - self.clock).max(0.0)
    }

    /// Points by the scenario's scoring rules (shown for standard runs).
    pub fn score(&self) -> f32 {
        let s = &self.scenario.scoring;
        let st = &self.stats;
        let points = s.per_kill * st.kills as f32
            + s.per_hit * st.hits as f32
            + s.per_miss * st.misses as f32
            + s.per_damage * st.damage_done
            + s.per_timeout * st.timeouts as f32;
        // Imported scenarios can scale the score by accuracy, as KovaaK's does.
        match &self.scenario.world {
            Some(w) => points * w.accuracy_mult.factor(st.accuracy()),
            None => points,
        }
    }

    pub fn result(&self) -> RunResult {
        let mut telemetry = self.telemetry.clone();
        let now = self.elapsed();
        telemetry.targets.extend(self.targets.iter().map(|t| t.record(now, Outcome::Open)));
        telemetry.targets.sort_by_key(|t| t.no);
        let adaptive = self.adaptive.as_ref().map(|a| {
            let r = &a.rating;
            let step = r.timeline.len().div_ceil(300).max(1);
            let mut timeline: Vec<(f32, f32)> = r.timeline.iter().step_by(step).copied().collect();
            if let Some(last) = r.timeline.last()
                && timeline.last() != Some(last)
            {
                timeline.push(*last);
            }
            let e = r.estimate();
            AdaptiveSummary {
                knob: r.cfg.knob,
                value: r.value(),
                range: r.range(),
                start_value: r.start_value,
                theta: e.theta,
                slope: e.slope,
                sd: e.sd,
                goal: r.cfg.goal,
                success_rate: r.success_rate(),
                outcomes: r.outcomes(),
                fastest: r.fastest,
                timeline,
                run_value: a.run.map(|x| x.0),
                pick: a.run.map_or(Pick::Calibrate, |x| x.2),
                planned_chance: a.run.map(|x| x.1),
                breakthrough: a.breakthrough,
            }
        });
        RunResult {
            score: adaptive.as_ref().map_or(self.score(), |a| a.value),
            stats: self.stats.clone(),
            kills: self.kills.clone(),
            duration_s: self.elapsed(),
            adaptive,
            telemetry,
        }
    }

    /// Advances the simulation by `dt` seconds with this frame's input.
    pub fn step(&mut self, dt: f32, input: Input) -> Vec<Event> {
        let events = Vec::new();
        if self.phase == Phase::Finished || dt <= 0.0 {
            return events;
        }
        self.clock += dt;
        if self.phase == Phase::Countdown {
            self.last_aim = input.aim.normalize_or(Vec3::NEG_Z);
            if self.clock < self.scenario.timing.countdown_s {
                return events;
            }
            self.phase = Phase::Running;
            // Flicks are timed from when the clock starts, not from the countdown.
            let aim = self.last_aim;
            for t in &mut self.targets {
                t.track.restart(0.0, aim, t.pos, t.radius);
            }
            // Only the part of this step past the countdown counts as play.
            let over = self.clock - self.scenario.timing.countdown_s;
            return self.run_step(over, input, events);
        }
        self.run_step(dt, input, events)
    }

    fn run_step(&mut self, dt: f32, input: Input, mut events: Vec<Event>) -> Vec<Event> {
        let aim = input.aim.normalize_or(Vec3::NEG_Z);
        self.last_aim = aim;
        let now = self.elapsed();

        self.move_targets(dt, &mut events);
        self.pace_tick(now);
        self.spawn_due(now, &mut events);
        self.track_aim(now, aim);
        match self.scenario.weapon.kind {
            WeaponKind::Hitscan => self.fire_hitscan(dt, aim, input, &mut events),
            WeaponKind::Beam => self.fire_beam(dt, aim, input, &mut events),
        }
        // Targets with no respawn delay come back in the same frame.
        self.spawn_due(now, &mut events);

        let limit = self.scenario.timing.kill_limit;
        if self.time_left() <= 0.0 || (limit > 0 && self.stats.kills >= limit) {
            self.phase = Phase::Finished;
            if let Some(a) = &mut self.adaptive
                && let Some((value, _, _)) = a.run
            {
                a.breakthrough = a.rating.finish_run(value);
                if a.breakthrough {
                    events.push(Event::Breakthrough { value });
                }
            }
            events.push(Event::Finished);
        }
        events
    }

    /// Whether the adaptive knob is set per target (lifetime, or size or speed when
    /// clicking) rather than per tracking segment. Tracking segments come from beam
    /// ticks, so a hitscan scenario has to be rated target by target.
    fn per_target(&self) -> bool {
        self.adaptive.as_ref().is_some_and(|a| {
            let hitscan = self.scenario.weapon.kind == WeaponKind::Hitscan;
            match a.rating.cfg.knob {
                Knob::Lifetime => true,
                Knob::Radius => hitscan,
                // With a beam and target lifetimes, each target is a kill to finish in
                // time (regen makes that a test of unbroken tracking).
                Knob::TargetSpeed => hitscan || self.scenario.targets.lifetime_s > 0.0,
                Knob::KillPace | Knob::Auto => false,
            }
        })
    }

    /// Kill pace: the current window's value, picking one if needed.
    fn pace_window(&mut self) -> Option<(f32, f32, Pick)> {
        if self.knob() != Some(Knob::KillPace) {
            return None;
        }
        if self.pace.window.is_none() {
            let a = self.adaptive.as_mut()?;
            self.pace.window = Some(a.next(&mut self.rng));
        }
        self.pace.window
    }

    /// Kill pace: each window that runs out with no kill is a failure.
    fn pace_tick(&mut self, now: f32) {
        for _ in 0..100 {
            let Some((value, _, _)) = self.pace_window() else { return };
            if now - self.pace.since < value {
                return;
            }
            self.outcome(value, 0.0, false);
            self.pace.since += value;
            self.pace.missed = true;
            self.pace.window = None;
        }
    }

    /// Kill pace: a kill inside the window is a success, and starts the next one.
    fn pace_kill(&mut self, now: f32) {
        let Some((value, _, _)) = self.pace_window() else { return };
        if !self.pace.missed && now - self.pace.since <= value {
            let ok = self.pace.misses <= MAX_MISSES_PER_TARGET;
            self.outcome(value, if ok { 1.0 } else { 0.0 }, false);
        }
        self.pace = Pace { since: now, ..Pace::default() };
    }

    /// Feeds an outcome at a knob value to the rating.
    fn outcome(&mut self, value: f32, outcome: f32, tracking: bool) {
        let now = self.elapsed();
        if let Some(a) = &mut self.adaptive {
            a.rating.update(now, value, outcome, tracking);
        }
    }

    /// Tracking: picks the next segment's difficulty and applies it to every target.
    /// In a fixed-difficulty run every segment keeps the run's value.
    fn start_segment(&mut self) {
        if self.per_target() {
            return;
        }
        let Some(a) = &mut self.adaptive else { return };
        if a.rating.cfg.knob == Knob::KillPace || (a.run.is_some() && a.segment.is_some()) {
            return;
        }
        let (value, chance, pick) = a.next(&mut self.rng);
        a.segment = Some((value, chance, pick));
        if a.run.is_some() {
            // Already applied to the scenario before the targets spawned.
            return;
        }
        match a.rating.cfg.knob {
            Knob::TargetSpeed => {
                self.current.targets.movement.speed = speed_for(&self.scenario, value);
                // Change speed now, keeping each target's direction.
                for t in &mut self.targets {
                    let screen = (t.rate[0] * t.pitch.to_radians().cos(), t.rate[1]);
                    let now = (screen.0 * screen.0 + screen.1 * screen.1).sqrt();
                    if now > 1e-3 {
                        let k = value * (0.85 + 0.15 * self.rng.next_f32()) / now;
                        t.rate = [t.rate[0] * k, t.rate[1] * k];
                    }
                }
            }
            Knob::Radius => {
                self.current.targets.radius = value;
                for t in &mut self.targets {
                    t.radius = value;
                    t.base_radius = value;
                }
            }
            Knob::Lifetime | Knob::KillPace | Knob::Auto => {}
        }
    }

    /// Updates per-target flick telemetry, tracking samples and the aim trace.
    fn track_aim(&mut self, now: f32, aim: Vec3) {
        let shape = self.scenario.targets.shape;
        for t in &mut self.targets {
            let on = t.hit_by(aim, shape).is_some();
            let k = &mut t.track;
            let since = now - k.start_at;
            if k.reaction_s.is_none() {
                let moved = k.distance_deg - angle_deg(aim, t.pos);
                if moved >= (0.1 * k.distance_deg).max(0.3) {
                    k.reaction_s = Some(since);
                }
            }
            if on && !k.inside {
                k.entries += 1;
                if k.reach_s.is_none() {
                    k.reach_s = Some(since);
                }
            }
            k.inside = on;
            if k.reach_s.is_some() {
                let (x, y) = screen_offset(t.pos.normalize(), aim);
                let along = x * k.flick.0 + y * k.flick.1;
                k.overshoot_deg = k.overshoot_deg.max(along - k.radius_deg);
            }
        }

        if now >= self.next_trace {
            self.next_trace = now + 1.0 / TRACE_HZ;
            let (yaw, pitch) = angles(aim);
            self.telemetry.trace.push([now, yaw, pitch]);
        }

        if now >= self.next_frame {
            // Step on from the last frame's slot, not from now, so frames keep to
            // 60 Hz instead of slipping to every third game frame.
            self.next_frame = (self.next_frame + 1.0 / FRAME_HZ).max(now);
            let targets = self.targets.iter().map(|t| t.pose(shape)).collect();
            self.telemetry.frames.push(TargetFrame { t: now, targets });
        }

        if self.scenario.weapon.kind == WeaponKind::Beam && now >= self.next_sample {
            self.next_sample = now + 1.0 / SAMPLE_HZ;
            let nearest = self.targets.iter().min_by(|a, b| angle_deg(aim, a.pos).total_cmp(&angle_deg(aim, b.pos)));
            if let Some(t) = nearest {
                let motion = (t.rate[0] * t.pitch.to_radians().cos(), t.rate[1]);
                let speed = (motion.0 * motion.0 + motion.1 * motion.1).sqrt();
                let (x, y) = screen_offset(t.pos.normalize(), aim);
                let lead = if speed > 1e-3 { (x * motion.0 + y * motion.1) / speed } else { 0.0 };
                self.telemetry.samples.push(TrackSample {
                    t: now,
                    on_target: t.hit_by(aim, shape).is_some(),
                    error_deg: angle_deg(aim, t.pos),
                    target_speed_dps: speed,
                    lead_deg: lead,
                    since_change_s: now - t.last_change,
                    engaged: self.targets.iter().any(Target::engaged),
                });
            }
        }
    }

    /// Replaces destroyed targets whose delay is up.
    fn spawn_due(&mut self, now: f32, events: &mut Vec<Event>) {
        let due = self.pending.iter().filter(|&&t| t <= now).count();
        self.pending.retain(|&t| t > now);
        for _ in 0..due {
            self.spawn(events);
        }
    }

    fn fire_hitscan(&mut self, dt: f32, aim: Vec3, input: Input, events: &mut Vec<Event>) {
        let w = &self.scenario.weapon;
        let interval = if w.fire_rate > 0.0 { 1.0 / w.fire_rate } else { 0.0 };
        self.cooldown = (self.cooldown - dt).max(0.0);
        let wants = match w.trigger {
            Trigger::Click => input.fire_pressed,
            Trigger::Hold => input.fire_held,
            Trigger::Always => true,
        };
        if !wants || self.cooldown > 0.0 || (w.trigger != Trigger::Click && interval == 0.0) {
            return;
        }
        self.cooldown = interval;
        let damage = w.damage;
        self.stats.shots += 1;
        self.stats.damage_possible += damage;
        let hit = self.pick(aim);
        if let Some(i) = hit.or_else(|| self.nearest_to(aim)) {
            let t = &self.targets[i];
            let (dx, dy) = screen_offset(t.pos.normalize(), aim);
            let (fx, fy) = t.track.flick;
            let r = t.track.radius_deg.max(1e-3);
            self.telemetry.shots.push(ShotRecord {
                t: self.elapsed(),
                hit: hit.is_some(),
                target_no: Some(t.track.no),
                dx_deg: dx,
                dy_deg: dy,
                along: (dx * fx + dy * fy) / r,
                across: (-dx * fy + dy * fx) / r,
            });
            self.targets[i].shots += 1;
            if hit.is_none() {
                self.targets[i].misses += 1;
            }
        }
        match hit {
            Some(i) => self.damage(i, damage, events),
            None => {
                self.stats.misses += 1;
                self.pace.misses += 1;
                events.push(Event::Miss);
            }
        }
    }

    fn fire_beam(&mut self, dt: f32, aim: Vec3, input: Input, events: &mut Vec<Event>) {
        let firing = match self.scenario.weapon.trigger {
            Trigger::Click | Trigger::Hold => input.fire_held,
            Trigger::Always => true,
        };
        let tick = 1.0 / BEAM_TICK_HZ;
        let per_tick = self.scenario.weapon.damage_per_s * tick;
        let regen = self.current.targets.regen_per_s * dt;
        let delay = self.current.targets.regen_delay_s;
        let now = self.elapsed();
        let on = self.pick(aim);
        if regen > 0.0 {
            for (i, t) in self.targets.iter_mut().enumerate() {
                if (!firing || Some(i) != on) && now - t.last_hit_at >= delay {
                    t.health = (t.health + regen).min(t.max_health);
                }
            }
        }
        self.beam_acc += dt;
        while self.beam_acc >= tick && self.phase == Phase::Running {
            self.beam_acc -= tick;
            let on = self.pick(aim);
            // Adaptive tracking judges aim, firing or not, so holding fire only when
            // on target can't game the level.
            let counts = !self.scenario.adaptive.engaged_only || self.targets.iter().any(Target::engaged);
            if counts {
                self.adaptive_tick(on.is_some(), tick);
            }
            if !firing {
                continue;
            }
            self.stats.shots += 1;
            self.stats.damage_possible += per_tick;
            match on {
                Some(i) => {
                    self.targets[i].shots += 1;
                    self.damage(i, per_tick, events);
                }
                None => self.stats.misses += 1,
            }
        }
    }

    fn adaptive_tick(&mut self, on: bool, tick: f32) {
        if self.per_target() || self.knob() == Some(Knob::KillPace) {
            return;
        }
        let Some(a) = &self.adaptive else { return };
        let (segment, current) = (a.rating.cfg.segment_s, a.segment);
        self.window.0 += 1;
        self.window.1 += u32::from(on);
        self.window.2 += tick;
        if self.window.2 >= segment {
            let share = self.window.1 as f32 / self.window.0.max(1) as f32;
            self.window = (0, 0, 0.0);
            if let Some((value, _, _)) = current {
                self.outcome(value, share, true);
            }
            self.start_segment();
        }
    }

    fn damage(&mut self, i: usize, amount: f32, events: &mut Vec<Event>) {
        let now = self.elapsed();
        let t = &mut self.targets[i];
        let dealt = amount.min(t.health);
        t.health -= dealt;
        t.last_hit_at = now;
        t.hits += 1;
        t.damage += dealt;
        self.stats.hits += 1;
        self.stats.damage_done += dealt;
        events.push(Event::Hit { id: t.id });
        if t.health > 1e-4 {
            return;
        }
        let t = self.targets.swap_remove(i);
        self.stats.kills += 1;
        self.kills.push(KillRecord {
            at_s: now,
            // Not from spawn: with several targets up, that counts time spent
            // killing the others.
            ttk_s: now - t.spawned_at.max(self.last_kill_at),
            shots: t.shots,
            hits: t.hits,
            damage: t.damage,
        });
        self.last_kill_at = now;
        self.telemetry.targets.push(t.record(now, Outcome::Killed));
        self.last_killed_dir = Some(t.pos.normalize());
        events.push(Event::Killed { id: t.id });
        self.pending.push(now + self.current.targets.respawn_delay_s);
        self.next_target(now);
        self.pace_kill(now);
        if self.per_target()
            && let Some((value, _, _)) = t.track.value
        {
            // A kill counts as a success unless it took a spray of misses.
            let ok = if t.misses <= MAX_MISSES_PER_TARGET { 1.0 } else { 0.0 };
            self.outcome(value, ok, false);
        }
    }

    /// After a kill or timeout the player starts on the next target, so flicks to the
    /// targets they haven't reached yet are timed from here.
    fn next_target(&mut self, now: f32) {
        let aim = self.last_aim;
        for t in &mut self.targets {
            if t.track.reach_s.is_none() {
                t.track.restart(now, aim, t.pos, t.radius);
            }
        }
    }

    /// Index of the nearest target the ray along `aim` passes through.
    fn pick(&self, aim: Vec3) -> Option<usize> {
        let shape = self.scenario.targets.shape;
        self.targets
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.hit_by(aim, shape).map(|d| (i, d)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn nearest_to(&self, aim: Vec3) -> Option<usize> {
        self.targets
            .iter()
            .enumerate()
            .max_by(|a, b| aim.dot(a.1.pos.normalize()).total_cmp(&aim.dot(b.1.pos.normalize())))
            .map(|(i, _)| i)
    }

    fn move_targets(&mut self, dt: f32, events: &mut Vec<Event>) {
        let now = self.elapsed();
        let tc = self.current.targets.clone();
        let mv = tc.movement.clone();
        let shrink = tc.shrink_to;
        let shape = self.scenario.targets.shape;
        let a = &self.scenario.arena;
        let (half_w, half_d) = (a.size[0] / 2.0, a.size[2] / 2.0);
        let (floor, ceiling) = (-a.eye_height, a.size[1] - a.eye_height);
        let mut timed_out: Vec<f32> = Vec::new();
        let mut i = 0;
        while i < self.targets.len() {
            let t = &mut self.targets[i];
            t.age += dt;
            if t.lifetime > 0.0 {
                if t.age >= t.lifetime {
                    let t = self.targets.swap_remove(i);
                    self.stats.timeouts += 1;
                    self.telemetry.targets.push(t.record(now, Outcome::TimedOut));
                    events.push(Event::TimedOut { id: t.id });
                    self.pending.push(now + tc.respawn_delay_s);
                    if let Some((value, _, _)) = t.track.value {
                        timed_out.push(value);
                    }
                    if t.track.value.is_none() {
                        timed_out.push(f32::NAN);
                    }
                    continue;
                }
                let k = t.age / t.lifetime;
                t.radius = t.base_radius * (1.0 + (shrink - 1.0) * k);
            }
            if mv.kind != MovementKind::Static || mv.approaches() || mv.jumps() {
                if mv.kind != MovementKind::Static {
                    t.next_change -= dt;
                    if t.next_change <= 0.0 {
                        t.next_change = self.rng.range(mv.change_every_s);
                        if mv.kind == MovementKind::Curve {
                            // Keep going, but bend the other way (or the same way at a new rate).
                            t.turn = new_turn(&mut self.rng, &mv);
                        } else {
                            let own = Movement { speed: t.speed.unwrap_or(mv.speed), ..mv.clone() };
                            t.goal = new_rate(&mut self.rng, &own, t.distance, t.pitch, t.goal);
                            if mv.accel <= 0.0 {
                                t.rate = t.goal;
                            }
                        }
                        t.last_change = now;
                    }
                    if mv.kind == MovementKind::Curve {
                        t.rate = rotate(t.rate, t.turn * dt, t.pitch);
                        t.goal = t.rate;
                    } else if mv.accel > 0.0 {
                        t.rate = steer(t.rate, t.goal, mv.accel, t.distance, t.pitch, dt);
                    }
                }
                if mv.approaches() {
                    t.next_radial -= dt;
                    if t.next_radial <= 0.0 {
                        let every = if mv.approach_every_s.1 > 0.0 { mv.approach_every_s } else { mv.change_every_s };
                        t.next_radial = self.rng.range(every);
                        let side = if t.radial_goal > 0.0 { -1.0 } else { 1.0 };
                        t.radial_goal = side * self.rng.range(mv.approach_speed);
                    }
                    t.radial = if mv.accel > 0.0 {
                        let step = mv.accel * dt;
                        t.radial + (t.radial_goal - t.radial).clamp(-step, step)
                    } else {
                        t.radial_goal
                    };
                    let dr = self.scenario.targets.spawn.distance;
                    let (d, v) = bounce(t.distance + t.radial * dt, t.radial, dr);
                    if v.signum() != t.radial.signum() {
                        t.radial_goal = t.radial_goal.abs() * v.signum();
                    }
                    // Keep sideways speed in meters per second: closer is faster across the view.
                    let k = t.distance / d.max(0.1);
                    t.rate = [t.rate[0] * k, t.rate[1] * k];
                    t.goal = [t.goal[0] * k, t.goal[1] * k];
                    (t.distance, t.radial) = (d, v);
                }
                if mv.jumps() {
                    let (h, air) = (mv.jump_height, mv.jump_air_s);
                    let g = 8.0 * h / (air * air);
                    if t.lift <= 0.0 && t.lift_v <= 0.0 {
                        t.next_jump -= dt;
                        if t.next_jump <= 0.0 {
                            t.lift_v = 4.0 * h / air;
                        }
                    }
                    if t.lift > 0.0 || t.lift_v > 0.0 {
                        t.lift += t.lift_v * dt;
                        t.lift_v -= g * dt;
                        if t.lift <= 0.0 {
                            t.lift = 0.0;
                            t.lift_v = 0.0;
                            t.next_jump = self.rng.range(mv.jump_every_s);
                        }
                    }
                }
                // Bounce off the edges of the spawn area, and off the floor, ceiling
                // and walls: the spawn area can reach further than the room allows.
                let half_h = match shape {
                    Shape::Sphere => t.radius,
                    Shape::Capsule => (t.height * t.radius / t.base_radius.max(1e-6) / 2.0).max(t.radius),
                };
                let (yr, mut pr) = (self.scenario.targets.spawn.yaw, self.scenario.targets.spawn.pitch);
                let d = t.distance.max(1e-3);
                let pitch_at = |y: f32| (y / d).clamp(-1.0, 1.0).asin().to_degrees();
                pr = Range(pr.0.max(pitch_at(floor + half_h)), pr.1.min(pitch_at(ceiling - half_h)));
                if pr.0 > pr.1 {
                    let mid = (pr.0 + pr.1) / 2.0;
                    pr = Range(mid, mid);
                }
                let before = t.rate;
                let (old_yaw, old_rate) = (t.yaw, t.rate[0]);
                (t.yaw, t.rate[0]) = bounce(t.yaw + t.rate[0] * dt, t.rate[0], yr);
                (t.pitch, t.rate[1]) = bounce(t.pitch + t.rate[1] * dt, t.rate[1], pr);
                let p = from_angles(t.yaw, t.pitch) * t.distance;
                if p.x.abs() + t.radius > half_w || p.z.abs() + t.radius > half_d {
                    // A side or back wall: stay put sideways and turn around.
                    t.yaw = old_yaw;
                    t.rate[0] = -old_rate;
                }
                // A bounce turns the target around, so it should head that way too.
                for ((goal, rate), was) in t.goal.iter_mut().zip(t.rate).zip(before) {
                    if rate.signum() != was.signum() && rate != 0.0 {
                        *goal = goal.abs() * rate.signum();
                    }
                }
                if before != t.rate {
                    t.last_change = now;
                }
                // Jumps go straight up from the target's base, under the ceiling.
                let base = from_angles(t.yaw, t.pitch) * t.distance;
                t.lift = t.lift.min((ceiling - half_h - base.y).max(0.0));
                t.pos = base + Vec3::Y * t.lift;
            }
            i += 1;
        }
        if !timed_out.is_empty() {
            self.next_target(now);
            if self.per_target() {
                for value in timed_out.into_iter().filter(|v| v.is_finite()) {
                    self.outcome(value, 0.0, false);
                }
            }
        }
    }

    fn spawn(&mut self, events: &mut Vec<Event>) {
        let tc = &self.current.targets;
        let sp = &tc.spawn;
        let min_sep = sp.min_separation.to_radians().cos();
        let max_last = (sp.max_from_last > 0.0).then(|| sp.max_from_last.to_radians().cos());
        let mut dir = Vec3::NEG_Z;
        for attempt in 0..40 {
            dir = from_angles(self.rng.range(sp.yaw), self.rng.range(sp.pitch));
            let apart = self.targets.iter().all(|t| dir.dot(t.pos.normalize()) <= min_sep);
            let near_last = match (max_last, self.last_killed_dir) {
                (Some(cos), Some(last)) => dir.dot(last) >= cos,
                _ => true,
            };
            // Give up on the reach limit before the spacing rule if nothing fits.
            if apart && (near_last || attempt >= 30) {
                break;
            }
        }
        let pos = match &self.current.world {
            // Imported scenarios: the map's spawn points, kept apart by distance.
            Some(w) => {
                let others: Vec<Vec3> = self.targets.iter().map(|t| t.pos).collect();
                w.spawn_pos(&mut self.rng, &others)
            }
            None => {
                let distance = self.rng.range(sp.distance);
                self.fit_in_room(dir * distance)
            }
        };
        let id = self.next_id;
        self.next_id += 1;
        let no = self.next_no;
        self.next_no += 1;
        let tc = self.current.targets.clone();
        let (yaw, pitch) = angles(pos);
        let distance = pos.length();
        let now = self.elapsed();
        let mut lifetime = tc.lifetime_s;
        let mut radius = tc.radius;
        let mut speed = None;
        let mut value = None;
        if self.per_target()
            && let Some(a) = &mut self.adaptive
        {
            let (v, chance, pick) = a.next(&mut self.rng);
            match a.rating.cfg.knob {
                Knob::Lifetime => lifetime = v,
                Knob::TargetSpeed => speed = Some(speed_for(&self.scenario, v)),
                _ => radius = v,
            }
            value = Some((v, chance, pick));
        } else if let Some(seg) = self.adaptive.as_ref().and_then(|a| a.segment) {
            value = Some(seg);
        }
        let own = Movement { speed: speed.unwrap_or(tc.movement.speed), ..tc.movement.clone() };
        let rate = new_rate(&mut self.rng, &own, distance, pitch, [0.0, 0.0]);
        let next_change = self.rng.range(tc.movement.change_every_s);
        let mv = &tc.movement;
        let (radial_goal, next_radial) = if mv.approaches() {
            let side = if self.rng.next_f32() < 0.5 { -1.0 } else { 1.0 };
            let every = if mv.approach_every_s.1 > 0.0 { mv.approach_every_s } else { mv.change_every_s };
            (side * self.rng.range(mv.approach_speed), self.rng.range(every))
        } else {
            (0.0, 0.0)
        };
        let next_jump = if mv.jumps() { self.rng.range(mv.jump_every_s) } else { 0.0 };
        let turn = new_turn(&mut self.rng, mv);
        let height = tc.height * radius / self.scenario.targets.radius.max(1e-6);
        self.targets.push(Target {
            id,
            pos,
            radius,
            health: tc.health,
            max_health: tc.health,
            yaw,
            pitch,
            distance,
            rate,
            goal: rate,
            radial: radial_goal,
            radial_goal,
            next_radial,
            lift: 0.0,
            lift_v: 0.0,
            next_jump,
            last_hit_at: f32::NEG_INFINITY,
            turn,
            next_change,
            last_change: now,
            age: 0.0,
            spawned_at: now,
            lifetime,
            speed,
            base_radius: radius,
            height,
            shots: 0,
            hits: 0,
            misses: 0,
            damage: 0.0,
            track: Track::begin(no, now, self.last_aim, pos, radius, value),
        });
        events.push(Event::Spawned { id });
    }

    fn fit_in_room(&self, p: Vec3) -> Vec3 {
        let a = &self.scenario.arena;
        let r = self.current.targets.radius.max(self.current.targets.height / 2.0);
        let half = |v: f32| (v / 2.0 - r).max(0.0);
        Vec3::new(
            p.x.clamp(-half(a.size[0]), half(a.size[0])),
            p.y.clamp(-a.eye_height + r, a.size[1] - a.eye_height - r),
            p.z.clamp(-half(a.size[2]), half(a.size[2])),
        )
    }
}

/// Angular velocity in degrees per second for a target moving at a speed picked
/// from `mv.speed` (meters per second at its distance).
fn new_rate(rng: &mut Rng, mv: &Movement, distance: f32, pitch: f32, old: [f32; 2]) -> [f32; 2] {
    let speed = rng.range(mv.speed);
    let per_m = (1.0 / distance.max(0.1)).to_degrees();
    // A yaw step covers less ground near the top and bottom, so scale it up there.
    let yaw_per_m = per_m / pitch.to_radians().cos().max(0.2);
    match mv.kind {
        MovementKind::Static => [0.0, 0.0],
        MovementKind::Strafe => {
            // Reverse the current direction, or pick one at random to start.
            let side = if old[0] != 0.0 {
                -old[0].signum()
            } else if rng.next_f32() < 0.5 {
                -1.0
            } else {
                1.0
            };
            [side * speed * yaw_per_m, 0.0]
        }
        MovementKind::Random | MovementKind::Curve => {
            let a = rng.next_f32() * std::f32::consts::TAU;
            // A curve turns through every heading anyway, so it starts in any direction.
            let vertical = if mv.kind == MovementKind::Curve { 1.0 } else { mv.vertical };
            let (x, y) = (a.cos(), a.sin() * vertical);
            let len = (x * x + y * y).sqrt().max(1e-6);
            [x / len * speed * yaw_per_m, y / len * speed * per_m]
        }
    }
}

/// Curve movement: a turn rate from the scenario's range, to a random side.
fn new_turn(rng: &mut Rng, mv: &Movement) -> f32 {
    if mv.kind != MovementKind::Curve {
        return 0.0;
    }
    let side = if rng.next_f32() < 0.5 { -1.0 } else { 1.0 };
    side * rng.range(mv.turn_deg_s)
}

/// Rotates a velocity's heading on screen by `deg`, keeping its speed.
fn rotate(rate: [f32; 2], deg: f32, pitch: f32) -> [f32; 2] {
    let c = pitch.to_radians().cos().max(0.2);
    let (x, y) = (rate[0] * c, rate[1]);
    let (sin, cos) = deg.to_radians().sin_cos();
    [(x * cos - y * sin) / c, x * sin + y * cos]
}

/// Turns `rate` toward `goal` at `accel` m/s², measured on screen at `distance`.
fn steer(rate: [f32; 2], goal: [f32; 2], accel: f32, distance: f32, pitch: f32, dt: f32) -> [f32; 2] {
    let c = pitch.to_radians().cos().max(0.2);
    let (r, g) = ([rate[0] * c, rate[1]], [goal[0] * c, goal[1]]);
    let (dx, dy) = (g[0] - r[0], g[1] - r[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let step = (accel / distance.max(0.1)).to_degrees() * dt;
    let next = if len <= step { g } else { [r[0] + dx / len * step, r[1] + dy / len * step] };
    [next[0] / c, next[1]]
}

fn bounce(v: f32, rate: f32, r: Range) -> (f32, f32) {
    if v < r.0 {
        (r.0, rate.abs())
    } else if v > r.1 {
        (r.1, -rate.abs())
    } else {
        (v, rate)
    }
}

/// Unit direction for yaw (degrees right of ahead) and pitch (degrees above level).
pub fn from_angles(yaw_deg: f32, pitch_deg: f32) -> Vec3 {
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    Vec3::new(y.sin() * p.cos(), p.sin(), -y.cos() * p.cos())
}

/// Yaw and pitch in degrees of a point seen from the origin.
pub fn angles(p: Vec3) -> (f32, f32) {
    let d = p.normalize_or(Vec3::NEG_Z);
    (d.x.atan2(-d.z).to_degrees(), d.y.clamp(-1.0, 1.0).asin().to_degrees())
}

/// Distance along the ray from the origin to where it meets the target, if it does.
pub fn ray_hit(dir: Vec3, center: Vec3, radius: f32, shape: Shape, height: f32) -> Option<f32> {
    match shape {
        Shape::Sphere => {
            let along = center.dot(dir);
            if along < 0.0 {
                return None;
            }
            let miss2 = center.length_squared() - along * along;
            (miss2 <= radius * radius).then(|| along - (radius * radius - miss2).sqrt())
        }
        Shape::Capsule => {
            // Closest approach between the ray and the capsule's core segment.
            let half = (height / 2.0 - radius).max(0.0);
            let a = center - Vec3::Y * half;
            let seg = Vec3::Y * (2.0 * half);
            let (t_ray, t_seg) = closest_ray_segment(dir, a, seg);
            let gap = (dir * t_ray - (a + seg * t_seg)).length();
            (t_ray >= 0.0 && gap <= radius).then_some(t_ray)
        }
    }
}

/// Parameters of the closest points between the ray `t * dir` (t ≥ 0) and the
/// segment `a + s * seg` (0 ≤ s ≤ 1).
fn closest_ray_segment(dir: Vec3, a: Vec3, seg: Vec3) -> (f32, f32) {
    let len2 = seg.length_squared();
    if len2 < 1e-9 {
        return (a.dot(dir).max(0.0), 0.0);
    }
    let b = dir.dot(seg);
    let d = dir.dot(a);
    let e = seg.dot(a);
    let denom = len2 - b * b;
    let mut s = if denom.abs() > 1e-9 { ((b * d - e) / denom).clamp(0.0, 1.0) } else { 0.0 };
    let mut t = d + b * s;
    if t < 0.0 {
        t = 0.0;
        s = (-e / len2).clamp(0.0, 1.0);
    }
    (t, s)
}

/// SplitMix64: tiny, fast and identical on every platform, so a seed replays the
/// same run.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, r: Range) -> f32 {
        r.0 + (r.1 - r.0) * self.next_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Scenario;
    use std::collections::HashMap;

    const FRAME: f32 = 1.0 / 240.0;

    fn idle() -> Input {
        Input { aim: Vec3::NEG_Z, fire_pressed: false, fire_held: false }
    }

    fn shoot(at: Vec3) -> Input {
        Input { aim: at.normalize(), fire_pressed: true, fire_held: true }
    }

    fn no_countdown(mut s: Scenario) -> Scenario {
        s.timing.countdown_s = 0.0;
        s
    }

    #[test]
    fn angles_round_trip() {
        for (y, p) in [(0.0, 0.0), (30.0, 10.0), (-120.0, -40.0)] {
            let (y2, p2) = angles(from_angles(y, p));
            assert!((y - y2).abs() < 1e-3 && (p - p2).abs() < 1e-3, "{y},{p} -> {y2},{p2}");
        }
        assert!(from_angles(0.0, 0.0).abs_diff_eq(Vec3::NEG_Z, 1e-6));
        assert!(from_angles(90.0, 0.0).abs_diff_eq(Vec3::X, 1e-6));
    }

    #[test]
    fn sphere_and_capsule_hits() {
        let c = Vec3::new(0.0, 0.0, -10.0);
        assert!((ray_hit(Vec3::NEG_Z, c, 0.5, Shape::Sphere, 0.0).unwrap() - 9.5).abs() < 1e-4);
        assert!(ray_hit(from_angles(3.0, 0.0), c, 0.5, Shape::Sphere, 0.0).is_none());
        assert!(ray_hit(Vec3::Z, c, 0.5, Shape::Sphere, 0.0).is_none(), "behind the player");
        // A 1.8 m capsule is hit near its top but not above it.
        assert!(ray_hit(from_angles(0.0, 4.5), c, 0.3, Shape::Capsule, 1.8).is_some());
        assert!(ray_hit(from_angles(0.0, 6.0), c, 0.3, Shape::Capsule, 1.8).is_none());
    }

    #[test]
    fn countdown_blocks_shots() {
        let mut sim = Sim::new(Scenario::default(), 1);
        let at = sim.targets()[0].pos;
        sim.step(FRAME, shoot(at));
        assert_eq!(sim.stats().shots, 0);
        assert_eq!(sim.phase(), Phase::Countdown);
    }

    #[test]
    fn clicking_a_target_kills_and_replaces_it() {
        let mut sim = Sim::new(no_countdown(Scenario::default()), 7);
        let first = sim.targets()[0].clone();
        let events = sim.step(FRAME, shoot(first.pos));
        assert!(events.contains(&Event::Killed { id: first.id }), "{events:?}");
        assert_eq!(sim.targets().len(), 1, "replacement spawned");
        assert_ne!(sim.targets()[0].id, first.id);
        assert_eq!(sim.stats().kills, 1);
        assert_eq!(sim.score(), 100.0);
    }

    #[test]
    fn misses_count_and_holding_click_fires_once() {
        let mut s = no_countdown(Scenario::default());
        s.scoring.per_miss = -10.0;
        let mut sim = Sim::new(s, 3);
        let away = -sim.targets()[0].pos;
        sim.step(FRAME, shoot(away));
        for _ in 0..10 {
            sim.step(FRAME, Input { aim: away.normalize(), fire_pressed: false, fire_held: true });
        }
        assert_eq!(sim.stats().shots, 1);
        assert_eq!(sim.stats().misses, 1);
        assert_eq!(sim.score(), -10.0);
        assert_eq!(sim.stats().accuracy(), Some(0.0));
    }

    #[test]
    fn hold_trigger_respects_fire_rate() {
        let mut s = no_countdown(Scenario::default());
        s.weapon.trigger = Trigger::Hold;
        s.weapon.fire_rate = 10.0;
        let mut sim = Sim::new(s, 3);
        let away = -sim.targets()[0].pos;
        for _ in 0..240 {
            sim.step(FRAME, Input { aim: away.normalize(), fire_pressed: false, fire_held: true });
        }
        assert!((9..=11).contains(&sim.stats().shots), "{} shots in 1 s at 10/s", sim.stats().shots);
    }

    #[test]
    fn run_ends_on_time_and_kill_limit() {
        let mut s = no_countdown(Scenario::default());
        s.timing.duration_s = 1.0;
        let mut sim = Sim::new(s.clone(), 1);
        for _ in 0..300 {
            sim.step(FRAME, idle());
        }
        assert_eq!(sim.phase(), Phase::Finished);
        assert!((sim.result().duration_s - 1.0).abs() < 1e-3);

        s.timing.duration_s = 60.0;
        s.timing.kill_limit = 3;
        let mut sim = Sim::new(s, 1);
        while sim.phase() != Phase::Finished {
            let at = sim.targets()[0].pos;
            sim.step(FRAME, shoot(at));
            sim.step(FRAME, idle());
        }
        assert_eq!(sim.stats().kills, 3);
        assert_eq!(sim.result().kills.len(), 3);
    }

    #[test]
    fn beam_on_target_is_full_accuracy() {
        let mut s = no_countdown(Scenario::default());
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Always;
        s.targets.health = 1e9;
        s.scoring = crate::scenario::Scoring { per_kill: 0.0, per_damage: 1.0, ..Default::default() };
        let mut sim = Sim::new(s, 1);
        let at = sim.targets()[0].pos;
        for _ in 0..240 {
            sim.step(FRAME, Input { aim: at.normalize(), fire_pressed: false, fire_held: false });
        }
        let st = sim.stats();
        assert!((59..=61).contains(&st.shots), "{} ticks in 1 s", st.shots);
        assert_eq!(st.misses, 0);
        assert!((sim.score() - 100.0).abs() < 2.0, "1 s at 100 dps, got {}", sim.score());
    }

    /// Regression: on Float Track the ball sank through the floor. Its spawn area
    /// reaches 10° below eye level at 10 m, which puts a 0.5 m ball's bottom under
    /// the floor, and movement only bounced off the spawn area, not the room.
    #[test]
    fn no_target_ever_leaves_the_room() {
        for e in crate::library::builtins() {
            let mut s = e.scenario.unwrap();
            s.timing.countdown_s = 0.0;
            let a = s.arena.clone();
            let (half_w, half_d) = (a.size[0] / 2.0, a.size[2] / 2.0);
            let (floor, ceiling) = (-a.eye_height, a.size[1] - a.eye_height);
            for adaptive in [false, true] {
                let mut sim = if adaptive {
                    Sim::adaptive(s.clone(), 5, None, crate::adaptive::Mix::Balanced)
                } else {
                    Sim::new(s.clone(), 5)
                };
                while sim.phase() != Phase::Finished {
                    sim.step(FRAME, idle());
                    for t in sim.targets() {
                        let half_h = match s.targets.shape {
                            Shape::Sphere => t.radius,
                            Shape::Capsule => (t.height * t.radius / t.base_radius.max(1e-6) / 2.0).max(t.radius),
                        };
                        let p = t.pos;
                        let ok = p.y - half_h >= floor - 1e-3
                            && p.y + half_h <= ceiling + 1e-3
                            && p.x.abs() + t.radius <= half_w + 1e-3
                            && p.z.abs() + t.radius <= half_d + 1e-3;
                        assert!(
                            ok,
                            "{}: target at {p:?} (r {}) left the room at {:.2} s",
                            e.id,
                            t.radius,
                            sim.elapsed()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn moving_targets_stay_in_their_area() {
        let mut s = no_countdown(Scenario::default());
        s.timing.duration_s = 600.0;
        s.targets.count = 3;
        s.targets.movement = Movement { kind: MovementKind::Random, speed: Range(4.0, 8.0), ..Default::default() };
        let mut sim = Sim::new(s.clone(), 9);
        let start: Vec<f32> = sim.targets().iter().map(|t| t.pos.length()).collect();
        for _ in 0..(240 * 30) {
            sim.step(FRAME, idle());
        }
        for (t, d) in sim.targets().iter().zip(start) {
            let (yaw, pitch) = angles(t.pos);
            assert!(s.targets.spawn.yaw.contains(yaw) && s.targets.spawn.pitch.contains(pitch), "{yaw},{pitch}");
            assert!((t.pos.length() - d).abs() < 1e-3, "keeps its distance");
        }
    }

    #[test]
    fn lifetime_times_out_and_shrinks() {
        let mut s = no_countdown(Scenario::default());
        s.targets.lifetime_s = 1.0;
        s.targets.shrink_to = 0.5;
        s.scoring.per_timeout = -50.0;
        let mut sim = Sim::new(s, 2);
        for _ in 0..120 {
            sim.step(FRAME, idle());
        }
        assert!((sim.targets()[0].radius - 0.225).abs() < 0.01, "half-way: {}", sim.targets()[0].radius);
        for _ in 0..121 {
            sim.step(FRAME, idle());
        }
        assert_eq!(sim.stats().timeouts, 1);
        assert_eq!(sim.score(), -50.0);
    }

    #[test]
    fn spawns_keep_their_spacing() {
        let mut s = no_countdown(Scenario::default());
        s.targets.count = 5;
        s.targets.spawn.min_separation = 8.0;
        let sim = Sim::new(s, 11);
        let t = sim.targets();
        for a in 0..t.len() {
            for b in a + 1..t.len() {
                let angle = t[a].pos.normalize().angle_between(t[b].pos.normalize()).to_degrees();
                assert!(angle >= 8.0 - 1e-3, "{angle}");
            }
        }
    }

    #[test]
    fn same_seed_same_run() {
        let run = |seed| {
            let mut sim = Sim::new(no_countdown(Scenario::default()), seed);
            let mut aim = Vec3::NEG_Z;
            for i in 0..2000 {
                if i % 50 == 0 {
                    aim = sim.targets()[0].pos.normalize();
                }
                sim.step(FRAME, Input { aim, fire_pressed: i % 50 == 0, fire_held: false });
            }
            sim.result()
        };
        assert_eq!(run(5), run(5));
    }

    #[test]
    fn adaptive_rating_speeds_up_for_hits_and_slows_for_timeouts() {
        let mut s = no_countdown(Scenario::default());
        s.timing.duration_s = 30.0;
        // A perfect player: kills each target the frame it appears.
        let mut sim = Sim::adaptive(s.clone(), 1, None, Mix::Balanced);
        let start = sim.rating().unwrap().1;
        while sim.phase() != Phase::Finished {
            let at = sim.targets()[0].pos;
            sim.step(FRAME, shoot(at));
            sim.step(FRAME, idle());
        }
        let r = sim.result();
        let a = r.adaptive.clone().unwrap();
        assert_eq!(a.knob, Knob::Lifetime);
        assert!(a.value < start * 0.5, "perfect play gets fast: {start} -> {}", a.value);
        assert_eq!(r.score, a.value, "the result is the rating");
        assert!(a.calibration() && a.pick == Pick::Calibrate, "no earlier rating: a calibration run");
        assert_eq!(a.fastest, None, "a calibration run sets no best");
        // Targets got their lifetimes from the rating, and record them.
        let t = &r.telemetry.targets[3];
        assert_eq!(t.value, Some(t.lifetime_s));
        assert!(t.chance.is_some() && t.pick.is_some());

        // A player who never shoots only sees timeouts and the rating slows down.
        let mut sim = Sim::adaptive(s, 1, None, Mix::Balanced);
        while sim.phase() != Phase::Finished {
            sim.step(FRAME, idle());
        }
        let a = sim.result().adaptive.unwrap();
        assert!(a.value > start && a.success_rate == Some(0.0), "{a:?}");
    }

    fn hitscan_strafer() -> Scenario {
        let mut s = no_countdown(Scenario::default());
        s.targets.lifetime_s = 1.5;
        s.targets.spawn.yaw = Range(-30.0, 30.0);
        s.targets.movement = Movement { kind: MovementKind::Strafe, ..Default::default() };
        s.adaptive.knob = Knob::TargetSpeed;
        s.timing.duration_s = 20.0;
        s
    }

    /// Found in the scoring review: an adaptive hitscan scenario set to target
    /// speed recorded no outcomes (only beam ticks fed tracking segments), so its
    /// rating never moved. Each target now gets its own speed, and a kill before
    /// it times out is a success.
    #[test]
    fn hitscan_target_speed_is_rated_per_target() {
        let s = hitscan_strafer();
        assert!(s.problems().is_empty(), "{:?}", s.problems());
        let mut sim = Sim::adaptive(s.clone(), 1, None, Mix::Balanced);
        let start = sim.rating().unwrap().1;
        while sim.phase() != Phase::Finished {
            let at = sim.targets()[0].pos;
            sim.step(FRAME, shoot(at));
            sim.step(FRAME, idle());
        }
        let r = sim.result();
        let a = r.adaptive.unwrap();
        assert!(a.outcomes > 20, "one outcome per target: {}", a.outcomes);
        assert!(a.value > start, "perfect play rates faster: {start} -> {}", a.value);
        // Targets move at the speed they were given, and keep it.
        let t = r.telemetry.targets.iter().rev().find(|t| t.value.is_some()).unwrap();
        assert!(t.value.unwrap() > start);

        // Never shooting: every target times out and the rating slows.
        let mut sim = Sim::adaptive(s, 1, None, Mix::Balanced);
        while sim.phase() != Phase::Finished {
            sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
        }
        let a = sim.result().adaptive.unwrap();
        assert!(a.outcomes > 5 && a.value < start, "{a:?}");
    }

    #[test]
    fn hitscan_target_speed_keeps_each_targets_speed() {
        let s = hitscan_strafer();
        let mut sim = Sim::adaptive(s, 2, None, Mix::Balanced);
        // Each target's position every quarter second, by spawn number.
        let mut path: HashMap<u32, Vec<Vec3>> = HashMap::new();
        let mut frame = 0;
        while sim.phase() != Phase::Finished {
            sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
            frame += 1;
            if frame % 60 == 0 {
                for t in sim.targets() {
                    path.entry(t.no()).or_default().push(t.pos.normalize());
                }
            }
        }
        let r = sim.result();
        let mut checked = 0;
        for rec in &r.telemetry.targets {
            let (Some(value), Some(p)) = (rec.value, path.get(&rec.no)) else { continue };
            if p.len() < 4 {
                continue;
            }
            // Fastest quarter second: a window without a reversal moves at full speed.
            let peak = p.windows(2).map(|w| (w[1] - w[0]).length().to_degrees() * 4.0).fold(0.0f32, f32::max);
            assert!(
                peak > value * 0.75 && peak < value * 1.1,
                "target {} given {value:.1}, moved {peak:.1} deg/s",
                rec.no
            );
            checked += 1;
        }
        assert!(checked >= 5, "{checked} targets checked");
    }

    fn mover(mv: Movement) -> Scenario {
        let mut s = no_countdown(Scenario::default());
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Always;
        s.targets.health = 1e9;
        s.targets.spawn.yaw = Range(-40.0, 40.0);
        s.targets.movement = mv;
        s.timing.duration_s = 30.0;
        s
    }

    fn look_away() -> Input {
        Input { aim: Vec3::Y, fire_pressed: false, fire_held: false }
    }

    /// Screen velocity in degrees per second.
    fn screen(t: &Target) -> (f32, f32) {
        (t.rate[0] * t.pitch.to_radians().cos(), t.rate[1])
    }

    #[test]
    fn acceleration_slows_down_and_speeds_back_up() {
        let accel = 20.0;
        let mv = Movement { kind: MovementKind::Strafe, speed: Range(5.0, 5.0), accel, ..Default::default() };
        let mut sim = Sim::new(mover(mv), 1);
        let d = sim.targets()[0].distance;
        let max_step = (accel / d).to_degrees() * FRAME * 1.05;
        let mut prev = screen(&sim.targets()[0]);
        let (mut slow, mut reversals) = (false, 0);
        for _ in 0..(240 * 10) {
            sim.step(FRAME, look_away());
            let t = &sim.targets()[0];
            let now = screen(t);
            let dv = ((now.0 - prev.0).powi(2) + (now.1 - prev.1).powi(2)).sqrt();
            // A bounce off the area's edge flips instantly; everything else is gradual.
            if now.0.signum() == prev.0.signum() || now.0.abs() < 1.0 {
                assert!(dv <= max_step, "velocity jumped {dv:.3} deg/s in a frame (limit {max_step:.3})");
            }
            if now.0.abs() < 0.5 {
                slow = true;
            }
            if now.0.signum() != prev.0.signum() && now.0 != 0.0 {
                reversals += 1;
            }
            prev = now;
        }
        assert!(slow && reversals >= 5, "slowed through reversals: {reversals}");
        // Full speed again between reversals.
        let full = (5.0 / d).to_degrees();
        assert!(prev.0.abs() <= full * 1.01);
    }

    #[test]
    fn forward_back_movement_stays_in_range_and_keeps_speed() {
        let mut s = mover(Movement {
            kind: MovementKind::Strafe,
            speed: Range(4.0, 4.0),
            change_every_s: Range(3.0, 3.0),
            approach_speed: Range(3.0, 3.0),
            approach_every_s: Range(0.7, 1.2),
            ..Default::default()
        });
        s.targets.spawn.distance = Range(6.0, 14.0);
        assert!(s.problems().is_empty(), "{:?}", s.problems());
        let mut sim = Sim::new(s.clone(), 2);
        let (mut lo, mut hi) = (f32::MAX, 0.0f32);
        for _ in 0..(240 * 20) {
            sim.step(FRAME, look_away());
            let t = &sim.targets()[0];
            assert!(t.distance >= 6.0 - 1e-3 && t.distance <= 14.0 + 1e-3, "{}", t.distance);
            lo = lo.min(t.distance);
            hi = hi.max(t.distance);
            // Sideways speed in m/s stays put while the distance changes.
            let (x, _) = screen(t);
            let mps = x.abs().to_radians() * t.distance;
            assert!(mps < 4.0 * 1.02, "{mps:.2} m/s at {:.1} m", t.distance);
        }
        assert!(hi - lo > 3.0, "it moved forward and back: {lo:.1}..{hi:.1} m");
        // Without a distance range there's nowhere to move.
        s.targets.spawn.distance = Range(10.0, 10.0);
        assert!(s.problems().iter().any(|p| p.contains("distance range")));
    }

    #[test]
    fn jumps_go_up_and_come_down_under_gravity() {
        let mut s = mover(Movement {
            kind: MovementKind::Strafe,
            jump_every_s: Range(0.5, 0.5),
            jump_height: 1.2,
            jump_air_s: 0.7,
            ..Default::default()
        });
        s.targets.spawn.pitch = Range(-8.0, -8.0);
        let mut sim = Sim::new(s, 3);
        let (mut peak, mut airborne, mut landings, mut air_frames) = (0.0f32, false, 0, 0);
        let mut airtimes = Vec::new();
        for _ in 0..(240 * 8) {
            sim.step(FRAME, look_away());
            let t = &sim.targets()[0];
            peak = peak.max(t.lift);
            if t.lift > 0.0 {
                airborne = true;
                air_frames += 1;
            } else if airborne {
                airborne = false;
                landings += 1;
                airtimes.push(air_frames as f32 * FRAME);
                air_frames = 0;
            }
        }
        assert!((peak - 1.2).abs() < 0.05, "peak {peak:.2} m");
        assert!(landings >= 5, "{landings} jumps");
        for a in airtimes {
            assert!((a - 0.7).abs() < 0.03, "airtime {a:.3} s");
        }
    }

    #[test]
    fn regen_waits_for_its_delay() {
        let mut s = builtin("builtin:stability-switch");
        s.targets.count = 1;
        s.targets.movement.kind = MovementKind::Static;
        s.targets.regen_per_s = 100.0;
        s.targets.regen_delay_s = 0.5;
        let mut sim = Sim::new(s, 1);
        let at = sim.targets()[0].pos.normalize();
        let held = |aim| Input { aim, fire_pressed: false, fire_held: true };
        for _ in 0..120 {
            sim.step(FRAME, held(at));
        }
        let damaged = sim.targets()[0].health;
        for _ in 0..108 {
            sim.step(FRAME, held(Vec3::Y));
        }
        assert_eq!(sim.targets()[0].health, damaged, "no healing in the first 0.45 s");
        for _ in 0..60 {
            sim.step(FRAME, held(Vec3::Y));
        }
        assert!(sim.targets()[0].health > damaged, "healing after the delay");
    }

    /// Regen smoothness: with a beam, target lifetimes and the target-speed knob,
    /// each target is a kill to finish in time. Fast regen after a short delay means
    /// slipping off loses the progress, so unbroken tracking is what succeeds.
    fn unbroken(slip_every: Option<f32>) -> AdaptiveSummary {
        let mut s = no_countdown(Scenario::default());
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Hold;
        s.weapon.damage_per_s = 100.0;
        s.targets.health = 150.0;
        s.targets.regen_per_s = 400.0;
        s.targets.regen_delay_s = 0.1;
        s.targets.lifetime_s = 4.0;
        s.targets.movement = Movement { kind: MovementKind::Strafe, accel: 20.0, ..Default::default() };
        s.adaptive.knob = Knob::TargetSpeed;
        s.timing.duration_s = 30.0;
        assert!(s.problems().is_empty(), "{:?}", s.problems());
        let mut sim = Sim::adaptive(s, 5, None, Mix::Balanced);
        let mut t = 0.0;
        while sim.phase() != Phase::Finished {
            t += FRAME;
            let slipping = slip_every.is_some_and(|e| t % e > e - 0.3);
            let aim = if slipping { Vec3::Y } else { sim.targets()[0].pos.normalize() };
            sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: true });
        }
        sim.result().adaptive.unwrap()
    }

    #[test]
    fn regen_smoothness_rewards_unbroken_tracking() {
        let steady = unbroken(None);
        assert!(steady.outcomes >= 10, "one outcome per target: {}", steady.outcomes);
        assert_eq!(steady.success_rate, Some(1.0));
        // Slipping off for 0.3 s every 1.2 s: the target heals about as fast as the beam hurts it, so no kills.
        let slips = unbroken(Some(1.2));
        assert_eq!(slips.success_rate, Some(0.0), "{slips:?}");
        assert!(slips.value < steady.value);
    }

    #[test]
    fn new_movement_fields_dont_change_old_fingerprints() {
        let a = Scenario::default();
        let mut b = a.clone();
        b.targets.movement.accel = 10.0;
        assert_ne!(a.fingerprint(), b.fingerprint(), "using a new field changes it");
        assert!(a.to_toml().contains("accel"), "files still list every field");
    }

    #[test]
    fn spraying_clicks_does_not_count_as_success() {
        let mut s = no_countdown(Scenario::default());
        s.timing.duration_s = 10.0;
        let mut sim = Sim::adaptive(s, 2, None, Mix::Balanced);
        while sim.phase() != Phase::Finished {
            let at = sim.targets()[0].pos;
            // Two misses, then the kill.
            sim.step(FRAME, shoot(-at));
            sim.step(FRAME, idle());
            sim.step(FRAME, shoot(-at));
            sim.step(FRAME, idle());
            sim.step(FRAME, shoot(at));
            sim.step(FRAME, idle());
        }
        let a = sim.result().adaptive.unwrap();
        assert_eq!(a.success_rate, Some(0.0));
    }

    #[test]
    fn adaptive_tracking_rates_target_speed() {
        let mut s = no_countdown(Scenario::default());
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Always;
        s.targets.health = 1e9;
        s.targets.spawn.yaw = Range(-40.0, 40.0);
        s.targets.movement = Movement { kind: MovementKind::Strafe, ..Default::default() };
        s.timing.duration_s = 20.0;
        let mut sim = Sim::adaptive(s, 3, None, Mix::Balanced);
        let start = sim.rating().unwrap().1;
        while sim.phase() != Phase::Finished {
            let at = sim.targets()[0].pos;
            sim.step(FRAME, Input { aim: at.normalize(), fire_pressed: false, fire_held: false });
        }
        let r = sim.result();
        let a = r.adaptive.unwrap();
        assert_eq!(a.knob, Knob::TargetSpeed);
        assert_eq!(a.success_rate, Some(1.0));
        assert!(a.value > start, "perfect tracking rates faster: {start} -> {}", a.value);
        assert!((9..=10).contains(&a.outcomes), "one outcome per 2 s segment: {}", a.outcomes);
        // Samples show the target really moving at the chosen speeds.
        let late: Vec<f32> = r.telemetry.samples.iter().filter(|x| x.t > 15.0).map(|x| x.target_speed_dps).collect();
        let mean = late.iter().sum::<f32>() / late.len() as f32;
        assert!(mean > start, "targets sped up: {mean} vs {start}");
    }

    fn prior_at(knob: Knob, value: f32, fastest: Option<f32>) -> Prior {
        Prior {
            theta: knob.h(value) + 0.12 * (0.85f32 / 0.15).ln(),
            slope: 0.12,
            sd: 0.08,
            days: 1.0,
            fastest,
            last_run: Some((Pick::Near, 0.85)),
        }
    }

    /// Playtest: adapting target by target inside a run felt chaotic. After the
    /// calibration run, every target in a run gets the same lifetime.
    #[test]
    fn later_runs_play_at_one_difficulty() {
        let mut s = no_countdown(Scenario::default());
        s.timing.duration_s = 20.0;
        let mut sim = Sim::adaptive(s, 4, Some(prior_at(Knob::Lifetime, 0.8, None)), Mix::Balanced);
        let (value, pick) = sim.run_pick().expect("a fixed-difficulty run");
        assert_ne!(pick, Pick::Calibrate);
        let start = sim.rating().unwrap().1;
        let mut i = 0;
        while sim.phase() != Phase::Finished {
            // Kill every other target; let the rest time out.
            if let Some(t) = sim.targets().first()
                && i % 40 == 0
            {
                let at = t.pos;
                sim.step(FRAME, shoot(at));
            } else {
                sim.step(FRAME, idle());
            }
            i += 1;
        }
        let r = sim.result();
        let targets = &r.telemetry.targets;
        assert!(targets.len() > 10);
        for t in targets {
            assert!((t.lifetime_s - value).abs() < 1e-6, "every target at {value}: {}", t.lifetime_s);
            assert_eq!(t.value, Some(value));
            assert_eq!(t.pick, Some(pick.as_str()));
        }
        let a = r.adaptive.unwrap();
        assert_eq!(a.run_value, Some(value));
        assert!(!a.calibration());
        assert!(a.outcomes >= 10, "the rating took in the run: {}", a.outcomes);
        assert_ne!(a.value, start, "and moved");
    }

    #[test]
    fn later_tracking_runs_keep_one_speed() {
        let mut s = no_countdown(Scenario::default());
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Always;
        s.targets.health = 1e9;
        s.targets.spawn.yaw = Range(-40.0, 40.0);
        s.targets.movement = Movement { kind: MovementKind::Strafe, ..Default::default() };
        s.timing.duration_s = 20.0;
        let mut sim = Sim::adaptive(s, 3, Some(prior_at(Knob::TargetSpeed, 30.0, None)), Mix::Balanced);
        let (value, _) = sim.run_pick().unwrap();
        while sim.phase() != Phase::Finished {
            let at = sim.targets()[0].pos;
            sim.step(FRAME, Input { aim: at.normalize(), fire_pressed: false, fire_held: false });
        }
        let r = sim.result();
        let a = r.adaptive.unwrap();
        assert!((9..=10).contains(&a.outcomes), "still one outcome per segment: {}", a.outcomes);
        let speeds: Vec<f32> = r.telemetry.samples.iter().map(|x| x.target_speed_dps).filter(|v| *v > 0.5).collect();
        let mean = speeds.iter().sum::<f32>() / speeds.len() as f32;
        assert!((mean / value - 1.0).abs() < 0.2, "targets move at the run's {value} deg/s: {mean}");
        assert!(a.value > 30.0, "perfect tracking raises the rating");
    }

    /// A breakthrough is holding the goal for a whole run at a value faster than
    /// your best, and is announced when the run ends.
    #[test]
    fn breakthroughs_come_at_the_end_of_a_run() {
        let mut s = no_countdown(Scenario::default());
        s.timing.duration_s = 20.0;
        let mut sim = Sim::adaptive(s, 6, Some(prior_at(Knob::Lifetime, 0.8, Some(3.0))), Mix::Balanced);
        let (value, _) = sim.run_pick().unwrap();
        let mut events = Vec::new();
        while sim.phase() != Phase::Finished {
            let at = sim.targets()[0].pos;
            events.push(sim.step(FRAME, shoot(at)));
            events.push(sim.step(FRAME, idle()));
        }
        let at: Vec<usize> =
            (0..events.len()).filter(|&i| events[i].iter().any(|e| matches!(e, Event::Breakthrough { .. }))).collect();
        assert_eq!(at.len(), 1, "one breakthrough");
        assert!(events[at[0]].contains(&Event::Finished), "on the last step");
        let a = sim.result().adaptive.unwrap();
        assert!(a.breakthrough);
        assert_eq!(a.fastest, Some(value));
    }

    fn speed_switch() -> Scenario {
        let mut s = builtin("builtin:speed-switch");
        s.timing.duration_s = 30.0;
        s
    }

    /// Plays a player who kills one target every `gap` seconds: looks away (up),
    /// then beams the first target until it dies. Returns the result.
    fn kill_every(mut sim: Sim, gap: f32) -> RunResult {
        let mut last = 0.0;
        while sim.phase() != Phase::Finished {
            let kills = sim.stats().kills;
            let input = if sim.elapsed() - last >= gap - 0.15 {
                let at = sim.targets()[0].pos;
                Input { aim: at.normalize(), fire_pressed: false, fire_held: true }
            } else {
                Input { aim: Vec3::Y, fire_pressed: false, fire_held: false }
            };
            sim.step(FRAME, input);
            if sim.stats().kills > kills {
                last = sim.elapsed();
            }
        }
        sim.result()
    }

    /// Playtest: Speed Switch's rating was mostly each target's own timer, which
    /// runs while you're busy with the others. It's now rated on the pace between
    /// kills, and its targets never time out.
    #[test]
    fn speed_switch_is_rated_on_kill_pace() {
        let s = speed_switch();
        assert_eq!(s.targets.lifetime_s, 0.0, "no per-target timers");
        assert_eq!(s.adaptive.resolve(&s).knob, Knob::KillPace);
        let prior = prior_at(Knob::KillPace, 1.0, None);
        let mut sim = Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced);
        let (value, _) = sim.run_pick().unwrap();
        assert!(sim.pace().is_some());
        sim.step(FRAME, idle());
        assert!(sim.pace().unwrap().1 > 0.0);

        // Comfortably inside the pace: every kill is a success.
        let fast = kill_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 0.6);
        let a = fast.adaptive.unwrap();
        assert_eq!(a.success_rate, Some(1.0), "{a:?}");
        assert!(fast.stats.kills as usize >= a.outcomes && a.outcomes > 20, "one outcome per kill: {}", a.outcomes);
        assert_eq!(fast.stats.timeouts, 0);

        // Slower than the pace: every window runs out, and the late kill isn't a
        // success either.
        let slow = kill_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 1.7);
        let a = slow.adaptive.unwrap();
        assert_eq!(a.success_rate, Some(0.0), "{a:?}");
        // Kills 1.7 paces apart: each gap holds one full empty window.
        let windows = (30.0 / (value * 1.7)) as usize;
        assert!(a.outcomes.abs_diff(windows) <= 2, "one failure per empty window: {} vs {windows}", a.outcomes);
        assert!(a.value > 1.0, "the rating slows down: {}", a.value);
    }

    /// Playtest: a workout showed a 1320 ms time to kill at 132 kills a minute.
    /// With several targets up, each one was timed from when it appeared, so it
    /// counted the time spent killing the others. TTK starts when you could start
    /// on the target: when it appears or at the previous kill, whichever is later.
    #[test]
    fn time_to_kill_doesnt_count_time_spent_on_other_targets() {
        let s = speed_switch();
        assert!(s.targets.count >= 3);
        let r = kill_every(Sim::new(s, 2), 0.45);
        let per_min = r.stats.kills as f32 * 2.0;
        assert!(per_min > 110.0, "{per_min} kills a minute");
        let ttk = r.avg_ttk_s().unwrap();
        assert!(ttk < 0.5, "a kill every 0.45 s can't average {ttk:.2} s to kill");
        // One target at a time: unchanged, from spawn to kill.
        let mut one = speed_switch();
        one.targets.count = 1;
        let r = kill_every(Sim::new(one, 2), 0.45);
        assert!((r.avg_ttk_s().unwrap() - 0.45).abs() < 0.05, "{:?}", r.avg_ttk_s());
    }

    #[test]
    fn kill_pace_calibration_finds_the_players_pace() {
        // A player who kills every 0.7 s, against a first guess of 1.5 s.
        let r = kill_every(Sim::adaptive(speed_switch(), 3, None, Mix::Balanced), 0.7);
        let a = r.adaptive.unwrap();
        assert!(a.calibration());
        assert!(a.value < 1.0 && a.value > 0.5, "rated near 0.7 s: {}", a.value);
    }

    /// Plays a clicker who shoots the oldest target (the first to time out)
    /// `gap` seconds after the last kill or timeout, after `misses` deliberate misses.
    fn click_every(mut sim: Sim, gap: f32, misses: u32) -> RunResult {
        let mut last = 0.0;
        while sim.phase() != Phase::Finished {
            let (kills, timeouts) = (sim.stats().kills, sim.stats().timeouts);
            let oldest = |sim: &Sim| sim.targets().iter().max_by(|a, b| a.age.total_cmp(&b.age)).map(|t| t.pos);
            if sim.elapsed() - last >= gap
                && let Some(at) = oldest(&sim)
            {
                for _ in 0..misses {
                    sim.step(FRAME, shoot(-at));
                    sim.step(FRAME, idle());
                }
                let at = oldest(&sim).unwrap_or(at);
                sim.step(FRAME, shoot(at));
            } else {
                sim.step(FRAME, idle());
            }
            if sim.stats().kills > kills || sim.stats().timeouts > timeouts {
                last = sim.elapsed();
            }
        }
        sim.result()
    }

    fn short(id: &str) -> Scenario {
        let mut s = builtin(id);
        s.timing.duration_s = 30.0;
        s
    }

    /// Scoring review, findings 1 and 2: in adaptive mode the untimed clicking
    /// drills got per-target timers and became copies of their timed versions, and
    /// with several targets up a lifetime rating measured the queue. They're rated
    /// on the pace between kills now.
    #[test]
    fn untimed_and_grid_clicking_is_rated_on_kill_pace() {
        for id in ["builtin:static-grid", "builtin:timed-grid", "builtin:wide-flicks"] {
            let s = short(id);
            assert_eq!(s.adaptive.resolve(&s).knob, Knob::KillPace, "{id}");
            let prior = prior_at(Knob::KillPace, 1.0, None);
            let value = Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced).run_pick().unwrap().0;
            let fast = click_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 0.6, 0);
            let a = fast.adaptive.unwrap();
            assert_eq!(a.success_rate, Some(1.0), "{id}: {a:?}");
            assert!(a.outcomes > 15, "{id}: one outcome per kill: {}", a.outcomes);
            let slow = click_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 1.7, 0);
            assert_eq!(slow.adaptive.unwrap().success_rate, Some(0.0), "{id}");
        }
        // Untimed stays untimed, and Timed Grid keeps its own 2 s timers.
        assert_eq!(short("builtin:static-grid").targets.lifetime_s, 0.0);
        let tg = short("builtin:timed-grid");
        let sim = Sim::adaptive(tg.clone(), 2, Some(prior_at(Knob::KillPace, 1.0, None)), Mix::Balanced);
        assert_eq!(sim.current().targets.lifetime_s, tg.targets.lifetime_s);
    }

    /// Kill pace on a clicking drill: a kill after a spray of misses isn't a
    /// success, as with the lifetime knob.
    #[test]
    fn spraying_clicks_doesnt_count_toward_kill_pace() {
        let s = short("builtin:static-grid");
        let prior = prior_at(Knob::KillPace, 1.0, None);
        let value = Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced).run_pick().unwrap().0;
        let one = click_every(Sim::adaptive(s.clone(), 2, Some(prior), Mix::Balanced), value * 0.5, 1);
        assert_eq!(one.adaptive.unwrap().success_rate, Some(1.0), "one miss is allowed");
        let spray = click_every(Sim::adaptive(s, 2, Some(prior), Mix::Balanced), value * 0.5, 2);
        assert_eq!(spray.adaptive.unwrap().success_rate, Some(0.0));
    }

    /// Scoring review, findings 3 and 4: Micro Flicks' lifetime rating was mostly
    /// reaction time, and Precise Track's speed rating turned it into reactive
    /// tracking as you improved. Both are rated on target size now, with time and
    /// speed held fixed.
    #[test]
    fn precision_drills_are_rated_on_size() {
        let micro = short("builtin:micro-flicks");
        assert_eq!(micro.adaptive.resolve(&micro).knob, Knob::Radius);
        let prior = prior_at(Knob::Radius, 0.1, None);
        let sim = Sim::adaptive(micro.clone(), 3, Some(prior), Mix::Balanced);
        let value = sim.run_pick().unwrap().0;
        assert!(sim.targets().iter().all(|t| (t.radius - value).abs() < 1e-6), "targets at the run's size");
        assert!(sim.targets().iter().all(|t| t.lifetime == 1.5), "a fixed, generous lifetime");
        // Hit in time: success. Too slow: each timeout is a failure.
        let quick = click_every(Sim::adaptive(micro.clone(), 3, Some(prior), Mix::Balanced), 0.4, 0);
        let a = quick.adaptive.unwrap();
        assert_eq!(a.success_rate, Some(1.0), "{a:?}");
        assert!(a.outcomes > 30);
        let late = click_every(Sim::adaptive(micro, 3, Some(prior), Mix::Balanced), 2.0, 0);
        assert_eq!(late.adaptive.unwrap().success_rate, Some(0.0));

        let precise = short("builtin:precise-track");
        assert_eq!(precise.adaptive.resolve(&precise).knob, Knob::Radius);
        let mut sim = Sim::adaptive(precise.clone(), 3, Some(prior_at(Knob::Radius, 0.15, None)), Mix::Balanced);
        let value = sim.run_pick().unwrap().0;
        assert_eq!(sim.current().targets.movement.speed, precise.targets.movement.speed, "speed stays slow");
        while sim.phase() != Phase::Finished {
            assert!((sim.targets()[0].radius - value).abs() < 1e-6);
            let aim = sim.targets()[0].pos.normalize();
            sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: false });
        }
        let a = sim.result().adaptive.unwrap();
        assert_eq!(a.success_rate, Some(1.0));
        assert!(a.outcomes >= 14, "one outcome per 2 s segment: {}", a.outcomes);
        // Calibration shrinks the ball segment by segment for a steady tracker.
        let mut cal = Sim::adaptive(precise.clone(), 3, None, Mix::Balanced);
        let mut sizes = Vec::new();
        while cal.phase() != Phase::Finished {
            sizes.push(cal.targets()[0].radius);
            let aim = cal.targets()[0].pos.normalize();
            cal.step(FRAME, Input { aim, fire_pressed: false, fire_held: false });
        }
        sizes.dedup();
        assert!(sizes.len() >= 5, "{sizes:?}");
        assert!(sizes.last().unwrap() < &precise.targets.radius, "{sizes:?}");
    }

    /// Scoring review, finding 6's follow-up: Strafe Clicks was a copy of Timed
    /// Strafe Clicks in adaptive mode. It's rated on strafe speed now, with a fixed
    /// lifetime, so the pair covers speed and time separately.
    #[test]
    fn strafe_clicks_is_rated_on_target_speed() {
        let s = short("builtin:strafe-clicks");
        assert_eq!(s.adaptive.resolve(&s).knob, Knob::TargetSpeed);
        assert!(s.problems().is_empty(), "{:?}", s.problems());
        let prior = prior_at(Knob::TargetSpeed, 20.0, None);
        let sim = Sim::adaptive(s.clone(), 4, Some(prior), Mix::Balanced);
        assert!(sim.rated_per_target());
        assert!(sim.targets().iter().all(|t| t.lifetime == s.targets.lifetime_s));
        let quick = click_every(Sim::adaptive(s.clone(), 4, Some(prior), Mix::Balanced), 0.3, 0);
        assert_eq!(quick.adaptive.unwrap().success_rate, Some(1.0));
        let late = click_every(Sim::adaptive(s, 4, Some(prior), Mix::Balanced), 2.5, 0);
        assert_eq!(late.adaptive.unwrap().success_rate, Some(0.0));
    }

    /// Scoring review, finding 5: Control Track promised smooth arcs but moved in
    /// straight lines with sharp turns. Curve movement keeps a steady speed while
    /// the heading turns a little every frame.
    #[test]
    fn curve_movement_turns_smoothly_at_constant_speed() {
        let control = builtin("builtin:control-track");
        assert_eq!(control.targets.movement.kind, MovementKind::Curve);
        let mv = control.targets.movement.clone();
        let mut s = mover(mv.clone());
        s.targets.spawn = control.targets.spawn.clone();
        let mut sim = Sim::new(s, 5);
        let heading = |t: &Target| {
            let (x, y) = screen(t);
            (y.atan2(x).to_degrees(), (x * x + y * y).sqrt())
        };
        let (mut prev, speed0) = heading(&sim.targets()[0]);
        let max_step = mv.turn_deg_s.1 * FRAME * 1.05;
        let (mut turned, mut smooth, mut frames) = (0.0f32, 0, 0);
        for _ in 0..(240 * 20) {
            let before = sim.targets()[0].rate;
            sim.step(FRAME, look_away());
            let t = &sim.targets()[0];
            let (h, speed) = heading(t);
            assert!((speed / speed0 - 1.0).abs() < 0.08, "speed {speed:.1} vs {speed0:.1} °/s");
            let step = (h - prev + 540.0).rem_euclid(360.0) - 180.0;
            // Bounces off the area's edge flip a component; everything else is a small turn.
            let bounced = t.rate[0].signum() != before[0].signum() || t.rate[1].signum() != before[1].signum();
            if !bounced {
                assert!(step.abs() <= max_step, "heading jumped {step:.2}° in a frame");
                turned += step.abs();
                smooth += 1;
            }
            frames += 1;
            prev = h;
        }
        assert!(smooth > frames * 9 / 10, "mostly smooth: {smooth} of {frames}");
        assert!(turned > 360.0, "it keeps curving: {turned:.0}°");
    }

    /// Median width of a strafe: degrees of yaw between one reversal and the next,
    /// following the first target (standard mode, nobody shooting).
    fn strafe_sweep(s: &Scenario, seed: u64) -> f32 {
        let mut s = s.clone();
        s.timing.duration_s = 40.0;
        let mut sim = Sim::new(s, seed);
        let (mut id, mut last_turn, mut dir) = (0, None::<f32>, 0.0f32);
        let mut sweeps = Vec::new();
        while sim.phase() != Phase::Finished {
            sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
            let Some(t) = sim.targets().first() else { continue };
            if t.id != id {
                (id, last_turn, dir) = (t.id, None, 0.0);
            }
            let d = t.rate[0].signum();
            if t.rate[0].abs() > 1e-3 && d != dir {
                if dir != 0.0 {
                    if let Some(y) = last_turn {
                        sweeps.push((t.yaw - y).abs());
                    }
                    last_turn = Some(t.yaw);
                }
                dir = d;
            }
        }
        sweeps.sort_by(f32::total_cmp);
        sweeps[sweeps.len() / 2]
    }

    /// Playtest: Hop Strafe Track barely moved side to side. It reversed every
    /// 0.35–1 s but took 0.4 s to turn around, so most of each strafe was spent
    /// turning: its strafes were 10.7° wide against Strafe Track's 15.4°. Drills
    /// with gradual turns must sweep at least as wide as the plain version.
    #[test]
    fn gradual_turns_still_sweep_as_wide_as_plain_strafes() {
        let sweep =
            |id: &str| (1..=3).map(|seed| strafe_sweep(&builtin(&format!("builtin:{id}")), seed)).sum::<f32>() / 3.0;
        let plain_track = sweep("strafe-track");
        let plain_clicks = sweep("timed-strafe-clicks");
        for (id, plain) in
            [("hop-strafe-track", plain_track), ("unbroken-strafes", plain_track), ("hop-clicks", plain_clicks)]
        {
            let w = sweep(id);
            assert!(w >= plain * 0.98, "{id} strafes {w:.1}° wide, the plain version {plain:.1}°");
        }
        let w = sweep("hop-switch");
        assert!(w >= 12.0, "hop-switch strafes {w:.1}° wide");
    }

    fn builtin(id: &str) -> Scenario {
        let e = crate::library::builtins().into_iter().find(|e| e.id == id).unwrap();
        no_countdown(e.scenario.unwrap())
    }

    /// Tracks the first target perfectly until it dies, then looks away for
    /// `travel` seconds before starting on the next one.
    fn track_with_travel(mut sim: Sim, travel: f32) -> RunResult {
        let mut away_until = 0.0;
        while sim.phase() != Phase::Finished {
            let kills = sim.stats().kills;
            let aim = if sim.elapsed() < away_until { Vec3::Y } else { sim.targets()[0].pos.normalize() };
            sim.step(FRAME, Input { aim, fire_pressed: false, fire_held: true });
            if sim.stats().kills > kills {
                away_until = sim.elapsed() + travel;
            }
        }
        sim.result()
    }

    /// Playtest: Stability Switch's rating was mostly the trip to the next target
    /// after each kill, which counted as off-target time. Now only time while a
    /// target is engaged (damaged, not yet healed) counts.
    #[test]
    fn stability_switch_ignores_travel_between_targets() {
        let mut s = builtin("builtin:stability-switch");
        s.timing.duration_s = 30.0;
        assert!(s.adaptive.engaged_only && s.targets.regen_per_s > 0.0 && s.targets.health_bar);
        let prior = prior_at(Knob::TargetSpeed, 12.0, None);
        let play = |s: &Scenario, travel| {
            let r = track_with_travel(Sim::adaptive(s.clone(), 4, Some(prior), Mix::Steady), travel);
            r.adaptive.unwrap().success_rate.unwrap()
        };
        let quick = play(&s, 0.0);
        let slow = play(&s, 0.6);
        assert!(quick > 0.97 && slow > 0.97, "travel doesn't count: {quick} vs {slow}");
        // The old scoring, for contrast: every trip is off-target time.
        let mut old = s.clone();
        old.adaptive.engaged_only = false;
        let slow_old = play(&old, 0.6);
        assert!(slow_old < 0.85, "travel used to count: {slow_old}");
    }

    /// Playtest: Stability Switch targets didn't move enough and were too big to
    /// make staying on them hard. Each sweep (one direction, between reversals)
    /// should average at least 15 degrees, on targets at most 3.5 degrees across.
    #[test]
    fn stability_switch_sweeps_wide_on_small_targets() {
        let mut s = builtin("builtin:stability-switch");
        s.targets.health = 1e9;
        s.timing.duration_s = 30.0;
        let d = (s.targets.spawn.distance.0 + s.targets.spawn.distance.1) / 2.0;
        let size = 2.0 * (s.targets.radius / d).atan().to_degrees();
        assert!(size <= 3.5, "targets are {size:.1} deg across");
        let mut sim = Sim::new(s, 3);
        let yaw = |t: &Target| t.pos.x.atan2(-t.pos.z).to_degrees();
        let mut last: HashMap<u64, (f32, f32)> = HashMap::new(); // yaw, direction
        let mut sweeps = Vec::new();
        let mut current: HashMap<u64, f32> = HashMap::new();
        while sim.phase() != Phase::Finished {
            sim.step(FRAME, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
            for t in sim.targets() {
                let y = yaw(t);
                if let Some(&(prev, dir)) = last.get(&t.id) {
                    let dy = y - prev;
                    let now_dir = dy.signum();
                    if dy.abs() > 1e-4 && dir != 0.0 && now_dir != dir {
                        sweeps.push(current.insert(t.id, 0.0).unwrap_or(0.0));
                    }
                    *current.entry(t.id).or_default() += dy.abs();
                    last.insert(t.id, (y, if dy.abs() > 1e-4 { now_dir } else { dir }));
                } else {
                    last.insert(t.id, (y, 0.0));
                }
            }
        }
        let mean = sweeps.iter().sum::<f32>() / sweeps.len().max(1) as f32;
        assert!(sweeps.len() >= 20, "{} sweeps", sweeps.len());
        assert!(mean >= 15.0, "average sweep {mean:.1} deg");
    }

    #[test]
    fn leaving_a_damaged_target_counts_until_it_heals() {
        let mut s = builtin("builtin:stability-switch");
        s.targets.count = 1;
        s.targets.movement.kind = MovementKind::Static;
        s.timing.duration_s = 10.0;
        let mut sim = Sim::adaptive(s.clone(), 1, Some(prior_at(Knob::TargetSpeed, 12.0, None)), Mix::Steady);
        let at = sim.targets()[0].pos.normalize();
        let held = |aim| Input { aim, fire_pressed: false, fire_held: true };
        // Half a second on target, then look away.
        for _ in 0..120 {
            sim.step(FRAME, held(at));
        }
        assert!(sim.targets()[0].engaged());
        let mut engaged_away = 0;
        for _ in 0..(240 * 4) {
            sim.step(FRAME, held(Vec3::Y));
            engaged_away += usize::from(sim.targets()[0].engaged());
        }
        assert_eq!(sim.targets()[0].health_fraction(), 1.0, "healed back to full");
        let healed_after = engaged_away as f32 * FRAME;
        // 50 damage at 40/s regen: 1.25 s to heal.
        assert!((healed_after - 1.25).abs() < 0.05, "engaged while healing: {healed_after}");
        let r = sim.result();
        let engaged: Vec<_> = r.telemetry.samples.iter().filter(|x| x.engaged).collect();
        let on = engaged.iter().filter(|x| x.on_target).count() as f32 / engaged.len() as f32;
        assert!((on - 0.5 / 1.75).abs() < 0.05, "half a second on, 1.25 s healing off: {on}");
    }

    #[test]
    fn flick_telemetry_measures_reaction_reach_and_overshoot() {
        let mut s = no_countdown(Scenario::default());
        s.targets.spawn.yaw = Range(20.0, 20.0);
        s.targets.spawn.pitch = Range(0.0, 0.0);
        let mut sim = Sim::new(s, 1);
        // Wait 0.2 s, then sweep right at 100°/s past the target, then come back and shoot.
        let mut yaw = 0.0f32;
        for i in 0..240 {
            if i >= 48 {
                yaw = (yaw + 100.0 * FRAME).min(24.0);
            }
            sim.step(FRAME, Input { aim: from_angles(yaw, 0.0), fire_pressed: false, fire_held: false });
        }
        sim.step(FRAME, Input { aim: from_angles(20.5, 0.0), fire_pressed: true, fire_held: true });
        let r = sim.result();
        let t = &r.telemetry.targets[0];
        assert_eq!(t.outcome, Outcome::Killed);
        assert!((t.distance_deg - 20.0).abs() < 0.01 && t.direction_deg.abs() < 0.5, "{t:?}");
        let reaction = t.reaction_s.unwrap();
        assert!((0.2..0.24).contains(&reaction), "moved 2° after 0.2 s wait: {reaction}");
        let reach = t.reach_s.unwrap();
        assert!((0.36..0.40).contains(&reach), "{reach}");
        assert!((t.overshoot_deg - (4.0 - t.radius_deg)).abs() < 0.05, "{t:?}");
        assert_eq!(t.entries, 2, "went past and came back");
        let shot = &r.telemetry.shots[0];
        assert!(shot.hit && shot.along > 0.0 && shot.across.abs() < 0.01, "{shot:?}");
    }

    #[test]
    fn replay_frames_follow_every_target() {
        let mut s = no_countdown(Scenario::default());
        s.targets.count = 2;
        s.targets.movement = Movement { kind: MovementKind::Strafe, ..Default::default() };
        let mut sim = Sim::new(s, 7);
        for _ in 0..240 {
            sim.step(FRAME, idle());
        }
        // Shoot one target dead, then keep going.
        let (no, at) = (sim.targets()[0].no(), sim.targets()[0].pos);
        while sim.targets().iter().any(|t| t.no() == no) {
            sim.step(FRAME, shoot(sim.targets().iter().find(|t| t.no() == no).map_or(at, |t| t.pos)));
        }
        let killed_at = sim.elapsed();
        for _ in 0..60 {
            sim.step(FRAME, idle());
        }
        let frames = sim.result().telemetry.frames;
        let span = frames.last().unwrap().t - frames[0].t;
        assert!((frames.len() as f32 / span - 60.0).abs() < 3.0, "{} frames in {span} s", frames.len());
        let last = frames.last().unwrap();
        assert_eq!(last.targets.len(), 2, "a new target replaced the dead one");
        assert!(frames.iter().filter(|f| f.t > killed_at + 0.02).all(|f| f.targets.iter().all(|p| p.no != no)));
        // Poses match the live targets.
        for (p, t) in last.targets.iter().zip(sim.targets()) {
            let (yaw, pitch) = angles(t.pos);
            assert_eq!(p.no, t.no());
            assert!((p.yaw_deg - yaw).abs() < 0.5 && (p.pitch_deg - pitch).abs() < 0.5, "{p:?}");
            assert!(p.radius_deg > 0.1 && p.height_deg == 0.0 && p.health > 0.0, "{p:?}");
        }
    }

    #[test]
    fn tracking_samples_show_lead_and_lag() {
        let mut s = no_countdown(Scenario::default());
        s.weapon.kind = WeaponKind::Beam;
        s.weapon.trigger = Trigger::Always;
        s.targets.health = 1e9;
        s.targets.spawn.yaw = Range(-30.0, 30.0);
        s.targets.movement =
            Movement { kind: MovementKind::Strafe, change_every_s: Range(30.0, 30.0), ..Default::default() };
        let mut sim = Sim::new(s, 4);
        for _ in 0..120 {
            let t = &sim.targets()[0];
            let (yaw, pitch) = angles(t.pos);
            // Aim 1° behind the target's motion.
            let behind = -t.rate[0].signum();
            sim.step(FRAME, Input { aim: from_angles(yaw + behind, pitch), fire_pressed: false, fire_held: false });
        }
        let samples = &sim.result().telemetry.samples;
        assert!(samples.len() >= 14, "{}", samples.len());
        let lead: f32 = samples.iter().skip(1).map(|s| s.lead_deg).sum::<f32>() / (samples.len() - 1) as f32;
        assert!(lead < -0.8, "behind the target reads as negative lead: {lead}");
        assert!(samples[3].target_speed_dps > 10.0);
    }
}
