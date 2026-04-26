use super::compound::CompoundShape;
use super::edge::Edge;
use super::face::Face;
use super::ffi;
use crate::common::error::Error;
use crate::traits::{Compound, ProfileOrient, SolidStruct, Transform};
use glam::DVec3;
use std::sync::{Mutex, OnceLock};

// OCCT の BRepOffsetAPI_ThruSections は内部で global state (おそらく
// BSplCLib のキャッシュや GeomFill_AppSurf の作業バッファ) を使うため、
// 複数スレッドから同時に呼び出すと heap corruption を起こす。
// 並列テスト実行下で再現する症状で、loft 呼び出し全体を Mutex で
// serialize すれば回避できる。性能劣化はあるが loft は重い操作なので
// ロック粒度の粗さは現実的に問題にならない。
static LOFT_LOCK: Mutex<()> = Mutex::new(());

/// Encode `ProfileOrient` into FFI arguments: (kind, ux, uy, uz, aux_spine_edges).
fn encode_orient(orient: ProfileOrient) -> (u32, f64, f64, f64, cxx::UniquePtr<cxx::CxxVector<ffi::TopoDS_Edge>>) {
	let mut aux_vec = ffi::edge_vec_new();
	let (kind, ux, uy, uz) = match orient {
		ProfileOrient::Fixed => (0u32, 0.0, 0.0, 0.0),
		ProfileOrient::Torsion => (1u32, 0.0, 0.0, 0.0),
		ProfileOrient::Up(v) => (2u32, v.x, v.y, v.z),
		ProfileOrient::Auxiliary(edges) => {
			for e in edges {
				ffi::edge_vec_push(aux_vec.pin_mut(), &e.inner);
			}
			(3u32, 0.0, 0.0, 0.0)
		}
	};
	(kind, ux, uy, uz, aux_vec)
}

#[cfg(feature = "color")]
fn remap_colormap_by_order(old_inner: &ffi::TopoDS_Shape, new_inner: &ffi::TopoDS_Shape, old_colormap: &std::collections::HashMap<u64, crate::common::color::Color>) -> std::collections::HashMap<u64, crate::common::color::Color> {
	let mut colormap = std::collections::HashMap::new();
	let old_faces = ffi::shape_faces(old_inner);
	let new_faces = ffi::shape_faces(new_inner);
	for (old_face, new_face) in old_faces.iter().zip(new_faces.iter()) {
		if let Some(&color) = old_colormap.get(&ffi::face_tshape_id(old_face)) {
			colormap.insert(ffi::face_tshape_id(new_face), color);
		}
	}
	colormap
}

/// A single solid topology shape wrapping a `TopoDS_Shape` guaranteed to be `TopAbs_SOLID`.
///
/// `inner` is private to prevent external mutation that could break the solid invariant.
/// Use the provided methods to query and transform the solid.
///
/// `edges` / `faces` are lazy `OnceLock` caches populated on first `iter_edge` /
/// `iter_face` call. Since topology-changing ops consume `self` and construct
/// a new `Solid` via `Solid::new`, these caches are invalidated automatically
/// (new instance → fresh `OnceLock::new()`). See
/// `notes/20260420-OCCTトポロジ不変性と設計含意.md`.
pub struct Solid {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	edges: OnceLock<Vec<Edge>>,
	faces: OnceLock<Vec<Face>>,
	#[cfg(feature = "color")]
	colormap: std::collections::HashMap<u64, crate::common::color::Color>,
	/// Face-derivation history from the most recent boolean operation.
	///
	/// Flat `[post_id, src_id, post_id, src_id, ...]` pairs:
	/// - `post_id` is the TShape* of a face in this Solid (or, after
	///   decompose, possibly in a sibling result Solid — over-inclusion
	///   is harmless because consumers filter by `src_id`).
	/// - `src_id` is the TShape* of the originating face in either
	///   boolean input (a or b — distinction is intentionally lost;
	///   TShape* is globally unique so callers filter by membership).
	///
	/// Empty for primitives, builders (extrude/sweep/loft/bspline/shell/
	/// fillet/chamfer), I/O reads, and after scale/mirror/Clone (which
	/// rebuild topology). Preserved across translate/rotate/color.
	history: Vec<u64>,
}

