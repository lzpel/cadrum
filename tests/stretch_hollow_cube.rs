use chijin::Shape;
use glam::DVec3;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

const NORMAL_THRESHOLD: f64 = 0.99;
const COORD_TOLERANCE: f64 = 0.5;

// ── ユーティリティ ────────────────────────────────────────────────

fn axis_vec(axis: usize, v: f64) -> DVec3 {
	match axis {
		0 => DVec3::new(v, 0.0, 0.0),
		1 => DVec3::new(0.0, v, 0.0),
		_ => DVec3::new(0.0, 0.0, v),
	}
}

fn axis_unit(axis: usize) -> DVec3 {
	axis_vec(axis, 1.0)
}

fn axis_component(v: DVec3, axis: usize) -> f64 {
	match axis {
		0 => v.x,
		1 => v.y,
		_ => v.z,
	}
}

fn extrude_cut_faces(shape: &Shape, axis: usize, cut_coord: f64, delta: f64) -> Shape {
	let extrude_dir = axis_vec(axis, delta);
	let mut filler: Option<Shape> = None;
	for face in shape.faces() {
		let normal = face.normal_at_center();
		let center = face.center_of_mass();
		if axis_component(normal, axis).abs() > NORMAL_THRESHOLD
			&& (axis_component(center, axis) - cut_coord).abs() < COORD_TOLERANCE
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

fn stretch_axis(shape: &Shape, axis: usize, cut_coord: f64, delta: f64) -> Shape {
	let plane_origin = axis_vec(axis, cut_coord);
	let plane_normal = axis_unit(axis);
	let half = Shape::half_space(plane_origin, plane_normal);

	let part_neg = shape.intersect(&half);
	let part_pos = shape.subtract(&half).translated(axis_vec(axis, delta));

	let filler = extrude_cut_faces(&part_neg, axis, cut_coord, delta);
	part_neg.union(&filler).union(&part_pos)
}

fn stretch(shape: &Shape, cx: f64, cy: f64, cz: f64, dx: f64, dy: f64, dz: f64) -> Shape {
	let eps = 1e-10;
	let x = if dx > eps { Some(stretch_axis(shape, 0, cx, dx)) } else { None };
	let after_x = x.as_ref().unwrap_or(shape);
	let y = if dy > eps { Some(stretch_axis(after_x, 1, cy, dy)) } else { None };
	let after_y = y.as_ref().unwrap_or(after_x);
	let z = if dz > eps { Some(stretch_axis(after_y, 2, cz, dz)) } else { None };
	let after_z = z.as_ref().unwrap_or(after_y);
	after_z.clean()
}

// ── 今回の要件 ──────────────────────────────────────────────────

pub fn hollow_cube() -> Shape {
	let outer = Shape::box_from_corners(DVec3::new(-10., -10., -10.), DVec3::new(10., 10., 10.));
	let inner = Shape::box_from_corners(DVec3::new(-5., -5., -5.), DVec3::new(5., 5., 5.));
	outer.subtract(&inner)
}

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

struct Lcg {
	state: u32,
}
impl Lcg {
	fn new(seed: u32) -> Self {
		Self { state: seed }
	}
	fn next_f64(&mut self) -> f64 {
		self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
		(self.state as f64) / (u32::MAX as f64)
	}
	fn gen_range(&mut self, min: f64, max: f64) -> f64 {
		min + self.next_f64() * (max - min)
	}
}

#[test]
fn map_ok() {
	use std::io::Write;

	let out_dir = Path::new("out");
	if !out_dir.exists() {
		std::fs::create_dir_all(out_dir).unwrap();
	}

	let mut file = std::fs::File::create("out/map_ok.csv").unwrap();
	writeln!(file, "cx,cy,cz,dx,dy,dz,success,error_msg").unwrap();

	let base_shape = hollow_cube();
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
