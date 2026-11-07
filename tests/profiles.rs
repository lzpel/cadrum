use cadrum::{DVec3, Edge, Face, Frame, RelationKind, Shape, Solid, Wire};
fn circle(r: f64) -> Wire {
	Wire::new([&Edge::circle(r, DVec3::Z).unwrap()]).unwrap()
}
fn square(a: f64, b: f64) -> Wire {
	Wire::new(&Edge::polygon(&[DVec3::new(a, a, 0.0), DVec3::new(b, a, 0.0), DVec3::new(b, b, 0.0), DVec3::new(a, b, 0.0)]).unwrap()).unwrap()
}
#[test]
fn annulus_extrudes_and_preserves_edge_provenance() {
	let outer = circle(3.0);
	let inner = circle(1.0);
	let face = Face::from_wires(&outer, &[&inner], 1e-7).unwrap();
	assert!((face.area() - 8.0 * std::f64::consts::PI).abs() < 1e-8);
	let profile = Shape::from(&face);
	let result = profile.extrude(DVec3::new(1.0, 2.0, 5.0)).unwrap();
	assert!((result.value.as_solid().unwrap().volume() / (40.0 * std::f64::consts::PI) - 1.0).abs() < 1e-6, "volume {}", result.value.as_solid().unwrap().volume());
	assert!(result.history.relations.iter().any(|r| r.relation == RelationKind::Generated && r.source.kind == cadrum::ShapeKind::Edge && r.target.unwrap().kind == cadrum::ShapeKind::Face));
	let scaled = profile.scale_xyz(DVec3::ZERO, DVec3::new(2.0, 3.0, 1.0)).unwrap().value;
	assert!((scaled.faces().unwrap()[0].area() - 48.0 * std::f64::consts::PI).abs() < 1e-4);
}
#[test]
fn nesting_disconnected_regions_and_reversed_winding() {
	let a = square(0.0, 10.0);
	let b = square(1.0, 9.0);
	let c = square(2.0, 8.0);
	let d = square(20.0, 22.0);
	let result = Shape::profile(&[&a, &b, &c, &d], 1e-7).unwrap();
	assert!(result.history.complete);
	for operand in 0..4 {
		let edges: Vec<_> = result.history.relations.iter().filter(|r| r.source.operand == operand && r.source.kind == cadrum::ShapeKind::Edge).collect();
		assert!(edges.iter().any(|r| r.relation == RelationKind::Unchanged));
		assert!(edges.iter().any(|r| r.relation == RelationKind::Generated && r.target.unwrap().kind == cadrum::ShapeKind::Face));
	}
	let p = result.value;
	assert_eq!(p.faces().unwrap().len(), 3);
	assert!((p.extrude(DVec3::Z).unwrap().value.solids().unwrap().iter().map(Solid::volume).sum::<f64>() - 76.0).abs() < 1e-7);
	let rev = Wire::new(&Edge::polygon(&[DVec3::ZERO, DVec3::Y, DVec3::ONE * DVec3::new(1.0, 1.0, 0.0), DVec3::X]).unwrap()).unwrap();
	assert!((Shape::profile(&[&rev], 1e-7).unwrap().value.faces().unwrap()[0].area() - 1.0).abs() < 1e-8);
	assert!(Face::from_wires(&a, &[&d], 1e-7).is_err());
}
#[test]
fn partial_revolution_and_invalid_contours() {
	let disc = Wire::new([&Edge::ellipse(Frame { origin: DVec3::X * 4.0, axis_z: DVec3::Y, axis_x: DVec3::X }, 1.0, 1.0, None).unwrap()]).unwrap();
	let profile = Shape::from(&Face::from_wires(&disc, &[], 1e-7).unwrap());
	let positive = profile.revolve(DVec3::ZERO, DVec3::Z, std::f64::consts::PI).unwrap().value.as_solid().unwrap().volume();
	let negative = profile.revolve(DVec3::ZERO, -DVec3::Z, -std::f64::consts::PI).unwrap().value.as_solid().unwrap().volume();
	assert!((positive - 4.0 * std::f64::consts::PI.powi(2)).abs() < 1e-6);
	assert!((positive - negative).abs() < 1e-8);
	let open = Wire::new([&Edge::line(DVec3::ZERO, DVec3::X).unwrap()]).unwrap();
	assert!(Shape::profile(&[&open], 1e-7).is_err());
	let crossing = Wire::new(&Edge::polygon(&[DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0), DVec3::Y, DVec3::X]).unwrap()).unwrap();
	assert!(Shape::profile(&[&crossing], 1e-7).is_err());
	assert!(Shape::profile(&[&circle(1.0), &circle(1.0)], 1e-7).is_err());
}
