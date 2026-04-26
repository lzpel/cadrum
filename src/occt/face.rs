use super::ffi;
use crate::traits::FaceStruct;
use glam::DVec3;

/// A face topology shape.
pub struct Face {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Face>,
}

impl Face {
	/// Create a Face wrapping a `TopoDS_Face`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Face>) -> Self {
		Face { inner }
	}
}

impl FaceStruct for Face {
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
}
