use super::ffi;
use crate::common::error::Error;
use crate::traits::{BSplineEnd, EdgeStruct, Transform};
use glam::DVec3;

/// An edge topology shape.
///
/// `id` is fixed when the edge is wrapped: its own TShape address for a standalone
/// edge, or the owner-relative id `solid_edges` / `face_edges` computed for an edge
/// taken from a solid (see `ffi.h`).
pub struct Edge {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Edge>,
	id: u64,
}

impl Edge {
	/// A standalone edge: its own owner, so its id is its TShape address.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Edge>) -> Self {
		let id = ffi::edge_tshape_id(&inner);
		Edge { inner, id }
	}

	/// An edge of a solid or face, with the id its owner assigned.
	pub(crate) fn with_id(inner: cxx::UniquePtr<ffi::TopoDS_Edge>, id: u64) -> Self {
		Edge { inner, id }
	}
}

// A shallow handle copy sharing the immutable TShape, so the id is kept.
impl Clone for Edge {
	fn clone(&self) -> Self {
		Edge { inner: ffi::clone_edge_handle(&self.inner), id: self.id }
	}
}

impl std::fmt::Debug for Edge {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Edge({}, start={:?}, end={:?})", self.id(), self.start_point(), self.end_point())
	}
}

impl EdgeStruct for Edge {
	fn id(&self) -> u64 {
		self.id
	}

	// ==================== Per-edge queries ====================

