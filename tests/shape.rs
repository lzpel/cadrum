use cadrum::{Compound, DVec3, Edge, Error, Shape, ShapeKind, Shell, Solid, Tessellation, Vertex, Wire};

fn round_trip(shape: &Shape) -> Shape {
	let mut bytes = Vec::new();
	shape.write_brep(&mut bytes).unwrap();
	let restored = Shape::read_brep(&mut bytes.as_slice()).unwrap();
	assert_eq!(restored.kind(), shape.kind());
	assert!(restored.is_valid().unwrap());
	assert_eq!(restored.faces().unwrap().len(), shape.faces().unwrap().len());
	restored
}

#[test]
fn open_box_native_boundaries() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let shape = Shape::from(&cube);
	let faces = shape.faces().unwrap();
	let open = shape.remove_faces([&faces[0]], 1e-6).unwrap();
	assert_eq!(open.kind(), ShapeKind::Shell);
	assert!(open.is_valid().unwrap());
	assert!(!open.is_closed().unwrap());
	assert!(!open.is_empty().unwrap());
	assert!(matches!(open.to_solid(), Err(Error::Shape(_))));
	assert!(open.as_solid().is_err());
	let mesh = open.mesh(Tessellation::default()).unwrap();
	assert!(!mesh.indices.is_empty());
	assert_eq!(mesh.face_ids.len(), mesh.indices.len() / 3);
	assert_eq!(mesh.normals.len(), mesh.vertices.len());
	assert!(!round_trip(&open).is_closed().unwrap());
}

#[test]
fn disconnected_sewing_retains_shells_and_isolated_faces() {
	let a = Solid::cube(DVec3::ZERO, DVec3::splat(2.0));
	let b = Solid::cube(DVec3::splat(10.0), DVec3::splat(12.0));
	let c = Solid::cube(DVec3::splat(20.0), DVec3::splat(22.0));
	let sewn = Shape::sew(a.iter_face().take(5).chain(b.iter_face()).chain(c.iter_face().take(1)), 1e-6).unwrap();
	assert_eq!(sewn.kind(), ShapeKind::Compound);
	assert_eq!(sewn.faces().unwrap().len(), 12);
	assert_eq!(sewn.shells().unwrap().len(), 2);
	assert!(sewn.to_solid().is_err());
	let restored = round_trip(&sewn);
	assert_eq!(restored.shells().unwrap().len(), 2);
	assert_eq!(restored.mesh(Tessellation::default()).unwrap().face_ids.iter().collect::<std::collections::HashSet<_>>().len(), 12);
}

#[test]
fn closed_shell_solid_conversion_and_typed_traversal() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::new(2.0, 3.0, 4.0));
	let shape = Shape::from(&cube);
	assert_eq!(shape.vertices().unwrap().len(), 8);
	assert_eq!(shape.edges().unwrap().len(), 12);
	assert_eq!(shape.wires().unwrap().len(), 6);
	assert_eq!(shape.faces().unwrap().len(), 6);
	assert_eq!(shape.shells().unwrap().len(), 1);
	assert_eq!(shape.solids().unwrap().len(), 1);
	let shell = Shape::sew(cube.iter_face(), 1e-6).unwrap();
	assert_eq!(shell.kind(), ShapeKind::Shell);
	assert!(shell.is_closed().unwrap());
	assert!((shell.to_solid().unwrap().volume() - 24.0).abs() < 1e-8);
	let built = Shell::new(cube.iter_face()).unwrap();
	assert!((built.to_solid().unwrap().volume() - 24.0).abs() < 1e-8);
	assert!(Shell::try_from(shape).is_err());
}

#[test]
fn construct_and_round_trip_all_categories() {
	let vertex = Vertex::new(DVec3::new(1.0, 2.0, 3.0)).unwrap();
	assert_eq!(vertex.point().unwrap(), DVec3::new(1.0, 2.0, 3.0));
	assert_eq!(round_trip(&vertex).vertices().unwrap().len(), 1);
	let edge = Edge::line(DVec3::ZERO, DVec3::X).unwrap();
	let wire = Wire::new([&edge]).unwrap();
	assert!(!wire.is_closed().unwrap());
	assert_eq!(round_trip(&wire).edges().unwrap().len(), 1);
	let edge = Shape::from(&edge);
	assert_eq!(round_trip(&edge).kind(), ShapeKind::Edge);
	assert!(edge.mesh(Tessellation::default()).unwrap().indices.is_empty());
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let shape = Shape::from(&cube);
	let face = Shape::from(&shape.faces().unwrap()[0]);
	assert!(round_trip(&face).mesh(Tessellation::default()).unwrap().indices.len() >= 6);
	let shell = Shape::sew(cube.iter_face(), 1e-6).unwrap();
	round_trip(&shell);
	round_trip(&shape);
	let compound = Shape::compound([&edge, &face, &shape]).unwrap();
	let restored = round_trip(&compound);
	assert_eq!(restored.solids().unwrap().len(), 1);
	assert_eq!(restored.compounds().unwrap().len(), 1);
	let _: Compound = restored.try_into().unwrap();
}

