use cadrum::*;

fn main() -> Result<(), Error> {
	let output = std::env::args().nth(1).unwrap_or_else(|| "out/surface_modeling".into());
	std::fs::create_dir_all(&output)?;
	let patch = SurfaceControlData::Bezier { u_count: 2, v_count: 2, poles: vec![DVec3::ZERO, DVec3::Y * 2.0, DVec3::X * 2.0, DVec3::new(2.0, 2.0, 0.0)], weights: vec![] }.face(None)?;
	let source = Shape::from(&patch);
	let edges = source.edges()?;
	let boundary: Vec<_> = edges.iter().map(|edge| FillBoundary { edge, support: None, continuity: FillContinuity::Position }).collect();
	let control = OperationControl::default();
	let filled = Face::fill(&boundary, &[DVec3::new(1.0, 1.0, 0.5)], FillingOptions::default(), &control)?;
	println!("Native filling constraint errors: {:?}", filled.maximum_errors);
	let extended = patch.extend_surface(SurfaceBoundary::UMax, 1.0, 2, 1e-7)?;
	let outer = source.wires()?.remove(0);
	let hole = Wire::new([&Edge::circle(0.3, DVec3::Z)?.translate(DVec3::new(1.0, 1.0, 0.0))])?;
	let trimmed = patch.retrim_surface(&outer, &[&hole], 1e-7)?;
	let line = Shape::from(&Edge::line(DVec3::new(0.0, 1.0, 0.0), DVec3::new(2.0, 1.0, 0.0))?);
	let imprinted = source.imprint(&patch, &[&line], 1e-7, &control)?;
	for (name, shape) in [("filling", filled.result.value), ("extension", extended.value), ("hole-trim", trimmed.value), ("imprint", imprinted.value)] {
		println!("{name}: {} faces; bounds {:?}", shape.faces()?.len(), shape.bounding_box()?);
		#[cfg(feature = "png")]
		shape.mesh(Tessellation::default())?.scene(SceneOption { shading: true, ..Default::default() }).write_png([640, 480], &mut std::fs::File::create(format!("{output}/{name}.png"))?)?;
	}
	Ok(())
}