	fn start_point(&self) -> DVec3 {
		let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ex, mut ey, mut ez) = (0.0_f64, 0.0_f64, 0.0_f64);
		if ffi::edge_endpoints(&self.inner, &mut sx, &mut sy, &mut sz, &mut ex, &mut ey, &mut ez).is_err() {
			return DVec3::NAN;
		}
		DVec3::new(sx, sy, sz)
	}

	fn end_point(&self) -> DVec3 {
		let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ex, mut ey, mut ez) = (0.0_f64, 0.0_f64, 0.0_f64);
		if ffi::edge_endpoints(&self.inner, &mut sx, &mut sy, &mut sz, &mut ex, &mut ey, &mut ez).is_err() {
			return DVec3::NAN;
		}
		DVec3::new(ex, ey, ez)
	}

	fn start_tangent(&self) -> DVec3 {
		let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ex, mut ey, mut ez) = (0.0_f64, 0.0_f64, 0.0_f64);
		if ffi::edge_tangents(&self.inner, &mut sx, &mut sy, &mut sz, &mut ex, &mut ey, &mut ez).is_err() {
			return DVec3::NAN;
		}
		DVec3::new(sx, sy, sz)
	}

	fn end_tangent(&self) -> DVec3 {
		let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ex, mut ey, mut ez) = (0.0_f64, 0.0_f64, 0.0_f64);
		if ffi::edge_tangents(&self.inner, &mut sx, &mut sy, &mut sz, &mut ex, &mut ey, &mut ez).is_err() {
			return DVec3::NAN;
		}
		DVec3::new(ex, ey, ez)
	}

	fn precision_distance() -> f64 {
		ffi::precision_confusion()
	}

	fn approximation_segments(&self, tessellation: crate::traits::Tessellation) -> Vec<DVec3> {
		ffi::edge_approximation_segments(&self.inner, tessellation.deflection_linear, tessellation.deflection_angular, tessellation.relative_linear).unwrap_or_default().chunks_exact(3).map(|c| DVec3::new(c[0], c[1], c[2])).collect()
	}

	fn project(&self, p: DVec3) -> (DVec3, DVec3) {
		let (mut cpx, mut cpy, mut cpz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut tx, mut ty, mut tz) = (0.0_f64, 0.0_f64, 0.0_f64);
		// Degenerate edges (sphere pole, cone apex) go through the pcurve and yield
		// the point with a zero tangent, so a failure here is a bug.
		ffi::edge_project_point(&self.inner, p.x, p.y, p.z, &mut cpx, &mut cpy, &mut cpz, &mut tx, &mut ty, &mut tz).unwrap_or_else(|e| panic!("Edge::project: {}", e.what()));
		(DVec3::new(cpx, cpy, cpz), DVec3::new(tx, ty, tz))
	}

	fn helix(radius: f64, pitch: f64, height: f64, axis: DVec3, x_ref: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_helix_edge(axis.x, axis.y, axis.z, x_ref.x, x_ref.y, x_ref.z, radius, pitch, height).map_err(|e| Error::Edge(format!("helix: radius={radius}, pitch={pitch}, height={height}, axis={axis:?}, x_ref={x_ref:?}: {}", e.what())))?;
		Ok(Edge::new(inner))
	}

	fn polygon<'a>(points: impl IntoIterator<Item = &'a DVec3>) -> Result<Vec<Self>, Error> {
		let coords: Vec<f64> = points.into_iter().flat_map(|p| [p.x, p.y, p.z]).collect();
		let edges = ffi::make_polygon_edges(&coords).map_err(|e| Error::Edge(format!("polygon: {} point(s): {}", coords.len() / 3, e.what())))?;
		Ok(edges.iter().map(|e| Edge::new(ffi::clone_edge_handle(e))).collect())
	}

	fn circle(radius: f64, axis: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_circle_edge(axis.x, axis.y, axis.z, radius).map_err(|e| Error::Edge(format!("circle: radius={radius}, axis={axis:?}: {}", e.what())))?;
		Ok(Edge::new(inner))
	}

	fn line(a: DVec3, b: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_line_edge(a.x, a.y, a.z, b.x, b.y, b.z).map_err(|e| Error::Edge(format!("line: a={a:?}, b={b:?}: {}", e.what())))?;
		Ok(Edge::new(inner))
	}

	fn arc_3pts(start: DVec3, mid: DVec3, end: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_arc_edge(start.x, start.y, start.z, mid.x, mid.y, mid.z, end.x, end.y, end.z).map_err(|e| Error::Edge(format!("arc_3pts: start={start:?}, mid={mid:?}, end={end:?}: {}", e.what())))?;
		Ok(Edge::new(inner))
	}

	fn bspline<'a>(points: impl IntoIterator<Item = &'a DVec3>, end: BSplineEnd) -> Result<Self, Error> {
		let pts: Vec<DVec3> = points.into_iter().copied().collect();

		// 最低点数チェック: Periodic は cubic 周期 spline の構造上 ≥ 3、その他は ≥ 2。
		let min_required = match end {
			BSplineEnd::Periodic => 3,
			BSplineEnd::NotAKnot | BSplineEnd::Clamped { .. } => 2,
		};
		if pts.len() < min_required {
			return Err(Error::Edge(format!("bspline: need ≥{} points for {:?}, got {}", min_required, end, pts.len())));
		}

		// Periodic では先頭と末尾が一致してはならない。OCCT は周期性を基底関数に
		// 組み込むので、ユーザーが点を重複させると行列が特異化して失敗する。
		if matches!(end, BSplineEnd::Periodic) && pts.first() == pts.last() {
			return Err(Error::Edge(format!("bspline(Periodic): first and last points coincide ({:?}); periodicity is encoded in the basis, do not duplicate the closing point", pts[0])));
		}

		let coords: Vec<f64> = pts.iter().flat_map(|p| [p.x, p.y, p.z]).collect();

		// kind: 0 = Periodic, 1 = NotAKnot, 2 = Clamped; the tangents matter to Clamped only.
		let (kind, sx, sy, sz, ex, ey, ez) = match end {
			BSplineEnd::Periodic => (0u32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
			BSplineEnd::NotAKnot => (1u32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
			BSplineEnd::Clamped { start: s, end: e } => (2u32, s.x, s.y, s.z, e.x, e.y, e.z),
		};

		let inner = ffi::make_bspline_edge(&coords, kind, sx, sy, sz, ex, ey, ez).map_err(|e| Error::Edge(format!("bspline: {} points, end={end:?}: {}", pts.len(), e.what())))?;
		Ok(Edge::new(inner))
	}
}

// `Transform` returns `Self`, and an affine transform of a valid edge only throws
// on a degenerate axis or plane — a caller bug, so it panics with OCCT's message.
// Translate and rotate move the edge and keep its id; scale and mirror rebuild it.
impl Transform for Edge {
	fn translate(self, t: DVec3) -> Self {
		Edge { inner: ffi::translate_edge(&self.inner, t.x, t.y, t.z).unwrap_or_else(|e| panic!("Edge::translate: {}", e.what())), id: self.id }
	}

	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		Edge {
			inner: ffi::rotate_edge(&self.inner, axis_origin.x, axis_origin.y, axis_origin.z, axis_direction.x, axis_direction.y, axis_direction.z, angle).unwrap_or_else(|e| panic!("Edge::rotate: {}", e.what())),
			id: self.id,
		}
	}

	fn scale(self, center: DVec3, factor: f64) -> Self {
		Edge::new(ffi::scale_edge(&self.inner, center.x, center.y, center.z, factor).unwrap_or_else(|e| panic!("Edge::scale: {}", e.what())))
	}

	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		Edge::new(ffi::mirror_edge(&self.inner, plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z).unwrap_or_else(|e| panic!("Edge::mirror: {}", e.what())))
	}
}

/// Collect edges into the `std::vector` the builders take (shallow handle copies).
pub(super) fn edges_to_ffi<'a>(edges: impl IntoIterator<Item = &'a Edge>) -> cxx::UniquePtr<cxx::CxxVector<ffi::TopoDS_Edge>> {
	let mut vec = ffi::edge_vec_new();
	for e in edges {
		ffi::edge_vec_push(vec.pin_mut(), &e.inner);
	}
	vec
}
