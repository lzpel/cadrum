//! mevius using BSplineEnd::Periodic and ProfileOrient::Auxiliary. Mevius but it's twisted more.

use cadrum::{BSplineEnd, DVec3, Edge, ProfileOrient, Solid};
use std::f64::consts::TAU;

fn main() -> Result<(), cadrum::Error> {
	let guided_spine = |phi: f64| {
		let p = DVec3::new(10., 0.0, 0.0);
		let g = p + 2. / 2. * DVec3::X;
		[p, (g - p).rotate_y(phi * 2.) + p].map(|v| v.rotate_z(phi))
	};
	const SIZE: usize = 10;
	let v: [[DVec3; 2]; SIZE] = std::array::from_fn(|i| guided_spine(TAU * i as f64 / SIZE as f64));
	let spine = Edge::bspline(&v.map(|a| a[0])[..SIZE], BSplineEnd::Periodic)?;
	let aux = Edge::bspline(&v.map(|a| a[1])[..SIZE], BSplineEnd::Periodic)?;

	let tube = |curve: &Edge| -> Result<Solid, cadrum::Error> {
		let profile = Edge::circle(0.1, DVec3::Z)?;
		Solid::sweep([&profile.align_z(curve.start_tangent(), DVec3::Z).translate(curve.start_point())], [curve], ProfileOrient::Up(DVec3::Z))
	};
	let spine_tube = tube(&spine)?.color("#4a90d9");
	let aux_tube = tube(&aux)?.color("#e67e22");
	println!("spine tube: faces={}  aux tube: faces={}", spine_tube.iter_face().count(), aux_tube.iter_face().count());
	output(&[spine_tube, aux_tube], Some("_tubes"))?;
	let prof = profile(2.0, 0.2)?.map(|v| v.align_z(spine.start_tangent(), DVec3::Y).translate(spine.start_point()));
	let mevius = Solid::sweep(&prof, &[spine], ProfileOrient::Auxiliary(&[aux]))?.color("#2ebc71");
	output(&[mevius], None)?;
	return Ok(());
}

fn profile(width: f64, height: f64) -> Result<[Edge; 4], cadrum::Error> {
	let v: Vec<Edge> = Edge::polygon(&[DVec3::new(-width / 2., -height / 2., 0.0), DVec3::new(width / 2., -height / 2., 0.0), DVec3::new(width / 2., height / 2., 0.0), DVec3::new(-width / 2., height / 2., 0.0)])?;
	Ok(v.try_into().unwrap())
}

fn output(solids: &[Solid], suffix: Option<&str>) -> Result<(), cadrum::Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap().to_string() + suffix.unwrap_or_default();
	Solid::write_step(solids, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;
	let mesh = Solid::mesh(solids, Default::default())?;
	let scene = mesh.scene(Default::default());
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;
	mesh.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;
	mesh.write_gltf_binary(&mut std::fs::File::create(format!("{example_name}.glb")).unwrap())?;

	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
	Ok(())
}
