//! Tests for solid props (plan K13a). Each expected number is worked out by hand from the measured table in the
//! module's note, in meters in the sim's axes: a point at Unreal `(x, y, z)` is `(y, z, -x) * map_scale / 100`.

use aimadapt_sim::collide::Collider;

use super::*;
use crate::map::{MapFormat, Placement};
use crate::scenario::Sce;

const SLAB: &str = include_str!("../../../tests/fixtures/AimAdapt Test - Slab Backdrop.sce");
const LANTERNS: &str = include_str!("../../../tests/fixtures/AimAdapt Test - Lantern Row.sce");

fn prop(name: &str, rotation: [f32; 3], scale: [f32; 3], collides: bool) -> Prop {
    Prop { name: name.into(), place: Placement { location: [0.0; 3], rotation, scale }, collides }
}

fn map_of(props: Vec<Prop>) -> Map {
    Map {
        format: MapFormat::Json,
        version: "1.0.0".into(),
        solids: Vec::new(),
        spawns: Vec::new(),
        waypoints: Vec::new(),
        props,
        volumes: Vec::new(),
        other: Vec::new(),
    }
}

fn built(props: Vec<Prop>, map_scale: f32) -> (Vec<WorldMesh>, Vec<WorldBox>) {
    build(&map_of(props), map_scale)
}

fn window() -> Prop {
    prop("Window", [0.0; 3], [1.0; 3], true)
}

