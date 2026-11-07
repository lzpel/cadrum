use cadrum::*;
fn square(z: f64) -> Shape {
	let wire = Wire::new(&Edge::polygon(&[DVec3::new(0.0, 0.0, z), DVec3::new(5.0, 0.0, z), DVec3::new(5.0, 5.0, z), DVec3::new(0.0, 5.0, z)]).unwrap()).unwrap();
	Shape::from(&Face::from_wires(&wire, &[], 1e-7).unwrap())
}
fn check(result: &OperationResult<Shape>, source: &Shape, target: &Shape) {
	assert!(result.history.complete, "{:?}", result.history.relations);
	assert_eq!(result.history.operand_count, 2);
	assert!(result.value.is_valid().unwrap());
	for relation in &result.history.relations {
		assert_ne!(relation.relation, RelationKind::Unknown);
		let input = if relation.source.operand == 0 { source } else { target };
		assert_eq!(input.topology_at(relation.source.index).unwrap().kind(), relation.source.kind);
		if let Some(output) = relation.target {
			assert_eq!(result.value.topology_at(output.index).unwrap().kind(), output.kind);
		}
	}
	let mut covered = std::collections::HashSet::new();
	for r in &result.history.relations {
		if r.source.operand == 0 && r.source.kind == ShapeKind::Edge && r.target.is_some_and(|t| t.kind == ShapeKind::Edge) {
			let output = r.target.unwrap();
			covered.insert(output);
			assert!(result.history.relations.iter().any(|other| other.source.operand == 1 && other.source.kind == ShapeKind::Face && other.target == Some(output)));
		}
	}
	assert_eq!(covered.len(), result.value.edges().unwrap().len());
	let copied = result.value.copied().unwrap();
	assert!(result.history.compose(&copied.history, 0).complete);
}
#[test]
fn clipping_tracks_edges_boundary_intersections_and_endpoint_removal() {
	let source = Shape::from(&Edge::line(DVec3::new(-1.0, 2.0, 3.0), DVec3::new(6.0, 2.0, 3.0)).unwrap());
	let target = square(0.0);
	let result = source.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap();
	check(&result, &source, &target);
	let endpoints: Vec<_> = result.history.relations.iter().filter(|r| r.source.operand == 0 && r.source.kind == ShapeKind::Vertex).collect();
	assert_eq!(endpoints.len(), 2);
	assert!(endpoints.iter().all(|r| r.relation == RelationKind::Removed));
	assert!(result.history.relations.iter().any(|r| r.source.operand == 1 && r.source.kind == ShapeKind::Edge && r.target.is_some_and(|t| t.kind == ShapeKind::Vertex)));
	let full = source.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions { untrimmed: true, ..Default::default() }).unwrap();
	check(&full, &source, &target);
	assert!(full.history.relations.iter().filter(|r| r.source.operand == 0 && r.source.kind == ShapeKind::Vertex).all(|r| r.target.is_some()));
	let miss = source.translate(DVec3::Y * 10.0).unwrap();
	let empty = miss.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap();
	check(&empty, &miss, &target);
	assert!(empty.value.is_empty().unwrap());
	assert!(empty.history.relations.iter().all(|r| r.relation == RelationKind::Removed));
}
#[test]
fn joined_wire_history_keeps_distinct_source_edges_and_shared_vertices() {
	let edges = [Edge::line(DVec3::new(-1.0, 1.0, 2.0), DVec3::new(2.0, 1.0, 2.0)).unwrap(), Edge::line(DVec3::new(2.0, 1.0, 2.0), DVec3::new(2.0, 6.0, 2.0)).unwrap()];
	let source: Shape = Wire::new(&edges).unwrap().into();
	let target = square(0.0);
	let result = source.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap();
	check(&result, &source, &target);
	assert_eq!(result.value.wires().unwrap().len(), 1);
	assert_eq!(result.value.edges().unwrap().len(), 2);
	assert_eq!(result.value.vertices().unwrap().len(), 3);
	let source_edges: std::collections::HashSet<_> = result.history.relations.iter().filter(|r| r.source.operand == 0 && r.source.kind == ShapeKind::Edge && r.target.is_some_and(|t| t.kind == ShapeKind::Edge)).map(|r| r.source).collect();
	assert_eq!(source_edges.len(), 2);
}
#[test]
fn curved_multi_hit_projection_keeps_each_branch_ancestry() {
	let source: Shape = Wire::new([&Edge::ellipse(Frame { origin: DVec3::new(-5.0, 0.0, 5.0), axis_z: DVec3::X, axis_x: DVec3::Y }, 0.5, 0.5, None).unwrap()]).unwrap().into();
	let target = Shape::from(&Solid::cylinder(2.0, DVec3::Z * 10.0));
	let result = source.project_onto(&target, Projection::Directional(DVec3::X), ProjectionOptions::default()).unwrap();
	check(&result, &source, &target);
	assert_eq!(result.value.wires().unwrap().len(), 2);
	let vertices: std::collections::HashSet<_> = result.history.relations.iter().filter(|r| r.source.operand == 0 && r.source.kind == ShapeKind::Vertex && r.target.is_some_and(|t| t.kind == ShapeKind::Vertex)).map(|r| r.target.unwrap()).collect();
	assert!(vertices.len() >= 2);
}
#[test]
fn central_projection_and_multiple_or_coincident_target_faces_keep_ancestry() {
	let source: Shape = Wire::new([&Edge::ellipse(Frame { origin: DVec3::new(2.5, 2.5, 2.0), ..Default::default() }, 0.5, 0.5, None).unwrap()]).unwrap().into();
	let plane = square(0.0);
	let central = source.project_onto(&plane, Projection::FromPoint(DVec3::new(2.5, 2.5, 5.0)), ProjectionOptions::default()).unwrap();
	check(&central, &source, &plane);
	for other in [square(-2.0), plane.copied().unwrap().value] {
		let target: Shape = Shape::compound([&plane, &other]).unwrap().into();
		let result = source.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap();
		check(&result, &source, &target);
		let face_sources: std::collections::HashSet<_> = result.history.relations.iter().filter(|r| r.source.operand == 1 && r.source.kind == ShapeKind::Face && r.target.is_some_and(|t| t.kind == ShapeKind::Edge)).map(|r| r.source).collect();
		assert_eq!(face_sources.len(), 2);
	}
}

