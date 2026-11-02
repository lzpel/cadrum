use cadrum::{DVec3, Shape, ShapeKind, Solid, Tessellation};

fn main() -> Result<(), cadrum::Error> {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let shape = Shape::from(&cube);
	let faces = shape.faces()?;
	let open = shape.remove_faces([&faces[0]], 1e-6)?;
	assert_eq!(open.kind(), ShapeKind::Shell);
	assert!(open.to_solid().is_err());
	let mesh = open.mesh(Tessellation::default())?;
	let mut bytes = Vec::new();
	open.write_brep(&mut bytes)?;
	let restored = Shape::read_brep(&mut bytes.as_slice())?;
	assert_eq!(restored.faces()?.len(), 5);
	println!("{} triangles; {} BRep bytes; valid {}", mesh.indices.len() / 3, bytes.len(), restored.validate(None)?.valid);
	Ok(())
}
