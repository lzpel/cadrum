//! Demo of `Solid::fillet_edges`:
//! - All 12 cube edges filleted uniformly (rounded cube)
//! - Only top 4 edges filleted (soft top, sharp base)
//! - Cylinder top circular edge filleted (coin shape)

use cadrum::{DVec3, Error, Solid};

fn rounded_cube(size: f64) -> Result<Solid, Error> {
	let cube = Solid::cube(size, size, size);
	let radius = size * 0.2;
	cube.fillet_edges(radius, cube.iter_edge())
}

fn soft_top_cube(size: f64) -> Result<Solid, Error> {
	let cube = Solid::cube(size, size, size);
	let radius = size * 0.2;
	// Top edges: straight segments whose both endpoints lie at z ≈ size.
	let top_edges: Vec<_> = cube
		.iter_edge()
		.filter(|e| (e.start_point().z - size).abs() < 1e-6 && (e.end_point().z - size).abs() < 1e-6)
		.collect();
	cube.fillet_edges(radius, top_edges)
}

fn coin(radius: f64, height: f64) -> Result<Solid, Error> {
	let cyl = Solid::cylinder(radius, DVec3::Z, height);
	// Top cap boundary: a closed circular edge whose start == end lives at z = h.
	let top_circle: Vec<_> = cyl
		.iter_edge()
		.filter(|e| (e.start_point().z - height).abs() < 1e-6)
		.collect();
	cyl.fillet_edges(height * 0.3, top_circle)
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let result = [
		rounded_cube(8.0)?.color("#d0a878"),
		soft_top_cube(8.0)?.color("#6fbf73").translate(DVec3::X * 12.0),
		coin(6.0, 2.0)?.color("#0052ff").translate(DVec3::X * 24.0),
	];

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::write_step(&result, &mut f).expect("failed to write STEP");

	let mut f = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	cadrum::mesh(&result, 0.2).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), true, true, &mut f)).expect("failed to write SVG");

	println!("wrote {example_name}.step / {example_name}.svg");
	Ok(())
}
