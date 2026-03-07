use chijin::Shape;
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

/// 10×10×10 ボックス（体積 1000）
fn test_box() -> Shape {
	Shape::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))
}

// ==================== deep_copy ====================

#[test]
fn test_deep_copy_preserves_volume() {
	let original = test_box();
	let copy = original.deep_copy();
	drop(original);
	assert!((copy.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_deep_copy_is_independent() {
	// コピー後にオリジナルを boolean 操作しても copy は影響を受けない
	let original = test_box();
	let copy = original.deep_copy();
	let other = Shape::box_from_corners(dvec3(5.0, 5.0, 5.0), dvec3(15.0, 15.0, 15.0));
	let _ = Shape::from(original.union(&other).unwrap());
	assert!((copy.volume() - 1000.0).abs() < 1e-6);
}

// ==================== translated ====================

#[test]
fn test_translated_preserves_volume() {
	let shape = test_box();
	let moved = shape.translated(dvec3(100.0, 200.0, -50.0));
	assert!((moved.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn test_translated_preserves_shell_count() {
	let shape = test_box();
	let moved = shape.translated(dvec3(5.0, 0.0, 0.0));
	assert_eq!(moved.shell_count(), 1);
}

// ==================== rotated ====================

#[test]
fn test_rotated_preserves_volume() {
	let shape = test_box();
	// Z 軸周りに 45° 回転
	let rotated = shape.rotated(DVec3::ZERO, DVec3::Z, std::f64::consts::FRAC_PI_4);
	assert!((rotated.volume() - 1000.0).abs() < 1e-3);
}

#[test]
fn test_rotated_full_turn_preserves_volume() {
	let shape = test_box();
	// 360° 回転（元に戻る）
	let rotated = shape.rotated(DVec3::ZERO, DVec3::Z, std::f64::consts::TAU);
	assert!((rotated.volume() - 1000.0).abs() < 1e-3);
}

#[test]
fn test_rotated_preserves_shell_count() {
	let shape = test_box();
	let rotated = shape.rotated(DVec3::ZERO, DVec3::Y, std::f64::consts::FRAC_PI_2);
	assert_eq!(rotated.shell_count(), 1);
}

// ==================== scaled ====================

#[test]
fn test_scaled_volume() {
	let shape = test_box();
	// 均一 2 倍スケール → 体積は 2³ = 8 倍
	let scaled = shape.scaled(DVec3::ZERO, 2.0);
	assert!((scaled.volume() - 8000.0).abs() < 1e-3);
}

#[test]
fn test_scaled_half_volume() {
	let shape = test_box();
	// 均一 0.5 倍スケール → 体積は (0.5)³ = 0.125 倍 = 125
	let scaled = shape.scaled(DVec3::ZERO, 0.5);
	assert!((scaled.volume() - 125.0).abs() < 1e-3);
}

#[test]
fn test_scaled_preserves_shell_count() {
	let shape = test_box();
	let scaled = shape.scaled(DVec3::ZERO, 3.0);
	assert_eq!(scaled.shell_count(), 1);
}
