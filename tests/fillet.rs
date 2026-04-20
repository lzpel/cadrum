//! Integration tests for `Solid::fillet_edges`.
//!
//! A unit cube with edge length `a` filleted by radius `r` removes:
//!   - 12 quarter-cylinder edges: each replaces a quarter-square prism of
//!     cross-section `r² − π r² / 4` over length `a − 2r`
//!   - 8 sphere-octant corners:   each replaces a cube corner of volume
//!     `r³ − (4/3) π r³ / 8 = r³ − π r³ / 6`
//! The analytical result below is used to validate OCCT's output within 0.1%.

use cadrum::{Error, Solid};
use std::f64::consts::PI;

const EPS: f64 = 1e-6;

#[test]
fn test_fillet_cube_reduces_volume_and_area() {
	let a = 10.0_f64;
	let r = 1.0_f64;
	let cube = Solid::cube(a, a, a);
	let original_volume = cube.volume();
	let original_area = cube.area();
	let edges: Vec<_> = cube.iter_edge().collect();
	let rounded = cube.fillet_edges(r, edges).expect("fillet should succeed");

	assert!(rounded.volume() < original_volume,
		"fillet must reduce volume: {} vs {}", rounded.volume(), original_volume);
	assert!(rounded.area() < original_area,
		"fillet must reduce area: {} vs {}", rounded.area(), original_area);
}

#[test]
fn test_fillet_cube_matches_analytical_volume() {
	let a = 10.0_f64;
	let r = 1.0_f64;
	let cube = Solid::cube(a, a, a);
	let edges: Vec<_> = cube.iter_edge().collect();
	let rounded = cube.fillet_edges(r, edges).expect("fillet cube");

	let removed_edges = 12.0 * (r * r - PI * r * r / 4.0) * (a - 2.0 * r);
	let removed_corners = 8.0 * (r.powi(3) - PI * r.powi(3) / 6.0);
	let expected = a.powi(3) - removed_edges - removed_corners;
	let rel_err = (rounded.volume() - expected).abs() / expected;
	assert!(rel_err < 1e-3,
		"rounded cube volume {} vs analytical {} (rel err {})",
		rounded.volume(), expected, rel_err);
}

#[test]
fn test_fillet_empty_edges_is_noop() {
	let cube = Solid::cube(5.0, 5.0, 5.0);
	let original_volume = cube.volume();
	let unchanged = cube
		.fillet_edges(0.5, std::iter::empty::<&cadrum::Edge>())
		.expect("empty fillet is a no-op, not an error");
	assert!((unchanged.volume() - original_volume).abs() < EPS,
		"no-op fillet should preserve volume exactly, got {}", unchanged.volume());
}

#[test]
fn test_fillet_radius_too_large_returns_err() {
	let cube = Solid::cube(2.0, 2.0, 2.0);
	let edges: Vec<_> = cube.iter_edge().collect();
	// r = 5 > a/2 = 1 → geometrically impossible; OCCT reports not-done.
	let err = cube.fillet_edges(5.0, edges).err().expect("oversized radius must fail");
	assert!(matches!(err, Error::FilletFailed),
		"expected FilletFailed, got {:?}", err);
}
