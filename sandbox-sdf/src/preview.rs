//! SDF の距離マップとセグメント輪郭を PNG に書き出すプレビュー。

use crate::{bounding, region::regions, Edge};
use glam::{DVec2, Vec3};
use std::path::Path;
use tiny_skia::{Paint, PathBuilder, Pixmap, PremultipliedColorU8, Stroke, Transform};

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
				// ゼロ等位線は regions(sdf) からの Edge 描画 (下) に一本化する。
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
			let mut pb = PathBuilder::new();
			match seg {
				Edge::Line { a, b } => {
					let (ax, ay) = w2p(*a);
					let (bx, by) = w2p(*b);
					pb.move_to(ax, ay);
					pb.line_to(bx, by);
				}
				Edge::Circle { a, b, m } => {
					// 3 点から円中心を出し、a → m → b を経由する弧を等角度刻みで折れ線近似
					if let Some((c, _radius)) = Edge::circumcircle(*a, *b, *m) {
						let va = *a - c;
						let vb = *b - c;
						let vm = *m - c;
						let arc_sign = (va.x * vm.y - va.y * vm.x).signum();
						let to_angle = |v: DVec2| -> f64 {
							let cross = va.x * v.y - va.y * v.x;
							let dot = va.dot(v);
							let theta = cross.atan2(dot) * arc_sign;
							if theta < 0.0 { theta + std::f64::consts::TAU } else { theta }
						};
						let span = to_angle(vb).max(to_angle(vm)); // closed circle で b≈a のとき m 側を採用
						const N: u32 = 96;
						let r = va.length();
						for k in 0..=N {
							let t = k as f64 / N as f64 * span;
							let theta = t * arc_sign;
							let cs = theta.cos();
							let sn = theta.sin();
							// va を theta だけ回転
							let v = DVec2::new(va.x * cs - va.y * sn, va.x * sn + va.y * cs);
							let p = c + v * (r / va.length().max(f64::EPSILON));
							let (px, py) = w2p(p);
							if k == 0 { pb.move_to(px, py); } else { pb.line_to(px, py); }
						}
					} else {
						// 共線フォールバック: a → b を直線で
						let (ax, ay) = w2p(*a);
						let (bx, by) = w2p(*b);
						pb.move_to(ax, ay);
						pb.line_to(bx, by);
					}
				}
			}
			if let Some(path) = pb.finish() {
				pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
			}
		}
	}

	if let Some(parent) = png.parent() { std::fs::create_dir_all(parent).ok(); }
	pixmap.save_png(png).unwrap();
}
