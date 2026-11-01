//! Demo of `Solid::loft`: skin a solid through cross-section wires.
//!
//! - **Frustum**: two circles of different radii → truncated cone (minimal loft)
//! - **Morph**: square polygon → circle (cross-section shape transition)
//! - **Tilted**: three non-parallel circular sections → twisted loft
//! - **Wing**: three NACA0012 sections lofted with `ruled=true` (straight ruled panels between sections — the sheet-metal / developable variant)

use cadrum::{BSplineEnd, DVec2, DVec3, Edge, Error, Solid};

/// Two circles → frustum (minimal loft example).
fn build_frustum() -> Result<Solid, Error> {
	let lower = [Edge::circle(3.0, DVec3::Z)?];
	let upper = [Edge::circle(1.5, DVec3::Z)?.translate(DVec3::Z * 8.0)];
	Ok(Solid::loft(&[lower, upper], false)?.color("#cd853f"))
}

/// Square polygon → circle (2-section morph loft).
fn build_morph() -> Result<Solid, Error> {
	let r = 2.5;
	let square = Edge::polygon(&[DVec3::new(-r, -r, 0.0), DVec3::new(r, -r, 0.0), DVec3::new(r, r, 0.0), DVec3::new(-r, r, 0.0)])?;
	let circle = Edge::circle(r, DVec3::Z)?.translate(DVec3::Z * 10.0);

	Ok(Solid::loft([square.as_slice(), std::slice::from_ref(&circle)], false)?.color("#808000"))
}

/// Three non-parallel circular sections → twisted loft.
fn build_tilted() -> Result<Solid, Error> {
	let bottom = [Edge::circle(2.5, DVec3::Z)?];
	let mid = [Edge::circle(2.0, DVec3::new(0.3, 0.0, 1.0).normalize())?.translate(DVec3::X + DVec3::Z * 5.0)];
	let top = [Edge::circle(1.5, DVec3::new(-0.2, 0.3, 1.0).normalize())?.translate(DVec3::new(-0.5, 1.0, 10.0))];

	Ok(Solid::loft(&[bottom, mid, top], false)?.color("#4682b4"))
}

/// NACA0012-like airfoil section points (unit chord, 2D: x = chord, y = thickness).
/// Cosine spacing walks TE → upper → LE → lower → TE, returning a closed loop
/// with the TE point duplicated at the end (a closed section with a sharp TE,
/// interpolated as a NotAKnot open curve).
fn naca_points(n: usize) -> Vec<DVec2> {
	let half = |x: f64| 5.0 * 0.12 * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x.powi(3) - 0.1036 * x.powi(4));
	let upper: Vec<DVec2> = (0..=n)
		.map(|i| {
			let x = (1.0 + (std::f64::consts::PI * i as f64 / n as f64).cos()) / 2.0;
			DVec2::new(x, half(x))
		})
		.collect();
	let lower: Vec<DVec2> = (1..=n)
		.map(|i| {
			let x = (1.0 - (std::f64::consts::PI * i as f64 / n as f64).cos()) / 2.0;
			DVec2::new(x, -half(x))
		})
		.collect();
	[upper, lower].concat()
}

/// Three NACA sections → tapered wing, lofted with `ruled=true` (straight panels).
fn build_wing(scale: f64) -> Result<Solid, Error> {
	let stations = [(1.0, 0.0), (0.6, 1.0), (0.5, 2.0)];
	let sections: Vec<[Edge; 1]> = stations
		.iter()
		.map(|&(c, z)| {
			let points: Vec<DVec3> = naca_points(60).into_iter().map(|p| DVec3::new(c * p.x, c * p.y, z) * scale).collect();
			[Edge::bspline(&points, BSplineEnd::NotAKnot).expect("NACA bspline section")]
		})
		.collect();
	Ok(Solid::loft(&sections, true)?.color("silver"))
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let frustum = build_frustum()?;
	let morph = build_morph()?.translate(DVec3::X * 10.0);
	let tilted = build_tilted()?.translate(DVec3::X * 20.0);
	let wing = build_wing(10.0)?.align_z(-DVec3::X, -DVec3::Y).translate(DVec3::X * 20.0 + DVec3::Y * 12.0);

	let result = [frustum, morph, tilted, wing];

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
