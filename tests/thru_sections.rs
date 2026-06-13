//! Integration tests for `Solid::thru_sections`.
//!
//! Covers:
//! - NACA 風閉断面 (bspline) の補間 loft → 体積妥当性 (テーパー prism 推定との比較)
//! - 断面データ点を**正確に通る**こと (中間断面の点を内外 ±ε で挟み込み検証)
//! - ruled=true (直線パネル) の体積が区分 frustum 推定と一致すること
//! - `Solid::loft` == `thru_sections(.., false)` の等価性
//!
//! 体積の数値比較には `Solid::volume()` (BRepGProp) ではなく fine mesh からの
//! 発散定理計算を使う。BRepGProp::VolumeProperties は B-spline 境界 face の
//! Gauss 積分次数が頭打ちになり、この種の断面で ~13% 過小評価するため
//! (mesh 体積・折れ線 shoelace 面積とは互いに <0.1% で一致する)。

use cadrum::{BSplineEnd, Edge, Solid, Tessellation};
use glam::DVec3;
use std::f64::consts::PI;

/// NACA0012 風の閉じた翼型断面点列 (chord `c`、断面は XY 平面、スパン方向 z)。
/// cosine spacing で TE → 上面 → LE → 下面 → TE と一周し、TE 点を末尾で
/// 重複させる: NotAKnot 開曲線として補間すると「TE に C⁰ コーナーを持つ
/// 閉曲線」になる (鋭い TE を periodic C² で補間するとコーナーで
/// オーバーシュートして自己交差するため、periodic は使わない)。
fn naca_points(c: f64, z: f64, n: usize) -> Vec<DVec3> {
	let half_thickness = |x: f64| -> f64 { 5.0 * 0.12 * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x.powi(3) - 0.1036 * x.powi(4)) };
	let mut pts = Vec::new();
	for i in 0..=n {
		let x = (1.0 + (PI * i as f64 / n as f64).cos()) / 2.0;
		pts.push(DVec3::new(c * x, c * half_thickness(x), z));
	}
	for i in 1..=n {
		let x = (1.0 - (PI * i as f64 / n as f64).cos()) / 2.0;
		pts.push(DVec3::new(c * x, -c * half_thickness(x), z));
	}
	pts
}

/// 閉多角形 (XY 平面) の shoelace 面積。
fn polygon_area(pts: &[DVec3]) -> f64 {
	let n = pts.len();
	let mut a = 0.0;
	for i in 0..n {
		let p = pts[i];
		let q = pts[(i + 1) % n];
		a += p.x * q.y - q.x * p.y;
	}
	0.5 * a.abs()
}

fn naca_section(c: f64, z: f64, n: usize) -> Vec<Edge> {
	vec![Edge::bspline(&naca_points(c, z, n), BSplineEnd::NotAKnot).expect("NACA bspline section")]
}

/// fine mesh からの発散定理体積 (モジュールコメント参照)。
fn mesh_volume(solid: &Solid) -> f64 {
	let mesh = Solid::mesh([solid], Tessellation { deflection_linear: 1.0e-4, relative_linear: false, ..Default::default() }).expect("mesh");
	let mut vol = 0.0;
	for t in mesh.indices.chunks_exact(3) {
		let (a, b, c) = (mesh.vertices[t[0]], mesh.vertices[t[1]], mesh.vertices[t[2]]);
		vol += a.dot(b.cross(c)) / 6.0;
	}
	vol.abs()
}

// ==================== (1) 2 断面 NACA 翼: 体積 vs テーパー prism 推定 ====================

#[test]
fn test_thru_sections_01_two_naca_sections_volume_sane() {
	let n = 60;
	let (c_root, c_tip, span) = (1.0, 0.5, 4.0);
	let root = naca_section(c_root, 0.0, n);
	let tip = naca_section(c_tip, span, n);

	let wing = Solid::thru_sections(&[root, tip], false).expect("two-section thru_sections should succeed");
	assert!(wing.volume() > 0.0, "wing must enclose positive volume");

	// 線形テーパー翼: A(z) ∝ c(z)² → V = span·A_root·(1+s+s²)/3, s = c_tip/c_root
	let a_root = polygon_area(&naca_points(c_root, 0.0, n));
	let s = c_tip / c_root;
	let expected = span * a_root * (1.0 + s + s * s) / 3.0;
	let actual = mesh_volume(&wing);
	let rel_err = (actual - expected).abs() / expected;
	assert!(rel_err < 0.01, "wing volume {:.6} vs tapered-prism estimate {:.6} (relative error {:.4})", actual, expected, rel_err);

	// 断面中央付近の点は solid 内部
	assert!(wing.contains(DVec3::new(0.4 * c_root, 0.0, 0.2)));
	println!("two-section NACA wing: mesh volume = {:.6} (estimate {:.6}, GProp volume {:.6})", actual, expected, wing.volume());
}

