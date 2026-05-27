use cadrum::{Boolean, Solid};
use glam::DVec3;

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
	let v: Vec<Solid> = Boolean::union_all([&a, &b, &c, &d]).build_vec().unwrap();
	// A∪C と B∪D の 2 グループに分かれる (重なる距離 1.0 で連結)
	assert!(v.len() <= 2, "disjoint groups should be ≤2 solids, got {}", v.len());
}

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

	let multi: Vec<Solid> = (&sphere - &hole_x - &hole_y - &hole_z).build_vec().unwrap();
	let vol: f64 = multi.iter().map(|s| s.volume()).sum();
	assert_eq!(multi.len(), 1, "result should be a single connected solid");
	// V(sphere) ≈ 523.6, V(3 cylinders inside sphere) ≈ 81.9
	// 期待: ≈ 441.7
	assert!((vol - 441.7).abs() < 5.0, "got volume {}", vol);
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

	let multi: Vec<Solid> = Boolean::intersect_all([&sphere, &cyl_x, &cyl_y, &cyl_z]).build_vec().unwrap();
	let vol: f64 = multi.iter().map(|s| s.volume()).sum();
	// 中心の小さなボリュームのみ ≈ 2.4
	assert!(vol > 0.0 && vol < 10.0, "expected small intersection volume, got {}", vol);
}

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
	let out_of_order: Solid = Boolean::union_all([&ring1, &ring3, &ring5, &ring2, &ring4]).build().unwrap();
	let in_order: Solid = Boolean::union_all([&ring1, &ring2, &ring3, &ring4, &ring5]).build().unwrap();

	assert!((out_of_order.volume() - in_order.volume()).abs() < 1e-6,
		"order-independent: {} vs {}", out_of_order.volume(), in_order.volume());
}
