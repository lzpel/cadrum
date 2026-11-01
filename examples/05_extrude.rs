//! Demo of `Solid::extrude`: push a closed 2D profile along a direction vector.

use cadrum::{BSplineEnd, DVec3, Edge, Error, Solid};

/// Circle extruded at a steep angle → oblique cylinder.
fn build_oblique_cylinder() -> Result<Solid, Error> {
	let profile = [Edge::circle(3.0, DVec3::Z)?];
	Solid::extrude(&profile, DVec3::new(-4.0, -6.0, 8.0))
}

/// L-shaped polygon → L-beam.
fn build_l_beam() -> Result<Solid, Error> {
	let profile = Edge::polygon(&[DVec3::new(0.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 0.0), DVec3::new(4.0, 1.0, 0.0), DVec3::new(1.0, 1.0, 0.0), DVec3::new(1.0, 3.0, 0.0), DVec3::new(0.0, 3.0, 0.0)])?;
	Solid::extrude(&profile, DVec3::Z * 12.0)
}

/// Square plate with a round bore: `Edge::loops` splits the profile, the first
/// loop bounds the solid and the second becomes the hole.
fn build_plate_with_bore() -> Result<Solid, Error> {
	let outer = Edge::polygon(&[DVec3::new(-4.0, -3.0, 0.0), DVec3::new(4.0, -3.0, 0.0), DVec3::new(4.0, 3.0, 0.0), DVec3::new(-4.0, 3.0, 0.0)])?;
	let bore = Edge::circle(1.5, DVec3::Z)?;
	Solid::extrude(&[outer, vec![bore]].concat(), DVec3::Z * 2.0)
}

/// Heart-shaped BSpline profile extruded along Z.
fn build_heart() -> Result<Solid, Error> {
	let profile = [Edge::bspline(
		&[
			DVec3::new(0.0, -4.0, 0.0), // bottom tip
			DVec3::new(2.0, -1.5, 0.0),
			DVec3::new(4.0, 1.5, 0.0),
			DVec3::new(2.5, 3.5, 0.0),  // right lobe top
			DVec3::new(0.0, 2.0, 0.0),  // center dip
			DVec3::new(-2.5, 3.5, 0.0), // left lobe top
			DVec3::new(-4.0, 1.5, 0.0),
			DVec3::new(-2.0, -1.5, 0.0),
		],
		BSplineEnd::Periodic,
	)?];
	Solid::extrude(&profile, DVec3::Z * 7.0)
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
	let result = [build_oblique_cylinder()?.color("#f1c8b0").translate(DVec3::X * 10.0), build_l_beam()?.color("#b0f1c8").translate(DVec3::X * 20.0), build_heart()?.color("#f1b0b0").translate(DVec3::X * 30.0), build_plate_with_bore()?.color("#d4b0f1").translate(DVec3::X * 40.0)];

	Solid::write_step(&result, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

	let mesh = Solid::mesh(&result, Default::default())?;
	let scene = mesh.scene(Default::default());
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;
	mesh.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;
	mesh.write_gltf_binary(&mut std::fs::File::create(format!("{example_name}.glb")).unwrap())?;

	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
	Ok(())
}
