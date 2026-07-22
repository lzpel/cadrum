//! C ABI between Rust and the OCCT wrapper (`src/ffi.cpp`).
//!
//! This file is the single source of truth for the ABI: `build.rs`'s
//! `bridge()` parses it with syn and generates `OUT_DIR/ffi.h`, which
//! `ffi.cpp` includes — so a signature mismatch between the [`raw`]
//! declarations here and the handwritten shims in ffi.cpp is caught by the
//! C++ compiler as a conflicting extern "C" declaration. The safe wrappers
//! below re-expose the same function names the rest of the crate has always
//! called; `#[no_mangle]` functions are the Rust-implemented callbacks the
//! C++ side calls back into (their prototypes are generated too).

use std::ffi::c_void;
use std::io::{Read, Write};

// ==================== Opaque C++ types ====================

/// Opaque OCCT handle owned by C++; only ever used behind a pointer.
#[repr(C)]
pub struct TopoDS_Shape {
	_opaque: [u8; 0],
}
#[repr(C)]
pub struct TopoDS_Face {
	_opaque: [u8; 0],
}
#[repr(C)]
pub struct TopoDS_Edge {
	_opaque: [u8; 0],
}
/// Opaque `std::vector<TopoDS_*>` owned by C++.
#[repr(C)]
pub struct ShapeVec {
	_opaque: [u8; 0],
}
#[repr(C)]
pub struct FaceVec {
	_opaque: [u8; 0],
}
#[repr(C)]
pub struct EdgeVec {
	_opaque: [u8; 0],
}

// The topo types are moved (never shared) across threads, which is what
// `Send` permits. `Sync` is intentionally NOT implemented: OCC's
// `Handle<Geom_XXX>` reference counts are non-atomic, so concurrent `&T`
// access across threads would be unsound.
unsafe impl Send for TopoDS_Shape {}
unsafe impl Send for TopoDS_Face {}
unsafe impl Send for TopoDS_Edge {}

// ==================== Raw C declarations ====================
// `bridge()` in build.rs turns this block (and the `#[no_mangle]` callbacks
// below) into OUT_DIR/ffi.h. Doc comments stay here — the generated header
// is read only by the compiler.

mod raw {
	use super::*;

