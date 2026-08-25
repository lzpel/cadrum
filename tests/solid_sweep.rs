//! Integration tests for `Solid::sweep`.
//!
//! Covers:
//! - 閉じた periodic spine + periodic auxiliary guide の sweep が通る
//! - 捻れた輪の体積が Pappus の定理と一致する

use cadrum::{BSplineEnd, DQuat, DVec3, Edge, Error, ProfileOrient, Solid};

/// spine/aux の補間点数。
const CURVE_STEPS: usize = 24;
/// spine の半径と profile の一辺。
const RADIUS: f64 = 10.;
const SIDE: f64 = 2.;

/// 半径 RADIUS の円上の点と、そこから接線まわりに theta 捻った guide 点。
fn wire_and_guide(phi: f64) -> [DVec3; 2] {
	let theta = (phi * 2.).sin();
	let rot_z = DQuat::from_rotation_z(phi);
	let rot_y = DQuat::from_rotation_y(theta);
	let wire_point = DVec3::X * RADIUS;
	let guide_offset = DVec3::X;
	[rot_z * wire_point, rot_z * (wire_point + rot_y * guide_offset)]
}

/// [0, 2π) を n 等分した点列。periodic B-spline へ渡すので始点は繰り返さない。
fn sample(idx: usize, n: usize) -> Vec<DVec3> {
	(0..n).map(|i| wire_and_guide(i as f64 / n as f64 * std::f64::consts::TAU)[idx]).collect()
}

/// 一辺 SIDE の正方形を spine 始点の接線に直交させ、始点へ移動。
fn square_profile(spine: &Edge) -> Result<Vec<Edge>, Error> {
	let h = SIDE / 2.;
	let square = Edge::polygon(&[DVec3::new(-h, -h, 0.), DVec3::new(h, -h, 0.), DVec3::new(h, h, 0.), DVec3::new(-h, h, 0.)])?;
	Ok(square.into_iter().map(|e| e.align_z(spine.start_tangent(), spine.start_point()).translate(spine.start_point())).collect())
}

/// spine も aux も periodic B-spline 1 本のまま `ProfileOrient::Auxiliary` で sweep。
/// 素の OCCT 8.0.1 ではこの組み合わせは SweepFailed になるか破綻形状を返すので、
/// patches/pr1.patch (fix D) と patches/pr2.patch (fix A) が効いていることの回帰テスト。
fn closed_auxiliary_sweep() -> Result<Solid, Error> {
	let spine = Edge::bspline(&sample(0, CURVE_STEPS), BSplineEnd::Periodic)?;
	let aux = Edge::bspline(&sample(1, CURVE_STEPS), BSplineEnd::Periodic)?;
	let profile = square_profile(&spine)?;
	Solid::sweep(&profile, &[spine], ProfileOrient::Auxiliary(&[aux]))
}

// ==================== (1) 閉じた spine + guide の sweep が通る ====================

#[test]
fn test_sweep_01_closed_periodic_auxiliary_succeeds() {
	let solid = closed_auxiliary_sweep().expect("closed periodic spine with a periodic guide must sweep");

	assert_eq!(solid.iter_face().count(), 4, "a square profile swept along a closed spine leaves 4 side faces and no cap");

	let mesh = Solid::mesh(std::iter::once(&solid), Default::default()).expect("mesh should succeed");
	assert!(!mesh.vertices.is_empty(), "swept solid must tessellate");
}

// ==================== (2) 体積が Pappus の定理と一致 ====================

#[test]
fn test_sweep_02_closed_auxiliary_volume_matches_pappus() {
	let solid = closed_auxiliary_sweep().expect("closed periodic spine with a periodic guide must sweep");

	// 断面積 SIDE² の輪。捻っても重心は半径 RADIUS の円上なので体積は面積×周長。
	let expected = SIDE * SIDE * std::f64::consts::TAU * RADIUS;
	let rel = (solid.volume() - expected).abs() / expected;
	assert!(rel < 1.0e-3, "volume {:.3} vs Pappus {:.3} (relative error {:.3e})", solid.volume(), expected, rel);
}
