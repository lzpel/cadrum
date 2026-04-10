//! Demo of `Solid::loft`: a closed plasma-like loft and an open stacked loft side by side.
//!
//! - **Plasma (closed)**: 8 elliptical poloidal sections placed around a Z-axis ring,
//!   with the cross-section gradually rotated as the toroidal angle advances —
//!   a stellarator-like helical twist. `closed=true` lets OCCT's
//!   `BRepOffsetAPI_ThruSections` build a v-direction periodic surface
//!   automatically (via the IsSame trick).
//! - **Stack (open)**: 5 elliptical sections stacked along Z with varying
//!   aspect ratio. `closed=false` lets OCCT cap the ends with planar faces
//!   for a tapered "cooling tower" shape.
//!
//! Both pieces are rendered together so the difference between closed and
//! open loft is visually obvious in the SVG.

use cadrum::{BSplineEnd, Edge, Error, Solid};
use glam::DVec3;
use std::f64::consts::TAU;

/// Build one elliptical poloidal rib for the plasma loft.
///
/// The rib lives in the (radial, +Z) plane at toroidal angle `phi` on a ring
/// of major radius `ring_r`. The local cross-section is an ellipse with
/// semi-axes `(a, b)` whose orientation is rotated by `twist_per_phi * phi`
/// to give the surface a stellarator-style helical twist.
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
			// Rotate (lx, ly) by `twist` in the (radial, axial) frame
			let r_offset = lx * cos_t - ly * sin_t;
			let z_offset = lx * sin_t + ly * cos_t;
			center + radial * r_offset + axial * z_offset
		})
		.collect();
	Edge::bspline(pts, BSplineEnd::Periodic).expect("plasma rib bspline")
}

/// 8 ribs around a ring of radius 6, closed loft → twisted plasma-like torus.
fn build_plasma_closed() -> Result<Solid, Error> {
	const N_RIBS: usize = 8;
	const N_POINTS: usize = 32;
	let ribs: Vec<Vec<Edge>> = (0..N_RIBS)
		.map(|i| {
			let phi = TAU * i as f64 / N_RIBS as f64;
			vec![plasma_rib(phi, /* ring_r */ 6.0, /* a */ 1.8, /* b */ 1.2, /* twist */ 1.0, N_POINTS)]
		})
		.collect();
	Ok(Solid::loft(&ribs, true)?.color("#87ceeb"))
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

/// 5 elliptical sections stacked along Z with varying aspect ratio,
/// open loft → tapered "cooling tower" shape.
fn build_stack_open() -> Result<Solid, Error> {
	const N_SECTIONS: usize = 5;
	const N_POINTS: usize = 32;
	let sections: Vec<Vec<Edge>> = (0..N_SECTIONS)
		.map(|i| {
			let t = i as f64 / (N_SECTIONS - 1) as f64;  // 0.0 → 1.0
			let z = i as f64 * 4.0;
			// Aspect ratio twists: a grows with z, b shrinks
			let a = 2.0 + 0.8 * t;
			let b = 1.6 - 0.5 * t;
			vec![elliptic_ring(a, b, z, N_POINTS)]
		})
		.collect();
	Ok(Solid::loft(&sections, false)?.color("#808000"))
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let plasma = build_plasma_closed()?;
	// Place the open stack to the right of the plasma so they don't overlap in the SVG.
	let stack = build_stack_open()?.translate(DVec3::new(20.0, 0.0, -8.0));

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
