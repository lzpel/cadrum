//! SDF（符号付き距離関数）のサンドボックス。
//! 基本形状の SDF と、形状を囲む円の推定を提供する。

pub mod preview;

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
	fn circle_outside() {
		// 半径1の円: 囲む円は中心≈原点・半径≈1 に収束する
		let (c, r) = super::circle_outside(super::sdf_circle);
		assert!(c.length() < 0.1, "center={c}");
		assert!((r - 1.0).abs() < 0.1, "radius={r}");
	}
}
