use cadrum::{DVec3, Edge, Face, Frame, LoftOptions, OperationControl, RepairOptions, Shape, ShapeKind, Solid, Vertex, Wire};
fn ring(z: f64, r: f64) -> Wire {
	Wire::new([&Edge::ellipse(Frame { origin: DVec3::Z * z, ..Default::default() }, r, r, None).unwrap()]).unwrap()
}
#[test]
fn lofts_support_shells_and_point_ends() {
	let a: Shape = ring(0.0, 2.0).into();
	let b: Shape = ring(3.0, 1.0).into();
	let ruled = Shape::loft(&[&a, &b], LoftOptions { ruled: true, ..Default::default() }, &OperationControl::default()).unwrap();
	assert!((ruled.value.as_solid().unwrap().volume() - 7.0 * std::f64::consts::PI).abs() < 1e-5);
	assert!(!ruled.history.relations.is_empty());
	let shell = Shape::loft(&[&a, &b], LoftOptions { solid: false, ..Default::default() }, &OperationControl::default()).unwrap().value;
	assert_eq!(shell.kind(), ShapeKind::Shell);
	assert!(!shell.is_closed().unwrap());
	let tip: Shape = Vertex::new(DVec3::Z * 3.0).unwrap().into();
	let cone = Shape::loft(&[&a, &tip], LoftOptions::default(), &OperationControl::default()).unwrap();
	assert!((cone.value.as_solid().unwrap().volume() - 4.0 * std::f64::consts::PI).abs() < 1e-5);
}
#[test]
fn corresponding_holes_and_disconnected_regions_remain_native() {
	let a = Shape::from(&Face::from_wires(&ring(0.0, 3.0), &[&ring(0.0, 1.0)], 1e-7).unwrap());
	let b = Shape::from(&Face::from_wires(&ring(5.0, 3.0), &[&ring(5.0, 1.0)], 1e-7).unwrap());
	let result = Shape::loft(&[&a, &b], LoftOptions { ruled: true, ..Default::default() }, &OperationControl::default()).unwrap();
	assert!((result.value.solids().unwrap()[0].volume() - 40.0 * std::f64::consts::PI).abs() < 1e-5);
	assert!(!result.history.complete);
	let other_a = a.translate(DVec3::X * 10.0).unwrap();
	let other_b = b.translate(DVec3::X * 10.0).unwrap();
	let aa = Shape::compound([&a, &other_a]).unwrap();
	let bb = Shape::compound([&b, &other_b]).unwrap();
	assert_eq!(Shape::loft(&[&aa, &bb], LoftOptions { ruled: true, ..Default::default() }, &OperationControl::default()).unwrap().value.solids().unwrap().len(), 2);
	let no_hole = Shape::from(&Face::from_wires(&ring(5.0, 3.0), &[], 1e-7).unwrap());
	assert!(Shape::loft(&[&a, &no_hole], LoftOptions::default(), &OperationControl::default()).is_err());
}
#[test]
fn healing_reports_bounded_sampled_changes() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let open = Shape::sew(cube.iter_face().take(5), 1e-7).unwrap();
	let report = open.repair(RepairOptions::default()).unwrap();
	assert!(report.sample_count > 0);
	assert!(report.maximum_sampled_deviation < 1e-6);
	assert!(!report.after.closed);
	assert!(open.repair(RepairOptions { precision: 1e-14, max_tolerance: 1e-12, ..Default::default() }).is_err());
}
