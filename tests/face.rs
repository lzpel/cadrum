//! Integration tests for `Face::face`.
//!
//! Covers:
//! - each elementary surface kind: area against the closed-form value, and
//!   `Face::surface` reporting the surface back
//! - a full turn of a periodic parameter gives one closed face
//! - faces built this way sew into a solid
//! - an empty range and an invalid surface are `Error::Validation`

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

/// The rectangle `[0, u] x [0, v]` of a plane, counterclockwise about its normal.
fn rectangle(s: &Surface, u: f64, v: f64) -> Vec<Edge> {
	let (du, dv) = (s.axis_x * u, s.axis_z.cross(s.axis_x) * v);
	Edge::polygon(&[s.origin, s.origin + du, s.origin + du + dv, s.origin + dv]).unwrap()
}

/// A circle about the Z axis at height `z`, counterclockwise about +Z when `ccw`.
fn ring(radius: f64, z: f64, ccw: bool) -> Edge {
	Edge::circle(radius, if ccw { DVec3::Z } else { DVec3::NEG_Z }).unwrap().translate(DVec3::Z * z)
}

/// A square `[0, a]²` at height `z`, counterclockwise about +Z, with a clockwise bore of radius `r` at its center.
fn plate(a: f64, r: f64, z: f64) -> Vec<Edge> {
	let mut edges = rectangle(&Surface { origin: DVec3::Z * z, ..at_origin(SurfaceKind::Plane) }, a, a);
	edges.push(ring(r, z, false).translate(DVec3::new(a / 2.0, a / 2.0, 0.0)));
	edges
}

#[test]
fn plane_rectangle_reports_its_surface() {
	let s = Surface { origin: DVec3::new(1.0, 2.0, 3.0), axis_z: DVec3::Y, axis_x: DVec3::X, kind: SurfaceKind::Plane };
	let f = Face::face(&s, &rectangle(&s, 4.0, 2.5)).unwrap();
	assert_close(f.area(), 4.0 * 2.5, "area");
	let back = f.surface().expect("a plane");
	assert_eq!(back.kind, SurfaceKind::Plane);
	assert!((back.origin - s.origin).length() < 1e-9 && (back.axis_z - s.axis_z).length() < 1e-9 && (back.axis_x - s.axis_x).length() < 1e-9, "{back:?}");
}

#[test]
fn plane_with_a_hole() {
	assert_close(Face::face(&at_origin(SurfaceKind::Plane), &plate(4.0, 1.0, 0.0)).unwrap().area(), 16.0 - PI, "area");
}

#[test]
fn edges_may_run_either_way() {
	let p = [DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y];
	// the first and third edges run backwards along the loop
	let edges = [Edge::line(p[1], p[0]).unwrap(), Edge::line(p[1], p[2]).unwrap(), Edge::line(p[3], p[2]).unwrap(), Edge::line(p[3], p[0]).unwrap()];
	assert_close(Face::face(&at_origin(SurfaceKind::Plane), &edges).unwrap().area(), 1.0, "area");
}

#[test]
fn cylinder_patch_and_band() {
	let (r, h) = (2.0, 3.0);
	let p = |angle: f64, z: f64| DVec3::new(r * angle.cos(), r * angle.sin(), z);
	let patch = [Edge::arc_3pts(p(0.0, 0.0), p(FRAC_PI_4, 0.0), p(FRAC_PI_2, 0.0)).unwrap(), Edge::line(p(FRAC_PI_2, 0.0), p(FRAC_PI_2, h)).unwrap(), Edge::arc_3pts(p(FRAC_PI_2, h), p(FRAC_PI_4, h), p(0.0, h)).unwrap(), Edge::line(p(0.0, h), p(0.0, 0.0)).unwrap()];
	let s = at_origin(SurfaceKind::Cylinder { radius: r });
	let f = Face::face(&s, &patch).unwrap();
	assert_close(f.area(), FRAC_PI_2 * r * h, "patch");
	assert_eq!(f.surface().unwrap().kind, SurfaceKind::Cylinder { radius: r });
	// closed on its seam: a point on the far side projects onto the band itself
	let band = Face::face(&s, [&ring(r, 0.0, true), &ring(r, h, false)]).unwrap();
	assert_close(band.area(), TAU * r * h, "band");
	let (cp, _) = band.project(DVec3::new(-5.0, 0.0, 1.0));
	assert!((cp - DVec3::new(-r, 0.0, 1.0)).length() < 1e-6, "{cp:?}");
}

