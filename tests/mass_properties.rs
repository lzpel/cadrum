//! Validate `area` / `center` / `inertia` queries against analytical solutions.
//!
//! OCCT computes properties with uniform density ρ = 1; the inertia tensor is
//! returned about the world origin (not the center of mass).

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