impl Solid {
	/// Create a `Solid` from a `TopoDS_Shape`.
	///
	/// # Panics
	/// Panics if `inner` is not `TopAbs_SOLID` (and not null).
	pub(crate) fn new(
		inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
		#[cfg(feature = "color")] colormap: std::collections::HashMap<u64, crate::common::color::Color>,
		history: Vec<u64>,
	) -> Self {
		debug_assert!(ffi::shape_is_null(&inner) || ffi::shape_is_solid(&inner), "Solid::new called with a non-SOLID shape");
		Solid {
			inner,
			edges: OnceLock::new(),
			faces: OnceLock::new(),
			#[cfg(feature = "color")]
			colormap,
			history,
		}
	}

	// ==================== Internal accessors ====================

	/// Borrow the underlying `TopoDS_Shape` (crate-internal only).
	pub(crate) fn inner(&self) -> &ffi::TopoDS_Shape {
		&self.inner
	}

	/// Return the underlying `TopoDS_TShape*` address as a `u64`.
	pub fn tshape_id(&self) -> u64 {
		ffi::shape_tshape_id(&self.inner)
	}

	/// Iterate over face-derivation pairs `[post_id, src_id]` from the most
	/// recent boolean operation that produced this Solid (or its source
	/// chain, while it stays through translate/rotate/color).
	///
	/// Empty after primitive/builder construction, I/O read, scale/mirror,
	/// or Clone. See the `history` field doc on `Solid` for the full list.
	pub fn iter_history(&self) -> impl Iterator<Item = [u64; 2]> + '_ {
		self.history.chunks_exact(2).map(|c| [c[0], c[1]])
	}

	// ==================== Color accessors ====================

	/// Read-only access to the per-face colormap.
	#[cfg(feature = "color")]
	pub fn colormap(&self) -> &std::collections::HashMap<u64, crate::common::color::Color> {
		&self.colormap
	}

	/// Mutable access to the per-face colormap.
	#[cfg(feature = "color")]
	pub fn colormap_mut(&mut self) -> &mut std::collections::HashMap<u64, crate::common::color::Color> {
		&mut self.colormap
	}

	// ==================== Constructors ====================

	/// Returns `true` if this solid wraps a null shape.
	pub fn is_null(&self) -> bool {
		ffi::shape_is_null(&self.inner)
	}

	// ==================== Topology iteration ====================

	/// Iterate over this solid's edges as `&Edge`. Unique under TShape identity
	/// (each OCCT edge appears once even when shared between faces). First call
	/// populates the internal cache; later calls are free.
	pub fn iter_edge(&self) -> std::slice::Iter<'_, Edge> {
		self.edges
			.get_or_init(|| {
				ffi::shape_edges(&self.inner)
					.iter()
					.map(|e_ref| {
						let owned = ffi::clone_edge_handle(e_ref);
						Edge::try_from_ffi(owned, "shape_edges: null".into()).expect("shape_edges: unexpected null (this is a bug)")
					})
					.collect()
			})
			.iter()
	}

	/// Iterate over this solid's faces as `&Face`. First call populates the
	/// internal cache; later calls are free.
	pub fn iter_face(&self) -> std::slice::Iter<'_, Face> {
		self.faces
			.get_or_init(|| {
				ffi::shape_faces(&self.inner)
					.iter()
					.map(|f_ref| Face::new(ffi::clone_face_handle(f_ref)))
					.collect()
			})
			.iter()
	}
}

impl SolidStruct for Solid {
	type Edge = Edge;
	type Face = Face;

	// ==================== Constructors ====================

