use cadrum::*;

fn patch() -> Face {
	SurfaceControlData::Bezier { u_count: 2, v_count: 2, poles: vec![DVec3::ZERO, DVec3::Y * 2.0, DVec3::X * 2.0, DVec3::new(2.0, 2.0, 0.0)], weights: vec![] }.face(None).unwrap()
}
fn rectangle(a: DVec3, b: DVec3) -> Wire {
	let corners = [a, DVec3::new(b.x, a.y, a.z), b, DVec3::new(a.x, b.y, b.z)];
	let edges: Vec<_> = (0..4).map(|i| Edge::line(corners[i], corners[(i + 1) % 4]).unwrap()).collect();
	Wire::new(&edges).unwrap()
}
#[test]
fn fills_closed_boundaries_and_reports_requested_continuity_errors() {
	let source = patch();
	let edges = Shape::from(&source).edges().unwrap();
	for continuity in [FillContinuity::Position, FillContinuity::Tangent, FillContinuity::Curvature] {
		let constraints: Vec<_> = edges.iter().map(|edge| FillBoundary { edge, support: Some(&source), continuity }).collect();
		let fill = Face::fill(&constraints, &[], FillingOptions { initial_surface: Some(&source), ..Default::default() }, &OperationControl::default()).unwrap_or_else(|e| panic!("{continuity:?}: {e}"));
		assert_eq!(fill.result.value.kind(), ShapeKind::Face);
		assert!(fill.result.value.validate(None).unwrap().valid);
		assert!(fill.maximum_errors[0] <= 1e-4);
		assert!(fill.maximum_errors[1] <= 0.01);
		assert!(fill.maximum_errors[2] <= 0.1);
		assert!((fill.result.value.mass_properties(MeasurementKind::Surface, 1e-8).unwrap().mass - 4.0).abs() < 1e-3);
	}
	assert_eq!(source.parameter_range().unwrap(), [0.0, 1.0, 0.0, 1.0]);
}
#[test]
fn fills_nonplanar_point_constraints_without_mutating_the_boundary() {
	let source = rectangle(DVec3::ZERO, DVec3::new(2.0, 2.0, 0.0));
	let edges = source.edges().unwrap();
	let boundary: Vec<_> = edges.iter().map(|edge| FillBoundary { edge, support: None, continuity: FillContinuity::Position }).collect();
	let fill = Face::fill(&boundary, &[DVec3::new(1.0, 1.0, 0.5)], FillingOptions::default(), &OperationControl::default()).unwrap();
	assert!(fill.result.value.bounding_box().unwrap().unwrap()[1].z > 0.49);
	assert!(source.bounding_box().unwrap().unwrap()[1].z < 1e-5);
}
#[test]
fn extends_each_boundary_and_preserves_the_source() {
	let face = patch();
	for boundary in [SurfaceBoundary::UMin, SurfaceBoundary::UMax, SurfaceBoundary::VMin, SurfaceBoundary::VMax] {
		for continuity in 1..=3 {
			let extension = face.extend_surface(boundary, 1.0, continuity, 1e-7).unwrap();
			let bounds = extension.value.bounding_box().unwrap().unwrap();
			let dimension = if matches!(boundary, SurfaceBoundary::UMin | SurfaceBoundary::UMax) { bounds[1].x - bounds[0].x } else { bounds[1].y - bounds[0].y };
			assert!((dimension - 3.0).abs() < 1e-4, "{boundary:?}: {bounds:?}");
			assert!(!extension.history.complete);
			assert!(!extension.warnings.is_empty());
		}
	}
	assert_eq!(face.parameter_range().unwrap(), [0.0, 1.0, 0.0, 1.0]);
	let trimmed = face.surface_patch([0.1, 0.9, 0.1, 0.9]).unwrap();
	assert!(matches!(trimmed.extend_surface(SurfaceBoundary::UMax, 1.0, 1, 1e-7), Err(Error::Unsupported { .. })));
	assert!(face.extend_surface(SurfaceBoundary::UMax, 1.0, 0, 1e-7).is_err());
}
#[test]
fn retrims_with_holes_and_rejects_off_surface_boundaries() {
	let face = patch();
	let outer = rectangle(DVec3::new(0.2, 0.2, 0.0), DVec3::new(1.8, 1.8, 0.0));
	let hole = rectangle(DVec3::new(0.8, 0.8, 0.0), DVec3::new(1.2, 1.2, 0.0));
	let trimmed = face.retrim_surface(&outer, &[&hole], 1e-7).unwrap();
	assert!(trimmed.value.validate(None).unwrap().valid);
	let result = trimmed.value.as_face().unwrap();
	assert_eq!(result.boundary_wires().unwrap().len(), 2);
	let area = trimmed.value.mass_properties(MeasurementKind::Surface, 1e-8).unwrap().mass;
	assert!((area - 2.4).abs() < 1e-6, "{area}");
	assert_eq!(Shape::from(&face).as_face().unwrap().boundary_wires().unwrap().len(), 1);
	let raised = Wire::try_from(outer.translate(DVec3::Z).unwrap()).unwrap();
	assert!(face.retrim_surface(&raised, &[], 1e-7).is_err());
}
#[test]
fn imprints_create_faces_without_changing_volume_and_reject_foreign_faces() {
	let solid = Solid::cube(DVec3::ZERO, DVec3::splat(2.0)).color("#cc3300");
	let shape = Shape::from(&solid);
	let faces = shape.faces().unwrap();
	let top = faces.iter().find(|f| f.center().z > 1.9).unwrap();
	let line = Shape::from(&Edge::line(DVec3::new(0.0, 1.0, 2.0), DVec3::new(2.0, 1.0, 2.0)).unwrap());
	let result = shape.imprint(top, &[&line], 1e-7, &OperationControl::default()).unwrap();
	assert_eq!(result.value.faces().unwrap().len(), 7);
	assert!((result.value.mass_properties(MeasurementKind::Volume, 1e-8).unwrap().mass - 8.0).abs() < 1e-7);
	assert!(result.value.mesh(Tessellation::default()).unwrap().colormap.len() >= 7);
	assert_eq!(shape.faces().unwrap().len(), 6);
	assert!(shape.imprint(&patch(), &[&line], 1e-7, &OperationControl::default()).is_err());
	let off_surface = line.translate(DVec3::Z).unwrap();
	assert!(shape.imprint(top, &[&off_surface], 1e-7, &OperationControl::default()).is_err());
}
#[test]
fn rejects_invalid_constraints_and_observes_cancellation() {
	let edge = Edge::line(DVec3::ZERO, DVec3::X).unwrap();
	let boundary = [FillBoundary { edge: &edge, support: None, continuity: FillContinuity::Tangent }];
	assert!(matches!(Face::fill(&boundary, &[], FillingOptions::default(), &OperationControl::default()), Err(Error::Validation(_))));
	let boundary = [FillBoundary { continuity: FillContinuity::Position, ..boundary[0] }];
	assert!(Face::fill(&boundary, &[], FillingOptions::default(), &OperationControl::default()).is_err());
	let control = OperationControl::default();
	control.cancellation_token().cancel();
	assert!(matches!(Face::fill(&boundary, &[], FillingOptions::default(), &control), Err(Error::Cancelled { .. })));
	assert!(Face::fill(&boundary, &[DVec3::NAN], FillingOptions::default(), &OperationControl::default()).is_err());
}

