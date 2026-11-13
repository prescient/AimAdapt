//! Where a bot with a head starts when its stack is inside a wall. A head makes a bot's stack (body and head
//! together, standing on the bottom of the body) taller than the body, and a stack that is taller than the bot's
//! rise above its spawn point starts with its bottom under the spawn point. On a floor that is inside the floor.
//!
//! Measured in KovaaK's 3.9.13 on 12 November 2026 with test maps (a ball body of radius 0.3 units under a head of
//! radius 40 at an offset of 200, so a stack 280.6 units tall that starts 139.7 under its spawn point):
//!
//! - a spawn point on the top of a block 300 thick and 360 wide, every side farther than that, puts the bot on
//!   the block, standing on the bottom of its body;
//! - on a block 200 wide, with the sides nearer, the bot is moved out of the block sideways by the distance to
//!   the side and then falls (a bot with no gravity floats there);
//! - on a block with a wall 100 thick flush against its nearest edge and higher than the stack, the falling bot
//!   ends at that edge against the wall, standing on the block's top.
//!
//! So a spawning body is moved out of a block the shortest way. The sim lifts a bot straight up, which is the
//! first case. The third is made here by moving the spawn point to the wall. The second, and every case that
//! wasn't measured, is refused with one feature, [`FEATURE`], and a detail that says which case it is.
//!
//! A fixed spawn offset counts first (measured the same day, with specks on blocks 300 thick): 150 up, which
//! leaves the stack 10.3 clear of a block 200 wide, the bot falls and stands on its block; 100 up, which leaves
//! it 39.7 inside, it stands on the block as well; and 60 level on a block 360 wide, which puts a side nearer
//! than the top, it is not on its block. So the stack is looked at where the offset puts it. A range of offsets
//! wasn't measured inside a block: the stack has to be clear of the walls everywhere in the range.
//!
//! Only a speck of a body under a head is looked at, because nothing wider was measured: a headless bot, and a
//! bot with a head whose body is [`SPECK`] or more in radius, start where they always did.

use aimadapt_sim::Vec3;
use aimadapt_sim::collide::Collider;
use aimadapt_sim::motion::Motion;
use aimadapt_sim::scenario::Head;
use aimadapt_sim::world::{World, WorldBox, WorldMesh};

use super::{UU_PER_M, heads, offsets, push_once, spawn_rise};
use crate::inventory::Gap;
use crate::settings::Section;

/// The feature of every refusal of this module.
pub(super) const FEATURE: &str = "a bot that starts inside a wall";

/// A body under this radius, 1 unit in meters, is a speck: smaller than any body with no head may be.
const SPECK: f32 = 0.01;
/// The places a range of spawn offsets is looked at, along each axis it has a length on.
const SHARES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
/// How far short of the wall's face a moved bot's body ends, beyond its radius: 0.1 of a unit, in meters.
const SHORT: f32 = 0.001;
/// How far a wall's face may be from the plane of the block's edge and still stand flush against it.
const FLUSH: f32 = 0.001;
/// A wall has to reach this far over the standing stack, 2 units, in meters.
const OVER: f32 = 0.02;
/// Two ways out within this factor of each other are a tie, which wasn't measured.
const TIE: f32 = 1.05;
/// What counts as level or square, as a fraction of an edge's length.
const SQUARE: f32 = 1e-4;

const SIDEWAYS: &str = "its shortest way out of the block it starts in is sideways with no wall flush against that \
                        edge; KovaaK's pushes it off and it falls, which the trainer doesn't play yet";
const BESIDE: &str = "a wall stands against the edge of the block it starts in but doesn't cover the whole bot, \
                      which wasn't measured";
const NEAR_TIE: &str = "two ways out of the wall are almost as short, and which one KovaaK's takes wasn't measured";
const DOWN: &str = "its shortest way out of the wall is down, which wasn't measured";
const NO_FALL: &str = "it doesn't fall, and where KovaaK's puts it inside a wall wasn't measured";
const MANY: &str = "it starts inside more than one wall, which wasn't measured";
const SHAPE: &str = "the wall it starts in isn't an upright block with the bot's middle over it, which wasn't measured";
const TOP: &str = "the top of the block it starts in isn't inside the bot's height, which wasn't measured";
const UNDER: &str = "its middle starts under the top of the block it is in, beside a wall, which wasn't measured";
const OFFSET: &str = "its spawn offset leaves it inside a block beside a wall, which wasn't measured";
const RANGE: &str = "its range of spawn offsets reaches into a wall, which wasn't measured";
const DIFFER: &str = "the bots that use one spawn point would start in different places";

