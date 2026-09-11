use super::edge::Edge;
use super::ffi;
use crate::common::surface::{Surface, SurfaceKind};
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

impl std::fmt::Debug for Face {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Face({})", self.id())
	}
}

impl FaceStruct for Face {
	type Edge = Edge;

	fn id(&self) -> u64 {
		ffi::face_tshape_id(&self.inner)
	}

	fn surface(&self) -> Option<Surface> {
		let (mut ox, mut oy, mut oz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ax, mut ay, mut az) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut xx, mut xy, mut xz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut p1, mut p2) = (0.0_f64, 0.0_f64);
		let kind = match ffi::face_surface(&self.inner, &mut ox, &mut oy, &mut oz, &mut ax, &mut ay, &mut az, &mut xx, &mut xy, &mut xz, &mut p1, &mut p2) {
			1 => SurfaceKind::Plane,
			2 => SurfaceKind::Cylinder { radius: p1 },
			3 => SurfaceKind::Cone { radius: p1, semi_angle: p2 },
			4 => SurfaceKind::Sphere { radius: p1 },
			5 => SurfaceKind::Torus { major_radius: p1, minor_radius: p2 },
			_ => return None,
		};
		Some(Surface { origin: DVec3::new(ox, oy, oz), axis: DVec3::new(ax, ay, az), x_dir: DVec3::new(xx, xy, xz), kind })
	}

	fn area(&self) -> f64 {
		ffi::face_surface_area(&self.inner)
	}

	fn center(&self) -> DVec3 {
		let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::face_center_of_mass(&self.inner, &mut x, &mut y, &mut z);
		DVec3::new(x, y, z)
	}

	fn project(&self, p: DVec3) -> (DVec3, DVec3) {
		let (mut cpx, mut cpy, mut cpz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut nx, mut ny, mut nz) = (0.0_f64, 0.0_f64, 0.0_f64);
		// FFI returns false only when OCCT throws or the face is unbounded: a
		// bounded face always has a boundary to fall back on.
		assert!(ffi::face_project_point(&self.inner, p.x, p.y, p.z, &mut cpx, &mut cpy, &mut cpz, &mut nx, &mut ny, &mut nz), "Face::project: OCCT threw or the face has no boundary (this is a bug)");
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
