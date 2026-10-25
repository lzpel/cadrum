use super::edge::{edges_to_ffi, Edge};
use super::ffi;
use crate::common::error::Error;
use crate::common::geometry::{split_frame, NurbsSurface, Surface, SurfaceKind};
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
	loops: OnceLock<Vec<Vec<Edge>>>,
}

impl Face {
	/// A face of a solid, with the id and placement its owner assigned.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Face>, local: cxx::UniquePtr<ffi::TopoDS_Face>, id: u64) -> Self {
		Face { inner, local, id, loops: OnceLock::new() }
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
		let edges = edges_to_ffi(profile);
		let (kind, p1, p2, data) = match &surface.kind {
			SurfaceKind::Plane => (1, 0.0, 0.0, Vec::new()),
			SurfaceKind::Cylinder { radius } => (2, *radius, 0.0, Vec::new()),
			SurfaceKind::Cone { radius, semi_angle } => (3, *radius, *semi_angle, Vec::new()),
			SurfaceKind::Sphere { radius } => (4, *radius, 0.0, Vec::new()),
			SurfaceKind::Torus { major_radius, minor_radius } => (5, *major_radius, *minor_radius, Vec::new()),
			SurfaceKind::Bspline(nurbs) => {
				let mut data = Vec::new();
				nurbs.to_flat(&mut data);
				(6, 0.0, 0.0, data)
			}
		};
		let (o, z, x) = (surface.origin, surface.axis_z, surface.axis_x);
		let inner = ffi::make_face(kind, o.x, o.y, o.z, z.x, z.y, z.z, x.x, x.y, x.z, p1, p2, &data, &edges).map_err(|e| Error::Validation(format!("face: surface={surface:?}: {}", e.what())))?;
		Ok(Face::standalone(inner))
	}

	fn id(&self) -> u64 {
		self.id
	}

	fn surface(&self) -> Result<Surface, Error> {
		let mut out = Vec::new();
		let kind = ffi::face_surface(&self.inner, &mut out).map_err(|e| Error::Query(format!("surface: {}", e.what())))?;
		let truncated = || Error::Query("surface: the kernel returned truncated data".into());
		let ((origin, axis_z, axis_x, p1, p2), rest) = split_frame(&out).ok_or_else(truncated)?;
		let kind = match kind {
			1 => SurfaceKind::Plane,
			2 => SurfaceKind::Cylinder { radius: p1 },
			3 => SurfaceKind::Cone { radius: p1, semi_angle: p2 },
			4 => SurfaceKind::Sphere { radius: p1 },
			5 => SurfaceKind::Torus { major_radius: p1, minor_radius: p2 },
			_ => SurfaceKind::Bspline(NurbsSurface::from_flat(rest).ok_or_else(truncated)?),
		};
		Ok(Surface { origin, axis_z, axis_x, kind })
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

	fn iter_loop(&self) -> impl Iterator<Item = &[Edge]> + '_ {
		self.loops
			.get_or_init(|| {
				let (mut ids, mut counts) = (Vec::new(), Vec::new());
				let Ok(edges) = ffi::face_loops(&self.inner, &self.local, &mut ids, &mut counts) else { return Vec::new() };
				let mut edges = edges.iter().zip(ids).map(|(e, id)| Edge::with_id(ffi::clone_edge_handle(e), id));
				counts.iter().map(|&n| edges.by_ref().take(n as usize).collect()).collect()
			})
			.iter()
			.map(Vec::as_slice)
	}
}
