//! SDF（符号付き距離関数）のサンドボックス。
//! 基本形状の SDF と、形状の解析（バウンディングボックス）を提供する。

pub mod boolean;
pub mod preview;

use glam::Vec2;

/// 原点中心・半径1の円の符号付き距離関数。
/// 円の内側で負、円周上で0、外側で正を返す。
pub fn sdf_circle(p: Vec2) -> f32 {
	p.length() - 1.0
}

/// SDF を origin だけ平行移動し scale 倍に拡縮する。
/// 評価点を局所座標へ変換して sdf を呼び、距離を scale 倍して戻すことで
/// 戻り値が正しい符号付き距離（距離の単位）であり続ける。
pub fn sdf_translate(p: Vec2, origin: Vec2, scale: f32, sdf: impl Fn(Vec2) -> f32) -> f32 {
	sdf((p - origin) / scale) * scale
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

/// 点 p における SDF の勾配 ∇sdf を返す。各点で「距離が最も増える方向」を指す。
/// 解析微分の代わりに微小量 EPS の中心差分で近似する。
/// 真の SDF であれば ∇sdf の長さはほぼ1（向きだけ欲しければ呼び出し側で normalize する）。
pub fn nabla(p: Vec2, sdf: impl Fn(Vec2) -> f32) -> Vec2 {
	const EPS: f32 = 1e-3;
	let dx = sdf(p + Vec2::new(EPS, 0.0)) - sdf(p - Vec2::new(EPS, 0.0));
	let dy = sdf(p + Vec2::new(0.0, EPS)) - sdf(p - Vec2::new(0.0, EPS));
	Vec2::new(dx, dy) / (2.0 * EPS)
}

/// 方向 dir に最も張り出す境界点（support point）をニュートン法で求める。
/// dir 方向のはるか遠方から出発し、各反復で点を sdf=0 の面へ寄せていく。
///
/// 1反復は `p -= sdf(p) * n`（n は ∇sdf の単位ベクトル）。これは点 p を通り
/// n 方向に伸びる直線上で方程式 sdf=0 を解くニュートン法そのもの。真の SDF なら
/// 遠方の ∇sdf は形状の最寄り点（= その向きの極値点）を正確に指すため、ごく
/// 数反復で support point に収束する（無限遠だと sdf も ∇sdf も発散するので、
/// 実際には形状サイズより十分大きい有限の FAR から出発する）。
fn support(p: Vec2, sdf: &impl Fn(Vec2) -> f32) -> Vec2 {
	let mut p = p;
	for _ in 0..64 {
		let n = nabla(p, sdf).normalize_or_zero();
		p -= sdf(p) * n;
	}
	p
}

/// 任意の SDF が表す形状の軸並行バウンディングボックスを [min, max] で返す。
/// グリッド走査ではなく、+x / +y / -x / -y の4方向それぞれで support point を
/// ニュートン法で求め、その x / y 成分から最小・最大を組み立てる。
pub fn bounding(sdf: impl Fn(Vec2) -> f32) -> [Vec2; 2] {
	const FAR: f32 = 1e3; // 形状サイズより十分遠い出発点
	let x_max = support(Vec2::X * FAR, &sdf).x;
	let y_max = support(Vec2::Y * FAR, &sdf).y;
	let x_min = support(-Vec2::X * FAR, &sdf).x;
	let y_min = support(-Vec2::Y * FAR, &sdf).y;
	[Vec2::new(x_min, y_min), Vec2::new(x_max, y_max)]
}
