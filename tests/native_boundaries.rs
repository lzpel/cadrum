use cadrum::{DVec3, Error, OperationControl, Shape, Solid, Tessellation};
#[test]
fn uv_mesh_and_reflected_normals_preserve_geometry() {
	let shape = Shape::from(&Solid::cylinder(2.0, DVec3::Z * 5.0).mirror(DVec3::ZERO, DVec3::X));
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
fn brep_boundary_io_errors_are_preserved() {
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
	assert!(matches!(shape.write_brep(&mut Broken), Err(Error::Io(_))));
}

#[test]
fn native_mesh_callbacks_cancel_and_contain_panics() {
	let shape = Shape::from(&Solid::sphere(2.0));
	let cancel = OperationControl::with_progress(|_| false);
	assert!(matches!(shape.tessellate(Tessellation::default(), &cancel), Err(Error::Cancelled { .. })));
	let panic = OperationControl::with_progress(|_| panic!("mesh progress failure"));
	assert!(matches!(shape.tessellate(Tessellation::default(), &panic), Err(Error::Operation { .. })));
	assert!(!shape.mesh(Tessellation::default()).unwrap().indices.is_empty());
}

#[test]
fn active_mesh_control_rejects_reuse_without_cancelling_its_owner() {
	let (started_tx, started_rx) = std::sync::mpsc::channel();
	let (release_tx, release_rx) = std::sync::mpsc::channel();
	let mut first = true;
	let control = std::sync::Arc::new(OperationControl::with_progress(move |_| {
		if first {
			first = false;
			started_tx.send(()).unwrap();
			release_rx.recv().unwrap();
		}
		true
	}));
	let worker_control = control.clone();
	let worker = std::thread::spawn(move || Shape::from(&Solid::sphere(2.0)).tessellate(Tessellation::default(), &worker_control));
	started_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
	let shape = Shape::from(&Solid::sphere(1.0));
	let reuse = shape.tessellate(Tessellation::default(), &control);
	release_tx.send(()).unwrap();
	assert!(matches!(reuse, Err(Error::Operation { .. })));
	assert!(worker.join().unwrap().is_ok());
	assert!(shape.tessellate(Tessellation::default(), &control).is_ok());
}

#[test]
fn curve_only_mesh_scenes_retain_polyline_bounds() {
	let edge = cadrum::Edge::line(DVec3::new(10.0, 20.0, 0.0), DVec3::new(12.0, 23.0, 0.0)).unwrap();
	let mesh = Shape::from(&edge).mesh(Tessellation::default()).unwrap();
	assert!(mesh.indices.is_empty());
	let scene = mesh.scene(cadrum::SceneOption { view: DVec3::Z, up: DVec3::Y, ..Default::default() });
	assert!(!scene.edges_visible.is_empty());
	let [min, max] = scene.viewbox();
	assert!((max.x - min.x - 2.0).abs() < 1e-8);
	assert!((max.y - min.y - 3.0).abs() < 1e-8);
}
