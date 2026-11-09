//! The simulation: target spawning, movement, hit detection, scoring, adaptive
//! difficulty and telemetry. It knows nothing about rendering or input devices. The
//! engine feeds it the aim direction and trigger state each frame and draws whatever
//! targets it reports, which keeps all the rules testable without a window.
//!
//! Coordinates: the player's eye is the origin, -Z is straight ahead, +Y is up and
//! +X is right (the same as a Bevy camera with no rotation).

pub use crate::rng::Rng;
use glam::Vec3;

use crate::adaptive::{Knob, Mix, Pick, Prior, Rating};
use crate::collide::Collider;
use crate::motion::{BotState, separate};
use crate::scenario::{Movement, MovementKind, Range, Scenario, Shape, Trigger, WeaponKind};
use crate::telemetry::{
    Outcome, ShotRecord, TargetFrame, TargetPose, TargetRecord, Telemetry, TrackSample, angle_deg, screen_offset,
};
use crate::world::World;

/// Beam damage and accuracy are sampled at this rate, independent of frame rate.
pub const BEAM_TICK_HZ: f32 = 60.0;
/// Adaptive clicking: a kill after more misses than this counts as a failure, so
/// spraying clicks can't fake a fast rating.
pub const MAX_MISSES_PER_TARGET: u32 = 1;
/// Tracking telemetry sample rate.
pub const SAMPLE_HZ: f32 = 30.0;
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
    /// An imported scenario's moving bot: its velocity and dodge timers.
    bot: Option<BotState>,
    /// Imported scenarios with several kinds of bot: which kind this is, and the bot
    /// slot it fills.
    kind: Option<u32>,
    slot: u32,
    shape: Shape,
    /// Health regenerated per second, and the seconds after a hit before it starts.
    regen: (f32, f32),
    /// Seconds before a replacement appears.
    respawn_s: f32,
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
    /// Which kind of bot this is, in a scenario imported with several.
    pub fn kind(&self) -> Option<u32> {
        self.kind
    }

    pub fn shape(&self) -> Shape {
        self.shape
    }

    fn pose(&self) -> TargetPose {
        let (yaw_deg, pitch_deg) = angles(self.pos);
        let distance = self.pos.length().max(0.1);
        let height_deg = match self.shape {
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

    fn hit_by(&self, aim: Vec3) -> Option<f32> {
        let h = self.height * self.radius / self.base_radius.max(1e-6);
        ray_hit(aim, self.pos, self.radius, self.shape, h)
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
    /// An imported map's solid walls: shots stop at them. Bots bump into those and
    /// the invisible clip walls too.
    walls: Collider,
    bot_walls: Collider,
    /// Seconds since the countdown began.
    clock: f32,
    phase: Phase,
    targets: Vec<Target>,
    next_id: u64,
    next_no: u32,
    /// Running-clock times at which a replacement target should appear.
    pending: Vec<f32>,
    /// Picks so far from each of the world's in-order rotations.
    turns: Vec<u32>,
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
        self.turns.clear();
        self.next_id = 1;
        self.next_no = 1;
        let mut events = Vec::new();
        for _ in 0..self.scenario.targets.count {
            self.spawn(&mut events);
        }
    }

    fn build(base: Scenario, seed: u64, adaptive: Option<AdaptiveRun>) -> Sim {
        let current = base.clone();
        let walls = base.world.as_ref().map(|w| Collider::new(&w.meshes)).unwrap_or_default();
        let bot_walls = base.world.as_ref().map(|w| Collider::for_bots(&w.meshes)).unwrap_or_default();
        let mut sim = Sim {
            scenario: base,
            current,
            adaptive,
            rng: Rng::new(seed),
            walls,
            bot_walls,
            clock: 0.0,
            phase: Phase::Countdown,
            targets: Vec::new(),
            next_id: 1,
            next_no: 1,
            pending: Vec::new(),
            turns: Vec::new(),
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
        let walls = &self.walls;
        for t in &mut self.targets {
            let on = t.hit_by(aim).is_some_and(|d| !blocked(walls, aim, d));
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
            let targets = self.targets.iter().map(|t| t.pose()).collect();
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
                    on_target: t.hit_by(aim).is_some_and(|d| !blocked(&self.walls, aim, d)),
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
        // Regen: a target heals once its regen delay has passed since it was last hit.
        let now = self.elapsed();
        for t in self.targets.iter_mut().filter(|t| t.regen.0 > 0.0 && now - t.last_hit_at >= t.regen.1) {
            t.health = (t.health + t.regen.0 * dt).min(t.max_health);
        }
        let w = &self.scenario.weapon;
        let interval = if w.fire_rate > 0.0 { 1.0 / w.fire_rate } else { 0.0 };
        let trigger = w.trigger;
        let wants = match trigger {
            Trigger::Click => input.fire_pressed,
            Trigger::Hold => input.fire_held,
            Trigger::Always => true,
        };
        // The cooldown keeps the time left over from each frame, so the fire rate is
        // the same at any frame rate and a frame longer than the interval fires more
        // than once. The first shot goes when the trigger is seen, and while it's up
        // no time is banked for a burst on the next press.
        if wants && (trigger == Trigger::Click || interval > 0.0) {
            while self.cooldown <= 0.0 && self.phase == Phase::Running {
                self.fire_shot(aim, events);
                if trigger == Trigger::Click {
                    // One shot per press.
                    self.cooldown = interval;
                    break;
                }
                self.cooldown += interval;
            }
        }
        self.cooldown -= dt;
        if !wants || trigger == Trigger::Click {
            self.cooldown = self.cooldown.max(0.0);
        }
    }

    fn fire_shot(&mut self, aim: Vec3, events: &mut Vec<Event>) {
        let damage = self.scenario.weapon.damage;
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
        let now = self.elapsed();
        let on = self.pick(aim);
        for (i, t) in self.targets.iter_mut().enumerate() {
            if t.regen.0 > 0.0 && (!firing || Some(i) != on) && now - t.last_hit_at >= t.regen.1 {
                t.health = (t.health + t.regen.0 * dt).min(t.max_health);
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
        // Invincible bots (KovaaK's `InvincibleBots`) take the hit but lose no health.
        let invincible = self.current.world.as_ref().is_some_and(|w| w.invincible);
        let t = &mut self.targets[i];
        let dealt = if invincible { amount } else { amount.min(t.health) };
        if !invincible {
            t.health -= dealt;
        }
        t.last_hit_at = now;
        if let (Some(b), Some(m)) = (&mut t.bot, self.current.world.as_ref().and_then(|w| w.motion_of(t.kind))) {
            b.on_hit(m, &mut self.rng, dealt, now);
        }
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
        self.pending.push(now + t.respawn_s);
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

    /// Index of the nearest target the ray along `aim` passes through, unless a wall
    /// is in the way.
    fn pick(&self, aim: Vec3) -> Option<usize> {
        self.targets
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.hit_by(aim).map(|d| (i, d)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, d)| !blocked(&self.walls, aim, *d))
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
            // Negative regen drains a target, once its delay has passed since the last
            // hit, until it goes as a timeout (VT Aether's targets last 19 s).
            let drained = t.regen.0 < 0.0 && now - t.last_hit_at >= t.regen.1 && {
                t.health += t.regen.0 * dt;
                t.health <= 1e-4
            };
            if t.lifetime > 0.0 || drained {
                if drained || t.age >= t.lifetime {
                    let t = self.targets.swap_remove(i);
                    self.stats.timeouts += 1;
                    self.telemetry.targets.push(t.record(now, Outcome::TimedOut));
                    events.push(Event::TimedOut { id: t.id });
                    self.pending.push(now + t.respawn_s);
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
            if let Some(b) = &mut t.bot
                && let Some(m) = self.current.world.as_ref().and_then(|w| w.motion_of(t.kind))
            {
                // An imported scenario's bot: dodge profile and character movement.
                let half = half_segment(t.shape, t.radius, t.height * t.radius / t.base_radius.max(1e-6));
                let (yaw, pitch) = (t.yaw, t.pitch);
                b.step(m, &mut self.rng, &self.bot_walls, &mut t.pos, t.radius, half, now, dt);
                (t.yaw, t.pitch) = angles(t.pos);
                t.distance = t.pos.length();
                // Angular speed, for tracking telemetry.
                let dyaw = (t.yaw - yaw + 540.0).rem_euclid(360.0) - 180.0;
                t.rate = [dyaw / dt.max(1e-6), (t.pitch - pitch) / dt.max(1e-6)];
                i += 1;
                continue;
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
        if self.current.world.as_ref().is_some_and(World::bots_bump) {
            let mut bodies: Vec<(Vec3, f32, f32)> = self
                .targets
                .iter()
                .map(|t| {
                    (t.pos, t.radius, half_segment(t.shape, t.radius, t.height * t.radius / t.base_radius.max(1e-6)))
                })
                .collect();
            separate(&mut bodies);
            for (t, (p, _, _)) in self.targets.iter_mut().zip(bodies) {
                t.pos = p;
            }
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
        // Several kinds of bot: fill the first empty slot with a kind it allows.
        let (slot, kind) = match &self.current.world {
            Some(w) if !w.slots.is_empty() => {
                let free = (0..w.slots.len()).find(|s| self.targets.iter().all(|t| t.slot as usize != *s)).unwrap_or(0);
                (free as u32, w.pick_kind(&mut self.rng, free, &mut self.turns))
            }
            _ => (0, None),
        };
        let chosen = kind.and_then(|k| self.current.world.as_ref().and_then(|w| w.kinds.get(k as usize)).cloned());
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
        let (pos, right) = match &self.current.world {
            // Imported scenarios: the map's spawn points, kept apart by distance, out
            // of walls and where the player's view allows.
            Some(w) => {
                let others: Vec<Vec3> = self.targets.iter().map(|t| t.pos).collect();
                let (shape, radius, height) = chosen
                    .as_ref()
                    .map_or((self.scenario.targets.shape, tc.radius, tc.height), |k| (k.shape, k.radius, k.height));
                let allowed = chosen.as_ref().map_or(&[][..], |k| &k.spawns[..]);
                let (pos, right) = w.spawn_point_in(&mut self.rng, &others, radius, self.last_aim, allowed);
                if w.motion_of(kind).is_some() {
                    // A moving bot also leaves any other wall it starts in.
                    (self.bot_walls.lift_out(pos, half_segment(shape, radius, height), radius), right)
                } else {
                    (pos, right)
                }
            }
            None => {
                let distance = self.rng.range(sp.distance);
                (self.fit_in_room(dir * distance), Vec3::X)
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
        let mut height = tc.height * radius / self.scenario.targets.radius.max(1e-6);
        let (mut health, mut shape) = (tc.health, self.scenario.targets.shape);
        let (mut regen, mut respawn_s) = ((tc.regen_per_s, tc.regen_delay_s), tc.respawn_delay_s);
        if let Some(k) = &chosen {
            (radius, height, health, shape) = (k.radius, k.height, k.health, k.shape);
            (regen, respawn_s) = ((k.regen_per_s, k.regen_delay_s), k.respawn_delay_s);
        }
        let bot = match self.current.world.as_ref().and_then(|w| w.motion_of(kind)) {
            Some(m) => Some(BotState::new(m, &mut self.rng, pos, right)),
            None => None,
        };
        self.targets.push(Target {
            id,
            pos,
            radius,
            health,
            max_health: health,
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
            bot,
            kind,
            slot,
            shape,
            regen,
            respawn_s,
        });
        events.push(Event::Spawned { id });
    }

    fn fit_in_room(&self, p: Vec3) -> Vec3 {
        let a = &self.scenario.arena;
        let r = self.current.targets.radius.max(self.current.targets.height / 2.0);
        let half = |v: f32| (v / 2.0 - r).max(0.0);
        // A target taller than the room (adaptive size can grow one) sits halfway
        // up; `clamp` panics when its limits cross.
        let (lo, hi) = (-a.eye_height + r, a.size[1] - a.eye_height - r);
        let y = if lo <= hi { p.y.clamp(lo, hi) } else { (lo + hi) / 2.0 };
        Vec3::new(p.x.clamp(-half(a.size[0]), half(a.size[0])), y, p.z.clamp(-half(a.size[2]), half(a.size[2])))
    }
}

/// Whether a wall is nearer than `d` along `aim` from the eye.
fn blocked(walls: &Collider, aim: Vec3, d: f32) -> bool {
    !walls.is_empty() && walls.ray(Vec3::ZERO, aim, d).is_some()
}

/// Half the length of a target's middle segment: 0 for a ball.
fn half_segment(shape: Shape, radius: f32, height: f32) -> f32 {
    match shape {
        Shape::Sphere => 0.0,
        Shape::Capsule => (height / 2.0 - radius).max(0.0),
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

#[cfg(test)]
mod tests;
