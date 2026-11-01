//! Demo of `Solid::sew` on faces cut from elementary surfaces by `Face::face`, with no planar caps beyond the tetrahedron.
//!
//! - **Tetrahedron**: four triangles sharing six lines between every other corner of a cube
//! - **Napkin ring**: a sphere band around a cylinder hole; its volume is π h³ / 6 whatever the sphere's radius
//! - **Ice cream cone**: an upside-down cone and a scoop hanging out of its rim, about to fall

use cadrum::{DVec3, Edge, Error, Face, SceneOption, Solid, Surface, SurfaceKind};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_8};

/// Regular tetrahedron on every other corner of `[-a, a]³`, its faces sharing the six lines between them.
fn sew_tetrahedron(a: f64) -> Result<Solid, Error> {
	let cube: [DVec3; 8] = std::array::from_fn(|i| DVec3::new(if i & 1 != 0 { a } else { -a }, if i & 2 != 0 { a } else { -a }, if i & 4 != 0 { a } else { -a }));
	let pairs = [(0, 3), (0, 6), (0, 5), (3, 6), (3, 5), (6, 5)];
	let edges: [Edge; 6] = std::array::from_fn(|n| Edge::line(cube[pairs[n].0], cube[pairs[n].1]).expect("distinct corners"));
	let loops: [[usize; 3]; 4] = [[3, 5, 4], [2, 5, 1], [0, 4, 2], [1, 3, 0]];
	let faces: [Face; 4] = loops.map(|l| {
		let (origin, u) = (edges[l[0]].start_point(), edges[l[0]].end_point() - edges[l[0]].start_point());
		let normal = u.cross(edges[l[1]].end_point() - edges[l[1]].start_point()).normalize();
		let normal = if normal.dot(origin) < 0.0 { -normal } else { normal };
		Face::face(&Surface { origin, axis_z: normal, axis_x: u.normalize(), kind: SurfaceKind::Plane }, l.map(|e| &edges[e])).expect("each loop winds counterclockwise")
	});
	Solid::sew(&faces, 1e-6)
}

/// Sphere of radius `r` drilled through along Z, leaving a ring of height `h`.
fn sew_napkin_ring() -> Result<Solid, Error> {
	let (r, h) = (3.0_f64, 4.0);
	let rho = (r * r - h * h / 4.0).sqrt();
	// Both faces lie left of the circles, between them.
	let rims = [Edge::circle(rho, DVec3::Z)?.translate(DVec3::NEG_Z * h / 2.0), Edge::circle(rho, DVec3::NEG_Z)?.translate(DVec3::Z * h / 2.0)];
	let band = Face::face(&Surface { origin: DVec3::ZERO, axis_z: DVec3::Z, axis_x: DVec3::X, kind: SurfaceKind::Sphere { radius: r } }, &rims)?;
	let hole = Face::face(&Surface { origin: DVec3::ZERO, axis_z: DVec3::Z, axis_x: DVec3::X, kind: SurfaceKind::Cylinder { radius: rho } }, &rims)?;
	Solid::sew([&band, &hole], 1e-6)
}

/// Upside-down cone of height `h` from the apex at the origin, with a scoop whose center hangs `d` below the rim.
fn sew_ice_cream_cone() -> Result<Solid, Error> {
	let (h, phi, d) = (6.0_f64, FRAC_PI_8, 1.5_f64);
	let rim = h * phi.tan();
	let r = rim.hypot(d);
	let cone = Face::face(&Surface { origin: DVec3::ZERO, axis_z: DVec3::NEG_Z, axis_x: DVec3::X, kind: SurfaceKind::Cone { radius: 0.0, semi_angle: phi } }, [&Edge::circle(rim, DVec3::Z)?.translate(DVec3::NEG_Z * h)])?;
	let scoop = Face::face(&Surface { origin: DVec3::NEG_Z * (h + d), axis_z: DVec3::NEG_Z, axis_x: DVec3::X, kind: SurfaceKind::Sphere { radius: r } }, [&Edge::circle(rim, DVec3::NEG_Z)?.translate(DVec3::NEG_Z * h)])?;
	Solid::sew([&cone, &scoop], 1e-6)
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
	let result = [sew_tetrahedron(2.0)?.color("#b0d4f1").rotate_z(FRAC_PI_2), sew_napkin_ring()?.color("#f1c8b0").translate(DVec3::X * 10.0), sew_ice_cream_cone()?.color("#b0f1c8").translate(DVec3::new(20.0, 0.0, 5.2))];
	Solid::write_step(&result, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;
	let mesh = Solid::mesh(&result, Default::default())?;
	let scene = mesh.scene(SceneOption { view: DVec3::X + DVec3::Y + DVec3::Z * 1.5, ..Default::default() });
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;
	mesh.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;
	mesh.write_gltf_binary(&mut std::fs::File::create(format!("{example_name}.glb")).unwrap())?;
	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
	Ok(())
}
