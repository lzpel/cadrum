//! Integration tests for `Edge` query APIs — `EdgeStruct::project` and the
//! `wire_project` iterator idiom for an ordered edge list.
//!
//! Covers single `Edge` and multi-edge wire (`Vec<Edge>`) paths across a
//! mix of curve kinds (line, circle, polygon, B-spline). The expected
//! closest point is computed analytically from the curve definition and
//! compared to the FFI result.

use cadrum::{BSplineEnd, Edge};
use glam::DVec3;

const TOL: f64 = 1e-6;

fn approx_eq(a: DVec3, b: DVec3, tol: f64) -> bool {
	(a - b).length() < tol
}

/// Project a point onto a wire (= ordered edge list) by taking the nearest
/// per-edge projection. This is the iterator idiom that replaces the removed
/// `Wire::project` collection method.
fn wire_project(wire: &[Edge], p: DVec3) -> (DVec3, DVec3) {
	wire.iter().map(|e| e.project(p)).min_by(|(a, _), (b, _)| (*a - p).length_squared().partial_cmp(&(*b - p).length_squared()).unwrap()).unwrap_or((DVec3::ZERO, DVec3::ZERO))
}

#[test]
fn project_on_line_midpoint() {
	// Line from -X to +X along X axis; query above the midpoint -> projects to origin.
	let e = Edge::line(DVec3::new(-1.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 0.0)).unwrap();
	let (cp, tg) = e.project(DVec3::new(0.0, 1.0, 0.0));
	assert!(approx_eq(cp, DVec3::ZERO, TOL), "cp={cp:?}");
	// Tangent is ±X (normalized).
	assert!((tg.x.abs() - 1.0).abs() < TOL && tg.y.abs() < TOL && tg.z.abs() < TOL, "tg={tg:?}");
}

#[test]
fn project_on_circle_returns_radius() {
	// Unit circle in XY plane; project (2, 0, 0) -> expect (1, 0, 0).
	let e = Edge::circle(1.0, DVec3::Z).unwrap();
	let (cp, tg) = e.project(DVec3::new(2.0, 0.0, 0.0));
	assert!(approx_eq(cp, DVec3::new(1.0, 0.0, 0.0), TOL), "cp={cp:?}");
	// At (1,0,0) the unit-tangent to a CCW-parameterized circle is ±Y.
	assert!(tg.x.abs() < TOL && (tg.y.abs() - 1.0).abs() < TOL && tg.z.abs() < TOL, "tg={tg:?}");
	// Off-plane query still lands on the circle in XY.
	let (cp2, _) = e.project(DVec3::new(3.0, 0.0, 5.0));
	assert!(approx_eq(cp2, DVec3::new(1.0, 0.0, 0.0), TOL), "cp2={cp2:?}");
}

#[test]
fn project_on_polygon_picks_nearest_edge() {
	// Closed square in XY plane: edges (±1, ±1, 0). Query point near +X edge.
	let square = Edge::polygon([DVec3::new(1.0, 1.0, 0.0), DVec3::new(-1.0, 1.0, 0.0), DVec3::new(-1.0, -1.0, 0.0), DVec3::new(1.0, -1.0, 0.0)].iter()).unwrap();
	// (2, 0, 0) is closest to the right edge x=1, y∈[-1,1] -> (1, 0, 0).
	let (cp, _) = wire_project(&square, DVec3::new(2.0, 0.0, 0.0));
	assert!(approx_eq(cp, DVec3::new(1.0, 0.0, 0.0), TOL), "cp={cp:?}");
	// (-2, -3, 0) is closest to the corner (-1, -1, 0).
	let (cp2, _) = wire_project(&square, DVec3::new(-2.0, -3.0, 0.0));
	assert!(approx_eq(cp2, DVec3::new(-1.0, -1.0, 0.0), TOL), "cp2={cp2:?}");
}

#[test]
fn project_on_bspline_converges_to_interpolant() {
	// Periodic cubic B-spline through four XY ring points. Origin should
	// project somewhere on the ring; its distance is roughly the ring's
	// mean radius (≈1.0 here since all control points are unit-distant).
	let pts = [DVec3::new(1.0, 0.0, 0.0), DVec3::new(0.0, 1.0, 0.0), DVec3::new(-1.0, 0.0, 0.0), DVec3::new(0.0, -1.0, 0.0)];
	let e = Edge::bspline(pts.iter(), BSplineEnd::Periodic).unwrap();
	let (cp, tg) = e.project(DVec3::ZERO);
	// A periodic cubic B-spline interpolating 4 unit-distance points is
	// not a perfect circle — it "cuts the corner" between knots, so the
	// closest-to-origin point sits strictly inside the unit circle.
	// Confirm the projection is on the curve (not at origin) and within
	// the sensible envelope (chord midpoint = √0.5 ≈ 0.707 .. unit = 1.0).
	let r = cp.length();
	assert!((0.7..=1.0).contains(&r), "radius out of envelope: {r}");
	// Tangent is unit-length.
	assert!((tg.length() - 1.0).abs() < TOL, "|tg|={}", tg.length());
}