#[test]
fn hole_clipping_keeps_one_to_many_edge_descendants() {
	let outer = Wire::new([&Edge::circle(3.0, DVec3::Z).unwrap()]).unwrap();
	let hole = Wire::new([&Edge::circle(1.0, DVec3::Z).unwrap()]).unwrap();
	let target = Shape::from(&Face::from_wires(&outer, &[&hole], 1e-7).unwrap());
	let source = Shape::from(&Edge::line(DVec3::new(-4.0, 0.0, 2.0), DVec3::new(4.0, 0.0, 2.0)).unwrap());
	let result = source.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap();
	check(&result, &source, &target);
	assert_eq!(result.value.edges().unwrap().len(), 2);
	assert_eq!(result.value.wires().unwrap().len(), 2);
	let descendants: std::collections::HashSet<_> = result.history.relations.iter().filter(|r| r.source.operand == 0 && r.source.kind == ShapeKind::Edge && r.target.is_some_and(|t| t.kind == ShapeKind::Edge)).map(|r| r.target.unwrap()).collect();
	assert_eq!(descendants.len(), 2);
	let length: f64 = result.value.edges().unwrap().iter().map(|edge| edge.length(1e-8).unwrap()).sum();
	assert!((length - 4.0).abs() < 1e-8);
}

#[test]
fn already_on_target_and_reversed_wire_references_resolve() {
	let target = square(0.0);
	let source = Shape::from(&target.edges().unwrap().remove(0));
	let result = source.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap();
	check(&result, &source, &target);
	assert_eq!(result.value.edges().unwrap().len(), 1);
	let circle: Shape = Wire::new([&Edge::ellipse(Frame { origin: DVec3::new(2.5, 2.5, 2.0), ..Default::default() }, 0.5, 0.5, None).unwrap()]).unwrap().into();
	let reversed = circle.reversed().unwrap().value;
	let result = reversed.project_onto(&target, Projection::Directional(-DVec3::Z), ProjectionOptions::default()).unwrap();
	check(&result, &reversed, &target);
	assert_eq!(result.value.wires().unwrap().len(), 1);
}
