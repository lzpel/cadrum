//! Integration tests for `Solid::chamfer_edges`.
//!
//! A cube with edge length `a` chamfered by distance `d` (d ≤ a/2) removes
//! material from 12 edges (triangular wedges) and their corner overlaps.
//! Inclusion-exclusion on 12 wedges / 24 pair-intersections at corners /
//! 8 triple-intersections at corners gives the closed form
//!   `V_removed = 6 d² (a − d)`.
//! This is used below to validate OCCT's output within 0.1%.

use cadrum::{Error, Solid};

const EPS: f64 = 1e-6;

#[test]
fn test_chamfer_cube_reduces_volume_and_area() {
	let a = 10.0_f64;
	let d = 1.0_f64;
	let cube = Solid::cube(a, a, a);
	let original_volume = cube.volume();
	let original_area = cube.area();
	let edges: Vec<_> = cube.iter_edge().collect();
	let beveled = cube.chamfer_edges(d, edges).expect("chamfer should succeed");

	assert!(beveled.volume() < original_volume,
		"chamfer must reduce volume: {} vs {}", beveled.volume(), original_volume);
	assert!(beveled.area() < original_area,
		"chamfer must reduce area: {} vs {}", beveled.area(), original_area);
}

#[test]
fn test_chamfer_cube_matches_analytical_volume() {
	let a = 10.0_f64;
	let d = 1.0_f64;
	let cube = Solid::cube(a, a, a);
	let edges: Vec<_> = cube.iter_edge().collect();
	let beveled = cube.chamfer_edges(d, edges).expect("chamfer cube");

	// Inclusion-exclusion:
	//   12 edge-wedges (d²/2 × a)       = 6 a d²
	//   − 24 corner pair-intersections  = 8 d³
	//   + 8 corner triple-intersections = 2 d³
	//   = 6 d² (a − d)
	let expected = a.powi(3) - 6.0 * d * d * (a - d);
	let rel_err = (beveled.volume() - expected).abs() / expected;
	assert!(rel_err < 1e-3,
		"chamfered cube volume {} vs analytical {} (rel err {})",
		beveled.volume(), expected, rel_err);
}

#[test]
fn test_chamfer_empty_edges_is_noop() {
	let cube = Solid::cube(5.0, 5.0, 5.0);
	let original_volume = cube.volume();
	let unchanged = cube
		.chamfer_edges(0.5, std::iter::empty::<&cadrum::Edge>())
		.expect("empty chamfer is a no-op, not an error");
	assert!((unchanged.volume() - original_volume).abs() < EPS,
		"no-op chamfer should preserve volume exactly, got {}", unchanged.volume());
}

#[test]
fn test_chamfer_distance_too_large_returns_err() {
	let cube = Solid::cube(2.0, 2.0, 2.0);
	let edges: Vec<_> = cube.iter_edge().collect();
	// d = 5 > a/2 = 1 → geometrically impossible; OCCT reports not-done.
	let err = cube.chamfer_edges(5.0, edges).err().expect("oversized distance must fail");
	assert!(matches!(err, Error::ChamferFailed),
		"expected ChamferFailed, got {:?}", err);
}
