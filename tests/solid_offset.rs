//! Integration tests for `Solid::offset`.
//!
//! Covers:
//! - 球の外向き/内向き offset → 体積が (r±t)³ 比で一致 (解析値)
//! - 立方体の外向き offset → 解析値 (a+2t)³ (Intersection join、角はシャープ)
//! - 薄板への過大な内向き offset → ドキュメント化された失敗モード (Err)
//! - 上面のみの部分 offset (push-pull) → 角柱が伸びる

use cadrum::{Error, Face, Solid};
use glam::DVec3;
use std::f64::consts::PI;

// ==================== (1) 球: 外向き offset ====================

#[test]
fn test_offset_01_sphere_outward_volume_matches_analytical() {
	let (r, t) = (2.0, 0.5);
	let sphere = Solid::sphere(r);

	let grown = sphere.offset(t, sphere.iter_face(), 1.0e-6).expect("outward sphere offset should succeed");

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
	let sphere = Solid::sphere(r);
	let shrunk = sphere.offset(t, sphere.iter_face(), 1.0e-6).expect("inward sphere offset should succeed");

	let expected = 4.0 / 3.0 * PI * (r + t).powi(3);
	let rel_err = (shrunk.volume() - expected).abs() / expected;
	assert!(rel_err < 0.01, "inward offset sphere volume {:.6} vs analytical {:.6} (relative error {:.4})", shrunk.volume(), expected, rel_err);
}

// ==================== (3) 立方体: 外向き offset (Intersection join、角はシャープ) ====================

#[test]
fn test_offset_03_cube_outward_volume_matches_analytical() {
	let (a, t) = (2.0, 0.5);
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(a));

	let grown = cube.offset(t, cube.iter_face(), 1.0e-6).expect("outward cube offset should succeed");

	// Intersection join: 隣接面が延長交差して角が保たれ、一辺 a+2t の立方体になる
	let expected = (a + 2.0 * t).powi(3);
	let rel_err = (grown.volume() - expected).abs() / expected;
	assert!(rel_err < 0.01, "offset cube volume {:.6} vs analytical {:.6} (relative error {:.4})", grown.volume(), expected, rel_err);
}

// ==================== (4) 薄板の過大な内向き offset → Err ====================

#[test]
fn test_offset_04_thin_plate_inward_returns_offset_failed() {
	// 厚さ 0.4 の板に -0.5 の offset: 対向 face の offset 面が交差する
	// (doc コメントに記載の thin-feature 失敗モード)
	let plate = Solid::cube(DVec3::ZERO, DVec3::new(10.0, 10.0, 0.4));

	let result = plate.offset(-0.5, plate.iter_face(), 1.0e-6);
	match result {
		Err(Error::Offset(msg)) => assert!(msg.contains("offset"), "got: {}", msg),
		Err(other) => panic!("expected Error::Offset, got {:?}", other),
		Ok(s) => panic!("thin-plate inward offset must fail, but produced a solid with volume {:.6}", s.volume()),
	}
}

// ==================== (5) 上面のみの部分 offset (push-pull) ====================

#[test]
fn test_offset_05_top_face_pad_extends_prism() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	// 中心からの最近点が z=10 に乗る face は上面だけ
	let top: Vec<&Face> = cube.iter_face().filter(|f| f.project(DVec3::splat(5.0)).0.z > 9.9).collect();
	assert_eq!(top.len(), 1);

	let padded = cube.offset(5.0, top, 1.0e-6).expect("padding the top face should succeed");
	assert!((padded.volume() - 1500.0).abs() < 1.0e-6, "volume {:.6} must be 10*10*15", padded.volume());
	let bbox = padded.bounding_box();
	assert!((bbox[0] - DVec3::ZERO).length() < 1.0e-6 && (bbox[1] - DVec3::new(10.0, 10.0, 15.0)).length() < 1.0e-6, "bbox must be the padded prism, got {:?}", bbox);
}
