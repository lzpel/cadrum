use cadrum::{DVec3, Error, OperationControl, Shape, Solid, Tessellation};
#[test]
fn uv_mesh_and_reflected_normals_preserve_geometry() {
	let shape = Shape::from(&Solid::cylinder(2.0, DVec3::Z * 5.0)).mirror(DVec3::ZERO, DVec3::X).unwrap();
	let mut before = Vec::new();
	shape.write_brep(&mut before).unwrap();
	for _ in 0..3 {
		let result = shape.tessellate(Tessellation::default(), &OperationControl::default()).unwrap();
		let data = result.value;
		assert_eq!(data.uvs.len(), data.mesh.vertices.len());
		assert!(data.uvs.iter().all(Option::is_some));
		for t in data.mesh.indices.chunks_exact(3) {
			let [a, b, c] = [t[0], t[1], t[2]];
			let geometric = (data.mesh.vertices[b] - data.mesh.vertices[a]).cross(data.mesh.vertices[c] - data.mesh.vertices[a]);
			assert!(geometric.dot(data.mesh.normals[a] + data.mesh.normals[b] + data.mesh.normals[c]) > 0.0);
		}
	}
	let mut after = Vec::new();
	shape.write_brep(&mut after).unwrap();
	let original = Shape::read_brep(&mut before.as_slice()).unwrap();
	let restored = Shape::read_brep(&mut after.as_slice()).unwrap();
	assert_eq!(original.kind(), restored.kind());
	assert_eq!(original.faces().unwrap().len(), restored.faces().unwrap().len());
	assert!((original.as_solid().unwrap().volume() - restored.as_solid().unwrap().volume()).abs() < 1e-9);
	assert_eq!(original.bounding_box().unwrap(), restored.bounding_box().unwrap());
	let control = OperationControl::default();
	control.cancellation_token().cancel();
	assert!(matches!(shape.tessellate(Tessellation::default(), &control), Err(Error::Cancelled { .. })));
}
#[test]
fn open_and_disconnected_step_geometry_round_trips() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let open = Shape::sew(cube.iter_face().take(5), 1e-7).unwrap();
	let another = Shape::from(&Solid::sphere(1.0).translate(DVec3::splat(5.0)));
	let mixed = Shape::compound([&open, &another]).unwrap();
	let mut bytes = Vec::new();
	mixed.write_step(&mut bytes, &OperationControl::default()).unwrap();
	let restored = Shape::read_step(&mut bytes.as_slice(), &OperationControl::default()).unwrap().value;
	assert_eq!(restored.faces().unwrap().len(), 6);
	assert_eq!(restored.solids().unwrap().len(), 1);
	assert!(!restored.shells().unwrap().is_empty());
}
#[test]
fn writer_and_reader_failures_remain_io_errors() {
	struct Broken;
	impl std::io::Write for Broken {
		fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
			Err(std::io::Error::other("write rejected"))
		}
		fn flush(&mut self) -> std::io::Result<()> {
			Ok(())
		}
	}
	let shape = Shape::from(&Solid::sphere(1.0));
	assert!(matches!(shape.write_step(&mut Broken, &OperationControl::default()), Err(Error::Io(_))));
	assert!(matches!(shape.write_brep(&mut Broken), Err(Error::Io(_))));
}
