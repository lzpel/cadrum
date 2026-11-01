//! Demo of `Solid::revolve`: sweep a closed profile about an axis.
//!
//! - **Pipe**: rectangle in the XZ plane turned a full circle about Z
//! - **Sphere**: half disc turned about its own diameter — a profile may touch the axis
//! - **Partial**: the same rectangle turned three quarters of the way, showing the angle
//! - **Channel**: rectangle with a circular hole → ring with a hidden toroidal channel

use cadrum::{DVec3, Edge, Error, Solid};
use std::f64::consts::TAU;

/// Rectangle `x ∈ [x0, x1]`, `z ∈ [z0, z1]` in the XZ plane, the plane that contains the Z axis.
fn rect(x0: f64, x1: f64, z0: f64, z1: f64) -> Result<Vec<Edge>, Error> {
	Edge::polygon(&[DVec3::new(x0, 0.0, z0), DVec3::new(x1, 0.0, z0), DVec3::new(x1, 0.0, z1), DVec3::new(x0, 0.0, z1)])
}

/// Full turn → pipe (simplest revolve).
fn build_pipe() -> Result<Solid, Error> {
	Solid::revolve(&rect(3.0, 5.0, 0.0, 4.0)?, DVec3::ZERO, DVec3::Z, TAU)
}

/// Half disc about its diameter → sphere. The straight edge lies on the axis.
fn build_sphere() -> Result<Solid, Error> {
	let r = 3.0;
	let profile = [Edge::arc_3pts(DVec3::new(0.0, 0.0, -r), DVec3::new(r, 0.0, 0.0), DVec3::new(0.0, 0.0, r))?, Edge::line(DVec3::new(0.0, 0.0, r), DVec3::new(0.0, 0.0, -r))?];
	Solid::revolve(&profile, DVec3::ZERO, DVec3::Z, TAU)
}

/// Three quarters of a turn → open ring segment.
fn build_partial() -> Result<Solid, Error> {
	Solid::revolve(&rect(3.0, 5.0, 0.0, 4.0)?, DVec3::ZERO, DVec3::Z, TAU * 0.75)
}

/// Rectangle with a circular hole: the hole sweeps into a toroidal channel inside the ring.
fn build_channel() -> Result<Solid, Error> {
	let hole = Edge::circle(1.0, DVec3::Y)?.translate(DVec3::new(4.0, 0.0, 2.0));
	Solid::revolve(&[rect(2.0, 6.0, 0.0, 4.0)?, vec![hole]].concat(), DVec3::ZERO, DVec3::Z, -TAU * 0.6)
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
	let result = [build_pipe()?.color("#b0d4f1"), build_sphere()?.color("#f1c8b0").translate(DVec3::X * 14.0), build_partial()?.color("#b0f1c8").translate(DVec3::X * 28.0), build_channel()?.color("#d4b0f1").translate(DVec3::X * 42.0)];
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
