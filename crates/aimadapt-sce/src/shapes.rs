//! Triangles for KovaaK's editor primitives other than `Cube`, made up from their
//! names: KovaaK's own meshes aren't in the files, so these are our own versions.
//!
//! Each shape fills a 100-unit box at scale 1, in Unreal axes (X forward, Y right,
//! Z up). Unlike a `Cube`, which is placed from its lowest corner, these stand on
//! their location and are centered on it sideways: a `Tube` arena has the player
//! at its middle, and cylinders and tubes stand on floors at their location. Only
//! cylinders and tubes, the shapes that evidence pins down, are solid; the rest
//! are drawn so the map looks right, and shots and bots pass through them.

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
        // Prisms: a side profile (x forward, z up) pushed across the width.
        "ramp" => prism(&[(-R, 0.0), (R, 0.0), (R, H)]),
        "halframp" => prism(&[(-R, 0.0), (R, 0.0), (R, H / 2.0)]),
        "doubleramp" => prism(&[(-R, 0.0), (R, 0.0), (0.0, H)]),
        "wedgea" => prism(&[(-R, 0.0), (R, 0.0), (R, H), (-R, H / 2.0)]),
        "wedgeb" => prism(&[(-R, 0.0), (R, 0.0), (-R, H)]),
        // A block with a quarter circle scooped out of its top, like a skate ramp. The
        // profile starts at the corner that sees all of it, so a fan of triangles
        // from there fills it.
        "concave" | "rampconcave" => {
            let mut p = vec![(R, 0.0), (R, H)];
            p.extend(arc(-R, H, 2.0 * R, 0.0, -FRAC_PI_2, 12).into_iter().skip(1));
            prism(&p)
        }
        // A quarter of a disk: the slope bulges out.
        "rampconvex" => {
            let mut p = vec![(-R, 0.0)];
            p.extend(arc(-R, 0.0, 2.0 * R, 0.0, FRAC_PI_2, 12));
            prism(&p)
        }
        // Four steps up toward +X.
        "stairsb" => {
            let mut p = vec![(R, 0.0), (R, H)];
            for k in 1..4 {
                let x = R - k as f32 * R / 2.0;
                p.push((x, H * (4 - k + 1) as f32 / 4.0));
                p.push((x, H * (4 - k) as f32 / 4.0));
            }
            p.push((-R, H / 4.0));
            p.push((-R, 0.0));
            prism(&p)
        }
        "quadpyramid" => pyramid(&[(-R, -R), (R, -R), (R, R), (-R, R)], 0.0),
        "tripyramid" => pyramid(&[(-R, -R), (R, -R), (0.0, R)], 0.0),
        "truncatedpyramid" => pyramid(&[(-R, -R), (R, -R), (R, R), (-R, R)], 0.5),
        _ => return None,
    })
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

/// A side profile (x, z) pushed across the full width. Its ends are fans of
/// triangles from the first point, which has to see the whole profile.
fn prism(profile: &[(f32, f32)]) -> Shape {
    let n = profile.len() as u32;
    let mut vertices: Vec<[f32; 3]> = Vec::new();
    for y in [-R, R] {
        for (x, z) in profile {
            vertices.push([*x, y, *z]);
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
    Shape { vertices, triangles, solid: false }
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

    #[test]
    fn shapes_fill_their_box_standing_on_their_location() {
        for name in [
            "Cylinder",
            "Tube",
            "Pipe",
            "Cone",
            "Sphere",
            "Dome",
            "Torus",
            "WideCapsule",
            "NarrowCapsule",
            "Ramp",
            "HalfRamp",
            "DoubleRamp",
            "WedgeA",
            "WedgeB",
            "Concave",
            "RampConcave",
            "RampConvex",
            "StairsB",
            "QuadPyramid",
            "TriPyramid",
            "TruncatedPyramid",
        ] {
            let s = primitive(name).unwrap_or_else(|| panic!("{name}"));
            let n = s.vertices.len() as u32;
            assert!(s.triangles.iter().flatten().all(|i| *i < n), "{name}");
            for k in 0..3 {
                let lo = s.vertices.iter().map(|v| v[k]).fold(f32::MAX, f32::min);
                let hi = s.vertices.iter().map(|v| v[k]).fold(f32::MIN, f32::max);
                let (want_lo, want_hi) = if k == 2 { (0.0, H) } else { (-R, R) };
                assert!(lo >= want_lo - 0.01 && hi <= want_hi + 0.01, "{name} axis {k}: {lo}..{hi}");
            }
        }
        assert!(primitive("Cylinder").unwrap().solid && primitive("Tube").unwrap().solid);
        assert!(!primitive("Ramp").unwrap().solid);
        assert!(primitive("Teapot").is_none());
    }
}
