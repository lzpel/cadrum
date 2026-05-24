//! 基本形状の符号付き距離関数 (SDF) プリミティブ。

use glam::{DVec2, DVec3};

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

/// 頂点ごとにフィレット半径を指定できる多角形の符号付き距離関数。
///
/// `vertex` の各要素は `(x, y, r)`: 位置 (x, y) と頂点での角丸半径 r。
/// `r == 0` ならその角はシャープ、`r > 0` なら隣接2辺に内接する半径 r の円弧で丸める。
/// 凸角・凹角 (interior angle > π) の両方に対応する。
///
/// アルゴリズム:
/// 1. 入力が CW なら CCW に反転 (符号付き面積で判定)。
/// 2. 各フィレット頂点で arc center / tangent offset / 接点方向を事前計算。
/// 3. 距離 = min(クリップ済み各辺との線分距離, 各フィレット弧との半径距離)。
/// 4. 符号 = シャープ多角形の巻き数判定 + フィレット領域での反転補正:
///    - 凸: シャープ角とフィレット弧の間の「くさび」は実は外側 (sharp_inside でも +)
///    - 凹: 凹角のくぼみを弧が埋めた fill 領域は実は内側 (sharp_outside でも −)
pub fn sdf_polygon_rounded(p: DVec2, vertex: impl IntoIterator<Item = DVec3>) -> f64 {
	let mut v: Vec<DVec3> = vertex.into_iter().collect();
	let n = v.len();
	assert!(n >= 3, "polygon needs at least 3 vertices");

	// CCW 化 (符号付き面積が負なら CW なので反転)
	let area: f64 = (0..n).map(|i| {
		let j = (i + 1) % n;
		v[i].x * v[j].y - v[j].x * v[i].y
	}).sum();
	if area < 0.0 { v.reverse(); }

	let pos = |i: usize| DVec2::new(v[i].x, v[i].y);
	let rad = |i: usize| v[i].z;

	// フィレット頂点ごとの幾何データ
	let mut arc_center = vec![DVec2::ZERO; n];
	let mut tan_off = vec![0.0_f64; n];
	let mut t_in_dir = vec![DVec2::ZERO; n];
	let mut t_out_dir = vec![DVec2::ZERO; n];
	let mut convex = vec![false; n];
	let mut has = vec![false; n];

	for i in 0..n {
		if rad(i) <= 0.0 { continue; }
		let prev = (i + n - 1) % n;
		let next = (i + 1) % n;
		let pc = pos(i);
		let e_in = (pc - pos(prev)).normalize();
		let e_out = (pos(next) - pc).normalize();
		let cross = e_in.x * e_out.y - e_in.y * e_out.x;
		let dot = e_in.dot(e_out);
		let turn = cross.atan2(dot);
		// 退化 (直線 ≈ 0 / 折返し ≈ ±π) はフィレット化スキップ
		if turn.abs() < 1e-9 || (std::f64::consts::PI - turn.abs()) < 1e-9 { continue; }

		let half = turn * 0.5;
		let arc_dist = rad(i) / half.cos().abs();
		let offset = rad(i) * half.tan().abs();

		let n_in_in = DVec2::new(-e_in.y, e_in.x);
		let n_out_in = DVec2::new(-e_out.y, e_out.x);
		let bisector_in = (n_in_in + n_out_in).normalize_or_zero();

		// 凸: bisector_in 方向、凹: その逆方向に arc center
		let center = pc + bisector_in * arc_dist * turn.signum();
		let t1 = pc - e_in * offset;
		let t2 = pc + e_out * offset;

		arc_center[i] = center;
		tan_off[i] = offset;
		t_in_dir[i] = (t1 - center).normalize();
		t_out_dir[i] = (t2 - center).normalize();
		convex[i] = turn > 0.0;
		has[i] = true;
	}

	// 距離 (二乗で持つ)
	let mut min_d2 = f64::INFINITY;

	// クリップ済み辺
	for i in 0..n {
		let next = (i + 1) % n;
		let a = pos(i);
		let b = pos(next);
		let e = b - a;
		let len_sq = e.length_squared();
		if len_sq <= 0.0 { continue; }
		let dir = e / len_sq.sqrt();
		let off_a = if has[i] { tan_off[i] } else { 0.0 };
		let off_b = if has[next] { tan_off[next] } else { 0.0 };
		let sa = a + dir * off_a;
		let sb = b - dir * off_b;
		let se = sb - sa;
		let se_sq = se.length_squared();
		if se_sq <= 0.0 { continue; }
		let w = p - sa;
		let t = (w.dot(se) / se_sq).clamp(0.0, 1.0);
		let near = sa + se * t;
		min_d2 = min_d2.min((p - near).length_squared());
	}

	// フィレット弧
	for i in 0..n {
		if !has[i] { continue; }
		let d = (p - arc_center[i]).length() - rad(i);
		min_d2 = min_d2.min(d * d);
	}

	// 符号: シャープ多角形の巻き数判定
	let mut s = 1.0_f64;
	let mut j = n - 1;
	for i in 0..n {
		let a = pos(j);
		let b = pos(i);
		let e = b - a;
		let w = p - a;
		let c = [p.y >= a.y, p.y < b.y, e.x * w.y > e.y * w.x];
		if c.iter().all(|&x| x) || c.iter().all(|&x| !x) {
			s = -s;
		}
		j = i;
	}
	let sharp_inside = s < 0.0;
	let mut rounded_inside = sharp_inside;

	// フィレット領域での符号補正
	for i in 0..n {
		if !has[i] { continue; }
		let to_p = p - arc_center[i];
		let dist = to_p.length();
		if dist < 1e-30 { continue; }
		let dir = to_p / dist;
		let c1 = t_in_dir[i].x * dir.y - t_in_dir[i].y * dir.x;
		let c2 = dir.x * t_out_dir[i].y - dir.y * t_out_dir[i].x;
		let arc_sign = (t_in_dir[i].x * t_out_dir[i].y - t_in_dir[i].y * t_out_dir[i].x).signum();
		let in_span = c1 * arc_sign > 0.0 && c2 * arc_sign > 0.0;
		if !in_span { continue; }
		if convex[i] && sharp_inside && dist > rad(i) {
			rounded_inside = false; // シャープ角と弧の間のくさび → 外側
		}
		if !convex[i] && !sharp_inside && dist < rad(i) {
			rounded_inside = true; // 凹角のくぼみを埋めた領域 → 内側
		}
	}

	let sign = if rounded_inside { -1.0 } else { 1.0 };
	sign * min_d2.sqrt()
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

	#[test]
	fn sdf_polygon_rounded_sharp() {
		// 全頂点 r=0 で sdf_polygon と完全一致
		let square_sharp = [
			DVec2::new(1.0, 1.0), DVec2::new(-1.0, 1.0),
			DVec2::new(-1.0, -1.0), DVec2::new(1.0, -1.0),
		];
		let square_dv3 = [
			DVec3::new(1.0, 1.0, 0.0), DVec3::new(-1.0, 1.0, 0.0),
			DVec3::new(-1.0, -1.0, 0.0), DVec3::new(1.0, -1.0, 0.0),
		];
		for p in [DVec2::ZERO, DVec2::new(1.0, 0.0), DVec2::new(2.0, 0.0), DVec2::new(0.5, 0.3)] {
			let e = (super::sdf_polygon_rounded(p, square_dv3) - super::sdf_polygon(p, square_sharp)).abs();
			assert!(e < 1e-14, "p={p:?} err={e:e}");
		}
	}

	#[test]
	fn sdf_polygon_rounded_uniform() {
		// 全頂点 r=0.1 の正方形 ≡ sdf_polygon(内側に 0.1 縮めた頂点) - 0.1
		const R: f64 = 0.1;
		let rounded = [
			DVec3::new(1.0, 1.0, R), DVec3::new(-1.0, 1.0, R),
			DVec3::new(-1.0, -1.0, R), DVec3::new(1.0, -1.0, R),
		];
		let shrunk = [
			DVec2::new(1.0 - R, 1.0 - R), DVec2::new(-(1.0 - R), 1.0 - R),
			DVec2::new(-(1.0 - R), -(1.0 - R)), DVec2::new(1.0 - R, -(1.0 - R)),
		];
		let probe = [
			DVec2::ZERO,             // 中心
			DVec2::new(1.0, 0.0),    // 右辺上
			DVec2::new(1.0, 0.85),   // 右辺フィレットなし部分
			DVec2::new(1.05, 0.0),   // 右辺の外側
			DVec2::new(1.1, 1.1),    // 右上角の外側 (弧の外)
		];
		for p in probe {
			let got = super::sdf_polygon_rounded(p, rounded);
			let want = super::sdf_polygon(p, shrunk) - R;
			let e = (got - want).abs();
			assert!(e < 1e-12, "p={p:?} got={got} want={want} err={e:e}");
		}
	}

	#[test]
	fn sdf_polygon_rounded_corner() {
		// ユーザー例: 1角だけ R=0.1 (4頂点目の typo は (-1, 1, 0) と解釈)
		let v = [
			DVec3::new( 1.0,  1.0, 0.1),
			DVec3::new( 1.0, -1.0, 0.0),
			DVec3::new(-1.0, -1.0, 0.0),
			DVec3::new(-1.0,  1.0, 0.0),
		];
		// 中心 → -1.0 (辺距離 1)
		assert!((super::sdf_polygon_rounded(DVec2::ZERO, v) + 1.0).abs() < 1e-14);
		// (1, 0) 右辺上 → 0.0 (フィレット非対象側)
		assert!(super::sdf_polygon_rounded(DVec2::new(1.0, 0.0), v).abs() < 1e-14);
		// (-1, 0) 左辺上 → 0.0
		assert!(super::sdf_polygon_rounded(DVec2::new(-1.0, 0.0), v).abs() < 1e-14);
		// (1, 0.9) フィレット接点 T1 上 → 0.0
		assert!(super::sdf_polygon_rounded(DVec2::new(1.0, 0.9), v).abs() < 1e-14);
		// (1, 1) シャープ角の位置 → 外側 (くさび内)
		// 距離 = arc_dist - r = r/sin(π/4) - r = r(√2 - 1)
		let got = super::sdf_polygon_rounded(DVec2::new(1.0, 1.0), v);
		let want = 0.1 * (2.0_f64.sqrt() - 1.0);
		assert!((got - want).abs() < 1e-14, "(1,1): got={got} want={want}");

		// PNG 出力でビジュアル確認 (sdf_polygon のテストに倣う)
		let png = std::path::Path::new("out/polygon_rounded_corner.png");
		crate::preview::preview(|p| super::sdf_polygon_rounded(p, v), png);
	}

	#[test]
	fn sdf_polygon_rounded_concave() {
		// L字 (凹角を含む): (0,0)(2,0)(2,2)(1,1)(0,2) で (1,1) が凹角
		let v = [
			DVec3::new(0.0, 0.0, 0.0),
			DVec3::new(2.0, 0.0, 0.0),
			DVec3::new(2.0, 2.0, 0.0),
			DVec3::new(1.0, 1.0, 0.2),  // 凹フィレット
			DVec3::new(0.0, 2.0, 0.0),
		];
		// (1, 1.1) くぼみ内 + フィレットディスク内 → 凹フィレットの fill 領域 → 内側
		let s1 = super::sdf_polygon_rounded(DVec2::new(1.0, 1.1), v);
		assert!(s1 < 0.0, "concave fill (1,1.1) should be inside: got {s1}");
		// (1, 1.5) くぼみ内 + フィレットディスク外 → 外側
		let s2 = super::sdf_polygon_rounded(DVec2::new(1.0, 1.5), v);
		assert!(s2 > 0.0, "outside concave wedge (1,1.5) should be outside: got {s2}");
		// (1, 0.5) 多角形本体の内側 (フィレット影響なし) → 内側
		let s3 = super::sdf_polygon_rounded(DVec2::new(1.0, 0.5), v);
		assert!(s3 < 0.0, "polygon body (1,0.5) should be inside: got {s3}");

		// PNG 出力でビジュアル確認
		let png = std::path::Path::new("out/polygon_rounded_concave.png");
		crate::preview::preview(|p| super::sdf_polygon_rounded(p, v), png);
	}
}