	fn cube(x: f64, y: f64, z: f64) -> Solid {
		let inner = ffi::make_box(0.0, 0.0, 0.0, x, y, z);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		)
	}

	fn cylinder(r: f64, axis: DVec3, h: f64) -> Solid {
		let inner = ffi::make_cylinder(0.0, 0.0, 0.0, axis.x, axis.y, axis.z, r, h);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		)
	}

	fn sphere(radius: f64) -> Solid {
		let inner = ffi::make_sphere(0.0, 0.0, 0.0, radius);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		)
	}

	fn cone(r1: f64, r2: f64, axis: DVec3, h: f64) -> Solid {
		let inner = ffi::make_cone(0.0, 0.0, 0.0, axis.x, axis.y, axis.z, r1, r2, h);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		)
	}

	fn torus(r1: f64, r2: f64, axis: DVec3) -> Solid {
		let inner = ffi::make_torus(0.0, 0.0, 0.0, axis.x, axis.y, axis.z, r1, r2);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		)
	}

	fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Solid {
		let inner = ffi::make_half_space(plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		)
	}

	// ==================== Extrude ====================

	fn extrude<'a>(profile: impl IntoIterator<Item = &'a Edge>, dir: DVec3) -> Result<Self, Error> {
		let mut profile_vec = ffi::edge_vec_new();
		for e in profile {
			ffi::edge_vec_push(profile_vec.pin_mut(), &e.inner);
		}
		let shape = ffi::make_extrude(&profile_vec, dir.x, dir.y, dir.z);
		if shape.is_null() {
			return Err(Error::ExtrudeFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		))
	}

	// ==================== Shell ====================

	fn shell<'a>(&self, thickness: f64, open_faces: impl IntoIterator<Item = &'a Face>) -> Result<Self, Error> {
		let mut face_vec = ffi::face_vec_new();
		for f in open_faces {
			ffi::face_vec_push(face_vec.pin_mut(), &f.inner);
		}
		let shape = ffi::builder_thick_solid(&self.inner, &face_vec, thickness);
		if shape.is_null() {
			return Err(Error::ShellFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		))
	}

	// ==================== Fillet / Chamfer ====================

	fn fillet_edges<'a>(&self, radius: f64, edges: impl IntoIterator<Item = &'a Edge>) -> Result<Self, Error> {
		let mut edge_vec = ffi::edge_vec_new();
		for e in edges {
			ffi::edge_vec_push(edge_vec.pin_mut(), &e.inner);
		}
		let shape = ffi::builder_fillet(&self.inner, &edge_vec, radius);
		if shape.is_null() {
			return Err(Error::FilletFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		))
	}

	fn chamfer_edges<'a>(&self, distance: f64, edges: impl IntoIterator<Item = &'a Edge>) -> Result<Self, Error> {
		let mut edge_vec = ffi::edge_vec_new();
		for e in edges {
			ffi::edge_vec_push(edge_vec.pin_mut(), &e.inner);
		}
		let shape = ffi::builder_chamfer(&self.inner, &edge_vec, distance);
		if shape.is_null() {
			return Err(Error::ChamferFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		))
	}

	// ==================== Sweep ====================

	fn sweep<'a, 'b, 'c>(profile: impl IntoIterator<Item = &'a Edge>, spine: impl IntoIterator<Item = &'b Edge>, orient: ProfileOrient<'c>) -> Result<Self, Error> {
		let mut profile_vec = ffi::edge_vec_new();
		for e in profile {
			ffi::edge_vec_push(profile_vec.pin_mut(), &e.inner);
		}
		let mut spine_vec = ffi::edge_vec_new();
		for e in spine {
			ffi::edge_vec_push(spine_vec.pin_mut(), &e.inner);
		}
		let (kind, ux, uy, uz, aux_vec) = encode_orient(orient);
		let shape = ffi::make_pipe_shell(&profile_vec, &spine_vec, kind, ux, uy, uz, &aux_vec);
		if shape.is_null() {
			return Err(Error::SweepFailed);
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		))
	}

	// ==================== Loft ====================

	fn loft<'a, S, I>(sections: S) -> Result<Self, Error> where S: IntoIterator<Item = I>, I: IntoIterator<Item = &'a Edge>, Edge: 'a {
		let _guard = LOFT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

		let mut all_edges = ffi::edge_vec_new();
		let mut section_count = 0usize;

		for sec in sections {
			if section_count > 0 {
				ffi::edge_vec_push_null(all_edges.pin_mut());
			}
			let mut count = 0u32;
			for edge in sec {
				ffi::edge_vec_push(all_edges.pin_mut(), &edge.inner);
				count += 1;
			}
			if count == 0 {
				return Err(Error::LoftFailed(format!(
					"loft: section {} is empty (each section must contain ≥1 edge)",
					section_count
				)));
			}
			section_count += 1;
		}

		if section_count < 2 {
			return Err(Error::LoftFailed(format!(
				"loft: need ≥2 sections, got {} (a single section has no thickness to skin across)",
				section_count
			)));
		}

		let shape = ffi::make_loft(&all_edges);
		if shape.is_null() {
			return Err(Error::LoftFailed(format!(
				"loft: OCCT BRepOffsetAPI_ThruSections failed (sections={}). \
				 Check that each section forms a valid closed wire and sections are not coplanar.",
				section_count
			)));
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		))
	}

	// ==================== Bspline ====================

	fn bspline(u: usize, v: usize, u_periodic: bool, point: impl Fn(usize, usize) -> DVec3) -> Result<Self, Error> {
		if u < 2 || v < 3 {
			return Err(Error::BsplineFailed(format!("grid must be at least 2x3 (u={}, v={})", u, v)));
		}

		let mut coords = Vec::with_capacity(3 * u * v);
		for i in 0..u {
			for j in 0..v {
				let p = point(i, j);
				coords.push(p.x);
				coords.push(p.y);
				coords.push(p.z);
			}
		}

		let shape = ffi::make_bspline_solid(&coords, u as u32, v as u32, u_periodic);
		if shape.is_null() {
			return Err(Error::BsplineFailed(format!("OCCT construction failed (u={}, v={}, u_periodic={})", u, v, u_periodic)));
		}
		Ok(Solid::new(
			shape,
			#[cfg(feature = "color")]
			std::collections::HashMap::new(),
			Default::default(),
		))
	}

	// ==================== Boolean primitives ====================

	fn boolean_union<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<Vec<Self>, Error> where Self: 'a + 'b {
		Self::boolean_union_impl(a, b)
	}

	fn boolean_subtract<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<Vec<Self>, Error> where Self: 'a + 'b {
		Self::boolean_subtract_impl(a, b)
	}

	fn boolean_intersect<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<Vec<Self>, Error> where Self: 'a + 'b {
		Self::boolean_intersect_impl(a, b)
	}
}

