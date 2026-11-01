use cadrum::*;
use std::{hint::black_box, time::Instant};

fn main() -> Result<(), Error> {
	let only = std::env::args().nth(1);
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color("#c04020");
	let shape = Shape::from(&cube);
	let sphere = Shape::from(&Solid::sphere(2.0));
	let tool = Shape::from(&Solid::sphere(5.0).translate(DVec3::splat(5.0)));
	let edges = shape.edges()?;
	let outer = Wire::new([&Edge::circle(3.0, DVec3::Z)?])?;
	let hole = Wire::new([&Edge::circle(1.0, DVec3::Z)?])?;
	let face = Face::from_wires(&outer, &[&hole], 1e-7)?;
	let profile = Shape::from(&face);
	let sweep_profile: Shape = Wire::new([&Edge::circle(0.5, DVec3::Z)?])?.into();
	let spine = Wire::new([&Edge::line(DVec3::ZERO, DVec3::Z * 5.0)?])?;
	let upper = sweep_profile.translate(DVec3::Z * 5.0)?;
	let projection = Edge::line(DVec3::new(1.5, 0.0, 2.0), DVec3::new(2.5, 0.0, 2.0))?;
	let side = cube.iter_face().find(|f| (f.center().x - 10.0).abs() < 1e-8).unwrap();
	let triangles = TriangleMesh { positions: vec![DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z], triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]] };
	let control = OperationControl::default();
	let jobs: Vec<(u32, &str, Box<dyn Fn() -> Result<(), Error> + '_>)> = vec![
		(
			2,
			"validation",
			Box::new(|| {
				black_box(shape.validate(Some(ShapeKind::Solid))?);
				Ok(())
			}),
		),
		(
			3,
			"affine",
			Box::new(|| {
				black_box(sphere.scale_xyz(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0))?);
				Ok(())
			}),
		),
		(
			4,
			"authored",
			Box::new(|| {
				let edge = CurveControlData::Bezier { poles: vec![DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y], weights: vec![1.0, 0.5f64.sqrt(), 1.0] }.edge(None)?;
				black_box(edge.evaluate(0.5)?);
				Ok(())
			}),
		),
		(
			5,
			"profiles",
			Box::new(|| {
				black_box(profile.extrude(DVec3::new(1.0, 2.0, 5.0))?);
				Ok(())
			}),
		),
		(
			6,
			"faceted",
			Box::new(|| {
				black_box(triangles.import(MeshImportOptions { outcome: MeshOutcome::ClosedSolid, ..Default::default() })?);
				Ok(())
			}),
		),
		(
			7,
			"history",
			Box::new(|| {
				let a = shape.copied()?;
				let b = a.value.transform_with_history(ShapeTransform::Mirror { origin: DVec3::ZERO, normal: DVec3::X })?;
				black_box(a.history.compose(&b.history, 0));
				Ok(())
			}),
		),
		(
			8,
			"split",
			Box::new(|| {
				black_box(sphere.split_by_plane(Plane { origin: DVec3::ZERO, normal: DVec3::new(1.0, 2.0, 3.0) }, 1e-7)?);
				Ok(())
			}),
		),
		(
			9,
			"boolean",
			Box::new(|| {
				black_box(Shape::boolean_batch(BooleanOp::Difference, &[&shape], &[&tool], BooleanOptions::default(), &control)?);
				Ok(())
			}),
		),
		(
			10,
			"tessellation",
			Box::new(|| {
				black_box(sphere.tessellate(Tessellation::default(), &control)?);
				Ok(())
			}),
		),
		(
			11,
			"measurements",
			Box::new(|| {
				black_box(shape.mass_properties(MeasurementKind::Volume, 1e-9)?);
				black_box(shape.distance(&sphere, 1e-7)?);
				Ok(())
			}),
		),
		(
			12,
			"sweep",
			Box::new(|| {
				black_box(spine.sweep(&[SweepSection { profile: &sweep_profile, location: None, contact: false, correction: true }], SweepOptions { simulation_sections: 3, ..Default::default() }, &control)?);
				Ok(())
			}),
		),
		(
			13,
			"loft-repair",
			Box::new(|| {
				black_box(Shape::loft(&[&sweep_profile, &upper], LoftOptions::default(), &control)?);
				black_box(shape.repair(RepairOptions::default())?);
				Ok(())
			}),
		),
		(
			14,
			"edge-treatments",
			Box::new(|| {
				black_box(shape.variable_fillet(&[VariableFillet { edge: &edges[0], radii: vec![[0.0, 0.2], [1.0, 0.5]] }], &control)?);
				Ok(())
			}),
		),
		(
			15,
			"draft-projection",
			Box::new(|| {
				black_box(shape.draft(&[side], DraftOptions { neutral_plane: Plane { origin: DVec3::ZERO, normal: DVec3::Z }, pull_direction: DVec3::Z, angle: 0.05, flag: true }, &control)?);
				black_box(projection.project_onto(&profile, Projection::Directional(-DVec3::Z), ProjectionOptions::default())?);
				Ok(())
			}),
		),
		(
			16,
			"step",
			Box::new(|| {
				let mut bytes = Vec::new();
				shape.write_step_with_options(&mut bytes, StepWriteOptions { output_unit: LengthUnit::Inch, ..Default::default() }, &control)?;
				black_box(Shape::read_step_with_options(&mut bytes.as_slice(), StepReadOptions::default(), &control)?);
				#[cfg(feature = "step-document")]
				{
					let document = StepDocument {
						definitions: vec![StepDefinition { id: "part".into(), name: "Example part".into(), geometry: Some(shape.copied()?.value), components: vec![] }],
						roots: vec![StepInstance { id: String::new(), name: String::new(), reference_designator: None, definition: 0, transform: Some(DMat4::IDENTITY) }],
					};
					let mut bytes = Vec::new();
					document.write(&mut bytes, StepWriteOptions::default(), &control)?;
					black_box(StepDocument::read(&mut bytes.as_slice(), StepReadOptions::default(), &control)?);
				}
				Ok(())
			}),
		),
	];
	for (number, name, run) in jobs {
		if only.as_ref().is_some_and(|v| v != &number.to_string()) {
			continue;
		}
		let start = Instant::now();
		run()?;
		let first = start.elapsed();
		let start = Instant::now();
		run()?;
		let repeat = start.elapsed();
		let peak = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|line| line.starts_with("VmHWM:")).map(str::to_owned)).unwrap_or_else(|| "process peak unavailable".into());
		println!("{number} {name}: first_us={} repeat_us={} {peak}", first.as_micros(), repeat.as_micros());
	}
	Ok(())
}
