//! Demo of `Solid::loft` and `Solid::sweep_sections`.
//!
//! - **Plasma (sweep_sections)**: 8 elliptical poloidal sections swept along
//!   a circular spine with `ProfileOrient::Torsion` — a stellarator-like
//!   helical twist. Using `sweep_sections` with an explicit spine preserves
//!   rotational symmetry that loft's implicit spine interpolation can break.
//! - **Stack (loft)**: 5 elliptical sections stacked along Z with varying
//!   aspect ratio. OCCT caps the ends with planar faces for a tapered
//!   "cooling tower" shape.

use cadrum::{BSplineEnd, Edge, Error, ProfileOrient, Solid};
use glam::DVec3;
use std::f64::consts::TAU;

/// Build one elliptical poloidal rib for the plasma sweep.
fn plasma_rib(phi: f64, ring_r: f64, a: f64, b: f64, twist_per_phi: f64, n: usize) -> Edge {
	let center = DVec3::new(ring_r * phi.cos(), ring_r * phi.sin(), 0.0);
	let radial = DVec3::new(phi.cos(), phi.sin(), 0.0);
	let axial = DVec3::Z;
	let twist = twist_per_phi * phi;
	let cos_t = twist.cos();
	let sin_t = twist.sin();

	let pts: Vec<DVec3> = (0..n)
		.map(|i| {
			let theta = TAU * i as f64 / n as f64;
			let lx = a * theta.cos();
			let ly = b * theta.sin();
			let r_offset = lx * cos_t - ly * sin_t;
			let z_offset = lx * sin_t + ly * cos_t;
			center + radial * r_offset + axial * z_offset
		})
		.collect();
	Edge::bspline(pts, BSplineEnd::Periodic).expect("plasma rib bspline")
}

/// 8 ribs swept along a circular spine → twisted plasma-like torus.
fn build_plasma() -> Result<Solid, Error> {
	const N_RIBS: usize = 8;
	const N_POINTS: usize = 32;
	const RING_R: f64 = 6.0;

	let spine = Edge::circle(RING_R, DVec3::Z)?;
	let sections: Vec<Vec<Edge>> = (0..N_RIBS)
		.map(|i| {
			let phi = TAU * i as f64 / N_RIBS as f64;
			vec![plasma_rib(phi, RING_R, 1.8, 1.2, 1.0, N_POINTS)]
		})
		.collect();
	Ok(Solid::sweep_sections(&sections, std::slice::from_ref(&spine), ProfileOrient::Torsion)?.color("#87ceeb"))
}

/// One elliptical section in an XY-parallel plane at height `z`.
fn elliptic_ring(a: f64, b: f64, z: f64, n: usize) -> Edge {
	let pts: Vec<DVec3> = (0..n)
		.map(|i| {
			let t = TAU * i as f64 / n as f64;
			DVec3::new(a * t.cos(), b * t.sin(), z)
		})
		.collect();
	Edge::bspline(pts, BSplineEnd::Periodic).expect("elliptic ring bspline")
}

/// 5 elliptical sections stacked along Z, open loft.
fn build_stack() -> Result<Solid, Error> {
	const N_SECTIONS: usize = 5;
	const N_POINTS: usize = 32;
	let sections: Vec<Vec<Edge>> = (0..N_SECTIONS)
		.map(|i| {
			let t = i as f64 / (N_SECTIONS - 1) as f64;
			let z = i as f64 * 4.0;
			let a = 2.0 + 0.8 * t;
			let b = 1.6 - 0.5 * t;
			vec![elliptic_ring(a, b, z, N_POINTS)]
		})
		.collect();
	Ok(Solid::loft(&sections)?.color("#808000"))
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let plasma = build_plasma()?;
	let stack = build_stack()?.translate(DVec3::new(20.0, 0.0, -8.0));

	let result = [plasma, stack];

	let step_path = format!("{example_name}.step");
	let mut f = std::fs::File::create(&step_path).expect("failed to create STEP file");
	cadrum::io::write_step(&result, &mut f).expect("failed to write STEP");
	println!("wrote {step_path}");

	let svg_path = format!("{example_name}.svg");
	let mut f = std::fs::File::create(&svg_path).expect("failed to create SVG file");
	cadrum::io::write_svg(&result, DVec3::new(1.0, 1.0, 1.0), 0.5, true, &mut f).expect("failed to write SVG");
	println!("wrote {svg_path}");

	Ok(())
}