/// The head and the body's radius, in meters, of a character this module looks at: a speck under a head.
fn speck(ch: &Section) -> Option<(Head, f32)> {
    let radius = ch.f32_or("MainBBRadius", 0.0) / UU_PER_M;
    heads::head(ch).filter(|_| radius < SPECK).map(|head| (head, radius))
}

/// Moves the spawn points where a speck of a body under a head would start inside a block, as measured, and
/// refuses the files where one starts inside a wall in a way that wasn't. `spawn_characters` lists, for each of
/// the world's spawn points, the characters allowed on it (none: all). Spawn volumes are left as they are.
pub(super) fn place(
    world: &mut World,
    kinds: &[(&Section, Option<Motion>)],
    spawn_characters: &[Vec<String>],
    turned: bool,
    notes: &mut Vec<String>,
) -> Result<(), Vec<Gap>> {
    let Some(first) = kinds.first().map(|(ch, _)| *ch) else { return Ok(()) };
    if world.meshes.is_empty() || !kinds.iter().any(|(ch, _)| speck(ch).is_some()) {
        return Ok(());
    }
    let scene = Scene { world, all: Collider::for_bots(&world.meshes), turned };
    let (mut gaps, mut moves) = (Vec::new(), Vec::new());
    for (i, spawn) in world.spawns.iter().enumerate() {
        // A volume is a box of places, not a point.
        if spawn.edges != [[0.0; 3]; 3] {
            continue;
        }
        let allowed = |ch: &Section| {
            let names = spawn_characters.get(i).map_or(&[][..], Vec::as_slice);
            names.is_empty() || names.iter().any(|n| n.eq_ignore_ascii_case(ch.name()))
        };
        let mut starts = Vec::new();
        for (ch, motion) in kinds.iter().filter(|(ch, _)| allowed(ch)) {
            let rise = spawn_rise(ch) - spawn_rise(first);
            let at = Vec3::from_array(spawn.pos) + Vec3::Y * rise;
            match scene.start(ch, motion, at, spawn.axes) {
                Ok(by) => starts.push(by),
                Err(detail) => push_once(&mut gaps, Gap::planned(FEATURE, format!("{}: {detail}", ch.name()))),
            }
        }
        let same = |a: Option<Vec3>, b: Option<Vec3>| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a.distance(b) < 1e-5,
            _ => false,
        };
        let Some(by) = starts.first().copied() else { continue };
        if !starts.iter().all(|s| same(*s, by)) {
            push_once(&mut gaps, Gap::planned(FEATURE, DIFFER));
        } else if let Some(by) = by {
            moves.push((i, by));
        }
    }
    if !gaps.is_empty() {
        return Err(gaps);
    }
    if !moves.is_empty() {
        notes.push(
            "a bot with a head that starts inside a block beside a wall is put at the block's edge, standing on it; \
             how far its middle is from the wall wasn't checked against KovaaK's"
                .into(),
        );
    }
    for (i, by) in moves {
        let pos = Vec3::from_array(world.spawns[i].pos) + by;
        world.spawns[i].pos = pos.to_array();
    }
    Ok(())
}

/// The map's walls as bots meet them.
struct Scene<'a> {
    world: &'a World,
    all: Collider,
    /// Whether the file's offsets are turned a quarter (see `offsets`).
    turned: bool,
}

/// A character's stack of body and head, standing upright.
struct Stack {
    middle: Vec3,
    /// Half the length of the middle segment of the capsule.
    half: f32,
    radius: f32,
}

impl Stack {
    fn bottom(&self) -> f32 {
        self.middle.y - self.half - self.radius
    }

    fn top(&self) -> f32 {
        self.middle.y + self.half + self.radius
    }
}

