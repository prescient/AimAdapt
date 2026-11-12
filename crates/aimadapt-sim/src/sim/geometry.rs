//! Where a target's body and head are, what a ray meets first, and how big a target is when it stands or
//! bumps into things (plan K9).
//!
//! What was measured in KovaaK's 3.9.13 on 12 November 2026, with test maps (sizes in meters; `B` is the
//! body's height in the stack, the capsule's height or a ball's `2R`):
//!
//! - The body keeps its own size: a capsule of radius `R` and height `H`, or a ball of radius `R` whatever
//!   its height setting says.
//! - The head is a ball of radius `r`. Its middle is `r + offset` above the body's top. Offset 0 rests it
//!   on the top, a negative offset sinks it into the body and a positive one leaves a gap that shots pass
//!   through.
//! - Body and head together are centred on the character's middle, where a headless bot's middle is: the
//!   body's middle is `(2r + offset) / 2` below it, the head's middle `(B + offset) / 2` above it, and the
//!   stack is `B + 2r + offset` tall.
//! - A shot hits one bot at most, and the first surface on the ray decides whether it is a head hit or a
//!   body hit. A capsule with a head stands with its body's bottom on the floor, so it is `B + 2r + offset`
//!   tall (measured at offsets 0 and -0.5). `MainBBHide` changes nothing about what a shot hits.
//!
//! Decisions that weren't measured:
//!
//! - An offset below `-2r`: the same formulas, read literally. The stack can then be shorter than the body.
//! - A ball with a head under gravity stands as the capsule was measured to, on the stack's bottom.
//! - What a bot with a head bumps into: a capsule of the body's radius `R` and the stack's full height
//!   (never less than `2R`), centred on the character's middle. A head wider than the body doesn't widen it.
//! - The sim doesn't pierce: a shot stops at the nearest surface, so the body of a bot that only takes
//!   headshots stops a shot meant for another bot behind it.
//! - A target that shrinks scales its head and offset with its radius, as its height does.
//! - A box body with a head: the body is a box `B` tall, like a capsule's height. The importer refuses it.
//! - Where a ray starts inside a body, the distance is 0.

use super::*;
use crate::scenario::Head;

/// A head on a target, as the target was made: the radius and offset at its base size, in meters, and
/// whether only the head counts. The colour stays with the scenario, as the body's does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadShape {
    pub radius: f32,
    pub offset: f32,
    pub only: bool,
}

impl HeadShape {
    /// The scenario's head, scaled by `scale` (a target whose radius is not the scenario's: its height
    /// scales the same way).
    pub fn of(head: &Head, scale: f32) -> HeadShape {
        HeadShape { radius: head.radius * scale, offset: head.offset * scale, only: head.headshots_only }
    }
}

/// Which part of a target a shot meets first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Body,
    Head,
}

impl Target {
    /// The head's radius and offset at the target's current size.
    fn head_now(&self) -> Option<(f32, f32)> {
        let scale = self.radius / self.base_radius.max(1e-6);
        self.head.map(|h| (h.radius * scale, h.offset * scale))
    }

    /// The body's height in the stack: a ball's `2R`, a capsule's or box's height.
    pub fn body_height(&self) -> f32 {
        if self.shape == Shape::Sphere { 2.0 * self.radius } else { self.height() }
    }

    /// How tall body and head are together: `B + 2r + offset`. The body's height without a head.
    pub fn stack_height(&self) -> f32 {
        match self.head_now() {
            Some((r, o)) => self.body_height() + 2.0 * r + o,
            None => self.body_height(),
        }
    }

    /// The middle of the body: `(2r + offset) / 2` below `pos` when there is a head, else `pos`.
    pub fn body_middle(&self) -> Vec3 {
        match self.head_now() {
            Some((r, o)) => self.pos - Vec3::Y * ((2.0 * r + o) / 2.0),
            None => self.pos,
        }
    }

    /// The middle of the head, `(B + offset) / 2` above `pos`.
    pub fn head_middle(&self) -> Option<Vec3> {
        self.head_now().map(|(_, o)| self.pos + Vec3::Y * ((self.body_height() + o) / 2.0))
    }

    /// The head's radius at the target's current size.
    pub fn head_radius(&self) -> Option<f32> {
        self.head_now().map(|(r, _)| r)
    }

    /// Whether only hits on the head count.
    pub fn headshots_only(&self) -> bool {
        self.head.is_some_and(|h| h.only)
    }

    /// Where to aim: the head's middle for a bot that only takes headshots, the body's middle otherwise.
    pub fn aim_point(&self) -> Vec3 {
        match self.head_middle() {
            Some(head) if self.headshots_only() => head,
            _ => self.body_middle(),
        }
    }

    /// The radius of what counts as a hit around [`Target::aim_point`]: the head's for a bot that only
    /// takes headshots, else the target's.
    pub fn aim_radius(&self) -> f32 {
        match self.head_radius() {
            Some(r) if self.headshots_only() => r,
            _ => self.radius,
        }
    }

    /// How far above `pos` the highest point of the target is, for the bars drawn over it. With a head
    /// that is the top of the stack, `(B + 2r + offset) / 2`, unless a head sunk deeper than twice its
    /// radius leaves the body's top higher.
    pub fn top(&self) -> f32 {
        match self.head_now() {
            Some((r, o)) => {
                let body_top = (self.body_height() - 2.0 * r - o) / 2.0;
                let head_top = (self.body_height() + 2.0 * r + o) / 2.0;
                body_top.max(head_top)
            }
            None if self.shape == Shape::Sphere => self.radius,
            None => (self.height() / 2.0).max(self.radius),
        }
    }

