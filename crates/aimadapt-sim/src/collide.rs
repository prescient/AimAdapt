//! An imported map's walls as solid triangles: shots stop at them, and moving bots
//! stand on them and bump into them. Triangles are two-sided, so a room made of one
//! big box works from the inside as well as the out.

use glam::Vec3;

use crate::world::WorldMesh;

#[derive(Debug, Clone, Copy)]
struct Tri {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    normal: Vec3,
    min: Vec3,
    max: Vec3,
}

/// A bounding box over a run of triangles; leaves hold triangles, inner nodes two
/// children (`first` and `first + 1`).
#[derive(Debug, Clone, Copy)]
struct Node {
    min: Vec3,
    max: Vec3,
    first: u32,
    count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Collider {
    tris: Vec<Tri>,
    nodes: Vec<Node>,
}

/// Where a sphere or capsule touched the walls after being pushed out of them.
#[derive(Debug, Clone, Default)]
pub struct Contact {
    pub pos: Vec3,
    /// Each wall it touched, as the direction from the wall toward its center.
    pub normals: Vec<Vec3>,
}

const LEAF: usize = 4;

impl Collider {
    /// The walls shots stop at: neither decorations nor invisible clip walls.
    pub fn new(meshes: &[WorldMesh]) -> Collider {
        Collider::from(meshes.iter().filter(|m| !m.decor && !m.clip))
    }

    /// The walls bots bump into: clip walls too.
    pub fn for_bots(meshes: &[WorldMesh]) -> Collider {
        Collider::from(meshes.iter().filter(|m| !m.decor))
    }

    fn from<'a>(meshes: impl Iterator<Item = &'a WorldMesh>) -> Collider {
        let mut tris = Vec::new();
        for m in meshes {
            for t in &m.triangles {
                let [a, b, c] = t.map(|i| Vec3::from_array(m.positions[i as usize]));
                let n = (b - a).cross(c - a);
                if n.length_squared() < 1e-12 {
                    continue;
                }
                tris.push(Tri { a, b, c, normal: n.normalize(), min: a.min(b).min(c), max: a.max(b).max(c) });
            }
        }
        let mut c = Collider { tris, nodes: Vec::new() };
        if !c.tris.is_empty() {
            c.nodes.push(Node { min: Vec3::ZERO, max: Vec3::ZERO, first: 0, count: 0 });
            let n = c.tris.len();
            c.build(0, 0, n);
        }
        c
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    fn build(&mut self, node: usize, start: usize, end: usize) {
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for t in &self.tris[start..end] {
            min = min.min(t.min);
            max = max.max(t.max);
        }
        if end - start <= LEAF {
            self.nodes[node] = Node { min, max, first: start as u32, count: (end - start) as u32 };
            return;
        }
        let size = max - min;
        let axis = if size.x >= size.y && size.x >= size.z {
            0
        } else if size.y >= size.z {
            1
        } else {
            2
        };
        let key = |t: &Tri| (t.min[axis] + t.max[axis]) / 2.0;
        self.tris[start..end].sort_by(|a, b| key(a).total_cmp(&key(b)));
        let mid = (start + end) / 2;
        let first = self.nodes.len();
        let blank = Node { min: Vec3::ZERO, max: Vec3::ZERO, first: 0, count: 0 };
        self.nodes.push(blank);
        self.nodes.push(blank);
        self.nodes[node] = Node { min, max, first: first as u32, count: 0 };
        self.build(first, start, mid);
        self.build(first + 1, mid, end);
    }

    /// Calls `f` with every triangle whose box overlaps `min..max`.
    fn each_near(&self, min: Vec3, max: Vec3, mut f: impl FnMut(&Tri)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let n = self.nodes[i];
            if n.min.cmpgt(max).any() || n.max.cmplt(min).any() {
                continue;
            }
            if n.count > 0 {
                for t in &self.tris[n.first as usize..(n.first + n.count) as usize] {
                    if !(t.min.cmpgt(max).any() || t.max.cmplt(min).any()) {
                        f(t);
                    }
                }
            } else {
                stack.push(n.first as usize);
                stack.push(n.first as usize + 1);
            }
        }
    }

