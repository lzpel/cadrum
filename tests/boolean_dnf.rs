//! Boolean<Solid> + boolean_build (BOPAlgo_CellsBuilder) の end-to-end 動作を
//! 体積比較で検証する。式の正確な DNF 表現は内部実装詳細なので、
//! ここでは「体積」というブラックボックス的不変量で検証する。

use cadrum::{Boolean, DVec3, Solid};

fn cube(x: f64, y: f64, z: f64, tx: f64, ty: f64, tz: f64) -> Solid {
	Solid::cube(x, y, z).translate(DVec3::new(tx, ty, tz))
}

#[test]
fn test_boolean_singleton_build() {
	let a = Solid::cube(10.0, 10.0, 10.0);
	let expected = a.volume();
	let b = Boolean::union_all([&a]);
	let s: Solid = b.build().unwrap();
	assert!((s.volume() - expected).abs() < 1e-6, "{} vs {}", s.volume(), expected);
}

#[test]
fn test_boolean_union_all_disjoint() {
	let a = cube(1.0, 1.0, 1.0, 0.0, 0.0, 0.0);
	let b = cube(1.0, 1.0, 1.0, 5.0, 0.0, 0.0);
	let c = cube(1.0, 1.0, 1.0, 10.0, 0.0, 0.0);
	// disjoint なので Solid 1 個には纏まらない → build_vec で 3 個
	let v: Vec<Solid> = Boolean::union_all([&a, &b, &c]).build_vec().unwrap();
	assert_eq!(v.len(), 3);
}

#[test]
fn test_boolean_union_all_connected() {
	// 全て重なる 3 cube → union = 1 Solid (体積は包含領域)
	let a = cube(10.0, 10.0, 10.0, 0.0, 0.0, 0.0);
	let b = cube(10.0, 10.0, 10.0, 3.0, 3.0, 3.0);
	let c = cube(10.0, 10.0, 10.0, 6.0, 6.0, 6.0);
	let s: Solid = Boolean::union_all([&a, &b, &c]).build().unwrap();
	// a と c は重なっていない場合がある (距離 6 vs 辺 10) — overlap あるので 1 個
	assert!(s.volume() > a.volume(), "union volume should grow");
}

#[test]
fn test_boolean_intersect_all() {
	let a = cube(10.0, 10.0, 10.0, 0.0, 0.0, 0.0);
	let b = cube(10.0, 10.0, 10.0, 5.0, 0.0, 0.0); // overlap 5×10×10 = 500
	let s: Solid = Boolean::intersect_all([&a, &b]).build().unwrap();
	assert!((s.volume() - 500.0).abs() < 1e-3, "got {}", s.volume());
}

#[test]
fn test_boolean_intersect_disjoint_yields_empty() {
	let a = cube(1.0, 1.0, 1.0, 0.0, 0.0, 0.0);
	let b = cube(1.0, 1.0, 1.0, 10.0, 0.0, 0.0);
	// 非交差 → build_vec で 0 個、build で OneFailed(0)
	let v: Vec<Solid> = Boolean::intersect_all([&a, &b]).build_vec().unwrap();
	assert_eq!(v.len(), 0);
}

#[test]
fn test_boolean_empty_returns_error() {
	let solids: Vec<Solid> = Vec::new();
	match Boolean::union_all(solids.iter()).build() {
		Err(cadrum::Error::OneFailed(0)) => {}
		other => panic!("expected OneFailed(0), got {:?}", other.is_ok()),
	}
}

#[test]
fn test_boolean_build_direct() {
	// Solid::boolean_build を直接呼ぶ低レベルテスト。
	// (A + B) - C で `A=cube@0, B=cube@5, C=cube@2` を計算。
	let a = Solid::cube(10.0, 10.0, 10.0);
	let b = Solid::cube(10.0, 10.0, 10.0).translate(DVec3::new(5.0, 0.0, 0.0));
	let c = Solid::cube(10.0, 10.0, 10.0).translate(DVec3::new(2.0, 0.0, 0.0));
	let solids = vec![a, b, c];
	// (A∪B)∖C → DNF: A∖C ∪ B∖C → clauses [1,-3,0, 2,-3,0]
	let clauses = vec![1, -3, 0, 2, -3, 0];
	let v = Solid::boolean_build(&solids, &clauses).unwrap();
	// A∪B の体積は 15×10×10 = 1500、C を引くので減るはず
	let total_volume: f64 = v.iter().map(|s| s.volume()).sum();
	assert!(total_volume < 1500.0);
	assert!(total_volume > 0.0);
}
