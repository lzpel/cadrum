//! SDF の零等位線を marching squares で抽出し、Newton 射影で SDF=0 に貼り付けた
//! 点列ループを返す。後段の Edge フィッティング (region.rs) と直交する責務。

use crate::{bounding, distance_nabla_laplacian};
use glam::DVec2;
use std::collections::{HashMap, HashSet};

/// SDF を入力に、SDF=0 等位線の閉ループを点列で返す。
/// 連結成分ごとに 1 ループ、面積降順 (外周境界が先頭、穴が後ろ)。
///
/// パラメータ:
/// - `res`: marching squares の片辺セル数 (1024 程度を推奨)
/// - `newton_iters`: 各点の Newton 射影反復回数 (8 程度で十分収束)
///
/// パイプライン:
/// 1. `bounding` に 10% マージンを足した範囲を `res²` グリッドで SDF サンプル
/// 2. marching squares で「inside on left」規約の閉ループを抽出 (saddle 解消は中心 SDF)
/// 3. 各点を Newton 射影 `p -= sdf(p) · n̂` で SDF=0 に貼り付け
/// 4. shoelace 面積の絶対値で降順ソート
pub fn point_loop(
	sdf: impl Fn(DVec2) -> f64,
	res: usize,
	newton_iters: usize,
) -> Vec<Vec<DVec2>> {
	let [raw_min, raw_max] = bounding(&sdf);
	let margin = (raw_max - raw_min) * 0.1;
	let min = raw_min - margin;
	let max = raw_max + margin;
	let size = max - min;

	let stride = res + 1;
	let samples: Vec<f64> = (0..stride * stride)
		.map(|i| {
			let [r, c] = [i / stride, i % stride];
			let p = min + DVec2::new(c as f64 / res as f64 * size.x, r as f64 / res as f64 * size.y);
			sdf(p)
		})
		.collect();

	let loops_raw = marching_squares(&samples, min, size, res, &sdf);

	// MS の生交差点 (cell-edge 線形補間) を SDF=0 にニュートン射影する。
	let loops_proj: Vec<Vec<DVec2>> = loops_raw
		.into_iter()
		.map(|pts| project_loop(pts, &sdf, newton_iters))
		.collect();

	// ループを面積降順に並べる: 最大ループ = 外周境界、小さい方 = 穴/窓。
	let mut sorted: Vec<(f64, Vec<DVec2>)> = loops_proj
		.into_iter()
		.map(|pts| (signed_area(&pts).abs(), pts))
		.collect();
	sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
	sorted.into_iter().map(|(_, pts)| pts).collect()
}

// ──────────────────────────────────────────────────────────────────────
// marching squares
// ──────────────────────────────────────────────────────────────────────

/// 16ケース → (from_edge, to_edge) ペア (最大2件)。エッジ番号: B=0, R=1, T=2, L=3。
/// 5, 10 は saddle で center_inside の符号で分岐。
/// 「inside on left」の規約で出力する (SDF d<0 側が進行方向の左)。
fn ms_segments(code: u8, center_inside: bool) -> ([(u8, u8); 2], u8) {
	let z = ([(0u8, 0u8); 2], 0u8);
	match code {
		0 | 15 => z,
		1 => ([(0, 3), (0, 0)], 1),
		2 => ([(1, 0), (0, 0)], 1),
		3 => ([(1, 3), (0, 0)], 1),
		4 => ([(2, 1), (0, 0)], 1),
		5 => if center_inside { ([(0, 1), (2, 3)], 2) } else { ([(0, 3), (2, 1)], 2) },
		6 => ([(2, 0), (0, 0)], 1),
		7 => ([(2, 3), (0, 0)], 1),
		8 => ([(3, 2), (0, 0)], 1),
		9 => ([(0, 2), (0, 0)], 1),
		10 => if center_inside { ([(3, 0), (1, 2)], 2) } else { ([(1, 0), (3, 2)], 2) },
		11 => ([(1, 2), (0, 0)], 1),
		12 => ([(3, 1), (0, 0)], 1),
		13 => ([(0, 1), (0, 0)], 1),
		14 => ([(3, 0), (0, 0)], 1),
		_ => z,
	}
}

/// セル(r,c) の指定エッジ (B=0, R=1, T=2, L=3) のグローバル ID。
/// 隣接セルと共有されるエッジは同一 ID を返す (左下コーナー + 向きで識別)。
fn cell_edge_id(r: usize, c: usize, edge: u8, res: usize) -> u32 {
	let stride = (res + 1) as u32;
	let (rr, cc, dir) = match edge {
		0 => (r as u32,       c as u32,       0),
		1 => (r as u32,       (c + 1) as u32, 1),
		2 => ((r + 1) as u32, c as u32,       0),
		3 => (r as u32,       c as u32,       1),
		_ => unreachable!(),
	};
	(rr * stride + cc) * 2 + dir
}

