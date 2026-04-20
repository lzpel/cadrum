//! Demo of `Solid::fillet_edges`:
//! - All 12 cube edges filleted uniformly (rounded cube)
//! - Only top 4 edges filleted (soft top, sharp base)
//! - Cylinder top circular edge filleted (coin shape)

use cadrum::{DVec3, Error, Solid};

fn rounded_cube() -> Result<Solid, Error> {
	let a = 8.0;
	let cube = Solid::cube(a, a, a);
	let edges: Vec<_> = cube.iter_edge().collect();
	cube.fillet_edges(1.2, edges)
}

fn soft_top_cube() -> Result<Solid, Error> {
	let a = 8.0;
	let cube = Solid::cube(a, a, a);
	// Top edges of a cube are straight segments — polyline approximation has
	// exactly 2 samples, both at z ≈ a. Filter on that.
	let top_edges: Vec<_> = cube
		.iter_edge()
		.filter(|e| e.approximation_segments(0.01).iter().all(|p| (p.z - a).abs() < 1e-3))
		.collect();
	cube.fillet_edges(1.5, top_edges)
}

fn coin() -> Result<Solid, Error> {
	let h = 2.0;
	let cyl = Solid::cylinder(6.0, DVec3::Z, h);
	// Circular edges on a cylinder live at z = 0 and z = h. Pick the top cap.
	let top_circle: Vec<_> = cyl
		.iter_edge()
		.filter(|e| e.approximation_segments(0.05).iter().all(|p| (p.z - h).abs() < 1e-3))
		.collect();
	cyl.fillet_edges(0.6, top_circle)
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let result = [
		rounded_cube()?.color("#d0a878"),
		soft_top_cube()?.color("#6fbf73").translate(DVec3::X * 12.0),
		coin()?.color("#0052ff").translate(DVec3::X * 24.0),
	];

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::write_step(&result, &mut f).expect("failed to write STEP");

	let mut f = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	cadrum::mesh(&result, 0.2).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), true, true, &mut f)).expect("failed to write SVG");

	println!("wrote {example_name}.step / {example_name}.svg");
	Ok(())
}