#[test]
fn extraction_empty_and_errors_are_distinct() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let shape = Shape::from(&cube);
	let faces = shape.faces().unwrap();
	assert_eq!(shape.remove_faces([], 1e-6).unwrap().kind(), ShapeKind::Solid);
	let single = shape.extract_faces([&faces[0]], 1e-6).unwrap();
	assert_eq!(single.kind(), ShapeKind::Face);
	assert_eq!(single.faces().unwrap().len(), 1);
	let empty = shape.remove_faces(&faces, 1e-6).unwrap();
	assert!(empty.is_empty().unwrap());
	assert!(empty.is_valid().unwrap());
	assert!(empty.bounding_box().unwrap().is_none());
	assert!(round_trip(&empty).is_empty().unwrap());
	assert!(empty.mesh(Tessellation::default()).unwrap().indices.is_empty());
	assert!(Shape::sew([], 1e-6).unwrap().is_empty().unwrap());
	assert!(Shape::compound([]).unwrap().is_empty().unwrap());
	let relocated = shape.as_solid().unwrap().translate(DVec3::splat(10.0));
	assert!(matches!(shape.extract_faces(relocated.iter_face().take(1), 1e-6), Err(Error::Validation(_))));
	assert!(matches!(shape.remove_faces(relocated.iter_face().take(1), 1e-6), Err(Error::Validation(_))));
	let foreign = Solid::sphere(1.0);
	assert!(matches!(shape.remove_faces(foreign.iter_face(), 1e-6), Err(Error::Validation(_))));
	assert!(Shape::sew(&faces, f64::NAN).is_err());
	assert!(Vertex::new(DVec3::INFINITY).is_err());
	assert!(Wire::new([]).is_err());
	assert!(Shape::read_brep(&mut b"garbage".as_slice()).is_err());
}

#[test]
fn owning_views_styles_and_independent_copies() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE).color("#ff0000");
	let shape = Shape::from(&cube);
	let faces = shape.faces().unwrap();
	let open = shape.remove_faces([&faces[0]], 1e-6).unwrap();
	let selected = shape.extract_faces([&faces[0]], 1e-6).unwrap();
	drop(shape);
	drop(cube);
	for view in [&open, &selected, &open.clone()] {
		let mesh = view.mesh(Tessellation::default()).unwrap();
		assert!(mesh.face_ids.iter().all(|id| mesh.colormap.contains_key(id)));
	}
	let copied = open.clone();
	assert!(copied.faces().unwrap().iter().all(|face| !open.faces().unwrap().iter().any(|old| old.id() == face.id())));
	let empty = Shape::compound([]).unwrap();
	assert!(empty.clone().is_empty().unwrap());
	let nested = Shape::compound([&*empty]).unwrap();
	assert!(nested.is_empty().unwrap());
	let edges = [Edge::line(DVec3::ZERO, DVec3::X).unwrap(), Edge::line(DVec3::splat(10.0), DVec3::splat(11.0)).unwrap(), Edge::line(DVec3::X, DVec3::Y).unwrap()];
	assert!(Wire::new(&edges).is_err());
}

#[test]
fn removal_preserves_loose_topology_in_mixed_compounds() {
	let cube = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let wire = Wire::new([&Edge::line(DVec3::splat(10.0), DVec3::splat(11.0)).unwrap()]).unwrap();
	let vertex = Vertex::new(DVec3::splat(20.0)).unwrap();
	let mixed = Shape::compound([&cube, &wire, &vertex]).unwrap();
	let faces = mixed.faces().unwrap();
	let opened = mixed.remove_faces([&faces[0]], 1e-6).unwrap();
	assert_eq!(opened.kind(), ShapeKind::Compound);
	assert_eq!(opened.faces().unwrap().len(), 5);
	assert_eq!(opened.edges().unwrap().len(), 13);
	let only_loose = mixed.remove_faces(&faces, 1e-6).unwrap();
	assert_eq!(only_loose.edges().unwrap().len(), 1);
	assert_eq!(only_loose.vertices().unwrap().len(), 3);
	assert_eq!(only_loose.wires().unwrap().len(), 1);
	assert!(only_loose.is_valid().unwrap());
}
