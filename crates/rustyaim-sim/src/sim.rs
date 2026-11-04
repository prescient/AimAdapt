//! The simulation: target spawning, movement, hit detection and scoring. It knows
//! nothing about rendering or input devices. The engine feeds it the aim direction
//! and trigger state each frame and draws whatever targets it reports, which keeps
//! all the rules testable without a window.
//!
//! Coordinates: the player's eye is the origin, -Z is straight ahead, +Y is up and
//! +X is right (the same as a Bevy camera with no rotation).

use glam::Vec3;

use crate::scenario::{Movement, MovementKind, Range, Scenario, Shape, Trigger, WeaponKind};

/// Beam damage and accuracy are sampled at this rate, independent of frame rate.
pub const BEAM_TICK_HZ: f32 = 60.0;

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
    /// Degrees per second of yaw and pitch.
    rate: [f32; 2],
    next_change: f32,
    age: f32,
    spawned_at: f32,
    shots: u32,
    hits: u32,
    damage: f32,
}

impl Target {
    /// Health left as a share of full health.
    pub fn health_fraction(&self) -> f32 {
        (self.health / self.max_health).clamp(0.0, 1.0)
    }
}

/// Things that happened during a step, for effects and sounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Spawned { id: u64 },
    Hit { id: u64 },
    Killed { id: u64 },
    Miss,
    TimedOut { id: u64 },
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
    /// Seconds from the target appearing (or the clock starting) to its destruction.
    pub ttk_s: f32,
    pub shots: u32,
    pub hits: u32,
    pub damage: f32,
}

/// The finished run, ready to save.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub score: f32,
    pub stats: Stats,
    pub kills: Vec<KillRecord>,
    /// Seconds of play (excludes the countdown).
    pub duration_s: f32,
}

impl RunResult {
    pub fn avg_ttk_s(&self) -> Option<f32> {
        (!self.kills.is_empty()).then(|| self.kills.iter().map(|k| k.ttk_s).sum::<f32>() / self.kills.len() as f32)
    }
}

pub struct Sim {
    scenario: Scenario,
    rng: Rng,
    /// Seconds since the countdown began.
    clock: f32,
    phase: Phase,
    targets: Vec<Target>,
    next_id: u64,
    /// Running-clock times at which a replacement target should appear.
    pending: Vec<f32>,
    last_killed_dir: Option<Vec3>,
    stats: Stats,
    kills: Vec<KillRecord>,
    cooldown: f32,
    beam_acc: f32,
}

impl Sim {
    pub fn new(scenario: Scenario, seed: u64) -> Sim {
        let mut sim = Sim {
            scenario,
            rng: Rng::new(seed),
            clock: 0.0,
            phase: Phase::Countdown,
            targets: Vec::new(),
            next_id: 1,
            pending: Vec::new(),
            last_killed_dir: None,
            stats: Stats::default(),
            kills: Vec::new(),
            cooldown: 0.0,
            beam_acc: 0.0,
        };
        let mut events = Vec::new();
        for _ in 0..sim.scenario.targets.count {
            sim.spawn(&mut events);
        }
        if sim.scenario.timing.countdown_s <= 0.0 {
            sim.phase = Phase::Running;
        }
        sim
    }

