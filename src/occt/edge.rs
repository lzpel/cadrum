use super::ffi;
use crate::common::error::Error;
use crate::traits::{BSplineEnd, EdgeStruct, Transform};
use glam::DVec3;

/// An edge topology shape.
pub struct Edge {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Edge>,
}

impl Clone for Edge {
	fn clone(&self) -> Self {
		Edge { inner: ffi::deep_copy_edge(&self.inner).unwrap_or_else(|e| panic!("Edge::clone: {}", e.what())) }
	}
}

impl std::fmt::Debug for Edge {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Edge({}, start={:?}, end={:?})", self.id(), self.start_point(), self.end_point())
	}
}

impl EdgeStruct for Edge {
	fn id(&self) -> u64 {
		ffi::edge_tshape_id(&self.inner)
	}

	// ==================== Per-edge queries ====================

	fn start_point(&self) -> DVec3 {
		let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ex, mut ey, mut ez) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::edge_endpoints(&self.inner, &mut sx, &mut sy, &mut sz, &mut ex, &mut ey, &mut ez);
		DVec3::new(sx, sy, sz)
	}

	fn end_point(&self) -> DVec3 {
		let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ex, mut ey, mut ez) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::edge_endpoints(&self.inner, &mut sx, &mut sy, &mut sz, &mut ex, &mut ey, &mut ez);
		DVec3::new(ex, ey, ez)
	}

	fn start_tangent(&self) -> DVec3 {
		let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ex, mut ey, mut ez) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::edge_tangents(&self.inner, &mut sx, &mut sy, &mut sz, &mut ex, &mut ey, &mut ez);
		DVec3::new(sx, sy, sz)
	}

	fn end_tangent(&self) -> DVec3 {
		let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut ex, mut ey, mut ez) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::edge_tangents(&self.inner, &mut sx, &mut sy, &mut sz, &mut ex, &mut ey, &mut ez);
		DVec3::new(ex, ey, ez)
	}

	fn precision_distance() -> f64 {
		ffi::precision_confusion()
	}

	fn approximation_segments(&self, tessellation: crate::traits::Tessellation) -> Vec<DVec3> {
		ffi::edge_approximation_segments(&self.inner, tessellation.deflection_linear, tessellation.deflection_angular, tessellation.relative_linear).chunks_exact(3).map(|c| DVec3::new(c[0], c[1], c[2])).collect()
	}

	fn project(&self, p: DVec3) -> (DVec3, DVec3) {
		let (mut cpx, mut cpy, mut cpz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut tx, mut ty, mut tz) = (0.0_f64, 0.0_f64, 0.0_f64);
		// FFI returns false only when OCCT throws. Degenerate edges (sphere pole,
		// cone apex) go through the pcurve and yield the point with a zero tangent.
		assert!(ffi::edge_project_point(&self.inner, p.x, p.y, p.z, &mut cpx, &mut cpy, &mut cpz, &mut tx, &mut ty, &mut tz), "Edge::project: OCCT projector threw (this is a bug)");
		(DVec3::new(cpx, cpy, cpz), DVec3::new(tx, ty, tz))
	}

	fn helix(radius: f64, pitch: f64, height: f64, axis: DVec3, x_ref: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_helix_edge(axis.x, axis.y, axis.z, x_ref.x, x_ref.y, x_ref.z, radius, pitch, height).map_err(|e| Error::Edge(format!("helix: radius={radius}, pitch={pitch}, height={height}, axis={axis:?}, x_ref={x_ref:?}: {}", e.what())))?;
		Ok(Edge { inner })
	}

	fn polygon<'a>(points: impl IntoIterator<Item = &'a DVec3>) -> Result<Vec<Self>, Error> {
		let coords: Vec<f64> = points.into_iter().flat_map(|p| [p.x, p.y, p.z]).collect();
		let edges = ffi::make_polygon_edges(&coords).map_err(|e| Error::Edge(format!("polygon: {} point(s): {}", coords.len() / 3, e.what())))?;
		// deep_copy_edge so each Edge owns its topology instead of borrowing the vector.
		Ok(edges.iter().map(|e| Edge { inner: ffi::deep_copy_edge(e).unwrap_or_else(|err| panic!("polygon: {}", err.what())) }).collect())
	}

	fn circle(radius: f64, axis: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_circle_edge(axis.x, axis.y, axis.z, radius).map_err(|e| Error::Edge(format!("circle: radius={radius}, axis={axis:?}: {}", e.what())))?;
		Ok(Edge { inner })
	}

	fn line(a: DVec3, b: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_line_edge(a.x, a.y, a.z, b.x, b.y, b.z).map_err(|e| Error::Edge(format!("line: a={a:?}, b={b:?}: {}", e.what())))?;
		Ok(Edge { inner })
	}

	fn arc_3pts(start: DVec3, mid: DVec3, end: DVec3) -> Result<Self, Error> {
		let inner = ffi::make_arc_edge(start.x, start.y, start.z, mid.x, mid.y, mid.z, end.x, end.y, end.z).map_err(|e| Error::Edge(format!("arc_3pts: start={start:?}, mid={mid:?}, end={end:?}: {}", e.what())))?;
		Ok(Edge { inner })
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
		// 自動除去はせず Error::Edge で誤用を明示する (AGENTS.md "誤解 vs 手間" 方針)。
		if matches!(end, BSplineEnd::Periodic) {
			let first = pts.first().expect("checked above");
			let last = pts.last().expect("checked above");
			if first == last {
				return Err(Error::Edge(format!("bspline(Periodic): first and last points coincide ({first:?}); periodicity is encoded in the basis, do not duplicate the closing point")));
			}
		}

		// FFI 用に flat な xyz 列にパック。
		let coords: Vec<f64> = pts.iter().flat_map(|p| [p.x, p.y, p.z]).collect();

		// BSplineEnd を (kind, start_tangent, end_tangent) にエンコード。
		// kind: 0 = Periodic, 1 = NotAKnot, 2 = Clamped。
		// 接線ベクトルは Clamped 以外では使われない (C++ 側で無視)。
		let (kind, sx, sy, sz, ex, ey, ez) = match end {
			BSplineEnd::Periodic => (0u32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
			BSplineEnd::NotAKnot => (1u32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
			BSplineEnd::Clamped { start: s, end: e } => (2u32, s.x, s.y, s.z, e.x, e.y, e.z),
		};

		let inner = ffi::make_bspline_edge(&coords, kind, sx, sy, sz, ex, ey, ez).map_err(|e| Error::Edge(format!("bspline: {} points, end={end:?}: {}", pts.len(), e.what())))?;
		Ok(Edge { inner })
	}
}

// endpoint / tangent / is_closed / approximation_segments / project は
// 上の `impl EdgeStruct for Edge` に統合済み。エッジ列 (= wire) はイテレータ
// 慣用句で扱い、専用トレイトは持たない。

// `Transform` returns `Self`, and an affine transform of a valid edge only throws
// on a degenerate axis or plane — a caller bug, so it panics with OCCT's message.
impl Transform for Edge {
	fn translate(self, t: DVec3) -> Self {
		Edge { inner: ffi::translate_edge(&self.inner, t.x, t.y, t.z).unwrap_or_else(|e| panic!("Edge::translate: {}", e.what())) }
	}

	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		Edge {
			inner: ffi::rotate_edge(&self.inner, axis_origin.x, axis_origin.y, axis_origin.z, axis_direction.x, axis_direction.y, axis_direction.z, angle).unwrap_or_else(|e| panic!("Edge::rotate: {}", e.what())),
		}
	}

	fn scale(self, center: DVec3, factor: f64) -> Self {
		Edge { inner: ffi::scale_edge(&self.inner, center.x, center.y, center.z, factor).unwrap_or_else(|e| panic!("Edge::scale: {}", e.what())) }
	}

	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		Edge {
			inner: ffi::mirror_edge(&self.inner, plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z).unwrap_or_else(|e| panic!("Edge::mirror: {}", e.what())),
		}
	}
}

