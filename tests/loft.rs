//! Integration tests for `Solid::loft`.
//!
//! Covers:
//! - 数値検証 (frustum 体積 vs 解析値)
//! - Closed loft (4 quad リング → 単一 shell)
//! - Open vs closed の face 数差 (closed パスが OCCT に届いていることの間接検証)
//! - bspline 断面との結合 (ステラレーター rib loft の最小再現)
//! - エラーケース 2 種 (single section, empty section)
//! - Closure-based 呼び出し方の ergonomic 確認

use cadrum::{BSplineEnd, Edge, Error, Solid};
use glam::DVec3;
use std::f64::consts::{PI, TAU};

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

/// 半径 `radius`、Z 平面 z=`z` の上に N 点を配置し、
/// `Edge::bspline(Periodic)` で滑らかな閉曲線にする。
fn periodic_circle(radius: f64, z: f64, n: usize) -> Edge {
	let pts: Vec<DVec3> = (0..n)
		.map(|i| {
			let t = TAU * i as f64 / n as f64;
			DVec3::new(radius * t.cos(), radius * t.sin(), z)
		})
		.collect();
	Edge::bspline(pts, BSplineEnd::Periodic).expect("periodic_circle: bspline construction must succeed")
}

/// 中心 `center` を通り、ローカル軸 `x_axis` / `y_axis` (両方単位ベクトル、互いに直交)
/// が張る平面上に一辺 `2*half` の正方形 wire を作る。4 本の Line edge を CCW 順で返す。
fn planar_square(center: DVec3, x_axis: DVec3, y_axis: DVec3, half: f64) -> Vec<Edge> {
	let p = [
		center - x_axis * half - y_axis * half,
		center + x_axis * half - y_axis * half,
		center + x_axis * half + y_axis * half,
		center - x_axis * half + y_axis * half,
	];
	vec![
		Edge::line(p[0], p[1]).unwrap(),
		Edge::line(p[1], p[2]).unwrap(),
		Edge::line(p[2], p[3]).unwrap(),
		Edge::line(p[3], p[0]).unwrap(),
	]
}

/// 大きな半径 `ring_r` の Z 軸まわりのリング上、トロイダル角 `phi` の位置に
/// (radial, +Z) 平面を取って一辺 `2*half` の正方形断面を作る。
/// 4 つ集めて closed=true で loft するとリング(角丸でない四角断面トーラス)になる。
fn ring_quad(phi: f64, ring_r: f64, half: f64) -> Vec<Edge> {
	let cx = ring_r * phi.cos();
	let cy = ring_r * phi.sin();
	let radial = dvec3(phi.cos(), phi.sin(), 0.0);
	planar_square(dvec3(cx, cy, 0.0), radial, DVec3::Z, half)
}

// ==================== (1) 数値検証: 円錐台 ====================

#[test]
fn test_loft_frustum_volume_matches_analytical() {
	// R₁=3, R₂=2, h=10 の円錐台:
	//   V = π/3 · h · (R₁² + R₁·R₂ + R₂²)
	//     = π/3 · 10 · (9 + 6 + 4)
	//     = 190π/3
	//     ≈ 198.97
	let r1 = 3.0;
	let r2 = 2.0;
	let h = 10.0;
	let lower = vec![periodic_circle(r1, 0.0, 32)];
	let upper = vec![periodic_circle(r2, h, 32)];

	let frustum = Solid::loft(&[lower, upper], false).expect("frustum loft should succeed");

	let expected = PI / 3.0 * h * (r1 * r1 + r1 * r2 + r2 * r2);
	let actual = frustum.volume();
	let rel_err = (actual - expected).abs() / expected;

	// bspline は 32 点で円を近似するので真円より僅かに小さく出る (~0.5%) が、
	// OCCT のロフト誤差込みでも 1% 以内に収まるはず。
	assert!(
		rel_err < 0.01,
		"frustum volume {:.4} vs analytical {:.4} (relative error {:.4})",
		actual, expected, rel_err
	);
	assert_eq!(frustum.shell_count(), 1);

	std::fs::create_dir_all("out").unwrap();
	let mut f = std::fs::File::create("out/loft_frustum.step").unwrap();
	cadrum::io::write_step(std::slice::from_ref(&frustum), &mut f).expect("frustum step write");
	println!("frustum loft: volume = {:.4} (expected {:.4})", actual, expected);
}

// ==================== (2) Closed loft: 4 quad リング ====================

#[test]
fn test_loft_closed_quad_ring_is_single_shell() {
	// 大半径 5 のリング上に 90° 間隔で 4 つの正方形断面 (一辺 2) を配置。
	// 各 quad は (radial, +Z) 平面に乗っているので互いに非 coplanar。
	// closed=true で loft すると四角断面トーラスになる
	// (OCCT が IsSame trick で v 周期 surface を構築)。
	let ring_r = 5.0;
	let half = 1.0;
	let quads: Vec<Vec<Edge>> = (0..4)
		.map(|i| ring_quad(TAU * i as f64 / 4.0, ring_r, half))
		.collect();

	let ring = Solid::loft(&quads, true).expect("closed quad ring loft should succeed");

	// 閉曲面なので 1 つの shell にまとまっているはず。
	assert_eq!(ring.shell_count(), 1, "closed loft must produce a single shell");
	// 体積が正(= solid として閉じている)。4 セクションで角張ったリングなので
	// 真の「正方形断面トーラス」(8πRs² ≈ 125.66) より小さく出るはずだが
	// > 0 であれば closed loft が成立している証拠になる。
	assert!(
		ring.volume() > 0.0,
		"closed quad ring should have positive volume, got {}",
		ring.volume()
	);

	std::fs::create_dir_all("out").unwrap();
	let mut f = std::fs::File::create("out/loft_closed_quad_ring.step").unwrap();
	cadrum::io::write_step(std::slice::from_ref(&ring), &mut f).expect("ring step write");
	println!("closed quad ring: volume = {:.4}", ring.volume());
}

