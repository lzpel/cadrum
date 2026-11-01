use cadrum::*;

fn require_send<T: Send>() {}

#[test]
fn native_owners_and_results_are_send() {
	require_send::<Solid>();
	require_send::<Face>();
	require_send::<Edge>();
	require_send::<Shape>();
	require_send::<Vertex>();
	require_send::<Wire>();
	require_send::<Shell>();
	require_send::<Compound>();
	require_send::<OperationResult<Shape>>();
	require_send::<RepairReport>();
	require_send::<PlaneSplit>();
	require_send::<SweepResult>();
	require_send::<DraftResult>();
	require_send::<FillingResult>();
	require_send::<MeshImportResult>();
	#[cfg(feature = "step-document")]
	require_send::<StepDocument>();
}

#[test]
fn solids_can_be_transferred_to_workers_and_returned() {
	let solid = Solid::cube(DVec3::ZERO, DVec3::splat(2.0));
	let solid = std::thread::spawn(move || {
		assert!((solid.volume() - 8.0).abs() < 1e-8);
		solid.translate(DVec3::X * 3.0)
	})
	.join()
	.unwrap();
	assert!((solid.volume() - 8.0).abs() < 1e-8);
	let shape = Shape::from(&solid);
	let mesh = std::thread::spawn(move || shape.mesh(Default::default()).unwrap()).join().unwrap();
	assert!(!mesh.indices.is_empty());
}

#[test]
fn typed_topology_and_operation_results_can_live_on_a_worker() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let face = shape.faces().unwrap().remove(0);
	let edge = shape.edges().unwrap().remove(0);
	let vertex = Vertex::new(DVec3::ZERO).unwrap();
	let wire = Wire::new([&Edge::circle(1.0, DVec3::Z).unwrap()]).unwrap();
	let shell = shape.shells().unwrap().remove(0);
	let compound = Shape::compound([&shape]).unwrap();
	let copied = std::thread::spawn(move || {
		assert!(face.area() > 0.0);
		assert!(edge.length(1e-7).unwrap() > 0.0);
		assert_eq!(vertex.point().unwrap(), DVec3::ZERO);
		assert_eq!(wire.kind(), ShapeKind::Wire);
		assert_eq!(shell.kind(), ShapeKind::Shell);
		assert_eq!(compound.solids().unwrap().len(), 1);
		shape.copied().unwrap()
	})
	.join()
	.unwrap();
	assert!(copied.value.is_valid().unwrap());
	assert!(copied.history.complete);
}
