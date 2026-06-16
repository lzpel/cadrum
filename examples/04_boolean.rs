//! Boolean operations: union, subtract, and intersect between a box and a cylinder.

use cadrum::{Boolean, DVec3, Solid};

fn main() -> Result<(), cadrum::Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let make_box = Solid::cube(DVec3::ZERO, DVec3::splat(20.0)).translate(DVec3::X * -10. + DVec3::Y * -10.).color("#4a90d9");
	let make_cyl = Solid::cylinder(8.0, DVec3::Z * 30.0).translate(DVec3::Z * -5.).color("#e67e22");

	// union: merge both shapes into one — offset X=0
	let union: Solid = (&make_box + &make_cyl).build()?;

	// subtract: box minus cylinder — offset X=40
	let subtract: Solid = (&make_box - &make_cyl).build()?;

	// intersect: only the overlapping volume — offset X=80
	let intersect: Solid = (&make_box * &make_cyl).build()?;

	let cylinder = Solid::cylinder(8.0, DVec3::Z * 30.0).translate(DVec3::X * 4.);
	let [cylinder0, cylinder1, cylinder2] = [cylinder.clone(), cylinder.clone().rotate_z(std::f64::consts::TAU / 3.), cylinder.clone().rotate_z(-std::f64::consts::TAU / 3.)];

	// union of all cylinders (fold from Boolean::default() = ⊥)
	let sum: Solid = [&cylinder0, &cylinder1, &cylinder2].into_iter().map(Boolean::from).reduce(|a, s| a + s).unwrap().build()?;
	let sum = sum.color("#d875ff");

	// intersection of all cylinders (reduce — intersect has no fixed init)
	let product: Solid = [&cylinder0, &cylinder1, &cylinder2].into_iter().map(Boolean::from).reduce(|a, b| a * b).unwrap().build()?;
	let product = product.color("#00ff22");

	let shapes = [union.translate(DVec3::X * 0.0), subtract.translate(DVec3::X * 40.0), intersect.translate(DVec3::X * 80.0), sum.translate(DVec3::X * 20.0 + DVec3::Y * 40.0), product.translate(DVec3::X * 60.0 + DVec3::Y * 40.0)];

	Solid::write_step(&shapes, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

	let mesh = Solid::mesh(&shapes, Default::default())?;
	let scene = mesh.scene(cadrum::SceneOption { view: DVec3::new(1.0, 1.0, 2.0), ..Default::default() });
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;
	mesh.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;
	mesh.write_gltf_binary(&mut std::fs::File::create(format!("{example_name}.glb")).unwrap())?;

	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
	Ok(())
}
