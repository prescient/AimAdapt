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
}

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
}

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
