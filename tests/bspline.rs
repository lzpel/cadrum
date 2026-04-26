//! Integration tests for `Solid::bspline`.
//!
//! 2 field-period ステラレーター風トーラスを作って XZ/YZ 平面で 4 象限
//! に切り、180° 回転対称(s1 ≈ s3, s2 ≈ s4)を体積で検証する。
//! 周期方向の制御点変動が `sin(2φ)`/`cos(2φ)` で構成されているため
//! `phi → phi + π` のシフトが離散グリッドを完全に保存する → 近似誤差を
//! 導入しないので、対称性は boolean op の数値ノイズ分しか揺れない想定。

use cadrum::Solid;
use glam::{DQuat, DVec3};
use std::f64::consts::TAU;

/// solid を out/ 以下に SVG, STL, STEP で書き出す。
fn write_outputs(solids: &[Solid], name: &str) {
	std::fs::create_dir_all("out").unwrap();
	let mut f = std::fs::File::create(format!("out/{name}.step")).unwrap();
	cadrum::write_step(solids, &mut f).expect("step write");
	let mut f = std::fs::File::create(format!("out/{name}.stl")).unwrap();
	cadrum::mesh(solids, 0.1).and_then(|m| m.write_stl(&mut f)).expect("stl write");
	let mut f = std::fs::File::create(format!("out/{name}.svg")).unwrap();
	cadrum::mesh(solids, 0.5).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), DVec3::Z, true, false, &mut f)).expect("svg write");
}

/// XZ 平面(法線 Y)と YZ 平面(法線 X)で 4 象限に分割し、180° 回転対称
/// (s1 ≈ s3, s2 ≈ s4)を体積で検証する。`tol` は相対誤差閾値。
fn assert_quadrant_point_symmetry(solid: &Solid, tol: f64) {
	let total = solid.volume();
	assert!(total > 0.0, "volume should be positive, got {}", total);

	// 各 half_space は法線の向きに solid が満ちる。
	let plus_x = Solid::half_space(DVec3::ZERO, DVec3::X);
	let minus_x = Solid::half_space(DVec3::ZERO, -DVec3::X);
	let plus_y = Solid::half_space(DVec3::ZERO, DVec3::Y);
	let minus_y = Solid::half_space(DVec3::ZERO, -DVec3::Y);

	let quadrant = |hs1: &Solid, hs2: &Solid| -> f64 {
		let ab = Solid::boolean_intersect(std::slice::from_ref(solid), std::slice::from_ref(hs1)).expect("intersect hs1");
		let q = Solid::boolean_intersect(&ab, std::slice::from_ref(hs2)).expect("intersect hs2");
		q.iter().map(|s| s.volume()).sum::<f64>()
	};

	let s1 = quadrant(&plus_x, &plus_y); // +X, +Y
	let s2 = quadrant(&minus_x, &plus_y); // -X, +Y
	let s3 = quadrant(&minus_x, &minus_y); // -X, -Y
	let s4 = quadrant(&plus_x, &minus_y); // +X, -Y

	let sum = s1 + s2 + s3 + s4;
	println!("total={:.6}, s1={:.6}, s2={:.6}, s3={:.6}, s4={:.6}, sum={:.6}", total, s1, s2, s3, s4, sum);

	// 180° 点対称: s1 ≈ s3, s2 ≈ s4
	let avg13 = (s1 + s3) / 2.0;
	let avg24 = (s2 + s4) / 2.0;
	let err13 = (s1 - s3).abs() / avg13;
	let err24 = (s2 - s4).abs() / avg24;
	println!("point symmetry: s1-s3 rel_err={:.6}, s2-s4 rel_err={:.6}", err13, err24);

	assert!(err13 < tol, "s1={:.4} vs s3={:.4} (rel err {:.4} >= {:.4})", s1, s3, err13, tol);
	assert!(err24 < tol, "s2={:.4} vs s4={:.4} (rel err {:.4} >= {:.4})", s2, s4, err24, tol);
}

// ==================== (1) 2-period stellarator-like torus ====================

