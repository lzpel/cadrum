use glam::Vec2;
use sandbox_sdf::{boolean, preview::preview, sdf_circle, sdf_polygon};
use std::path::Path;

fn main() {
	// 原点中心・半径1の正五角形（頂点は上向き始点で反時計回り）
	let pentagon: Vec<Vec2> = (0..5)
		.map(|i| {
			let a = std::f32::consts::TAU * i as f32 / 5.0 + std::f32::consts::FRAC_PI_2;
			Vec2::new(a.cos(), a.sin())
		})
		.collect();
	let png = Path::new("pentagon.png");
	preview(|p| sdf_polygon(p, pentagon.iter().copied()), png);
	println!("wrote {}", png.display());

	// 左右にずらした半径1の2円で和・積・差を可視化する
	let left = |p| sdf_circle(p - Vec2::new(-0.5, 0.0));
	let right = |p| sdf_circle(p - Vec2::new(0.5, 0.0));
	for (name, op) in [
		("boolean_union", boolean::union as fn(f32, f32) -> f32),
		("boolean_intersection", boolean::intersection),
		("boolean_difference", boolean::difference),
	] {
		let png = Path::new(name).with_extension("png");
		preview(|p| op(left(p), right(p)), &png);
		println!("wrote {}", png.display());
	}

	// 5角星（外半径1・内半径0.4の頂点が交互に並ぶ凹多角形）の凸包 → 五角形
	let star: Vec<Vec2> = (0..10)
		.map(|i| {
			let r = if i % 2 == 0 { 1.0 } else { 0.4 };
			let a = std::f32::consts::TAU * i as f32 / 10.0 + std::f32::consts::FRAC_PI_2;
			Vec2::new(a.cos(), a.sin()) * r
		})
		.collect();
	let star_sdf = move |p| sdf_polygon(p, star.iter().copied());
	let png = Path::new("star.png");
	preview(&star_sdf, png);
	println!("wrote {}", png.display());
	let hull = boolean::convex_hull(&star_sdf, 256);
	let png = Path::new("star_hull.png");
	preview(&hull, png);
	println!("wrote {}", png.display());
	// 凸包から星を引いた差分 = 凸包で「埋められた」へこみの領域
	let png = Path::new("star_filled.png");
	preview(|p| boolean::difference(hull(p), star_sdf(p)), png);
	println!("wrote {}", png.display());
}