	extern "C" {
		// Ownership: pointers returned by constructors / `*_new` transfer
		// ownership to Rust and are freed with the matching `*_delete`.
		pub fn cadrum_shape_delete(shape: *mut TopoDS_Shape);
		pub fn cadrum_face_delete(face: *mut TopoDS_Face);
		pub fn cadrum_edge_delete(edge: *mut TopoDS_Edge);
		pub fn cadrum_shape_vec_delete(v: *mut ShapeVec);
		pub fn cadrum_face_vec_delete(v: *mut FaceVec);
		pub fn cadrum_edge_vec_delete(v: *mut EdgeVec);

		/// Plain STEP read — only built without FEATURE_COLOR; with color, STEP
		/// goes through XCAF (`cadrum_read_step_color_stream`) instead.
		#[cfg(not(feature = "color"))]
		pub fn cadrum_read_step_stream(reader: *mut c_void) -> *mut TopoDS_Shape;
		#[cfg(not(feature = "color"))]
		pub fn cadrum_write_step_stream(shape: *const TopoDS_Shape, writer: *mut c_void) -> bool;
		/// `out_consumed` = length of the BinTools payload, where Rust's color
		/// trailer begins. Written ONLY on success; untouched when NULL comes back.
		pub fn cadrum_read_brep_stream(data: *const u8, data_len: usize, out_consumed: *mut usize) -> *mut TopoDS_Shape;
		pub fn cadrum_write_brep_stream(shape: *const TopoDS_Shape, writer: *mut c_void) -> bool;

		// Shape constructors. All functions returning a pointer return NULL on failure.
		pub fn cadrum_make_half_space(ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_box(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_cylinder(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, radius: f64, height: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_cone(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64, height: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_torus(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_empty() -> *mut TopoDS_Shape;
		pub fn cadrum_deep_copy(shape: *const TopoDS_Shape) -> *mut TopoDS_Shape;

		// Builders (solid → solid): rebuild topology and append flat
		// [post_id, src_id, ...] face-derivation pairs to `out_history`
		// (a Rust Vec<u64> filled via cadrum_u64_extend).

		/// Evaluate an arbitrary boolean expression on N solids in one
		/// BOPAlgo_CellsBuilder pass. `clauses` は DIMACS-flat DNF (`+i` =
		/// solids\[i-1\] を take、`-i` = avoid、`0` = clause 終端、末尾 0 必須)。
		/// 例: (A ∪ B) − C → solids=\[A,B,C\], clauses=\[1,-3, 0, 2,-3, 0\]
		pub fn cadrum_builder_cells(solids: *const ShapeVec, clauses: *const i64, clauses_len: usize, out_history: *mut c_void) -> *mut TopoDS_Shape;
		/// Unify shared faces / collinear edges via ShapeUpgrade_UnifySameDomain.
		/// `out_history` maps each surviving old face onto the unified result.
		pub fn cadrum_builder_clean(shape: *const TopoDS_Shape, out_history: *mut c_void) -> *mut TopoDS_Shape;
		/// Shell (hollow) the solid by removing `open_faces` and offsetting the
		/// rest by `thickness` (negative hollows inward). NULL on failure
		/// (e.g. self-intersecting offset at sharp corners).
		pub fn cadrum_builder_thick_solid(solid: *const TopoDS_Shape, open_faces: *const FaceVec, thickness: f64, out_history: *mut c_void) -> *mut TopoDS_Shape;
		/// Fillet the given edges with a uniform radius. Empty `edges` is a
		/// no-op (shallow copy). NULL on OCCT failure (radius too large, etc.).
		pub fn cadrum_builder_fillet(solid: *const TopoDS_Shape, edges: *const EdgeVec, radius: f64, out_history: *mut c_void) -> *mut TopoDS_Shape;
		/// Chamfer (symmetric bevel) the given edges with a uniform distance.
		pub fn cadrum_builder_chamfer(solid: *const TopoDS_Shape, edges: *const EdgeVec, distance: f64, out_history: *mut c_void) -> *mut TopoDS_Shape;

		// Transforms: translate/rotate move via TopLoc_Location and preserve
		// TShape*; scale/mirror rebuild topology via BRepBuilderAPI_Transform
		// (no face-derivation table, so the Rust side clears Solid::history).
		pub fn cadrum_transform_translate(shape: *const TopoDS_Shape, tx: f64, ty: f64, tz: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_transform_rotate(shape: *const TopoDS_Shape, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_transform_scale(shape: *const TopoDS_Shape, cx: f64, cy: f64, cz: f64, factor: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_transform_mirror(shape: *const TopoDS_Shape, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> *mut TopoDS_Shape;

		// Shape queries
		pub fn cadrum_shape_is_null(shape: *const TopoDS_Shape) -> bool;
		pub fn cadrum_shape_is_solid(shape: *const TopoDS_Shape) -> bool;
		pub fn cadrum_shape_volume(shape: *const TopoDS_Shape) -> f64;
		pub fn cadrum_shape_surface_area(shape: *const TopoDS_Shape) -> f64;
		pub fn cadrum_shape_center_of_mass(shape: *const TopoDS_Shape, x: *mut f64, y: *mut f64, z: *mut f64);
		pub fn cadrum_shape_inertia_tensor(shape: *const TopoDS_Shape, m00: *mut f64, m01: *mut f64, m02: *mut f64, m10: *mut f64, m11: *mut f64, m12: *mut f64, m20: *mut f64, m21: *mut f64, m22: *mut f64);
		pub fn cadrum_shape_contains_point(shape: *const TopoDS_Shape, x: f64, y: f64, z: f64) -> bool;
		pub fn cadrum_shape_bounding_box(shape: *const TopoDS_Shape, xmin: *mut f64, ymin: *mut f64, zmin: *mut f64, xmax: *mut f64, ymax: *mut f64, zmax: *mut f64);

		// Compound decompose/compose
		pub fn cadrum_decompose_into_solids(shape: *const TopoDS_Shape) -> *mut ShapeVec;
		pub fn cadrum_compound_add(compound: *mut TopoDS_Shape, child: *const TopoDS_Shape);

		/// Triangulate `shape`. On success fills the four Rust Vecs (flat xyz
		/// vertices, per-vertex normals, triangle indices, per-triangle TShape*
		/// ids) and returns true; on failure leaves them untouched.
		pub fn cadrum_mesh_shape(shape: *const TopoDS_Shape, linear: f64, angular: f64, relative: bool, out_vertices: *mut c_void, out_normals: *mut c_void, out_indices: *mut c_void, out_face_ids: *mut c_void) -> bool;

		// Topology enumeration: one-shot listings of unique sub-shapes
		// (shared edges deduplicated). Pointers returned by `*_vec_get` on the
		// results are borrows owned by the parent vector.
		pub fn cadrum_shape_edges(shape: *const TopoDS_Shape) -> *mut EdgeVec;
		pub fn cadrum_shape_faces(shape: *const TopoDS_Shape) -> *mut FaceVec;
		pub fn cadrum_face_edges(face: *const TopoDS_Face) -> *mut EdgeVec;
		// Shallow handle clones — the C++ copy-ctor shares the underlying
		// TShape via OCCT's ref count (deep_copy* create new TShapes).
		pub fn cadrum_clone_shape_handle(shape: *const TopoDS_Shape) -> *mut TopoDS_Shape;
		pub fn cadrum_clone_edge_handle(edge: *const TopoDS_Edge) -> *mut TopoDS_Edge;
		pub fn cadrum_clone_face_handle(face: *const TopoDS_Face) -> *mut TopoDS_Face;

		// TShape* address as u64 — the identity used across boolean history,
		// color maps, and BREP I/O.
		pub fn cadrum_face_tshape_id(face: *const TopoDS_Face) -> u64;
		pub fn cadrum_shape_tshape_id(shape: *const TopoDS_Shape) -> u64;
		pub fn cadrum_edge_tshape_id(edge: *const TopoDS_Edge) -> u64;
		/// Project a 3D point onto `face`: closest point on the trimmed surface
		/// plus the outward normal there (zero vector when undefined). False on
		/// catastrophic OCCT failure.
		pub fn cadrum_face_project_point(face: *const TopoDS_Face, px: f64, py: f64, pz: f64, cpx: *mut f64, cpy: *mut f64, cpz: *mut f64, nx: *mut f64, ny: *mut f64, nz: *mut f64) -> bool;

		/// Approximate an edge as a polyline; fills `out` (Rust Vec<f64>) with
		/// flat xyz triples.
		pub fn cadrum_edge_approximation_segments(edge: *const TopoDS_Edge, linear: f64, angular: f64, relative: bool, out: *mut c_void);
		/// Helical edge on a cylinder at the world origin: `axis` is the
		/// cylinder axis, `x_ref` anchors the local +X (must not be parallel),
		/// rising `height` over `height/pitch` turns.
		pub fn cadrum_make_helix_edge(ax: f64, ay: f64, az: f64, xrx: f64, xry: f64, xrz: f64, radius: f64, pitch: f64, height: f64) -> *mut TopoDS_Edge;
		/// Closed polygon from flat xyz triples (≥3 points), closing edge
		/// included. Failure returns an EMPTY vector, not NULL.
		pub fn cadrum_make_polygon_edges(coords: *const f64, coords_len: usize) -> *mut EdgeVec;
		pub fn cadrum_make_circle_edge(ax: f64, ay: f64, az: f64, radius: f64) -> *mut TopoDS_Edge;
		pub fn cadrum_make_line_edge(ax: f64, ay: f64, az: f64, bx: f64, by: f64, bz: f64) -> *mut TopoDS_Edge;
		/// Circular arc through three points; NULL on collinear input.
		pub fn cadrum_make_arc_edge(sx: f64, sy: f64, sz: f64, mx: f64, my: f64, mz: f64, ex: f64, ey: f64, ez: f64) -> *mut TopoDS_Edge;
		/// Cubic B-spline through flat xyz triples. `end_kind`: 0 = Periodic,
		/// 1 = NotAKnot, 2 = Clamped (start/end tangents in sx..ez; ignored otherwise).
		pub fn cadrum_make_bspline_edge(coords: *const f64, coords_len: usize, end_kind: u32, sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64) -> *mut TopoDS_Edge;
		pub fn cadrum_edge_endpoints(edge: *const TopoDS_Edge, sx: *mut f64, sy: *mut f64, sz: *mut f64, ex: *mut f64, ey: *mut f64, ez: *mut f64);
		pub fn cadrum_edge_tangents(edge: *const TopoDS_Edge, sx: *mut f64, sy: *mut f64, sz: *mut f64, ex: *mut f64, ey: *mut f64, ez: *mut f64);
		pub fn cadrum_edge_is_closed(edge: *const TopoDS_Edge) -> bool;
		pub fn cadrum_edge_project_point(edge: *const TopoDS_Edge, px: f64, py: f64, pz: f64, cpx: *mut f64, cpy: *mut f64, cpz: *mut f64, tx: *mut f64, ty: *mut f64, tz: *mut f64) -> bool;
		pub fn cadrum_deep_copy_edge(edge: *const TopoDS_Edge) -> *mut TopoDS_Edge;
		pub fn cadrum_translate_edge(edge: *const TopoDS_Edge, tx: f64, ty: f64, tz: f64) -> *mut TopoDS_Edge;
		pub fn cadrum_rotate_edge(edge: *const TopoDS_Edge, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> *mut TopoDS_Edge;
		pub fn cadrum_scale_edge(edge: *const TopoDS_Edge, cx: f64, cy: f64, cz: f64, factor: f64) -> *mut TopoDS_Edge;
		pub fn cadrum_mirror_edge(edge: *const TopoDS_Edge, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> *mut TopoDS_Edge;

		/// Extrude a closed profile wire into a solid (Wire → Face → Prism).
		pub fn cadrum_make_extrude(profile_edges: *const EdgeVec, dx: f64, dy: f64, dz: f64) -> *mut TopoDS_Shape;
		/// Sweep profile wires along a spine (BRepOffsetAPI_MakePipeShell);
		/// sections in `all_edges` are separated by null-edge sentinels.
		/// `orient`: 0 = Fixed (spine-start frame), 1 = Torsion (raw Frenet),
		/// 2 = Up (constant binormal ux,uy,uz), 3 = Auxiliary (aux spine);
		/// anything else falls back to Torsion.
		pub fn cadrum_make_pipe_shell(all_edges: *const EdgeVec, spine_edges: *const EdgeVec, orient: u32, ux: f64, uy: f64, uz: f64, aux_spine_edges: *const EdgeVec) -> *mut TopoDS_Shape;
		/// Loft through N section wires (null-edge sentinels); ruled=false gives
		/// a smooth B-spline skin, true straight panels between sections.
		pub fn cadrum_make_loft(all_edges: *const EdgeVec, ruled: bool) -> *mut TopoDS_Shape;
		/// Sew free faces into exactly one closed shell and upgrade to a solid
		/// (oriented for positive volume); gaps, stray faces, or multiple
		/// shells return NULL.
		pub fn cadrum_make_sewn_solid(faces: *const FaceVec, tolerance: f64) -> *mut TopoDS_Shape;
		/// Offset every face by signed `offset` (positive = outward). NULL when
		/// OCCT rejects the offset (self-intersecting result).
		pub fn cadrum_make_offset_shape(shape: *const TopoDS_Shape, offset: f64, tolerance: f64) -> *mut TopoDS_Shape;
		/// B-spline surface solid from a nu×nv point grid (flat xyz, len =
		/// 3*nu*nv). V is always periodic; periodic U gives a torus, open U a
		/// pipe with planar caps.
		pub fn cadrum_make_bspline_solid(coords: *const f64, coords_len: usize, nu: u32, nv: u32, u_periodic: bool) -> *mut TopoDS_Shape;

		// C++ vector helpers: Rust-side construction and iteration of
		// std::vector<TopoDS_*>.
		pub fn cadrum_edge_vec_new() -> *mut EdgeVec;
		pub fn cadrum_edge_vec_push(v: *mut EdgeVec, e: *const TopoDS_Edge);
		/// Push a null edge — the section separator sentinel for pipe shell / loft.
		pub fn cadrum_edge_vec_push_null(v: *mut EdgeVec);
		pub fn cadrum_edge_vec_len(v: *const EdgeVec) -> usize;
		pub fn cadrum_edge_vec_get(v: *const EdgeVec, i: usize) -> *const TopoDS_Edge;
		pub fn cadrum_face_vec_new() -> *mut FaceVec;
		pub fn cadrum_face_vec_push(v: *mut FaceVec, f: *const TopoDS_Face);
		pub fn cadrum_face_vec_len(v: *const FaceVec) -> usize;
		pub fn cadrum_face_vec_get(v: *const FaceVec, i: usize) -> *const TopoDS_Face;
		pub fn cadrum_shape_vec_new() -> *mut ShapeVec;
		pub fn cadrum_shape_vec_push(v: *mut ShapeVec, s: *const TopoDS_Shape);
		pub fn cadrum_shape_vec_len(v: *const ShapeVec) -> usize;
		pub fn cadrum_shape_vec_get(v: *const ShapeVec, i: usize) -> *const TopoDS_Shape;

		/// Colored STEP read via XCAF. `out_ids` (Rust Vec<u64>) = TShape* of
		/// each styled sub-shape (a FACE's or a SOLID's), `out_rgb` (Rust
		/// Vec<f32>) = flat [r,g,b,...] in OCC native space.
		#[cfg(feature = "color")]
		pub fn cadrum_read_step_color_stream(reader: *mut c_void, out_ids: *mut c_void, out_rgb: *mut c_void) -> *mut TopoDS_Shape;
		/// A solid id is written as one styled_item on that solid; a face
		/// style, being the more specific one, overrides it.
		#[cfg(feature = "color")]
		pub fn cadrum_write_step_color_stream(shape: *const TopoDS_Shape, ids: *const u64, ids_len: usize, rgb: *const f32, rgb_len: usize, writer: *mut c_void) -> bool;
	}
}

// ==================== Rust-side callbacks called from C++ ====================
// Written as plain `#[no_mangle]` fns (no macro) so `bridge()` in build.rs can
// parse them with syn and emit their prototypes into the generated ffi.h.
// `vec` points at a Rust `Vec<T>` handed out by the safe wrappers below; the
// pointer never outlives the wrapper call. `len == 0` is skipped because
// `data` may then be null (empty std::vector).

/// Append `len` u32 elements to the Rust `Vec<u32>` behind `vec`.
#[no_mangle]
extern "C" fn cadrum_u32_extend(vec: *mut c_void, data: *const u32, len: usize) {
	if len == 0 {
		return;
	}
	unsafe { (*vec.cast::<Vec<u32>>()).extend_from_slice(std::slice::from_raw_parts(data, len)) };
}

/// Append `len` u64 elements to the Rust `Vec<u64>` behind `vec`.
#[no_mangle]
extern "C" fn cadrum_u64_extend(vec: *mut c_void, data: *const u64, len: usize) {
	if len == 0 {
		return;
	}
	unsafe { (*vec.cast::<Vec<u64>>()).extend_from_slice(std::slice::from_raw_parts(data, len)) };
}

/// Append `len` f32 elements to the Rust `Vec<f32>` behind `vec`.
#[no_mangle]
extern "C" fn cadrum_f32_extend(vec: *mut c_void, data: *const f32, len: usize) {
	if len == 0 {
		return;
	}
	unsafe { (*vec.cast::<Vec<f32>>()).extend_from_slice(std::slice::from_raw_parts(data, len)) };
}

/// Append `len` f64 elements to the Rust `Vec<f64>` behind `vec`.
#[no_mangle]
extern "C" fn cadrum_f64_extend(vec: *mut c_void, data: *const f64, len: usize) {
	if len == 0 {
		return;
	}
	unsafe { (*vec.cast::<Vec<f64>>()).extend_from_slice(std::slice::from_raw_parts(data, len)) };
}

/// Read up to `len` bytes from the `RustReader` behind `reader`.
/// Returns the number of bytes actually read (0 = EOF). Called by the C++
/// `RustReadStreambuf` with an opaque pointer produced by `reader_out`.
#[no_mangle]
extern "C" fn cadrum_reader_read(reader: *mut c_void, buf: *mut u8, len: usize) -> usize {
	let reader = unsafe { &mut *reader.cast::<RustReader>() };
	let buf = unsafe { std::slice::from_raw_parts_mut(buf, len) };
	unsafe { (*reader.inner).read(buf).unwrap_or(0) }
}

/// Write `len` bytes into the `RustWriter` behind `writer`.
/// Returns the number of bytes actually written.
#[no_mangle]
extern "C" fn cadrum_writer_write(writer: *mut c_void, buf: *const u8, len: usize) -> usize {
	let writer = unsafe { &mut *writer.cast::<RustWriter>() };
	let buf = unsafe { std::slice::from_raw_parts(buf, len) };
	unsafe { (*writer.inner).write(buf).unwrap_or(0) }
}

// ==================== Stream wrappers ====================

/// Wrapper around `dyn Read` passed to C++ as an opaque pointer.
///
/// C++ calls `cadrum_reader_read()` to pull bytes from the Rust reader,
/// feeding them into a `std::streambuf` subclass that OCC reads from.
///
/// # Safety
/// The lifetime is erased internally. The caller must ensure the reader
/// outlives the FFI call (which is always the case since C++ calls are
/// synchronous and blocking).
pub struct RustReader {
	inner: *mut dyn Read,
}

impl RustReader {
	/// Create a new RustReader wrapping the given reader.
	///
	/// # Safety
	/// The caller must ensure that the resulting `RustReader` is not used
	/// after `reader` is dropped. In practice, this is guaranteed because
	/// the C++ FFI call is synchronous.
	pub fn from_ref<'a>(reader: &'a mut (dyn Read + 'a)) -> Self {
		// SAFETY: Caller must ensure `reader` outlives this RustReader.
		// The `'static` bound is required by the raw pointer type, so we
		// use transmute to erase the lifetime (lifetimes are compile-time only).
		RustReader { inner: unsafe { std::mem::transmute::<*mut (dyn Read + 'a), *mut (dyn Read + 'static)>(reader as *mut (dyn Read + 'a)) } }
	}
}

/// Wrapper around `dyn Write` passed to C++ as an opaque pointer.
///
/// C++ calls `cadrum_writer_write()` to push bytes into the Rust writer,
/// receiving them from a `std::streambuf` subclass that OCC writes to.
pub struct RustWriter {
	inner: *mut dyn Write,
}

impl RustWriter {
	/// Create a new RustWriter wrapping the given writer.
	///
	/// # Safety
	/// Same as `RustReader::from_ref`.
	pub fn from_ref<'a>(writer: &'a mut (dyn Write + 'a)) -> Self {
		// SAFETY: Caller must ensure `writer` outlives this RustWriter.
		// See RustReader::from_ref for the same rationale.
		RustWriter { inner: unsafe { std::mem::transmute::<*mut (dyn Write + 'a), *mut (dyn Write + 'static)>(writer as *mut (dyn Write + 'a)) } }
	}
}

fn vec_out<T>(v: &mut Vec<T>) -> *mut c_void {
	(v as *mut Vec<T>).cast()
}

fn reader_out(r: &mut RustReader) -> *mut c_void {
	(r as *mut RustReader).cast()
}

fn writer_out(w: &mut RustWriter) -> *mut c_void {
	(w as *mut RustWriter).cast()
}

// ==================== Owning pointer ====================

/// C++ types that Rust owns through [`Ptr`] and frees via a `cadrum_*_delete`.
pub trait Opaque {
	/// # Safety
	/// `raw` must be a pointer obtained from the wrapper's C ABI, not yet freed.
	unsafe fn delete(raw: *mut Self);
}

impl Opaque for TopoDS_Shape {
	unsafe fn delete(raw: *mut Self) {
		raw::cadrum_shape_delete(raw)
	}
}
impl Opaque for TopoDS_Face {
	unsafe fn delete(raw: *mut Self) {
		raw::cadrum_face_delete(raw)
	}
}
impl Opaque for TopoDS_Edge {
	unsafe fn delete(raw: *mut Self) {
		raw::cadrum_edge_delete(raw)
	}
}
impl Opaque for ShapeVec {
	unsafe fn delete(raw: *mut Self) {
		raw::cadrum_shape_vec_delete(raw)
	}
}
impl Opaque for FaceVec {
	unsafe fn delete(raw: *mut Self) {
		raw::cadrum_face_vec_delete(raw)
	}
}
impl Opaque for EdgeVec {
	unsafe fn delete(raw: *mut Self) {
		raw::cadrum_edge_vec_delete(raw)
	}
}

/// Owning, nullable pointer to a C++ object (the crate's `cxx::UniquePtr`
/// replacement). Deref panics on null, matching `UniquePtr`'s behavior;
/// callers null-check with [`Ptr::is_null`] first.
pub struct Ptr<T: Opaque> {
	raw: *mut T,
}

impl<T: Opaque> Ptr<T> {
	/// # Safety
	/// `raw` must be null or an owned pointer from the wrapper's C ABI.
	unsafe fn from_raw(raw: *mut T) -> Self {
		Ptr { raw }
	}

	pub fn is_null(&self) -> bool {
		self.raw.is_null()
	}
}

impl<T: Opaque> Drop for Ptr<T> {
	fn drop(&mut self) {
		if !self.raw.is_null() {
			unsafe { T::delete(self.raw) }
		}
	}
}

impl<T: Opaque> std::ops::Deref for Ptr<T> {
	type Target = T;
	fn deref(&self) -> &T {
		assert!(!self.raw.is_null(), "deref of null Ptr");
		unsafe { &*self.raw }
	}
}

impl<T: Opaque> std::ops::DerefMut for Ptr<T> {
	fn deref_mut(&mut self) -> &mut T {
		assert!(!self.raw.is_null(), "deref of null Ptr");
		unsafe { &mut *self.raw }
	}
}

// Moving a Ptr to another thread is fine (exclusive ownership, no aliasing);
// the raw pointer field keeps `Ptr` `!Sync`, preserving the old `UniquePtr`
// thread-safety profile.
unsafe impl<T: Opaque + Send> Send for Ptr<T> {}

// ==================== C++ vector views ====================

macro_rules! vec_view {
	($vec:ty, $elem:ty, $len:path, $get:path) => {
		impl $vec {
			pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a $elem> {
				(0..unsafe { $len(self) }).map(move |i| unsafe { &*$get(self, i) })
			}
		}
	};
}

vec_view!(ShapeVec, TopoDS_Shape, raw::cadrum_shape_vec_len, raw::cadrum_shape_vec_get);
vec_view!(FaceVec, TopoDS_Face, raw::cadrum_face_vec_len, raw::cadrum_face_vec_get);
vec_view!(EdgeVec, TopoDS_Edge, raw::cadrum_edge_vec_len, raw::cadrum_edge_vec_get);

impl EdgeVec {
	pub fn is_empty(&self) -> bool {
		unsafe { raw::cadrum_edge_vec_len(self) == 0 }
	}
}

// ==================== Safe wrappers ====================
// Same names and shapes as the old cxx bridge so call sites stay unchanged.

// Mesh data returned from C++.
pub struct MeshData {
	pub vertices: Vec<f64>, // flat xyz
	pub normals: Vec<f64>,  // flat xyz, one per vertex
	pub indices: Vec<u32>,
	pub face_tshape_ids: Vec<u64>, // per-triangle TShape* address
	pub success: bool,
}

// ==================== Shape I/O (streambuf callback) ====================

#[cfg(not(feature = "color"))]
pub fn read_step_stream(reader: &mut RustReader) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_read_step_stream(reader_out(reader))) }
}

#[cfg(not(feature = "color"))]
pub fn write_step_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool {
	unsafe { raw::cadrum_write_step_stream(shape, writer_out(writer)) }
}

/// `out_consumed` = payload length, where the color trailer begins. Written
/// only when the returned pointer is non-null.
pub fn read_brep_stream(data: &[u8], out_consumed: &mut usize) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_read_brep_stream(data.as_ptr(), data.len(), out_consumed)) }
}

pub fn write_brep_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool {
	unsafe { raw::cadrum_write_brep_stream(shape, writer_out(writer)) }
}

// ==================== Shape Constructors ====================

pub fn make_half_space(ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_half_space(ox, oy, oz, nx, ny, nz)) }
}

pub fn make_box(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_box(x1, y1, z1, x2, y2, z2)) }
}

pub fn make_cylinder(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, radius: f64, height: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_cylinder(px, py, pz, dx, dy, dz, radius, height)) }
}

pub fn make_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_sphere(cx, cy, cz, radius)) }
}

pub fn make_cone(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64, height: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_cone(px, py, pz, dx, dy, dz, r1, r2, height)) }
}

pub fn make_torus(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_torus(px, py, pz, dx, dy, dz, r1, r2)) }
}