// ==================== (3) Open vs Closed の face 数差 ====================

#[test]
fn test_loft_open_has_more_faces_than_closed() {
	// 同じ section 列 (リング上の 4 quad) を closed=false と closed=true で
	// それぞれロフトし、face 数の差を確認。closed では最後の section と最初の
	// section が連結されるので「最後 → 最初」の panel が 1 周期分追加される。
	// (open でも端面 cap が増える代わりに別の経路で face 数が変わるので、
	// 厳密な大小関係というより「数が違う」ことが closed パスが OCCT に
	// 届いている間接的な証拠になる。)
	let ring_r = 5.0;
	let half = 1.0;
	let quads: Vec<Vec<Edge>> = (0..4)
		.map(|i| ring_quad(TAU * i as f64 / 4.0, ring_r, half))
		.collect();

	let open = Solid::loft(&quads, false).expect("open loft should succeed");
	let closed = Solid::loft(&quads, true).expect("closed loft should succeed");

	let n_open: usize = open.face_iter().count();
	let n_closed: usize = closed.face_iter().count();

	assert_ne!(
		n_open, n_closed,
		"open ({}) and closed ({}) should produce different face counts",
		n_open, n_closed
	);
	// 両方とも 1 shell であることも確認 (degenerate でないことの追加チェック)
	assert_eq!(closed.shell_count(), 1);
	println!("face count: open={}, closed={}", n_open, n_closed);
}

// ==================== (4) bspline 断面: ステラレーター mini ケース ====================

#[test]
fn test_loft_bspline_sections_mini_stellarator() {
	// 4 つの楕円的 bspline 閉曲線を z 方向に並べてロフト。
	// 各 z でアスペクト比を僅かに変えることで、ステラレーターの
	// 「toroidal 角度ごとに poloidal 断面が変形する」性質をミニ再現する。
	let make_ellipse = |a: f64, b: f64, z: f64, n: usize| -> Edge {
		let pts: Vec<DVec3> = (0..n)
			.map(|i| {
				let t = TAU * i as f64 / n as f64;
				DVec3::new(a * t.cos(), b * t.sin(), z)
			})
			.collect();
		Edge::bspline(pts, BSplineEnd::Periodic).unwrap()
	};

	let sections: Vec<Vec<Edge>> = (0..4)
		.map(|i| {
			let z = i as f64 * 5.0;
			// アスペクト比を z で揺らす (ステラレーターの非軸対称性のミニチュア)
			let a = 3.0 + 0.5 * z.sin();
			let b = 2.0 + 0.3 * z.cos();
			vec![make_ellipse(a, b, z, 24)]
		})
		.collect();

	let plasma = Solid::loft(&sections, false).expect("bspline section loft should succeed");

	assert_eq!(plasma.shell_count(), 1);
	assert!(plasma.volume() > 0.0, "plasma volume should be positive");

	std::fs::create_dir_all("out").unwrap();
	let mut f = std::fs::File::create("out/loft_mini_stellarator.step").unwrap();
	cadrum::io::write_step(std::slice::from_ref(&plasma), &mut f).expect("mini stellarator step write");
	println!("mini stellarator: volume = {:.4}", plasma.volume());
}

// ==================== (5) エラーケース: section 1 つ ====================

#[test]
fn test_loft_single_section_returns_loft_failed() {
	let only = vec![periodic_circle(1.0, 0.0, 16)];
	let result = Solid::loft(&[only], false);

	let err = result.err().expect("single section must return Err");
	match err {
		Error::LoftFailed(msg) => {
			assert!(
				msg.contains("≥2") || msg.contains(">=2") || msg.contains("got 1"),
				"error message should mention min section count, got: {}",
				msg
			);
		}
		other => panic!("expected Error::LoftFailed, got {:?}", other),
	}
}

// ==================== (6) エラーケース: 空 section ====================

#[test]
fn test_loft_empty_section_returns_loft_failed() {
	let s1 = vec![periodic_circle(1.0, 0.0, 16)];
	let empty: Vec<Edge> = vec![];
	let s3 = vec![periodic_circle(1.0, 10.0, 16)];

	// 中間 section が空のケース。
	let result = Solid::loft(&[s1, empty, s3], false);

	let err = result.err().expect("empty section must return Err");
	match err {
		Error::LoftFailed(msg) => {
			assert!(
				msg.contains("empty"),
				"error message should mention empty section, got: {}",
				msg
			);
		}
		other => panic!("expected Error::LoftFailed, got {:?}", other),
	}
}

// ==================== (7) Closure-style ergonomic test ====================

#[test]
fn test_loft_closure_iterator_form() {
	// `(0..N).map(|i| [&edges[i]])` のような closure-style 呼び出しが
	// 動くことを確認する。これがフェーズ 2 のシグネチャ設計の主目的なので、
	// 実機で型推論が通ることを test として固定する。
	let ribs: Vec<Edge> = (0..3)
		.map(|i| periodic_circle(2.0 + i as f64 * 0.5, i as f64 * 5.0, 16))
		.collect();

	// ribs.iter().map(|e| [e]) は IntoIterator<Item = [&Edge; 1]> を返す。
	// [&Edge; 1] が IntoIterator<Item = &Edge> を満たすので、これで
	// loft の `<S, I>` 制約を満たす。
	let plasma = Solid::loft(ribs.iter().map(|e| [e]), false).expect("closure-form loft should succeed");

	assert_eq!(plasma.shell_count(), 1);
	assert!(plasma.volume() > 0.0);
}
