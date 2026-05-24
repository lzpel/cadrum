//! SDF（符号付き距離関数）のサンドボックス。
//! 基本形状の SDF (primitive) と、形状を囲むバウンディングボックスの推定を提供する。

pub mod issue;
pub mod preview;
pub mod primitive;
pub mod region;

pub use primitive::*;

use glam::DVec2;

/// 境界上の直線・円弧を表すセグメント。
pub enum Segment {
	Line { point: DVec2, direction: DVec2 },
	Circle { center: DVec2, radius: f64 },
}

impl Segment {
	/// セグメントの両端点を返す。Line は十分長い線分として扱う。
	pub fn distance(&self, p: DVec2) -> f64 {
		match self {
			Segment::Line { point, direction } => {
				let d = p - *point;
				let t = d.dot(*direction) / direction.length_squared().max(f64::EPSILON);
				(d - *direction * t).length()
			}
			Segment::Circle { center, radius } => (p - *center).length() - *radius,
		}
	}
}

pub type EdgeLoop = Vec<Segment>;

/// 点 p における SDF の値・勾配・ラプラシアンを5点差分で同時に返す。
/// 戻り値: `(d, ∇d, ∇²d)` — (距離, 勾配, ラプラシアン)
pub fn distance_nabla_laplacian(p: DVec2, sdf: impl Fn(DVec2) -> f64) -> (f64, DVec2, f64) {
	// f64 のマシン精度 ≈ 1e-16 → 一次微分の最適 h ≈ 1e-5, 二次微分の最適 h ≈ 1e-4。
	// 両方をそこそこ通すために 1e-5 を採用。
	const EPS: f64 = 1e-5;
	let c  = sdf(p);
	let px = sdf(p + DVec2::X * EPS);
	let mx = sdf(p - DVec2::X * EPS);
	let py = sdf(p + DVec2::Y * EPS);
	let my = sdf(p - DVec2::Y * EPS);
	let nabla = DVec2::new(px - mx, py - my) / (2.0 * EPS);
	let lap   = (px + mx + py + my - 4.0 * c) / (EPS * EPS);
	(c, nabla, lap)
}

/// SDF が表す形状の軸並行バウンディングボックスを [min, max] で返す。
///
/// 1. 巨大な円から「形状に貼りつく外接円 (center, radius)」を反復で求める:
///    円周 N 点で sdf を測り、最遠点から離れる向きへ中心を寄せて、最近点の分だけ
///    半径を縮める。最近点が十分0に近づいたら収束。
/// 2. 求めた外接円を 1.5 倍に膨らませた円周上に N' 点を取り、各点をニュートン射影
///    `p -= sdf(p)·n̂` で境界 sdf=0 へ落とす。射影先の min/max が bbox。
///
/// 4方向の support 探索と違い、凹形状でも extremum を取りこぼさない。
pub fn bounding(sdf: impl Fn(DVec2) -> f64) -> [DVec2; 2] {
	// ── Step 1: 外接円 (center, radius) を反復で求める ──────────────────
	const FIT_RATE: f64 = 0.2;
	const FIT_N: usize = 8;
	let mut center = DVec2::ZERO;
	let mut radius = 1e9_f64; // 形状より十分大きい初期半径
	for _ in 0..1000 {
		let probe: Vec<(DVec2, f64)> = (0..FIT_N)
			.map(|i| {
				let a = std::f64::consts::TAU * i as f64 / FIT_N as f64;
				let p = center + radius * DVec2::new(a.cos(), a.sin());
				(p, sdf(p))
			})
			.collect();
		let cmp = |a: &&(DVec2, f64), b: &&(DVec2, f64)| a.1.total_cmp(&b.1);
		let &(far, far_d) = probe.iter().max_by(cmp).unwrap();
		let near_d = probe.iter().min_by(cmp).unwrap().1;
		center += (center - far).normalize_or_zero() * far_d * FIT_RATE;
		radius -= near_d * FIT_RATE;
		if near_d * FIT_RATE < radius * 1e-4 { break; }
	}

	// ── Step 2: 外接円を 1.5 倍に膨らませて N 点から Newton 射影 ─────────
	const N: usize = 256;
	let probe_r = radius * 1.5;
	let mut min = DVec2::splat(f64::INFINITY);
	let mut max = DVec2::splat(f64::NEG_INFINITY);
	for i in 0..N {
		let a = std::f64::consts::TAU * i as f64 / N as f64;
		let mut p = center + probe_r * DVec2::new(a.cos(), a.sin());
		for _ in 0..32 {
			let (d, n, _) = distance_nabla_laplacian(p, &sdf);
			p -= d * n.normalize_or_zero();
		}
		min = min.min(p);
		max = max.max(p);
	}
	[min, max]
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn distance_nabla_laplacian() {
		// (1.1,0): sdf_circle=0.1、∇d=(1,0)、∇²d=1/r≈0.909
		let (d, n, l) =
			super::distance_nabla_laplacian(DVec2::new(1.1, 0.0), super::sdf_circle);
		let ed = (d - 0.1).abs();
		let en = (n - DVec2::X).length();
		let el = (l - 1.0 / 1.1).abs();
		assert!(ed < 1e-15, "d err={ed:e}");
		assert!(en < 1e-11, "n err={en:e}");
		assert!(el < 1e-6, "lap err={el:e}");
		// 辺中央 (1,0): sdf_rect=0、∇²d≈0（直線）
		let a = DVec2::new(-1.0, -1.0);
		let b = DVec2::new(1.0, 1.0);
		let (_, _, l2) = super::distance_nabla_laplacian(DVec2::new(1.0, 0.0), |p| {
			super::sdf_rect(p, a, b)
		});
		assert!(l2.abs() < 1e-5, "rect edge lap err={l2:e}");
	}

	#[test]
	fn bounding() {
		// 半径1の円: bbox は [-1,-1] → [1,1]
		let [min, max] = super::bounding(super::sdf_circle);
		let e1 = (min - DVec2::splat(-1.0)).length();
		let e2 = (max - DVec2::splat(1.0)).length();
		assert!(e1 < 1e-14, "circle min err={e1:e}");
		assert!(e2 < 1e-14, "circle max err={e2:e}");

		// 5角星: 外半径1・内半径0.4。凹形状でも全頂点が境界射影で拾われる
		let star: [DVec2; 10] = std::array::from_fn(|i| {
			let r = if i % 2 == 0 { 1.0_f64 } else { 0.4 };
			let a = std::f64::consts::TAU * i as f64 / 10.0;
			DVec2::new(a.sin(), a.cos()) * r
		});
		let [gt_min, gt_max] = star
			.into_iter()
			.map(|p| [p, p])
			.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
			.unwrap();
		let [min, max] = super::bounding(|p| super::sdf_polygon(p, star.iter().copied()));
		let e3 = (min - gt_min).length();
		let e4 = (max - gt_max).length();
		assert!(e3 < 1e-11, "star min err={e3:e}");
		assert!(e4 < 1e-12, "star max err={e4:e}");
	}
}
