//! The simulation: target spawning, movement, hit detection, scoring, adaptive
//! difficulty and telemetry. It knows nothing about rendering or input devices. The
//! engine feeds it the aim direction and trigger state each frame and draws whatever
//! targets it reports, which keeps all the rules testable without a window.
//!
//! Coordinates: the player's eye is the origin, -Z is straight ahead, +Y is up and
//! +X is right (the same as a Bevy camera with no rotation).

pub use crate::rng::Rng;
use glam::Vec3;

use crate::adaptive::{Knob, Mix, Pick, Prior, Rating, planned_kind};
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
            Shape::Capsule | Shape::Box => 2.0 * (self.height() / 2.0 / distance).atan().to_degrees(),
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

    /// Damage done over damage possible, which is what the run file's "Damage Done" and
    /// "Damage Possible" hold. `None` before anything could be done. Overkill isn't
    /// counted as done (a hit does no more than the target had left), so it lowers this.
    pub fn damage_efficiency(&self) -> Option<f32> {
        (self.damage_possible > 0.0).then(|| (self.damage_done / self.damage_possible).clamp(0.0, 1.0))
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
    /// Model parameters, kept to start the next run (log scale). `theta` is the curve
    /// midpoint that, with `slope`, puts the goal rate (slips counted) exactly at the
    /// unclamped rating: `h(value) = theta - slope * logit_with_slips(goal)`.
    pub theta: f32,
    pub slope: f32,
    /// Standard deviation of ln(slope).
    pub slope_sd: f32,
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
    /// The outcomes in target-equivalents (1 per clicking target, more per tracking
    /// segment), the measure the goal and plan rules use.
    pub evidence: f32,
    pub breakthrough: bool,
    /// Days since the last run, for a calibration run that follows a long break.
    pub break_days: Option<f32>,
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
    /// Days away, when this calibration run follows a long break.
    break_days: Option<f32>,
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
    /// Seconds into the run up to which helper bots' kicks have been applied.
    pushed_to: f64,
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
    /// level. After a long break (see [`Prior::needs_calibration`]) it's a
    /// calibration run too, but it starts from the old rating and keeps the old
    /// best; [`Sim::break_days`] says how long the break was. Otherwise the whole
    /// run plays at one difficulty picked from the mix around the rating, and the
    /// rating takes in the run as it's played.
    pub fn adaptive(scenario: Scenario, seed: u64, prior: Option<Prior>, mix: Mix) -> Sim {
        Sim::adaptive_in_block(scenario, seed, prior, mix, 1, 1)
    }

    /// An adaptive run that is run `rep` of `reps` in a block of the same drill. In a
    /// block of two or more, the run's kind follows [`planned_kind`] instead of the
    /// random draw. Calibration comes first: with no prior, or after a long break, it's
    /// a calibration run whatever the plan says.
    pub fn adaptive_in_block(
        scenario: Scenario,
        seed: u64,
        prior: Option<Prior>,
        mix: Mix,
        rep: u32,
        reps: u32,
    ) -> Sim {
        let cfg = scenario.adaptive.resolve(&scenario);
        let rating = Rating::new(cfg, prior);
        let after_break = prior.filter(Prior::needs_calibration).map(|p| p.days);
        let run = (prior.is_some() && after_break.is_none()).then(|| {
            let mut rng = Rng::new(seed ^ 0x9e37_79b9);
            match planned_kind(rep, reps, mix) {
                Some(kind) => rating.pick_planned(&mut rng, kind),
                None => rating.pick_run(&mut rng, mix),
            }
        });
        let state = AdaptiveRun { rating, mix, segment: None, run, breakthrough: false, break_days: after_break };
        let mut sim = Sim::build(scenario, seed, Some(state));
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
            pushed_to: 0.0,
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

    /// Days since the last run, if this is a calibration run that follows a long break.
    pub fn break_days(&self) -> Option<f32> {
        self.adaptive.as_ref().and_then(|a| a.break_days)
    }

    /// Adaptive mode: the value this whole run plays at and what kind of run it is,
    /// or `None` for a calibration run (and standard runs).
    pub fn run_pick(&self) -> Option<(f32, Pick)> {
        self.run_plan().map(|(v, _, p)| (v, p))
    }

    /// Adaptive mode: the value this whole run plays at, the chance of success it is
    /// planned at and what kind of run it is, or `None` for a calibration run.
    pub fn run_plan(&self) -> Option<(f32, f32, Pick)> {
        self.adaptive.as_ref().and_then(|a| a.run)
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

    /// The rate of the game clock against real time (1.0 unless the scenario is a
    /// time-scaled import).
    pub fn time_scale(&self) -> f32 {
        self.scenario.world.as_ref().map_or(1.0, |w| w.time_scale)
    }

    /// Real seconds of play so far: `elapsed()` is game seconds.
    pub fn real_elapsed(&self) -> f32 {
        self.elapsed() / self.time_scale()
    }

    pub fn countdown_left(&self) -> f32 {
        (self.scenario.timing.countdown_s - self.clock).max(0.0)
    }

    /// Points by the scenario's scoring rules (shown for standard runs).
    pub fn score(&self) -> f32 {
        let s = &self.scenario.scoring;
        let st = &self.stats;
        let mut points = s.per_kill * st.kills as f32
            + s.per_hit * st.hits as f32
            + s.per_miss * st.misses as f32
            + s.per_damage * st.damage_done
            + s.per_timeout * st.timeouts as f32;
        // Imported scenarios can add points for the time left and scale the whole score by
        // accuracy or damage efficiency, as KovaaK's does.
        let Some(w) = &self.scenario.world else { return points };
        points += w.time_score * self.time_left();
        let efficiency = if w.damage_efficiency { st.damage_efficiency().unwrap_or(1.0) } else { 1.0 };
        points * w.accuracy_mult.factor(st.accuracy()) * efficiency
    }

    /// Advances the simulation by `dt` real seconds with this frame's input. Once the
    /// run is going the game clock moves `dt * time_scale` seconds, and `clock` counts
    /// the countdown in real seconds and then game seconds.
    pub fn step(&mut self, dt: f32, input: Input) -> Vec<Event> {
        let events = Vec::new();
        if self.phase == Phase::Finished || dt <= 0.0 {
            return events;
        }
        let scale = self.time_scale();
        if self.phase == Phase::Running {
            let dt = dt * scale;
            self.clock += dt;
            return self.run_step(dt, input, events);
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
            // Only the part of this step past the countdown counts as play, and that
            // part is on the game clock.
            let over = (self.clock - self.scenario.timing.countdown_s) * scale;
            if scale != 1.0 {
                self.clock = self.scenario.timing.countdown_s + over;
            }
            return self.run_step(over, input, events);
        }
        events
    }

    fn run_step(&mut self, dt: f32, input: Input, mut events: Vec<Event>) -> Vec<Event> {
        let aim = input.aim.normalize_or(Vec3::NEG_Z);
        self.last_aim = aim;
        let now = self.elapsed();

        self.push_targets();
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
}

/// Whether a wall is nearer than `d` along `aim` from the eye.
fn blocked(walls: &Collider, aim: Vec3, d: f32) -> bool {
    !walls.is_empty() && walls.ray(Vec3::ZERO, aim, d).is_some()
}

/// Half the length of a target's middle segment: 0 for a ball.
fn half_segment(shape: Shape, radius: f32, height: f32) -> f32 {
    match shape {
        Shape::Sphere => 0.0,
        Shape::Capsule | Shape::Box => (height / 2.0 - radius).max(0.0),
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
        Shape::Box => box_hit(dir, center, Vec3::new(radius, height / 2.0, radius)),
    }
}

/// Slab test: the distance along the ray from the origin to the box centred on `center`
/// with half extents `half`. 0 when the origin is inside, `None` when the box is missed or behind.
fn box_hit(dir: Vec3, center: Vec3, half: Vec3) -> Option<f32> {
    let (mut near, mut far) = (f32::NEG_INFINITY, f32::INFINITY);
    for i in 0..3 {
        let (d, lo, hi) = (dir[i], center[i] - half[i], center[i] + half[i]);
        if d.abs() < 1e-9 {
            // Parallel to this pair of faces: the ray stays inside the slab or never enters it.
            if lo > 0.0 || hi < 0.0 {
                return None;
            }
            continue;
        }
        let (t0, t1) = (lo / d, hi / d);
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
    }
    (near <= far && far >= 0.0).then_some(near.max(0.0))
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

mod firing;
mod movement;
mod pushing;
mod rating;
mod record;
mod spawning;

#[cfg(test)]
mod tests;
