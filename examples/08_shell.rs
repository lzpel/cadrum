//! Demo of `Solid::shell`:
//! - Cube: remove top face, offset inward → open-top container
//! - Sealed cube: empty open_faces → solid with an internal void (outer skin
//!   + reversed inner shell)
//! - Torus: bisect with a half-space to introduce planar cut faces, then
//!   shell using those cut faces as the openings → thin-walled half-ring
//!   with both cross-sections exposed

use cadrum::{DVec3, Error, Solid};

fn hollow_cube() -> Result<Solid, Error> {
	let cube = Solid::cube(8.0, 8.0, 8.0);
	// TopExp_Explorer order on a box is stable; +Z face ends up last.
	let top = cube.iter_face().last().expect("cube has faces");
	cube.shell(-1.0, [top])
}

fn sealed_cube() -> Result<Solid, Error> {
	let cube = Solid::cube(8.0, 8.0, 8.0);
	cube.shell(-1.0, std::iter::empty::<&cadrum::Face>())
}

fn halved_shelled_torus(thickness: f64) -> Result<Solid, Error> {
	let torus = Solid::torus(6.0, 2.0, DVec3::Y);
	// Bisect with Y=0 half-space (normal +Y): keep the +Y half of the ring — always 1 solid.
	let cutter = Solid::half_space(DVec3::ZERO, -DVec3::Z);
	// `iter_history()` yields [post_id, src_id] pairs for every result face.
	// Filter to those whose src_id is one of the cutter's faces, then collect
	// their post_ids — these are the planar cut faces in the result that we
	// want to use as shell openings.
	let cutter_face_ids: std::collections::HashSet<u64> =
		cutter.iter_face().map(|f| f.tshape_id()).collect();
	let halves = torus.intersect(&[cutter])?;
	let half = halves.into_iter().next().ok_or(Error::BooleanOperationFailed)?;
	let from_cutter: std::collections::HashSet<u64> = half
		.iter_history()
		.filter_map(|[post, src]| cutter_face_ids.contains(&src).then_some(post))
		.collect();
	half.shell(thickness, half.iter_face().filter(|f| from_cutter.contains(&f.tshape_id())))
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let result = [
		hollow_cube()?.color("#d0a878"),
		sealed_cube()?.color("#6fbf73").translate(DVec3::Y * 10.0),
		halved_shelled_torus(1.0)?.color("#ff5e00").translate(DVec3::X * 18.0),
		halved_shelled_torus(-1.0)?.color("#0052ff").translate(DVec3::X * 18.0 + DVec3::Y * 10.0),
	];

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::write_step(&result, &mut f).expect("failed to write STEP");

	// Isometric view from (1, 1, 2) with shading so the cavity depth reads
	// naturally.
	let mut f = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	cadrum::mesh(&result, 0.2).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), DVec3::Z, true, true, &mut f)).expect("failed to write SVG");

	println!("wrote {example_name}.step / {example_name}.svg");
	Ok(())
}