#[test]
fn curvature_fill_accepts_curved_support_and_extension_keeps_orientation() {
	let poles = (0..3).flat_map(|u| (0..2).map(move |v| DVec3::new(u as f64, v as f64 * 2.0, if u == 1 { 0.5 } else { 0.0 }))).collect();
	let face = SurfaceControlData::Bezier { u_count: 3, v_count: 2, poles, weights: vec![] }.face(None).unwrap();
	let edges = Shape::from(&face).edges().unwrap();
	let constraints: Vec<_> = edges.iter().map(|edge| FillBoundary { edge, support: Some(&face), continuity: FillContinuity::Curvature }).collect();
	let fill = Face::fill(&constraints, &[], FillingOptions { initial_surface: Some(&face), ..Default::default() }, &OperationControl::default()).unwrap();
	assert!(fill.maximum_errors[2] <= 0.1);
	assert!(fill.result.value.bounding_box().unwrap().unwrap()[1].z > 0.1);
	let reversed = Shape::from(&face).reversed().unwrap().value.as_face().unwrap();
	let extension = reversed.extend_surface(SurfaceBoundary::VMax, 0.25, 2, 1e-7).unwrap();
	let normal = extension.value.as_face().unwrap().evaluate_uv(0.5, 0.5).unwrap().normal;
	assert!(normal.dot(reversed.evaluate_uv(0.5, 0.5).unwrap().normal) > 0.99);
}

#[test]
fn imprints_closed_loops_and_checks_curved_trim_domains() {
	let cylinder = Shape::from(&Solid::cylinder(2.0, DVec3::Z * 4.0));
	let faces = cylinder.faces().unwrap();
	let side = faces.iter().find(|f| f.surface_info().unwrap().kind == GeometrySurfaceKind::Cylinder).unwrap();
	let loop_shape: Shape = Wire::new([&Edge::circle(2.0, DVec3::Z).unwrap()]).unwrap().translate(DVec3::Z * 2.0).unwrap();
	let split = cylinder.imprint(side, &[&loop_shape], 1e-7, &OperationControl::default()).unwrap();
	assert_eq!(split.value.faces().unwrap().len(), 4);
	assert!((split.value.mass_properties(MeasurementKind::Volume, 1e-8).unwrap().mass - std::f64::consts::PI * 16.0).abs() < 1e-6);
	let face = patch();
	let outer = rectangle(DVec3::new(0.2, 0.2, 0.0), DVec3::new(1.8, 1.8, 0.0));
	let imprint = Shape::from(&face).imprint(&face, &[&outer], 1e-7, &OperationControl::default()).unwrap();
	assert_eq!(imprint.value.faces().unwrap().len(), 2);
}

#[test]
fn retrims_a_cylindrical_patch_away_from_the_seam() {
	let cylinder = Shape::from(&Solid::cylinder(2.0, DVec3::Z * 4.0));
	let side = cylinder.faces().unwrap().into_iter().find(|f| f.surface_info().unwrap().kind == GeometrySurfaceKind::Cylinder).unwrap();
	let patch = side.surface_patch([0.2, 1.2, 0.5, 3.5]).unwrap();
	let boundary = Shape::from(&patch).wires().unwrap().remove(0);
	let trimmed = side.retrim_surface(&boundary, &[], 1e-7).unwrap();
	assert!((trimmed.value.mass_properties(MeasurementKind::Surface, 1e-8).unwrap().mass - 6.0).abs() < 1e-6);
	assert_eq!(trimmed.value.as_face().unwrap().surface_info().unwrap().kind, GeometrySurfaceKind::Cylinder);
}
