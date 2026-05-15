use cadrum::Solid;
use glam::DVec3;

#[test]
fn test_union_cylinders() {
	// 互いに自己交差している円柱

	let a = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(1.0, 0.0, 0.0));
	let b = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(-1.0, 0.0, 0.0));
	let c = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, 1.0, 0.0));
	let d = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, -1.0, 0.0));

	let union_a_b = Solid::boolean_union([&a], [&b]).unwrap();
	println!("union([A], [B]) solid count: {}", union_a_b.len());

	let union_ab_cd = Solid::boolean_union([&a, &b], [&c, &d]).unwrap();

	println!("union([A, B], [C, D]) solid count: {}", union_ab_cd.len());

	let all = [a, b, c, d];
	let union_all_all = Solid::boolean_union(&all, &all).unwrap();
	println!("union([ABCD], [ABCD]) solid count: {}", union_all_all.len());

	// Output:
	// union([A], [B]) solid count: 1
	// union([A, B], [C, D]) solid count: 4
	// union([ABCD], [ABCD]) solid count: 4
	// test test_union_cylinders ... ok
	//
}

#[test]
fn test_union_shifted() {
	// ユーザー指定の座標 (AとBの距離=2.0なので実はr=1.1だと自己交差する)
	let a = Solid::cylinder(1.1, DVec3::Z, 1.0);
	let b = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(2.0, 0.0, 0.0));
	let c = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, 1.0, 0.0));
	let d = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(2.0, 1.0, 0.0));

	let union_ab_cd = Solid::boolean_union([&a, &b], [&c, &d]).unwrap();
	println!("union([A(0,0), B(2,0)], [C(0,1), D(2,1)]) solid count: {}", union_ab_cd.len());

	// 完全にグループ内が自己交差しない座標 (AとBの距離=4.0 > 2.2)
	let a_sep = Solid::cylinder(1.1, DVec3::Z, 1.0);
	let b_sep = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(4.0, 0.0, 0.0));
	let c_sep = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(0.0, 1.0, 0.0));
	let d_sep = Solid::cylinder(1.1, DVec3::Z, 1.0).translate(DVec3::new(4.0, 1.0, 0.0));

	let union_sep = Solid::boolean_union([&a_sep, &b_sep], [&c_sep, &d_sep]).unwrap();
	println!("union([A(0,0), B(4.0,0)], [C(0,1), D(4.0,1)]) solid count: {}", union_sep.len());

	// 結論をプリントする
	println!("\n=== 結論 (Conclusion) ===");
	println!("距離が2.0の場合: 引数(A,B)の配列内ですでに自己交差しているため、1つのSolidに自動結合されず2個のままになります。");
	println!("距離が4.0の場合: 配列内で自己交差がないため、A-CとB-Dの重なりが正しく結合され、2個のSolid(ACの融合体, BDの融合体)になります。");
	println!("つまり、OpenCASCADEの Boolean演算(Fuse) は「グループ間の交差」のみを融合し、「グループ内部(同一配列内)の交差」は無視して結合してくれません。");
	println!("複数の重なるソリッドを結合したい場合は、内部で自己交差のない引数を作るか、事前結合(Sewing/逐次Fuseなど)が必要です。");
	println!("==========================\n");
}

#[test]
fn test_subtract_sphere_with_multiple_holes() {
	// 球の中心を通る3本(X/Y/Z軸)の円柱穴を一括 subtract する。
	// tools=[hole_x, hole_y, hole_z] は中心で互いに交差している(tools内自己交差)。
	// 期待: 球から3本の直交穴が抜かれた1つの Solid
	let sphere = Solid::sphere(5.0);
	let len = 12.0;
	let half = len / 2.0;
	let r = 1.0;
	let hole_x = Solid::cylinder(r, DVec3::X, len).translate(DVec3::new(-half, 0.0, 0.0));
	let hole_y = Solid::cylinder(r, DVec3::Y, len).translate(DVec3::new(0.0, -half, 0.0));
	let hole_z = Solid::cylinder(r, DVec3::Z, len).translate(DVec3::new(0.0, 0.0, -half));

	let multi = Solid::boolean_subtract([&sphere], [&hole_x, &hole_y, &hole_z]).unwrap();
	let multi_vol: f64 = multi.iter().map(|s| s.volume()).sum();
	println!("subtract sphere - [X,Y,Z holes] (multi-tools): count={}, volume={:.4}", multi.len(), multi_vol);
	write_outputs(&multi, "subtract_sphere_multi");

	// 逐次: 1個ずつ引く
	let mut current = vec![sphere.clone()];
	for tool in [&hole_x, &hole_y, &hole_z] {
		current = current.iter().flat_map(|o| Solid::boolean_subtract([o], [tool]).unwrap()).collect();
	}
	let seq_vol: f64 = current.iter().map(|s| s.volume()).sum();
	println!("subtract sphere - X then Y then Z (sequential):    count={}, volume={:.4}", current.len(), seq_vol);
	write_outputs(&current, "subtract_sphere_sequential");

	// === 結論 ===
	// multi-tools: count=1, volume=430.30
	// sequential : count=1, volume=441.61
	// V(sphere)=523.598、V(cyl∩sphere)=31.10 (1本)、V(∪3本 inside sphere)=81.93。
	// 期待値は 523.6 - 81.9 = 441.7 で sequential と一致。
	// multi-tools は 523.6 - 93.3 = 430.3、つまり Σ V(cyl_i ∩ sphere) を引いていて、
	// 重なり領域(bicyl/tricyl)を2重・3重カウントしている。
	// → tools 同士が交差する場合、multi-tools subtract は体積が物理的に意味を失う。
	// → tools 内で交差が無い保証がなければ pairwise/逐次 subtract を使うべき。
}

