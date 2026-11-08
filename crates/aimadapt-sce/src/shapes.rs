//! Triangles for KovaaK's editor primitives other than `Cube`. KovaaK's own meshes
//! aren't in the files, so these are our own versions, fitted to how maps use them
//! and to how those maps look in KovaaK's.
//!
//! Each shape fills a 100-unit box at scale 1, in Unreal axes (X forward, Y right,
//! Z up). Round shapes stand on their location and are centered on it sideways: a
//! `Tube` arena has the player at its middle, and cylinders and tubes stand on
//! floors at their location. Angular ones are placed from a corner, like a `Cube`:
//! four `Concave`s at a square room's corners make it round (VT ww5t's map), and a
//! `Ramp` turned to limit the view has its side on the limit (VT DriftTS's).
//!
//! The ramps' slopes run across them (Y to Z), high at the corner: so do the letters
//! built from them on Clicking Static 5's wall, and VT ControlTS's floor, a `WedgeB`,
//! rises toward its corner in KovaaK's. A `DoubleRamp` is the exception: centered on
//! its peak, which runs along Y, as the middle of an M on that wall needs.
//!
//! Shapes these maps pin down are solid. The rest are drawn from their names so the
//! map looks about right, and shots and bots pass through them.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A shape's vertices and triangles, and whether it's solid.
pub struct Shape {
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub solid: bool,
}

const R: f32 = 50.0;
const H: f32 = 100.0;
const SEGMENTS: usize = 32;

/// Which local axes a prism's profile is drawn on; it's pushed along the third,
/// from 0 to 100.
#[derive(Clone, Copy)]
enum Plane {
    /// (Y, Z), pushed along X.
    Across,
    /// (X, Z), pushed along Y.
    Side,
    /// (X, Y), pushed up Z.
    Plan,
}

/// The shape for a primitive's name, if there is one.
pub fn primitive(mesh: &str) -> Option<Shape> {
    let m = mesh.to_ascii_lowercase();
    Some(match m.as_str() {
        "cylinder" => lathe(&[(0.0, 0.0), (R, 0.0), (R, H), (0.0, H)], TAU, true),
        // A tube is a cylinder's wall; how thick isn't known, so a tenth of its radius.
        "tube" => lathe(&[(R * 0.9, 0.0), (R, 0.0), (R, H), (R * 0.9, H), (R * 0.9, 0.0)], TAU, true),
        "pipe" => lathe(&[(R * 0.8, 0.0), (R, 0.0), (R, H), (R * 0.8, H), (R * 0.8, 0.0)], TAU, false),
        "cone" => lathe(&[(0.0, 0.0), (R, 0.0), (0.0, H)], TAU, false),
        "sphere" => lathe(&arc(0.0, H / 2.0, R, -FRAC_PI_2, FRAC_PI_2, 12), TAU, false),
        "dome" => lathe(&arc(0.0, 0.0, R, 0.0, FRAC_PI_2, 8), TAU, false),
        "torus" => {
            let mut ring = arc(R * 0.75, H / 2.0, R * 0.25, -PI, PI, 12);
            ring.push(ring[0]);
            lathe(&ring, TAU, false)
        }
        "widecapsule" | "narrowcapsule" => {
            // Lying along X, rounded at both ends.
            let r = if m == "widecapsule" { R } else { R * 0.5 };
            let mut s = lathe(&capsule_profile(r, R), TAU, false);
            for v in &mut s.vertices {
                *v = [v[2] - R, v[1], v[0] + H / 2.0];
            }
            s
        }
        // The ramps: a profile across the shape, high at its corner.
        "ramp" | "wedgeb" => prism(&[(0.0, 0.0), (H, 0.0), (0.0, H)], Plane::Across, true),
        "halframp" => prism(&[(0.0, 0.0), (H, 0.0), (0.0, H / 2.0)], Plane::Across, false),
        "wedgea" => prism(&[(0.0, 0.0), (H, 0.0), (H, H)], Plane::Across, false),
        // The slope curves in (a quarter circle around the far corner) or bulges out
        // (one around the near corner).
        "rampconcave" => prism(&scooped(), Plane::Across, true),
        "rampconvex" => {
            let mut p = vec![(0.0, 0.0)];
            p.extend(arc(0.0, 0.0, H, 0.0, FRAC_PI_2, 12));
            prism(&p, Plane::Across, true)
        }
        // The same scooped block standing up, curved as seen from above.
        "concave" => prism(&scooped(), Plane::Plan, true),
        // Two ramps back to back, peaking over its location.
        "doubleramp" => {
            let mut s = prism(&[(0.0, 0.0), (H, 0.0), (R, H)], Plane::Side, true);
            for v in &mut s.vertices {
                v[0] -= R;
            }
            s
        }
        // Four steps down from the corner, like the ramps.
        "stairsb" => {
            let mut p = vec![(0.0, 0.0), (H, 0.0)];
            for k in (1..4).rev() {
                let (y, z) = (H * k as f32 / 4.0, H * (4 - k) as f32 / 4.0);
                p.push((y + H / 4.0, z));
                p.push((y, z));
            }
            p.push((H / 4.0, H));
            p.push((0.0, H));
            prism(&p, Plane::Across, false)
        }
        "quadpyramid" => pyramid(&[(-R, -R), (R, -R), (R, R), (-R, R)], 0.0),
        "tripyramid" => pyramid(&[(-R, -R), (R, -R), (0.0, R)], 0.0),
        "truncatedpyramid" => pyramid(&[(-R, -R), (R, -R), (R, R), (-R, R)], 0.5),
        _ => return None,
    })
}

