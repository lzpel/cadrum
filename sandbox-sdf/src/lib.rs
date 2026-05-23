//! SDF（符号付き距離関数）のサンドボックス。
//! 基本形状の SDF と、形状を囲む円・バウンディングボックスの推定を提供する。

pub mod issue;
pub mod preview;
pub mod region;

use glam::Vec2;

/// 原点中心・半径1の円の符号付き距離関数。
/// 円の内側で負、円周上で0、外側で正を返す。
pub fn sdf_circle(p: Vec2) -> f32 {
	p.length() - 1.0
}

/// 任意の多角形（凸でなくてもよい）の符号付き距離関数。
/// vertex は閉路の頂点列。内側で負、外側で正を返す。
/// 各辺への最短距離を求めつつ、巻き数判定で符号を決める（Inigo Quilez 方式）。
pub fn sdf_polygon(p: Vec2, vertex: impl IntoIterator<Item = Vec2>) -> f32 {
	let v: Vec<Vec2> = vertex.into_iter().collect();
	let n = v.len();
	let mut d = (p - v[0]).length_squared();
	let mut s = 1.0_f32;
	let mut j = n - 1;
	for i in 0..n {
		let e = v[j] - v[i]; // 辺ベクトル
		let w = p - v[i]; // 始点から評価点へ
		let b = w - e * (w.dot(e) / e.dot(e)).clamp(0.0, 1.0); // 辺上の最近点への差分
		d = d.min(b.length_squared());
		// 評価点が辺をまたぐかで巻き数を更新し、奇数回なら内側
		let c = [p.y >= v[i].y, p.y < v[j].y, e.x * w.y > e.y * w.x];
		if c.iter().all(|&x| x) || c.iter().all(|&x| !x) {
			s = -s;
		}
		j = i;
	}
	s * d.sqrt()
}

/// SDF を origin だけ平行移動し scale 倍に拡縮する。
/// 評価点を局所座標へ変換して sdf を呼び、距離を scale 倍して戻すことで
/// 戻り値が正しい符号付き距離（距離の単位）であり続ける。
pub fn sdf_translate(p: Vec2, origin: Vec2, scale: f32, sdf: impl Fn(Vec2) -> f32) -> f32 {
	sdf((p - origin) / scale) * scale
}

/// 2頂点 a, b を対角とする軸並行矩形の符号付き距離関数。
/// a と b の順序・大小は問わない。内側で負、外側で正。
pub fn sdf_rect(p: Vec2, a: Vec2, b: Vec2) -> f32 {
	let half = (b - a).abs() * 0.5;
	let d = (p - (a + b) * 0.5).abs() - half;
	d.max(Vec2::ZERO).length() + d.max_element().min(0.0)
}

/// SDF が表す形状を囲む円を (中心, 半径) で返す（preview の表示範囲決定用）。
///
/// 巨大な円から出発し、円周 N 点で sdf を測りながら反復で形状へ貼りつける:
///  - 中心: 形状から最も遠い点（クリアランス最大）の逆向きへ寄せる → 形状側へ移動
///  - 半径: 形状に最も近い点（クリアランス最小）の分だけ縮める → 形状に接したら止まる
///
/// 縮小量に「最遠点」のクリアランスを使うと内接円へ収束して形状がはみ出るため、
/// 円が外接し続ける（= circle_outside）よう「最近点」のクリアランスで縮める。
/// rate=0.2 で少しずつ更新し、最近点が十分0に近づいたら収束とみなす。
pub fn circle_outside(sdf: impl Fn(Vec2) -> f32) -> (Vec2, f32) {
	const RATE: f32 = 0.2;
	const N: usize = 8;
	let mut c = Vec2::ZERO;
	let mut r = 1e9_f32; // 形状より十分大きい初期半径（f32::MAX だと多角形 sdf 内の二乗が発散する）
	for _ in 0..1000 {
		// 円周 N 点で sdf を評価する
		let probe: Vec<(Vec2, f32)> = (0..N)
			.map(|i| {
				let a = std::f32::consts::TAU * i as f32 / N as f32;
				let p = c + r * Vec2::new(a.cos(), a.sin());
				(p, sdf(p))
			})
			.collect();
		let cmp = |a: &&(Vec2, f32), b: &&(Vec2, f32)| a.1.total_cmp(&b.1);
		let &(far, far_d) = probe.iter().max_by(cmp).unwrap(); // 最遠点（クリアランス最大）
		let near_d = probe.iter().min_by(cmp).unwrap().1; // 最近点のクリアランス
		// 最遠点から離れる向きへ中心を寄せ、最近点の分だけ半径を縮める
		c += (c - far).normalize_or_zero() * far_d * RATE;
		r -= near_d * RATE;
		if near_d * RATE < r * 1e-4 { break; } // 形状に十分貼りついたら収束
	}
	(c, r)
}

