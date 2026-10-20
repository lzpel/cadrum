//! Demo of `Solid::sew` on faces cut from elementary surfaces by `Face::face`, with no planar caps beyond the cube.
//!
//! - **Cube**: six plane rectangles
//! - **Napkin ring**: a sphere band around a cylinder hole; its volume is π h³ / 6 whatever the sphere's radius
//! - **Ice cream cone**: an upside-down cone and a scoop hanging out of its rim, about to fall

use cadrum::{DVec3, Error, Face, SceneOption, Solid, Surface, SurfaceKind};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_8, TAU};

/// Cube `[0, a]³`: each side's `v` runs along `axis_z x axis_x`.
fn sew_cube() -> Result<Solid, Error> {
	let a = 4.0;
	let side = |origin: DVec3, axis_z: DVec3, axis_x: DVec3| Face::face(&Surface { origin, axis_z, axis_x, kind: SurfaceKind::Plane }, [0.0, a], [0.0, a]);
	let faces = [side(DVec3::ZERO, DVec3::Z, DVec3::X)?, side(DVec3::Z * a, DVec3::Z, DVec3::X)?, side(DVec3::ZERO, DVec3::NEG_Y, DVec3::X)?, side(DVec3::Y * a, DVec3::NEG_Y, DVec3::X)?, side(DVec3::ZERO, DVec3::X, DVec3::Y)?, side(DVec3::X * a, DVec3::X, DVec3::Y)?];
	Solid::sew(&faces, 1e-6)
}

/// Sphere of radius `r` drilled through along Z, leaving a ring of height `h`.
fn sew_napkin_ring() -> Result<Solid, Error> {
	let (r, h) = (3.0_f64, 4.0);
	let lat = (h / 2.0 / r).asin();
	let band = Face::face(&Surface { origin: DVec3::ZERO, axis_z: DVec3::Z, axis_x: DVec3::X, kind: SurfaceKind::Sphere { radius: r } }, [0.0, TAU], [-lat, lat])?;
	let hole = Face::face(&Surface { origin: DVec3::ZERO, axis_z: DVec3::Z, axis_x: DVec3::X, kind: SurfaceKind::Cylinder { radius: r * lat.cos() } }, [0.0, TAU], [-h / 2.0, h / 2.0])?;
	Solid::sew([&band, &hole], 1e-6)
}

/// Upside-down cone of height `h` from the apex at the origin, with a scoop whose center hangs `d` below the rim.
fn sew_ice_cream_cone() -> Result<Solid, Error> {
	let (h, phi, d) = (6.0_f64, FRAC_PI_8, 1.5_f64);
	let rim = h * phi.tan();
	// The scoop passes through the rim; the cone stays outside it while d >= rim * tan(phi).
	let r = rim.hypot(d);
	let cone = Face::face(&Surface { origin: DVec3::ZERO, axis_z: DVec3::NEG_Z, axis_x: DVec3::X, kind: SurfaceKind::Cone { radius: 0.0, semi_angle: phi } }, [0.0, TAU], [0.0, h])?;
	let scoop = Face::face(&Surface { origin: DVec3::NEG_Z * (h + d), axis_z: DVec3::NEG_Z, axis_x: DVec3::X, kind: SurfaceKind::Sphere { radius: r } }, [0.0, TAU], [-(d / r).asin(), FRAC_PI_2])?;
	Solid::sew([&cone, &scoop], 1e-6)
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
	let result = [sew_cube()?.color("#b0d4f1").translate(DVec3::new(-2.0, -2.0, -2.0)), sew_napkin_ring()?.color("#f1c8b0").translate(DVec3::X * 10.0), sew_ice_cream_cone()?.color("#b0f1c8").translate(DVec3::new(20.0, 0.0, 5.2))];
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