// ==================== impl Transform for Solid ====================

impl Transform for Solid {
	fn translate(self, translation: DVec3) -> Self {
		let inner = ffi::transform_translate(&self.inner, translation.x, translation.y, translation.z);
		// translate/rotate use shape.Moved() — TShape is shared but Location
		// changes, so cached edges/faces (which embed Location) would go stale.
		// Solid::new gives a fresh OnceLock::new() cache matching the new Location.
		// `history` is preserved because TShape* (= post_id) is unchanged.
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			self.colormap,
			self.history,
		)
	}

	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		let inner = ffi::transform_rotate(&self.inner, axis_origin.x, axis_origin.y, axis_origin.z, axis_direction.x, axis_direction.y, axis_direction.z, angle);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			self.colormap,
			self.history,
		)
	}

	// scale/mirror consume self for API consistency, but internally clone the geometry.
	// Unlike translate/rotate which use gp_Trsf + shape.Moved() (preserving TShape),
	// scale/mirror cannot use Moved(): since OCCT Fix 0027457 (v7.6), TopLoc_Location
	// rejects gp_Trsf with scale != 1 or negative determinant, because downstream
	// algorithms (meshing, booleans) break on non-rigid transforms in locations.
	// Therefore BRepBuilderAPI_Transform is required, which rebuilds topology.
	// C++ impl: cpp/wrapper.cpp transform_scale() / transform_mirror()
	// See: https://dev.opencascade.org/content/how-scale-or-mirror-shape
	//      BRepBuilderAPI_Transform.cxx:48-49 (myUseModif branch)

	fn scale(self, center: DVec3, factor: f64) -> Self {
		let inner = ffi::transform_scale(&self.inner, center.x, center.y, center.z, factor);
		#[cfg(feature = "color")]
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		// scale/mirror rebuild topology via BRepBuilderAPI_Transform → post_ids
		// in old `history` no longer exist. Drop history (caller must re-derive
		// from a fresh boolean call).
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
			Default::default(),
		)
	}

	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		let inner = ffi::transform_mirror(&self.inner, plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z);
		#[cfg(feature = "color")]
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
			Default::default(),
		)
	}
}

