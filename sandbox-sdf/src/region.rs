//! SDF の内外マップ・連結成分・形状分類。

use crate::{bounding, distance_nabla_laplacian};
use glam::Vec2;
use std::collections::{HashSet, VecDeque};

pub const RES: usize = 1024;

/// SDF を 1024×1024 グリッドで評価し、内外マスクを返す。
/// bbox は bounding に 10% マージンを加えたもの。
pub fn inner_map(sdf: impl Fn(Vec2) -> f32) -> ([Vec2; 2], Vec<bool>) {
	let [raw_min, raw_max] = bounding(&sdf);
	let margin = (raw_max - raw_min) * 0.1;
	let min = raw_min - margin;
	let max = raw_max + margin;
	let size = max - min;
	let mut map = Vec::with_capacity(RES * RES);
	for row in 0..RES {
		for col in 0..RES {
			let p = min
				+ Vec2::new(
					(col as f32 + 0.5) / RES as f32 * size.x,
					(row as f32 + 0.5) / RES as f32 * size.y,
				);
			map.push(sdf(p) < 0.0);
		}
	}
	([min, max], map)
}

/// Moore 8近傍トレース: false (外側) 境界画素集合からループを順序付きで返す。
fn trace_boundary(start: usize, boundary: &HashSet<usize>) -> Vec<usize> {
	// 8方向 CCW 順: E, NE, N, NW, W, SW, S, SE
	const DIRS: [(i32, i32); 8] =
		[(0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1), (1, 0), (1, 1)];
	let rc = |idx: usize| ((idx / RES) as i32, (idx % RES) as i32);
	let at = |r: i32, c: i32| -> Option<usize> {
		if r >= 0 && r < RES as i32 && c >= 0 && c < RES as i32 {
			Some(r as usize * RES + c as usize)
		} else {
			None
		}
	};
	if boundary.is_empty() {
		return vec![];
	}
	let mut path = vec![start];
	let mut entry = 4usize; // W から入ったと仮定
	for _ in 0..boundary.len() * 3 {
		let cur = *path.last().unwrap();
		let (cr, cc) = rc(cur);
		let back = (entry + 4) % 8;
		let mut moved = false;
		for i in 1..=8 {
			let dir = (back + i) % 8;
			let (dr, dc) = DIRS[dir];
			if let Some(nb) = at(cr + dr, cc + dc) {
				if boundary.contains(&nb) {
					if nb == start && path.len() > 2 {
						return path;
					}
					path.push(nb);
					entry = dir;
					moved = true;
					break;
				}
			}
		}
		if !moved {
			break;
		}
	}
	path
}

/// SDF の外側 (false) 連結成分ごとに境界画素を順序付きで返す。
///
/// 各要素は `(p, d, nabla, lap)`:
/// - `p`: ピクセル中心のワールド座標 (射影前)
/// - `d`: `sdf(p)`
/// - `nabla`: `∇d` at p
/// - `lap`: `∇²d` at p
///
/// 成分は面積降順 (最大 = 外周境界、小さい方 = 穴・窓)。
pub fn regions_raw(sdf: impl Fn(Vec2) -> f32) -> Vec<Vec<(Vec2, f32, Vec2, f32)>> {
	let ([min, max], map) = inner_map(&sdf);
	let size = max - min;
	let idx_to_world = |idx: usize| -> Vec2 {
		let row = idx / RES;
		let col = idx % RES;
		min + Vec2::new(
			(col as f32 + 0.5) / RES as f32 * size.x,
			(row as f32 + 0.5) / RES as f32 * size.y,
		)
	};

	// false 成分ごとの (面積, 境界ピクセル集合) を BFS で収集
	let mut visited = vec![false; RES * RES];
	let mut components: Vec<(usize, HashSet<usize>)> = Vec::new();

	for start in 0..RES * RES {
		if visited[start] || map[start] {
			continue;
		}
		let mut queue = VecDeque::new();
		queue.push_back(start);
		visited[start] = true;
		let mut area = 0usize;
		let mut boundary: HashSet<usize> = HashSet::new();

		while let Some(idx) = queue.pop_front() {
			area += 1;
			let row = (idx / RES) as i32;
			let col = (idx % RES) as i32;
			let mut adj_true = false;
			for (dr, dc) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
				let nr = row + dr;
				let nc = col + dc;
				if nr < 0 || nr >= RES as i32 || nc < 0 || nc >= RES as i32 {
					continue;
				}
				let nidx = nr as usize * RES + nc as usize;
				if map[nidx] {
					adj_true = true;
				} else if !visited[nidx] {
					visited[nidx] = true;
					queue.push_back(nidx);
				}
			}
			if adj_true {
				boundary.insert(idx);
			}
		}
		if !boundary.is_empty() {
			components.push((area, boundary));
		}
	}

	// 面積降順: 最大 = 外周、小さい方 = 穴
	components.sort_unstable_by(|a, b| b.0.cmp(&a.0));

	components
		.into_iter()
		.map(|(_, boundary)| {
			let start = *boundary.iter().min().unwrap();
			let ordered = trace_boundary(start, &boundary);
			ordered
				.into_iter()
				.map(|idx| {
					let q = idx_to_world(idx);
					let (d, nabla, lap) = distance_nabla_laplacian(q, &sdf);
					(q, d, nabla, lap)
				})
				.collect()
		})
		.collect()
}