pub fn make_empty() -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_empty()) }
}

pub fn deep_copy(shape: &TopoDS_Shape) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_deep_copy(shape)) }
}

// ==================== Colored STEP I/O (color feature only) ====================

#[cfg(feature = "color")]
pub fn read_step_color_stream(reader: &mut RustReader, out_ids: &mut Vec<u64>, out_rgb: &mut Vec<f32>) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_read_step_color_stream(reader_out(reader), vec_out(out_ids), vec_out(out_rgb))) }
}

#[cfg(feature = "color")]
pub fn write_step_color_stream(shape: &TopoDS_Shape, ids: &[u64], rgb: &[f32], writer: &mut RustWriter) -> bool {
	unsafe { raw::cadrum_write_step_color_stream(shape, ids.as_ptr(), ids.len(), rgb.as_ptr(), rgb.len(), writer_out(writer)) }
}

// ==================== Builders (solid → solid with history) ====================

// Evaluate any boolean expression on N solids via BOPAlgo_CellsBuilder.
// `clauses` は DIMACS-flat DNF (`+i` = solids[i-1] を take、`-i` = avoid、`0` = clause 終端)。
pub fn builder_cells(solids: &ShapeVec, clauses: &[i64], out_history: &mut Vec<u64>) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_builder_cells(solids, clauses.as_ptr(), clauses.len(), vec_out(out_history))) }
}

