use chijin::{utils::{revolve_section, stretch_vector}, Shape};
use glam::DVec3;

fn dvec3(x: f64, y: f64, z: f64) -> DVec3 {
	DVec3::new(x, y, z)
}

fn test_box() -> Shape {
	Shape::box_from_corners(dvec3(0.0, 0.0, 0.0), dvec3(10.0, 10.0, 10.0))
}

#[test]
fn test_stretch_vector_volume() {
	let shape = test_box();
	// X=5 で切断し +X 方向に 1 引き延ばす → 10×10×11 = 1100
	let result = stretch_vector(&shape, dvec3(5.0, 0.0, 0.0), dvec3(1.0, 0.0, 0.0)).unwrap();
	let v = result.volume();
	assert!((v - 1100.0).abs() < 1e-3, "expected volume ≈ 1100, got {v}");
}

#[test]
fn test_revolve_section_volume() {
	let shape = test_box();
	// y=5 の平面（法線=Y）で切断し、Z 軸（x=0,y=5 を通る）周りに全周回転。
	// 断面は x:0..10, z:0..10 の矩形 → 半径 10・高さ 10 の円柱。
	// 期待体積 = π × 10² × 10 = 1000π ≈ 3141.59
	let result = revolve_section(
		&shape,
		dvec3(0.0, 5.0, 0.0), // origin: Z軸上かつ y=5 平面上
		dvec3(0.0, 0.0, 1.0), // axis_dir: Z
		dvec3(0.0, 1.0, 0.0), // plane_normal: Y（axis_dir ⊥ plane_normal）
		std::f64::consts::TAU,
	)
	.unwrap();
	let v = result.volume();

	std::fs::create_dir_all("out").unwrap();
	let mut file = std::fs::File::create("out/revolve_section.step").unwrap();
	result.write_step(&mut file).expect("STEP write failed");

	let expected = std::f64::consts::PI * 10.0f64.powi(2) * 10.0;
	assert!((v - expected).abs() < 1.0, "expected volume ≈ {expected:.2}, got {v}");
}