#[test]
fn cone_band_and_tip() {
	// at 45 degrees, heights 1..3 are radii 1..3 and a slant of 2 sqrt 2
	let kind = SurfaceKind::Cone { radius: 0.0, semi_angle: FRAC_PI_4 };
	let band = Face::face(&at_origin(kind.clone()), [&ring(1.0, 1.0, true), &ring(3.0, 3.0, false)]).unwrap();
	assert_close(band.area(), PI * (1.0 + 3.0) * 2.0 * SQRT_2, "band");
	assert_eq!(band.surface().unwrap().kind, kind);
	// a single ring closes at the apex
	let tip = Face::face(&at_origin(kind), [&ring(3.0, 3.0, false)]).unwrap();
	assert_close(tip.area(), PI * 3.0 * 3.0 * SQRT_2, "tip");
}

#[test]
fn sphere_whole_and_either_side_of_a_ring() {
	let (r, lat) = (3.0_f64, 1.0_f64);
	let s = at_origin(SurfaceKind::Sphere { radius: r });
	assert_close(Face::face(&s, []).unwrap().area(), 2.0 * TAU * r * r, "whole");
	assert_close(Face::face(&s, [&ring(r, 0.0, true)]).unwrap().area(), TAU * r * r, "hemisphere");
	let (rho, z) = (r * lat.cos(), r * lat.sin());
	let cap = Face::face(&s, [&ring(rho, z, true)]).unwrap();
	assert_close(cap.area(), TAU * r * r * (1.0 - lat.sin()), "cap");
	assert_eq!(cap.surface().unwrap().kind, SurfaceKind::Sphere { radius: r });
	assert_close(Face::face(&s, [&ring(rho, z, false)]).unwrap().area(), TAU * r * r * (1.0 + lat.sin()), "rest");
}

#[test]
fn torus_whole_and_either_half() {
	let (big, small) = (5.0, 1.0);
	let s = at_origin(SurfaceKind::Torus { major_radius: big, minor_radius: small });
	assert_close(Face::face(&s, []).unwrap().area(), 4.0 * PI * PI * big * small, "whole");
	// the top and bottom circles of the tube split it into an inner and an outer half
	let inner = Face::face(&s, [&ring(big, small, true), &ring(big, -small, false)]).unwrap();
	assert_close(inner.area(), TAU * small * (PI * big - 2.0 * small), "inner");
	let outer = Face::face(&s, [&ring(big, small, false), &ring(big, -small, true)]).unwrap();
	assert_close(outer.area(), TAU * small * (PI * big + 2.0 * small), "outer");
	assert_eq!(outer.surface().unwrap().kind, SurfaceKind::Torus { major_radius: big, minor_radius: small });
}

#[test]
fn six_plane_rectangles_sew_into_a_box() {
	let (a, b, c) = (2.0, 3.0, 4.0);
	let face = |origin: DVec3, axis_z: DVec3, axis_x: DVec3, u: f64, v: f64| {
		let s = Surface { origin, axis_z, axis_x, kind: SurfaceKind::Plane };
		Face::face(&s, &rectangle(&s, u, v)).unwrap()
	};
	// The frame is right-handed, so `v` runs along axis_z x axis_x: -Y x X = +Z.
	let faces = [face(DVec3::ZERO, DVec3::Z, DVec3::X, a, b), face(DVec3::new(0.0, 0.0, c), DVec3::Z, DVec3::X, a, b), face(DVec3::ZERO, DVec3::NEG_Y, DVec3::X, a, c), face(DVec3::new(0.0, b, 0.0), DVec3::NEG_Y, DVec3::X, a, c), face(DVec3::ZERO, DVec3::X, DVec3::Y, b, c), face(DVec3::new(a, 0.0, 0.0), DVec3::X, DVec3::Y, b, c)];
	let solid = Solid::sew(faces.iter(), 1.0e-6).expect("six rectangles close a shell");
	assert_close(solid.volume().abs(), a * b * c, "volume");
}