// Unify shared faces. `out_history` receives flat [new_id, old_id, ...] pairs,
// used by Solid::clean to populate `Solid::history` and remap the colormap.
pub fn builder_clean(shape: &TopoDS_Shape, out_history: &mut Vec<u64>) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_builder_clean(shape, vec_out(out_history))) }
}

pub fn builder_thick_solid(solid: &TopoDS_Shape, open_faces: &FaceVec, thickness: f64, out_history: &mut Vec<u64>) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_builder_thick_solid(solid, open_faces, thickness, vec_out(out_history))) }
}

pub fn builder_fillet(solid: &TopoDS_Shape, edges: &EdgeVec, radius: f64, out_history: &mut Vec<u64>) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_builder_fillet(solid, edges, radius, vec_out(out_history))) }
}

pub fn builder_chamfer(solid: &TopoDS_Shape, edges: &EdgeVec, distance: f64, out_history: &mut Vec<u64>) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_builder_chamfer(solid, edges, distance, vec_out(out_history))) }
}

// ==================== Transforms (solid → solid, no history) ====================

pub fn transform_translate(shape: &TopoDS_Shape, tx: f64, ty: f64, tz: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_transform_translate(shape, tx, ty, tz)) }
}

pub fn transform_rotate(shape: &TopoDS_Shape, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_transform_rotate(shape, ox, oy, oz, dx, dy, dz, angle)) }
}

