//! Shots and the beam: what they hit, damage and kills.

use super::*;

impl Sim {
    pub(super) fn fire_hitscan(&mut self, dt: f32, aim: Vec3, input: Input, events: &mut Vec<Event>) {
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

    pub(super) fn fire_shot(&mut self, aim: Vec3, events: &mut Vec<Event>) {
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

    pub(super) fn fire_beam(&mut self, dt: f32, aim: Vec3, input: Input, events: &mut Vec<Event>) {
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

    pub(super) fn damage(&mut self, i: usize, amount: f32, events: &mut Vec<Event>) {
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

    /// Index of the nearest target the ray along `aim` passes through, unless a wall
    /// is in the way.
    pub(super) fn pick(&self, aim: Vec3) -> Option<usize> {
        self.targets
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.hit_by(aim).map(|d| (i, d)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, d)| !blocked(&self.walls, aim, *d))
            .map(|(i, _)| i)
    }

    pub(super) fn nearest_to(&self, aim: Vec3) -> Option<usize> {
        self.targets
            .iter()
            .enumerate()
            .max_by(|a, b| aim.dot(a.1.pos.normalize()).total_cmp(&aim.dot(b.1.pos.normalize())))
            .map(|(i, _)| i)
    }
}
