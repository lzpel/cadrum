use cadrum::{ChamferKind, ChamferSpec, DVec3, Edge, Face, OperationControl, PlanarChamfer, PointLocation, RelationKind, Shape, ShapeKind, Solid, VariableFillet, Wire};
fn square(a: f64, b: f64) -> Wire {
	Wire::new(&Edge::polygon(&[DVec3::new(a, a, 0.0), DVec3::new(b, a, 0.0), DVec3::new(b, b, 0.0), DVec3::new(a, b, 0.0)]).unwrap()).unwrap()
}
#[test]
fn unequal_and_angle_chamfers_respect_reference_face() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(10.0)));
	let edges = shape.edges().unwrap();
	let edge = edges.iter().find(|e| e.start_point().x.abs() + e.start_point().y.abs() + e.end_point().x.abs() + e.end_point().y.abs() < 1e-8).unwrap();
	let faces = shape.ancestors(&Shape::from(edge), ShapeKind::Face).unwrap();
	let x = faces.iter().map(|s| s.as_face().unwrap()).find(|f| f.center().x.abs() < 1e-8).unwrap();
	let y = faces.iter().map(|s| s.as_face().unwrap()).find(|f| f.center().y.abs() < 1e-8).unwrap();
	let kind = ChamferKind::TwoDistances { first: 1.0, second: 2.0 };
	let a = shape.chamfer(&[ChamferSpec { edge, reference_face: &x, kind }], &OperationControl::default()).unwrap().value;
	let b = shape.chamfer(&[ChamferSpec { edge, reference_face: &y, kind }], &OperationControl::default()).unwrap().value;
	assert!((a.solids().unwrap()[0].volume() - 990.0).abs() < 1e-7);
	assert_eq!(a.classify_point(DVec3::new(0.1, 1.1, 5.0), 1e-7).unwrap(), PointLocation::Inside);
	assert_eq!(b.classify_point(DVec3::new(0.1, 1.1, 5.0), 1e-7).unwrap(), PointLocation::Outside);
	let angle = shape.chamfer(&[ChamferSpec { edge, reference_face: &x, kind: ChamferKind::DistanceAngle { distance: 1.0, angle: std::f64::consts::FRAC_PI_4 } }], &OperationControl::default()).unwrap().value;
	assert!((angle.solids().unwrap()[0].volume() - 995.0).abs() < 1e-7);
}
#[test]
fn radius_laws_generate_native_blend_faces() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(10.0)));
	let edges = shape.edges().unwrap();
	let result = shape.variable_fillet(&[VariableFillet { edge: &edges[0], radii: vec![[0.0, 0.2], [0.5, 0.5], [1.0, 0.8]] }], &OperationControl::default()).unwrap();
	assert!(result.value.is_valid().unwrap());
	assert!(result.history.relations.iter().any(|r| r.relation == RelationKind::Generated));
	assert!(shape.variable_fillet(&[VariableFillet { edge: &edges[0], radii: vec![[0.0, 100.0], [1.0, 100.0]] }], &OperationControl::default()).is_err());
}
#[test]
fn planar_corners_include_holes_and_return_history() {
	let face = Face::from_wires(&square(0.0, 10.0), &[&square(2.0, 4.0)], 1e-7).unwrap();
	let shape = Shape::from(&face);
	let vertices = shape.vertices().unwrap();
	let hole = vertices.iter().find(|v| v.point().unwrap().distance(DVec3::new(2.0, 2.0, 0.0)) < 1e-8).unwrap();
	let fillet = face.fillet_corners(0.2, &[hole]).unwrap();
	assert!(fillet.value.is_valid().unwrap());
	assert_eq!(fillet.value.faces().unwrap()[0].boundary_wires().unwrap().len(), 2);
	assert!(fillet.history.relations.iter().any(|r| r.relation == RelationKind::Generated));
	let edges = face.iter_edge().collect::<Vec<_>>();
	let first = edges.iter().find(|e| e.start_point().distance(DVec3::ZERO) < 1e-8 || e.end_point().distance(DVec3::ZERO) < 1e-8).unwrap();
	let second = edges.iter().find(|e| e.id() != first.id() && (e.start_point().distance(DVec3::ZERO) < 1e-8 || e.end_point().distance(DVec3::ZERO) < 1e-8)).unwrap();
	assert!(face.chamfer_corners(&[PlanarChamfer { first_edge: first, second_edge: second, first_distance: 0.3, second_distance: 0.5 }]).unwrap().value.is_valid().unwrap());
}
