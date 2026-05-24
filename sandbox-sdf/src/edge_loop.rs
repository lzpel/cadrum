//! SDF=0 等位線の点列ループ (`point_loop`) を入力に、隣接点の勾配ジャンプで
//! コーナーを切り分けた上で bottom-up merge により Line / Circle セグメント列に
//! 集約する。境界抽出は `point_loop` に分離してある。

use crate::{bounding, distance_nabla_laplacian, point_loop::point_loop, Edge, EdgeLoop};
use glam::DVec2;

/// 連続2点の勾配 cos がこれを下回ったらコーナー (= マージ禁止)。cos(0.3 rad) ≈ 0.955。
const CORNER_COS_THRESHOLD: f64 = 0.955;
/// フィット残差の許容値 (bbox 対角線比)。
const FIT_TOL_REL: f64 = 0.003;
/// Circle として認める最小半径 (bbox 対角線比)。これ未満は corner straddle の誤フィット扱い。
const MIN_CIRCLE_RADIUS_REL: f64 = 0.003;
/// point_loop に渡す marching squares 解像度。
const POINT_LOOP_RES: usize = 1024;
/// point_loop に渡す Newton 射影反復回数。
const POINT_LOOP_NEWTON_ITERS: usize = 8;

/// SDF を入力に Line / Circle Edge 列の EdgeLoop 集合を返す。
/// 連結成分ごとに 1 EdgeLoop、面積降順 (外周境界が先頭、穴が後ろ)。
pub fn edge_loop(sdf: impl Fn(DVec2) -> f64) -> Vec<EdgeLoop> {
	let [raw_min, raw_max] = bounding(&sdf);
	let bbox_diag = ((raw_max - raw_min) * 1.2).length(); // 10% マージン込みで point_loop と整合
	let tol = FIT_TOL_REL * bbox_diag;
	let min_r = MIN_CIRCLE_RADIUS_REL * bbox_diag;

	// 各ループを bottom-up merge にかけて Line/Circle セグメント列に集約する。
	// コーナー検出に必要な勾配は fit_segments 内部で sdf から再計算する。
	point_loop(&sdf, POINT_LOOP_RES, POINT_LOOP_NEWTON_ITERS)
		.into_iter()
		.map(|pts| fit_segments(&pts, &sdf, tol, min_r))
		.collect()
}

// ──────────────────────────────────────────────────────────────────────
// bottom-up merge
// ──────────────────────────────────────────────────────────────────────

/// run i (start_idx[i] から run_len[i] 個) の点を pts から循環インデックスで取り出す。
fn run_points(i: usize, pts: &[DVec2], start_idx: &[usize], run_len: &[usize]) -> Vec<DVec2> {
	let n = pts.len();
	(0..run_len[i]).map(|k| pts[(start_idx[i] + k) % n]).collect()
}

/// run i と next_arr[i] をマージしたときの最良フィット残差。コーナーをまたぐと INF。
fn merge_residual(
	i: usize,
	pts: &[DVec2],
	barrier: &[bool],
	start_idx: &[usize],
	run_len: &[usize],
	next_arr: &[usize],
	min_circle_radius: f64,
) -> f64 {
	let n = pts.len();
	let j = next_arr[i];
	if i == j { return f64::INFINITY; }
	let last_of_i = (start_idx[i] + run_len[i] - 1) % n;
	if barrier[last_of_i] { return f64::INFINITY; }
	let mut merged = run_points(i, pts, start_idx, run_len);
	merged.extend(run_points(j, pts, start_idx, run_len));
	let (_, _, lr) = fit_line(&merged);
	let cr = fit_circle(&merged)
		.filter(|&(_, r, _)| r >= min_circle_radius)
		.map(|(_, _, res)| res)
		.unwrap_or(f64::INFINITY);
	lr.min(cr)
}

