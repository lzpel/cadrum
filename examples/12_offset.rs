//! Signed surface offset: one drilled block grown by +3, the original, and shrunk by -3 side by side — every face moves along its normal, so the hole shrinks as the body grows.

use cadrum::{DVec3, Solid};

fn main() -> Result<(), cadrum::Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let block = Solid::cube(DVec3::ZERO, DVec3::new(24.0, 40.0, 16.0));
	let hole = Solid::cylinder(5.0, DVec3::Z * 16.0).translate(DVec3::new(12.0, 20.0, 0.0));
	let part = (&block - &hole).build()?.color("#4a90d9");

	// offset > 0 grows outward (hole tightens), offset < 0 shrinks inward (hole widens).
	let grown = part.offset(3.0, part.iter_face(), 1.0e-6)?.translate(-DVec3::X * 50.0).color("#e67e22");
	let shrunk = part.offset(-3.0, part.iter_face(), 1.0e-6)?.translate(DVec3::X * 50.0).color("#2ecc71");

	let solids = [grown, part, shrunk];
	Solid::write_step(&solids, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

	let mesh = Solid::mesh(&solids, Default::default())?;
	let scene = mesh.scene(Default::default());
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;
	mesh.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;
	mesh.write_gltf_binary(&mut std::fs::File::create(format!("{example_name}.glb")).unwrap())?;

	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
	Ok(())
}
