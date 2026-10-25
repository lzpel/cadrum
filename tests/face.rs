//! Integration tests for `Face::face` and `Face::trim`.
//!
//! Covers:
//! - each elementary surface kind: area against the closed-form value, and
//!   `Face::surface` reporting the surface back
//! - a full turn of a periodic parameter gives one closed face
//! - faces built this way sew into a solid
//! - an empty range and an invalid surface are `Error::Validation`
//! - `trim` on each surface kind, taking the side left of each loop as STEP
//!   does; edges running either way; a cube sewn from 12 shared edges; bad profiles

use cadrum::{Edge, Error, Face, Solid, Surface, SurfaceKind};
use glam::DVec3;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2, TAU};

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
fn cone_band_is_measured_along_the_axis() {
	// at 45 degrees, heights 1..3 are radii 1..3 and a slant of 2 sqrt 2
	let kind = SurfaceKind::Cone { radius: 0.0, semi_angle: FRAC_PI_4 };
	let f = Face::face(&at_origin(kind), [0.0, TAU], [1.0, 3.0]).unwrap();
	assert_close(f.area(), PI * (1.0 + 3.0) * 2.0 * SQRT_2, "area");
	assert_eq!(f.surface().unwrap().kind, kind);
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

/// A square `[0, a]²` at height `z`, counterclockwise about +Z, with a clockwise bore of radius `r` at its center.
fn plate(a: f64, r: f64, z: f64) -> Vec<Edge> {
	let mut edges = Edge::polygon(&[DVec3::new(0.0, 0.0, z), DVec3::new(a, 0.0, z), DVec3::new(a, a, z), DVec3::new(0.0, a, z)]).unwrap();
	edges.push(Edge::circle(r, DVec3::NEG_Z).unwrap().translate(DVec3::new(a / 2.0, a / 2.0, z)));
	edges
}

/// A circle about the Z axis at height `z`, counterclockwise about +Z when `ccw`.
fn ring(radius: f64, z: f64, ccw: bool) -> Edge {
	Edge::circle(radius, if ccw { DVec3::Z } else { DVec3::NEG_Z }).unwrap().translate(DVec3::Z * z)
}

fn assert_area(f: &Face, expected: f64, what: &str) {
	assert!((f.area() - expected).abs() < 1e-6 * expected.max(1.0), "{what}: {} vs {expected}", f.area());
}

#[test]
fn trim_plane_with_a_hole() {
	let f = Face::trim(&at_origin(SurfaceKind::Plane), &plate(4.0, 1.0, 0.0)).unwrap();
	assert_area(&f, 16.0 - PI, "plate");
	assert_eq!(f.surface().unwrap().kind, SurfaceKind::Plane);
}

#[test]
fn trim_plane_accepts_edges_running_either_way() {
	let p = [DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y];
	// the first and third edges run backwards along the loop
	let edges = [Edge::line(p[1], p[0]).unwrap(), Edge::line(p[1], p[2]).unwrap(), Edge::line(p[3], p[2]).unwrap(), Edge::line(p[3], p[0]).unwrap()];
	assert_area(&Face::trim(&at_origin(SurfaceKind::Plane), &edges).unwrap(), 1.0, "square");
}

#[test]
fn trim_cylinder_patch_and_band() {
	let (r, h) = (2.0, 3.0);
	let p = |angle: f64, z: f64| DVec3::new(r * angle.cos(), r * angle.sin(), z);
	let patch = [Edge::arc_3pts(p(0.0, 0.0), p(FRAC_PI_4, 0.0), p(FRAC_PI_2, 0.0)).unwrap(), Edge::line(p(FRAC_PI_2, 0.0), p(FRAC_PI_2, h)).unwrap(), Edge::arc_3pts(p(FRAC_PI_2, h), p(FRAC_PI_4, h), p(0.0, h)).unwrap(), Edge::line(p(0.0, h), p(0.0, 0.0)).unwrap()];
	let s = at_origin(SurfaceKind::Cylinder { radius: r });
	let f = Face::trim(&s, &patch).unwrap();
	assert_area(&f, FRAC_PI_2 * r * h, "patch");
	assert_eq!(f.surface().unwrap().kind, SurfaceKind::Cylinder { radius: r });
	assert_area(&Face::trim(&s, [&ring(r, 0.0, true), &ring(r, h, false)]).unwrap(), TAU * r * h, "band");
}

#[test]
fn trim_cone_band() {
	// at 45 degrees, heights 1..3 are radii 1..3 and a slant of 2 sqrt 2
	let kind = SurfaceKind::Cone { radius: 0.0, semi_angle: FRAC_PI_4 };
	let f = Face::trim(&at_origin(kind), [&ring(1.0, 1.0, true), &ring(3.0, 3.0, false)]).unwrap();
	assert_area(&f, PI * (1.0 + 3.0) * 2.0 * SQRT_2, "band");
	assert_eq!(f.surface().unwrap().kind, kind);
}

#[test]
fn trim_sphere_takes_the_side_left_of_the_loop() {
	let (r, lat) = (3.0_f64, 1.0_f64);
	let s = at_origin(SurfaceKind::Sphere { radius: r });
	let (rho, z) = (r * lat.cos(), r * lat.sin());
	assert_area(&Face::trim(&s, [&ring(rho, z, true)]).unwrap(), TAU * r * r * (1.0 - lat.sin()), "cap");
	assert_area(&Face::trim(&s, [&ring(rho, z, false)]).unwrap(), TAU * r * r * (1.0 + lat.sin()), "rest");
}

#[test]
fn trim_torus_takes_the_side_left_of_the_loops() {
	let (big, small) = (5.0, 1.0);
	let s = at_origin(SurfaceKind::Torus { major_radius: big, minor_radius: small });
	// the top and bottom circles of the tube split it into an inner and an outer half
	let inner = Face::trim(&s, [&ring(big, small, true), &ring(big, -small, false)]).unwrap();
	assert_area(&inner, TAU * small * (PI * big - 2.0 * small), "inner");
	let outer = Face::trim(&s, [&ring(big, small, false), &ring(big, -small, true)]).unwrap();
	assert_area(&outer, TAU * small * (PI * big + 2.0 * small), "outer");
	assert_eq!(outer.surface().unwrap().kind, SurfaceKind::Torus { major_radius: big, minor_radius: small });
}

#[test]
fn trimmed_plates_sew_into_a_bored_block() {
	let (a, r, h) = (4.0, 1.0, 2.0);
	let plane = |origin: DVec3, axis_z: DVec3, axis_x: DVec3| Surface { origin, axis_z, axis_x, kind: SurfaceKind::Plane };
	let side = |origin: DVec3, axis_z: DVec3, axis_x: DVec3| Face::face(&plane(origin, axis_z, axis_x), [0.0, a], [0.0, h]).unwrap();
	let bore = Surface { origin: DVec3::new(a / 2.0, a / 2.0, 0.0), axis_z: DVec3::Z, axis_x: DVec3::X, kind: SurfaceKind::Cylinder { radius: r } };
	let faces = [Face::trim(&plane(DVec3::ZERO, DVec3::Z, DVec3::X), &plate(a, r, 0.0)).unwrap(), Face::trim(&plane(DVec3::Z * h, DVec3::Z, DVec3::X), &plate(a, r, h)).unwrap(), side(DVec3::ZERO, DVec3::NEG_Y, DVec3::X), side(DVec3::new(0.0, a, 0.0), DVec3::NEG_Y, DVec3::X), side(DVec3::ZERO, DVec3::X, DVec3::Y), side(DVec3::new(a, 0.0, 0.0), DVec3::X, DVec3::Y), Face::face(&bore, [0.0, TAU], [0.0, h]).unwrap()];
	let solid = Solid::sew(faces.iter(), 1.0e-6).expect("the plates, sides and bore close a shell");
	assert!((solid.volume().abs() - (a * a - PI * r * r) * h).abs() < 1e-6, "{}", solid.volume());
}

/// Corner `i` of the unit cube and its 12 edges as corner pairs, each running one fixed way.
fn corner(i: usize) -> DVec3 {
	DVec3::new((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64)
}
const CUBE_EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];

/// The six sides of the unit cube trimmed from `edges` (indexed as `CUBE_EDGES`).
fn cube_sides(edges: &[&Edge]) -> Vec<Face> {
	let plane = |origin: DVec3, axis_z: DVec3| Surface { origin, axis_z, axis_x: if axis_z.x == 0.0 { DVec3::X } else { DVec3::Y }, kind: SurfaceKind::Plane };
	// each loop winds counterclockwise about its outward normal, so some edges run against it
	let sides = [(plane(DVec3::ZERO, DVec3::NEG_Z), [4, 1, 5, 0]), (plane(DVec3::Z, DVec3::Z), [2, 7, 3, 6]), (plane(DVec3::ZERO, DVec3::NEG_Y), [0, 9, 2, 8]), (plane(DVec3::Y, DVec3::Y), [10, 3, 11, 1]), (plane(DVec3::ZERO, DVec3::NEG_X), [8, 6, 10, 4]), (plane(DVec3::X, DVec3::X), [5, 11, 7, 9])];
	sides.iter().map(|(s, ids)| Face::trim(s, ids.iter().map(|&i| edges[i])).unwrap()).collect()
}

#[test]
fn cube_sews_from_twelve_lines() {
	let lines: Vec<Edge> = CUBE_EDGES.iter().map(|&(a, b)| Edge::line(corner(a), corner(b)).unwrap()).collect();
	let solid = Solid::sew(cube_sides(&lines.iter().collect::<Vec<_>>()).iter(), 1.0e-6).unwrap();
	assert_close(solid.volume().abs(), 1.0, "volume");
	assert_eq!(solid.iter_edge().count(), 12);
}

#[test]
fn cube_sides_share_the_edges_of_a_solid() {
	// unlike separate lines, a solid's edges share their vertices, so wires keep the edges themselves
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let ends = |e: &Edge| [e.start_point(), e.end_point()];
	let edges: Vec<&Edge> = CUBE_EDGES.iter().map(|&(a, b)| cube.iter_edge().find(|e| ends(e) == [corner(a), corner(b)] || ends(e) == [corner(b), corner(a)]).unwrap()).collect();
	let faces = cube_sides(&edges);
	for e in &edges {
		assert_eq!(faces.iter().filter(|f| f.iter_edge().any(|fe| fe.id() == e.id())).count(), 2, "{:?}", ends(e));
	}
	assert_close(Solid::sew(faces.iter(), 1.0e-6).unwrap().volume().abs(), 1.0, "volume");
}

#[test]
fn trim_rejects_bad_profiles() {
	let s = at_origin(SurfaceKind::Plane);
	let lifted = Edge::polygon(&[DVec3::new(0.0, 0.0, 1.0), DVec3::new(1.0, 0.0, 1.0), DVec3::new(1.0, 1.0, 1.0), DVec3::new(0.0, 1.0, 1.0)]).unwrap();
	let clockwise = Edge::polygon(&[DVec3::ZERO, DVec3::Y, DVec3::new(1.0, 1.0, 0.0), DVec3::X]).unwrap();
	let mut hole_ccw = plate(4.0, 1.0, 0.0);
	*hole_ccw.last_mut().unwrap() = ring(1.0, 0.0, true).translate(DVec3::new(2.0, 2.0, 0.0));
	let mut hole_outside = plate(4.0, 1.0, 0.0);
	*hole_outside.last_mut().unwrap() = ring(1.0, 0.0, false).translate(DVec3::new(10.0, 0.0, 0.0));
	for (what, profile) in [("off the surface", lifted), ("empty", vec![]), ("clockwise", clockwise), ("hole counterclockwise", hole_ccw), ("hole outside", hole_outside)] {
		assert!(matches!(Face::trim(&s, &profile), Err(Error::Validation(_))), "{what}");
	}
}
