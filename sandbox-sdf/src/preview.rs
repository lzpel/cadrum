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

/// SDF 背景の上に連結成分ごとの直線・円弧セグメントを重ねて PNG 出力する。
/// EdgeLoop 列は呼び出し側で生成する (region / region2 を切り替えて比較できる)。
pub fn preview_regions_segment(
	sdf: impl Fn(Vec2) -> f32,
	loops: &[crate::segment::EdgeLoop],
	png: &Path,
) {
	const SIZE: u32 = 512;
	const FILL: f32 = 0.7;

	let [min_bb, max_bb] = bounding(&sdf);
	let center = (min_bb + max_bb) * 0.5;
	let long = (max_bb - min_bb).max_element().max(f32::EPSILON);
	let world_per_px = long / (SIZE as f32 * FILL);

	// SDF 背景
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

	const PALETTE: &[(u8, u8, u8)] = &[
		(255, 255, 255),
		(255, 200, 50),
		(50, 255, 200),
		(200, 50, 255),
		(50, 200, 255),
		(255, 50, 200),
		(200, 255, 50),
	];

	for (i, comp_segs) in loops.iter().enumerate() {
		let (r, g, b) = PALETTE[i % PALETTE.len()];
		let mut paint = Paint::default();
		paint.set_color_rgba8(r, g, b, 220);
		paint.anti_alias = true;
		let stroke = Stroke { width: 1.5, ..Default::default() };

		for seg in comp_segs {
			use crate::segment::Segment;
			match seg {
				Segment::Line { point, direction } => {
					let (px, py) = w2p(*point);
					let dpx = direction.normalize_or_zero();
					// 方向をピクセルスケールに変換して画像幅 1.5倍に延長
					let dpx_px = Vec2::new(dpx.x, dpx.y) / long * SIZE as f32;
					let dpx_unit = dpx_px.normalize_or_zero();
					let t = SIZE as f32 * 1.5;
					let mut pb = PathBuilder::new();
					pb.move_to(px - dpx_unit.x * t, py - dpx_unit.y * t);
					pb.line_to(px + dpx_unit.x * t, py + dpx_unit.y * t);
					if let Some(path) = pb.finish() {
						pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
					}
				}
				Segment::Circle { center: c, radius } => {
					let (cx, cy) = w2p(*c);
					let r_px = radius / long * SIZE as f32;
					if let Some(rect) =
						Rect::from_xywh(cx - r_px, cy - r_px, r_px * 2.0, r_px * 2.0)
					{
						let mut pb = PathBuilder::new();
						pb.push_oval(rect);
						if let Some(path) = pb.finish() {
							pixmap.stroke_path(
								&path, &paint, &stroke, Transform::identity(), None,
							);
						}
					}
				}
			}
		}
	}

	pixmap.save_png(png).unwrap();
}
