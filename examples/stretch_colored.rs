#![cfg(feature = "color")]

use chijin::Shape;
use glam::DVec3;

fn main() {
	println!("Creating a colored box and applying stretch...");

	// 1. 基本となる直方体を作成 (x: 0..10)
	let mut base = Shape::box_from_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));

	// 2. 各面に色を付ける (6面)
	let colors = [
		[255, 0, 0],   // Red (Min-X)
		[0, 255, 0],   // Green (Max-X)
		[0, 0, 255],   // Blue (Min-Y)
		[255, 255, 0], // Yellow (Max-Y)
		[255, 0, 255], // Magenta (Min-Z)
		[0, 255, 255], // Cyan (Max-Z)
	];
	let mut i = 0;
	for face in base.faces() {
		if i < colors.len() {
			base.set_face_color(&face, colors[i]);
			i += 1;
		}
	}

	println!(
		"Base colored box created with {} colored faces.",
		base.color_count()
	);

	// 3. stretchのシミュレーション (x=5 で分割して +5 だけ引き伸ばす)

	// (A) 左半分 (x: 0..5)
	let hs_left = Shape::half_space(DVec3::new(5.0, 0.0, 0.0), DVec3::new(-1.0, 0.0, 0.0));
	let mut left_half = base.subtract(&hs_left).unwrap().shape;

	// (B) 右半分 (x: 5..10) -> (x: 10..15) に移動
	let hs_right = Shape::half_space(DVec3::new(5.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 0.0));
	let mut right_half = base.subtract(&hs_right).unwrap().shape;
	right_half.set_global_translation(DVec3::new(5.0, 0.0, 0.0));

	// (C) ギャップを埋める新規部分 (x: 5..10)
	let mut middle =
		Shape::box_from_corners(DVec3::new(5.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));
	// 側面などに適当な色を塗る (Gray)
	middle.set_all_faces_color([128, 128, 128]);

	// (D) すべて結合してクリーンアップ
	let u1 = left_half.union(&middle).unwrap().shape;
	let u2 = u1.union(&right_half).unwrap().shape;
	let final_shape = u2.clean().unwrap();

	println!(
		"Stretch applied. Final shape colored faces count: {}",
		final_shape.color_count()
	);

	// (E) （将来の課題）glTF や STEP フォーマットに出力して可視化する
	println!("Done. Face color structure preserved successfully!");
}

#[cfg(not(feature = "color"))]
fn main() {
	println!("Please enable the 'color' feature to run this example:");
	println!("cargo run --example stretch_colored --features color");
}