/// SDF=0 上の点列ループを Line / Circle セグメント列に集約する。
///
/// 設計方針: 「どこで切るか」を SDF 勾配で先に決め、「どう繋ぐか」を残差ベースの
/// 貪欲マージで後から決める。点列だけ見て両方同時に解こうとすると角を曲線に
/// 取り込むなどの誤フィットが起きるが、SDF が提供する勾配情報を切断判断に
/// 専用化することで責任分離している。
///
/// フェーズ:
///   1. 勾配ジャンプで barrier 配列を計算 (コーナー検出)
///   2. 1点=1run の双方向リンクリストを初期化
///   3. bottom-up merge: 最小残差 ≤ tol を貪欲に連結
///   4. 残った run を Line/Circle にフィット (run_len < 3 は corner artifact として捨てる)
fn fit_segments(
	pts: &[DVec2],
	sdf: &impl Fn(DVec2) -> f64,
	tol: f64,
	min_circle_radius: f64,
) -> EdgeLoop {
	let n = pts.len();
	match n {
		0 | 1 => return Vec::new(),
		2 => return vec![Edge::Line { a: pts[0], b: pts[1] }],
		_ => {}
	}

	// ── フェーズ1: コーナー検出 (barrier 配列) ───────────────────────────
	// SDF の勾配 ∇d は零等位線上では法線方向そのもの。隣接2点の単位法線
	// 同士の cos が CORNER_COS_THRESHOLD (= cos 0.3 rad ≈ 0.955) を下回ったら
	// その辺をマージ禁止境界としてマークする。
	// なぜ「フィット残差」ではなく「勾配の不連続」でコーナーを判定するか:
	// 点列だけ見ると鈍角コーナーは曲線と区別しづらく、残差判定では「角を
	// 含んだ円弧」に過大フィットしやすい。SDF 勾配のジャンプは離散的に
	// 出るので閾値判定が安定する。
	let grad = |i: usize| distance_nabla_laplacian(pts[i], sdf).1;
	let barrier: Vec<bool> = (0..n).map(|i| {
		let a = grad(i).normalize_or_zero();
		let b = grad((i + 1) % n).normalize_or_zero();
		a.dot(b) < CORNER_COS_THRESHOLD
	}).collect();

	// ── フェーズ2: run の双方向リンクリスト初期化 ────────────────────────
	// 各 run は (start_idx[i], run_len[i]) で表される循環部分列。
	// next_arr / prev_arr で隣接 run を O(1) で辿れるようにし、マージは
	// 「next の付け替え + run_len 加算 + alive[j] = false」だけで完結する。
	// 配列連結 O(run_len) は実際にフィットする時 (run_points) のみ発生。
	let start_idx: Vec<usize> = (0..n).collect();
	let mut run_len: Vec<usize> = vec![1; n];
	let mut prev_arr: Vec<usize> = (0..n).map(|i| (i + n - 1) % n).collect();
	let mut next_arr: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();
	let mut alive: Vec<bool> = vec![true; n];
	// merge_res[i] = 「run i と next(i) を連結したときの最良残差」。
	// barrier をまたぐ場合 INF。差分更新で使い回す。
	let mut merge_res: Vec<f64> = vec![f64::INFINITY; n];

	for i in 0..n {
		merge_res[i] = merge_residual(i, pts, &barrier, &start_idx, &run_len, &next_arr, min_circle_radius);
	}

	// ── フェーズ3: bottom-up 貪欲マージ ────────────────────────────────
	// 「現存する全 run のうち merge_res が最小、かつ tol 以下」のペアを
	// 1つ選んで連結することを、候補がなくなるまで繰り返す。top-down
	// (Douglas-Peucker 系) との違いは、切断位置を残差で決めずに最初に
	// barrier で固定してしまう点。
	loop {
		// 最小残差でマージ可能なペアを線形走査で探す (全体 O(n²))。
		let mut best: Option<(usize, f64)> = None;
		for i in 0..n {
			if !alive[i] { continue; }
			let r = merge_res[i];
			if r <= tol && best.map_or(true, |(_, br)| r < br) {
				best = Some((i, r));
			}
		}
		let i = match best { Some((i, _)) => i, None => break };

		// run i に j = next(i) を吸収: リンクの付け替え + run_len 加算のみ。
		let j = next_arr[i];
		run_len[i] += run_len[j];
		let nx = next_arr[j];
		next_arr[i] = nx;
		prev_arr[nx] = i;
		alive[j] = false;
		merge_res[j] = f64::INFINITY;

		// 影響を受ける merge_res は i 自身 (後続が変わった) と prev(i)
		// (後続が i に変わり長さも増えた) の2件のみ。他の run は不変なので
		// merge_res をそのまま使い回せる。これが差分更新のキモ。
		merge_res[i] = if next_arr[i] == i {
			f64::INFINITY
		} else {
			merge_residual(i, pts, &barrier, &start_idx, &run_len, &next_arr, min_circle_radius)
		};
		let p = prev_arr[i];
		if p != i {
			merge_res[p] = merge_residual(p, pts, &barrier, &start_idx, &run_len, &next_arr, min_circle_radius);
		}
	}

	// ── フェーズ4: 残った run を Edge に変換 ────────────────────────
	// run_len < 3 のガード: コーナー付近で barrier に挟まれた1〜2点の
	// 極小 run は marching squares + Newton 射影の組み合わせで生じる
	// corner artifact なので捨てる。真面目に Line/Circle にすると
	// EdgeLoop に微小セグメントが混入する。
	// best_fit_segment は Occam バイアス: Line が tol 以下なら Circle が
	// 勝っても Line を選ぶ (極小半径の Circle 誤フィットを避ける)。
	let mut segs = Vec::new();
	let first = (0..n).find(|&i| alive[i]);
	if let Some(s) = first {
		let mut i = s;
		loop {
			if run_len[i] >= 3 {
				let run_pts = run_points(i, pts, &start_idx, &run_len);
				segs.push(best_fit_segment(&run_pts, tol, min_circle_radius));
			}
			i = next_arr[i];
			if i == s { break; }
		}
	}
	segs
}

