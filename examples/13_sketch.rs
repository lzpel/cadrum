//! Demo of `Sketch` -> `Edge::sketch` -> `Solid::extrude`: build a 2D region as a
//! boolean expression of generalized circles, lower it to boundary edges, extrude it.
//!
//! - **Box**: four half-planes intersected
//! - **Oblique cylinder**: a single disk extruded at a steep angle
//! - **L-beam**: two rectangles unioned (one clause is always convex, so an L needs `+`)
//! - **Heart**: polygon approximation -- `Sketch` has no bspline, so the curve is
//!   traded for a union of convex polygons
use cadrum::{DVec2, DVec3, Edge, Error, Sketch, Solid};

fn p(x: f64, y: f64) -> DVec2 {
	DVec2::new(x, y)
}

/// CCW 頂点列 -> 凸多角形 (半平面の積)。`Sketch::line(a,b)` は左が内側なので巻きは CCW。
fn convex(pts: &[DVec2]) -> Sketch {
	let mut s = Sketch::line(pts[pts.len() - 1], pts[0]);
	for w in pts.windows(2) {
		s = s * Sketch::line(w[0], w[1]);
	}
	s
}

/// Square -> box.
fn build_box() -> Result<Solid, Error> {
	let s = convex(&[p(0.0, 0.0), p(5.0, 0.0), p(5.0, 5.0), p(0.0, 5.0)]);
	Solid::extrude(&Edge::sketch(&s)?, DVec3::Z * 8.0)
}

/// Disk extruded at a steep angle -> oblique cylinder.
fn build_oblique_cylinder() -> Result<Solid, Error> {
	let s = Sketch::circle(p(0.0, 0.0), 3.0);
	Solid::extrude(&Edge::sketch(&s)?, DVec3::new(-4.0, -6.0, 8.0))
}

/// L-beam. A clause is an AND of half-planes, hence always convex -- the reflex
/// corner has to come from the union of the two arms.
fn build_l_beam() -> Result<Solid, Error> {
	let arm_h = convex(&[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 1.0), p(0.0, 1.0)]);
	let arm_v = convex(&[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 3.0), p(0.0, 3.0)]);
	Solid::extrude(&Edge::sketch(&(arm_h + arm_v))?, DVec3::Z * 12.0)
}

/// Heart, deliberately faceted: `Sketch` has no bspline, so 05_extrude's periodic
/// curve becomes a union of three convex polygons (the V and the two lobes).
fn build_heart() -> Result<Solid, Error> {
	// 凸な外形から谷の楔を削る。和 (葉を 2 枚足す) でも同じ形は書けるが、そちらは 2 枚が
	// 接する谷の頂点に 4 曲線が集まり、`boundary` の degree 2 前提を外れる。
	let outline = convex(&[p(0.0, -4.0), p(4.0, 1.5), p(2.5, 3.5), p(-2.5, 3.5), p(-4.0, 1.5)]);
	let notch = convex(&[p(0.0, 2.0), p(2.5, 4.0), p(-2.5, 4.0)]);
	Solid::extrude(&Edge::sketch(&(outline - notch))?, DVec3::Z * 7.0)
}

fn main() -> Result<(), Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let box_solid = build_box()?.color("#b0d4f1");
	let oblique = build_oblique_cylinder()?.color("#f1c8b0").translate(DVec3::X * 10.0);
	let l_beam = build_l_beam()?.color("#b0f1c8").translate(DVec3::X * 20.0);
	let heart = build_heart()?.color("#f1b0b0").translate(DVec3::X * 30.0);

	println!("box={:.3} oblique={:.3} l_beam={:.3} heart={:.3}", box_solid.volume(), oblique.volume(), l_beam.volume(), heart.volume());

	let result = [box_solid, oblique, l_beam, heart];
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
