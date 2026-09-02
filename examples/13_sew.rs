//! Sew closes a seam no boolean can: a hexagonal ring of mitered triangular prisms is lofted with coincident first/last sections, the caps are dropped, sew fuses the ring (genus 0 → 1), and clean erases the seam edges entirely.
// sew (#186) targets surface-first work: closing lofted skin panels or surface-only STEP into a watertight solid.
// The seam sits mid-segment, so its two half-panels are coplanar and clean can dissolve the stitched edges.

use cadrum::{DVec3, Edge, Face, Solid};
use std::f64::consts::{PI, TAU};

fn main() -> Result<(), cadrum::Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let (m, r) = (6usize, 20.0);
	let profile = [(-5.0, -4.0), (5.0, -2.0), (0.0, 6.0)]; // (radial, z), tilted so only the seam planes coincide
	let vert = |k: usize| DVec3::new((TAU * k as f64 / m as f64).cos(), (TAU * k as f64 / m as f64).sin(), 0.0) * r;

	// Miter section at ring vertex k: profile stretched by 1/cos(alpha) along the radial bisector.
	let alpha = PI / m as f64;
	let miter = |k: usize| profile.map(|(u, z)| vert(k) + vert(k).normalize() * (u / alpha.cos()) + DVec3::Z * z);
	// Perpendicular section at the middle of segment 0 — the seam, where the loft starts and ends.
	let mid_pt = (vert(0) + vert(1)) / 2.0;
	let mid = profile.map(|(u, z)| mid_pt + mid_pt.normalize() * u + DVec3::Z * z);

	let sections: Vec<Vec<Edge>> = std::iter::once(mid).chain((1..=m).map(miter)).chain(std::iter::once(mid)).map(|pts| Edge::polygon(&pts)).collect::<Result<_, _>>()?;
	let fake = Solid::loft(sections.iter(), true)?;

	let seam = mid_pt - DVec3::Z;
	let d0 = (vert(1) - vert(0)).normalize();
	let ring: Vec<&Face> = fake
		.iter_face()
		.filter(|f| {
			let (p, normal) = f.project(seam);
			!((p - seam).length() < 1e-6 && normal.dot(d0).abs() > 0.9)
		})
		.collect();
	let torus = Solid::sew(ring, 1.0e-6)?.clean()?.color("#2ecc71");
	println!("faces: fake={} torus={} / contains(seam): fake={} torus={}", fake.iter_face().count(), torus.iter_face().count(), fake.contains(seam), torus.contains(seam));

	let solids = [torus];
	Solid::write_step(&solids, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

	let mesh = Solid::mesh(&solids, Default::default())?;
	let scene = mesh.scene(Default::default());
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;
	mesh.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;
	mesh.write_gltf_binary(&mut std::fs::File::create(format!("{example_name}.glb")).unwrap())?;

	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
	Ok(())
}