pub fn transform_scale(shape: &TopoDS_Shape, cx: f64, cy: f64, cz: f64, factor: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_transform_scale(shape, cx, cy, cz, factor)) }
}

pub fn transform_mirror(shape: &TopoDS_Shape, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_transform_mirror(shape, ox, oy, oz, nx, ny, nz)) }
}

// ==================== Shape Queries ====================

pub fn shape_is_null(shape: &TopoDS_Shape) -> bool {
	unsafe { raw::cadrum_shape_is_null(shape) }
}

pub fn shape_is_solid(shape: &TopoDS_Shape) -> bool {
	unsafe { raw::cadrum_shape_is_solid(shape) }
}

pub fn shape_volume(shape: &TopoDS_Shape) -> f64 {
	unsafe { raw::cadrum_shape_volume(shape) }
}

pub fn shape_surface_area(shape: &TopoDS_Shape) -> f64 {
	unsafe { raw::cadrum_shape_surface_area(shape) }
}

pub fn shape_center_of_mass(shape: &TopoDS_Shape, x: &mut f64, y: &mut f64, z: &mut f64) {
	unsafe { raw::cadrum_shape_center_of_mass(shape, x, y, z) }
}

#[allow(clippy::too_many_arguments)]
pub fn shape_inertia_tensor(shape: &TopoDS_Shape, m00: &mut f64, m01: &mut f64, m02: &mut f64, m10: &mut f64, m11: &mut f64, m12: &mut f64, m20: &mut f64, m21: &mut f64, m22: &mut f64) {
	unsafe { raw::cadrum_shape_inertia_tensor(shape, m00, m01, m02, m10, m11, m12, m20, m21, m22) }
}

