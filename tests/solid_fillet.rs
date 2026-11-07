//! Integration tests for `Solid::fillet_edges`.
//!
//! A unit cube with edge length `a` filleted by radius `r` removes:
//!   - 12 quarter-cylinder edges: each replaces a quarter-square prism of
//!     cross-section `r² − π r² / 4` over length `a − 2r`
//!   - 8 sphere-octant corners:   each replaces a cube corner of volume
//!     `r³ − (4/3) π r³ / 8 = r³ − π r³ / 6`
//! The analytical result below is used to validate OCCT's output within 0.1%.

use cadrum::{DVec3, Edge, Error, Solid};
use std::f64::consts::PI;

const EPS: f64 = 1e-6;

#[test]
fn test_fillet_cube_reduces_volume_and_area() {
	let a = 10.0_f64;
	let r = 1.0_f64;
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(a));
	let original_volume = cube.volume();
	let original_area = cube.area();
	let edges: Vec<_> = cube.iter_edge().collect();
	let rounded = cube.fillet_edges(r, edges).expect("fillet should succeed");

	assert!(rounded.volume() < original_volume, "fillet must reduce volume: {} vs {}", rounded.volume(), original_volume);
	assert!(rounded.area() < original_area, "fillet must reduce area: {} vs {}", rounded.area(), original_area);
}

#[test]
fn test_fillet_cube_matches_analytical_volume() {
	let a = 10.0_f64;
	let r = 1.0_f64;
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(a));
	let edges: Vec<_> = cube.iter_edge().collect();
	let rounded = cube.fillet_edges(r, edges).expect("fillet cube");

	let removed_edges = 12.0 * (r * r - PI * r * r / 4.0) * (a - 2.0 * r);
	let removed_corners = 8.0 * (r.powi(3) - PI * r.powi(3) / 6.0);
	let expected = a.powi(3) - removed_edges - removed_corners;
	let rel_err = (rounded.volume() - expected).abs() / expected;
	assert!(rel_err < 1e-3, "rounded cube volume {} vs analytical {} (rel err {})", rounded.volume(), expected, rel_err);
}

#[test]
fn test_fillet_empty_edges_is_noop() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(5.0));
	let original_volume = cube.volume();
	let unchanged = cube.fillet_edges(0.5, std::iter::empty::<&cadrum::Edge>()).expect("empty fillet is a no-op, not an error");
	assert!((unchanged.volume() - original_volume).abs() < EPS, "no-op fillet should preserve volume exactly, got {}", unchanged.volume());
}

#[test]
fn test_fillet_radius_too_large_returns_err() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(2.0));
	let edges: Vec<_> = cube.iter_edge().collect();
	// r = 5 > a/2 = 1 → geometrically impossible; OCCT reports not-done.
	let err = cube.fillet_edges(5.0, edges).err().expect("oversized radius must fail");
	assert!(matches!(err, Error::Fillet(_)), "expected Error::Fillet, got {:?}", err);
}

/// Issue #324: two fillets on edges that share no vertex, on a prism whose
/// profile has an arc. OCCT indexed the wrong stripe in PerformMoreSurfdata
/// and crashed; patches/pr3.patch fixes it. Expected volume is the prism
/// minus what each fillet alone removes (30.18 and 40.28).
#[test]
fn test_fillet_two_disjoint_edges_on_arc_prism() {
	let mut profile = Edge::polygon(&[DVec3::ZERO, DVec3::new(22.5, 0.0, 0.0), DVec3::new(22.5, 22.5, 0.0), DVec3::new(0.0, 22.5, 0.0)]).unwrap();
	profile.truncate(3);
	profile.push(Edge::arc_3pts(DVec3::new(0.0, 22.5, 0.0), DVec3::new(-7.5, 11.25, 0.0), DVec3::ZERO).unwrap());
	let prism = Solid::extrude(&profile, DVec3::Z * 5.0).unwrap();
	let nearest = |p: DVec3| prism.iter_edge().min_by(|a, b| a.project(p).0.distance(p).total_cmp(&b.project(p).0.distance(p))).unwrap();
	let straight = nearest(DVec3::new(22.5, 11.25, 0.0));
	let arc = nearest(DVec3::new(-7.5, 11.25, 5.0));

	let removed_straight = prism.volume() - prism.fillet_edges(2.5, [straight]).unwrap().volume();
	let removed_arc = prism.volume() - prism.fillet_edges(2.5, [arc]).unwrap().volume();
	let both = prism.fillet_edges(2.5, [straight, arc]).expect("two disjoint fillets must succeed");
	let expected = prism.volume() - removed_straight - removed_arc;
	assert!((both.volume() - expected).abs() < 1e-3, "volume {} vs expected {}", both.volume(), expected);
}