/// A square with a quarter circle around its far corner taken out, from its near
/// corner: (0, 0), along the first axis, around the curve, and back.
fn scooped() -> Vec<(f32, f32)> {
    let mut p = vec![(0.0, 0.0)];
    p.extend(arc(H, H, H, -FRAC_PI_2, -PI, 12));
    p
}

/// Points along a circle around (cx, cz) from angle `a0` to `a1` (radians, 0 along
/// +x, counterclockwise toward +z), as (x, z).
fn arc(cx: f32, cz: f32, r: f32, a0: f32, a1: f32, steps: usize) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            (cx + r * a.cos(), cz + r * a.sin())
        })
        .collect()
}

/// A capsule's outline for [`lathe`]: radius `r`, from z = 0 to `2 * half`.
fn capsule_profile(r: f32, half: f32) -> Vec<(f32, f32)> {
    let r = r.min(half);
    let mut p = arc(0.0, r, r, -FRAC_PI_2, 0.0, 6);
    p.extend(arc(0.0, 2.0 * half - r, r, 0.0, FRAC_PI_2, 6));
    p
}

/// Spins a profile of (distance from the axis, height) around the up axis.
fn lathe(profile: &[(f32, f32)], sweep: f32, solid: bool) -> Shape {
    let n = profile.len() as u32;
    let mut vertices = Vec::new();
    for s in 0..=SEGMENTS {
        let a = sweep * s as f32 / SEGMENTS as f32;
        for (r, z) in profile {
            vertices.push([r * a.cos(), r * a.sin(), *z]);
        }
    }
    let mut triangles = Vec::new();
    for s in 0..SEGMENTS as u32 {
        for k in 0..n - 1 {
            let (a, b) = (s * n + k, (s + 1) * n + k);
            triangles.push([a, b, b + 1]);
            triangles.push([a, b + 1, a + 1]);
        }
    }
    Shape { vertices, triangles, solid }
}

/// A profile on `plane`, pushed from 0 to 100 along the third axis. Its ends are
/// fans of triangles from the first point, which has to see the whole profile.
fn prism(profile: &[(f32, f32)], plane: Plane, solid: bool) -> Shape {
    let n = profile.len() as u32;
    let place = |(u, v): (f32, f32), w: f32| match plane {
        Plane::Across => [w, u, v],
        Plane::Side => [u, w, v],
        Plane::Plan => [u, v, w],
    };
    let mut vertices: Vec<[f32; 3]> = Vec::new();
    for w in [0.0, H] {
        for p in profile {
            vertices.push(place(*p, w));
        }
    }
    let mut triangles = Vec::new();
    for k in 1..n - 1 {
        triangles.push([0, k, k + 1]);
        triangles.push([n, n + k + 1, n + k]);
    }
    for k in 0..n {
        let j = (k + 1) % n;
        triangles.push([k, j, n + j]);
        triangles.push([k, n + j, n + k]);
    }
    Shape { vertices, triangles, solid }
}

