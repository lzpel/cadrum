//! Primitive solids: box, cylinder, sphere, cone, torus — colored and exported as STEP + SVG.

use cadrum::{DVec3, Solid};

fn main() -> Result<(), cadrum::Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let solids = [Solid::cube(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0)).color("#4a90d9"), Solid::cylinder(8.0, DVec3::Z * 30.0).translate(DVec3::X * 30.0).color("#e67e22"), Solid::sphere(8.0).translate(DVec3::X * 60.0 + DVec3::Z * 15.0).color("#2ecc71"), Solid::cone(8.0, 1.0, DVec3::Z * 30.0).translate(DVec3::X * 90.0).color("#e74c3c"), Solid::torus(12.0, 4.0, DVec3::Z).translate(DVec3::X * 130.0 + DVec3::Z * 15.0).color("#9b59b6")];

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