#[test]
fn plates_and_a_bore_sew_into_a_block() {
	let (a, r, h) = (4.0, 1.0, 2.0);
	let side = |origin: DVec3, axis_z: DVec3, axis_x: DVec3| {
		let s = Surface { origin, axis_z, axis_x, kind: SurfaceKind::Plane };
		Face::face(&s, &rectangle(&s, a, h)).unwrap()
	};
	let plane = |z: f64| Surface { origin: DVec3::Z * z, ..at_origin(SurfaceKind::Plane) };
	let bore = Surface { origin: DVec3::new(a / 2.0, a / 2.0, 0.0), ..at_origin(SurfaceKind::Cylinder { radius: r }) };
	let center = DVec3::new(a / 2.0, a / 2.0, 0.0);
	let faces = [Face::face(&plane(0.0), &plate(a, r, 0.0)).unwrap(), Face::face(&plane(h), &plate(a, r, h)).unwrap(), side(DVec3::ZERO, DVec3::NEG_Y, DVec3::X), side(DVec3::new(0.0, a, 0.0), DVec3::NEG_Y, DVec3::X), side(DVec3::ZERO, DVec3::X, DVec3::Y), side(DVec3::new(a, 0.0, 0.0), DVec3::X, DVec3::Y), Face::face(&bore, [&ring(r, 0.0, true).translate(center), &ring(r, h, false).translate(center)]).unwrap()];
	let solid = Solid::sew(faces.iter(), 1.0e-6).expect("the plates, sides and bore close a shell");
	assert_close(solid.volume().abs(), (a * a - PI * r * r) * h, "volume");
}

#[test]
fn cube_sews_from_twelve_lines() {
	let corner = |i: usize| DVec3::new((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64);
	let edges: Vec<Edge> = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)].iter().map(|&(a, b)| Edge::line(corner(a), corner(b)).unwrap()).collect();
	let plane = |origin: DVec3, axis_z: DVec3| Surface { origin, axis_z, axis_x: if axis_z.x == 0.0 { DVec3::X } else { DVec3::Y }, kind: SurfaceKind::Plane };
	// each loop winds counterclockwise about its outward normal, so some edges run against it
	let sides = [(plane(DVec3::ZERO, DVec3::NEG_Z), [4, 1, 5, 0]), (plane(DVec3::Z, DVec3::Z), [2, 7, 3, 6]), (plane(DVec3::ZERO, DVec3::NEG_Y), [0, 9, 2, 8]), (plane(DVec3::Y, DVec3::Y), [10, 3, 11, 1]), (plane(DVec3::ZERO, DVec3::NEG_X), [8, 6, 10, 4]), (plane(DVec3::X, DVec3::X), [5, 11, 7, 9])];
	let faces: Vec<Face> = sides.iter().map(|(s, ids)| Face::face(s, ids.iter().map(|&i| &edges[i])).unwrap()).collect();
	let solid = Solid::sew(faces.iter(), 1.0e-6).unwrap();
	assert_close(solid.volume().abs(), 1.0, "volume");
	assert_eq!(solid.iter_edge().count(), 12);
}

#[test]
fn bad_profiles_are_validation_errors() {
	let s = at_origin(SurfaceKind::Plane);
	let lifted = rectangle(&Surface { origin: DVec3::Z, ..s.clone() }, 1.0, 1.0);
	let clockwise = Edge::polygon(&[DVec3::ZERO, DVec3::Y, DVec3::new(1.0, 1.0, 0.0), DVec3::X]).unwrap();
	let open = Edge::polygon(&[DVec3::ZERO, DVec3::X, DVec3::Y]).unwrap()[..2].to_vec();
	let mut hole_ccw = plate(4.0, 1.0, 0.0);
	*hole_ccw.last_mut().unwrap() = ring(1.0, 0.0, true).translate(DVec3::new(2.0, 2.0, 0.0));
	let mut hole_outside = plate(4.0, 1.0, 0.0);
	*hole_outside.last_mut().unwrap() = ring(1.0, 0.0, false).translate(DVec3::new(10.0, 0.0, 0.0));
	for (what, profile) in [("off the surface", lifted), ("empty on a plane", vec![]), ("open", open), ("clockwise", clockwise), ("hole counterclockwise", hole_ccw), ("hole outside", hole_outside)] {
		assert!(matches!(Face::face(&s, &profile), Err(Error::Validation(_))), "{what}");
	}
}

#[test]
fn surfaces_occt_cannot_build_are_validation_errors() {
	for kind in [SurfaceKind::Cylinder { radius: 0.0 }, SurfaceKind::Cylinder { radius: -1.0 }, SurfaceKind::Cone { radius: 0.0, semi_angle: 0.0 }, SurfaceKind::Sphere { radius: 0.0 }, SurfaceKind::Torus { major_radius: 5.0, minor_radius: 0.0 }] {
		assert!(matches!(Face::face(&at_origin(kind.clone()), []), Err(Error::Validation(_))), "{kind:?}");
	}
}

