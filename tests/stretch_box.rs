use chijin::Shape;
use glam::DVec3;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

const NORMAL_THRESHOLD: f64 = 0.99;
const COORD_TOLERANCE: f64 = 0.5;

// ── ユーティリティ ────────────────────────────────────────────────

/// 指定された
/// 座標でカットされた面を押し出し、切断面を塞ぐための形状（フィラー）を生成します。
fn extrude_cut_faces(shape: &Shape, origin: DVec3, delta: DVec3) -> Shape {
	let plane_normal = delta.normalize();
	let extrude_dir = delta;
	let mut filler: Option<Shape> = None;
	for face in shape.faces() {
		let normal = face.normal_at_center();
		let center = face.center_of_mass();
		if normal.dot(plane_normal).abs() > NORMAL_THRESHOLD
			&& (center - origin).dot(plane_normal).abs() < COORD_TOLERANCE
		{
			let extruded = Shape::from(face.extrude(extrude_dir));
			filler = Some(match filler {
				None => extruded,
				Some(f) => f.union(&extruded),
			});
		}
	}
	filler.unwrap_or_else(Shape::empty)
}

/// 指定された座標とベクトルで形状を分割し、片方を平行移動させた後、隙間を押し出し形状で埋めることで引き伸ばしを行います。
fn stretch_vector(shape: &Shape, origin: DVec3, delta: DVec3) -> Shape {
	let plane_normal = delta.normalize();
	let half = Shape::half_space(origin, plane_normal);

	let part_neg = shape.intersect(&half);
	let part_pos = shape.subtract(&half).translated(delta);

	let filler = extrude_cut_faces(&part_neg, origin, delta);
	part_neg.union(&filler).union(&part_pos)
}

/// 形状をX, Y, Zの各軸方向に順番に引き伸ばします。
fn stretch(shape: &Shape, cx: f64, cy: f64, cz: f64, dx: f64, dy: f64, dz: f64) -> Shape {
	let eps = 1e-10;
	let origin = DVec3::new(cx, cy, cz);

	let x = if dx > eps {
		Some(stretch_vector(shape, origin, DVec3::new(dx, 0.0, 0.0)))
	} else {
		None
	};
	let after_x = x.as_ref().unwrap_or(shape);

	let y = if dy > eps {
		Some(stretch_vector(after_x, origin, DVec3::new(0.0, dy, 0.0)))
	} else {
		None
	};
	let after_y = y.as_ref().unwrap_or(after_x);

	let z = if dz > eps {
		Some(stretch_vector(after_y, origin, DVec3::new(0.0, 0.0, dz)))
	} else {
		None
	};
	let after_z = z.as_ref().unwrap_or(after_y);

	after_z.clean()
}

// ── 今回の要件 ──────────────────────────────────────────────────

/// テスト用のベース形状として、外部のSTEPファイルを読み込みます。
pub fn lambda360box() -> Shape {
	let mut file = std::fs::File::open(
		"tests/LAMBDA360-BOX-d6cb2eb2d6e0d802095ea1eda691cf9a3e9bf3394301a0d148f53e55f0f97951.step",
	)
	.expect("Failed to open step file");
	Shape::read_step(&mut file).expect("Failed to read step file")
}

/// 形状の引き伸ばし処理をパニックキャッチ付きで安全に実行し、結果を返します。
pub fn stretch_ok(
	shape: &Shape,
	cx: f64,
	cy: f64,
	cz: f64,
	dx: f64,
	dy: f64,
	dz: f64,
) -> Result<Shape, String> {
	let result = panic::catch_unwind(AssertUnwindSafe(|| stretch(shape, cx, cy, cz, dx, dy, dz)));

	match result {
		Ok(s) => {
			if s.is_null() {
				Err("Result shape is null".to_string())
			} else {
				Ok(s)
			}
		}
		Err(err) => {
			let msg = if let Some(s) = err.downcast_ref::<&str>() {
				(*s).to_string()
			} else if let Some(s) = err.downcast_ref::<String>() {
				s.clone()
			} else {
				"Unknown panic in shape operations".to_string()
			};
			Err(msg)
		}
	}
}

/// 線形合同法(LCG)によるシンプルな疑似乱数生成器です。
struct Lcg {
	state: u32,
}
impl Lcg {
	/// 指定されたシードで乱数生成器を初期化します。
	fn new(seed: u32) -> Self {
		Self { state: seed }
	}
	/// 0.0以上1.0未満の乱数を生成します。
	fn next_f64(&mut self) -> f64 {
		self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
		(self.state as f64) / (u32::MAX as f64)
	}
	/// 指定された範囲[min, max)の乱数を生成します。
	fn gen_range(&mut self, min: f64, max: f64) -> f64 {
		min + self.next_f64() * (max - min)
	}
}

#[test]
/// ランダムなパラメータで引き伸ばし処理を多数実行し、成功・失敗の結果をCSVに出力します。
fn map_ok() {
	use std::io::Write;

	let out_dir = Path::new("out");
	if !out_dir.exists() {
		std::fs::create_dir_all(out_dir).unwrap();
	}

	let mut file = std::fs::File::create("out/map_ok.csv").unwrap();
	writeln!(file, "cx,cy,cz,dx,dy,dz,success,error_msg").unwrap();

	let base_shape = lambda360box();
	let mut rng = Lcg::new(42);

	let mut success_count = 0;
	let total_attempts = 1000;

	for _i in 0..total_attempts {
		let cx = rng.gen_range(-15.0, 15.0);
		let cy = rng.gen_range(-15.0, 15.0);
		let cz = rng.gen_range(-15.0, 15.0);

		for (dx, dy, dz) in [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)] {
			match stretch_ok(&base_shape, cx, cy, cz, dx, dy, dz) {
				Ok(_) => {
					success_count += 1;
					writeln!(file, "{},{},{},{},{},{},1,", cx, cy, cz, dx, dy, dz).unwrap();
				}
				Err(e) => {
					writeln!(file, "{},{},{},{},{},{},0,{}", cx, cy, cz, dx, dy, dz, e).unwrap();
				}
			}
		}
	}

	println!(
		"Out of {} total tries, {} succeeded.",
		total_attempts * 3,
		success_count
	);
}
