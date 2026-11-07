use cadrum::{DVec3, Shape, ShapeKind, Solid, Tessellation};

fn main() -> Result<(), cadrum::Error> {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let shape = Shape::from(&cube);
	let faces = shape.faces()?;
	let open = shape.remove_faces([&faces[0]], 1e-6)?;
	assert_eq!(open.kind(), ShapeKind::Shell);
	assert!(open.to_solid().is_err());
	let placed = open.translate(DVec3::new(2.0, 3.0, 4.0))?;
	let mesh = placed.mesh(Tessellation::default())?;
	let mut bytes = Vec::new();
	placed.write_brep(&mut bytes)?;
	let restored = Shape::read_brep(&mut bytes.as_slice())?;
	let thick = restored.thicken(0.5)?.as_solid()?;
	println!("{} triangles; thickened volume {}", mesh.indices.len() / 3, thick.volume());
	Ok(())
}
