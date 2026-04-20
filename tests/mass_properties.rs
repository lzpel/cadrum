//! Validate `area` / `center` / `inertia` queries against analytical solutions.
//!
//! OCCT computes properties with uniform density ρ = 1. The inertia tensor is
//! returned about the world origin (not the center of mass), which makes
//! aggregation over collections a straight matrix sum (parallel-axis theorem
//! is already folded in).

use cadrum::{Compound, Solid};
use glam::DVec3;

const EPS: f64 = 1e-6;

/// Cube of side `a` with corner at the world origin, ρ = 1.
/// Analytical values referenced throughout:
///   volume = a³
///   center = (a/2, a/2, a/2)
///   area   = 6 a²
///   I_xx = I_yy = I_zz = ∫(y² + z²) dV over [0,a]³ = 2 a⁵ / 3
///   |I_xy| = |I_yz| = |I_zx| = a⁵ / 4  (sign depends on tensor sign convention)
#[test]
fn test_cube_mass_properties_match_analytical() {
	let a = 10.0_f64;
	let cube = Solid::cube(a, a, a);

	assert!((cube.volume() - a.powi(3)).abs() < EPS);
	assert!((cube.area() - 6.0 * a.powi(2)).abs() < EPS);
	assert!((cube.center() - DVec3::splat(a / 2.0)).length() < EPS);

	let i = cube.inertia();
	let expected_diag = 2.0 * a.powi(5) / 3.0;
	let expected_off = a.powi(5) / 4.0;
	// Diagonals are positive and equal for a symmetric cube.
	assert!((i.col(0).x - expected_diag).abs() < 1e-3, "I_xx = {}, expected {expected_diag}", i.col(0).x);
	assert!((i.col(1).y - expected_diag).abs() < 1e-3, "I_yy = {}, expected {expected_diag}", i.col(1).y);
	assert!((i.col(2).z - expected_diag).abs() < 1e-3, "I_zz = {}, expected {expected_diag}", i.col(2).z);
	// Off-diagonals: magnitude only (sign depends on OCCT convention).
	assert!((i.col(1).x.abs() - expected_off).abs() < 1e-3, "|I_xy| = {}, expected {expected_off}", i.col(1).x.abs());
	assert!((i.col(2).y.abs() - expected_off).abs() < 1e-3, "|I_yz| = {}, expected {expected_off}", i.col(2).y.abs());
	assert!((i.col(2).x.abs() - expected_off).abs() < 1e-3, "|I_zx| = {}, expected {expected_off}", i.col(2).x.abs());
}

/// Sphere of radius `r` centered at origin.
///   volume = (4/3) π r³
///   area   = 4 π r²
///   center = 0
///   I_diag = (2/5) m r² = (8/15) π r⁵  (sphere centered at origin → COM = origin)
#[test]
fn test_sphere_mass_properties_match_analytical() {
	let r = 5.0_f64;
	let sphere = Solid::sphere(r);

	let pi = std::f64::consts::PI;
	assert!((sphere.volume() - 4.0 / 3.0 * pi * r.powi(3)).abs() < 1e-2);
	assert!((sphere.area() - 4.0 * pi * r.powi(2)).abs() < 1e-2);
	assert!(sphere.center().length() < 1e-3, "sphere COM should be at origin, got {:?}", sphere.center());

	let i = sphere.inertia();
	let expected_diag = 8.0 / 15.0 * pi * r.powi(5);
	assert!((i.col(0).x - expected_diag).abs() < 1e-1, "I_xx = {}, expected {expected_diag}", i.col(0).x);
	assert!((i.col(1).y - expected_diag).abs() < 1e-1);
	assert!((i.col(2).z - expected_diag).abs() < 1e-1);
	// Off-diagonals should be ~0 for a sphere at origin.
	assert!(i.col(1).x.abs() < 1e-3);
	assert!(i.col(2).x.abs() < 1e-3);
	assert!(i.col(2).y.abs() < 1e-3);
}

/// Aggregate of two identical cubes offset along +X. The collection's center
/// of mass is the volume-weighted average of the per-cube centers; here both
/// weights are equal so the result is the midpoint.
#[test]
fn test_vec_center_is_volume_weighted_average() {
	let a = 2.0_f64;
	let offset = 10.0_f64;
	let cubes = vec![
		Solid::cube(a, a, a),
		Solid::cube(a, a, a).translate(DVec3::new(offset, 0.0, 0.0)),
	];

	let expected_center = DVec3::new((a / 2.0 + (a / 2.0 + offset)) / 2.0, a / 2.0, a / 2.0);
	assert!(
		(cubes.center() - expected_center).length() < EPS,
		"aggregate center = {:?}, expected {expected_center:?}", cubes.center()
	);
	assert!((cubes.volume() - 2.0 * a.powi(3)).abs() < EPS);
	assert!((cubes.area() - 2.0 * 6.0 * a.powi(2)).abs() < EPS);
}

/// World-origin inertia is additive: `inertia(Vec)` equals the sum of
/// per-element `inertia()`. This is tautological for the current implementation
/// but guards against someone "fixing" the aggregator to re-center to the
/// collection's COM (which would break parallel-axis additivity).
#[test]
fn test_vec_inertia_equals_sum_of_elements() {
	let cubes = vec![
		Solid::cube(3.0, 3.0, 3.0),
		Solid::cube(2.0, 2.0, 2.0).translate(DVec3::new(5.0, 0.0, 0.0)),
		Solid::cube(4.0, 4.0, 4.0).translate(DVec3::new(0.0, 7.0, 0.0)),
	];

	let expected = cubes.iter().map(|c| c.inertia()).fold(glam::DMat3::ZERO, |a, b| a + b);
	let actual = cubes.inertia();
	for col in 0..3 {
		for row in 0..3 {
			assert!(
				(actual.col(col)[row] - expected.col(col)[row]).abs() < 1e-9,
				"inertia[{row}][{col}] = {}, expected {}", actual.col(col)[row], expected.col(col)[row]
			);
		}
	}
}

/// Empty Vec returns zero/identity values without panicking. Exercises the
/// zero-volume guard in `center()`.
#[test]
fn test_empty_vec_queries_do_not_panic() {
	let empty: Vec<Solid> = vec![];
	assert_eq!(empty.volume(), 0.0);
	assert_eq!(empty.area(), 0.0);
	assert_eq!(empty.center(), DVec3::ZERO);
	assert_eq!(empty.inertia(), glam::DMat3::ZERO);
}
