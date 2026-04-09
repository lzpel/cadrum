use super::ffi;
use super::iterators::ApproximationSegmentIterator;
use crate::traits::{EdgeExt, EdgeStruct, Transform};
use glam::DVec3;

/// An edge topology shape.
pub struct Edge {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Edge>,
}

impl Edge {
	/// Create an Edge wrapping a `TopoDS_Edge`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Edge>) -> Self {
		Edge { inner }
	}

	/// Get the approximation segments (polyline points) of this edge as an iterator.
	///
	/// `tolerance` controls both the angular deflection (radians) and the
	/// chord deflection (model units) of the approximation. Smaller values
	/// produce more points (finer approximation).
	pub fn approximation_segments_iter(&self, tolerance: f64) -> ApproximationSegmentIterator {
		let approx = ffi::edge_approximation_segments(&self.inner, tolerance);
		ApproximationSegmentIterator::new(approx)
	}

	/// Get the approximation segments with independent angular and chord deflection.
	///
	/// - `angular`: maximum angular deflection in radians between consecutive
	///   tangent directions. Controls how well curves are followed angularly.
	/// - `chord`: maximum chord deflection in model units (straight-line error
	///   between the polyline and the true curve). Controls absolute accuracy.
	pub fn approximation_segments_ex(&self, angular: f64, chord: f64) -> ApproximationSegmentIterator {
		let approx = ffi::edge_approximation_segments_ex(&self.inner, angular, chord);
		ApproximationSegmentIterator::new(approx)
	}
}

impl Clone for Edge {
	fn clone(&self) -> Self {
		Edge::new(ffi::deep_copy_edge(&self.inner))
	}
}

impl EdgeStruct for Edge {
	fn helix(radius: f64, pitch: f64, height: f64, axis: DVec3, x_ref: DVec3) -> Self {
		let inner = ffi::make_helix_edge(axis.x, axis.y, axis.z, x_ref.x, x_ref.y, x_ref.z, radius, pitch, height);
		Edge::new(inner)
	}

	fn polygon(points: impl IntoIterator<Item = DVec3>) -> Vec<Self> {
		let coords: Vec<f64> = points.into_iter().flat_map(|p| [p.x, p.y, p.z]).collect();
		let cxx_vec = ffi::make_polygon_edges(&coords);
		// CxxVector<TopoDS_Edge> → Vec<Edge>: pull each element out into a
		// UniquePtr<TopoDS_Edge> via deep_copy_edge so we own the topology.
		cxx_vec.iter().map(|e| Edge::new(ffi::deep_copy_edge(e))).collect()
	}
}

impl EdgeExt for Edge {
	type Elem = Edge;

	fn start_point(&self) -> DVec3 {
		let mut x = 0.0;
		let mut y = 0.0;
		let mut z = 0.0;
		ffi::edge_start_point(&self.inner, &mut x, &mut y, &mut z);
		DVec3::new(x, y, z)
	}

	fn start_tangent(&self) -> DVec3 {
		let mut x = 0.0;
		let mut y = 0.0;
		let mut z = 0.0;
		ffi::edge_start_tangent(&self.inner, &mut x, &mut y, &mut z);
		DVec3::new(x, y, z)
	}

	fn is_closed(&self) -> bool {
		ffi::edge_is_closed(&self.inner)
	}

	fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3> {
		let approx = ffi::edge_approximation_segments(&self.inner, tolerance);
		ApproximationSegmentIterator::new(approx).collect()
	}
}

impl Transform for Edge {
	fn translate(self, t: DVec3) -> Self {
		Edge::new(ffi::translate_edge(&self.inner, t.x, t.y, t.z))
	}

	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		Edge::new(ffi::rotate_edge(&self.inner, axis_origin.x, axis_origin.y, axis_origin.z, axis_direction.x, axis_direction.y, axis_direction.z, angle))
	}

	fn scale(self, center: DVec3, factor: f64) -> Self {
		Edge::new(ffi::scale_edge(&self.inner, center.x, center.y, center.z, factor))
	}

	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		Edge::new(ffi::mirror_edge(&self.inner, plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z))
	}
}
