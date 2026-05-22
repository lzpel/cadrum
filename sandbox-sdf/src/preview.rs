//! SDF の距離マップを PNG に書き出すプレビュー。

use crate::circle_outside;
use glam::{Vec2, Vec3};
use std::path::Path;
use tiny_skia::{Pixmap, PremultipliedColorU8};

/// エルミート補間（GLSL の smoothstep 相当）。
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
	let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
}

/// SDF をピクセルごとに評価して距離マップを PNG 出力する。
///
/// 表示範囲は `circle_outside(sdf)` が返す囲み円から決める。円の中心を画像
/// 中央に置き、直径が画像の FILL（70%）を占めるよう等方スケールする。
///
/// 内側を青系・外側をオレンジ系で塗り、距離の等高線を縞、ゼロ等位線を白で描く。
pub fn preview(sdf: impl Fn(Vec2) -> f32, png: &Path) {
	const SIZE: u32 = 512;
	const FILL: f32 = 0.7; // 形状の長辺が画像に占める割合

	// 形状の中心と長辺から、ピクセル⇔ワールドの等方スケールを決める
	let (center, radius) = circle_outside(&sdf);
	let long = (2.0 * radius).max(f32::EPSILON);
	let world_per_px = long / (SIZE as f32 * FILL);

	let mut pixmap = Pixmap::new(SIZE, SIZE).unwrap();
	let pixels = pixmap.pixels_mut();
	// ピクセル座標 -> 中心からのワールドオフセット
	for y in 0..SIZE {
		for x in 0..SIZE {
			let p = center + Vec2::new((x as f32/SIZE as f32-0.5) * long, (y as f32/SIZE as f32-0.5) * long); // y は上向き
			let d = sdf(p);

			// 内外で色相を変える（IQ 方式の距離可視化）
			let mut col = Vec3::splat(1.0) - d.signum() * Vec3::new(0.1, 0.4, 0.7);
			col *= 1.0 - (-3.0 * d.abs() / long).exp(); // 距離で減衰（形状サイズで正規化）
			let cycle = d / world_per_px / 12.0 * std::f32::consts::TAU; // 12px ごとの等高線
			col *= 0.8 + 0.2 * cycle.cos();
			let edge = 1.0 - smoothstep(0.0, 1.5 * world_per_px, d.abs());
			col = col.lerp(Vec3::splat(1.0), edge); // ゼロ等位線を白で強調

			let to = |c: f32| (c.clamp(0.0, 1.0) * 255.0) as u8;
			pixels[(y * SIZE + x) as usize] =
				PremultipliedColorU8::from_rgba(to(col.x), to(col.y), to(col.z), 255).unwrap();
		}
	}
	pixmap.save_png(png).unwrap();
}
