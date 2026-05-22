//! SDF 同士の集合演算（和・積・差）と、その応用としての凸包。
//!
//! 和・積・差は2つの符号付き距離 `a`, `b`（内側で負・外側で正）を受け取り、
//! 合成形状の符号付き距離を返す。`min`/`max` による合成は境界付近で
//! 厳密な距離（とくに凹角の外側）からは僅かにずれるが、符号は常に正しい。

use glam::Vec2;

/// 和（union）: a または b の内側を内側とする。
pub fn union(a: f32, b: f32) -> f32 {
	a.min(b)
}

/// 積（intersection）: a かつ b の内側を内側とする。
pub fn intersection(a: f32, b: f32) -> f32 {
	a.max(b)
}

/// 差（difference）: a の内側から b の内側を取り除く。
/// b の符号を反転（内外を入れ替え）して a と積を取る。
pub fn difference(a: f32, b: f32) -> f32 {
	a.max(-b)
}

/// 任意の SDF の凸包（凹みを埋めた最小の凸形状）の SDF を返す。
///
/// 凹を凸にする操作は和積差のような点ごとの局所演算では書けない。凹みを埋めるか
/// どうかは図形全体の形に依存する大域的な操作だからである。そこで凸包を
/// 「図形を囲むすべての半平面の積」として捉える:
///   1. `dirs` 本の方向 n を一様サンプルする
///   2. 各 n について支持距離 h(n) = max_{p in shape} p·n を求める
///      （その方向に図形が最も張り出す位置。境界の走査が必要なので大域的）
///   3. 半平面 {p : p·n ≤ h(n)} の SDF `p·n - h(n)` を全方向で積（intersection）する
///
/// 星形のような凹多角形を入れると、凹みはどの支持半平面にも切り取られないため
/// 埋まり、外接する凸多角形（星形なら五角形）が得られる。`dirs` を増やすほど
/// 真の凸包に収束する（有限本では各辺がわずかに面取りされた凸多角形になる）。
pub fn convex_hull(sdf: impl Fn(Vec2) -> f32, dirs: usize) -> impl Fn(Vec2) -> f32 {
	// 図形を含むと仮定する範囲 [-R, R]^2 を密にサンプルし、内部点 (sdf ≤ 0) を集める
	const R: f32 = 2.0;
	const GRID: usize = 256;
	let to_world = |i: usize| (i as f32 + 0.5) / GRID as f32 * 2.0 * R - R;
	let inside: Vec<Vec2> = (0..GRID * GRID)
		.map(|k| Vec2::new(to_world(k % GRID), to_world(k / GRID)))
		.filter(|&p| sdf(p) <= 0.0)
		.collect();

	// 各方向の法線と支持距離 h(n) を前計算する
	let planes: Vec<(Vec2, f32)> = (0..dirs)
		.map(|i| {
			let a = std::f32::consts::TAU * i as f32 / dirs as f32;
			let n = Vec2::new(a.cos(), a.sin());
			let h = inside
				.iter()
				.map(|p| p.dot(n))
				.fold(f32::NEG_INFINITY, f32::max);
			(n, h)
		})
		.collect();

	// 凸包 = 全半平面の積。半平面 SDF `p·n - h` を intersection で畳み込む
	move |p| {
		planes
			.iter()
			.map(|&(n, h)| p.dot(n) - h)
			.fold(f32::NEG_INFINITY, intersection)
	}
}