/// Line / Circle の両方を試し、Line が tol 以下なら Line (Occam)、そうでなければ
/// 残差の小さい方を採用する。端点は run の始点・終点・中央点をそのまま用いる。
fn best_fit_segment(pts: &[DVec2], tol: f64, min_circle_radius: f64) -> Edge {
	let a = pts[0];
	let b = pts[pts.len() - 1];
	let m = pts[pts.len() / 2];
	let (_, _, lr) = fit_line(pts);
	let circle = fit_circle(pts).filter(|&(_, r, _)| r >= min_circle_radius);
	match circle {
		None => Edge::Line { a, b },
		Some((_, _, cres)) => {
			if lr <= tol || cres >= lr {
				Edge::Line { a, b }
			} else {
				Edge::Circle { a, b, m }
			}
		}
	}
}

// ──────────────────────────────────────────────────────────────────────
// fitting
// ──────────────────────────────────────────────────────────────────────

/// 直線フィット (PCA / TLS)。戻り値: (centroid, direction_unit, max_perpendicular_residual)
fn fit_line(pts: &[DVec2]) -> (DVec2, DVec2, f64) {
	let n = pts.len() as f64;
	let centroid = pts.iter().copied().fold(DVec2::ZERO, std::ops::Add::add) / n;
	let mut sxx = 0.0_f64; let mut sxy = 0.0_f64; let mut syy = 0.0_f64;
	for p in pts {
		let q = *p - centroid;
		sxx += q.x * q.x; sxy += q.x * q.y; syy += q.y * q.y;
	}
	let trace = sxx + syy;
	let disc = (trace * trace - 4.0 * (sxx * syy - sxy * sxy)).max(0.0).sqrt();
	let lambda_max = 0.5 * (trace + disc);
	let dir = if sxy.abs() > 1e-12_f64 * (trace.abs() + 1.0) {
		DVec2::new(sxy, lambda_max - sxx).normalize_or_zero()
	} else if sxx >= syy { DVec2::X } else { DVec2::Y };
	let normal = DVec2::new(-dir.y, dir.x);
	let max_res = pts.iter().map(|p| ((*p - centroid).dot(normal)).abs()).fold(0.0_f64, f64::max);
	(centroid, dir, max_res)
}

