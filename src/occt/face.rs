use super::edge::Edge;
use super::ffi;
use crate::traits::FaceStruct;
use glam::DVec3;
use std::sync::OnceLock;

/// A face topology shape.
///
/// `edges` is a lazy `OnceLock` cache populated on first `iter_edge` call,
/// matching the pattern used by `Solid`. Faces yielded from `Solid::iter_face`
/// are constructed fresh each time the parent solid's face cache is built, so
/// the OnceLock matches the lifetime of the enclosing `Vec<Face>`.
pub struct Face {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Face>,
	edges: OnceLock<Vec<Edge>>,
}

impl Face {
	/// Create a Face wrapping a `TopoDS_Face`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Face>) -> Self {
		Face { inner, edges: OnceLock::new() }
	}
}

impl FaceStruct for Face {
	type Edge = Edge;

	fn id(&self) -> u64 {
		ffi::face_tshape_id(&self.inner)
	}

	fn project(&self, p: DVec3) -> (DVec3, DVec3) {
		let (mut cpx, mut cpy, mut cpz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut nx, mut ny, mut nz) = (0.0_f64, 0.0_f64, 0.0_f64);
		// FFI returns false only on truly catastrophic OCCT failure; for a
		// well-formed face this is effectively unreachable.
		assert!(
			ffi::face_project_point(&self.inner, p.x, p.y, p.z, &mut cpx, &mut cpy, &mut cpz, &mut nx, &mut ny, &mut nz),
			"Face::project: BRepExtrema_ExtPF failed (this is a bug)"
		);
		(DVec3::new(cpx, cpy, cpz), DVec3::new(nx, ny, nz))
	}

	fn iter_edge(&self) -> impl Iterator<Item = &Edge> + '_ {
		self.edges
			.get_or_init(|| {
				ffi::face_edges(&self.inner)
					.iter()
					.map(|e_ref| {
						let owned = ffi::clone_edge_handle(e_ref);
						Edge::try_from_ffi(owned, "face_edges: null".into()).expect("face_edges: unexpected null (this is a bug)")
					})
					.collect()
			})
			.iter()
	}
}