#[test]
fn test_intersect_sphere_with_multiple_cylinders() {
	// 球 ∩ [複数方向の円柱]
	// 質問: OCCT の multi-tools Common は
	//   (a) obj ∩ (tool1 ∪ tool2 ∪ ...)  → ウニの胴体(3本のシリンダ和と球の積)
	//   (b) obj ∩ tool1 ∩ tool2 ∩ ...   → 中心の3本全部が重なる領域のみ
	let sphere = Solid::sphere(5.0);
	let r = 0.8;
	let len = 20.0;
	let half = len / 2.0;
	let cyl_x = Solid::cylinder(r, DVec3::X, len).translate(DVec3::new(-half, 0.0, 0.0));
	let cyl_y = Solid::cylinder(r, DVec3::Y, len).translate(DVec3::new(0.0, -half, 0.0));
	let cyl_z = Solid::cylinder(r, DVec3::Z, len).translate(DVec3::new(0.0, 0.0, -half));

	let multi = Solid::boolean_intersect([&sphere], [&cyl_x, &cyl_y, &cyl_z]).unwrap();
	let multi_vol: f64 = multi.iter().map(|s| s.volume()).sum();
	println!("intersect sphere ∩ [cyl_x, cyl_y, cyl_z]: count={}, volume={:.4}", multi.len(), multi_vol);
	write_outputs(&multi, "intersect_sphere_multi");

	let v_x: f64 = Solid::boolean_intersect([&sphere], [&cyl_x]).unwrap().iter().map(|s| s.volume()).sum();
	let v_y: f64 = Solid::boolean_intersect([&sphere], [&cyl_y]).unwrap().iter().map(|s| s.volume()).sum();
	let v_z: f64 = Solid::boolean_intersect([&sphere], [&cyl_z]).unwrap().iter().map(|s| s.volume()).sum();

	// 逐次 intersect: obj ∩ X ∩ Y ∩ Z
	let mut current = vec![sphere.clone()];
	for tool in [&cyl_x, &cyl_y, &cyl_z] {
		current = current.iter().flat_map(|o| Solid::boolean_intersect([o], [tool]).unwrap()).collect();
	}
	let seq_vol: f64 = current.iter().map(|s| s.volume()).sum();

	println!("  sphere ∩ cyl_x = {:.4}", v_x);
	println!("  sphere ∩ cyl_y = {:.4}", v_y);
	println!("  sphere ∩ cyl_z = {:.4}", v_z);
	println!("  sphere ∩ X ∩ Y ∩ Z (sequential) = {:.4} (count={})", seq_vol, current.len());
	write_outputs(&current, "intersect_sphere_sequential");

	// === 結論 ===
	// multi-tools: count=3, volume=59.93 (≒ V(sphere∩cyl_x) × 3)
	// sequential : count=1, volume=2.40 (= sphere ∩ cyl_x ∩ cyl_y ∩ cyl_z)
	// multi-tools intersect は (a)「∩(∪tools)」でも (b)「∩ tool1 ∩ tool2 ∩ ...」でもなく、
	// 「各 tool に対する sphere∩tool_i を別々の Solid として返す」挙動。
	// = OCCT General BOP の「Object×Tool の各ペアごとに分割ピースを出力」設計の表れ。
	// → ユーザーが集合論的に期待する intersect の意味論を持たない。
	// → 単一 tool での sphere.intersect(&cyl) を逐次 fold するのが正しい。
}

