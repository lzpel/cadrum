//! Validate `area` / `center` / `inertia` queries against analytical solutions.
//!
//! OCCT computes properties with uniform density ρ = 1. The inertia tensor is
//! returned about the world origin (not the center of mass), which makes
//! aggregation over collections a straight matrix sum (parallel-axis theorem
//! is already folded in).

use cadrum::Solid;
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
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(a));

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
	let cubes = vec![Solid::cube(DVec3::ZERO, DVec3::splat(a)), Solid::cube(DVec3::ZERO, DVec3::splat(a)).translate(DVec3::new(offset, 0.0, 0.0))];

	let expected_center = DVec3::new((a / 2.0 + (a / 2.0 + offset)) / 2.0, a / 2.0, a / 2.0);
	// Aggregate manually with iterator idioms: volume-weighted center plus
	// summed volume / area (the library no longer provides collection methods).
	let total_vol: f64 = cubes.iter().map(|c| c.volume()).sum();
	let center = cubes.iter().map(|c| c.center() * c.volume()).sum::<DVec3>() / total_vol;
	assert!((center - expected_center).length() < EPS, "aggregate center = {center:?}, expected {expected_center:?}");
	assert!((total_vol - 2.0 * a.powi(3)).abs() < EPS);
	assert!((cubes.iter().map(|c| c.area()).sum::<f64>() - 2.0 * 6.0 * a.powi(2)).abs() < EPS);
}

/// World-origin inertia is additive across elements: summing the per-element
/// tensors of two identical co-located cubes gives exactly twice a single
/// cube's tensor. Confirms both the single-element `inertia()` and that the
/// `map(..).fold(DMat3::ZERO, +)` idiom is the correct way to aggregate.
#[test]
fn test_inertia_is_additive_over_elements() {
	let single = Solid::cube(DVec3::ZERO, DVec3::splat(3.0)).inertia();
	let cubes = vec![Solid::cube(DVec3::ZERO, DVec3::splat(3.0)), Solid::cube(DVec3::ZERO, DVec3::splat(3.0))];
	let summed = cubes.iter().map(|c| c.inertia()).fold(glam::DMat3::ZERO, |a, b| a + b);
	for col in 0..3 {
		for row in 0..3 {
			assert!((summed.col(col)[row] - 2.0 * single.col(col)[row]).abs() < 1e-9, "summed inertia[{row}][{col}] = {}, expected {}", summed.col(col)[row], 2.0 * single.col(col)[row]);
		}
	}
}

/// Aggregating over an empty collection with iterator idioms yields the
/// natural identity values (sum = 0, fold = ZERO) without panicking.
#[test]
fn test_empty_vec_queries_do_not_panic() {
	let empty: Vec<Solid> = vec![];
	assert_eq!(empty.iter().map(|s| s.volume()).sum::<f64>(), 0.0);
	assert_eq!(empty.iter().map(|s| s.area()).sum::<f64>(), 0.0);
	assert_eq!(empty.iter().map(|s| s.inertia()).fold(glam::DMat3::ZERO, |a, b| a + b), glam::DMat3::ZERO);
}