/// 円フィット (Kasa 線形最小二乗、中心化版)。3点未満や共線で None。
fn fit_circle(pts: &[DVec2]) -> Option<(DVec2, f64, f64)> {
	if pts.len() < 3 { return None; }
	let n = pts.len() as f64;
	let mean = pts.iter().copied().fold(DVec2::ZERO, std::ops::Add::add) / n;
	let mut sxx = 0.0_f64; let mut sxy = 0.0_f64; let mut syy = 0.0_f64;
	let mut sxz = 0.0_f64; let mut syz = 0.0_f64; let mut sz = 0.0_f64;
	for p in pts {
		let x = p.x - mean.x; let y = p.y - mean.y;
		let z = x * x + y * y;
		sxx += x * x; sxy += x * y; syy += y * y;
		sxz += x * z; syz += y * z; sz += z;
	}
	let det = sxx * syy - sxy * sxy;
	let scale = sxx + syy;
	if scale <= 0.0 || det.abs() < 1e-12_f64 * scale * scale { return None; }
	let a = (syy * sxz - sxy * syz) / det;
	let b = (sxx * syz - sxy * sxz) / det;
	let c = sz / n;
	let r2 = c + 0.25 * (a * a + b * b);
	if r2 <= 0.0 { return None; }
	let radius = r2.sqrt();
	let center = DVec2::new(mean.x + 0.5 * a, mean.y + 0.5 * b);
	let max_res = pts.iter().map(|p| ((*p - center).length() - radius).abs()).fold(0.0_f64, f64::max);
	Some((center, radius, max_res))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{sdf_circle, sdf_polygon, sdf_rect};

	#[test]
	fn circle() {
		let loops = super::edge_loop(sdf_circle);
		assert_eq!(loops.len(), 1, "1 loop");
		let l = &loops[0];
		assert_eq!(l.len(), 1, "expected 1 segment, got {}", l.len());
		match &l[0] {
			Edge::Circle { a, b, m } => {
				let (c, r) = Edge::circumcircle(*a, *b, *m).expect("non-collinear");
				let ec = c.length();
				let er = (r - 1.0).abs();
				assert!(ec < 1e-7, "center err={ec:e}");
				assert!(er < 1e-7, "radius err={er:e}");
			}
			Edge::Line { .. } => panic!("expected Circle"),
		}
	}

	#[test]
	fn rectangle() {
		let [lo, hi] = [DVec2::new(-1.0, -1.0), DVec2::new(1.0, 1.0)];
		let loops = super::edge_loop(|p| sdf_rect(p, lo, hi));
		assert_eq!(loops.len(), 1);
		let l = &loops[0];
		assert_eq!(l.len(), 4, "expected 4 sides, got {}", l.len());
		let corners = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
		let nearest = |p: DVec2| corners.iter().map(|c| (p - *c).length()).fold(f64::INFINITY, f64::min);
		for seg in l {
			let Edge::Line { a, b } = *seg else { panic!("expected Line"); };
			// 各端点は marching squares のセル境界上にあるので、真のコーナーから
			// 最大でも cell size (≈ 2.2/RES ≈ 2.2e-3) 程度の距離。
			assert!(nearest(a) < 5e-3, "endpoint a={a:?} far from corner");
			assert!(nearest(b) < 5e-3, "endpoint b={b:?} far from corner");
		}
	}

	#[test]
	fn pentagon() {
		let pent: [DVec2;5] = std::array::from_fn(|i| {
			let a = std::f64::consts::TAU * i as f64 / 5.0;
			DVec2::new(a.cos(), a.sin())
		});
		let loops = super::edge_loop(|p| sdf_polygon(p, pent.iter().copied()));
		assert_eq!(loops.len(), 1);
		let l = &loops[0];
		assert_eq!(l.len(), 5, "expected 5 sides");
		let nearest = |p: DVec2| pent.iter().map(|v| (p - *v).length()).fold(f64::INFINITY, f64::min);
		for seg in l {
			let Edge::Line { a, b } = *seg else { panic!("expected Line"); };
			assert!(nearest(a) < 5e-3, "endpoint a={a:?} far from any pent vertex");
			assert!(nearest(b) < 5e-3, "endpoint b={b:?} far from any pent vertex");
		}
	}
}
