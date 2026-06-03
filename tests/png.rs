//! PNG output tests (Scene2D::write_png via tiny-skia).
//!
//! Run with `cargo test --features png`.

#![cfg(feature = "png")]

use cadrum::{DVec3, Solid};

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

/// PNG signature: 89 50 4E 47 0D 0A 1A 0A
const PNG_MAGIC: &[u8; 8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Read width/height from the IHDR chunk (bytes 16..24 of a PNG file).
fn png_dimensions(buf: &[u8]) -> (u32, u32) {
	let w = u32::from_be_bytes(buf[16..20].try_into().unwrap());
	let h = u32::from_be_bytes(buf[20..24].try_into().unwrap());
	(w, h)
}

#[test]
fn test_png_box_isometric() {
	let shape = [Solid::cube(DVec3::ZERO, DVec3::splat(10.0))];
	let mesh = Solid::mesh(&shape, cadrum::Tessellation { deflection_linear: 0.1, relative_linear: false, ..Default::default() }).unwrap();
	let scene = mesh.scene(cadrum::SceneOption { view: dvec3(1.0, 1.0, 1.0).normalize(), ..Default::default() });

	let mut buf = Vec::new();
	scene.write_png([400, 400], &mut buf).unwrap();

	assert_eq!(&buf[0..8], PNG_MAGIC, "PNG signature missing");
	assert_eq!(png_dimensions(&buf), (400, 400));

	std::fs::create_dir_all("out").unwrap();
	std::fs::write("out/box_isometric.png", &buf).unwrap();
}

#[test]
fn test_png_cylinder_shaded() {
	let shape = [Solid::cylinder(5.0, DVec3::Z * 10.0)];
	let mesh = Solid::mesh(&shape, cadrum::Tessellation { deflection_linear: 0.1, relative_linear: false, ..Default::default() }).unwrap();
	let scene = mesh.scene(cadrum::SceneOption { view: dvec3(1.0, 0.5, 0.3).normalize(), shading: true, ..Default::default() });

	let mut buf = Vec::new();
	scene.write_png([400, 600], &mut buf).unwrap();

	assert_eq!(&buf[0..8], PNG_MAGIC);
	assert_eq!(png_dimensions(&buf), (400, 600));

	std::fs::create_dir_all("out").unwrap();
	std::fs::write("out/cylinder.png", &buf).unwrap();
}

#[test]
fn test_png_dimensions_are_exact() {
	// User-specified [width, height] must appear verbatim in the IHDR,
	// regardless of viewbox aspect (letterboxed when aspects differ).
	let shape = [Solid::cube(DVec3::ZERO, DVec3::new(50.0, 10.0, 10.0))];
	let mesh = Solid::mesh(&shape, cadrum::Tessellation { deflection_linear: 0.5, relative_linear: false, ..Default::default() }).unwrap();
	let scene = mesh.scene(cadrum::SceneOption { view: DVec3::Z, up: DVec3::Y, hidden_edges: false, shading: false });

	for dims in [[500, 500], [800, 200], [200, 800]] {
		let mut buf = Vec::new();
		scene.write_png(dims, &mut buf).unwrap();
		assert_eq!(png_dimensions(&buf), (dims[0] as u32, dims[1] as u32));
	}
}
