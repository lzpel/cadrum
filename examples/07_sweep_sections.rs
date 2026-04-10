//! Demo of `Solid::sweep_sections`: morph between cross-section profiles
//! along an explicit spine curve.
//!
//! - **Plasma**: 8 elliptical poloidal ribs swept along a circular spine
//!   with `ProfileOrient::Torsion` — a stellarator-like helical twist.
//! - **Morphing pipe**: circle-to-square transition swept along a straight
//!   spine — demonstrates cross-section morphing between dissimilar shapes.

use cadrum::{BSplineEnd, Edge, Error, ProfileOrient, Solid};
use glam::DVec3;
use std::f64::consts::TAU;

// ==================== Plasma: stellarator-like torus ====================

/// Build one elliptical poloidal rib at toroidal angle `phi`.
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

// ==================== Morphing pipe: circle → square ====================

/// Rounded-polygon section with `n_pts` points and corner radius blending
/// controlled by `squareness` (0.0 = circle, 1.0 = square-ish).
fn blended_section(radius: f64, squareness: f64, z: f64, n_pts: usize) -> Edge {
	let pts: Vec<DVec3> = (0..n_pts)
		.map(|i| {
			let theta = TAU * i as f64 / n_pts as f64;
			// Superellipse: |x/a|^p + |y/b|^p = 1, p=2 → circle, p→∞ → square
			let p = 2.0 + 8.0 * squareness; // 2.0 .. 10.0
			let ct = theta.cos();
			let st = theta.sin();
			let x = radius * ct.abs().powf(2.0 / p) * ct.signum();
			let y = radius * st.abs().powf(2.0 / p) * st.signum();
			DVec3::new(x, y, z)
		})
		.collect();
	Edge::bspline(pts, BSplineEnd::Periodic).expect("blended section bspline")
}

/// Straight-spine sweep morphing from circle to square over 5 sections.
fn build_morphing_pipe() -> Result<Solid, Error> {
	const N_SECTIONS: usize = 5;
	const N_POINTS: usize = 32;
	const RADIUS: f64 = 2.0;
	const LENGTH: f64 = 16.0;

	let spine = Edge::line(DVec3::ZERO, DVec3::Z * LENGTH)?;
	let sections: Vec<Vec<Edge>> = (0..N_SECTIONS)
		.map(|i| {
			let t = i as f64 / (N_SECTIONS - 1) as f64;
			let z = t * LENGTH;
			vec![blended_section(RADIUS, t, z, N_POINTS)]
		})
		.collect();
	Ok(Solid::sweep_sections(&sections, std::slice::from_ref(&spine), ProfileOrient::Fixed)?.color("#d2691e"))
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let plasma = build_plasma()?;
	let morphing = build_morphing_pipe()?.translate(DVec3::new(18.0, 0.0, -8.0));

	let result = [plasma, morphing];

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
