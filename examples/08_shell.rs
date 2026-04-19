//! Demo of `Solid::shell`: hollow a solid into a thin-walled container.
//!
//! Removes the top face of a cube and a sphere, then offsets remaining faces
//! inward by 1mm. The isometric view looks down into each opening so the
//! inside walls are visible.

use cadrum::{DVec3, Error, Solid};

/// Pick the face with the largest Z centroid by approximating face center via
/// edge endpoint averaging — avoids adding a normal accessor just for this demo.
/// Here we rely on a simpler contract: OCCT's TopExp_Explorer order on a box
/// primitive is stable, and the last face of `iter_face` is the +Z face.
fn top_face(solid: &Solid) -> &cadrum::Face {
	solid.iter_face().last().expect("solid must have at least one face")
}

fn hollow_cube() -> Result<Solid, Error> {
	let cube = Solid::cube(8.0, 8.0, 8.0);
	let top = top_face(&cube);
	cube.shell(-1.0, [top])
}

fn hollow_sphere() -> Result<Solid, Error> {
	let sphere = Solid::sphere(5.0);
	let open = sphere.iter_face().next().expect("sphere has a face");
	sphere.shell(-0.8, [open])
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let cube = hollow_cube()?.color("#d0a878");
	let sphere = hollow_sphere()?.color("#a8c8d0").translate(DVec3::X * 14.0);

	let result = [cube, sphere];

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::write_step(&result, &mut f).expect("failed to write STEP");

	// Isometric view from (1, 1, 2): camera sits above-front-right so the top
	// (+Z) opening is in-frame and the inner walls of the cavity are visible.
	// `shading = true` fills faces with per-face lighting so the cavity depth
	// reads naturally.
	let mut f = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	cadrum::mesh(&result, 0.2).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), false, true, &mut f)).expect("failed to write SVG");

	println!("wrote {example_name}.step / {example_name}.svg");
	Ok(())
}