    /// How far along `dir` (a unit vector) from `origin` the ray first meets a wall,
    /// if it does within `max_t`.
    pub fn ray(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = Vec3::ONE / dir;
        let mut best = max_t;
        let mut hit = false;
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            let n = self.nodes[i];
            if !slab(origin, inv, n.min, n.max, best) {
                continue;
            }
            if n.count > 0 {
                for t in &self.tris[n.first as usize..(n.first + n.count) as usize] {
                    if let Some(d) = ray_tri(origin, dir, t)
                        && d < best
                    {
                        best = d;
                        hit = true;
                    }
                }
            } else {
                stack.push(n.first as usize);
                stack.push(n.first as usize + 1);
            }
        }
        hit.then_some(best)
    }

    /// A capsule (a sphere when `half` is 0) standing upright at `center`, pushed out
    /// of every wall it overlaps: `half` is half the length of its middle segment.
    pub fn push_out(&self, center: Vec3, half: f32, radius: f32) -> Contact {
        let mut pos = center;
        let mut normals: Vec<Vec3> = Vec::new();
        for _ in 0..4 {
            let reach = Vec3::new(radius, half + radius, radius);
            let mut moved = false;
            let mut found = Vec::new();
            self.each_near(pos - reach, pos + reach, |t| found.push(*t));
            for t in found {
                let (s0, s1) = (pos - Vec3::Y * half, pos + Vec3::Y * half);
                let Some((normal, depth)) = capsule_tri(s0, s1, radius, &t) else { continue };
                if depth > 1e-5 {
                    pos += normal * depth;
                    moved = true;
                }
                if !normals.iter().any(|n| n.dot(normal) > 0.999) {
                    normals.push(normal);
                }
            }
            if !moved {
                break;
            }
        }
        Contact { pos, normals }
    }

    /// Where an upright capsule that goes from `from` to `to` ends, pushed out of the walls, and what it
    /// met on the way.
    ///
    /// [`Collider::push_out`] only sees a wall within the body's radius of its middle line, and a middle
    /// line that has passed a wall's face is pushed on inward. So a body that would go further than half
    /// its radius in one go, with a wall anywhere near its path, goes in parts no longer than that and is
    /// pushed out after each. Any other body is pushed out where it ends.
    pub fn travel(&self, from: Vec3, to: Vec3, half: f32, radius: f32) -> Contact {
        let parts = ((to - from).length() / (0.5 * radius)).ceil().clamp(1.0, MAX_PARTS) as usize;
        let reach = Vec3::new(radius, half + radius, radius);
        if parts <= 1 || !self.any_near(from.min(to) - reach, from.max(to) + reach) {
            return self.push_out(to, half, radius);
        }
        let step = (to - from) / parts as f32;
        let mut pos = from;
        let mut normals: Vec<Vec3> = Vec::new();
        for _ in 0..parts {
            let c = self.push_out(pos + step, half, radius);
            pos = c.pos;
            for normal in c.normals {
                if !normals.iter().any(|n| n.dot(normal) > 0.999) {
                    normals.push(normal);
                }
            }
        }
        Contact { pos, normals }
    }

    /// Whether any triangle's box overlaps `min..max`.
    fn any_near(&self, min: Vec3, max: Vec3) -> bool {
        let mut any = false;
        self.each_near(min, max, |_| any = true);
        any
    }
}

/// The most parts [`Collider::travel`] cuts one move into. A walking speck needs a dozen; this only bounds
/// the work for a body thinner than any that plays.
const MAX_PARTS: f32 = 512.0;

impl Collider {
    /// Whether an upright capsule at `center` overlaps any wall.
    pub fn touches(&self, center: Vec3, half: f32, radius: f32) -> bool {
        let reach = Vec3::new(radius, half + radius, radius);
        let (s0, s1) = (center - Vec3::Y * half, center + Vec3::Y * half);
        let mut hit = false;
        self.each_near(center - reach, center + reach, |t| {
            hit |= capsule_tri(s0, s1, radius, t).is_some_and(|(_, depth)| depth > 1e-4);
        });
        hit
    }

