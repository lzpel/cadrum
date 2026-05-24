//! 基本形状の符号付き距離関数 (SDF) プリミティブ。

use glam::DVec2;

/// 原点中心・半径1の円の符号付き距離関数。
/// 円の内側で負、円周上で0、外側で正を返す。
pub fn sdf_circle(p: DVec2) -> f64 {
	p.length() - 1.0
}

/// 任意の多角形（凸でなくてもよい）の符号付き距離関数。
/// vertex は閉路の頂点列。内側で負、外側で正を返す。
/// 各辺への最短距離を求めつつ、巻き数判定で符号を決める（Inigo Quilez 方式）。
pub fn sdf_polygon(p: DVec2, vertex: impl IntoIterator<Item = DVec2>) -> f64 {
	let v: Vec<DVec2> = vertex.into_iter().collect();
	let n = v.len();
	let mut d = (p - v[0]).length_squared();
	let mut s = 1.0_f64;
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
pub fn sdf_translate(p: DVec2, origin: DVec2, scale: f64, sdf: impl Fn(DVec2) -> f64) -> f64 {
	sdf((p - origin) / scale) * scale
}

/// 2頂点 a, b を対角とする軸並行矩形の符号付き距離関数。
/// a と b の順序・大小は問わない。内側で負、外側で正。
pub fn sdf_rect(p: DVec2, a: DVec2, b: DVec2) -> f64 {
	let half = (b - a).abs() * 0.5;
	let d = (p - (a + b) * 0.5).abs() - half;
	d.max(DVec2::ZERO).length() + d.max_element().min(0.0)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sdf_circle() {
		assert_eq!(super::sdf_circle(DVec2::X), 0.0); // 境界上
		assert_eq!(super::sdf_circle(DVec2::ZERO), -1.0); // 中心 = -半径
		assert_eq!(super::sdf_circle(DVec2::new(2.0, 0.0)), 1.0); // 外側
	}

	#[test]
	fn sdf_translate() {
		// origin=(1,0) に平行移動: (2,0) が境界上
		let d = super::sdf_translate(DVec2::new(2.0, 0.0), DVec2::X, 1.0, super::sdf_circle);
		assert_eq!(d, 0.0);
		// scale=2: 半径2の円になる。(2,0) が境界上
		let d = super::sdf_translate(DVec2::new(2.0, 0.0), DVec2::ZERO, 2.0, super::sdf_circle);
		assert_eq!(d, 0.0);
	}

	#[test]
	fn sdf_polygon() {
		// 5角星（外半径1・内半径0.4の頂点が交互に並ぶ凹多角形）
		let star: Vec<DVec2> = (0..10)
			.map(|i| {
				let r = if i % 2 == 0 { 1.0 } else { 0.4 };
				let a = std::f64::consts::TAU * i as f64 / 10.0 + std::f64::consts::FRAC_PI_2;
				DVec2::new(a.cos(), a.sin()) * r
			})
			.collect();
		let png = std::path::Path::new("out/star.png");
		crate::preview::preview(|p| super::sdf_polygon(p, star.iter().copied()), png);
	}

	#[test]
	fn sdf_rect() {
		let a = DVec2::new(-1.0, -1.0);
		let b = DVec2::new(1.0, 1.0);
		assert_eq!(super::sdf_rect(DVec2::ZERO, a, b), -1.0); // 中心: 辺まで距離1
		assert_eq!(super::sdf_rect(DVec2::new(1.0, 0.0), a, b), 0.0); // 辺上
		assert_eq!(super::sdf_rect(DVec2::new(2.0, 0.0), a, b), 1.0); // 外側（辺から）
		assert_eq!(super::sdf_rect(DVec2::new(2.0, 2.0), a, b), 2.0_f64.sqrt()); // 外側（角から）
		// a, b の順序を逆にしても同じ結果
		assert_eq!(super::sdf_rect(DVec2::ZERO, b, a), -1.0);
	}
}