#[test]
fn test_bspline_01_two_period_torus_point_symmetry() {
	const M: usize = 48; // toroidal (U) — 180° 対称のため偶数
	const N: usize = 24; // poloidal (V) — 任意
	const RING_R: f64 = 6.0;

	// 2 field-period ステラレーター風トーラス。以下すべて phi → phi+π で
	// 不変(または 2π の倍数分だけずれる)ため 180° 回転対称を保つ:
	//   a(phi)      = 1.8 + 0.6 * sin(2φ)       radial 半軸
	//   b(phi)      = 1.0 + 0.4 * cos(2φ)       Z 半軸
	//   psi(phi)    = 2 * phi                   cross-section ひねり(1周で2回転)
	//   z_shift(phi)= 1.0 * sin(2φ)             上下方向のうねり
	// psi(phi+π) = 2phi+2π ≡ 2phi (mod 2π) → 楕円の向きは同じ
	// z_shift(phi+π) = sin(2phi+2π) = sin(2phi) → 同じ高さ
	// a/b も同様に同じ値 → 形状は phi+π でも同一 → Z 軸まわり 180° 対称。
	let point = |i: usize, j: usize| -> DVec3 {
		let phi = TAU * (i as f64) / (M as f64);
		let theta = TAU * (j as f64) / (N as f64);
		let two_phi = 2.0 * phi;
		let a = 1.8 + 0.6 * two_phi.sin();
		let b = 1.0 + 0.4 * two_phi.cos();
		let psi = two_phi; // ひねり 2 回転 per loop
		let z_shift = 1.0 * two_phi.sin();
		// 1. 局所断面(まだひねる前、(X,Z) 平面の楕円)
		let local_raw = DVec3::X * (a * theta.cos()) + DVec3::Z * (b * theta.sin());
		// 2. 局所 Y 軸(大径接線方向)まわりに psi 回転 — これが断面のひねり
		let local_twisted = DQuat::from_axis_angle(DVec3::Y, psi) * local_raw;
		// 3. 局所フレームで上下に揺らす
		let local_shifted = local_twisted + DVec3::Z * z_shift;
		// 4. 大径方向に RING_R だけ外へ
		let translated = local_shifted + DVec3::X * RING_R;
		// 5. 全体として Z 軸まわりに phi 回転
		DQuat::from_axis_angle(DVec3::Z, phi) * translated
	};

	let plasma = Solid::bspline(M, N, true, &point).expect("2-period bspline torus should succeed");
	assert!(plasma.volume() > 0.0);

	assert_quadrant_point_symmetry(&plasma, 0.01);

	write_outputs(&[plasma, Solid::bspline(M, N, false, &point).unwrap().translate(DVec3::Z * -10.0)], "test_bspline_01_two_period_torus");
}


// ==================== (2) #120 reproducer: VMEC-like LCFS, U=0 seam dent ====================

/// #120: `Solid::bspline(grid, periodic=true)` produces only C⁰-continuous
/// surfaces at the U=0 seam when the input has non-trivial high-Fourier
/// content. Visible as mm-scale dents in the tessellation.
///
/// Writes `out/test_bspline_02_seam_dent_120.stl` for visual inspection in
/// MeshLab / Blender. No assertions — this is an investigation aid.
#[test]
fn test_bspline_02_seam_dent_120() {
	const M: usize = 48;
	const N: usize = 24;
	const PHI_OFFSET: f64 = std::f64::consts::FRAC_PI_4;

	// (m, n, amplitude) — VMEC LCFS top modes + amplified high-frequency
	// content to make the seam dent visible.
	const RMNC: &[(f64, f64, f64)] = &[
		(0.0, 0.0, 11.06), (1.0, 0.0, 1.89), (0.0, 4.0, 1.53),
		(1.0, -4.0, -1.39), (1.0, 4.0, 0.58), (2.0, -4.0, 0.26),
		(3.0, -8.0, 0.12), (4.0, -8.0, 0.10), (4.0, -12.0, 0.08),
		(5.0, -12.0, 0.07), (6.0, -16.0, 0.06), (8.0, -24.0, 0.05),
		(10.0, -32.0, 0.04), (3.0, 8.0, 0.08), (6.0, 16.0, 0.06),
	];
	const ZMNS: &[(f64, f64, f64)] = &[
		(1.0, 0.0, 1.94), (0.0, 4.0, 1.24), (1.0, -4.0, 0.67),
		(1.0, 4.0, 0.53), (2.0, -4.0, 0.04),
		(3.0, -8.0, 0.10), (4.0, -8.0, 0.08), (4.0, -12.0, 0.07),
		(5.0, -12.0, 0.06), (6.0, -16.0, 0.06), (8.0, -24.0, 0.05),
		(10.0, -32.0, 0.04), (3.0, 8.0, 0.07), (6.0, 16.0, 0.05),
	];

	let point = |i: usize, j: usize| -> DVec3 {
		let phi = TAU * (i as f64) / (M as f64) + PHI_OFFSET;
		let theta = TAU * (j as f64) / (N as f64);
		let r: f64 = RMNC.iter().map(|&(m, n, a)| a * (m * theta - n * phi).cos()).sum();
		let z: f64 = ZMNS.iter().map(|&(m, n, a)| a * (m * theta - n * phi).sin()).sum();
		let (sp, cp) = phi.sin_cos();
		DVec3::new(r * cp, r * sp, z)
	};

	let plasma = Solid::bspline(M, N, true, point).expect("bspline should succeed");

	write_outputs(&[plasma], "test_bspline_02_seam_dent_120");
}


