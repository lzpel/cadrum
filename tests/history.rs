//! `Solid::iter_history` population tests (#156).
//!
//! history は「派生系譜」: 結果に生き残った各 face を同種(face)の元 face に
//! 対応付けた flat `[post_id, src_id]` 列。identity(pass-through) ∪ Modified を
//! 含み、Generated(edge→face の新規面) は含まない。詳細は
//! notes/20260603-history定義の明確化.md を参照。

use cadrum::{DVec3, Edge, ProfileOrient, Solid};
use std::collections::HashSet;
use std::f64::consts::TAU;

/// 一辺 `side` の閉じた正方形プロファイル（extrude 用）。
fn square(side: f64) -> Vec<Edge> {
	Edge::polygon(&[DVec3::new(0.0, 0.0, 0.0), DVec3::new(side, 0.0, 0.0), DVec3::new(side, side, 0.0), DVec3::new(0.0, side, 0.0)]).expect("square polygon")
}

/// 入力に face を持たない演算（プリミティブ / edge・grid ソースの builder）は
/// history が空のまま（保持元 face が無いので by design）。
#[test]
fn test_no_face_source_ops_have_empty_history() {
	assert_eq!(Solid::cube(DVec3::ZERO, DVec3::splat(1.0)).iter_history().count(), 0, "cube");
	assert_eq!(Solid::sphere(1.0).iter_history().count(), 0, "sphere");

	let extruded = Solid::extrude(&square(4.0), DVec3::Z * 3.0).expect("extrude");
	assert_eq!(extruded.iter_history().count(), 0, "extrude");

	let profile = [Edge::circle(1.0, DVec3::Z).expect("circle")];
	let spine = [Edge::line(DVec3::ZERO, DVec3::Z * 5.0).expect("line")];
	let swept = Solid::sweep(&profile, &spine, ProfileOrient::Fixed).expect("sweep");
	assert_eq!(swept.iter_history().count(), 0, "sweep");

	let lower = [Edge::circle(3.0, DVec3::Z).expect("circle")];
	let upper = [Edge::circle(1.5, DVec3::Z).expect("circle").translate(DVec3::Z * 8.0)];
	let lofted = Solid::loft(&[lower, upper], false).expect("loft");
	assert_eq!(lofted.iter_history().count(), 0, "loft");

	let bspline = Solid::bspline(16, 8, true, |i, j| {
		let phi = TAU * i as f64 / 16.0;
		let theta = TAU * j as f64 / 8.0;
		let r = 3.0 + theta.cos();
		DVec3::new(r * phi.cos(), r * phi.sin(), theta.sin())
	})
	.expect("bspline torus");
	assert_eq!(bspline.iter_history().count(), 0, "bspline");
}

/// shell: cube の top を開けて内側 offset。残り 5 面は Modified されて outer wall
/// になる。除去された top は Deleted で history に出ない。inner wall は Generated
/// なので出ない。
#[test]
fn test_shell_history_maps_five_retained_faces() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let original: HashSet<u64> = cube.iter_face().map(|f| f.id()).collect();
	let top = cube.iter_face().last().expect("cube has faces");
	let top_id = top.id();
	let shelled = cube.shell(-1.0, [top]).expect("shell");

	let hist: Vec<[u64; 2]> = shelled.iter_history().collect();
	assert!(!hist.is_empty(), "shell must populate history");
	for [_, src] in &hist {
		assert!(original.contains(src), "src {src} is not an original cube face");
	}
	let srcs: HashSet<u64> = hist.iter().map(|[_, s]| *s).collect();
	assert_eq!(srcs.len(), 5, "5 retained faces should map (top removed); got {}", srcs.len());
	assert!(!srcs.contains(&top_id), "removed top face must not appear as src");
}

/// fillet: cube の edge 1 本を fillet → その edge を共有する 2 面が Modified
/// (post != src)。非影響の 4 面は identity (post == src) として現れ、全 6 面が
/// src として登場する（= 無変更面の tshape 保持＝設計前提の検証）。
#[test]
fn test_fillet_history_modifies_adjacent_identity_elsewhere() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let original: HashSet<u64> = cube.iter_face().map(|f| f.id()).collect();
	let edge = cube.iter_edge().next().expect("cube has edges");
	let edge_id = edge.id();
	let adjacent: HashSet<u64> = cube.iter_face().filter(|f| f.iter_edge().any(|e| e.id() == edge_id)).map(|f| f.id()).collect();
	assert_eq!(adjacent.len(), 2, "a cube edge borders exactly 2 faces");

	let filleted = cube.fillet_edges(0.5, [edge]).expect("fillet");
	let hist: Vec<[u64; 2]> = filleted.iter_history().collect();

	for [_, src] in &hist {
		assert!(original.contains(src), "src {src} is not an original cube face");
	}
	let srcs: HashSet<u64> = hist.iter().map(|[_, s]| *s).collect();
	assert_eq!(srcs.len(), 6, "all 6 faces should appear as src (identity preserved); got {}", srcs.len());

	let modified: HashSet<u64> = hist.iter().filter(|[p, s]| p != s).map(|[_, s]| *s).collect();
	for id in &adjacent {
		assert!(modified.contains(id), "adjacent face {id} must be Modified (post != src)");
	}
}

/// chamfer: fillet と同形（edge を共有する 2 面が Modified、全 6 面が src 登場）。
#[test]
fn test_chamfer_history_modifies_adjacent_identity_elsewhere() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let original: HashSet<u64> = cube.iter_face().map(|f| f.id()).collect();
	let edge = cube.iter_edge().next().expect("cube has edges");
	let edge_id = edge.id();
	let adjacent: HashSet<u64> = cube.iter_face().filter(|f| f.iter_edge().any(|e| e.id() == edge_id)).map(|f| f.id()).collect();
	assert_eq!(adjacent.len(), 2, "a cube edge borders exactly 2 faces");

	let chamfered = cube.chamfer_edges(0.5, [edge]).expect("chamfer");
	let hist: Vec<[u64; 2]> = chamfered.iter_history().collect();

	for [_, src] in &hist {
		assert!(original.contains(src), "src {src} is not an original cube face");
	}
	let srcs: HashSet<u64> = hist.iter().map(|[_, s]| *s).collect();
	assert_eq!(srcs.len(), 6, "all 6 faces should appear as src (identity preserved); got {}", srcs.len());

	let modified: HashSet<u64> = hist.iter().filter(|[p, s]| p != s).map(|[_, s]| *s).collect();
	for id in &adjacent {
		assert!(modified.contains(id), "adjacent face {id} must be Modified (post != src)");
	}
}

/// color: fillet 後も Modified/identity 面が src 面の色を history 経由で引き継ぐ
/// （colormap remap が history を流路にしている）。
#[cfg(feature = "color")]
#[test]
fn test_fillet_carries_face_color_via_history() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color("#ff0000");
	let edge = cube.iter_edge().next().expect("cube has edges");
	let filleted = cube.fillet_edges(0.5, [edge]).expect("fillet");

	let hist: Vec<[u64; 2]> = filleted.iter_history().collect();
	assert!(!hist.is_empty(), "fillet must populate history");
	for [post, _] in &hist {
		assert!(filleted.colormap().contains_key(post), "face {post} should inherit color via history");
	}
}
