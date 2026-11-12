//! A rough stand-in for a player, used by `--smoke`: it waits a human-ish reaction
//! time, moves toward the target with some error, corrects, and clicks once the
//! crosshair is on it. It exists to exercise the whole pipeline (input, hits,
//! adaptive levels, telemetry, saving) with realistic-looking data, not to play well.

use aimadapt_sim::Input;
use aimadapt_sim::scenario::WeaponKind;
use aimadapt_sim::sim::{Rng, Sim, angles, from_angles};
use aimadapt_sim::telemetry::angle_deg;

pub struct Bot {
    rng: Rng,
    target: Option<u64>,
    since: f32,
    delay: f32,
    /// Aim error in degrees (yaw, pitch), shrinking as the bot corrects.
    offset: (f32, f32),
    cooldown: f32,
    miss_early: bool,
}

impl Bot {
    pub fn new(seed: u64) -> Bot {
        Bot {
            rng: Rng::new(seed ^ 0xb07),
            target: None,
            since: 0.0,
            delay: 0.2,
            offset: (0.0, 0.0),
            cooldown: 0.0,
            miss_early: false,
        }
    }

    fn noise(&mut self) -> f32 {
        // Roughly normal: the sum of three uniforms, centered.
        (self.rng.next_f32() + self.rng.next_f32() + self.rng.next_f32() - 1.5) * 1.15
    }

    /// Moves `yaw`/`pitch` (degrees) for this frame and returns the trigger state.
    pub fn input(&mut self, sim: &Sim, yaw: &mut f64, pitch: &mut f64, dt: f32) -> Input {
        let aim = from_angles(*yaw as f32, *pitch as f32);
        let alive = |id| sim.targets().iter().any(|t| t.id == id);
        if self.target.is_none_or(|id| !alive(id)) {
            let next = sim.targets().iter().min_by(|a, b| angle_deg(aim, a.pos).total_cmp(&angle_deg(aim, b.pos)));
            self.target = next.map(|t| t.id);
            self.since = 0.0;
            self.delay = 0.16 + self.rng.next_f32() * 0.14;
            let r = next.map_or(0.5, |t| (t.aim_radius() / t.pos.length()).atan().to_degrees());
            self.offset = (self.noise() * r * 1.6, self.noise() * r * 1.2);
            self.miss_early = self.rng.next_f32() < 0.12;
        }
        self.since += dt;
        self.cooldown -= dt;
        let Some(t) = self.target.and_then(|id| sim.targets().iter().find(|t| t.id == id)) else {
            return Input { aim, fire_pressed: false, fire_held: false };
        };
        let beam = sim.scenario().weapon.kind == WeaponKind::Beam;
        if self.since > self.delay || beam {
            // At the head of a bot that only takes headshots, else at the middle of its body.
            let (ty, tp) = angles(t.aim_point());
            let k = 1.0 - (-dt * if beam { 9.0 } else { 12.0 }).exp();
            *yaw += f64::from((ty + self.offset.0 - *yaw as f32) * k);
            *pitch += f64::from((tp + self.offset.1 - *pitch as f32) * k);
            let settle = (-dt * if beam { 1.5 } else { 5.0 }).exp();
            self.offset = (self.offset.0 * settle, self.offset.1 * settle);
            if beam && self.rng.next_f32() < dt * 2.0 {
                self.offset = (self.noise() * 0.8, self.noise() * 0.5);
            }
        }
        let aim = from_angles(*yaw as f32, *pitch as f32);
        let r = (t.aim_radius() / t.pos.length()).atan().to_degrees();
        let off = angle_deg(aim, t.aim_point());
        let ready = self.since > self.delay + 0.05 && self.cooldown <= 0.0;
        let fire = !beam && ready && (off <= r * 0.9 || (self.miss_early && off <= r * 2.2));
        if fire {
            self.cooldown = 0.12;
            self.miss_early = false;
        }
        Input { aim, fire_pressed: fire, fire_held: beam }
    }
}
