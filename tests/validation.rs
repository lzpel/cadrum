use cadrum::{DVec3, Edge, Error, RepairOptions, Shape, ShapeKind, Solid};

#[test]
fn validity_and_category_expectations_are_independent() {
	let solid = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let report = solid.validate(Some(ShapeKind::Shell)).unwrap();
	assert!(report.valid);
	assert!(!report.category_matches);
	assert!(report.issues.is_empty());
	let open = Shape::sew(solid.faces().unwrap().iter().take(5), 1e-6).unwrap();
	let report = open.validate(Some(ShapeKind::Shell)).unwrap();
	assert!(report.valid && report.category_matches && !report.closed && !report.empty);
	let empty: Shape = Shape::compound([]).unwrap().into();
	let report = empty.validate(None).unwrap();
	assert!(report.empty && report.valid);
}

#[test]
fn repair_is_explicit_and_keeps_open_boundaries() {
	let solid = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let open = Shape::sew(solid.iter_face().take(5), 1e-6).unwrap();
	let report = open.repair(RepairOptions::default()).unwrap();
	assert!(report.before.valid && report.after.valid);
	assert!(!report.after.closed);
	assert_eq!(report.boundary_edges_before, report.boundary_edges_after);
	assert!(report.maximum_tolerance_after <= RepairOptions::default().max_tolerance);
	assert!(open.repair(RepairOptions { precision: f64::NAN, ..Default::default() }).is_err());
}

#[test]
fn boundary_errors_and_unsupported_inputs_are_typed() {
	assert!(matches!(Edge::circle(f64::NAN, DVec3::Z), Err(Error::Validation(_))));
	assert!(matches!(Edge::line(DVec3::ZERO, DVec3::INFINITY), Err(Error::Validation(_))));
	assert!(matches!(Solid::extrude([], DVec3::NAN), Err(Error::Validation(_))));
	let solid = Shape::from(&Solid::sphere(1.0));
	assert!(matches!(solid.thicken(0.1), Err(Error::Unsupported { .. })));
	let malformed = solid.repair(RepairOptions { max_tolerance: 1e-12, ..Default::default() });
	assert!(matches!(malformed, Err(Error::Validation(_))));
}

#[test]
fn legacy_native_boundaries_reject_nonfinite_parameters() {
	assert!(matches!(Solid::primitive(cadrum::Primitive::Sphere { radius: f64::NAN }), Err(Error::Validation(_))));
	assert!(matches!(Solid::primitive(cadrum::Primitive::Cylinder { radius: 1.0, height: DVec3::ZERO }), Err(Error::Validation(_))));
	assert!(matches!(Solid::primitive(cadrum::Primitive::Cone { radii: [1.0, 1.0], height: DVec3::Z }), Err(Error::Operation { .. })));
	let solid = Solid::sphere(1.0);
	assert!(matches!(Solid::mesh([&solid], cadrum::Tessellation { deflection_linear: f64::INFINITY, ..Default::default() }), Err(Error::Validation(_))));
	assert!(matches!(solid.fillet_edges(f64::NAN, solid.iter_edge()), Err(Error::Validation(_))));
}

#[test]
fn healing_preserves_styles_through_native_replacements() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE).color("#336699"));
	let report = shape.repair(RepairOptions::default()).unwrap();
	let mesh = report.shape.mesh(Default::default()).unwrap();
	assert!(mesh.face_ids.iter().all(|id| mesh.colormap.contains_key(id)));
}
