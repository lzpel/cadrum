//! `Boolean<Solid>` 演算子 (`+`/`-`/`*`)・`Sum`/`Product` 集約・`boolean_build`
//! (BOPAlgo_CellsBuilder) の end-to-end 動作を検証する。
//!
//! 式の正確な DNF 表現は内部実装詳細なので、ここでは「体積」と「結果 Solid 数」
//! というブラックボックス的不変量で検証する。

use cadrum::{Boolean, DVec3, Solid};

/// 原点に置いた cube を `(tx, ty, tz)` だけ平行移動して返す。
fn cube(x: f64, y: f64, z: f64, tx: f64, ty: f64, tz: f64) -> Solid {
	Solid::cube(x, y, z).translate(DVec3::new(tx, ty, tz))
}

// ==================== union (`+` / `Sum`) ====================

#[test]
fn test_union_two_cylinders() {
	// 2 つのオーバーラップする円柱の union は 1 つの Solid になる。
	let a = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(1.0, 0.0, 0.0));
	let b = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(-1.0, 0.0, 0.0));
	let v: Vec<Solid> = (&a + &b).build_vec().unwrap();
	assert_eq!(v.len(), 1, "overlapping cylinders should union to 1 solid");
}

#[test]
fn test_union_disjoint() {
	// 距離 4.0 離れた 2 つの円柱を 4 つ union → 2 つ x 2 つで disjoint なので 4 ペア
	let a = Solid::cylinder(1.1, DVec3::Z, 1.0);
	let b = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(4.0, 0.0, 0.0));
	let c = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, 1.0, 0.0));
	let d = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(4.0, 1.0, 0.0));
	let v: Vec<Solid> = [&a, &b, &c, &d].into_iter().map(Boolean::from).reduce(|a, b| a + b).unwrap().build_vec().unwrap();
	// A∪C と B∪D の 2 グループに分かれる (重なる距離 1.0 で連結)
	assert!(v.len() == 2, "disjoint groups should be 2 solids, got {}", v.len());
}

#[test]
fn test_union_all_connected() {
	// 全て重なる 3 cube → union = 1 Solid (体積は包含領域)
	let a = cube(10.0, 10.0, 10.0, 0.0, 0.0, 0.0);
	let b = cube(10.0, 10.0, 10.0, 3.0, 3.0, 3.0);
	let c = cube(10.0, 10.0, 10.0, 6.0, 6.0, 6.0);
	let s: Solid = [&a, &b, &c].into_iter().map(Boolean::from).reduce(|a, b| a + b).unwrap().build().unwrap();
	// a と c は重なっていない場合がある (距離 6 vs 辺 10) — overlap あるので 1 個
	assert!(s.volume() > a.volume(), "union volume should grow");
}

#[test]
fn test_union_olympic_rings_out_of_order() {
	// 5 つの cube が「隣り同士のみ重なる」鎖状配置: 1-2-3-4-5
	// CellsBuilder は全交差を 1 パスで計算するので並び順に依存しない。
	let s = 1.0;
	let step = 0.8;
	let mk = |i: f64| Solid::cube(s, s, s).translate(DVec3::new(i * step, 0.0, 0.0));
	let ring1 = mk(0.0);
	let ring2 = mk(1.0);
	let ring3 = mk(2.0);
	let ring4 = mk(3.0);
	let ring5 = mk(4.0);

	// out-of-order でも順番通りでも同じ結果
	let out_of_order: Solid = [&ring1, &ring3, &ring5, &ring2, &ring4].into_iter().map(Boolean::from).reduce(|a, b| a + b).unwrap().build().unwrap();
	let in_order: Solid = [&ring1, &ring2, &ring3, &ring4, &ring5].into_iter().map(Boolean::from).reduce(|a, b| a + b).unwrap().build().unwrap();

	assert!((out_of_order.volume() - in_order.volume()).abs() < 1e-6,
		"order-independent: {} vs {}", out_of_order.volume(), in_order.volume());
}

// ==================== intersect (`*` / reduce) ====================

#[test]
fn test_intersect_two_cubes() {
	let a = cube(10.0, 10.0, 10.0, 0.0, 0.0, 0.0);
	let b = cube(10.0, 10.0, 10.0, 5.0, 0.0, 0.0); // overlap 5×10×10 = 500
	let s: Solid = [&a, &b].into_iter().map(Boolean::from).reduce(|x, y| x * y).unwrap().build().unwrap();
	assert!((s.volume() - 500.0).abs() < 1e-3, "got {}", s.volume());
}

#[test]
fn test_intersect_sphere_with_multiple_cylinders() {
	// 球と 3 本円柱の intersect: sphere ∩ cyl_x ∩ cyl_y ∩ cyl_z
	// DNF: [1, 2, 3, 4, 0] (1 clause、4 lit すべて take)
	let sphere = Solid::sphere(5.0);
	let r = 0.8;
	let len = 20.0;
	let half = len / 2.0;
	let cyl_x = Solid::cylinder(r, DVec3::X, len).translate(DVec3::new(-half, 0.0, 0.0));
	let cyl_y = Solid::cylinder(r, DVec3::Y, len).translate(DVec3::new(0.0, -half, 0.0));
	let cyl_z = Solid::cylinder(r, DVec3::Z, len).translate(DVec3::new(0.0, 0.0, -half));

	let multi: Solid = [&sphere, &cyl_x, &cyl_y, &cyl_z].into_iter().map(Boolean::from).reduce(|x, y| x * y).unwrap().build().unwrap();
	// 中心の小さなボリュームのみ ≈ 2.4
	assert!(multi.volume() > 0.0 && multi.volume() < 10.0, "expected small intersection volume, got {}", multi.volume());
}

