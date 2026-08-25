//! Integration tests for `Solid::offset_surface`.
//!
//! Covers:
//! - 球の外向き/内向き offset → 体積が (r±t)³ 比で一致 (解析値)
//! - 立方体の外向き offset → 解析値 (面スラブ + 辺 1/4 円柱 + 角 1/8 球)
//! - 薄板への過大な内向き offset → ドキュメント化された失敗モード (Err)

use cadrum::{Error, Solid};
use glam::DVec3;
use std::f64::consts::PI;

// ==================== (1) 球: 外向き offset ====================

#[test]
fn test_offset_01_sphere_outward_volume_matches_analytical() {
	let (r, t) = (2.0, 0.5);
	let sphere = Solid::sphere(r);

	let grown = sphere.offset_surface(t, 1.0e-6).expect("outward sphere offset should succeed");

	let expected = 4.0 / 3.0 * PI * (r + t).powi(3);
	let rel_err = (grown.volume() - expected).abs() / expected;
	assert!(rel_err < 0.01, "offset sphere volume {:.6} vs analytical {:.6} (relative error {:.4})", grown.volume(), expected, rel_err);

	// 体積比は ((r+t)/r)³
	let ratio = grown.volume() / sphere.volume();
	let expected_ratio = ((r + t) / r).powi(3);
	assert!((ratio - expected_ratio).abs() / expected_ratio < 0.01, "volume ratio {:.6} vs ((r+t)/r)³ = {:.6}", ratio, expected_ratio);
}

// ==================== (2) 球: 内向き offset ====================

#[test]
fn test_offset_02_sphere_inward_volume_matches_analytical() {
	let (r, t) = (2.0, -0.5);
	let shrunk = Solid::sphere(r).offset_surface(t, 1.0e-6).expect("inward sphere offset should succeed");

	let expected = 4.0 / 3.0 * PI * (r + t).powi(3);
	let rel_err = (shrunk.volume() - expected).abs() / expected;
	assert!(rel_err < 0.01, "inward offset sphere volume {:.6} vs analytical {:.6} (relative error {:.4})", shrunk.volume(), expected, rel_err);
}

// ==================== (3) 立方体: 外向き offset (Arc join の丸め込み) ====================

#[test]
fn test_offset_03_cube_outward_volume_matches_analytical() {
	let (a, t) = (2.0, 0.5);
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(a));

	let grown = cube.offset_surface(t, 1.0e-6).expect("outward cube offset should succeed");

	// Arc join: 面スラブ 6a²t + 辺 1/4 円柱 12·(πt²/4)·a + 角 1/8 球 8·(4πt³/3)/8
	let expected = a.powi(3) + 6.0 * a * a * t + 3.0 * PI * a * t * t + 4.0 / 3.0 * PI * t.powi(3);
	let rel_err = (grown.volume() - expected).abs() / expected;
	assert!(rel_err < 0.01, "offset cube volume {:.6} vs analytical {:.6} (relative error {:.4})", grown.volume(), expected, rel_err);
}

// ==================== (4) 薄板の過大な内向き offset → Err ====================

#[test]
fn test_offset_04_thin_plate_inward_returns_offset_failed() {
	// 厚さ 0.4 の板に -0.5 の offset: 対向 face の offset 面が交差する
	// (doc コメントに記載の thin-feature 失敗モード)
	let plate = Solid::cube(DVec3::ZERO, DVec3::new(10.0, 10.0, 0.4));

	let result = plate.offset_surface(-0.5, 1.0e-6);
	match result {
		Err(Error::OffsetFailed(msg)) => assert!(msg.contains("offset"), "got: {}", msg),
		Err(other) => panic!("expected Error::OffsetFailed, got {:?}", other),
		Ok(s) => panic!("thin-plate inward offset must fail, but produced a solid with volume {:.6}", s.volume()),
	}
}
