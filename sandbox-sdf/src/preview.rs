//! SDF の距離マップとセグメント輪郭を PNG に書き出すプレビュー。

use crate::{bounding, region::regions, Segment};
use glam::{DVec2, Vec3};
use std::path::Path;
use tiny_skia::{Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform};

/// エルミート補間（GLSL の smoothstep 相当）。
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
	let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
}

/// SDF をピクセル評価して距離マップを描き、その上に `regions(sdf)` で抽出した
/// Line / Circle セグメント列を連結成分ごとに色分けして重ねた PNG を出力する。
///
/// 表示範囲は `bounding(sdf)` の bbox 中央を画像中央に置き、長辺が画像の
/// FILL（70%）を占めるよう等方スケールで決める。
/// 内側を cyan、外側を yellow とし、距離の等高線を周期的な明暗で、ゼロ等位線を
/// マゼンタで強調する。
pub fn preview(sdf: impl Fn(DVec2) -> f64, png: &Path) {
	const SIZE: u32 = 512;
	const FILL: f64 = 0.7;

	let [min, max] = bounding(&sdf);
	let center = (min + max) * 0.5;
	let long = (max - min).max_element().max(f64::EPSILON);
	let world_per_px = long / (SIZE as f64 * FILL);

	let mut pixmap = Pixmap::new(SIZE, SIZE).unwrap();

	// ── SDF 背景 ──────────────────────────────────────────────────────
	{
		let pixels = pixmap.pixels_mut();
		for y in 0..SIZE {
			for x in 0..SIZE {
				let p = center
					+ DVec2::new(
						(x as f64 / SIZE as f64 - 0.5) * long,
						(y as f64 / SIZE as f64 - 0.5) * long,
					);
				let d = sdf(p);
				let t = (-3.0 * d.abs() / long).exp() as f32; // 1 at |d|=0, 0 at |d|=∞
				let d_f32 = d as f32;
				let wpx_f32 = world_per_px as f32;
				let mut col = if d >= 0.0 {
					Vec3::new(1.0, 1.0, 0.0) * t // 黄 → 黒
				} else {
					Vec3::new(0.0, 1.0, 1.0).lerp(Vec3::splat(1.0), 1.0 - t) // cyan → 白
				};
				let cycle = d_f32 / wpx_f32 / 12.0 * std::f32::consts::TAU; // 12px ごとの等高線
				col *= 0.8 + 0.2 * cycle.cos();
				let edge = 1.0 - smoothstep(0.0, 1.5 * wpx_f32, d_f32.abs());
				col = col.lerp(Vec3::new(1.0, 0.0, 1.0), edge); // ゼロ等位線をマゼンタで強調
				let to = |c: f32| (c.clamp(0.0, 1.0) * 255.0) as u8;
				pixels[(y * SIZE + x) as usize] =
					PremultipliedColorU8::from_rgba(to(col.x), to(col.y), to(col.z), 255).unwrap();
			}
		}
	}

	// ── セグメント描画 ────────────────────────────────────────────────
	// ワールド座標 (f64) → ピクセル座標 (f32 — tiny-skia 用)
	let w2p = |p: DVec2| -> (f32, f32) {
		(
			((p.x - center.x) / long * SIZE as f64 + SIZE as f64 * 0.5) as f32,
			((p.y - center.y) / long * SIZE as f64 + SIZE as f64 * 0.5) as f32,
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

	for (i, comp_segs) in regions(&sdf).iter().enumerate() {
		let (r, g, b) = PALETTE[i % PALETTE.len()];
		let mut paint = Paint::default();
		paint.set_color_rgba8(r, g, b, 220);
		paint.anti_alias = true;
		let stroke = Stroke { width: 1.5, ..Default::default() };

		for seg in comp_segs {
			match seg {
				Segment::Line { point, direction } => {
					let (px, py) = w2p(*point);
					let dpx = direction.normalize_or_zero();
					let t = SIZE as f32 * 1.5;
					let dx = dpx.x as f32;
					let dy = dpx.y as f32;
					let mut pb = PathBuilder::new();
					pb.move_to(px - dx * t, py - dy * t);
					pb.line_to(px + dx * t, py + dy * t);
					if let Some(path) = pb.finish() {
						pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
					}
				}
				Segment::Circle { center: c, radius } => {
					let (cx, cy) = w2p(*c);
					let r_px = (*radius / long * SIZE as f64) as f32;
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

	if let Some(parent) = png.parent() { std::fs::create_dir_all(parent).ok(); }
	pixmap.save_png(png).unwrap();
}