// ==================== loops and B-spline surfaces ====================

/// A plate with a bore has two loops: the outer rectangle first, then the hole.
#[test]
fn loops_list_the_outer_boundary_first() {
	let f = Face::face(&at_origin(SurfaceKind::Plane), &plate(4.0, 1.0, 0.0)).unwrap();
	let loops: Vec<&[Edge]> = f.iter_loop().collect();
	assert_eq!(loops.len(), 2);
	assert_eq!(loops[0].len(), 4, "the outer square has four edges");
	assert_eq!(loops[1].len(), 1, "the bore is one circle");
	assert_eq!(f.iter_edge().count(), 5);
	// Consecutive edges of the outer loop meet end to end, either way round.
	for pair in loops[0].windows(2) {
		let ends = [pair[0].start_point(), pair[0].end_point()];
		assert!(ends.iter().any(|&p| (p - pair[1].start_point()).length() < 1e-9 || (p - pair[1].end_point()).length() < 1e-9));
	}
	// A cube face has one loop of four edges, all of them edges of the cube.
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(2.0));
	let cube_edges: std::collections::HashSet<u64> = cube.iter_edge().map(|e| e.id()).collect();
	for face in cube.iter_face() {
		let loops: Vec<&[Edge]> = face.iter_loop().collect();
		assert_eq!(loops.len(), 1);
		assert_eq!(loops[0].len(), 4);
		assert!(loops[0].iter().all(|e| cube_edges.contains(&e.id())));
	}
}

/// A lofted face reads back as a B-spline whose coefficients rebuild the same face.
#[test]
fn bspline_surface_reads_back_and_rebuilds() {
	// Two circles loft into a cone, which reads back as one; a waist makes a true B-spline.
	let lower = [Edge::circle(3.0, DVec3::Z).unwrap()];
	let waist = [Edge::circle(1.0, DVec3::Z).unwrap().translate(DVec3::Z * 4.0)];
	let upper = [Edge::circle(3.0, DVec3::Z).unwrap().translate(DVec3::Z * 8.0)];
	let loft = Solid::loft([&lower[..], &waist[..], &upper[..]], false).unwrap();
	let skin = loft.iter_face().max_by(|a, b| a.area().total_cmp(&b.area())).unwrap();
	let surface = skin.surface().unwrap();
	let SurfaceKind::Bspline(nurbs) = &surface.kind else { panic!("the skin is a B-spline: {surface:?}") };
	assert_eq!(nurbs.poles.len(), nurbs.weights.len());
	assert!(nurbs.poles.iter().all(|row| row.len() == nurbs.poles[0].len()));
	assert_eq!(nurbs.knots[0].len(), nurbs.multiplicities[0].len());
	assert_eq!(nurbs.knots[1].len(), nurbs.multiplicities[1].len());
	// Every control point of a loft between two circles lies between the two planes.
	assert!(nurbs.poles.iter().flatten().all(|p| p.z >= -1e-9 && p.z <= 8.0 + 1e-9));
	// An open patch rebuilds from its own loop; the closed skin needs a seam, which
	// `Face::face` adds only on a periodic surface, so it is rebuilt whole below.
	let patch = Solid::bspline(4, 4, false, false, |i, j| DVec3::new(i as f64, j as f64, ((i * j) as f64 * 0.7).sin())).unwrap();
	let patch_face = patch.iter_face().find(|f| matches!(f.surface().unwrap().kind, SurfaceKind::Bspline(_))).expect("the half-space keeps the patch as a face");
	let rebuilt = Face::face(&patch_face.surface().unwrap(), patch_face.iter_loop().flatten()).unwrap();
	assert_close(rebuilt.area(), patch_face.area(), "rebuilt patch area");
	// The whole surface, with no loops, is the natural-bounds face.
	let whole = Face::face(&surface, std::iter::empty()).unwrap();
	assert_close(whole.area(), skin.area(), "natural bounds");
	assert!(matches!(whole.surface().unwrap().kind, SurfaceKind::Bspline(_)));
}

/// Vertices of a solid are reported once each.
#[test]
fn cube_has_eight_vertices() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(2.0));
	let vertices: Vec<&DVec3> = cube.iter_vertex().collect();
	assert_eq!(vertices.len(), 8);
	assert!(vertices.iter().all(|v| v.to_array().iter().all(|&c| c == 0.0 || c == 2.0)));
}