/// Split `profile` into closed loops and flatten them with null-edge sentinels,
/// the form `make_extrude` / `make_revolve` read. An edge may run either way: it
/// is reversed when its end, not its start, meets the loop within `Edge::precision_distance`.
pub(super) fn loops_to_ffi<'a>(profile: impl IntoIterator<Item = &'a Edge>) -> Result<cxx::UniquePtr<cxx::CxxVector<ffi::TopoDS_Edge>>, Error> {
	let tolerance = Edge::precision_distance();
	let near = |a: DVec3, b: DVec3| a.distance(b) <= tolerance;
	let mut edges = ffi::edge_vec_new();
	// The loop being traced as (edge, reversed), with its start and end.
	let mut open: Vec<(&Edge, bool)> = Vec::new();
	let (mut start, mut end) = (DVec3::ZERO, DVec3::ZERO);
	for edge in profile {
		let (a, b) = (edge.start_point(), edge.end_point());
		if open.is_empty() {
			(start, end) = (a, b);
			open.push((edge, false));
		} else {
			// The first edge's direction is unknown until the second one meets it.
			if open.len() == 1 && !near(end, a) && !near(end, b) && (near(start, a) || near(start, b)) {
				open[0].1 = true;
				(start, end) = (end, start);
			}
			let reversed = if near(end, a) {
				false
			} else if near(end, b) {
				true
			} else {
				return Err(Error::Edge(format!("profile: edge from {a:?} to {b:?} meets neither end of the previous edge at {end:?}")));
			};
			end = if reversed { a } else { b };
			open.push((edge, reversed));
		}
		if near(end, start) {
			if !edges.is_empty() {
				ffi::edge_vec_push_null(edges.pin_mut());
			}
			for (e, reversed) in open.drain(..) {
				ffi::edge_vec_push_oriented(edges.pin_mut(), &e.inner, reversed);
			}
		}
	}
	match open.len() {
		0 => Ok(edges),
		count => Err(Error::Edge(format!("profile: {count} trailing edges do not close a loop"))),
	}
}
