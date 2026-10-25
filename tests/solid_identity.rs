//! Sub-shape identity: an id is unique within its solid, tells a face apart from the
//! same face reused at another placement, and survives moves and shallow clones.

use cadrum::{DVec3, Edge, Solid, Tessellation};
use std::collections::HashSet;

fn face_ids(s: &Solid) -> Vec<u64> {
	s.iter_face().map(|f| f.id()).collect()
}

fn edge_ids(s: &Solid) -> Vec<u64> {
	s.iter_edge().map(|e| e.id()).collect()
}

fn square_prism() -> Solid {
	let profile = Edge::polygon(&[DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 0.0), DVec3::new(0.0, 10.0, 0.0)]).unwrap();
	Solid::extrude(&profile, DVec3::Z * 10.0).unwrap()
}

#[test]
fn prism_caps_have_distinct_ids() {
	// MakePrism builds the top as the bottom moved, so a TShape-only id would repeat.
	let prism = square_prism();
	let ids = face_ids(&prism);
	assert_eq!(ids.len(), 6);
	assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 6, "every face has its own id: {ids:?}");
	let edges = edge_ids(&prism);
	assert_eq!(edges.len(), 12);
	assert_eq!(edges.iter().collect::<HashSet<_>>().len(), 12, "every edge has its own id: {edges:?}");
}

#[test]
fn moves_and_clones_keep_ids() {
	let prism = square_prism();
	let (faces, edges) = (face_ids(&prism), edge_ids(&prism));
	let moved = prism.clone().translate(DVec3::new(5.0, -3.0, 20.0)).rotate_z(0.7).unwrap();
	assert_eq!(face_ids(&moved), faces, "translate/rotate keep face ids, the top included");
	assert_eq!(edge_ids(&moved), edges, "translate/rotate keep edge ids");
	assert_eq!(moved.id(), prism.id());
	let copy = moved.clone();
	assert_eq!(face_ids(&copy), faces, "Clone is a shallow handle copy");
	assert!((copy.volume() - 1000.0).abs() < 1e-6);
}

#[test]
fn face_edges_carry_the_solid_edge_ids() {
	let prism = square_prism().translate(DVec3::X * 100.0);
	let all: HashSet<u64> = edge_ids(&prism).into_iter().collect();
	for face in prism.iter_face() {
		let ids: Vec<u64> = face.iter_edge().map(|e| e.id()).collect();
		assert_eq!(ids.len(), 4);
		assert!(ids.iter().all(|id| all.contains(id)), "face edges {ids:?} not all in {all:?}");
	}
	// Each edge of the prism is shared by exactly two faces, the moved top edges included.
	for id in &all {
		let owners = prism.iter_face().filter(|f| f.iter_edge().any(|e| e.id() == *id)).count();
		assert_eq!(owners, 2, "edge {id} owners");
	}
}

#[test]
fn mesh_face_ids_tell_the_caps_apart() {
	let prism = square_prism();
	let mesh = Solid::mesh([&prism], Tessellation::default()).unwrap();
	let seen: HashSet<u64> = mesh.face_ids.iter().copied().collect();
	assert_eq!(seen, face_ids(&prism).into_iter().collect::<HashSet<_>>());
}

#[test]
fn standalone_edges_keep_their_id_when_moved() {
	let e = Edge::line(DVec3::ZERO, DVec3::X).unwrap();
	let id = e.id();
	assert_eq!(e.clone().translate(DVec3::Y).rotate_x(1.0).unwrap().id(), id);
	assert_eq!(e.clone().id(), id);
}
