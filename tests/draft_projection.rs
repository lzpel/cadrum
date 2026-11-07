use cadrum::{DVec3, DraftOptions, Edge, Face, Frame, MultipleResults, NoHitPolicy, OperationControl, Plane, Projection, ProjectionOptions, Shape, Solid, Wire};
fn square(z: f64, size: f64) -> Face {
	let wire = Wire::new(&Edge::polygon(&[DVec3::new(0.0, 0.0, z), DVec3::new(size, 0.0, z), DVec3::new(size, size, z), DVec3::new(0.0, size, z)]).unwrap()).unwrap();
	Face::from_wires(&wire, &[], 1e-7).unwrap()
}
#[test]
fn signed_draft_and_tapered_extrusion() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(5.0)));
	let faces = shape.faces().unwrap();
	let side = faces.iter().find(|f| (f.center().x - 5.0).abs() < 1e-8).unwrap();
	for angle in [0.05, -0.05] {
		let result = shape.draft(&[side], DraftOptions { neutral_plane: Plane { origin: DVec3::ZERO, normal: DVec3::Z }, pull_direction: DVec3::Z, angle, flag: true }, &OperationControl::default()).unwrap();
		assert!(result.result.value.is_valid().unwrap());
		assert_eq!(result.face_status[0].status, 0);
		assert!(!result.result.history.relations.is_empty());
	}
	let tapered = square(0.0, 5.0).tapered_extrude(DVec3::Z * 5.0, 0.05, &OperationControl::default()).unwrap();
	assert!(tapered.result.value.is_valid().unwrap());
	assert!((tapered.result.value.solids().unwrap()[0].volume() - 125.0).abs() > 1.0);
	let sphere = Shape::from(&Solid::sphere(2.0));
	assert!(sphere.draft(&[&sphere.faces().unwrap()[0]], DraftOptions { neutral_plane: Plane { origin: DVec3::ZERO, normal: DVec3::Z }, pull_direction: DVec3::Z, angle: 0.1, flag: true }, &OperationControl::default()).is_err());
}
#[test]
fn planar_projection_clipping_and_no_hit_policies() {
	let target = Shape::from(&square(0.0, 5.0));
	let edge = Edge::line(DVec3::new(-1.0, 2.0, 3.0), DVec3::new(6.0, 2.0, 3.0)).unwrap();
	let clipped = edge.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap();
	assert!((clipped.value.edges().unwrap()[0].length(1e-8).unwrap() - 5.0).abs() < 1e-8);
	let full = edge.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions { untrimmed: true, ..Default::default() }).unwrap();
	assert!((full.value.edges().unwrap()[0].length(1e-8).unwrap() - 7.0).abs() < 1e-8);
	let miss = Edge::line(DVec3::new(10.0, 2.0, 3.0), DVec3::new(11.0, 2.0, 3.0)).unwrap();
	assert!(miss.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap().value.is_empty().unwrap());
	assert!(miss.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions { no_hit: NoHitPolicy::Error, ..Default::default() }).is_err());
}
#[test]
fn curved_multi_hit_and_point_projection() {
	let cylinder = Shape::from(&Solid::cylinder(2.0, DVec3::Z * 10.0));
	let wire: Shape = Wire::new([&Edge::ellipse(Frame { origin: DVec3::new(-5.0, 0.0, 5.0), axis_z: DVec3::X, axis_x: DVec3::Y }, 0.5, 0.5, None).unwrap()]).unwrap().into();
	let result = wire.project_onto(&cylinder, Projection::Directional(DVec3::X), ProjectionOptions::default()).unwrap();
	assert!(result.value.wires().unwrap().len() >= 2);
	assert!(wire.project_onto(&cylinder, Projection::Directional(DVec3::X), ProjectionOptions { multiple: MultipleResults::Reject, ..Default::default() }).is_err());
	let circle: Shape = Wire::new([&Edge::ellipse(Frame { origin: DVec3::new(2.5, 2.5, 2.0), ..Default::default() }, 0.5, 0.5, None).unwrap()]).unwrap().into();
	let target = Shape::from(&square(0.0, 5.0));
	let projected = circle.project_onto(&target, Projection::FromPoint(DVec3::new(2.5, 2.5, 5.0)), ProjectionOptions::default()).unwrap();
	assert!(!projected.value.is_empty().unwrap());
}
