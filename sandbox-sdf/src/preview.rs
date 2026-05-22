//! SDF の距離マップを PNG に書き出すプレビュー。

use crate::bounding;
use glam::{Vec2, Vec3};
use std::path::Path;
use tiny_skia::{Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform};

/// エルミート補間（GLSL の smoothstep 相当）。
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
	let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
}

/// SDF をピクセルごとに評価して距離マップを PNG 出力する。
///
/// 表示範囲は `bounding(sdf)` が返す bbox から決める。bbox の中心を画像
/// 中央に置き、長辺が画像の FILL（70%）を占めるよう等方スケールする。
///
/// 内側を青系・外側をオレンジ系で塗り、距離の等高線を縞、ゼロ等位線を白で描く。
pub fn preview(sdf: impl Fn(Vec2) -> f32, png: &Path) {
	const SIZE: u32 = 512;
	const FILL: f32 = 0.7; // 形状の長辺が画像に占める割合

	// 形状の bbox の中心と長辺から、ピクセル⇔ワールドの等方スケールを決める
	let [min, max] = bounding(&sdf);
	let center = (min + max) * 0.5;
	let long = (max - min).max_element().max(f32::EPSILON);
	let world_per_px = long / (SIZE as f32 * FILL);

	let mut pixmap = Pixmap::new(SIZE, SIZE).unwrap();
	let pixels = pixmap.pixels_mut();
	// ピクセル座標 -> 中心からのワールドオフセット
	for y in 0..SIZE {
		for x in 0..SIZE {
			let p = center + Vec2::new((x as f32/SIZE as f32-0.5) * long, (y as f32/SIZE as f32-0.5) * long); // y は上向き
			let d = sdf(p);

			// 外側: 黄色(d=0) → 黒(d=+∞)
			// 内側: cyan(d=0) → 白(d=-∞)
			let t = (-3.0 * d.abs() / long).exp(); // 1 at |d|=0, 0 at |d|=∞
			let mut col = if d >= 0.0 {
				Vec3::new(1.0, 1.0, 0.0) * t // 黄 → 黒
			} else {
				Vec3::new(0.0, 1.0, 1.0).lerp(Vec3::splat(1.0), 1.0 - t) // cyan → 白
			};
			let cycle = d / world_per_px / 12.0 * std::f32::consts::TAU; // 12px ごとの等高線
			col *= 0.8 + 0.2 * cycle.cos();
			let edge = 1.0 - smoothstep(0.0, 1.5 * world_per_px, d.abs());
			col = col.lerp(Vec3::new(1.0, 0.0, 1.0), edge); // ゼロ等位線をマゼンタで強調

			let to = |c: f32| (c.clamp(0.0, 1.0) * 255.0) as u8;
			pixels[(y * SIZE + x) as usize] =
				PremultipliedColorU8::from_rgba(to(col.x), to(col.y), to(col.z), 255).unwrap();
		}
	}
	pixmap.save_png(png).unwrap();
}

/// SDF 背景の上に、連結成分ごとの輪郭線と代表点を重ねて PNG 出力する。
pub fn preview_regions(sdf: impl Fn(Vec2) -> f32, png: &std::path::Path) {
	const SIZE: u32 = 512;
	const FILL: f32 = 0.7;

	let [min_bb, max_bb] = bounding(&sdf);
	let center = (min_bb + max_bb) * 0.5;
	let long = (max_bb - min_bb).max_element().max(f32::EPSILON);
	let world_per_px = long / (SIZE as f32 * FILL);

	// SDF 背景（preview と同じ配色）
	let mut pixmap = Pixmap::new(SIZE, SIZE).unwrap();
	{
		let pixels = pixmap.pixels_mut();
		for y in 0..SIZE {
			for x in 0..SIZE {
				let p = center
					+ Vec2::new(
						(x as f32 / SIZE as f32 - 0.5) * long,
						(y as f32 / SIZE as f32 - 0.5) * long,
					);
				let d = sdf(p);
				let t = (-3.0 * d.abs() / long).exp();
				let mut col = if d >= 0.0 {
					Vec3::new(1.0, 1.0, 0.0) * t
				} else {
					Vec3::new(0.0, 1.0, 1.0).lerp(Vec3::splat(1.0), 1.0 - t)
				};
				let cycle = d / world_per_px / 12.0 * std::f32::consts::TAU;
				col *= 0.8 + 0.2 * cycle.cos();
				let edge = 1.0 - smoothstep(0.0, 1.5 * world_per_px, d.abs());
				col = col.lerp(Vec3::new(1.0, 0.0, 1.0), edge);
				let to = |c: f32| (c.clamp(0.0, 1.0) * 255.0) as u8;
				pixels[(y * SIZE + x) as usize] =
					PremultipliedColorU8::from_rgba(to(col.x), to(col.y), to(col.z), 255)
						.unwrap();
			}
		}
	}

	// ワールド座標 → ピクセル座標
	let w2p = |p: Vec2| -> (f32, f32) {
		(
			(p.x - center.x) / long * SIZE as f32 + SIZE as f32 * 0.5,
			(p.y - center.y) / long * SIZE as f32 + SIZE as f32 * 0.5,
		)
	};

	// 連結成分を取得
	let (grid_bbox, map) = crate::region::inner_map(&sdf);
	let region_list = crate::region::regions(grid_bbox, &map);

	const PALETTE: &[(u8, u8, u8)] = &[
		(255, 255, 255),
		(255, 200, 50),
		(50, 255, 200),
		(200, 50, 255),
		(50, 200, 255),
		(255, 50, 200),
		(200, 255, 50),
	];

	for (i, &(rep, _)) in region_list.iter().enumerate() {
		let (r, g, b) = PALETTE[i % PALETTE.len()];

		// 輪郭ポリライン
		let pts = crate::region::contour(rep, &sdf, 100);
		if pts.len() >= 2 {
			let mut pb = PathBuilder::new();
			let (px0, py0) = w2p(pts[0]);
			pb.move_to(px0, py0);
			for &wp in &pts[1..] {
				let (px, py) = w2p(wp);
				pb.line_to(px, py);
			}
			pb.close();
			if let Some(path) = pb.finish() {
				let mut paint = Paint::default();
				paint.set_color_rgba8(r, g, b, 220);
				paint.anti_alias = true;
				let stroke = Stroke { width: 1.5, ..Default::default() };
				pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
			}
		}

		// 代表点（6×6 矩形）
		let (px, py) = w2p(rep);
		if let Some(rect) = Rect::from_xywh(px - 3.0, py - 3.0, 6.0, 6.0) {
			let mut paint = Paint::default();
			paint.set_color_rgba8(r, g, b, 255);
			pixmap.fill_rect(rect, &paint, Transform::identity(), None);
		}
	}

	pixmap.save_png(png).unwrap();
}
