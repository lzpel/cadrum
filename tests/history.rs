use cadrum::{DVec3, History, RelationKind, Shape, ShapeKind, Solid, TopologyRef, TopologyRelation};

#[test]
fn native_copies_compose_and_resolve_styled_topology() {
	let original = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE).color("#ff0000"));
	let first = original.copied().unwrap();
	let second = first.value.copied().unwrap();
	let history = first.history.compose(&second.history, 0);
	assert!(history.complete);
	assert_eq!(history.operand_count, 1);
	assert_eq!(history.relations.len(), 26);
	for relation in &history.relations {
		let target = relation.target.unwrap();
		assert_eq!(original.topology_at(relation.source.index).unwrap().kind(), relation.source.kind);
		let resolved = second.value.topology_at(target.index).unwrap();
		assert_eq!(resolved.kind(), target.kind);
		if target.kind == ShapeKind::Face {
			let mesh = resolved.mesh(Default::default()).unwrap();
			assert!(!mesh.indices.is_empty());
			assert!(mesh.face_ids.iter().all(|id| mesh.colormap.contains_key(id)));
		}
	}
	assert!(second.value.topology_at(0).is_err());
	assert!(second.value.topology_at(usize::MAX).is_err());
}

#[test]
fn composition_distinguishes_removed_descendants_and_unknown_correspondence() {
	let ancestor = TopologyRef { operand: 0, index: 1, kind: ShapeKind::Face };
	let left = TopologyRef { operand: 0, index: 2, kind: ShapeKind::Face };
	let right = TopologyRef { operand: 0, index: 3, kind: ShapeKind::Face };
	let first = History {
		operand_count: 1,
		complete: true,
		relations: vec![TopologyRelation { source: ancestor, target: Some(left), relation: RelationKind::Modified }, TopologyRelation { source: ancestor, target: Some(right), relation: RelationKind::Modified }],
	};
	let mut next = History {
		operand_count: 1,
		complete: true,
		relations: vec![TopologyRelation { source: left, target: None, relation: RelationKind::Removed }, TopologyRelation { source: right, target: Some(right), relation: RelationKind::Unchanged }],
	};
	let retained = first.compose(&next, 0);
	assert!(retained.complete);
	assert!(retained.relations.iter().any(|r| r.source == ancestor && r.target == Some(right)));
	assert!(!retained.relations.iter().any(|r| r.relation == RelationKind::Removed));
	next.relations.pop();
	let unknown = first.compose(&next, 0);
	assert!(!unknown.complete);
	assert!(unknown.relations.iter().any(|r| r.source == ancestor && r.relation == RelationKind::Unknown));
	assert!(!unknown.relations.iter().any(|r| r.relation == RelationKind::Removed));
	next.relations.push(TopologyRelation { source: right, target: None, relation: RelationKind::Removed });
	let removed = first.compose(&next, 0);
	assert!(removed.complete);
	assert_eq!(removed.relations.len(), 1);
	assert_eq!(removed.relations[0].relation, RelationKind::Removed);
}