impl Scene<'_> {
    /// How far the spawn point of a bot of this character has to move so the sim starts it as KovaaK's does:
    /// `None` for no move. Its middle starts at `at` and its spawn offsets, which run along `axes` when the
    /// world doesn't say, are added to that. The reason (a detail) when that wasn't measured.
    fn start(
        &self,
        ch: &Section,
        motion: &Option<Motion>,
        at: Vec3,
        axes: [[f32; 3]; 3],
    ) -> Result<Option<Vec3>, &'static str> {
        let Some((head, radius)) = speck(ch) else { return Ok(None) };
        // A character whose offsets can't be read is refused for that where its kind is made.
        let Ok([lo, hi]) = offsets::read(ch, self.turned) else { return Ok(None) };
        // Body and head, never shorter than the body is wide: the stack the sim stands and bumps as.
        let body =
            if ch.str("MainBBType") == "Spheroid" { 2.0 * radius } else { ch.f32_or("MainBBHeight", 0.0) / UU_PER_M };
        let tall = (body + 2.0 * head.radius + head.offset).max(2.0 * radius);
        // The stack where a share of the way along each offset puts it, as the sim adds the offsets.
        let frame = self.world.offset_axes.unwrap_or(axes).map(Vec3::from_array);
        let stack_at = |share: [f32; 3]| {
            let offset: Vec3 = (0..3).map(|i| frame[i] * (lo[i] + (hi[i] - lo[i]) * share[i])).sum();
            Stack { middle: at + offset, half: (tall / 2.0 - radius).max(0.0), radius }
        };
        let clear = |stack: &Stack| !self.all.touches(stack.middle, stack.half, stack.radius);
        if lo != hi {
            let shares = |i: usize| if lo[i] == hi[i] { &SHARES[..1] } else { &SHARES[..] };
            let places = shares(0)
                .iter()
                .flat_map(|a| shares(1).iter().flat_map(move |b| shares(2).iter().map(move |c| [*a, *b, *c])));
            return if places.into_iter().all(|share| clear(&stack_at(share))) { Ok(None) } else { Err(RANGE) };
        }
        let stack = stack_at([0.0; 3]);
        if clear(&stack) {
            return Ok(None);
        }
        let falls = motion.as_ref().is_some_and(|m| !m.stays_put && m.gravity > 0.0 && m.flight.is_none());
        match self.way_out(&stack, falls)? {
            Some(_) if lo != [0.0; 3] => Err(OFFSET),
            by => Ok(by),
        }
    }

    /// The move for a stack that starts inside a block with no spawn offset, or where its fixed offset put it.
    fn way_out(&self, stack: &Stack, falls: bool) -> Result<Option<Vec3>, &'static str> {
        let block = self.block_of(stack)?;
        let ways = block.ways(stack);
        if !(ways[0] > SQUARE && block.top < stack.top() - SQUARE) {
            return Err(TOP);
        }
        let mut order = [0, 1, 2, 3, 4, 5];
        order.sort_by(|a, b| ways[*a].total_cmp(&ways[*b]));
        let (least, next) = (ways[order[0]], ways[order[1]]);
        if next < TIE * least {
            return Err(NEAR_TIE);
        }
        match order[0] {
            _ if !falls => Err(NO_FALL),
            0 => Ok(None),
            1 => Err(DOWN),
            side => self.against_a_wall(&block, side - 2, stack, ways[side]).map(Some),
        }
    }

    /// The block the stack starts in: the one solid it touches, as an upright box with the stack's axis over it.
    fn block_of(&self, stack: &Stack) -> Result<Upright, &'static str> {
        let touched: Vec<&WorldMesh> = self
            .world
            .meshes
            .iter()
            .filter(|m| !m.decor)
            .filter(|m| Collider::for_bots(std::slice::from_ref(*m)).touches(stack.middle, stack.half, stack.radius))
            .collect();
        let [mesh] = touched[..] else { return Err(MANY) };
        let block = self.world.walls.iter().find(|w| is_mesh_of(mesh, w)).and_then(Upright::of).ok_or(SHAPE)?;
        let (a, b) = block.local(stack.middle);
        let inside = |v: f32, len: f32| v > stack.radius && len - v > stack.radius;
        if inside(a, block.lu) && inside(b, block.lv) { Ok(block) } else { Err(SHAPE) }
    }

    /// The move to a wall flush against the block's edge `side`, where the stack stands on the block against it.
    fn against_a_wall(&self, block: &Upright, side: usize, stack: &Stack, out: f32) -> Result<Vec3, &'static str> {
        let n = block.normal(side);
        let Some(face) = self.world.walls.iter().find_map(|w| block.wall_face(w, side, stack)) else {
            // Something else beside the edge, or nothing: the stack moved just clear of the block.
            let clear = stack.middle + n * (out + SHORT);
            return Err(if self.all.touches(clear, stack.half, stack.radius) { BESIDE } else { SIDEWAYS });
        };
        // The sim first pushes a ball of the body's radius at the bot's middle out of the walls, the shortest
        // way. Beside the wall that way is into it, so the middle has to start over the block (as measured).
        if stack.middle.y - stack.radius < block.top - SQUARE {
            return Err(UNDER);
        }
        Ok(n * (face - stack.radius - SHORT - stack.middle.dot(n)))
    }
}

/// Whether a drawn cube is the mesh of this wall box: the same eight corners.
fn is_mesh_of(mesh: &WorldMesh, w: &WorldBox) -> bool {
    let [c, a, b, _, e, ..] = mesh.positions[..] else { return false };
    let (corner, edges) = (Vec3::from_array(w.corner), w.edges.map(Vec3::from_array));
    let near = |p: [f32; 3], q: Vec3| Vec3::from_array(p).distance(q) < 1e-4;
    mesh.positions.len() == 8
        && near(c, corner)
        && near(a, corner + edges[0])
        && near(b, corner + edges[1])
        && near(e, corner + edges[2])
}

