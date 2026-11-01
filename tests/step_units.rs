use cadrum::{DVec3, LengthUnit, OperationControl, Shape, Solid, StepReadOptions, StepWriteOptions};
#[test]
fn units_and_styles_round_trip_with_explicit_normalization() {
	for unit in [LengthUnit::Millimeter, LengthUnit::Meter, LengthUnit::Inch] {
		let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(25.4)).color("#c04020"));
		let mut bytes = Vec::new();
		let warnings = shape.write_step_with_options(&mut bytes, StepWriteOptions { output_unit: unit, ..Default::default() }, &OperationControl::default()).unwrap();
		assert!(warnings.is_empty(), "{warnings:?}");
		let result = Shape::read_step_with_options(&mut bytes.as_slice(), StepReadOptions::default(), &OperationControl::default()).unwrap();
		assert!(!result.report.detected_length_units.is_empty());
		assert_eq!(result.report.normalized_millimeters_per_unit, 1.0);
		assert!((result.result.value.bounding_box().unwrap().unwrap()[1].x - 25.4).abs() < 1e-5);
		assert!(!result.result.value.mesh(Default::default()).unwrap().colormap.is_empty());
		let in_inches = Shape::read_step_with_options(&mut bytes.as_slice(), StepReadOptions { normalize_to: LengthUnit::Inch, ..Default::default() }, &OperationControl::default()).unwrap();
		assert!((in_inches.result.value.bounding_box().unwrap().unwrap()[1].x - 1.0).abs() < 1e-6);
	}
}
#[test]
fn model_units_are_distinct_from_export_units() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let mut bytes = Vec::new();
	shape.write_step_with_options(&mut bytes, StepWriteOptions { model_unit: LengthUnit::Meter, output_unit: LengthUnit::Inch, ..Default::default() }, &OperationControl::default()).unwrap();
	let read = Shape::read_step_with_options(&mut bytes.as_slice(), StepReadOptions::default(), &OperationControl::default()).unwrap();
	assert!((read.result.value.bounding_box().unwrap().unwrap()[1].x - 1000.0).abs() < 1e-5);
}

#[test]
fn absent_units_are_rejected_before_transfer() {
	let step = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('point'),'2;1');\nFILE_NAME('point','2026-11-01T00:00:00',(''),(''),'','','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,0.));\n#2=GEOMETRIC_CURVE_SET('',(#1));\n#3=(GEOMETRIC_REPRESENTATION_CONTEXT(3)REPRESENTATION_CONTEXT('',''));\n#4=GEOMETRICALLY_BOUNDED_WIREFRAME_SHAPE_REPRESENTATION('',(#2),#3);\nENDSEC;\nEND-ISO-10303-21;\n";
	let error = Shape::read_step_with_options(&mut step.as_slice(), StepReadOptions::default(), &OperationControl::default()).unwrap_err();
	assert!(error.to_string().contains("units are missing"), "{error}");
}

#[test]
fn convenience_step_io_retains_styles() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE).color("#c04020"));
	let mut bytes = Vec::new();
	shape.write_step(&mut bytes, &OperationControl::default()).unwrap();
	let read = Shape::read_step(&mut bytes.as_slice(), &OperationControl::default()).unwrap();
	let mesh = read.value.mesh(Default::default()).unwrap();
	assert!(!mesh.face_ids.is_empty());
	assert!(mesh.face_ids.iter().all(|id| mesh.colormap.contains_key(id)));
}
