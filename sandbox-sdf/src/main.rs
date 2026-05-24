use glam::Vec2;
use sandbox_sdf::{
	issue::sdf_issue,
	preview::{preview, preview_regions_segment},
	sdf_circle, sdf_polygon,
};
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

	// 5角星（外半径1・内半径0.4の頂点が交互に並ぶ凹多角形）
	let star: Vec<Vec2> = (0..10)
		.map(|i| {
			let r = if i % 2 == 0 { 1.0 } else { 0.4 };
			let a = std::f32::consts::TAU * i as f32 / 10.0 + std::f32::consts::FRAC_PI_2;
			Vec2::new(a.cos(), a.sin()) * r
		})
		.collect();
	let png = Path::new("star.png");
	preview(|p| sdf_polygon(p, star.iter().copied()), png);
	println!("wrote {}", png.display());

	// 左右にずらした半径1の2円の和
	let left = |p: Vec2| sdf_circle(p + Vec2::new(0.5, 0.0));
	let right = |p: Vec2| sdf_circle(p - Vec2::new(0.5, 0.0));
	let png = Path::new("union.png");
	preview(|p| left(p).min(right(p)), png);
	println!("wrote {}", png.display());

	let png = Path::new("issue.png");
	preview(sdf_issue, png);
	println!("wrote {}", png.display());

	let raw = sandbox_sdf::region::regions_raw(sdf_issue);
	let segs1 = sandbox_sdf::region::regions_segment(&raw);
	let png = Path::new("issue_regions.png");
	preview_regions_segment(sdf_issue, &segs1, png);
	println!("wrote {}", png.display());

	let segs2 = sandbox_sdf::region2::regions(sdf_issue);
	let png = Path::new("issue_regions2.png");
	preview_regions_segment(sdf_issue, &segs2, png);
	println!("wrote {}", png.display());
}