    /// Half the length of the middle segment of the capsule the target stands and bumps as: the body's
    /// own, or with a head the stack's (radius `R`, height `B + 2r + offset` and never under `2R`).
    pub(super) fn bump_half_segment(&self) -> f32 {
        stack_half_segment(self.shape, self.radius, self.height(), self.head_now())
    }

    /// Half the height of that capsule, for a target that moves freely inside its spawn area. Without a
    /// head it is worked out as it was before there were heads, to the last bit: a ball's radius, or half
    /// the height and never under the radius.
    pub(super) fn bump_half_height(&self) -> f32 {
        match self.head {
            None if self.shape == Shape::Sphere => self.radius,
            None => (self.height() / 2.0).max(self.radius),
            Some(_) => self.bump_half_segment() + self.radius,
        }
    }

    /// Distance along the ray from the eye to the first surface of the target it meets, and which part
    /// that is. Without a head this is the body's own test (`ray_hit`); with one, both parts are tested
    /// for where the ray enters them and the nearer wins.
    pub(super) fn hit_by(&self, aim: Vec3) -> Option<(f32, Part)> {
        let Some((r, _)) = self.head_now() else {
            return ray_hit(aim, self.pos, self.radius, self.shape, self.height()).map(|d| (d, Part::Body));
        };
        let body_at = self.body_middle();
        let body = match self.shape {
            Shape::Sphere => sphere_entry(aim, body_at, self.radius),
            Shape::Capsule => capsule_entry(aim, body_at, self.radius, (self.height() / 2.0 - self.radius).max(0.0)),
            Shape::Box => box_hit(aim, body_at, Vec3::new(self.radius, self.height() / 2.0, self.radius)),
        };
        let head = self.head_middle().and_then(|at| sphere_entry(aim, at, r));
        match (body, head) {
            (Some(b), Some(h)) if h <= b => Some((h, Part::Head)),
            (Some(b), _) => Some((b, Part::Body)),
            (None, Some(h)) => Some((h, Part::Head)),
            (None, None) => None,
        }
    }

    /// Whether a hit on `part` does anything: every hit on a head, and those on a body unless the bot
    /// only takes headshots.
    pub(super) fn counts(&self, part: Part) -> bool {
        part == Part::Head || !self.headshots_only()
    }

    /// The distance to where the ray meets a part that counts, if it does.
    pub(super) fn counted_hit(&self, aim: Vec3) -> Option<f32> {
        self.hit_by(aim).filter(|(_, part)| self.counts(*part)).map(|(d, _)| d)
    }
}

/// Half the length of the middle segment of the capsule a target of this shape and size stands and
/// bumps as. `head` is its radius and offset at the target's current size. Without one it is
/// `half_segment`; with one, the stack's: radius `R`, height `B + 2r + offset` and never under `2R`.
pub(super) fn stack_half_segment(shape: Shape, radius: f32, height: f32, head: Option<(f32, f32)>) -> f32 {
    match head {
        None => half_segment(shape, radius, height),
        Some((r, o)) => {
            let body = if shape == Shape::Sphere { 2.0 * radius } else { height };
            ((body + 2.0 * r + o).max(2.0 * radius) / 2.0 - radius).max(0.0)
        }
    }
}

/// Distance from the eye along `dir` to where the ray enters the ball, 0 when it starts inside.
fn sphere_entry(dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let along = center.dot(dir);
    let miss2 = center.length_squared() - along * along;
    if miss2 > radius * radius {
        return None;
    }
    let half = (radius * radius - miss2).sqrt();
    (along + half >= 0.0).then(|| (along - half).max(0.0))
}

/// Distance from the eye along `dir` to where the ray enters the upright capsule whose middle is `center`,
/// with a middle segment of half-length `half`: the nearest of its side and its two rounded ends, 0 when the
/// eye is inside. (`ray_hit` gives the distance of closest approach to the core instead, which is enough to
/// tell whether a headless target is hit but not where a surface is.)
fn capsule_entry(dir: Vec3, center: Vec3, radius: f32, half: f32) -> Option<f32> {
    let (lo, hi) = (center.y - half, center.y + half);
    let core = Vec3::new(center.x, 0.0f32.clamp(lo, hi), center.z);
    if core.length() <= radius {
        return Some(0.0);
    }
    let mut best: Option<f32> = None;
    let mut take = |t: f32| best = Some(best.map_or(t, |b: f32| b.min(t)));
    // The side: the infinite cylinder, where it is between the ends.
    let a = dir.x * dir.x + dir.z * dir.z;
    if a > 1e-12 {
        let b = dir.x * center.x + dir.z * center.z;
        let c = center.x * center.x + center.z * center.z - radius * radius;
        let disc = b * b - a * c;
        if disc >= 0.0 {
            let s = disc.sqrt();
            if let Some(t) =
                [(b - s) / a, (b + s) / a].into_iter().find(|t| *t >= 0.0 && (lo..=hi).contains(&(dir.y * t)))
            {
                take(t);
            }
        }
    }
    // The ends: the half of each ball that sticks out past the side.
    for (end, up) in [(hi, true), (lo, false)] {
        if let Some(t) = sphere_entry(dir, Vec3::new(center.x, end, center.z), radius) {
            let y = dir.y * t;
            if (up && y >= end) || (!up && y <= end) {
                take(t);
            }
        }
    }
    best
}
