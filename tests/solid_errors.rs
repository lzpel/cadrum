//! The failure shape: degenerate input is an `Err`, never a panic or a crash.

use cadrum::{DVec3, Edge, Error, Solid};

#[test]
fn degenerate_primitives_are_validation_errors() {
	assert!(matches!(Solid::cube(DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0)), Err(Error::Validation(_))), "a flat box");
	assert!(matches!(Solid::sphere(0.0), Err(Error::Validation(_))));
	assert!(matches!(Solid::sphere(-1.0), Err(Error::Validation(_))));
	assert!(matches!(Solid::cylinder(1.0, DVec3::ZERO), Err(Error::Validation(_))), "zero height");
	assert!(matches!(Solid::cylinder(0.0, DVec3::Z), Err(Error::Validation(_))), "zero radius");
	assert!(matches!(Solid::cone(2.0, 2.0, DVec3::Z), Err(Error::Validation(_))), "equal radii");
	assert!(matches!(Solid::cone(0.0, 0.0, DVec3::Z), Err(Error::Validation(_))), "no radius");
	assert!(matches!(Solid::torus(1.0, 2.0, DVec3::Z), Err(Error::Validation(_))), "minor beyond major");
	assert!(matches!(Solid::torus(1.0, 2.0, DVec3::ZERO), Err(Error::Validation(_))), "zero axis");
	assert!(matches!(Solid::half_space(DVec3::ZERO, DVec3::ZERO), Err(Error::Validation(_))), "zero normal");
	assert!(Solid::cone(0.0, 2.0, DVec3::Z).is_ok(), "a pointed cone is fine");
}

#[test]
fn degenerate_transforms_are_transform_errors() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(1.0)).unwrap();
	assert!(matches!(cube.clone().rotate(DVec3::ZERO, DVec3::ZERO, 1.0), Err(Error::Transform(_))), "zero axis");
	assert!(matches!(cube.clone().scale(DVec3::ZERO, 0.0), Err(Error::Transform(_))), "zero factor");
	assert!(matches!(cube.clone().mirror(DVec3::ZERO, DVec3::ZERO), Err(Error::Transform(_))), "zero normal");
	assert!(matches!(cube.clone().align_x(DVec3::ZERO, DVec3::Y), Err(Error::Transform(_))), "zero target");
	assert!(matches!(cube.clone().align_z(DVec3::Z, DVec3::Z), Err(Error::Transform(_))), "parallel hint");
	assert!(cube.clone().rotate_z(1.0).is_ok() && cube.clone().scale(DVec3::ZERO, 2.0).is_ok());

	let edge = Edge::line(DVec3::ZERO, DVec3::X).unwrap();
	assert!(matches!(edge.clone().rotate(DVec3::ZERO, DVec3::ZERO, 1.0), Err(Error::Transform(_))));
	assert!(matches!(edge.clone().scale(DVec3::ZERO, 0.0), Err(Error::Transform(_))));
	assert!(matches!(edge.clone().mirror(DVec3::ZERO, DVec3::ZERO), Err(Error::Transform(_))));
	assert!(edge.align_y(DVec3::X, DVec3::Z).is_ok());
}

#[test]
fn a_built_solid_is_valid_and_measurable() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(2.0)).unwrap();
	assert!(cube.is_valid());
	assert!((cube.volume() - 8.0).abs() < 1e-9);
	let face = cube.iter_face().next().unwrap();
	let (point, normal) = face.project(DVec3::splat(5.0)).unwrap();
	assert!(point.is_finite() && (normal.length() - 1.0).abs() < 1e-9);
	let edge = cube.iter_edge().next().unwrap();
	assert!(edge.project(DVec3::splat(5.0)).unwrap().0.is_finite());
}
