use cadrum::{DVec3, MeshImportOptions, MeshOutcome, MeshRepair, ShapeKind, Solid, TriangleMesh};
fn tetra() -> TriangleMesh {
	TriangleMesh { positions: vec![DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z], triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]] }
}
fn options(outcome: MeshOutcome) -> MeshImportOptions {
	MeshImportOptions { outcome, ..Default::default() }
}
#[test]
fn native_faceted_solids_support_booleans_and_step() {
	let result = tetra().import(options(MeshOutcome::ClosedSolid)).unwrap();
	let solid = result.shape.unwrap().as_solid().unwrap();
	assert!((solid.volume() - 1.0 / 6.0).abs() < 1e-10);
	assert_eq!(result.report.components, 1);
	assert!(result.report.boundary_edges.is_empty());
	let clip = Solid::cube(DVec3::ZERO, DVec3::splat(0.2));
	let cut = (&solid - &clip).build_vec().unwrap();
	assert_eq!(cut.len(), 1);
	assert!(cut[0].volume() < solid.volume());
	let mut step = Vec::new();
	Solid::write_step([&solid], &mut step).unwrap();
	assert_eq!(Solid::read_step(&mut step.as_slice()).unwrap().len(), 1);
	let mut multi = tetra();
	multi.positions.extend(tetra().positions.into_iter().map(|p| p + DVec3::splat(10.0)));
	multi.triangles.extend(tetra().triangles.into_iter().map(|t| t.map(|v| v + 4)));
	let result = multi.import(options(MeshOutcome::ClosedSolid)).unwrap();
	assert_eq!(result.report.components, 2);
	assert_eq!(result.shape.unwrap().solids().unwrap().len(), 2);
}
#[test]
fn open_nonmanifold_and_explicit_repairs() {
	let mut open = tetra();
	open.triangles.pop();
	let result = open.import(options(MeshOutcome::OpenShell)).unwrap();
	assert_eq!(result.shape.unwrap().kind(), ShapeKind::Shell);
	assert_eq!(result.report.boundary_edges.len(), 3);
	assert!(open.import(options(MeshOutcome::ClosedSolid)).is_err());
	let mut bad = tetra();
	bad.triangles.push([0, 2, 1]);
	bad.triangles.push([0, 0, 1]);
	assert!(bad.import(options(MeshOutcome::ClosedSolid)).is_err());
	let result = bad.import(MeshImportOptions { outcome: MeshOutcome::ClosedSolid, repair: MeshRepair { remove_degenerate: true, remove_duplicates: true, ..Default::default() }, ..Default::default() }).unwrap();
	assert_eq!(result.report.removed_degenerate, 1);
	assert_eq!(result.report.removed_duplicates, 1);
	let mut reversed = tetra();
	for t in &mut reversed.triangles {
		t.swap(1, 2);
	}
	assert!(reversed.import(options(MeshOutcome::ClosedSolid)).is_err());
	let fixed = reversed.import(MeshImportOptions { outcome: MeshOutcome::ClosedSolid, repair: MeshRepair { orient_components: true, ..Default::default() }, ..Default::default() }).unwrap();
	assert_eq!(fixed.report.flipped_triangles, 4);
	assert!(fixed.shape.unwrap().as_solid().unwrap().volume() > 0.0);
	let mut nonmanifold = tetra();
	nonmanifold.positions.push(DVec3::splat(3.0));
	nonmanifold.triangles.push([0, 1, 4]);
	assert!(!nonmanifold.import(options(MeshOutcome::MeshOnly)).unwrap().report.non_manifold_edges.is_empty());
	assert!(nonmanifold.import(options(MeshOutcome::OpenShell)).is_err());
	let mut invalid = tetra();
	invalid.triangles[0][0] = 999;
	assert!(invalid.import(options(MeshOutcome::MeshOnly)).is_err());
}
#[test]
fn stl_scales_and_rejects_truncation() {
	let ascii = b"solid test\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid test\n";
	let read = TriangleMesh::read_stl(&mut ascii.as_slice(), 2.0, options(MeshOutcome::OpenShell)).unwrap();
	assert!((read.shape.unwrap().faces().unwrap()[0].area() - 2.0).abs() < 1e-10);
	let mut binary = vec![0u8; 80];
	binary.extend(1u32.to_le_bytes());
	for value in [0f32, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
		binary.extend(value.to_le_bytes());
	}
	binary.extend(0u16.to_le_bytes());
	assert_eq!(TriangleMesh::read_stl(&mut binary.as_slice(), 1.0, MeshImportOptions::default()).unwrap().mesh.triangles.len(), 1);
	binary.pop();
	assert!(TriangleMesh::read_stl(&mut binary.as_slice(), 1.0, MeshImportOptions::default()).is_err());
	assert!(TriangleMesh::read_stl(&mut ascii.as_slice(), 1.0, MeshImportOptions { max_input_bytes: 10, ..Default::default() }).is_err());
}
#[test]
fn welding_is_an_explicit_repair() {
	let original = tetra();
	let mut soup = TriangleMesh { positions: Vec::new(), triangles: Vec::new() };
	for t in original.triangles {
		let base = soup.positions.len();
		soup.positions.extend(t.map(|v| original.positions[v]));
		soup.triangles.push([base, base + 1, base + 2]);
	}
	assert!(soup.import(options(MeshOutcome::ClosedSolid)).is_err());
	let result = soup.import(MeshImportOptions { outcome: MeshOutcome::ClosedSolid, repair: MeshRepair { weld: true, ..Default::default() }, ..Default::default() }).unwrap();
	assert_eq!(result.report.welded_vertices, 8);
	assert_eq!(result.mesh.positions.len(), 4);
}
