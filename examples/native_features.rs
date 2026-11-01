use cadrum::*;
use std::{fs::File, hint::black_box, path::Path, time::Instant};

fn main() -> Result<(), Error> {
	let mut args = std::env::args().skip(1);
	let only = args.next().unwrap_or_else(|| "all".into());
	if only != "all" && !only.parse::<u32>().is_ok_and(|n| (1..=16).contains(&n)) {
		return Err(Error::Validation("usage: native_features [all|1..16] [output-directory]".into()));
	}
	let output = args.next().unwrap_or_else(|| "figure/native_features".into());
	let output = Path::new(&output);
	std::fs::create_dir_all(output)?;
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color("#c04020");
	let shape = Shape::from(&cube);
	let sphere = Shape::from(&Solid::sphere(2.0));
	let tool = Shape::from(&Solid::sphere(6.0).translate(DVec3::splat(8.0)));
	let edges = shape.edges()?;
	let outer = Wire::new([&Edge::circle(3.0, DVec3::Z)?])?;
	let hole = Wire::new([&Edge::circle(1.0, DVec3::Z)?])?;
	let face = Face::from_wires(&outer, &[&hole], 1e-7)?;
	let profile = Shape::from(&face);
	let sweep_profile: Shape = Wire::new([&Edge::circle(0.5, DVec3::Z)?])?.into();
	let spine = Wire::new([&Edge::line(DVec3::ZERO, DVec3::Z * 5.0)?])?;
	let upper = sweep_profile.scale_xyz(DVec3::ZERO, DVec3::new(2.0, 1.2, 1.0))?.value.translate(DVec3::new(1.0, 0.0, 5.0))?;
	let projection = Edge::line(DVec3::new(1.5, 0.0, 2.0), DVec3::new(2.5, 0.0, 2.0))?;
	let side = cube.iter_face().find(|f| (f.center().x - 10.0).abs() < 1e-8).unwrap();
	let triangles = TriangleMesh { positions: vec![DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z], triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]] };
	let control = OperationControl::default();
	#[cfg(feature = "step-document")]
	let document = StepDocument {
		definitions: vec![
			StepDefinition { id: "part".into(), name: "Shared cube".into(), geometry: Some(shape.copied()?.value), components: vec![] },
			StepDefinition {
				id: "assembly".into(),
				name: "Two placed instances".into(),
				geometry: None,
				components: vec![
					StepInstance { id: "left".into(), name: "Left cube".into(), reference_designator: Some("C1".into()), definition: 0, transform: Some(DMat4::IDENTITY) },
					StepInstance {
						id: "right".into(),
						name: "Rotated cube".into(),
						reference_designator: Some("C2".into()),
						definition: 0,
						transform: Some(DMat4::from_translation(DVec3::X * 18.0) * DMat4::from_rotation_z(0.35)),
					},
				],
			},
		],
		roots: vec![StepInstance { id: String::new(), name: String::new(), reference_designator: None, definition: 1, transform: Some(DMat4::IDENTITY) }],
	};
	let jobs: Vec<(u32, &str, Box<dyn Fn() -> Result<Shape, Error> + '_>)> = vec![
		(
			1,
			"open-topology",
			Box::new(|| {
				let faces = shape.faces()?;
				let top = faces.iter().max_by(|a, b| a.center().z.total_cmp(&b.center().z)).unwrap();
				shape.remove_faces([top], 1e-6)
			}),
		),
		(
			2,
			"validation",
			Box::new(|| {
				black_box(shape.validate(Some(ShapeKind::Solid))?);
				Ok(shape.clone())
			}),
		),
		(3, "affine", Box::new(|| Ok(sphere.scale_xyz(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0))?.value))),
		(
			4,
			"authored",
			Box::new(|| {
				let edge = CurveControlData::Bezier { poles: vec![DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y], weights: vec![1.0, 0.5f64.sqrt(), 1.0] }.edge(None)?;
				black_box(edge.evaluate(0.5)?);
				Ok(Shape::from(&edge))
			}),
		),
		(5, "profiles", Box::new(|| Ok(profile.extrude(DVec3::new(1.0, 2.0, 5.0))?.value))),
		(6, "faceted", Box::new(|| Ok(triangles.import(MeshImportOptions { outcome: MeshOutcome::ClosedSolid, ..Default::default() })?.shape.unwrap()))),
		(
			7,
			"history",
			Box::new(|| {
				let a = shape.copied()?;
				let b = a.value.transform_with_history(ShapeTransform::Mirror { origin: DVec3::ZERO, normal: DVec3::X })?;
				black_box(a.history.compose(&b.history, 0));
				Ok(Shape::compound([&shape, &b.value.translate(-DVec3::X * 3.0)?])?.into())
			}),
		),
		(
			8,
			"split",
			Box::new(|| {
				let normal = DVec3::new(1.0, 2.0, 3.0).normalize();
				let split = sphere.split_by_plane(Plane { origin: DVec3::ZERO, normal }, 1e-7)?;
				let parts = split.partitions.iter().map(|p| p.shape.translate(normal * if p.side == PartitionSide::Positive { 0.5 } else { -0.5 })).collect::<Result<Vec<_>, _>>()?;
				Ok(Shape::compound(&parts)?.into())
			}),
		),
		(9, "boolean", Box::new(|| Ok(Shape::boolean_batch(BooleanOp::Difference, &[&shape], &[&tool], BooleanOptions::default(), &control)?.value))),
		(
			10,
			"tessellation",
			Box::new(|| {
				black_box(sphere.tessellate(Tessellation::default(), &control)?);
				Ok(sphere.clone())
			}),
		),
		(
			11,
			"measurements",
			Box::new(|| {
				black_box(shape.mass_properties(MeasurementKind::Volume, 1e-9)?);
				let separate = sphere.translate(-DVec3::X * 12.0)?;
				black_box(shape.distance(&separate, 1e-7)?);
				Ok(Shape::compound([&shape, &separate])?.into())
			}),
		),
		(12, "sweep", Box::new(|| Ok(spine.sweep(&[SweepSection { profile: &sweep_profile, location: None, contact: false, correction: true }], SweepOptions { simulation_sections: 3, scaling: vec![[0.0, 1.0], [1.0, 2.0]], ..Default::default() }, &control)?.result.value))),
		(
			13,
			"loft-repair",
			Box::new(|| {
				black_box(shape.repair(RepairOptions::default())?);
				Ok(Shape::loft(&[&sweep_profile, &upper], LoftOptions::default(), &control)?.value)
			}),
		),
		(14, "edge-treatments", Box::new(|| Ok(shape.variable_fillet(&[VariableFillet { edge: &edges[0], radii: vec![[0.0, 0.8], [1.0, 2.0]] }], &control)?.value))),
		(
			15,
			"draft-projection",
			Box::new(|| {
				let drafted = shape.draft(&[side], DraftOptions { neutral_plane: Plane { origin: DVec3::ZERO, normal: DVec3::Z }, pull_direction: DVec3::Z, angle: 0.15, flag: true }, &control)?;
				black_box(projection.project_onto(&profile, Projection::Directional(-DVec3::Z), ProjectionOptions::default())?);
				Ok(drafted.result.value)
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
					let mut bytes = Vec::new();
					document.write(&mut bytes, StepWriteOptions::default(), &control)?;
					Ok(StepDocument::read(&mut bytes.as_slice(), StepReadOptions::default(), &control)?.result.value)
				}
				#[cfg(not(feature = "step-document"))]
				Ok(shape.clone())
			}),
		),
	];
	for (number, name, run) in jobs {
		if only != "all" && only != number.to_string() {
			continue;
		}
		let start = Instant::now();
		run()?;
		let first = start.elapsed();
		let start = Instant::now();
		let result = run()?;
		let repeat = start.elapsed();
		let peak = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|line| line.starts_with("VmHWM:")).map(str::to_owned)).unwrap_or_else(|| "process peak unavailable".into());
		println!("{number} {name}: first_us={} repeat_us={} {peak}", first.as_micros(), repeat.as_micros());
		export(&result, &output.join(format!("{number:02}-{name}")), if number == 6 { DVec3::new(1.0, -1.0, 1.0) } else { DVec3::ONE }, &control)?;
		if number == 4 {
			let poles = (0..3).flat_map(|u| (0..3).map(move |v| DVec3::new(u as f64, v as f64, if u == 1 && v == 1 { 3.0 } else { 0.0 }))).collect();
			let patch = SurfaceControlData::Bezier { u_count: 3, v_count: 3, poles, weights: vec![] }.face(None)?;
			export(&Shape::from(&patch), &output.join("04-authored-surface"), DVec3::ONE, &control)?;
		}
		if number == 15 {
			let projected = projection.project_onto(&profile, Projection::Directional(-DVec3::Z), ProjectionOptions::default())?;
			let curves = Shape::compound([&Shape::from(&projection), &projected.value])?;
			export(&curves, &output.join("15-projected-curves"), DVec3::ONE, &control)?;
		}
		#[cfg(feature = "step-document")]
		if number == 16 {
			document.write(&mut File::create(output.join("16-step-assembly.step"))?, StepWriteOptions { output_unit: LengthUnit::Inch, ..Default::default() }, &control)?;
		}
	}
	println!("Artifacts: {}", output.display());
	Ok(())
}

