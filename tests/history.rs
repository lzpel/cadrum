use cadrum::{BooleanOp, DVec3, RelationKind, Shape, ShapeKind, ShapeTransform, Solid};
#[test]
fn copy_transform_and_composed_history_resolve_topology() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let copied = shape.copied().unwrap();
	assert!(copied.history.complete);
	assert_eq!(copied.history.operand_count, 1);
	assert_eq!(copied.history.relations.len(), 26);
	let mirrored = copied.value.transform_with_history(ShapeTransform::Mirror { origin: DVec3::ZERO, normal: DVec3::new(1.0, 1.0, 0.0) }).unwrap();
	for r in &mirrored.history.relations {
		assert_eq!(copied.value.topology_at(r.source.index).unwrap().kind(), r.source.kind);
		if let Some(target) = r.target {
			assert_eq!(mirrored.value.topology_at(target.index).unwrap().kind(), target.kind);
		}
	}
	let composed = copied.history.compose(&mirrored.history, 0);
	assert!(composed.complete);
	assert_eq!(composed.operand_count, 1);
	assert_eq!(composed.relations.len(), 26);
	assert!(composed.relations.iter().all(|r| r.target.is_some()));
}
#[test]
fn boolean_ancestry_and_confirmed_deletion_are_distinct() {
	let a = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(2.0)));
	let b = Shape::from(&Solid::cube(DVec3::X, DVec3::new(3.0, 2.0, 2.0)));
	let union = a.boolean(BooleanOp::Union, &[&b]).unwrap();
	assert!(union.history.relations.iter().any(|r| r.source.operand == 0));
	assert!(union.history.relations.iter().any(|r| r.source.operand == 1));
	assert!((union.value.solids().unwrap().iter().map(Solid::volume).sum::<f64>() - 12.0).abs() < 1e-8);
	let unified = union.value.unify_faces(1e-7, 1e-7).unwrap();
	assert_eq!(unified.value.faces().unwrap().len(), 6);
	assert!(unified.history.relations.iter().any(|r| r.relation == RelationKind::Modified));
	let empty = a.boolean(BooleanOp::Difference, &[&a]).unwrap();
	assert!(empty.value.is_empty().unwrap());
	assert!(empty.history.relations.iter().any(|r| r.relation == RelationKind::Removed));
	assert!(!empty.history.relations.iter().any(|r| r.target.is_some()));
}
#[test]
fn blend_and_sewing_history_include_generated_and_retained_topology() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(5.0)));
	let edges = shape.edges().unwrap();
	let blend = shape.fillet_edges(0.2, &[&edges[0]]).unwrap();
	assert!(blend.history.relations.iter().any(|r| r.relation == RelationKind::Generated && r.source.kind == ShapeKind::Edge && r.target.is_some_and(|v| v.kind == ShapeKind::Face)));
	let faces: Vec<Shape> = shape.faces().unwrap().iter().map(Shape::from).collect();
	let refs: Vec<_> = faces.iter().collect();
	let sewn = Shape::sew_with_history(&refs, 1e-7).unwrap();
	assert_eq!(sewn.value.faces().unwrap().len(), 6);
	assert!(sewn.history.relations.iter().any(|r| r.relation == RelationKind::Unchanged));
}

#[test]
fn deleting_one_split_descendant_does_not_delete_its_ancestor() {
	let original = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(10.0)));
	let middle = Shape::from(&Solid::cube(DVec3::new(4.0, -1.0, -1.0), DVec3::new(6.0, 11.0, 11.0)));
	let split = original.boolean(BooleanOp::Difference, &[&middle]).unwrap();
	let left = Shape::from(&Solid::cube(DVec3::splat(-1.0), DVec3::new(5.0, 11.0, 11.0)));
	let retained = split.value.boolean(BooleanOp::Difference, &[&left]).unwrap();
	let mut descendants = std::collections::HashMap::new();
	for relation in &split.history.relations {
		if relation.source.operand == 0 && relation.source.kind == ShapeKind::Face && relation.target.is_some() {
			descendants.entry(relation.source).or_insert_with(Vec::new).push(relation.target.unwrap());
		}
	}
	let ancestor = *descendants.iter().find(|(_, targets)| targets.len() > 1).unwrap().0;
	let composed = split.history.compose(&retained.history, 0);
	assert!(composed.relations.iter().any(|r| r.source == ancestor && r.target.is_some()));
	assert!(!composed.relations.iter().any(|r| r.source == ancestor && r.relation == RelationKind::Removed));
}

#[test]
fn profile_history_composes_through_extrusion() {
	let circle = cadrum::Wire::new([&cadrum::Edge::circle(1.0, DVec3::Z).unwrap()]).unwrap();
	let profile = Shape::profile(&[&circle], 1e-7).unwrap();
	assert!(profile.history.complete);
	assert!(profile.history.relations.iter().any(|r| r.source.kind == ShapeKind::Edge && r.target.is_some_and(|t| t.kind == ShapeKind::Face)));
	assert_eq!(profile.history.operand_count, 1);
	let prism = profile.value.extrude(DVec3::Z).unwrap();
	let composed = profile.history.compose(&prism.history, 0);
	assert_eq!(composed.operand_count, 1);
	assert!(composed.complete);
}

#[test]
fn resolving_history_topology_retains_face_styles() {
	let shape = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE).color("#ff0000"));
	let copied = shape.copied().unwrap();
	let target = copied.history.relations.iter().find_map(|r| r.target.filter(|t| t.kind == ShapeKind::Face)).unwrap();
	let face = copied.value.topology_at(target.index).unwrap();
	let mesh = face.mesh(Default::default()).unwrap();
	assert!(!mesh.face_ids.is_empty());
	assert!(mesh.face_ids.iter().all(|id| mesh.colormap.contains_key(id)));
}