pub fn shape_contains_point(shape: &TopoDS_Shape, x: f64, y: f64, z: f64) -> bool {
	unsafe { raw::cadrum_shape_contains_point(shape, x, y, z) }
}

pub fn shape_bounding_box(shape: &TopoDS_Shape, xmin: &mut f64, ymin: &mut f64, zmin: &mut f64, xmax: &mut f64, ymax: &mut f64, zmax: &mut f64) {
	unsafe { raw::cadrum_shape_bounding_box(shape, xmin, ymin, zmin, xmax, ymax, zmax) }
}

// ==================== Compound Decompose/Compose ====================

pub fn decompose_into_solids(shape: &TopoDS_Shape) -> Ptr<ShapeVec> {
	unsafe { Ptr::from_raw(raw::cadrum_decompose_into_solids(shape)) }
}

pub fn compound_add(compound: &mut TopoDS_Shape, child: &TopoDS_Shape) {
	unsafe { raw::cadrum_compound_add(compound, child) }
}

// ==================== Meshing ====================

pub fn mesh_shape(shape: &TopoDS_Shape, linear: f64, angular: f64, relative: bool) -> MeshData {
	let mut data = MeshData { vertices: Vec::new(), normals: Vec::new(), indices: Vec::new(), face_tshape_ids: Vec::new(), success: false };
	data.success = unsafe { raw::cadrum_mesh_shape(shape, linear, angular, relative, vec_out(&mut data.vertices), vec_out(&mut data.normals), vec_out(&mut data.indices), vec_out(&mut data.face_tshape_ids)) };
	data
}