// ==================== (3) #120 simple seam-dent reproducer ====================

/// #120 simpler reproducer: R=12 大半径、cross-section が phi 方向に
/// 「縦長 → 横長 → 縦長 → ...」を急激に繰り返す楕円。半長径 a, b が
/// 0.6-1.2 で逆相に振動 (周波数 = M/2 = Nyquist 近傍)、隣接 segment
/// 間で完全に向きが入れ替わる極端なパターン。
///
/// 高 Fourier モードを単独で持たせて seam dent を最大化する設計。
/// assertion 無し、`out/` に STEP/STL/SVG を吐くだけ。
#[test]
fn test_bspline_03_seam_dent_alternating_ellipse() {
	const M: usize = 48;
	const N: usize = 24;
	const R0: f64 = 6.0;
	const N_OSC: f64 = 15.0;
	const AMP: f64 = 0.3;  // 周期補間 fix 前は 0.17 以上で +X 側 boolean が退化していた; fix 後は 0.3 でも安定

	let point = |i: usize, j: usize| -> DVec3 {
		let phi = TAU * (i as f64) / (M as f64);
		let theta = TAU * (j as f64) / (N as f64);
		// 0.6-1.2 で逆相振動: cos(N_OSC·φ) の符号で a, b が入れ替わる
		let osc = (N_OSC * phi).cos();
		let a = 0.9 + AMP * osc;
		let b = 0.9 - AMP * osc;
		// 局所断面 (x: 大径方向, z: 上下) → トロイダル φ 回転
		let local = DVec3::new(a * theta.cos() + R0, 0.0, b * theta.sin());
		DQuat::from_axis_angle(DVec3::Z, phi) * local
	};

	let periodic = Solid::bspline(M, N, true, &point).expect("periodic bspline should succeed");
	let nonperiodic = Solid::bspline(M, N, false, &point).expect("non-periodic bspline should succeed");
	// periodic を上 (Z=0)、non-periodic を下 (Z=-5) に並べて保存。
	// 断面の z 範囲は ±1.2 なので 5 離せばクリアに分離する。
	write_outputs(
		&[periodic.clone(), nonperiodic.translate(DVec3::Z * -5.0)],
		"test_bspline_03_seam_dent_alternating_ellipse",
	);

	// 保存後に periodic 側で 4 象限の体積を比較。
	// 入力グリッドは φ → -φ 対称 (cos のみ + sin·θ で z はゼロクロス) なので
	// 数学上は 180° 回転対称が成立するはずだが、seam dent が +X (φ=0) 周辺
	// にのみ出るため s1 (+X+Y) ≠ s3 (-X-Y) が ~0.6% で検出される。
	// 閾値 0.005 (0.5%) で seam dent を確実に拾う。
	assert_quadrant_point_symmetry(&periodic, 0.005);

	// #140 副タスク: u=0 (= φ=0) における surface normal の Y 成分を測定。
	// 入力 a(φ), b(φ) は cos の偶関数で a'(0) = b'(0) = 0 → ∂P/∂θ は XZ 平面内、
	// ∂P/∂φ は Y 軸方向 → 法線 = ∂P/∂θ × ∂P/∂φ ∈ XZ 平面 → N_y ≡ 0 が数学値。
	// 真の C^1 周期補間が達成できていれば |N_y|/|N| は数値ノイズレベル。残差が
	// 大きければ補間戦略を再検討する根拠 (#140)。
	//
	// 完全周期トーラスは 1 face しか持たないので iter_face().next() で取れる。
	let face = periodic.iter_face().next().expect("periodic torus has at least one face");
	const N_THETA: usize = 16;
	let mut max_y_ratio = 0.0_f64;
	for j in 0..N_THETA {
		let theta = TAU * (j as f64) / (N_THETA as f64);
		// φ=0 における解析的な surface 上の点 (a(0)=1.2, b(0)=0.6)
		let target = DVec3::new(R0 + 1.2 * theta.cos(), 0.0, 0.6 * theta.sin());
		let (_cp, normal) = face.project(target);
		if normal.length() == 0.0 {
			continue;  // BRepLProp が法線未定義 (degenerate point) → skip
		}
		let y_ratio = normal.y.abs() / normal.length();
		max_y_ratio = max_y_ratio.max(y_ratio);
	}
	println!("seam |N_y|/|N| max over {N_THETA} θ samples at u=0: {max_y_ratio:.6}");
}