/// The lowest and highest point of a mesh.
fn span(m: &WorldMesh) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &m.positions {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

fn assert_span(m: &WorldMesh, lo: [f32; 3], hi: [f32; 3]) {
    let (l, h) = span(m);
    for k in 0..3 {
        assert!((l[k] - lo[k]).abs() < 1e-4 && (h[k] - hi[k]).abs() < 1e-4, "axis {k}: {l:?} {h:?}");
    }
}

/// How far a ray along +x from `(-5, y, z)` goes before a prop stops it.
fn shot(meshes: &[WorldMesh], y: f32, z: f32) -> Option<f32> {
    Collider::new(meshes).ray(Vec3::new(-5.0, y, z), Vec3::X, 100.0)
}

#[test]
fn four_kinds_are_measured_and_names_are_exact() {
    let Some(Measured::Boxes { boxes, color }) = measured("Window") else { panic!("Window") };
    assert_eq!((boxes.len(), color), (1, "#46505f"));
    assert_eq!((boxes[0].lo, boxes[0].hi), ([-198.0, -4.25, 0.0], [198.0, 3.55, 302.0]));
    let Some(Measured::Boxes { boxes, color }) = measured("Tree") else { panic!("Tree") };
    assert_eq!((boxes.len(), color), (3, "#2f4a3a"));
    assert_eq!((boxes[0].lo, boxes[0].hi), ([-37.5, -37.5, 0.0], [37.5, 37.5, 130.0]));
    assert_eq!((boxes[1].lo, boxes[1].hi), ([-22.5, -22.5, 130.0], [22.5, 22.5, 190.0]));
    assert_eq!((boxes[2].lo, boxes[2].hi), ([-12.5, -12.5, 190.0], [12.5, 12.5, 222.0]));
    assert!(matches!(measured("Banner"), Some(Measured::NotSolid)));
    assert!(matches!(measured("BannerB"), Some(Measured::NotSolid)));
    for other in ["window", "Windows", "Crate", "banner", ""] {
        assert!(measured(other).is_none(), "{other}");
    }
}

#[test]
fn a_window_blocks_the_measured_rectangle() {
    let (meshes, walls) = built(vec![window()], 1.0);
    assert_eq!((meshes.len(), walls.len()), (1, 1));
    assert_span(&meshes[0], [-0.0425, 0.0, -1.98], [0.0355, 3.02, 1.98]);
    let hit = shot(&meshes, 1.0, 0.0).expect("the pane stops a shot");
    assert!((hit - 4.9575).abs() < 1e-3, "{hit}");
    // Just past each edge of the rectangle: the side (1.98), the top (3.02) and below the foot (0).
    assert_eq!(shot(&meshes, 1.0, 1.99), None);
    assert_eq!(shot(&meshes, 3.03, 0.0), None);
    assert_eq!(shot(&meshes, -0.01, 0.0), None);
}

#[test]
fn a_small_prop_on_a_big_map_scale_is_the_same_as_a_big_one_on_a_small_map_scale() {
    let (a, _) = built(vec![window()], 1.0);
    let (b, _) = built(vec![prop("Window", [0.0; 3], [0.25; 3], true)], 4.0);
    for (p, q) in a[0].positions.iter().zip(&b[0].positions) {
        assert!(Vec3::from_array(*p).distance(Vec3::from_array(*q)) < 1e-4, "{p:?} {q:?}");
    }
}

#[test]
fn rotations_turn_the_box_as_they_turn_a_brush() {
    // Yaw 90: the wide side lies along sim x, the thin side along sim z, the height stays.
    let (m, _) = built(vec![prop("Window", [0.0, 0.0, 90.0], [1.0; 3], true)], 1.0);
    assert_span(&m[0], [-1.98, 0.0, -0.0425], [1.98, 3.02, 0.0355]);
    // Roll 90: the pane lies flat. The 3.02 side runs along sim x, the thin side along sim y and the wide side
    // along sim z. The thin side's span is -0.0355 to 0.0425 here (the brief worked only its size).
    let (m, _) = built(vec![prop("Window", [90.0, 0.0, 0.0], [1.0; 3], true)], 1.0);
    assert_span(&m[0], [0.0, -0.0355, -1.98], [3.02, 0.0425, 1.98]);
}

#[test]
fn each_axis_has_its_own_scale() {
    let (m, _) = built(vec![prop("Window", [0.0; 3], [0.02, 25.0, 1.0], true)], 1.0);
    assert_span(&m[0], [-1.0625, 0.0, -0.0396], [0.8875, 3.02, 0.0396]);
}

#[test]
fn a_mirrored_window_still_stops_a_shot() {
    let (meshes, _) = built(vec![prop("Window", [0.0; 3], [-1.0, 1.0, 1.0], true)], 1.0);
    let hit = shot(&meshes, 1.0, 0.0).expect("a mirrored pane stops a shot");
    assert!((hit - 4.9575).abs() < 1e-3, "{hit}");
}

#[test]
fn a_tree_is_three_stacked_boxes() {
    let (meshes, walls) = built(vec![prop("Tree", [0.0; 3], [1.0; 3], true)], 1.0);
    assert_eq!((meshes.len(), walls.len()), (3, 3));
    // (height, face of the box there, its half-width): a ray just inside the width hits, just outside passes.
    for (height, face, half) in [(0.45, -0.375, 0.375), (1.75, -0.225, 0.225), (2.05, -0.125, 0.125)] {
        let hit = shot(&meshes, height, 0.0).unwrap_or_else(|| panic!("a shot at {height} hits"));
        assert!((hit - (5.0 + face)).abs() < 1e-3, "{height}: {hit}");
        assert_eq!(shot(&meshes, height, half + 0.005), None, "{height}");
        assert!(shot(&meshes, height, half - 0.005).is_some(), "{height}");
    }
    assert_eq!(shot(&meshes, 2.23, 0.0), None);
    // Where the boxes meet, and the two ends: (height, how far off the middle, whether a shot stops there).
    for (height, side, stops) in [
        (-0.005, 0.30, false),
        (0.005, 0.30, true),
        (1.295, 0.30, true),
        (1.305, 0.30, false),
        (1.305, 0.18, true),
        (1.895, 0.18, true),
        (1.905, 0.18, false),
        (1.905, 0.10, true),
        (2.215, 0.10, true),
        (2.225, 0.10, false),
    ] {
        assert_eq!(shot(&meshes, height, side).is_some(), stops, "height {height}, {side} off the middle");
    }
}

#[test]
fn props_that_are_not_solid_add_nothing() {
    for p in [
        prop("Banner", [0.0; 3], [1.0; 3], true),
        prop("BannerB", [0.0; 3], [1.0; 3], true),
        prop("Window", [0.0; 3], [1.0; 3], false),
        prop("Tree", [0.0; 3], [1.0; 3], false),
        prop("Window", [0.0; 3], [1.0, 0.0, 1.0], true),
        prop("Window", [0.0; 3], [1.0, 1.0, 1e-7], true),
        prop("Crate", [0.0; 3], [1.0; 3], true),
    ] {
        let (meshes, walls) = built(vec![p.clone()], 1.0);
        assert!(meshes.is_empty() && walls.is_empty(), "{} {:?}", p.name, p.place.scale);
    }
}

/// `embedded.rs` pairs a wall with its mesh through the eight corners in `cube` order.
#[test]
fn every_mesh_has_a_wall_with_the_same_corners_and_is_neither_decor_nor_clip() {
    let tilted = prop("Tree", [10.0, 20.0, 30.0], [1.5, 1.0, 2.0], true);
    let (meshes, walls) = built(vec![window(), tilted], 2.0);
    assert_eq!((meshes.len(), walls.len()), (4, 4));
    for (m, w) in meshes.iter().zip(&walls) {
        assert!(!m.decor && !m.clip);
        assert_eq!(m.positions.len(), 8);
        let (corner, edges) = (Vec3::from_array(w.corner), w.edges.map(Vec3::from_array));
        for (i, at) in [(0, corner), (1, corner + edges[0]), (2, corner + edges[1]), (4, corner + edges[2])] {
            assert!(Vec3::from_array(m.positions[i]).distance(at) < 1e-4, "corner {i}");
        }
    }
    assert_eq!((meshes[0].color.as_str(), meshes[1].color.as_str()), ("#46505f", "#2f4a3a"));
}

#[test]
fn a_ball_overlapping_the_window_touches_it_and_one_clear_of_it_does_not() {
    let (meshes, _) = built(vec![window()], 1.0);
    let bots = Collider::for_bots(&meshes);
    // A ball of radius 0.3: its middle 0.2575 from the face overlaps, 0.4575 doesn't.
    assert!(bots.touches(Vec3::new(-0.3, 1.0, 0.0), 0.0, 0.3));
    assert!(!bots.touches(Vec3::new(-0.5, 1.0, 0.0), 0.0, 0.3));
}

#[test]
fn only_colliding_props_without_a_measurement_are_unmeasured() {
    let map = map_of(vec![
        prop("Lantern", [0.0; 3], [1.0; 3], true),
        window(),
        prop("Banner", [0.0; 3], [1.0; 3], true),
        prop("Pebbles", [0.0; 3], [1.0; 3], false),
        prop("Lantern", [0.0; 3], [1.0; 3], true),
        prop("Crate", [0.0; 3], [1.0; 3], true),
    ]);
    assert_eq!(unmeasured(&map), vec!["Lantern", "Lantern", "Crate"]);
    assert!(unmeasured(&map_of(vec![window(), prop("Tree", [0.0; 3], [1.0; 3], true)])).is_empty());
}

fn note_list(props: Vec<Prop>) -> Vec<String> {
    let mut out = Vec::new();
    notes(&map_of(props), &mut out);
    out
}

#[test]
fn the_notes_name_each_kind_once_and_in_order() {
    let drawn = "some props are drawn as plain boxes the size of what blocks shots in KovaaK's (Tree, Window)";
    let left = "some props aren't drawn (Banner, Pebbles); nothing collides with them in KovaaK's either";
    let both = vec![
        prop("Window", [0.0; 3], [1.0; 3], true),
        prop("Tree", [0.0; 3], [1.0; 3], true),
        prop("Window", [0.0; 3], [1.0; 3], true),
        prop("Pebbles", [0.0; 3], [1.0; 3], false),
        prop("Banner", [0.0; 3], [1.0; 3], true),
        prop("Banner", [0.0; 3], [1.0; 3], true),
    ];
    assert_eq!(note_list(both), vec![drawn.to_string(), left.to_string()]);
    let only_window = "some props are drawn as plain boxes the size of what blocks shots in KovaaK's (Window)";
    assert_eq!(note_list(vec![window()]), vec![only_window]);
    assert_eq!(note_list(vec![prop("Pebbles", [0.0; 3], [1.0; 3], false)]).len(), 1);
    assert!(note_list(vec![]).is_empty());
    // A prop with no size is left out, and a colliding kind that is refused anyway has no note.
    let sizeless = note_list(vec![prop("Window", [0.0; 3], [0.0, 1.0, 1.0], true)]);
    assert_eq!(sizeless, vec!["some props aren't drawn (Window); nothing collides with them in KovaaK's either"]);
    assert!(note_list(vec![prop("Lantern", [0.0; 3], [1.0; 3], true)]).is_empty());
}

fn prop_gaps(text: &str) -> Vec<String> {
    let gaps = crate::inventory::gaps(&Sce::parse(text));
    gaps.into_iter().filter(|g| g.feature == "solid props").map(|g| g.detail).collect()
}

#[test]
fn only_a_kind_that_was_not_measured_is_a_gap() {
    let found = prop_gaps(LANTERNS);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("Lantern") && !found[0].contains("Window"), "{found:?}");
    assert_eq!(prop_gaps(SLAB), Vec::<String>::new());
    let off = LANTERNS.replace(r#""value": true"#, r#""value": false"#);
    assert_ne!(off, LANTERNS);
    assert_eq!(prop_gaps(&off), Vec::<String>::new());
}