// ==================== Topology enumeration ====================

pub fn shape_edges(shape: &TopoDS_Shape) -> Ptr<EdgeVec> {
	unsafe { Ptr::from_raw(raw::cadrum_shape_edges(shape)) }
}

pub fn shape_faces(shape: &TopoDS_Shape) -> Ptr<FaceVec> {
	unsafe { Ptr::from_raw(raw::cadrum_shape_faces(shape)) }
}

pub fn face_edges(face: &TopoDS_Face) -> Ptr<EdgeVec> {
	unsafe { Ptr::from_raw(raw::cadrum_face_edges(face)) }
}

pub fn clone_shape_handle(shape: &TopoDS_Shape) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_clone_shape_handle(shape)) }
}

pub fn clone_edge_handle(edge: &TopoDS_Edge) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_clone_edge_handle(edge)) }
}

pub fn clone_face_handle(face: &TopoDS_Face) -> Ptr<TopoDS_Face> {
	unsafe { Ptr::from_raw(raw::cadrum_clone_face_handle(face)) }
}

// ==================== Face Methods ====================

pub fn face_tshape_id(face: &TopoDS_Face) -> u64 {
	unsafe { raw::cadrum_face_tshape_id(face) }
}

pub fn shape_tshape_id(shape: &TopoDS_Shape) -> u64 {
	unsafe { raw::cadrum_shape_tshape_id(shape) }
}

pub fn edge_tshape_id(edge: &TopoDS_Edge) -> u64 {
	unsafe { raw::cadrum_edge_tshape_id(edge) }
}

#[allow(clippy::too_many_arguments)]
pub fn face_project_point(face: &TopoDS_Face, px: f64, py: f64, pz: f64, cpx: &mut f64, cpy: &mut f64, cpz: &mut f64, nx: &mut f64, ny: &mut f64, nz: &mut f64) -> bool {
	unsafe { raw::cadrum_face_project_point(face, px, py, pz, cpx, cpy, cpz, nx, ny, nz) }
}

// ==================== Edge Methods ====================

pub fn edge_approximation_segments(edge: &TopoDS_Edge, linear: f64, angular: f64, relative: bool) -> Vec<f64> {
	let mut out = Vec::new();
	unsafe { raw::cadrum_edge_approximation_segments(edge, linear, angular, relative, vec_out(&mut out)) };
	out
}

pub fn make_helix_edge(ax: f64, ay: f64, az: f64, xrx: f64, xry: f64, xrz: f64, radius: f64, pitch: f64, height: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_make_helix_edge(ax, ay, az, xrx, xry, xrz, radius, pitch, height)) }
}

pub fn make_polygon_edges(coords: &[f64]) -> Ptr<EdgeVec> {
	unsafe { Ptr::from_raw(raw::cadrum_make_polygon_edges(coords.as_ptr(), coords.len())) }
}