// ==================== impl Compound for Solid ====================
//
// Solid-specific per-element ops (queries / color / boolean wrappers / clean).
// `Vec<Solid>` and `[Solid; N]` impls live in src/traits.rs and delegate to this one.
impl Compound for Solid {
	type Elem = Solid;

	fn clean(&self) -> Result<Self, Error> {
		let mut history: Vec<u64> = Default::default();
		let inner = ffi::builder_clean(&self.inner, &mut history);
		if inner.is_null() {
			return Err(Error::CleanFailed);
		}
		#[cfg(feature = "color")]
		let colormap = {
			let mut m = std::collections::HashMap::new();
			for pair in history.chunks_exact(2) {
				if let Some(&color) = self.colormap.get(&pair[1]) {
					m.entry(pair[0]).or_insert(color);
				}
			}
			m
		};
		Ok(Solid::new(inner, #[cfg(feature = "color")] colormap, history))
	}

	// ==================== Queries ====================

	fn volume(&self) -> f64 {
		ffi::shape_volume(&self.inner)
	}

	fn area(&self) -> f64 {
		ffi::shape_surface_area(&self.inner)
	}

	fn center(&self) -> DVec3 {
		let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::shape_center_of_mass(&self.inner, &mut x, &mut y, &mut z);
		DVec3::new(x, y, z)
	}

	fn inertia(&self) -> glam::DMat3 {
		let (mut m00, mut m01, mut m02) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut m10, mut m11, mut m12) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut m20, mut m21, mut m22) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::shape_inertia_tensor(&self.inner,
			&mut m00, &mut m01, &mut m02,
			&mut m10, &mut m11, &mut m12,
			&mut m20, &mut m21, &mut m22);
		// OCCT fills row-major; DMat3::from_cols_array is column-major so
		// transpose when handing the components over.
		glam::DMat3::from_cols_array(&[
			m00, m10, m20,
			m01, m11, m21,
			m02, m12, m22,
		])
	}

	fn contains(&self, point: DVec3) -> bool {
		ffi::shape_contains_point(&self.inner, point.x, point.y, point.z)
	}

	fn bounding_box(&self) -> [DVec3; 2] {
		let (mut xmin, mut ymin, mut zmin) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut xmax, mut ymax, mut zmax) = (0.0_f64, 0.0_f64, 0.0_f64);
		ffi::shape_bounding_box(&self.inner, &mut xmin, &mut ymin, &mut zmin, &mut xmax, &mut ymax, &mut zmax);
		[DVec3::new(xmin, ymin, zmin), DVec3::new(xmax, ymax, zmax)]
	}

	// ==================== Color ====================

	#[cfg(feature = "color")]
	fn color(self, color: impl Into<crate::common::color::Color>) -> Self {
		let c = color.into();
		let colormap = ffi::shape_faces(&self.inner).iter().map(|f| (ffi::face_tshape_id(f), c)).collect();
		Self::new(self.inner, colormap, self.history)
	}

	#[cfg(feature = "color")]
	fn color_clear(self) -> Self {
		Self::new(self.inner, std::collections::HashMap::new(), self.history)
	}

	// ==================== Boolean ====================

	fn union<'a>(&self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<Vec<Solid>, Error> {
		<Solid as SolidStruct>::boolean_union([self], tool)
	}

	fn subtract<'a>(&self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<Vec<Solid>, Error> {
		<Solid as SolidStruct>::boolean_subtract([self], tool)
	}

	fn intersect<'a>(&self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<Vec<Solid>, Error> {
		<Solid as SolidStruct>::boolean_intersect([self], tool)
	}
}