/// A base polygon (x, y) at z = 0 rising to a point, or to a smaller copy of the
/// base (`top` of its size) for a truncated one.
fn pyramid(base: &[(f32, f32)], top: f32) -> Shape {
    let n = base.len() as u32;
    let mut vertices: Vec<[f32; 3]> = base.iter().map(|(x, y)| [*x, *y, 0.0]).collect();
    let mut triangles = Vec::new();
    for k in 1..n - 1 {
        triangles.push([0, k + 1, k]);
    }
    if top > 0.0 {
        vertices.extend(base.iter().map(|(x, y)| [x * top, y * top, H]));
        for k in 1..n - 1 {
            triangles.push([n, n + k, n + k + 1]);
        }
        for k in 0..n {
            let j = (k + 1) % n;
            triangles.push([k, j, n + j]);
            triangles.push([k, n + j, n + k]);
        }
    } else {
        vertices.push([0.0, 0.0, H]);
        for k in 0..n {
            triangles.push([k, (k + 1) % n, n]);
        }
    }
    Shape { vertices, triangles, solid: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUND: [&str; 12] = [
        "Cylinder",
        "Tube",
        "Pipe",
        "Cone",
        "Sphere",
        "Dome",
        "Torus",
        "WideCapsule",
        "NarrowCapsule",
        "QuadPyramid",
        "TriPyramid",
        "TruncatedPyramid",
    ];
    const ANGULAR: [&str; 9] =
        ["Ramp", "HalfRamp", "WedgeA", "WedgeB", "Concave", "RampConcave", "RampConvex", "StairsB", "DoubleRamp"];

    fn bounds(s: &Shape, axis: usize) -> (f32, f32) {
        let lo = s.vertices.iter().map(|v| v[axis]).fold(f32::MAX, f32::min);
        let hi = s.vertices.iter().map(|v| v[axis]).fold(f32::MIN, f32::max);
        (lo, hi)
    }

    #[test]
    fn round_shapes_stand_centered_and_angular_ones_start_at_a_corner() {
        for name in ROUND.iter().chain(&ANGULAR) {
            let s = primitive(name).unwrap_or_else(|| panic!("{name}"));
            let n = s.vertices.len() as u32;
            assert!(s.triangles.iter().flatten().all(|i| *i < n), "{name}");
            for k in 0..3 {
                let (lo, hi) = bounds(&s, k);
                let centered = ROUND.contains(name) || (*name == "DoubleRamp" && k == 0);
                let (want_lo, want_hi) = if k == 2 || !centered { (0.0, H) } else { (-R, R) };
                assert!(lo >= want_lo - 0.01 && hi <= want_hi + 0.01, "{name} axis {k}: {lo}..{hi}");
            }
        }
        assert!(primitive("Teapot").is_none());
    }

    #[test]
    fn shapes_the_maps_pin_down_are_solid() {
        for name in ["Cylinder", "Tube", "Ramp", "WedgeB", "Concave", "RampConcave", "RampConvex", "DoubleRamp"] {
            assert!(primitive(name).unwrap().solid, "{name}");
        }
        for name in ["Sphere", "HalfRamp", "WedgeA", "StairsB", "QuadPyramid"] {
            assert!(!primitive(name).unwrap().solid, "{name}");
        }
    }

    /// A ramp's slope runs across it (Y to Z), high at its corner: its end is the
    /// triangle (0, 0), (100, 0), (0, 100) and nothing is at the far top edge.
    #[test]
    fn ramps_slope_across_from_their_corner() {
        for name in ["Ramp", "WedgeB"] {
            let s = primitive(name).unwrap();
            for c in [[0.0, 0.0, 0.0], [0.0, H, 0.0], [0.0, 0.0, H], [H, 0.0, H]] {
                assert!(s.vertices.contains(&c), "{name} lacks {c:?}");
            }
            assert!(s.vertices.iter().all(|v| v[1] + v[2] <= H + 0.01), "{name}");
        }
    }

    /// The scooped shapes keep away from their far corner by its width: around Z for a
    /// `Concave`, seen from above, and around X for a `RampConcave`. A `RampConvex`
    /// stays within that of its near corner.
    #[test]
    fn curved_ramps_curve_around_a_corner() {
        let far = |s: &Shape, a: usize, b: usize| s.vertices.iter().all(|v| (v[a] - H).hypot(v[b] - H) >= H - 0.01);
        assert!(far(&primitive("Concave").unwrap(), 0, 1));
        assert!(far(&primitive("RampConcave").unwrap(), 1, 2));
        assert!(primitive("RampConvex").unwrap().vertices.iter().all(|v| v[1].hypot(v[2]) <= H + 0.01));
    }

    #[test]
    fn a_double_ramp_peaks_over_its_location() {
        let s = primitive("DoubleRamp").unwrap();
        let top = s.vertices.iter().filter(|v| v[2] > H - 0.01);
        assert!(top.into_iter().all(|v| v[0].abs() < 0.01));
    }
}