pub fn make_circle_edge(ax: f64, ay: f64, az: f64, radius: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_make_circle_edge(ax, ay, az, radius)) }
}

pub fn make_line_edge(ax: f64, ay: f64, az: f64, bx: f64, by: f64, bz: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_make_line_edge(ax, ay, az, bx, by, bz)) }
}

pub fn make_arc_edge(sx: f64, sy: f64, sz: f64, mx: f64, my: f64, mz: f64, ex: f64, ey: f64, ez: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_make_arc_edge(sx, sy, sz, mx, my, mz, ex, ey, ez)) }
}

#[allow(clippy::too_many_arguments)]
pub fn make_bspline_edge(coords: &[f64], end_kind: u32, sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_make_bspline_edge(coords.as_ptr(), coords.len(), end_kind, sx, sy, sz, ex, ey, ez)) }
}

pub fn edge_endpoints(edge: &TopoDS_Edge, sx: &mut f64, sy: &mut f64, sz: &mut f64, ex: &mut f64, ey: &mut f64, ez: &mut f64) {
	unsafe { raw::cadrum_edge_endpoints(edge, sx, sy, sz, ex, ey, ez) }
}

pub fn edge_tangents(edge: &TopoDS_Edge, sx: &mut f64, sy: &mut f64, sz: &mut f64, ex: &mut f64, ey: &mut f64, ez: &mut f64) {
	unsafe { raw::cadrum_edge_tangents(edge, sx, sy, sz, ex, ey, ez) }
}

pub fn edge_is_closed(edge: &TopoDS_Edge) -> bool {
	unsafe { raw::cadrum_edge_is_closed(edge) }
}

#[allow(clippy::too_many_arguments)]
pub fn edge_project_point(edge: &TopoDS_Edge, px: f64, py: f64, pz: f64, cpx: &mut f64, cpy: &mut f64, cpz: &mut f64, tx: &mut f64, ty: &mut f64, tz: &mut f64) -> bool {
	unsafe { raw::cadrum_edge_project_point(edge, px, py, pz, cpx, cpy, cpz, tx, ty, tz) }
}

pub fn deep_copy_edge(edge: &TopoDS_Edge) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_deep_copy_edge(edge)) }
}

pub fn translate_edge(edge: &TopoDS_Edge, tx: f64, ty: f64, tz: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_translate_edge(edge, tx, ty, tz)) }
}

pub fn rotate_edge(edge: &TopoDS_Edge, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_rotate_edge(edge, ox, oy, oz, dx, dy, dz, angle)) }
}

pub fn scale_edge(edge: &TopoDS_Edge, cx: f64, cy: f64, cz: f64, factor: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_scale_edge(edge, cx, cy, cz, factor)) }
}

pub fn mirror_edge(edge: &TopoDS_Edge, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> Ptr<TopoDS_Edge> {
	unsafe { Ptr::from_raw(raw::cadrum_mirror_edge(edge, ox, oy, oz, nx, ny, nz)) }
}

// ==================== Sweeps / lofts / offsets ====================

pub fn make_extrude(profile_edges: &EdgeVec, dx: f64, dy: f64, dz: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_extrude(profile_edges, dx, dy, dz)) }
}

pub fn make_pipe_shell(all_edges: &EdgeVec, spine_edges: &EdgeVec, orient: u32, ux: f64, uy: f64, uz: f64, aux_spine_edges: &EdgeVec) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_pipe_shell(all_edges, spine_edges, orient, ux, uy, uz, aux_spine_edges)) }
}

pub fn make_loft(all_edges: &EdgeVec, ruled: bool) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_loft(all_edges, ruled)) }
}

pub fn make_sewn_solid(faces: &FaceVec, tolerance: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_sewn_solid(faces, tolerance)) }
}

pub fn make_offset_shape(shape: &TopoDS_Shape, offset: f64, tolerance: f64) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_offset_shape(shape, offset, tolerance)) }
}

pub fn make_bspline_solid(coords: &[f64], nu: u32, nv: u32, u_periodic: bool) -> Ptr<TopoDS_Shape> {
	unsafe { Ptr::from_raw(raw::cadrum_make_bspline_solid(coords.as_ptr(), coords.len(), nu, nv, u_periodic)) }
}

// ==================== C++ vector helpers ====================

pub fn edge_vec_new() -> Ptr<EdgeVec> {
	unsafe { Ptr::from_raw(raw::cadrum_edge_vec_new()) }
}

pub fn edge_vec_push(v: &mut EdgeVec, e: &TopoDS_Edge) {
	unsafe { raw::cadrum_edge_vec_push(v, e) }
}

pub fn edge_vec_push_null(v: &mut EdgeVec) {
	unsafe { raw::cadrum_edge_vec_push_null(v) }
}

pub fn face_vec_new() -> Ptr<FaceVec> {
	unsafe { Ptr::from_raw(raw::cadrum_face_vec_new()) }
}

pub fn face_vec_push(v: &mut FaceVec, f: &TopoDS_Face) {
	unsafe { raw::cadrum_face_vec_push(v, f) }
}

pub fn shape_vec_new() -> Ptr<ShapeVec> {
	unsafe { Ptr::from_raw(raw::cadrum_shape_vec_new()) }
}

pub fn shape_vec_push(v: &mut ShapeVec, s: &TopoDS_Shape) {
	unsafe { raw::cadrum_shape_vec_push(v, s) }
}
