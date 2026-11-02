use cadrum::{DVec3, Error, OperationControl, SceneOption, Shape, ShapeKind, Solid, Tessellation};
use std::path::PathBuf;

fn main() -> Result<(), Error> {
	let output = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("figure/native_features"));
	std::fs::create_dir_all(&output)?;
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color("#c04020"));
	let faces = shape.faces()?;
	let top = faces.iter().max_by(|a, b| a.center().z.total_cmp(&b.center().z)).unwrap();
	let open = shape.remove_faces([top], 1e-6)?;
	let report = shape.validate(Some(ShapeKind::Solid))?;
	assert!(report.valid && report.category_matches);
	let first = shape.copied()?;
	let second = first.value.copied()?;
	assert!(first.history.compose(&second.history, 0).complete);
	let displayed_copy = Shape::from(&second.value.as_solid()?.translate(DVec3::X * 13.0));
	let copies = Shape::compound([&shape, &displayed_copy])?;
	let options = Tessellation { deflection_linear: 0.004, deflection_angular: 0.2, ..Default::default() };
	let sphere = Shape::from(&Solid::sphere(2.0)).tessellate(options, &OperationControl::default())?;
	assert!(sphere.value.uvs.iter().all(Option::is_some));
	let previews = [("01-open-topology", open.mesh(options)?), ("02-validation", shape.mesh(options)?), ("07-copy-history", copies.mesh(options)?), ("10-tessellation", sphere.value.mesh)];
	for (name, mesh) in previews {
		let scene = mesh.scene(SceneOption { view: DVec3::ONE, shading: true, hidden_edges: false, ..Default::default() });
		let mut bytes = Vec::new();
		scene.write_png([640, 480], &mut bytes)?;
		let image = tiny_skia::Pixmap::decode_png(&bytes).map_err(|e| Error::Io(std::io::Error::other(e)))?;
		let mut background = tiny_skia::Pixmap::new(640, 480).unwrap();
		background.fill(tiny_skia::Color::WHITE);
		background.draw_pixmap(0, 0, image.as_ref(), &Default::default(), Default::default(), None);
		let path = output.join(format!("{name}.png"));
		background.save_png(&path).map_err(|e| Error::Io(std::io::Error::other(e)))?;
		println!("{}", path.display());
	}
	Ok(())
}
