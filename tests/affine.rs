use cadrum::{DMat4, DVec3, Shape, Solid, Tessellation};
#[test]
fn ellipsoid_and_elliptical_cylinder_have_expected_volumes() {
	for original in [Solid::sphere(2.0), Solid::cylinder(2.0, DVec3::Z * 3.0)] {
		let shape = Shape::from(&original);
		let result = shape.scale_xyz(DVec3::splat(1.0), DVec3::new(2.0, 3.0, 4.0)).unwrap();
		assert!(result.value.is_valid().unwrap());
		assert!((result.value.mass_properties(cadrum::MeasurementKind::Volume, 1e-9).unwrap().mass / original.volume() / 24.0 - 1.0).abs() < 1e-6, "actual {} original {}", result.value.mass_properties(cadrum::MeasurementKind::Volume, 1e-9).unwrap().mass, original.volume());
		assert!(!result.history.relations.is_empty());
	}
}
#[test]
fn shear_reflection_and_inverse_round_trip() {
	let original = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let matrix = DMat4::from_cols_array(&[-2.0, 0.0, 0.0, 0.0, 0.4, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 5.0, 6.0, 7.0, 1.0]);
	let result = original.affine(matrix).unwrap();
	assert!((result.value.mass_properties(cadrum::MeasurementKind::Volume, 1e-9).unwrap().mass - 24.0).abs() < 1e-7);
	let restored = result.value.affine(matrix.inverse()).unwrap().value;
	for (a, b) in restored.bounding_box().unwrap().unwrap().iter().zip(original.bounding_box().unwrap().unwrap()) {
		assert!((*a - b).length() < 1e-6);
	}
	let open = Shape::sew(original.faces().unwrap().iter().take(5), 1e-6).unwrap();
	let reflected = open.affine(matrix).unwrap().value;
	assert_eq!(reflected.faces().unwrap().len(), 5);
	assert!(!reflected.mesh(Tessellation::default()).unwrap().indices.is_empty());
	assert!(original.scale_xyz(DVec3::ZERO, DVec3::new(1.0, 0.0, 1.0)).is_err());
	assert!(original.affine(DMat4::NAN).is_err());
}