// ==================== subtract (`-`) ====================

#[test]
fn test_subtract_sphere_with_multiple_holes() {
	// 球から X/Y/Z 軸の 3 本円柱を一括差し引く: sphere - (hole_x ∪ hole_y ∪ hole_z)
	// DNF: sphere ∩ ¬hole_x ∩ ¬hole_y ∩ ¬hole_z = [1, -2, -3, -4, 0]
	let sphere = Solid::sphere(5.0);
	let len = 12.0;
	let half = len / 2.0;
	let r = 1.0;
	let hole_x = Solid::cylinder(r, DVec3::X, len).translate(DVec3::new(-half, 0.0, 0.0));
	let hole_y = Solid::cylinder(r, DVec3::Y, len).translate(DVec3::new(0.0, -half, 0.0));
	let hole_z = Solid::cylinder(r, DVec3::Z, len).translate(DVec3::new(0.0, 0.0, -half));

	let multi: Solid = (&sphere - &hole_x - &hole_y - &hole_z).build().unwrap();
	// V(sphere) ≈ 523.6, V(3 cylinders inside sphere) ≈ 81.9
	// 期待: ≈ 441.7
	assert!((multi.volume() - 441.7).abs() < 5.0, "got volume {}", multi.volume());
}

// ==================== 演算子混在 / 終端評価 ====================

#[test]
fn test_operator_overloads() {
	// `+` / `-` / `*` for Solid/&Solid combinations → Boolean<Solid>
	let a = Solid::cube(10.0, 10.0, 10.0);
	let b = Solid::cube(10.0, 10.0, 10.0).translate(DVec3::new(5.0, 5.0, 5.0));

	let u: Solid = (&a + &b).build().expect("a + b should yield one solid");
	println!("a + b (union):     volume = {:.4}", u.volume());

	let s: Solid = (&a - &b).build().expect("a - b should yield one solid");
	println!("a - b (subtract):  volume = {:.4}", s.volume());

	let i: Solid = (&a * &b).build().expect("a * b should yield one solid");
	println!("a * b (intersect): volume = {:.4}", i.volume());

	// 非交差での intersect → build_vec で 0 個、build で OneFailed(0)
	let far = Solid::cube(1.0, 1.0, 1.0).translate(DVec3::new(100.0, 0.0, 0.0));
	match (&a * &far).build() {
		Err(e @ cadrum::Error::OneFailed(0)) => println!("a * far (disjoint) -> {:?}", e),
		Err(e) => panic!("expected OneFailed(0), got {:?}", e),
		Ok(_) => panic!("expected OneFailed(0), got Ok"),
	}
}

#[test]
fn test_singleton_build() {
	let a = Solid::cube(10.0, 10.0, 10.0);
	let expected = a.volume();
	let b = std::iter::once(&a).map(Boolean::from).reduce(|x, y| x + y).unwrap(); // fold でも reduce でも同じ結果 (単一要素はそのまま)
	let s: Solid = b.build().unwrap();
	assert!((s.volume() - expected).abs() < 1e-6, "{} vs {}", s.volume(), expected);
}

#[test]
fn test_empty_returns_error() {
	let solids: Vec<Solid> = Vec::new();
	match solids.iter().map(Boolean::from).reduce(|x, y| x + y).unwrap_or_else(Boolean::default).build() {
		Err(cadrum::Error::OneFailed(0)) => {}
		other => panic!("expected OneFailed(0), got {:?}", other.is_ok()),
	}
}

#[test]
fn test_build_direct() {
	// Solid::boolean_build を直接呼ぶ低レベルテスト。
	// (A + B) - C で `A=cube@0, B=cube@5, C=cube@2` を計算。
	let a = Solid::cube(10.0, 10.0, 10.0);
	let b = Solid::cube(10.0, 10.0, 10.0).translate(DVec3::new(5.0, 0.0, 0.0));
	let c = Solid::cube(10.0, 10.0, 10.0).translate(DVec3::new(2.0, 0.0, 0.0));
	let solids = vec![a, b, c];
	// (A∪B)∖C → DNF: A∖C ∪ B∖C → clauses [1,-3,0, 2,-3,0]
	let clauses = vec![1, -3, 0, 2, -3, 0];
	let v = Solid::boolean(solids.iter(), clauses).build_vec().unwrap();
	// A∪B の体積は 15×10×10 = 1500、C を引くので減るはず
	let total_volume: f64 = v.iter().map(|s| s.volume()).sum();
	assert!(total_volume < 1500.0);
	assert!(total_volume > 0.0);
}

// ==================== history / face identity ====================

#[test]
fn test_preserves_src_face_identity() {
	// `S::boolean` が shallow copy で TShape を共有することの担保。
	// ユーザが入力 Solid から face id を集めて boolean 結果の history と
	// 照合する用途 (examples/08_shell.rs の halved_shelled_torus 等) が動作するか。
	let torus = Solid::torus(6.0, 2.0, DVec3::Y);
	let cutter = Solid::half_space(DVec3::ZERO, -DVec3::Z);
	let cutter_ids: std::collections::HashSet<u64> =
		cutter.iter_face().map(|f| f.id()).collect();
	let half: Solid = (&torus * &cutter).build().unwrap();
	let matched: Vec<u64> = half.iter_history()
		.filter_map(|[p, s]| cutter_ids.contains(&s).then_some(p))
		.collect();
	assert!(!matched.is_empty(),
		"history must contain at least one face sourced from cutter");
}
