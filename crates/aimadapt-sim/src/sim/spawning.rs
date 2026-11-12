//! Where new targets appear, and when.

use super::firing::HEAL_STEP_S;
use super::*;

/// What the abilities of a bot that left keep, with where it was and when it left.
pub(super) struct Left {
    pub(super) slot: u32,
    pub(super) kind: Option<u32>,
    pub(super) at: f32,
    brain: Brain,
}

impl Sim {
    /// A target that was taken out is replaced after its respawn delay. Its bot's brain (what its
    /// abilities keep, see `Brain`) is saved for the bot that takes its place in the same slot and kind.
    /// A slot that comes back as another kind leaves the brain waiting for the next bot of its own kind
    /// there (a decision: KovaaK's wasn't measured on it). The list can't grow without end: a bot leaves
    /// one brain when it goes and takes one of its slot and kind when it comes, so a slot and kind never
    /// has more waiting than it had bots up at once.
    pub(super) fn queue_respawn(&mut self, t: &mut Target, now: f32) {
        self.pending.push(now + t.respawn_s);
        let motion = self.current.world.as_ref().and_then(|w| w.motion_of(t.kind));
        if let (Some(b), Some(m)) = (&mut t.bot, motion)
            && let Some(brain) = b.take_brain(m)
        {
            self.brains.push(Left { slot: t.slot, kind: t.kind, at: now, brain });
        }
    }

    /// Replaces the opening targets, so they use the current difficulty.
    pub(super) fn respawn_all(&mut self) {
        self.targets.clear();
        self.turns.clear();
        self.brains.clear();
        self.next_id = 1;
        self.next_no = 1;
        let mut events = Vec::new();
        for _ in 0..self.scenario.targets.count {
            self.spawn(&mut events);
        }
    }

    /// Replaces destroyed targets whose delay is up.
    pub(super) fn spawn_due(&mut self, now: f32, events: &mut Vec<Event>) {
        let due = self.pending.iter().filter(|&&t| t <= now).count();
        self.pending.retain(|&t| t > now);
        for _ in 0..due {
            self.spawn(events);
        }
    }

    pub(super) fn spawn(&mut self, events: &mut Vec<Event>) {
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
        let mut on_floor_at_spawn = false;
        let (pos, right) = match &self.current.world {
            // Imported scenarios: the map's spawn points, kept apart by distance, out
            // of walls and where the player's view allows.
            Some(w) => {
                let others: Vec<Vec3> = self.targets.iter().map(|t| t.pos).collect();
                let (shape, radius, height) = chosen
                    .as_ref()
                    .map_or((self.scenario.targets.shape, tc.radius, tc.height), |k| (k.shape, k.radius, k.height));
                let (pos, right) = w.spawn_point_in(&mut self.rng, &others, radius, self.last_aim, chosen.as_ref());
                match w.motion_of(kind) {
                    // A moving bot also leaves any other wall it starts in, and starts on a
                    // floor that is within its step-up height below its feet.
                    Some(m) => {
                        let half = half_segment(shape, radius, height);
                        let pos = self.bot_walls.lift_out(pos, half, radius);
                        match m.spawn_floor(&self.bot_walls, pos, half, radius) {
                            Some(on_floor) => {
                                on_floor_at_spawn = true;
                                (on_floor, right)
                            }
                            None => (pos, right),
                        }
                    }
                    None => (pos, right),
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
        // The oldest brain left by a bot of this slot and kind goes to the bot that takes its place.
        let left = self.brains.iter().position(|l| l.slot == slot && l.kind == kind).map(|i| self.brains.remove(i));
        let bot = match self.current.world.as_ref().and_then(|w| w.motion_of(kind)) {
            Some(m) => {
                let mut b = match left {
                    Some(l) => BotState::reborn(m, &mut self.rng, pos, right, Some(l.brain), (now - l.at).max(0.0)),
                    None => BotState::new(m, &mut self.rng, pos, right),
                };
                b.grounded = on_floor_at_spawn;
                Some(b)
            }
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
            // Healing timers go by the start of the frame (see `heal_in_steps`).
            heal_at: self.frame_began + if regen.1 > 0.0 { regen.1 } else { HEAL_STEP_S },
            respawn_s,
        });
        events.push(Event::Spawned { id });
    }

    pub(super) fn fit_in_room(&self, p: Vec3) -> Vec3 {
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