    /// Where a character that appears at `center` ends up: if it overlaps a floor, it's
    /// lifted until it stands on it (as Unreal does when it spawns one inside the
    /// map), by up to its own height; otherwise pushed out of what it touches.
    pub fn lift_out(&self, center: Vec3, half: f32, radius: f32) -> Vec3 {
        if !self.touches(center, half, radius) {
            return center;
        }
        let tall = 2.0 * (half + radius);
        let step = (tall / 32.0).max(0.01);
        let mut lift = step;
        while lift <= tall + step / 2.0 {
            if !self.touches(center + Vec3::Y * lift, half, radius) {
                // Settle down onto the floor.
                let (mut lo, mut hi) = (lift - step, lift);
                for _ in 0..12 {
                    let mid = (lo + hi) / 2.0;
                    if self.touches(center + Vec3::Y * mid, half, radius) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                return center + Vec3::Y * hi;
            }
            lift += step;
        }
        self.push_out(center, half, radius).pos
    }

    /// Where a walking character that appears at `center` comes to rest, as Unreal's
    /// walking movement puts it: lifted onto a floor it overlaps, or pulled down onto
    /// one whose top is at most `step_up` (plus [`FLOOR_REACH`]) below its feet; on a
    /// floor it then stands [`FLOOR_GAP`] above it. Otherwise it stays where it is.
    pub fn stand(&self, center: Vec3, half: f32, radius: f32, step_up: f32) -> Vec3 {
        if self.touches(center, half, radius) {
            let lifted = self.lift_out(center, half, radius);
            return if lifted.y > center.y { lifted + Vec3::Y * FLOOR_GAP } else { lifted };
        }
        let reach = step_up.max(0.0) + FLOOR_REACH;
        // Whether it touches anything once it has dropped this far. A body at least as
        // tall as its reach is tested where it ends up. A shorter one can be wholly under
        // a floor's surface there (the walls are surfaces, with nothing inside them), so
        // everything it passes through on the way down is tested: one longer capsule.
        let short = 2.0 * (half + radius) < reach;
        let touches = |drop: f32| {
            if short {
                self.touches(center - Vec3::Y * (drop / 2.0), half + drop / 2.0, radius)
            } else {
                self.touches(center - Vec3::Y * drop, half, radius)
            }
        };
        if !touches(reach) {
            return center;
        }
        // How far it can drop before it touches.
        let (mut free, mut hit) = (0.0, reach);
        for _ in 0..16 {
            let mid = (free + hit) / 2.0;
            if touches(mid) {
                hit = mid;
            } else {
                free = mid;
            }
        }
        center - Vec3::Y * (free - FLOOR_GAP)
    }
}

/// How high above its floor a walking character stands, in meters: Unreal keeps it
/// 1.9 to 2.4 units up, and KovaaK's test maps measure about 2.
pub const FLOOR_GAP: f32 = 0.02;

/// How far below its feet a walking character still finds a floor beyond its step-up
/// height, in meters: Unreal's `MAX_FLOOR_DIST`, 2.4 units.
pub const FLOOR_REACH: f32 = 0.024;

fn slab(origin: Vec3, inv: Vec3, min: Vec3, max: Vec3, max_t: f32) -> bool {
    let t1 = (min - origin) * inv;
    let t2 = (max - origin) * inv;
    let lo = t1.min(t2).max_element().max(0.0);
    let hi = t1.max(t2).min_element().min(max_t);
    // NaN (a ray along a box face) counts as a hit, to be safe.
    lo <= hi || lo.is_nan() || hi.is_nan()
}

/// Möller–Trumbore, both sides.
fn ray_tri(origin: Vec3, dir: Vec3, t: &Tri) -> Option<f32> {
    let (e1, e2) = (t.b - t.a, t.c - t.a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - t.a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let d = e2.dot(q) * inv;
    (d > 1e-4).then_some(d)
}

/// The point of triangle `a b c` nearest to `p` (Ericson, Real-Time Collision
/// Detection, 5.1.5).
fn closest_on_tri(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

fn closest_on_segment(p: Vec3, s0: Vec3, s1: Vec3) -> Vec3 {
    let d = s1 - s0;
    let len2 = d.length_squared();
    if len2 < 1e-12 {
        return s0;
    }
    s0 + d * ((p - s0).dot(d) / len2).clamp(0.0, 1.0)
}

/// How a capsule around segment `s0 s1` overlaps a triangle: the direction to push
/// it and how far. Uses the segment point nearest the triangle (where the segment
/// meets its plane, or else its nearest end).
fn capsule_tri(s0: Vec3, s1: Vec3, radius: f32, t: &Tri) -> Option<(Vec3, f32)> {
    let d = s1 - s0;
    let denom = t.normal.dot(d);
    let reference = if denom.abs() > 1e-6 {
        let k = (t.normal.dot(t.a - s0) / denom).clamp(0.0, 1.0);
        closest_on_tri(s0 + d * k, t.a, t.b, t.c)
    } else {
        closest_on_tri(s0, t.a, t.b, t.c)
    };
    let on_seg = closest_on_segment(reference, s0, s1);
    let on_tri = closest_on_tri(on_seg, t.a, t.b, t.c);
    let away = on_seg - on_tri;
    let dist = away.length();
    if dist >= radius {
        return None;
    }
    let normal = if dist > 1e-6 {
        away / dist
    } else {
        // Exactly on the surface: push to the side the segment's middle is on.
        let mid = (s0 + s1) / 2.0;
        if t.normal.dot(mid - t.a) >= 0.0 { t.normal } else { -t.normal }
    };
    Some((normal, radius - dist))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor at y = -1, 20 m across, and a wall at x = 3.
    fn room() -> Collider {
        let floor = WorldMesh {
            positions: vec![[-10.0, -1.0, -10.0], [10.0, -1.0, -10.0], [10.0, -1.0, 10.0], [-10.0, -1.0, 10.0]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..WorldMesh::default()
        };
        let wall = WorldMesh {
            positions: vec![[3.0, -1.0, -10.0], [3.0, 5.0, -10.0], [3.0, 5.0, 10.0], [3.0, -1.0, 10.0]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..WorldMesh::default()
        };
        let mut glass = WorldMesh { decor: true, ..wall.clone() };
        for p in &mut glass.positions {
            p[0] = -3.0;
        }
        let mut clip = WorldMesh { clip: true, ..wall.clone() };
        for p in &mut clip.positions {
            p[2] = if p[2] < 0.0 { 2.0 } else { 3.0 };
            p[0] = if p[1] < 0.0 { -1.0 } else { 1.0 };
        }
        Collider::new(&[floor, wall, glass, clip])
    }

    #[test]
    fn rays_stop_at_walls_from_either_side() {
        let c = room();
        let d = c.ray(Vec3::ZERO, Vec3::X, 100.0).unwrap();
        assert!((d - 3.0).abs() < 1e-4, "{d}");
        assert!((c.ray(Vec3::new(5.0, 0.0, 0.0), Vec3::NEG_X, 100.0).unwrap() - 2.0).abs() < 1e-4);
        assert!((c.ray(Vec3::ZERO, Vec3::NEG_Y, 100.0).unwrap() - 1.0).abs() < 1e-4);
        assert_eq!(c.ray(Vec3::ZERO, Vec3::X, 2.0), None, "the wall is past the ray's end");
        assert_eq!(c.ray(Vec3::ZERO, Vec3::NEG_X, 100.0), None, "decorations don't stop shots");
        assert_eq!(c.ray(Vec3::ZERO, Vec3::Y, 100.0), None);
        assert_eq!(c.ray(Vec3::ZERO, Vec3::Z, 100.0), None, "nor do clip walls");
    }

    /// Bug: a body that went further than its own radius in one go could pass a wall's face, and was then
    /// pushed on into the wall (a speck of a body under a head walked through walls and fell out of the
    /// map). `travel` takes such a move in parts.
    #[test]
    fn a_thin_or_fast_body_cannot_step_over_a_face() {
        let c = room();
        // A needle 6 mm wide that would end 4 cm past the wall at x = 3 stops against it.
        let (half, radius) = (1.4, 0.003);
        let stood = -1.0 + half + radius;
        let end = c.travel(Vec3::new(2.99, stood, 0.0), Vec3::new(3.04, stood, 0.0), half, radius);
        assert!((end.pos.x - (3.0 - radius)).abs() < 1e-4, "{}", end.pos);
        assert!(end.normals.iter().any(|n| n.x < -0.99), "it met the wall: {:?}", end.normals);
        // Pushed out only where it would have ended, it is on the wall's far side.
        assert!(c.push_out(Vec3::new(3.04, stood, 0.0), half, radius).pos.x > 3.0);
        // A small ball that falls 17 cm in one go lands on the floor at y = -1.
        let radius = 0.05;
        let end = c.travel(Vec3::new(0.0, -0.9, 0.0), Vec3::new(0.0, -1.07, 0.0), 0.0, radius);
        assert!((end.pos.y - (-1.0 + radius)).abs() < 1e-4, "{}", end.pos);
        assert!(end.normals.iter().any(|n| n.y > 0.99), "it met the floor: {:?}", end.normals);
        assert!(c.push_out(Vec3::new(0.0, -1.07, 0.0), 0.0, radius).pos.y < -1.0);
    }

    /// A move that is short for the body, or has no wall near it, is one push out of the walls where it
    /// ends, bit for bit: ordinary bots move as they did before `travel`.
    #[test]
    fn an_ordinary_move_is_one_push_out_where_it_ends() {
        let c = room();
        let (half, radius) = (0.6, 0.4);
        for (from, to) in [
            (Vec3::new(2.55, 0.0, 0.3), Vec3::new(2.7, 0.0, 0.3)),
            (Vec3::new(0.1, 0.05, 0.3), Vec3::new(0.13, -0.11, 0.31)),
            (Vec3::new(2.55, 0.02, 0.3), Vec3::new(2.69, -0.05, 0.37)),
        ] {
            let (went, pushed) = (c.travel(from, to, half, radius), c.push_out(to, half, radius));
            assert_eq!(went.pos, pushed.pos, "{from} to {to}");
            assert_eq!(went.normals, pushed.normals, "{from} to {to}");
            assert!(!pushed.normals.is_empty(), "the move ends against something: {from} to {to}");
        }
        // A needle that flies 3.7 m through open air goes in one piece too.
        let (from, to) = (Vec3::new(-2.0, 30.0, 0.3), Vec3::new(-1.1, 26.4, 0.9));
        assert_eq!(c.travel(from, to, 1.4, 0.003).pos, to);
    }

    #[test]
    fn characters_that_appear_in_the_floor_are_lifted_onto_it() {
        let c = room();
        // A capsule (2 m tall) half sunk into the floor at y = -1 stands on it...
        let p = c.lift_out(Vec3::new(0.0, -1.2, 0.0), 0.6, 0.4);
        assert!((p.y - 0.0).abs() < 0.01, "{p}");
        // ...even when its middle is at the floor's surface, where pushing it out
        // the nearest way could drop it through.
        let p = c.lift_out(Vec3::new(0.0, -1.0, 0.0), 0.6, 0.4);
        assert!((p.y - 0.0).abs() < 0.01, "{p}");
        // One in the open stays put, and one against a wall is pushed away from it.
        assert_eq!(c.lift_out(Vec3::new(0.0, 1.0, 0.0), 0.6, 0.4), Vec3::new(0.0, 1.0, 0.0));
        assert!((c.lift_out(Vec3::new(2.8, 2.0, 0.0), 0.0, 0.4).x - 2.6).abs() < 1e-3);
    }

    /// KovaaK's (Unreal) walking characters stand 2 units above their floor, and
    /// one whose feet are within its step-up height of a floor is pulled down onto it
    /// (measured with KovaaK's test maps).
    #[test]
    fn walking_characters_stand_on_floors_within_their_step() {
        let c = room();
        let near = |p: Vec3, y: f32| assert!((p.y - y).abs() < 1e-3, "{p} not at y {y}");
        // A 2 m capsule (middle 1 m above its feet) with its feet 0.7 m above the
        // floor at y = -1: a 0.75 m step pulls it down, a 0.3 m one doesn't.
        near(c.stand(Vec3::new(0.0, 0.7, 0.0), 0.6, 0.4, 0.75), 0.02);
        near(c.stand(Vec3::new(0.0, 0.7, 0.0), 0.6, 0.4, 0.3), 0.7);
        // 2.4 units of reach past the step: feet 2 cm up are pulled down even with no step.
        near(c.stand(Vec3::new(0.0, 0.02, 0.0), 0.6, 0.4, 0.0), 0.02);
        // Sunk into the floor, it's lifted onto it and stands 2 cm up too.
        near(c.stand(Vec3::new(0.0, -1.2, 0.0), 0.6, 0.4, 0.0), 0.02);
        // Against a wall it's only pushed away from it.
        let p = c.stand(Vec3::new(2.8, 2.0, 0.0), 0.0, 0.4, 0.0);
        assert!((p.x - 2.6).abs() < 1e-3 && (p.y - 2.0).abs() < 1e-3, "{p}");
    }

    /// Bug: a body shorter than its reach was only tested for a floor at the far end of
    /// its drop, where the whole of it had already passed through the floor's surface. So
    /// a speck of a character just above a floor stayed where it appeared, instead of
    /// standing 2 units above the floor as every other character does.
    #[test]
    fn a_body_shorter_than_its_reach_still_finds_the_floor_below_it() {
        let c = room();
        let near = |p: Vec3, y: f32| assert!((p.y - y).abs() < 1e-3, "{p} not at y {y}");
        // A 1 cm ball with its bottom half a millimeter above the floor at y = -1 and no
        // step: 2.4 cm of reach is more than its 2 cm. It ends 2 cm above the floor.
        near(c.stand(Vec3::new(0.0, -0.9895, 0.0), 0.0, 0.01, 0.0), -0.97);
        // A 5 cm ball with a 45 cm step, its bottom 10 cm above the floor, is pulled down.
        near(c.stand(Vec3::new(0.0, -0.85, 0.0), 0.0, 0.05, 0.45), -0.93);
        // A short capsule too: 12 cm tall, its feet 30 cm up.
        near(c.stand(Vec3::new(0.0, -0.64, 0.0), 0.02, 0.04, 0.45), -0.92);
        // With the floor past its reach it stays where it is, to the last bit.
        let high = Vec3::new(0.0, -0.4, 0.0);
        assert_eq!(c.stand(high, 0.0, 0.05, 0.45), high);
        // Past the floor's edge there is nothing to stand on.
        let off = Vec3::new(10.5, -0.85, 0.0);
        assert_eq!(c.stand(off, 0.0, 0.05, 0.45), off);
    }

    /// Bug: a small body over a thick floor (a top face and a bottom face, as a map's
    /// boxes have) was stood inside the floor. The far end of its drop touched the bottom
    /// face, and the search for how far it could drop then passed the top face, which the
    /// body is too short to still be touching halfway down.
    #[test]
    fn a_small_body_stands_on_top_of_a_thick_floor() {
        let face = |y: f32| WorldMesh {
            positions: vec![[-10.0, y, -10.0], [10.0, y, -10.0], [10.0, y, 10.0], [-10.0, y, 10.0]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..WorldMesh::default()
        };
        // 40 cm thick, its top at y = -1.
        let c = Collider::new(&[face(-1.0), face(-1.4)]);
        // A 5 cm ball with a 45 cm step, its bottom 5 cm above the top face.
        let stood = c.stand(Vec3::new(0.0, -0.9, 0.0), 0.0, 0.05, 0.45);
        assert!((stood.y - -0.93).abs() < 1e-3, "{stood} isn't on the top face");
    }

    #[test]
    fn bots_bump_into_clip_walls() {
        let clip = WorldMesh {
            positions: vec![[-5.0, -5.0, 2.0], [5.0, -5.0, 2.0], [5.0, 5.0, 2.0], [-5.0, 5.0, 2.0]],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            clip: true,
            ..WorldMesh::default()
        };
        let bots = Collider::for_bots(std::slice::from_ref(&clip));
        assert!((bots.push_out(Vec3::new(0.0, 0.0, 1.8), 0.0, 0.5).pos.z - 1.5).abs() < 1e-4);
        assert!(Collider::new(&[clip]).is_empty());
    }

    #[test]
    fn spheres_and_capsules_are_pushed_out_of_walls() {
        let c = room();
        // A ball sunk into the floor comes up to rest on it.
        let hit = c.push_out(Vec3::new(0.0, -0.8, 0.0), 0.0, 0.5);
        assert!((hit.pos.y - -0.5).abs() < 1e-4, "{:?}", hit.pos);
        assert!(hit.normals.iter().any(|n| n.y > 0.99));
        // A capsule in the corner leaves the wall and the floor.
        let hit = c.push_out(Vec3::new(2.8, -0.2, 0.0), 0.5, 0.4);
        assert!((hit.pos.x - 2.6).abs() < 1e-4 && (hit.pos.y - -0.1).abs() < 1e-4, "{:?}", hit.pos);
        assert_eq!(hit.normals.len(), 2);
        // In the open, nothing changes.
        let free = c.push_out(Vec3::new(0.0, 2.0, 0.0), 0.5, 0.4);
        assert_eq!((free.pos, free.normals.len()), (Vec3::new(0.0, 2.0, 0.0), 0));
    }

    #[test]
    fn many_triangles_are_searched_through_the_tree() {
        // A row of 200 small floor tiles: the tree has to find the one under the ball.
        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        for i in 0..200u32 {
            let x = i as f32;
            positions.extend([[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x + 1.0, 0.0, 1.0], [x, 0.0, 1.0]]);
            triangles.extend([[4 * i, 4 * i + 1, 4 * i + 2], [4 * i, 4 * i + 2, 4 * i + 3]]);
        }
        let c = Collider::new(&[WorldMesh { positions, triangles, ..WorldMesh::default() }]);
        let hit = c.push_out(Vec3::new(150.5, 0.1, 0.5), 0.0, 0.3);
        assert!((hit.pos.y - 0.3).abs() < 1e-4);
        assert!((c.ray(Vec3::new(77.5, 5.0, 0.5), Vec3::NEG_Y, 10.0).unwrap() - 5.0).abs() < 1e-4);
    }
}
