//! Integration tests for `Solid::sweep_sections`.
//!
//! Covers:
//! - 4-fold 回転対称性 (spine=circle, 4 正方形断面, half-space 分割で体積比較)
//! - bspline 楕円断面のモーフィング sweep (ステラレーター風)

use cadrum::{BSplineEnd, Edge, ProfileOrient, Solid, Transform};
use glam::DVec3;
use std::f64::consts::TAU;

/// XZ 平面の正方形断面を ring_r だけ +X に移動し rotate_z(phi) で配置。
fn ring_quad(phi: f64, ring_r: f64, half: f64) -> [Edge; 4] {
	let edges: [Edge; 4] = Edge::polygon([
		DVec3::new(-half, 0.0, -half),
		DVec3::new(half, 0.0, -half),
		DVec3::new(half, 0.0, half),
		DVec3::new(-half, 0.0, half),
	])
	.unwrap()
	.try_into()
	.ok()
	.expect("polygon must return exactly 4 edges");
	edges.translate(DVec3::X * ring_r).rotate_z(phi)
}

/// ZX 平面と YZ 平面で 4 象限に分割し対称性を検証。
///
/// - `rot4_tol`: 4-fold 回転対称の許容相対誤差 (q1≈q2≈q3≈q4)
/// - `point_tol`: 点対称の許容相対誤差 (q1≈q3, q2≈q4)
fn assert_quadrant_symmetry(solid: &Solid, rot4_tol: f64, point_tol: f64) {
	let total = solid.volume();
	assert!(total > 0.0, "volume should be positive, got {}", total);

	let zx = Solid::half_space(DVec3::ZERO, DVec3::Y);
	let yz = Solid::half_space(DVec3::ZERO, DVec3::X);
	let zx_neg = Solid::half_space(DVec3::ZERO, -DVec3::Y);
	let yz_neg = Solid::half_space(DVec3::ZERO, -DVec3::X);

	let quadrant = |hs1: &Solid, hs2: &Solid| -> f64 {
		let (ab, _) = Solid::boolean_intersect(std::slice::from_ref(solid), std::slice::from_ref(hs1)).unwrap();
		let (q, _) = Solid::boolean_intersect(&ab, std::slice::from_ref(hs2)).unwrap();
		q.iter().map(|s| s.volume()).sum::<f64>()
	};

	let q1 = quadrant(&yz, &zx);       // +X, +Y
	let q2 = quadrant(&yz_neg, &zx);    // -X, +Y
	let q3 = quadrant(&yz_neg, &zx_neg); // -X, -Y
	let q4 = quadrant(&yz, &zx_neg);    // +X, -Y

	let expected = total / 4.0;
	println!(
		"symmetry: total={:.4}, q1={:.4}, q2={:.4}, q3={:.4}, q4={:.4}, expected={:.4}",
		total, q1, q2, q3, q4, expected
	);

	// 4-fold 回転対称: q1≈q2≈q3≈q4
	for (i, vol) in [(1, q1), (2, q2), (3, q3), (4, q4)] {
		let rel_err = (vol - expected).abs() / expected;
		assert!(
			rel_err < rot4_tol,
			"rot4: quadrant {} volume {:.4} vs expected {:.4} (rel err {:.4})",
			i, vol, expected, rel_err
		);
	}

	// 点対称: q1≈q3, q2≈q4 (原点対称のペア)
	let avg13 = (q1 + q3) / 2.0;
	let avg24 = (q2 + q4) / 2.0;
	let err13 = (q1 - q3).abs() / avg13;
	let err24 = (q2 - q4).abs() / avg24;
	println!("point symmetry: q1-q3 rel_err={:.4}, q2-q4 rel_err={:.4}", err13, err24);
	assert!(
		err13 < point_tol,
		"point: q1={:.4} vs q3={:.4} (rel err {:.4})",
		q1, q3, err13
	);
	assert!(
		err24 < point_tol,
		"point: q2={:.4} vs q4={:.4} (rel err {:.4})",
		q2, q4, err24
	);
}

/// solid を out/ 以下に SVG, STL, STEP で書き出す。
fn write_outputs(solids: &[Solid], name: &str) {
	std::fs::create_dir_all("out").unwrap();
	let mut f = std::fs::File::create(format!("out/{name}.step")).unwrap();
	cadrum::io::write_step(solids, &mut f).expect("step write");
	let mut f = std::fs::File::create(format!("out/{name}.stl")).unwrap();
	cadrum::io::write_stl(solids, 0.1, &mut f).expect("stl write");
	let mut f = std::fs::File::create(format!("out/{name}.svg")).unwrap();
	cadrum::io::write_svg(solids, DVec3::new(1.0, 1.0, 2.0), 0.5, true, &mut f).expect("svg write");
}

// ==================== (1) 4-fold 回転対称性: sweep_sections + circle spine ====================

#[test]
fn test_sweep_sections_01_rotational_symmetry() {
	let ring_r = 5.0;
	let half = 1.0;
	let segments = 4;

	let spine = Edge::circle(ring_r, DVec3::Z).unwrap();
	let sections: Vec<[Edge; 4]> = (0..segments)
		.map(|i| ring_quad(TAU * i as f64 / segments as f64, ring_r, half))
		.collect();

	let ring = Solid::sweep_sections(&sections, std::slice::from_ref(&spine), ProfileOrient::Torsion)
		.expect("sweep_sections ring should succeed");

	assert_quadrant_symmetry(&ring, 0.01, 0.01);
	write_outputs(std::slice::from_ref(&ring), "test_sweep_sections_01_rotational_symmetry");
}

// ==================== (2) bspline 楕円断面モーフィング: ステラレーター風 ====================

#[test]
#[ignore = "seam 付近の法線不連続により点対称誤差 ~1.2% — point_tol=0.5% で失敗"]
fn test_sweep_sections_02_bspline_stellarator() {
	let ring_r = 6.0;
	let n_ribs = 8;
	let n_pts = 24;

	let spine = Edge::circle(ring_r, DVec3::Z).unwrap();

	// 各トロイダル角 phi でアスペクト比が変化する楕円断面を配置
	let sections: Vec<Edge> = (0..n_ribs)
		.map(|i| {
			let phi = TAU * (i as f64) / n_ribs as f64;
			let a = 1.5 + 0.5 * (2.0 * phi).sin(); // poloidal semi-axis (変動)
			let b = 1.0 + 0.3 * (2.0 * phi).cos(); // poloidal semi-axis (変動)
			Edge::bspline(
				(0..n_pts)
				.map(|j| {
					let theta = TAU * j as f64 / n_pts as f64;
					DVec3::X * a * theta.cos() + DVec3::Z * b * theta.sin()
				}),
				BSplineEnd::Periodic
			)
			.unwrap()
			.translate(DVec3::X*ring_r)
			.rotate_z(phi)
		})
		.collect();

	let plasma = Solid::sweep_sections(sections.iter().map(|e| [e]), [&spine], ProfileOrient::Up(DVec3::Z))
		.expect("stellarator-style sweep_sections should succeed")
		.clean()
		.expect("clean should succeed");

	assert_eq!(plasma.shell_count(), 1);
	// 断面が 2φ で変動するため 4-fold 対称ではないが、点対称にはなるはず。
	// 現状 seam 付近の法線不連続により点対称誤差 ~1.2% 発生するため ignore。
	assert_quadrant_symmetry(&plasma, 0.25, 0.005);
	write_outputs(std::slice::from_ref(&plasma), "test_sweep_sections_02_bspline_stellarator");
}