impl Clone for Solid {
	fn clone(&self) -> Self {
		let inner = ffi::deep_copy(&self.inner);
		#[cfg(feature = "color")]
		let colormap = remap_colormap_by_order(&self.inner, &inner, &self.colormap);
		// deep_copy rebuilds topology — post_ids in `history` no longer point
		// to faces of the new shape. Drop history rather than remapping.
		Solid::new(
			inner,
			#[cfg(feature = "color")]
			colormap,
			Default::default(),
		)
	}
}

// ==================== Boolean operations ====================

#[cfg(feature = "color")]
fn merge_colormaps(history: &[u64], colormap_a: &std::collections::HashMap<u64, crate::common::color::Color>, colormap_b: &std::collections::HashMap<u64, crate::common::color::Color>) -> std::collections::HashMap<u64, crate::common::color::Color> {
	let mut result = std::collections::HashMap::new();
	for pair in history.chunks_exact(2) {
		// TShape* pointers are globally unique across both inputs, so a
		// single lookup against either colormap suffices (no collision).
		if let Some(&color) = colormap_a.get(&pair[1]).or_else(|| colormap_b.get(&pair[1])) {
			result.insert(pair[0], color);
		}
	}
	result
}

// `ca` / `cb` carry the source colormaps and are only consulted by the
// `color` feature; the boolean result and history are derived purely from
// the FFI out-parameter, so they go unused without `color`.
#[cfg_attr(not(feature = "color"), allow(unused_variables))]
fn build_boolean_result(inner: cxx::UniquePtr<ffi::TopoDS_Shape>, history: Vec<u64>, ca: CompoundShape, cb: CompoundShape) -> Result<Vec<Solid>, Error> {
	#[cfg(feature = "color")]
	let colormap = merge_colormaps(&history, ca.colormap(), cb.colormap());

	let compound = CompoundShape::from_raw(
		inner,
		#[cfg(feature = "color")]
		colormap,
		history,
	);

	Ok(compound.decompose())
}

// Op kind tags matching the C++ side `boolean_op` switch.
const BOOLEAN_OP_FUSE: u32 = 0;
const BOOLEAN_OP_CUT: u32 = 1;
const BOOLEAN_OP_COMMON: u32 = 2;

impl Solid {
	fn boolean_op_impl<'a, 'b>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'b Solid>, op_kind: u32) -> Result<Vec<Solid>, Error> {
		let ca = CompoundShape::new(a);
		let cb = CompoundShape::new(b);
		let mut history: Vec<u64> = Default::default();
		let inner = ffi::builder_boolean(ca.inner(), cb.inner(), op_kind, &mut history);
		if inner.is_null() { return Err(Error::BooleanOperationFailed); }
		build_boolean_result(inner, history, ca, cb)
	}

	pub(crate) fn boolean_union_impl<'a, 'b>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'b Solid>) -> Result<Vec<Solid>, Error> {
		Self::boolean_op_impl(a, b, BOOLEAN_OP_FUSE)
	}

	pub(crate) fn boolean_subtract_impl<'a, 'b>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'b Solid>) -> Result<Vec<Solid>, Error> {
		Self::boolean_op_impl(a, b, BOOLEAN_OP_CUT)
	}

	pub(crate) fn boolean_intersect_impl<'a, 'b>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'b Solid>) -> Result<Vec<Solid>, Error> {
		Self::boolean_op_impl(a, b, BOOLEAN_OP_COMMON)
	}
}
