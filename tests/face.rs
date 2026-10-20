//! Integration tests for `Face::face`.
//!
//! Covers:
//! - each elementary surface kind: area against the closed-form value, and
//!   `Face::surface` reporting the surface back
//! - a full turn of a periodic parameter gives one closed face
//! - faces built this way sew into a solid
//! - an empty range and an invalid surface are `Error::Validation`

use cadrum::{Error, Face, Solid, Surface, SurfaceKind};
use glam::DVec3;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const REL: f64 = 1e-9;

fn assert_close(actual: f64, expected: f64, what: &str) {
	assert!((actual - expected).abs() <= REL * expected.abs().max(1.0), "{what}: {actual} vs {expected}");
}

fn at_origin(kind: SurfaceKind) -> Surface {
	Surface { origin: DVec3::ZERO, axis_z: DVec3::Z, axis_x: DVec3::X, kind }
}

#[test]
fn plane_rectangle_has_the_area_and_reports_its_surface() {
	let s = Surface { origin: DVec3::new(1.0, 2.0, 3.0), axis_z: DVec3::Y, axis_x: DVec3::X, kind: SurfaceKind::Plane };
	let f = Face::face(&s, [-1.0, 3.0], [0.0, 2.5]).unwrap();
	assert_close(f.area(), 4.0 * 2.5, "area");
	let back = f.surface().expect("a plane");
	assert_eq!(back.kind, SurfaceKind::Plane);
	assert!((back.origin - s.origin).length() < 1e-9 && (back.axis_z - s.axis_z).length() < 1e-9 && (back.axis_x - s.axis_x).length() < 1e-9, "{back:?}");
}

#[test]
fn cylinder_full_turn_is_a_closed_band() {
	let s = Surface { origin: DVec3::new(0.0, 1.0, 0.0), axis_z: DVec3::Y, axis_x: DVec3::X, kind: SurfaceKind::Cylinder { radius: 2.0 } };
	let f = Face::face(&s, [0.0, TAU], [0.5, 3.5]).unwrap();
	assert_close(f.area(), TAU * 2.0 * 3.0, "area");
	assert_eq!(f.surface().unwrap().kind, SurfaceKind::Cylinder { radius: 2.0 });
	// closed on its seam: a point on the far side projects onto the face itself
	let (cp, _) = f.project(DVec3::new(-5.0, 2.0, 0.0));
	assert!((cp - DVec3::new(-2.0, 2.0, 0.0)).length() < 1e-6, "{cp:?}");
}

#[test]
fn cylinder_partial_turn_covers_its_fraction() {
	let f = Face::face(&at_origin(SurfaceKind::Cylinder { radius: 1.5 }), [FRAC_PI_2, PI], [0.0, 4.0]).unwrap();
	assert_close(f.area(), FRAC_PI_2 * 1.5 * 4.0, "area");
}

#[test]
fn cone_band_is_measured_along_the_generatrix() {
	let a = 0.4_f64;
	let f = Face::face(&at_origin(SurfaceKind::Cone { radius: 0.0, semi_angle: a }), [0.0, TAU], [1.0, 3.0]).unwrap();
	assert_close(f.area(), PI * a.sin() * (9.0 - 1.0), "area");
	match f.surface().unwrap().kind {
		SurfaceKind::Cone { radius, semi_angle } => assert!(radius.abs() < 1e-9 && (semi_angle - a).abs() < 1e-12, "{radius} {semi_angle}"),
		other => panic!("{other:?}"),
	}
}

#[test]
fn sphere_hemisphere_and_cap() {
	let s = at_origin(SurfaceKind::Sphere { radius: 3.0 });
	let hemisphere = Face::face(&s, [0.0, TAU], [0.0, FRAC_PI_2]).unwrap();
	assert_close(hemisphere.area(), TAU * 9.0, "hemisphere");
	let cap = Face::face(&s, [0.0, TAU], [1.0, FRAC_PI_2]).unwrap();
	assert_close(cap.area(), TAU * 9.0 * (1.0 - 1.0_f64.sin()), "cap");
	assert_eq!(cap.surface().unwrap().kind, SurfaceKind::Sphere { radius: 3.0 });
}

#[test]
fn torus_full_turn_both_ways() {
	let f = Face::face(&at_origin(SurfaceKind::Torus { major_radius: 5.0, minor_radius: 1.0 }), [0.0, TAU], [0.0, TAU]).unwrap();
	assert_close(f.area(), 4.0 * PI * PI * 5.0 * 1.0, "area");
	assert_eq!(f.surface().unwrap().kind, SurfaceKind::Torus { major_radius: 5.0, minor_radius: 1.0 });
}

#[test]
fn six_plane_rectangles_sew_into_a_box() {
	let face = |origin: DVec3, axis_z: DVec3, axis_x: DVec3, u: f64, v: f64| Face::face(&Surface { origin, axis_z, axis_x, kind: SurfaceKind::Plane }, [0.0, u], [0.0, v]).unwrap();
	let (a, b, c) = (2.0, 3.0, 4.0);
	// The frame is right-handed, so `v` runs along axis_z x axis_x: -Y x X = +Z.
	let faces = [face(DVec3::ZERO, DVec3::Z, DVec3::X, a, b), face(DVec3::new(0.0, 0.0, c), DVec3::Z, DVec3::X, a, b), face(DVec3::ZERO, DVec3::NEG_Y, DVec3::X, a, c), face(DVec3::new(0.0, b, 0.0), DVec3::NEG_Y, DVec3::X, a, c), face(DVec3::ZERO, DVec3::X, DVec3::Y, b, c), face(DVec3::new(a, 0.0, 0.0), DVec3::X, DVec3::Y, b, c)];
	let solid = Solid::sew(faces.iter(), 1.0e-6).expect("six rectangles close a shell");
	assert_close(solid.volume().abs(), a * b * c, "volume");
}

#[test]
fn empty_range_is_a_validation_error() {
	let s = at_origin(SurfaceKind::Plane);
	for (u, v) in [([1.0, 1.0], [0.0, 1.0]), ([0.0, 1.0], [2.0, 1.0])] {
		match Face::face(&s, u, v) {
			Err(Error::Validation(msg)) => assert!(msg.contains("empty parameter range"), "{msg}"),
			other => panic!("expected Error::Validation, got {other:?}"),
		}
	}
}

#[test]
fn surfaces_occt_cannot_build_are_validation_errors() {
	for kind in [SurfaceKind::Cylinder { radius: 0.0 }, SurfaceKind::Cylinder { radius: -1.0 }, SurfaceKind::Cone { radius: 0.0, semi_angle: 0.0 }, SurfaceKind::Sphere { radius: 0.0 }] {
		assert!(matches!(Face::face(&at_origin(kind), [0.0, 1.0], [0.0, 1.0]), Err(Error::Validation(_))), "{kind:?}");
	}
}
