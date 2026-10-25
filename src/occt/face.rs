use super::edge::{wires_from_edges, Edge};
use super::ffi;
use crate::common::error::Error;
use crate::common::surface::{Surface, SurfaceKind};
use crate::traits::FaceStruct;
use glam::DVec3;
use std::sync::OnceLock;

/// A face topology shape.
///
/// `inner` is the face where OCCT sees it. `local` is the same face placed relative
/// to its owning solid, which is what gives this face's edges the ids
/// `Solid::iter_edge` gives them (see `ffi.h`). A standalone face is its own owner.
pub struct Face {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Face>,
	local: cxx::UniquePtr<ffi::TopoDS_Face>,
	id: u64,
	edges: OnceLock<Vec<Edge>>,
}

impl Face {
	/// A face of a solid, with the id and placement its owner assigned.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Face>, local: cxx::UniquePtr<ffi::TopoDS_Face>, id: u64) -> Self {
		Face { inner, local, id, edges: OnceLock::new() }
	}

	/// A standalone face: its own owner, so its id is its TShape address.
	pub(crate) fn standalone(inner: cxx::UniquePtr<ffi::TopoDS_Face>) -> Self {
		let (local, id) = (ffi::clone_face_handle(&inner), ffi::face_tshape_id(&inner));
		Face::new(inner, local, id)
	}
}

// A shallow handle copy sharing the immutable TShape, so the id is kept.
impl Clone for Face {
	fn clone(&self) -> Self {
		Face::new(ffi::clone_face_handle(&self.inner), ffi::clone_face_handle(&self.local), self.id)
	}
}

impl std::fmt::Debug for Face {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Face({})", self.id())
	}
}

impl FaceStruct for Face {
	type Edge = Edge;

	fn face<'a>(surface: &Surface, profile: impl IntoIterator<Item = &'a Edge>) -> Result<Self, Error> {
		let wires = wires_from_edges(profile).map_err(|e| Error::Validation(format!("face: {}", e.what())))?;
		let (kind, p1, p2) = match surface.kind {
			SurfaceKind::Plane => (1, 0.0, 0.0),
			SurfaceKind::Cylinder { radius } => (2, radius, 0.0),
			SurfaceKind::Cone { radius, semi_angle } => (3, radius, semi_angle),
			SurfaceKind::Sphere { radius } => (4, radius, 0.0),
			SurfaceKind::Torus { major_radius, minor_radius } => (5, major_radius, minor_radius),
		};
		let (o, z, x) = (surface.origin, surface.axis_z, surface.axis_x);
		let inner = ffi::make_face(kind, o.x, o.y, o.z, z.x, z.y, z.z, x.x, x.y, x.z, p1, p2, &wires).map_err(|e| Error::Validation(format!("face: surface={surface:?}: {}", e.what())))?;
		Ok(Face::standalone(inner))
	}

	fn id(&self) -> u64 {
		self.id
	}

	fn surface(&self) -> Option<Surface> {
		let (mut ox, mut oy, mut oz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut zx, mut zy, mut zz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut xx, mut xy, mut xz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut p1, mut p2) = (0.0_f64, 0.0_f64);
		let kind = match ffi::face_surface(&self.inner, &mut ox, &mut oy, &mut oz, &mut zx, &mut zy, &mut zz, &mut xx, &mut xy, &mut xz, &mut p1, &mut p2).unwrap_or(0) {
			1 => SurfaceKind::Plane,
			2 => SurfaceKind::Cylinder { radius: p1 },
			3 => SurfaceKind::Cone { radius: p1, semi_angle: p2 },
			4 => SurfaceKind::Sphere { radius: p1 },
			5 => SurfaceKind::Torus { major_radius: p1, minor_radius: p2 },
			_ => return None,
		};
		Some(Surface { origin: DVec3::new(ox, oy, oz), axis_z: DVec3::new(zx, zy, zz), axis_x: DVec3::new(xx, xy, xz), kind })
	}

	fn area(&self) -> f64 {
		ffi::face_surface_area(&self.inner).unwrap_or(f64::NAN)
	}

	fn center(&self) -> DVec3 {
		let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
		if ffi::face_center_of_mass(&self.inner, &mut x, &mut y, &mut z).is_err() {
			return DVec3::NAN;
		}
		DVec3::new(x, y, z)
	}

	fn project(&self, p: DVec3) -> (DVec3, DVec3) {
		let (mut cpx, mut cpy, mut cpz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut nx, mut ny, mut nz) = (0.0_f64, 0.0_f64, 0.0_f64);
		// A bounded face always has a boundary to fall back on, so a failure is a bug.
		ffi::face_project_point(&self.inner, p.x, p.y, p.z, &mut cpx, &mut cpy, &mut cpz, &mut nx, &mut ny, &mut nz).unwrap_or_else(|e| panic!("Face::project: {}", e.what()));
		(DVec3::new(cpx, cpy, cpz), DVec3::new(nx, ny, nz))
	}

	fn iter_edge(&self) -> impl Iterator<Item = &Edge> + '_ {
		self.edges
			.get_or_init(|| {
				let mut ids = Vec::new();
				ffi::face_edges(&self.inner, &self.local, &mut ids).map(|v| v.iter().zip(ids).map(|(e, id)| Edge::with_id(ffi::clone_edge_handle(e), id)).collect()).unwrap_or_default()
			})
			.iter()
	}
}
