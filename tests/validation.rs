use cadrum::{DVec3, Shape, ShapeKind, Solid};

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