// ==================== (2) 補間の正確性: 中間断面の点が表面上に乗る ====================

#[test]
fn test_thru_sections_02_sections_interpolated_exactly() {
	let n = 40;
	let (c_mid, z_mid) = (0.7, 2.0);
	let sections = [naca_section(1.0, 0.0, n), naca_section(c_mid, z_mid, n), naca_section(0.5, 4.0, n)];
	let wing = Solid::thru_sections(&sections, false).expect("three-section thru_sections should succeed");

	// 中間断面のデータ点 (bspline が正確に通る点) は loft 表面上にも乗る:
	// 各点を厚み方向に ±ε ずらすと内/外で contains が反転する
	// (ε に対して表面が点を通っていなければどちらかが破れる)。
	let eps = 1.0e-3;
	for p in naca_points(c_mid, z_mid, n).iter().filter(|p| p.y.abs() > 8.0e-3) {
		let inward = DVec3::new(p.x, p.y - eps * p.y.signum(), p.z);
		let outward = DVec3::new(p.x, p.y + eps * p.y.signum(), p.z);
		assert!(wing.contains(inward), "point {:.4?} - ε must be inside (surface missed the section point)", p);
		assert!(!wing.contains(outward), "point {:.4?} + ε must be outside (surface missed the section point)", p);
	}
}

// ==================== (3) ruled=true: 区分 frustum 推定との一致 ====================

#[test]
fn test_thru_sections_03_ruled_matches_piecewise_estimate() {
	let n = 60;
	let stations = [(1.0, 0.0), (0.6, 2.0), (0.5, 4.0)];
	let sections: Vec<Vec<Edge>> = stations.iter().map(|&(c, z)| naca_section(c, z, n)).collect();

	let ruled = Solid::thru_sections(&sections, true).expect("ruled thru_sections should succeed");

	// 区間ごとの線形補間断面: V = Σ h/3·(A1 + A2 + √(A1·A2))
	let mut expected = 0.0;
	for w in stations.windows(2) {
		let (a1, a2) = (polygon_area(&naca_points(w[0].0, w[0].1, n)), polygon_area(&naca_points(w[1].0, w[1].1, n)));
		expected += (w[1].1 - w[0].1) / 3.0 * (a1 + a2 + (a1 * a2).sqrt());
	}
	let actual = mesh_volume(&ruled);
	let rel_err = (actual - expected).abs() / expected;
	assert!(rel_err < 0.01, "ruled volume {:.6} vs piecewise frustum estimate {:.6} (relative error {:.4})", actual, expected, rel_err);

	// smooth 版も成功し、ruled と同程度の体積になる (補間の膨らみ分だけ差は出る)
	let smooth = Solid::thru_sections(&sections, false).expect("smooth thru_sections should succeed");
	let ratio = mesh_volume(&smooth) / actual;
	assert!((0.85..1.15).contains(&ratio), "smooth/ruled volume ratio {:.4} out of sanity band", ratio);
	println!("ruled = {:.6}, smooth = {:.6}, estimate = {:.6}", actual, mesh_volume(&smooth), expected);
}

// ==================== (4) loft == thru_sections(.., false) ====================

#[test]
fn test_thru_sections_04_loft_is_thru_sections_unruled() {
	let n = 40;
	let make_sections = || [naca_section(1.0, 0.0, n), naca_section(0.5, 4.0, n)];

	let via_loft = Solid::loft(&make_sections()).expect("loft should succeed");
	let via_thru = Solid::thru_sections(&make_sections(), false).expect("thru_sections should succeed");

	let rel = (via_loft.volume() - via_thru.volume()).abs() / via_thru.volume();
	assert!(rel < 1.0e-9, "loft and thru_sections(.., false) must build the same solid (volume rel diff {:.3e})", rel);
}