/// グローバルエッジ ID の交差位置 (線形補間)。
fn edge_crossing_pos(edge_id: u32, samples: &[f64], min: DVec2, size: DVec2, res: usize) -> DVec2 {
	let stride = (res + 1) as u32;
	let corner = edge_id / 2;
	let r = (corner / stride) as usize;
	let c = (corner % stride) as usize;
	let is_vertical = (edge_id & 1) == 1;
	let d0 = samples[r * (res + 1) + c];
	let cell_dx = size.x / res as f64;
	let cell_dy = size.y / res as f64;
	let p0 = min + DVec2::new(c as f64 * cell_dx, r as f64 * cell_dy);
	if is_vertical {
		let d1 = samples[(r + 1) * (res + 1) + c];
		let t = d0 / (d0 - d1);
		p0 + DVec2::new(0.0, t * cell_dy)
	} else {
		let d1 = samples[r * (res + 1) + c + 1];
		let t = d0 / (d0 - d1);
		p0 + DVec2::new(t * cell_dx, 0.0)
	}
}

fn marching_squares(
	samples: &[f64],
	min: DVec2,
	size: DVec2,
	res: usize,
	sdf: &impl Fn(DVec2) -> f64,
) -> Vec<Vec<DVec2>> {
	let stride = res + 1;
	let inside = |idx: usize| samples[idx] < 0.0;
	let mut next: HashMap<u32, u32> = HashMap::new();
	for r in 0..res {
		for c in 0..res {
			let bl = inside(r * stride + c);
			let br = inside(r * stride + c + 1);
			let tr = inside((r + 1) * stride + c + 1);
			let tl = inside((r + 1) * stride + c);
			let code: u8 = (bl as u8) | ((br as u8) << 1) | ((tr as u8) << 2) | ((tl as u8) << 3);
			if code == 0 || code == 15 { continue; }
			let center_inside = if code == 5 || code == 10 {
				let cx = min.x + (c as f64 + 0.5) * (size.x / res as f64);
				let cy = min.y + (r as f64 + 0.5) * (size.y / res as f64);
				sdf(DVec2::new(cx, cy)) < 0.0
			} else { false };
			let (segs, ns) = ms_segments(code, center_inside);
			for k in 0..ns as usize {
				let (from, to) = segs[k];
				next.insert(cell_edge_id(r, c, from, res), cell_edge_id(r, c, to, res));
			}
		}
	}

	let mut visited: HashSet<u32> = HashSet::with_capacity(next.len());
	let mut loops = Vec::new();
	let keys: Vec<u32> = next.keys().copied().collect();
	for &start in &keys {
		if visited.contains(&start) { continue; }
		let mut loop_edges = Vec::new();
		let mut cur = start;
		while visited.insert(cur) {
			loop_edges.push(cur);
			match next.get(&cur) {
				Some(&nx) => {
					cur = nx;
					if cur == start { break; }
				}
				None => break,
			}
		}
		if loop_edges.len() >= 3 {
			let pts: Vec<DVec2> = loop_edges
				.iter()
				.map(|&e| edge_crossing_pos(e, samples, min, size, res))
				.collect();
			loops.push(pts);
		}
	}
	loops
}

// ──────────────────────────────────────────────────────────────────────
// Newton 射影
// ──────────────────────────────────────────────────────────────────────

fn project_loop<F: Fn(DVec2) -> f64>(pts: Vec<DVec2>, sdf: &F, iters: usize) -> Vec<DVec2> {
	pts.into_iter()
		.map(|p| {
			let mut q = p;
			for _ in 0..iters {
				let (d, g, _) = distance_nabla_laplacian(q, sdf);
				let g_unit = g.normalize_or_zero();
				if g_unit == DVec2::ZERO { break; }
				q -= d * g_unit;
				if d.abs() < 1e-6 { break; }
			}
			q
		})
		.collect()
}

/// 多角形の符号付き面積 (shoelace)。CCW で正、CW で負。
fn signed_area(pts: &[DVec2]) -> f64 {
	let n = pts.len();
	if n < 3 { return 0.0; }
	let mut a = 0.0_f64;
	for i in 0..n {
		let p = pts[i];
		let q = pts[(i + 1) % n];
		a += p.x * q.y - q.x * p.y;
	}
	0.5 * a
}
