//! Sphere of radius a with a through hole of radius b (a > b): the simplest case for differentiable CAD.

use cadrum::{DVec3, Solid};
use std::f64::consts::PI;

fn holed_sphere(a: f64, b: f64) -> Result<Solid, cadrum::Error> {
	let sphere = Solid::sphere(a);
	// Twice the sphere's height so the hole goes all the way through.
	let cylinder = Solid::cylinder(b, DVec3::Z * 4.0 * a).translate(DVec3::Z * -2.0 * a);
	(&sphere - &cylinder).build()
}

fn main() -> Result<(), cadrum::Error> {
	let (a, b) = (10.0, 4.0);
	holed_sphere(a, b)?;
	// h is the half length of the hole; the outer surface is the spherical zone |z| <= h.
	let h = (a * a - b * b).sqrt();
	let rows = [("V", 4.0 / 3.0 * PI * h.powi(3)), ("∂V/∂a", 4.0 * PI * a * h), ("∂V/∂b", -4.0 * PI * b * h), ("S", 4.0 * PI * h * (a + b)), ("∂S/∂a", 4.0 * PI * (2.0 * a - b) * (a + b) / h), ("∂S/∂b", 4.0 * PI * (a - 2.0 * b) * (a + b) / h)];
	println!("a={a} b={b}");
	for (name, analytic) in rows {
		println!("name={name:<6} analytic={analytic:>10.1}");
	}
	Ok(())
}
