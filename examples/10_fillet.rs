//! Demo of `Solid::fillet_edges`:
//! - All 12 cube edges filleted uniformly (rounded cube)
//! - Only top 4 edges filleted (soft top, sharp base)
//! - Cylinder top circular edge filleted (coin shape)

use cadrum::{DVec3, Error, Solid};

fn rounded_cube(size: f64) -> Result<Solid, Error> {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(size)).translate(-DVec3::ONE * (size / 2.0));
	let radius = size * 0.2;
	cube.fillet_edges(radius, cube.iter_edge())
}

fn soft_top_cube(size: f64) -> Result<Solid, Error> {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(size)).translate(-DVec3::ONE * (size / 2.0));
	let radius = size * 0.2;
	// Top cap boundary: a closed circular edge whose start == end lives at z = h.
	let top_edges = cube
		.iter_edge()
		.filter(|e| [e.start_point(), e.end_point()].iter().all(|p| (p.z - size / 2.0).abs() < 1e-6));
	cube.fillet_edges(radius, top_edges)
}

fn coin(radius: f64, height: f64) -> Result<Solid, Error> {
	let cyl = Solid::cylinder(radius, DVec3::Z * height);
	let radius = height * 0.3;
	// Top cap boundary: a closed circular edge whose start == end lives at z = h.
	let top_circle = cyl
		.iter_edge()
		.filter(|e| [e.start_point(), e.end_point()].iter().all(|p| (p.z - height).abs() < 1e-6));
	cyl.fillet_edges(radius, top_circle)
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let result = [
		rounded_cube(8.0)?.color("#d0a878"),
		soft_top_cube(8.0)?.color("#6fbf73").translate(DVec3::X * 12.0),
		coin(4.0, 2.0)?.color("#0052ff").translate(DVec3::X * 24.0),
	];

	Solid::write_step(&result, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

	let scene = Solid::mesh(&result, Default::default())?.scene(DVec3::new(1.0, 1.0, 2.0), DVec3::Z, true, true);
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;
	let mesh = Solid::mesh(&result, Default::default())?;
	mesh.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;
	mesh.write_gltf_binary(&mut std::fs::File::create(format!("{example_name}.glb")).unwrap())?;

	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
	Ok(())
}
