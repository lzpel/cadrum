use cadrum::{DVec3, OffsetOptions, PartitionSide, Plane, SectionOptions, Shape, Solid};
#[test]
fn oblique_curved_section_and_plane_partition() {
	let sphere = Shape::from(&Solid::sphere(2.0));
	let plane = Plane { origin: DVec3::ZERO, normal: DVec3::new(1.0, 2.0, 3.0) };
	let tool = Shape::from(&plane.face().unwrap());
	let section = sphere.section(&tool, SectionOptions::default()).unwrap();
	assert!(!section.value.edges().unwrap().is_empty());
	for edge in section.value.edges().unwrap() {
		let [a, b] = edge.parameter_range().unwrap();
		for t in [a, (a + b) * 0.5, b] {
			let p = edge.evaluate(t).unwrap().point;
			assert!(p.dot(plane.normal).abs() < 1e-6);
			assert!((p.length() - 2.0).abs() < 1e-6);
		}
	}
	let split = sphere.split_by_plane(plane, 1e-7).unwrap();
	assert_eq!(split.partitions.len(), 2);
	assert!(split.partitions.iter().any(|p| p.side == PartitionSide::Positive));
	assert!(split.partitions.iter().any(|p| p.side == PartitionSide::Negative));
	assert!(split.partitions.iter().all(|p| !p.ancestors.is_empty()));
	let volume: f64 = split.partitions.iter().flat_map(|p| p.shape.solids().unwrap()).map(|s| s.volume()).sum();
	assert!((volume - Solid::sphere(2.0).volume()).abs() < 1e-5);
}
#[test]
fn disjoint_and_tangent_sections_and_multi_body_split() {
	let cube = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let plane = Shape::from(&Plane { origin: DVec3::Z * 3.0, normal: DVec3::Z }.face().unwrap());
	assert!(cube.section(&plane, SectionOptions::default()).unwrap().value.is_empty().unwrap());
	let other = Shape::from(&Solid::cube(DVec3::X * 3.0, DVec3::new(4.0, 1.0, 1.0)));
	let compound = Shape::compound([&cube, &other]).unwrap();
	let split = compound.split_by_plane(Plane { origin: DVec3::Z * 0.5, normal: DVec3::Z }, 1e-7).unwrap();
	assert_eq!(split.result.value.solids().unwrap().len(), 4);
	let sphere = Shape::from(&Solid::sphere(1.0));
	let tangent = Shape::from(&Plane { origin: DVec3::Z, normal: DVec3::Z }.face().unwrap());
	assert!(sphere.section(&tangent, SectionOptions::default()).is_ok());
}
#[test]
fn open_surface_offsets_and_configurable_thickening() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let shell = Shape::sew(cube.iter_face().take(5), 1e-7).unwrap();
	let result = shell.thicken_with_options(0.5, OffsetOptions::default()).unwrap();
	assert!(result.value.as_solid().unwrap().volume() > 0.0);
	assert!(!result.history.relations.is_empty());
	assert!(shell.thicken_with_options(-6.0, OffsetOptions::default()).is_err());
	let cylinder = Solid::cylinder(2.0, DVec3::Z * 5.0);
	let face = cylinder.iter_face().find(|f| matches!(f.surface().map(|s| s.kind), Some(cadrum::SurfaceKind::Cylinder { .. }))).unwrap();
	let offset = Shape::from(face).offset_surface(0.5, OffsetOptions::default()).unwrap();
	assert!(offset.value.is_valid().unwrap());
	assert!(!offset.value.faces().unwrap().is_empty());
}

#[test]
fn plane_missing_a_shape_preserves_partition_provenance() {
	let cube = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let split = cube.split_by_plane(Plane { origin: DVec3::Z * 3.0, normal: DVec3::Z }, 1e-7).unwrap();
	assert_eq!(split.partitions.len(), 1);
	assert_eq!(split.partitions[0].side, PartitionSide::Negative);
	assert!(!split.partitions[0].ancestors.is_empty());
}