    pub fn scenario(&self) -> &Scenario {
        &self.scenario
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

    pub fn score(&self) -> f32 {
        let s = &self.scenario.scoring;
        let st = &self.stats;
        s.per_kill * st.kills as f32
            + s.per_hit * st.hits as f32
            + s.per_miss * st.misses as f32
            + s.per_damage * st.damage_done
            + s.per_timeout * st.timeouts as f32
    }

    pub fn result(&self) -> RunResult {
        RunResult {
            score: self.score(),
            stats: self.stats.clone(),
            kills: self.kills.clone(),
            duration_s: self.elapsed(),
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
            if self.clock < self.scenario.timing.countdown_s {
                return events;
            }
            self.phase = Phase::Running;
            // Only the part of this step past the countdown counts as play.
            let over = self.clock - self.scenario.timing.countdown_s;
            return self.run_step(over, input, events);
        }
        self.run_step(dt, input, events)
    }

    fn run_step(&mut self, dt: f32, input: Input, mut events: Vec<Event>) -> Vec<Event> {
        let aim = input.aim.normalize_or(Vec3::NEG_Z);
        let now = self.elapsed();

        self.move_targets(dt, &mut events);

        self.spawn_due(now, &mut events);
        match self.scenario.weapon.kind {
            WeaponKind::Hitscan => self.fire_hitscan(dt, aim, input, &mut events),
            WeaponKind::Beam => self.fire_beam(dt, aim, input, &mut events),
        }
        // Targets with no respawn delay come back in the same frame.
        self.spawn_due(now, &mut events);

        let limit = self.scenario.timing.kill_limit;
        if self.time_left() <= 0.0 || (limit > 0 && self.stats.kills >= limit) {
            self.phase = Phase::Finished;
            events.push(Event::Finished);
        }
        events
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
        match self.pick(aim) {
            Some(i) => {
                self.targets[i].shots += 1;
                self.damage(i, damage, events);
            }
            None => {
                self.stats.misses += 1;
                events.push(Event::Miss);
                // A miss counts against whichever target is nearest the crosshair.
                if let Some(i) = self.nearest_to(aim) {
                    self.targets[i].shots += 1;
                }
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
        let regen = self.scenario.targets.regen_per_s * dt;
        let on = self.pick(aim);
        if regen > 0.0 {
            for (i, t) in self.targets.iter_mut().enumerate() {
                if !firing || Some(i) != on {
                    t.health = (t.health + regen).min(t.max_health);
                }
            }
        }
        if !firing {
            self.beam_acc = 0.0;
            return;
        }
        self.beam_acc += dt;
        while self.beam_acc >= tick && self.phase == Phase::Running {
            self.beam_acc -= tick;
            self.stats.shots += 1;
            self.stats.damage_possible += per_tick;
            match self.pick(aim) {
                Some(i) => {
                    self.targets[i].shots += 1;
                    self.damage(i, per_tick, events);
                }
                None => self.stats.misses += 1,
            }
        }
    }

    fn damage(&mut self, i: usize, amount: f32, events: &mut Vec<Event>) {
        let now = self.elapsed();
        let t = &mut self.targets[i];
        let dealt = amount.min(t.health);
        t.health -= dealt;
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
            ttk_s: now - t.spawned_at,
            shots: t.shots,
            hits: t.hits,
            damage: t.damage,
        });
        self.last_killed_dir = Some(t.pos.normalize());
        events.push(Event::Killed { id: t.id });
        self.pending.push(now + self.scenario.targets.respawn_delay_s);
    }

    /// Index of the nearest target the ray along `aim` passes through.
    fn pick(&self, aim: Vec3) -> Option<usize> {
        let shape = self.scenario.targets.shape;
        let height = self.scenario.targets.height;
        self.targets
            .iter()
            .enumerate()
            .filter_map(|(i, t)| ray_hit(aim, t.pos, t.radius, shape, height).map(|d| (i, d)))
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
        let tc = self.scenario.targets.clone();
        let mv = tc.movement.clone();
        let mut i = 0;
        while i < self.targets.len() {
            let t = &mut self.targets[i];
            t.age += dt;
            if tc.lifetime_s > 0.0 {
                if t.age >= tc.lifetime_s {
                    let t = self.targets.swap_remove(i);
                    self.stats.timeouts += 1;
                    events.push(Event::TimedOut { id: t.id });
                    self.pending.push(now + tc.respawn_delay_s);
                    continue;
                }
                let k = t.age / tc.lifetime_s;
                t.radius = tc.radius * (1.0 + (tc.shrink_to - 1.0) * k);
            }
            if mv.kind != MovementKind::Static {
                t.next_change -= dt;
                if t.next_change <= 0.0 {
                    t.next_change = self.rng.range(mv.change_every_s);
                    t.rate = new_rate(&mut self.rng, &mv, t.distance, t.pitch, t.rate);
                }
                // Bounce off the edges of the spawn area.
                let (yr, pr) = (tc.spawn.yaw, tc.spawn.pitch);
                (t.yaw, t.rate[0]) = bounce(t.yaw + t.rate[0] * dt, t.rate[0], yr);
                (t.pitch, t.rate[1]) = bounce(t.pitch + t.rate[1] * dt, t.rate[1], pr);
                t.pos = from_angles(t.yaw, t.pitch) * t.distance;
            }
            i += 1;
        }
    }

    fn spawn(&mut self, events: &mut Vec<Event>) {
        let tc = &self.scenario.targets;
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
        let distance = self.rng.range(sp.distance);
        let pos = self.fit_in_room(dir * distance);
        let id = self.next_id;
        self.next_id += 1;
        let mv = tc.movement.clone();
        let (yaw, pitch) = angles(pos);
        let distance = pos.length();
        let rate = new_rate(&mut self.rng, &mv, distance, pitch, [0.0, 0.0]);
        let next_change = self.rng.range(mv.change_every_s);
        self.targets.push(Target {
            id,
            pos,
            radius: tc.radius,
            health: tc.health,
            max_health: tc.health,
            yaw,
            pitch,
            distance,
            rate,
            next_change,
            age: 0.0,
            spawned_at: self.elapsed(),
            shots: 0,
            hits: 0,
            damage: 0.0,
        });
        events.push(Event::Spawned { id });
    }

    fn fit_in_room(&self, p: Vec3) -> Vec3 {
        let a = &self.scenario.arena;
        let r = self.scenario.targets.radius.max(self.scenario.targets.height / 2.0);
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
        MovementKind::Random => {
            let a = rng.next_f32() * std::f32::consts::TAU;
            let (x, y) = (a.cos(), a.sin() * mv.vertical);
            let len = (x * x + y * y).sqrt().max(1e-6);
            [x / len * speed * yaw_per_m, y / len * speed * per_m]
        }
    }
}

/// Keeps an angle inside `r`, reversing the rate when it hits an edge.
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
}
