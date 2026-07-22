//! C ABI bindings for `cpp/wrapper.h`.
//!
//! The raw declarations in [`raw`] mirror the pure-C header one-to-one
//! (bindgen-equivalent output, committed instead of generated so consumers
//! need neither cxx nor a build-time bindgen/libclang install). The safe
//! wrappers below re-expose the same function names the rest of the crate
//! has always called.

use super::stream::{RustReader, RustWriter};
use std::ffi::c_void;

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

// ==================== Raw C declarations (kept in sync with cpp/wrapper.h) ====================

mod raw {
	use super::*;

	extern "C" {
		// Ownership
		pub fn cadrum_shape_delete(shape: *mut TopoDS_Shape);
		pub fn cadrum_face_delete(face: *mut TopoDS_Face);
		pub fn cadrum_edge_delete(edge: *mut TopoDS_Edge);
		pub fn cadrum_shape_vec_delete(v: *mut ShapeVec);
		pub fn cadrum_face_vec_delete(v: *mut FaceVec);
		pub fn cadrum_edge_vec_delete(v: *mut EdgeVec);

		// Shape I/O
		#[cfg(not(feature = "color"))]
		pub fn cadrum_read_step_stream(reader: *mut c_void) -> *mut TopoDS_Shape;
		#[cfg(not(feature = "color"))]
		pub fn cadrum_write_step_stream(shape: *const TopoDS_Shape, writer: *mut c_void) -> bool;
		pub fn cadrum_read_brep_stream(data: *const u8, data_len: usize, out_consumed: *mut usize) -> *mut TopoDS_Shape;
		pub fn cadrum_write_brep_stream(shape: *const TopoDS_Shape, writer: *mut c_void) -> bool;

		// Shape constructors
		pub fn cadrum_make_half_space(ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_box(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_cylinder(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, radius: f64, height: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_cone(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64, height: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_torus(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_empty() -> *mut TopoDS_Shape;
		pub fn cadrum_deep_copy(shape: *const TopoDS_Shape) -> *mut TopoDS_Shape;

		// Builders
		pub fn cadrum_builder_cells(solids: *const ShapeVec, clauses: *const i64, clauses_len: usize, out_history: *mut c_void) -> *mut TopoDS_Shape;
		pub fn cadrum_builder_clean(shape: *const TopoDS_Shape, out_history: *mut c_void) -> *mut TopoDS_Shape;
		pub fn cadrum_builder_thick_solid(solid: *const TopoDS_Shape, open_faces: *const FaceVec, thickness: f64, out_history: *mut c_void) -> *mut TopoDS_Shape;
		pub fn cadrum_builder_fillet(solid: *const TopoDS_Shape, edges: *const EdgeVec, radius: f64, out_history: *mut c_void) -> *mut TopoDS_Shape;
		pub fn cadrum_builder_chamfer(solid: *const TopoDS_Shape, edges: *const EdgeVec, distance: f64, out_history: *mut c_void) -> *mut TopoDS_Shape;

		// Transforms
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

		// Meshing
		pub fn cadrum_mesh_shape(shape: *const TopoDS_Shape, linear: f64, angular: f64, relative: bool, out_vertices: *mut c_void, out_normals: *mut c_void, out_indices: *mut c_void, out_face_ids: *mut c_void) -> bool;

		// Topology enumeration
		pub fn cadrum_shape_edges(shape: *const TopoDS_Shape) -> *mut EdgeVec;
		pub fn cadrum_shape_faces(shape: *const TopoDS_Shape) -> *mut FaceVec;
		pub fn cadrum_face_edges(face: *const TopoDS_Face) -> *mut EdgeVec;
		pub fn cadrum_clone_shape_handle(shape: *const TopoDS_Shape) -> *mut TopoDS_Shape;
		pub fn cadrum_clone_edge_handle(edge: *const TopoDS_Edge) -> *mut TopoDS_Edge;
		pub fn cadrum_clone_face_handle(face: *const TopoDS_Face) -> *mut TopoDS_Face;

		// Face methods
		pub fn cadrum_face_tshape_id(face: *const TopoDS_Face) -> u64;
		pub fn cadrum_shape_tshape_id(shape: *const TopoDS_Shape) -> u64;
		pub fn cadrum_edge_tshape_id(edge: *const TopoDS_Edge) -> u64;
		pub fn cadrum_face_project_point(face: *const TopoDS_Face, px: f64, py: f64, pz: f64, cpx: *mut f64, cpy: *mut f64, cpz: *mut f64, nx: *mut f64, ny: *mut f64, nz: *mut f64) -> bool;

		// Edge methods
		pub fn cadrum_edge_approximation_segments(edge: *const TopoDS_Edge, linear: f64, angular: f64, relative: bool, out: *mut c_void);
		pub fn cadrum_make_helix_edge(ax: f64, ay: f64, az: f64, xrx: f64, xry: f64, xrz: f64, radius: f64, pitch: f64, height: f64) -> *mut TopoDS_Edge;
		pub fn cadrum_make_polygon_edges(coords: *const f64, coords_len: usize) -> *mut EdgeVec;
		pub fn cadrum_make_circle_edge(ax: f64, ay: f64, az: f64, radius: f64) -> *mut TopoDS_Edge;
		pub fn cadrum_make_line_edge(ax: f64, ay: f64, az: f64, bx: f64, by: f64, bz: f64) -> *mut TopoDS_Edge;
		pub fn cadrum_make_arc_edge(sx: f64, sy: f64, sz: f64, mx: f64, my: f64, mz: f64, ex: f64, ey: f64, ez: f64) -> *mut TopoDS_Edge;
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

		// Sweeps / lofts / offsets
		pub fn cadrum_make_extrude(profile_edges: *const EdgeVec, dx: f64, dy: f64, dz: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_pipe_shell(all_edges: *const EdgeVec, spine_edges: *const EdgeVec, orient: u32, ux: f64, uy: f64, uz: f64, aux_spine_edges: *const EdgeVec) -> *mut TopoDS_Shape;
		pub fn cadrum_make_loft(all_edges: *const EdgeVec, ruled: bool) -> *mut TopoDS_Shape;
		pub fn cadrum_make_sewn_solid(faces: *const FaceVec, tolerance: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_offset_shape(shape: *const TopoDS_Shape, offset: f64, tolerance: f64) -> *mut TopoDS_Shape;
		pub fn cadrum_make_bspline_solid(coords: *const f64, coords_len: usize, nu: u32, nv: u32, u_periodic: bool) -> *mut TopoDS_Shape;

		// C++ vector helpers
		pub fn cadrum_edge_vec_new() -> *mut EdgeVec;
		pub fn cadrum_edge_vec_push(v: *mut EdgeVec, e: *const TopoDS_Edge);
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

		// Colored STEP I/O
		#[cfg(feature = "color")]
		pub fn cadrum_read_step_color_stream(reader: *mut c_void, out_ids: *mut c_void, out_rgb: *mut c_void) -> *mut TopoDS_Shape;
		#[cfg(feature = "color")]
		pub fn cadrum_write_step_color_stream(shape: *const TopoDS_Shape, ids: *const u64, ids_len: usize, rgb: *const f32, rgb_len: usize, writer: *mut c_void) -> bool;
	}
}

// ==================== Rust-side callbacks called from C++ ====================

/// Append `len` elements to the Rust `Vec<T>` behind the opaque `vec` pointer.
/// C++ shims pass the address of a `Vec<T>` handed out by the safe wrappers
/// below; the pointer never outlives the wrapper call.
macro_rules! extend_callback {
	($name:ident, $t:ty) => {
		#[no_mangle]
		extern "C" fn $name(vec: *mut c_void, data: *const $t, len: usize) {
			if len == 0 {
				return;
			}
			unsafe { (*vec.cast::<Vec<$t>>()).extend_from_slice(std::slice::from_raw_parts(data, len)) };
		}
	};
}

extend_callback!(cadrum_u32_extend, u32);
extend_callback!(cadrum_u64_extend, u64);
extend_callback!(cadrum_f32_extend, f32);
extend_callback!(cadrum_f64_extend, f64);

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
