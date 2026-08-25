//! Integration tests for `Solid::sew`.
//!
//! Covers:
//! - 分解した box の 6 face を縫合 → 元の体積が回復する
//! - 開いた shell (face 不足) はエラー
//! - 空入力はエラー

use cadrum::{Error, Face, Solid};
use glam::DVec3;

// ==================== (1) box の 6 face を縫合して体積回復 ====================

#[test]
fn test_sew_01_box_faces_recover_volume() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
	let expected = cube.volume();

	let sewn = Solid::sew(cube.iter_face(), 1.0e-6).expect("sewing 6 box faces should succeed");

	let rel = (sewn.volume() - expected).abs() / expected;
	assert!(rel < 1.0e-9, "sewn volume {:.9} vs original {:.9} (relative error {:.3e})", sewn.volume(), expected, rel);

	let bbox = sewn.bounding_box();
	assert!((bbox[0] - DVec3::ZERO).length() < 1.0e-6 && (bbox[1] - DVec3::new(2.0, 3.0, 4.0)).length() < 1.0e-6, "sewn bounding box {:?} must match the original box", bbox);
	assert!(sewn.contains(DVec3::new(1.0, 1.5, 2.0)));
}

// ==================== (2) face 不足 (開いた shell) はエラー ====================

#[test]
fn test_sew_02_open_shell_returns_sew_failed() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let five: Vec<&Face> = cube.iter_face().take(5).collect();

	let err = Solid::sew(five, 1.0e-6).err().expect("5 of 6 faces must not sew into a solid");
	match err {
		Error::SewFailed(msg) => assert!(msg.contains("closed shell"), "error message should mention closed shell, got: {}", msg),
		other => panic!("expected Error::SewFailed, got {:?}", other),
	}
}

// ==================== (3) 空入力はエラー ====================

#[test]
fn test_sew_03_empty_input_returns_sew_failed() {
	let none: Vec<&Face> = vec![];
	let err = Solid::sew(none, 1.0e-6).err().expect("empty face set must return Err");
	match err {
		Error::SewFailed(msg) => assert!(msg.contains("no faces"), "error message should mention empty input, got: {}", msg),
		other => panic!("expected Error::SewFailed, got {:?}", other),
	}
}