fn export(shape: &Shape, path: &Path, view: DVec3, control: &OperationControl) -> Result<(), Error> {
	shape.write_step(&mut File::create(path.with_extension("step"))?, control)?;
	let mesh = shape.mesh(Tessellation { deflection_linear: 0.004, deflection_angular: 0.2, ..Default::default() })?;
	let scene = mesh.scene(SceneOption { view, shading: true, hidden_edges: false, ..Default::default() });
	if !mesh.indices.is_empty() {
		mesh.write_stl(&mut File::create(path.with_extension("stl"))?)?;
	}
	scene.write_svg(&mut File::create(path.with_extension("svg"))?)?;
	#[cfg(feature = "png")]
	{
		let mut bytes = Vec::new();
		scene.write_png([640, 480], &mut bytes)?;
		let image = tiny_skia::Pixmap::decode_png(&bytes).map_err(|e| Error::Io(std::io::Error::other(e)))?;
		let mut background = tiny_skia::Pixmap::new(640, 480).unwrap();
		background.fill(tiny_skia::Color::WHITE);
		background.draw_pixmap(0, 0, image.as_ref(), &Default::default(), Default::default(), None);
		background.save_png(path.with_extension("png")).map_err(|e| Error::Io(std::io::Error::other(e)))?;
	}
	Ok(())
}