/// 境界上の直線・円弧を表すセグメント。
pub enum Segment {
	Line { point: Vec2, direction: Vec2 },
	Circle { center: Vec2, radius: f32 },
}

/// `regions_raw` の出力を受け取り、連結成分ごとに `Segment` 列を返す。
///
/// 分類 (スケール不変):
/// - `d * |lap| > 0.8` → Corner (スキップ: `d·∇²d = 1` の角点条件)
/// - `|lap| < 0.3`      → Line
/// - その他             → Circle
pub fn regions_segment(regions: &[Vec<(Vec2, f32, Vec2, f32)>]) -> Vec<Vec<Segment>> {
	regions
		.iter()
		.map(|loop_pts| {
			// コーナーを除いた (loop インデックス, is_line) 列
			let non_corners: Vec<(usize, bool)> = loop_pts
				.iter()
				.enumerate()
				.filter_map(|(i, &(_, d, _, lap))| {
					let l = lap.abs();
					if d * l > 0.8 { None } else { Some((i, l < 0.3)) }
				})
				.collect();

			let mut segments = Vec::new();
			let mut j = 0;
			while j < non_corners.len() {
				let is_line = non_corners[j].1;
				let run_start = j;
				while j < non_corners.len() && non_corners[j].1 == is_line {
					j += 1;
				}
				let run: Vec<&(Vec2, f32, Vec2, f32)> =
					non_corners[run_start..j].iter().map(|&(i, _)| &loop_pts[i]).collect();
				if run.is_empty() {
					continue;
				}
				let n = run.len() as f32;

				if is_line {
					let point =
						run.iter().fold(Vec2::ZERO, |a, pt| a + pt.0) / n;
					let mean_n = run
						.iter()
						.fold(Vec2::ZERO, |a, pt| a + pt.2.normalize_or_zero())
						/ n;
					let direction = Vec2::new(-mean_n.y, mean_n.x).normalize_or_zero();
					segments.push(Segment::Line { point, direction });
				} else {
					// center = p − (1/|lap|) × nabla_unit
					let center = run.iter().fold(Vec2::ZERO, |a, pt| {
						let r = (pt.3.abs()).recip().min(1e6_f32);
						a + pt.0 - r * pt.2.normalize_or_zero()
					}) / n;
					let mean_inv_lap =
						run.iter().map(|pt| pt.3.abs().recip().min(1e6_f32)).sum::<f32>() / n;
					let mean_d = run.iter().map(|pt| pt.1).sum::<f32>() / n;
					let radius = (mean_inv_lap - mean_d).max(f32::EPSILON);
					segments.push(Segment::Circle { center, radius });
				}
			}
			segments
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sdf_circle;

	#[test]
	fn inner_map() {
		let (bbox, map) = super::inner_map(sdf_circle);
		let [min, max] = bbox;
		assert!(min.x < -1.0 && min.y < -1.0, "min={min}");
		assert!(max.x > 1.0 && max.y > 1.0, "max={max}");
		assert_eq!(map.len(), RES * RES);
		assert!(map.iter().any(|&v| v), "no inside pixels");
		assert!(map.iter().any(|&v| !v), "no outside pixels");
		assert!(map[RES / 2 * RES + RES / 2], "center should be inside");
	}

	#[test]
	fn regions_raw() {
		let raw = super::regions_raw(sdf_circle);
		assert_eq!(raw.len(), 1, "circle: 1 false component");
		let pts = &raw[0];
		assert!(!pts.is_empty(), "non-empty loop");
		for &(_, d, nabla, lap) in pts {
			assert!(d > 0.0 && d < 0.05, "d={d}");
			assert!((nabla.length() - 1.0).abs() < 0.1, "|nabla|={}", nabla.length());
			// EPS=1e-3 と境界画素 d≈0.002 が同オーダーのため f32 精度で lap に誤差が出る
			// 分類閾値 (0.3/0.8) が安定して機能することが重要で精度は問わない
			assert!(lap > 0.3 && lap < 2.0, "lap={lap}");
		}
	}

	#[test]
	fn regions_segment() {
		let raw = super::regions_raw(sdf_circle);
		let segs = super::regions_segment(&raw);
		assert_eq!(segs.len(), 1, "1 component");
		let comp = &segs[0];
		assert_eq!(comp.len(), 1, "circle: 1 segment");
		match comp[0] {
			Segment::Circle { center, radius } => {
				assert!(center.length() < 0.2, "center={center}");
				assert!((radius - 1.0).abs() < 0.15, "radius={radius}");
			}
			Segment::Line { .. } => panic!("expected Circle, got Line"),
		}
	}
}