/// 点 p における SDF の値・勾配・ラプラシアンを5点差分で同時に返す。
/// 戻り値: `(d, ∇d, ∇²d)` — (距離, 勾配, ラプラシアン)
pub fn distance_nabla_laplacian(p: Vec2, sdf: impl Fn(Vec2) -> f32) -> (f32, Vec2, f32) {
	const EPS: f32 = 1e-3;
	let c  = sdf(p);
	let px = sdf(p + Vec2::X * EPS);
	let mx = sdf(p - Vec2::X * EPS);
	let py = sdf(p + Vec2::Y * EPS);
	let my = sdf(p - Vec2::Y * EPS);
	let nabla = Vec2::new(px - mx, py - my) / (2.0 * EPS);
	let lap   = (px + mx + py + my - 4.0 * c) / (EPS * EPS);
	(c, nabla, lap)
}

/// SDF が表す形状の軸並行バウンディングボックスを [min, max] で返す。
///
/// circle_outside の囲み円を 1.5 倍に膨らませた円周上に N 点を取り、各点を
/// ニュートン射影 `p -= sdf(p)·n̂`（n̂ は ∇sdf の単位ベクトル）で境界 sdf=0 へ
/// 落とす。射影先は最寄りの境界点なので、凸頂点・凹みのノッチ・複数の連結成分が
/// すべて拾われる。集めた境界点の成分ごとの min/max が bbox になる。
/// 4方向の support 探索と違い、凹形状でも extremum を取りこぼさない。
pub fn bounding(sdf: impl Fn(Vec2) -> f32) -> [Vec2; 2] {
	const N: usize = 256;
	let (center, radius) = circle_outside(&sdf);
	let radius = radius * 1.5; // 全サンプル点を形状の外側に置くための余裕
	let mut min = Vec2::splat(f32::INFINITY);
	let mut max = Vec2::splat(f32::NEG_INFINITY);
	for i in 0..N {
		let a = std::f32::consts::TAU * i as f32 / N as f32;
		let mut p = center + radius * Vec2::new(a.cos(), a.sin());
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
	fn sdf_circle() {
		assert_eq!(super::sdf_circle(Vec2::X), 0.0); // 境界上
		assert_eq!(super::sdf_circle(Vec2::ZERO), -1.0); // 中心 = -半径
		assert_eq!(super::sdf_circle(Vec2::new(2.0, 0.0)), 1.0); // 外側
	}

	#[test]
	fn sdf_translate() {
		// origin=(1,0) に平行移動: (2,0) が境界上
		let d = super::sdf_translate(Vec2::new(2.0, 0.0), Vec2::X, 1.0, super::sdf_circle);
		assert_eq!(d, 0.0);
		// scale=2: 半径2の円になる。(2,0) が境界上
		let d = super::sdf_translate(Vec2::new(2.0, 0.0), Vec2::ZERO, 2.0, super::sdf_circle);
		assert_eq!(d, 0.0);
	}

	#[test]
	fn sdf_polygon() {
		let square = [
			Vec2::new(1.0, 1.0), Vec2::new(-1.0, 1.0),
			Vec2::new(-1.0, -1.0), Vec2::new(1.0, -1.0),
		];
		assert_eq!(super::sdf_polygon(Vec2::ZERO, square), -1.0); // 中心: 辺まで距離1
		assert_eq!(super::sdf_polygon(Vec2::new(1.0, 0.0), square), 0.0); // 辺上
		assert_eq!(super::sdf_polygon(Vec2::new(2.0, 0.0), square), 1.0); // 外側
	}

	#[test]
	fn sdf_rect() {
		let a = Vec2::new(-1.0, -1.0);
		let b = Vec2::new(1.0, 1.0);
		assert_eq!(super::sdf_rect(Vec2::ZERO, a, b), -1.0); // 中心: 辺まで距離1
		assert_eq!(super::sdf_rect(Vec2::new(1.0, 0.0), a, b), 0.0); // 辺上
		assert_eq!(super::sdf_rect(Vec2::new(2.0, 0.0), a, b), 1.0); // 外側（辺から）
		assert!((super::sdf_rect(Vec2::new(2.0, 2.0), a, b) - 2.0_f32.sqrt()).abs() < 1e-5); // 外側（角から）
		// a, b の順序を逆にしても同じ結果
		assert_eq!(super::sdf_rect(Vec2::ZERO, b, a), -1.0);
	}

	#[test]
	fn circle_outside() {
		// 半径1の円: 囲む円は中心≈原点・半径≈1 に収束する
		let (c, r) = super::circle_outside(super::sdf_circle);
		assert!(c.length() < 0.1, "center={c}");
		assert!((r - 1.0).abs() < 0.1, "radius={r}");
	}

	#[test]
	fn distance_nabla_laplacian() {
		// (1.1,0): sdf_circle=0.1、∇d=(1,0)、∇²d=1/r≈0.909
		// d=0.1 >> EPS=1e-3 なので f32 精度で十分安定
		let (d, n, l) =
			super::distance_nabla_laplacian(Vec2::new(1.1, 0.0), super::sdf_circle);
		assert!((d - 0.1).abs() < 1e-3, "d={d}");
		assert!((n - Vec2::X).length() < 1e-2, "n={n}");
		assert!((l - 1.0 / 1.1).abs() < 0.05, "lap={l}");
		// 辺中央 (1,0): sdf_rect=0、∇²d≈0（直線）
		let a = Vec2::new(-1.0, -1.0);
		let b = Vec2::new(1.0, 1.0);
		let (_, _, l2) = super::distance_nabla_laplacian(Vec2::new(1.0, 0.0), |p| {
			super::sdf_rect(p, a, b)
		});
		assert!(l2.abs() < 0.5, "rect edge lap={l2}");
	}

	#[test]
	fn bounding() {
		// 半径1の円: bbox は [-1,-1] → [1,1]
		let [min, max] = super::bounding(super::sdf_circle);
		assert!((min - Vec2::splat(-1.0)).length() < 1e-2, "min={min}");
		assert!((max - Vec2::splat(1.0)).length() < 1e-2, "max={max}");

		// 5角星: 外半径1・内半径0.4。凹形状でも全頂点が境界射影で拾われる
		let star: [Vec2; 10] = std::array::from_fn(|i| {
			let r = if i % 2 == 0 { 1.0_f32 } else { 0.4 };
			let a = std::f32::consts::TAU * i as f32 / 10.0;
			Vec2::new(a.sin(), a.cos()) * r
		});
		let [gt_min, gt_max] = star
			.into_iter()
			.map(|p| [p, p])
			.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
			.unwrap();
		let [min, max] = super::bounding(|p| super::sdf_polygon(p, star.iter().copied()));
		assert!((min - gt_min).length() < 1e-2, "min={min} gt_min={gt_min}");
		assert!((max - gt_max).length() < 1e-2, "max={max} gt_max={gt_max}");
	}
}