/// A box with one vertical edge: the block a stack starts in, or a wall. Its footprint runs from `corner` along
/// the unit vectors `u` and `v` for `lu` and `lv`, its height from `bottom` to `top`. The corner's own height
/// doesn't matter.
struct Upright {
    corner: Vec3,
    u: Vec3,
    lu: f32,
    v: Vec3,
    lv: f32,
    bottom: f32,
    top: f32,
}

impl Upright {
    fn of(w: &WorldBox) -> Option<Upright> {
        let corner = Vec3::from_array(w.corner);
        let edges = w.edges.map(Vec3::from_array);
        let flat = |e: Vec3| Vec3::new(e.x, 0.0, e.z);
        let up = edges.iter().position(|e| e.length() > 0.0 && flat(*e).length() < SQUARE * e.length())?;
        let [a, b] = match up {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        let level = |e: Vec3| e.length() > 0.0 && e.y.abs() < SQUARE * e.length();
        if !level(edges[a]) || !level(edges[b]) {
            return None;
        }
        let (y0, y1) = (corner.y, corner.y + edges[up].y);
        Some(Upright {
            corner,
            u: flat(edges[a]).normalize(),
            lu: edges[a].length(),
            v: flat(edges[b]).normalize(),
            lv: edges[b].length(),
            bottom: y0.min(y1),
            top: y0.max(y1),
        })
    }

    /// A point's place across the footprint, along `u` and `v` from the corner.
    fn local(&self, p: Vec3) -> (f32, f32) {
        let d = p - self.corner;
        (d.dot(self.u), d.dot(self.v))
    }

    /// The six distances a stack inside the block has to move to be out of it: up (to the top from the stack's
    /// bottom), down (to the bottom from the stack's top), then out of each side: toward -u, +u, -v, +v. A side
    /// distance reaches the stack's own radius past the face.
    fn ways(&self, stack: &Stack) -> [f32; 6] {
        let (a, b) = self.local(stack.middle);
        let r = stack.radius;
        [self.top - stack.bottom(), stack.top() - self.bottom, a + r, self.lu - a + r, b + r, self.lv - b + r]
    }

    /// The direction out of a side, level.
    fn normal(&self, side: usize) -> Vec3 {
        [-self.u, self.u, -self.v, self.v][side]
    }

    /// Where the plane of a side is along its normal.
    fn plane(&self, side: usize) -> f32 {
        let n = self.normal(side);
        let reach = [0.0, self.lu, 0.0, self.lv][side];
        n.dot(self.corner) + reach
    }

    /// Where `w`, a wall, has its face along the normal of `side`, if it stands flush against that side and
    /// covers the stack standing on the block there: an upright box lined up with the block, its face on the
    /// side's plane (within [`FLUSH`]) facing the block, reaching along the edge past the stack's axis by its
    /// radius each way, from the block's top or below it up to the stack's height and [`OVER`] more over it.
    fn wall_face(&self, w: &WorldBox, side: usize, stack: &Stack) -> Option<f32> {
        let wall = Upright::of(w)?;
        if (wall.corner - self.corner).length() < 1e-6 && wall.top == self.top {
            return None;
        }
        let n = self.normal(side);
        let t = Vec3::Y.cross(n);
        let lined_up = |e: Vec3| [e.dot(n).abs(), e.dot(t).abs()].iter().any(|d| (d - 1.0).abs() < 1e-3);
        if !lined_up(wall.u) || !lined_up(wall.v) {
            return None;
        }
        let corners = [0.0, 1.0].into_iter().flat_map(|i| {
            [0.0, 1.0].into_iter().map(move |j| wall.corner + wall.u * (i * wall.lu) + wall.v * (j * wall.lv))
        });
        let (mut face, mut lo, mut hi) = (f32::MAX, f32::MAX, f32::MIN);
        for c in corners {
            face = face.min(c.dot(n));
            lo = lo.min(c.dot(t));
            hi = hi.max(c.dot(t));
        }
        let axis = stack.middle.dot(t);
        let flush = (face - self.plane(side)).abs() <= FLUSH;
        let covers = lo <= axis - stack.radius && hi >= axis + stack.radius;
        let tall = wall.bottom <= self.top + SQUARE && wall.top >= self.top + (stack.top() - stack.bottom()) + OVER;
        (flush && covers && tall).then_some(face)
    }
}

#[cfg(test)]
mod tests;