// ==================== is_loop ====================

#[test]
fn is_loop_single() {
	assert_eq!(Edge::precision_distance(), 1.0e-7);
	// A single closed edge is a loop — the case the removed `is_closed` covered.
	let circle = Edge::circle(2.0, DVec3::Z).unwrap();
	assert!(Edge::is_loop(&[circle]));
	let line = Edge::line(DVec3::ZERO, DVec3::X).unwrap();
	assert!(!Edge::is_loop(&[line]));
	assert!(!Edge::is_loop(&[]));
}

#[test]
fn is_loop_gap_at_precision_distance() {
	//without gap
	let square = Edge::polygon(&[DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y]).unwrap();
	assert!(Edge::is_loop(&square));
	// with gap
	let tolerance = Edge::precision_distance();
	let a = DVec3::ZERO;
	let b = DVec3::X;
	let c = DVec3::Y;
	// Reopen the loop by a gap an order of magnitude above the tolerance.
	let gap = DVec3::new(0.0, 0.0, tolerance * 10.0);
	let open = [Edge::line(a, b).unwrap(), Edge::line(b, c).unwrap(), Edge::line(c, a + gap).unwrap()];
	assert!(!Edge::is_loop(&open));
	let closed = [Edge::line(a, b).unwrap(), Edge::line(b, c).unwrap(), Edge::line(c, a).unwrap()];
	assert!(Edge::is_loop(&closed));
}

// ==================== curve ====================

/// Each edge kind reads back as its curve, and the curve rebuilds an equal edge.
#[test]
fn curve_reads_back_and_rebuilds() {
	use cadrum::{Curve, CurveKind};
	let same = |a: &Edge, b: &Edge| approx_eq(a.start_point(), b.start_point(), TOL) && approx_eq(a.end_point(), b.end_point(), TOL) && approx_eq(a.project(DVec3::new(0.3, 0.2, 0.1)).0, b.project(DVec3::new(0.3, 0.2, 0.1)).0, TOL);

	let line = Edge::line(DVec3::new(1.0, 2.0, 3.0), DVec3::new(4.0, 6.0, 3.0)).unwrap();
	let c = line.curve().unwrap();
	assert_eq!(c.kind, CurveKind::Line);
	assert!((c.range[1] - c.range[0] - 5.0).abs() < TOL, "a line's range is its length: {c:?}");
	assert!(same(&line, &Edge::edge(&c).unwrap()));

	let circle = Edge::circle(2.0, DVec3::Y).unwrap().translate(DVec3::X * 3.0);
	let c = circle.curve().unwrap();
	assert_eq!(c.kind, CurveKind::Circle { radius: 2.0 });
	assert!(approx_eq(c.origin, DVec3::X * 3.0, TOL) && approx_eq(c.axis_z, DVec3::Y, TOL), "{c:?}");
	assert!((c.range[1] - c.range[0] - std::f64::consts::TAU).abs() < TOL);
	assert!(same(&circle, &Edge::edge(&c).unwrap()));

	let ellipse = Edge::edge(&Curve { origin: DVec3::ZERO, axis_z: DVec3::Z, axis_x: DVec3::X, kind: CurveKind::Ellipse { major_radius: 10.0, minor_radius: 4.0 }, range: [0.0, std::f64::consts::PI] }).unwrap();
	assert!(approx_eq(ellipse.start_point(), DVec3::X * 10.0, TOL) && approx_eq(ellipse.end_point(), DVec3::X * -10.0, TOL));
	assert!(approx_eq(ellipse.project(DVec3::Y * 100.0).0, DVec3::Y * 4.0, TOL), "the minor axis is 4");
	assert_eq!(ellipse.curve().unwrap().kind, CurveKind::Ellipse { major_radius: 10.0, minor_radius: 4.0 });

	let spline = Edge::bspline(&[DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0), DVec3::new(2.0, 0.0, 1.0), DVec3::new(3.0, 1.0, 1.0)], BSplineEnd::NotAKnot).unwrap();
	let c = spline.curve().unwrap();
	let CurveKind::Bspline(n) = &c.kind else { panic!("{c:?}") };
	assert_eq!(n.degree, 3);
	assert_eq!(n.poles.len(), n.weights.len());
	assert_eq!(n.knots.len(), n.multiplicities.len());
	assert!(same(&spline, &Edge::edge(&c).unwrap()));

	// A degenerate edge has no curve: the pole of a sphere.
	let sphere = cadrum::Solid::sphere(1.0);
	assert!(sphere.iter_edge().any(|e| e.curve().is_none()), "a sphere has a degenerate edge at each pole");

	// An empty range is refused.
	assert!(Edge::edge(&Curve { range: [1.0, 1.0], ..c.clone() }).is_err());
}
