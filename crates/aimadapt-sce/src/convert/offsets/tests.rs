//! Tests for spawn offsets: when two ways of running them fill the same box.

use aimadapt_sim::Vec3;

use super::same_box;

const MAP: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];

/// The map's frame turned by `deg` degrees about the vertical.
fn yawed(deg: f32) -> [Vec3; 3] {
    let q = glam::Quat::from_rotation_y(deg.to_radians());
    MAP.map(|v| q * v)
}

#[test]
fn the_same_frame_fills_the_same_box() {
    assert!(same_box(MAP, MAP, [-1.0, 0.0, 2.0], [3.0, 5.0, 2.0]));
}

/// A point turned half round with offsets that mirror each other keeps its box.
#[test]
fn a_half_turn_with_mirrored_offsets_is_the_same_box() {
    assert!(same_box(yawed(180.0), MAP, [-0.5, 0.0, -0.5], [0.5, 0.0, 0.5]));
    assert!(same_box(yawed(180.0), MAP, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]));
}

/// Offsets that don't mirror put the box on the other side.
#[test]
fn a_half_turn_with_offsets_that_arent_mirrored_is_another_box() {
    assert!(!same_box(yawed(180.0), MAP, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    assert!(!same_box(yawed(180.0), MAP, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
}

/// A quarter turn with an offset along one axis puts it along another.
#[test]
fn a_quarter_turn_with_an_offset_on_one_axis_is_another_box() {
    assert!(!same_box(yawed(90.0), MAP, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    assert!(!same_box(yawed(90.0), MAP, [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
}

/// A quarter turn about the axis the only offset runs along changes nothing.
#[test]
fn a_turn_about_the_axis_of_the_only_offset_is_the_same_box() {
    assert!(same_box(yawed(90.0), MAP, [0.0, -2.0, 0.0], [0.0, 3.0, 0.0]));
    assert!(same_box(yawed(37.0), MAP, [0.0, 1.0, 0.0], [0.0, 3.0, 0.0]));
}

/// With no offsets there is nothing to move, however the point is turned.
#[test]
fn zero_offsets_on_a_turned_point_are_the_same_box() {
    assert!(same_box(yawed(90.0), MAP, [0.0; 3], [0.0; 3]));
    assert!(same_box(yawed(33.0), MAP, [0.0; 3], [0.0; 3]));
}

/// A box a millimeter off isn't told from the same box, and an edge swapped for another
/// of the same length is told.
#[test]
fn boxes_are_compared_to_a_millimeter() {
    assert!(same_box(yawed(0.01), MAP, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
    assert!(!same_box(yawed(0.2), MAP, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
}
