//! SDF の内外マップ・連結成分・輪郭抽出。

use crate::{bounding, nabla};
use glam::Vec2;
use std::collections::VecDeque;

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

/// 内外マスクから BFS で連結成分を抽出し、各成分の (代表ワールド座標, 概算境界長) を返す。
pub fn regions(bbox: [Vec2; 2], map: &[bool]) -> Vec<(Vec2, f32)> {
    let [min, max] = bbox;
    let size = max - min;
    let world_per_px = (size.x + size.y) * 0.5 / RES as f32;

    let mut visited = vec![false; RES * RES];
    let mut result = Vec::new();

    for start in 0..RES * RES {
        if visited[start] {
            continue;
        }
        let val = map[start];
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited[start] = true;
        let mut boundary_count = 0usize;

        while let Some(idx) = queue.pop_front() {
            let row = (idx / RES) as i32;
            let col = (idx % RES) as i32;
            let mut is_boundary = false;
            for (dr, dc) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nr = row + dr;
                let nc = col + dc;
                if nr < 0 || nr >= RES as i32 || nc < 0 || nc >= RES as i32 {
                    is_boundary = true;
                    continue;
                }
                let nidx = nr as usize * RES + nc as usize;
                if map[nidx] != val {
                    is_boundary = true;
                    continue;
                }
                if !visited[nidx] {
                    visited[nidx] = true;
                    queue.push_back(nidx);
                }
            }
            if is_boundary {
                boundary_count += 1;
            }
        }

        let rep_row = start / RES;
        let rep_col = start % RES;
        let rep_world = min
            + Vec2::new(
                (rep_col as f32 + 0.5) / RES as f32 * size.x,
                (rep_row as f32 + 0.5) / RES as f32 * size.y,
            );
        result.push((rep_world, boundary_count as f32 * world_per_px));
    }
    result
}

/// `start` を最寄り境界に射影後、CCW 接線方向に 1 周し、n_points 点に等間隔リサンプルして返す。
pub fn contour(start: Vec2, sdf: &impl Fn(Vec2) -> f32, n_points: usize) -> Vec<Vec2> {
    let mut p = start;
    for _ in 0..64 {
        let n = nabla(p, sdf).normalize_or_zero();
        p -= sdf(p) * n;
    }
    let p_start = p;

    let step = 0.005_f32;
    let mut path = vec![p_start];

    for _ in 0..20000 {
        let n = nabla(p, sdf).normalize_or_zero();
        let t = Vec2::new(-n.y, n.x); // CCW 接線
        p += t * step;
        for _ in 0..8 {
            let n2 = nabla(p, sdf).normalize_or_zero();
            p -= sdf(p) * n2;
        }
        path.push(p);
        if path.len() > 100 && (p - p_start).length() < step * 2.0 {
            break;
        }
    }

    // 累積弧長
    let mut cumlen = vec![0.0_f32; path.len()];
    for i in 1..path.len() {
        cumlen[i] = cumlen[i - 1] + (path[i] - path[i - 1]).length();
    }
    let total = cumlen[path.len() - 1];
    if total < f32::EPSILON || n_points == 0 {
        return path;
    }

    // 等間隔リサンプル
    (0..n_points)
        .map(|k| {
            let target = total * k as f32 / n_points as f32;
            let i = cumlen.partition_point(|&c| c <= target).saturating_sub(1);
            let i = i.min(path.len() - 2);
            let seg = cumlen[i + 1] - cumlen[i];
            let t = if seg > f32::EPSILON {
                (target - cumlen[i]) / seg
            } else {
                0.0
            };
            path[i].lerp(path[i + 1], t)
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
        // 10% マージン付きで bbox が circle 半径1を囲む
        assert!(min.x < -1.0 && min.y < -1.0, "min={min}");
        assert!(max.x > 1.0 && max.y > 1.0, "max={max}");
        assert_eq!(map.len(), RES * RES);
        assert!(map.iter().any(|&v| v), "no inside pixels");
        assert!(map.iter().any(|&v| !v), "no outside pixels");
        // グリッド中心は円の内側
        assert!(map[RES / 2 * RES + RES / 2], "center should be inside");
    }

    #[test]
    fn regions() {
        let (bbox, map) = super::inner_map(sdf_circle);
        let rs = super::regions(bbox, &map);
        // 内側1成分 + 外側1成分 = 2成分
        assert_eq!(rs.len(), 2, "expected 2 regions, got {}", rs.len());
        for &(_, len) in &rs {
            assert!(len > 0.0, "boundary length should be positive");
        }
        // 内側成分の境界長 ≈ 2π ≈ 6.28
        let &(_, inside_len) = rs
            .iter()
            .find(|&&(p, _)| sdf_circle(p) < 0.0)
            .expect("no inside region");
        assert!(
            (inside_len - 2.0 * std::f32::consts::PI).abs() < 1.5,
            "circle perimeter {inside_len} not close to 2π"
        );
    }

    #[test]
    fn contour() {
        let start = Vec2::new(1.1, 0.0); // circle 外側
        let pts = super::contour(start, &sdf_circle, 100);
        assert_eq!(pts.len(), 100);
        for p in &pts {
            assert!(
                sdf_circle(*p).abs() < 0.05,
                "point {p} off boundary: sdf={}",
                sdf_circle(*p)
            );
        }
    }
}