#[test]
fn test_operator_overloads() {
	// `+` / `-` / `*` for &Solid → Result<Solid, Error>
	// 結果が単一 Solid のケース、Error::OneFailed のケースを両方検証する。
	let a = Solid::cube(10.0, 10.0, 10.0);
	let b = Solid::cube(10.0, 10.0, 10.0).translate(DVec3::new(5.0, 5.0, 5.0));

	let u = (&a + &b).expect("a + b should yield exactly one solid");
	println!("a + b (union):     volume = {:.4}", u.volume());

	let s = (&a - &b).expect("a - b should yield exactly one solid");
	println!("a - b (subtract):  volume = {:.4}", s.volume());

	let i = (&a * &b).expect("a * b should yield exactly one solid");
	println!("a * b (intersect): volume = {:.4}", i.volume());

	// 非交差での intersect → Ok(Vec::new()) → Err(OneFailed(0))
	let far = Solid::cube(1.0, 1.0, 1.0).translate(DVec3::new(100.0, 0.0, 0.0));
	match &a * &far {
		Err(e @ cadrum::Error::OneFailed(0)) => println!("a * far (disjoint) -> {:?}", e),
		Err(e) => panic!("expected OneFailed(0), got {:?}", e),
		Ok(_) => panic!("expected OneFailed(0), got Ok"),
	}

	// iter.sum() / iter.product() で union / intersect を畳む
	let c = Solid::cube(10.0, 10.0, 10.0).translate(DVec3::new(2.0, 2.0, 2.0));
	let solids = vec![a.clone(), b.clone(), c.clone()];

	let unioned: Solid = solids.iter().sum::<Result<Solid, _>>().expect("sum union should succeed");
	println!("sum (union of 3):       volume = {:.4}", unioned.volume());

	let intersected: Solid = solids.iter().product::<Result<Solid, _>>().expect("product intersect should succeed");
	println!("product (intersect 3):  volume = {:.4}", intersected.volume());

	// 空イテレータ → Err(OneFailed(0))
	let empty: Vec<Solid> = Vec::new();
	match empty.iter().sum::<Result<Solid, _>>() {
		Err(cadrum::Error::OneFailed(0)) => println!("sum of empty -> OneFailed(0) ✓"),
		other => panic!("expected OneFailed(0), got {:?}", other.is_ok()),
	}
}

#[test]
fn test_sum_olympic_rings_out_of_order() {
	// 5 つのソリッドが「隣り合うもの同士のみ重なる」鎖状配置: 1-2-3-4-5
	// 順 1,3,5,2,4 で sum() すると、1+3 の時点では disjoint (2個) になるが
	// 最後の 4 を足したときに全体が連結して 1 個になる。
	// try_fold + (&acc + s) だと中間 OneFailed(2) で失敗するが、
	// Vec<Solid> を中間に保つ実装なら正しく Ok(Solid) で返る。
	let s = 1.0; // cube サイズ
	let step = 0.8; // ステップ (重なるよう s より小さく)
	let ring1 = Solid::cube(s, s, s).translate(DVec3::new(0.0 * step, 0.0, 0.0));
	let ring2 = Solid::cube(s, s, s).translate(DVec3::new(1.0 * step, 0.0, 0.0));
	let ring3 = Solid::cube(s, s, s).translate(DVec3::new(2.0 * step, 0.0, 0.0));
	let ring4 = Solid::cube(s, s, s).translate(DVec3::new(3.0 * step, 0.0, 0.0));
	let ring5 = Solid::cube(s, s, s).translate(DVec3::new(4.0 * step, 0.0, 0.0));

	// out-of-order: 1, 3, 5, 2, 4
	let out_of_order = [&ring1, &ring3, &ring5, &ring2, &ring4];
	let unioned: Solid = out_of_order.iter().copied().sum::<Result<Solid, _>>()
		.expect("out-of-order union should succeed at the end");
	println!("out-of-order sum: volume = {:.4}", unioned.volume());

	// 順番通りでも当然成功
	let in_order = [&ring1, &ring2, &ring3, &ring4, &ring5];
	let unioned_ordered: Solid = in_order.iter().copied().sum::<Result<Solid, _>>().unwrap();

	// 両方とも同じ最終ボリュームになるはず
	assert!((unioned.volume() - unioned_ordered.volume()).abs() < 1e-6,
		"order-independent: {} vs {}", unioned.volume(), unioned_ordered.volume());
}

/// solid を out/ 以下に SVG, STL, STEP で書き出す。
fn write_outputs(solids: &[Solid], name: &str) {
	std::fs::create_dir_all("out").unwrap();
	let mut f = std::fs::File::create(format!("out/{name}.step")).unwrap();
	cadrum::Solid::write_step(solids, &mut f).expect("step write");
	let mut f = std::fs::File::create(format!("out/{name}.stl")).unwrap();
	cadrum::Solid::mesh(solids, 0.1).and_then(|m| m.write_stl(&mut f)).expect("stl write");
	let mut f = std::fs::File::create(format!("out/{name}.svg")).unwrap();
	cadrum::Solid::mesh(solids, 0.5).and_then(|m| m.scene(DVec3::new(1.0, 1.0, 2.0), DVec3::Z, true, false).write_svg(&mut f)).expect("svg write");
}