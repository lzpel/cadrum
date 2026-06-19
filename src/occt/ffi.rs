//! Hand-written FFI to the OCCT C++ wrapper (`cpp/wrapper.cpp` via `cpp/ffi.h`).
//!
//! This replaces the former `#[cxx::bridge]`. The C++ side keeps its OCCT logic
//! in `namespace cadrum` and exposes a thin `extern "C"` shim; here we declare
//! the matching raw signatures (`mod raw`) and wrap them in safe-ish functions
//! that mirror the old bridge API so the rest of `src/occt/` is mostly
//! unchanged. Owned C++ objects are held by [`Owned`] (a `UniquePtr`-like smart
//! pointer); variable-length results are copied out of malloc'd buffers and the
//! buffers freed via `cadrum_free`.

use super::stream::{CReader, CWriter, RustReader, RustWriter};
use std::ffi::c_void;

// ==================== Opaque OCCT topology types ====================
//
// Only ever used behind pointers. The zero-size + PhantomData idiom makes them
// `!Send`/`!Sync`/`!Unpin` and impossible to construct on the Rust side.

#[repr(C)]
pub struct TopoDS_Shape {
	_data: [u8; 0],
	_marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[repr(C)]
pub struct TopoDS_Face {
	_data: [u8; 0],
	_marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[repr(C)]
pub struct TopoDS_Edge {
	_data: [u8; 0],
	_marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

/// Per-type deallocation (C++ `delete`). Implemented for the three opaque types.
pub trait OcctFree {
	/// # Safety
	/// `ptr` must be a non-null pointer obtained from the C++ side and not yet freed.
	unsafe fn occt_free(ptr: *mut Self);
}

impl OcctFree for TopoDS_Shape {
	unsafe fn occt_free(ptr: *mut Self) {
		raw::cadrum_shape_free(ptr)
	}
}
impl OcctFree for TopoDS_Face {
	unsafe fn occt_free(ptr: *mut Self) {
		raw::cadrum_face_free(ptr)
	}
}
impl OcctFree for TopoDS_Edge {
	unsafe fn occt_free(ptr: *mut Self) {
		raw::cadrum_edge_free(ptr)
	}
}

/// Owned C++ handle — the hand-written replacement for `cxx::UniquePtr<T>`.
///
/// Holds a raw `*mut T` returned by the wrapper and frees it on drop via the
/// matching `cadrum_*_free`. `Deref` yields `&T` (callers rely on deref coercion
/// when passing `&Owned<T>` to functions taking `&T`); `is_null` checks the
/// Rust-level pointer (the FFI returns null on failure).
pub struct Owned<T: OcctFree> {
	ptr: *mut T,
}

impl<T: OcctFree> Owned<T> {
	/// # Safety
	/// `ptr` is either null or a valid owning pointer from the wrapper.
	pub(crate) unsafe fn from_raw(ptr: *mut T) -> Self {
		Owned { ptr }
	}

	pub fn is_null(&self) -> bool {
		self.ptr.is_null()
	}

	pub(crate) fn as_ptr(&self) -> *const T {
		self.ptr as *const T
	}

	pub(crate) fn as_mut_ptr(&mut self) -> *mut T {
		self.ptr
	}
}

impl<T: OcctFree> Drop for Owned<T> {
	fn drop(&mut self) {
		if !self.ptr.is_null() {
			unsafe { T::occt_free(self.ptr) }
		}
	}
}

impl<T: OcctFree> std::ops::Deref for Owned<T> {
	type Target = T;
	fn deref(&self) -> &T {
		// SAFETY: only ever called on a non-null handle — callers check
		// `is_null()` on FFI returns before any deref (matching the previous
		// `cxx::UniquePtr` contract).
		unsafe { &*self.ptr }
	}
}

// Mirrors the previous `unsafe impl Send for TopoDS_*`: an `Owned` handle has
// exclusive ownership and may be *moved* across threads. `Sync` is intentionally
// absent (raw pointer; OCC handle refcounts are non-atomic).
unsafe impl<T: OcctFree> Send for Owned<T> {}

// ==================== Mesh data ====================

/// Triangulated mesh returned by [`mesh_shape`]. Plain Rust struct (the wrapper
/// fills POD arrays which are copied in here).
pub struct MeshData {
	pub vertices: Vec<f64>, // flat xyz
	pub uvs: Vec<f64>,      // flat uv
	pub indices: Vec<u32>,
	pub face_tshape_ids: Vec<u64>, // per-triangle TShape* address
	pub success: bool,
}

/// POD mirror of `CMeshData` in `cpp/ffi.h`.
#[repr(C)]
struct CMeshData {
	vertices: *mut f64,
	vertices_len: usize,
	uvs: *mut f64,
	uvs_len: usize,
	indices: *mut u32,
	indices_len: usize,
	face_ids: *mut u64,
	face_ids_len: usize,
	success: bool,
}

// ==================== Raw extern "C" declarations ====================

mod raw {
	use super::{CMeshData, CReader, CWriter, TopoDS_Edge, TopoDS_Face, TopoDS_Shape};
	use std::ffi::c_void;

	extern "C" {
		pub fn cadrum_free(p: *mut c_void);
		pub fn cadrum_shape_free(p: *mut TopoDS_Shape);
		pub fn cadrum_face_free(p: *mut TopoDS_Face);
		pub fn cadrum_edge_free(p: *mut TopoDS_Edge);

		#[cfg(not(feature = "color"))]
		pub fn read_step_stream(reader: *const CReader) -> *mut TopoDS_Shape;
		#[cfg(not(feature = "color"))]
		pub fn write_step_stream(shape: *const TopoDS_Shape, writer: *const CWriter) -> bool;
		pub fn read_brep_bin_stream(reader: *const CReader) -> *mut TopoDS_Shape;
		pub fn write_brep_bin_stream(shape: *const TopoDS_Shape, writer: *const CWriter) -> bool;
		pub fn read_brep_text_stream(reader: *const CReader) -> *mut TopoDS_Shape;
		pub fn write_brep_text_stream(shape: *const TopoDS_Shape, writer: *const CWriter) -> bool;

		pub fn make_half_space(ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> *mut TopoDS_Shape;
		pub fn make_box(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> *mut TopoDS_Shape;
		pub fn make_cylinder(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, radius: f64, height: f64) -> *mut TopoDS_Shape;
		pub fn make_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> *mut TopoDS_Shape;
		pub fn make_cone(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64, height: f64) -> *mut TopoDS_Shape;
		pub fn make_torus(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64) -> *mut TopoDS_Shape;
		pub fn make_empty() -> *mut TopoDS_Shape;
		pub fn deep_copy(shape: *const TopoDS_Shape) -> *mut TopoDS_Shape;

		pub fn builder_cells(solids: *const *const TopoDS_Shape, n_solids: usize, clauses: *const i64, n_clauses: usize, out_history: *mut *mut u64, out_history_len: *mut usize) -> *mut TopoDS_Shape;
		pub fn builder_clean(shape: *const TopoDS_Shape, out_history: *mut *mut u64, out_history_len: *mut usize) -> *mut TopoDS_Shape;
		pub fn builder_thick_solid(solid: *const TopoDS_Shape, open_faces: *const *const TopoDS_Face, n_faces: usize, thickness: f64, out_history: *mut *mut u64, out_history_len: *mut usize) -> *mut TopoDS_Shape;
		pub fn builder_fillet(solid: *const TopoDS_Shape, edges: *const *const TopoDS_Edge, n_edges: usize, radius: f64, out_history: *mut *mut u64, out_history_len: *mut usize) -> *mut TopoDS_Shape;
		pub fn builder_chamfer(solid: *const TopoDS_Shape, edges: *const *const TopoDS_Edge, n_edges: usize, distance: f64, out_history: *mut *mut u64, out_history_len: *mut usize) -> *mut TopoDS_Shape;

		pub fn transform_translate(shape: *const TopoDS_Shape, tx: f64, ty: f64, tz: f64) -> *mut TopoDS_Shape;
		pub fn transform_rotate(shape: *const TopoDS_Shape, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> *mut TopoDS_Shape;
		pub fn transform_scale(shape: *const TopoDS_Shape, cx: f64, cy: f64, cz: f64, factor: f64) -> *mut TopoDS_Shape;
		pub fn transform_mirror(shape: *const TopoDS_Shape, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> *mut TopoDS_Shape;

		pub fn shape_is_null(shape: *const TopoDS_Shape) -> bool;
		pub fn shape_is_solid(shape: *const TopoDS_Shape) -> bool;
		pub fn shape_volume(shape: *const TopoDS_Shape) -> f64;
		pub fn shape_surface_area(shape: *const TopoDS_Shape) -> f64;
		pub fn shape_center_of_mass(shape: *const TopoDS_Shape, x: *mut f64, y: *mut f64, z: *mut f64);
		pub fn shape_inertia_tensor(shape: *const TopoDS_Shape, m00: *mut f64, m01: *mut f64, m02: *mut f64, m10: *mut f64, m11: *mut f64, m12: *mut f64, m20: *mut f64, m21: *mut f64, m22: *mut f64);
		pub fn shape_contains_point(shape: *const TopoDS_Shape, x: f64, y: f64, z: f64) -> bool;
		pub fn shape_bounding_box(shape: *const TopoDS_Shape, xmin: *mut f64, ymin: *mut f64, zmin: *mut f64, xmax: *mut f64, ymax: *mut f64, zmax: *mut f64);

		pub fn decompose_into_solids(shape: *const TopoDS_Shape, out_len: *mut usize) -> *mut *mut TopoDS_Shape;
		pub fn compound_add(compound: *mut TopoDS_Shape, child: *const TopoDS_Shape);

		pub fn mesh_shape(shape: *const TopoDS_Shape, linear: f64, angular: f64, relative: bool) -> CMeshData;

		pub fn shape_edges(shape: *const TopoDS_Shape, out_len: *mut usize) -> *mut *mut TopoDS_Edge;
		pub fn shape_faces(shape: *const TopoDS_Shape, out_len: *mut usize) -> *mut *mut TopoDS_Face;
		pub fn face_edges(face: *const TopoDS_Face, out_len: *mut usize) -> *mut *mut TopoDS_Edge;
		pub fn clone_shape_handle(shape: *const TopoDS_Shape) -> *mut TopoDS_Shape;

		pub fn face_tshape_id(face: *const TopoDS_Face) -> u64;
		pub fn shape_tshape_id(shape: *const TopoDS_Shape) -> u64;
		pub fn edge_tshape_id(edge: *const TopoDS_Edge) -> u64;
		pub fn face_project_point(face: *const TopoDS_Face, px: f64, py: f64, pz: f64, cpx: *mut f64, cpy: *mut f64, cpz: *mut f64, nx: *mut f64, ny: *mut f64, nz: *mut f64) -> bool;

		pub fn edge_approximation_segments(edge: *const TopoDS_Edge, linear: f64, angular: f64, relative: bool, out_len: *mut usize) -> *mut f64;
		pub fn make_helix_edge(ax: f64, ay: f64, az: f64, xrx: f64, xry: f64, xrz: f64, radius: f64, pitch: f64, height: f64) -> *mut TopoDS_Edge;
		pub fn make_polygon_edges(coords: *const f64, n_coords: usize, out_len: *mut usize) -> *mut *mut TopoDS_Edge;
		pub fn make_circle_edge(ax: f64, ay: f64, az: f64, radius: f64) -> *mut TopoDS_Edge;
		pub fn make_line_edge(ax: f64, ay: f64, az: f64, bx: f64, by: f64, bz: f64) -> *mut TopoDS_Edge;
		pub fn make_arc_edge(sx: f64, sy: f64, sz: f64, mx: f64, my: f64, mz: f64, ex: f64, ey: f64, ez: f64) -> *mut TopoDS_Edge;
		pub fn make_bspline_edge(coords: *const f64, n_coords: usize, end_kind: u32, sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64) -> *mut TopoDS_Edge;
		pub fn edge_endpoints(edge: *const TopoDS_Edge, sx: *mut f64, sy: *mut f64, sz: *mut f64, ex: *mut f64, ey: *mut f64, ez: *mut f64);
		pub fn edge_tangents(edge: *const TopoDS_Edge, sx: *mut f64, sy: *mut f64, sz: *mut f64, ex: *mut f64, ey: *mut f64, ez: *mut f64);
		pub fn edge_is_closed(edge: *const TopoDS_Edge) -> bool;
		pub fn edge_project_point(edge: *const TopoDS_Edge, px: f64, py: f64, pz: f64, cpx: *mut f64, cpy: *mut f64, cpz: *mut f64, tx: *mut f64, ty: *mut f64, tz: *mut f64) -> bool;
		pub fn deep_copy_edge(edge: *const TopoDS_Edge) -> *mut TopoDS_Edge;
		pub fn translate_edge(edge: *const TopoDS_Edge, tx: f64, ty: f64, tz: f64) -> *mut TopoDS_Edge;
		pub fn rotate_edge(edge: *const TopoDS_Edge, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> *mut TopoDS_Edge;
		pub fn scale_edge(edge: *const TopoDS_Edge, cx: f64, cy: f64, cz: f64, factor: f64) -> *mut TopoDS_Edge;
		pub fn mirror_edge(edge: *const TopoDS_Edge, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> *mut TopoDS_Edge;
		pub fn make_extrude(profile_edges: *const *const TopoDS_Edge, n_profile: usize, dx: f64, dy: f64, dz: f64) -> *mut TopoDS_Shape;
		pub fn make_pipe_shell(all_edges: *const *const TopoDS_Edge, n_all: usize, spine_edges: *const *const TopoDS_Edge, n_spine: usize, orient: u32, ux: f64, uy: f64, uz: f64, aux_spine_edges: *const *const TopoDS_Edge, n_aux: usize) -> *mut TopoDS_Shape;
		pub fn make_loft(all_edges: *const *const TopoDS_Edge, n_all: usize, ruled: bool) -> *mut TopoDS_Shape;
		pub fn make_sewn_solid(faces: *const *const TopoDS_Face, n_faces: usize, tolerance: f64) -> *mut TopoDS_Shape;
		pub fn make_offset_shape(shape: *const TopoDS_Shape, offset: f64, tolerance: f64) -> *mut TopoDS_Shape;
		pub fn make_bspline_solid(coords: *const f64, n_coords: usize, nu: u32, nv: u32, u_periodic: bool) -> *mut TopoDS_Shape;

		#[cfg(feature = "color")]
		pub fn read_step_color_stream(reader: *const CReader, out_ids: *mut *mut u64, out_ids_len: *mut usize, out_rgb: *mut *mut f32, out_rgb_len: *mut usize) -> *mut TopoDS_Shape;
		#[cfg(feature = "color")]
		pub fn write_step_color_stream(shape: *const TopoDS_Shape, ids: *const u64, n_ids: usize, rgb: *const f32, n_rgb: usize, writer: *const CWriter) -> bool;
	}
}

// ==================== Marshaling helpers ====================

/// Copy a malloc'd POD buffer into a `Vec` and free it.
///
/// # Safety
/// `ptr` is null or points to `len` `T` allocated by the wrapper with malloc.
unsafe fn take_pod<T: Copy>(ptr: *mut T, len: usize) -> Vec<T> {
	if ptr.is_null() {
		return Vec::new();
	}
	let v = std::slice::from_raw_parts(ptr, len).to_vec();
	raw::cadrum_free(ptr as *mut c_void);
	v
}

/// Wrap a malloc'd array of owned handles into `Vec<Owned<T>>` and free the array.
///
/// # Safety
/// `arr` is null or points to `len` owning `*mut T` allocated with malloc.
unsafe fn take_owned<T: OcctFree>(arr: *mut *mut T, len: usize) -> Vec<Owned<T>> {
	let v = (0..len).map(|i| Owned::from_raw(*arr.add(i))).collect();
	if !arr.is_null() {
		raw::cadrum_free(arr as *mut c_void);
	}
	v
}

/// Append a malloc'd `u64` history buffer to `out` and free it.
///
/// # Safety
/// `ptr` is null or points to `len` `u64` allocated with malloc.
unsafe fn append_history(out: &mut Vec<u64>, ptr: *mut u64, len: usize) {
	if !ptr.is_null() {
		out.extend_from_slice(std::slice::from_raw_parts(ptr, len));
		raw::cadrum_free(ptr as *mut c_void);
	}
}

// ==================== Safe wrappers ====================

// ---- Shape I/O ----

#[cfg(not(feature = "color"))]
pub fn read_step_stream(reader: &mut RustReader) -> Owned<TopoDS_Shape> {
	let c = reader.as_c();
	unsafe { Owned::from_raw(raw::read_step_stream(&c)) }
}

#[cfg(not(feature = "color"))]
pub fn write_step_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool {
	let c = writer.as_c();
	unsafe { raw::write_step_stream(shape, &c) }
}

pub fn read_brep_bin_stream(reader: &mut RustReader) -> Owned<TopoDS_Shape> {
	let c = reader.as_c();
	unsafe { Owned::from_raw(raw::read_brep_bin_stream(&c)) }
}

pub fn write_brep_bin_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool {
	let c = writer.as_c();
	unsafe { raw::write_brep_bin_stream(shape, &c) }
}

pub fn read_brep_text_stream(reader: &mut RustReader) -> Owned<TopoDS_Shape> {
	let c = reader.as_c();
	unsafe { Owned::from_raw(raw::read_brep_text_stream(&c)) }
}

pub fn write_brep_text_stream(shape: &TopoDS_Shape, writer: &mut RustWriter) -> bool {
	let c = writer.as_c();
	unsafe { raw::write_brep_text_stream(shape, &c) }
}

// ---- Constructors ----

pub fn make_half_space(ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_half_space(ox, oy, oz, nx, ny, nz)) }
}
pub fn make_box(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_box(x1, y1, z1, x2, y2, z2)) }
}
pub fn make_cylinder(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, radius: f64, height: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_cylinder(px, py, pz, dx, dy, dz, radius, height)) }
}
pub fn make_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_sphere(cx, cy, cz, radius)) }
}
pub fn make_cone(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64, height: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_cone(px, py, pz, dx, dy, dz, r1, r2, height)) }
}
pub fn make_torus(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_torus(px, py, pz, dx, dy, dz, r1, r2)) }
}
pub fn make_empty() -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_empty()) }
}
pub fn deep_copy(shape: &TopoDS_Shape) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::deep_copy(shape)) }
}

// ---- Builders ----

pub fn builder_cells(solids: &[*const TopoDS_Shape], clauses: &[i64], out_history: &mut Vec<u64>) -> Owned<TopoDS_Shape> {
	let mut hp: *mut u64 = std::ptr::null_mut();
	let mut hl: usize = 0;
	unsafe {
		let s = raw::builder_cells(solids.as_ptr(), solids.len(), clauses.as_ptr(), clauses.len(), &mut hp, &mut hl);
		append_history(out_history, hp, hl);
		Owned::from_raw(s)
	}
}

pub fn builder_clean(shape: &TopoDS_Shape, out_history: &mut Vec<u64>) -> Owned<TopoDS_Shape> {
	let mut hp: *mut u64 = std::ptr::null_mut();
	let mut hl: usize = 0;
	unsafe {
		let s = raw::builder_clean(shape, &mut hp, &mut hl);
		append_history(out_history, hp, hl);
		Owned::from_raw(s)
	}
}

pub fn builder_thick_solid(solid: &TopoDS_Shape, open_faces: &[*const TopoDS_Face], thickness: f64, out_history: &mut Vec<u64>) -> Owned<TopoDS_Shape> {
	let mut hp: *mut u64 = std::ptr::null_mut();
	let mut hl: usize = 0;
	unsafe {
		let s = raw::builder_thick_solid(solid, open_faces.as_ptr(), open_faces.len(), thickness, &mut hp, &mut hl);
		append_history(out_history, hp, hl);
		Owned::from_raw(s)
	}
}

pub fn builder_fillet(solid: &TopoDS_Shape, edges: &[*const TopoDS_Edge], radius: f64, out_history: &mut Vec<u64>) -> Owned<TopoDS_Shape> {
	let mut hp: *mut u64 = std::ptr::null_mut();
	let mut hl: usize = 0;
	unsafe {
		let s = raw::builder_fillet(solid, edges.as_ptr(), edges.len(), radius, &mut hp, &mut hl);
		append_history(out_history, hp, hl);
		Owned::from_raw(s)
	}
}

pub fn builder_chamfer(solid: &TopoDS_Shape, edges: &[*const TopoDS_Edge], distance: f64, out_history: &mut Vec<u64>) -> Owned<TopoDS_Shape> {
	let mut hp: *mut u64 = std::ptr::null_mut();
	let mut hl: usize = 0;
	unsafe {
		let s = raw::builder_chamfer(solid, edges.as_ptr(), edges.len(), distance, &mut hp, &mut hl);
		append_history(out_history, hp, hl);
		Owned::from_raw(s)
	}
}

// ---- Transforms ----

pub fn transform_translate(shape: &TopoDS_Shape, tx: f64, ty: f64, tz: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::transform_translate(shape, tx, ty, tz)) }
}
pub fn transform_rotate(shape: &TopoDS_Shape, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::transform_rotate(shape, ox, oy, oz, dx, dy, dz, angle)) }
}
pub fn transform_scale(shape: &TopoDS_Shape, cx: f64, cy: f64, cz: f64, factor: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::transform_scale(shape, cx, cy, cz, factor)) }
}
pub fn transform_mirror(shape: &TopoDS_Shape, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::transform_mirror(shape, ox, oy, oz, nx, ny, nz)) }
}

// ---- Queries ----

pub fn shape_is_null(shape: &TopoDS_Shape) -> bool {
	unsafe { raw::shape_is_null(shape) }
}
pub fn shape_is_solid(shape: &TopoDS_Shape) -> bool {
	unsafe { raw::shape_is_solid(shape) }
}
pub fn shape_volume(shape: &TopoDS_Shape) -> f64 {
	unsafe { raw::shape_volume(shape) }
}
pub fn shape_surface_area(shape: &TopoDS_Shape) -> f64 {
	unsafe { raw::shape_surface_area(shape) }
}
pub fn shape_center_of_mass(shape: &TopoDS_Shape, x: &mut f64, y: &mut f64, z: &mut f64) {
	unsafe { raw::shape_center_of_mass(shape, x, y, z) }
}
#[allow(clippy::too_many_arguments)]
pub fn shape_inertia_tensor(shape: &TopoDS_Shape, m00: &mut f64, m01: &mut f64, m02: &mut f64, m10: &mut f64, m11: &mut f64, m12: &mut f64, m20: &mut f64, m21: &mut f64, m22: &mut f64) {
	unsafe { raw::shape_inertia_tensor(shape, m00, m01, m02, m10, m11, m12, m20, m21, m22) }
}
pub fn shape_contains_point(shape: &TopoDS_Shape, x: f64, y: f64, z: f64) -> bool {
	unsafe { raw::shape_contains_point(shape, x, y, z) }
}
pub fn shape_bounding_box(shape: &TopoDS_Shape, xmin: &mut f64, ymin: &mut f64, zmin: &mut f64, xmax: &mut f64, ymax: &mut f64, zmax: &mut f64) {
	unsafe { raw::shape_bounding_box(shape, xmin, ymin, zmin, xmax, ymax, zmax) }
}

// ---- Compound ----

pub fn decompose_into_solids(shape: &TopoDS_Shape) -> Vec<Owned<TopoDS_Shape>> {
	let mut len: usize = 0;
	unsafe {
		let arr = raw::decompose_into_solids(shape, &mut len);
		take_owned(arr, len)
	}
}

pub fn compound_add(compound: &mut Owned<TopoDS_Shape>, child: &TopoDS_Shape) {
	unsafe { raw::compound_add(compound.as_mut_ptr(), child) }
}

// ---- Meshing ----

pub fn mesh_shape(shape: &TopoDS_Shape, linear: f64, angular: f64, relative: bool) -> MeshData {
	unsafe {
		let m = raw::mesh_shape(shape, linear, angular, relative);
		MeshData {
			vertices: take_pod(m.vertices, m.vertices_len),
			uvs: take_pod(m.uvs, m.uvs_len),
			indices: take_pod(m.indices, m.indices_len),
			face_tshape_ids: take_pod(m.face_ids, m.face_ids_len),
			success: m.success,
		}
	}
}

// ---- Topology enumeration ----

pub fn shape_edges(shape: &TopoDS_Shape) -> Vec<Owned<TopoDS_Edge>> {
	let mut len: usize = 0;
	unsafe {
		let arr = raw::shape_edges(shape, &mut len);
		take_owned(arr, len)
	}
}
pub fn shape_faces(shape: &TopoDS_Shape) -> Vec<Owned<TopoDS_Face>> {
	let mut len: usize = 0;
	unsafe {
		let arr = raw::shape_faces(shape, &mut len);
		take_owned(arr, len)
	}
}
pub fn face_edges(face: &TopoDS_Face) -> Vec<Owned<TopoDS_Edge>> {
	let mut len: usize = 0;
	unsafe {
		let arr = raw::face_edges(face, &mut len);
		take_owned(arr, len)
	}
}
pub fn clone_shape_handle(shape: &TopoDS_Shape) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::clone_shape_handle(shape)) }
}

// ---- Face / Edge ids ----

pub fn face_tshape_id(face: &TopoDS_Face) -> u64 {
	unsafe { raw::face_tshape_id(face) }
}
pub fn shape_tshape_id(shape: &TopoDS_Shape) -> u64 {
	unsafe { raw::shape_tshape_id(shape) }
}
pub fn edge_tshape_id(edge: &TopoDS_Edge) -> u64 {
	unsafe { raw::edge_tshape_id(edge) }
}
#[allow(clippy::too_many_arguments)]
pub fn face_project_point(face: &TopoDS_Face, px: f64, py: f64, pz: f64, cpx: &mut f64, cpy: &mut f64, cpz: &mut f64, nx: &mut f64, ny: &mut f64, nz: &mut f64) -> bool {
	unsafe { raw::face_project_point(face, px, py, pz, cpx, cpy, cpz, nx, ny, nz) }
}

// ---- Edge methods ----

pub fn edge_approximation_segments(edge: &TopoDS_Edge, linear: f64, angular: f64, relative: bool) -> Vec<f64> {
	let mut len: usize = 0;
	unsafe {
		let p = raw::edge_approximation_segments(edge, linear, angular, relative, &mut len);
		take_pod(p, len)
	}
}
pub fn make_helix_edge(ax: f64, ay: f64, az: f64, xrx: f64, xry: f64, xrz: f64, radius: f64, pitch: f64, height: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::make_helix_edge(ax, ay, az, xrx, xry, xrz, radius, pitch, height)) }
}
pub fn make_polygon_edges(coords: &[f64]) -> Vec<Owned<TopoDS_Edge>> {
	let mut len: usize = 0;
	unsafe {
		let arr = raw::make_polygon_edges(coords.as_ptr(), coords.len(), &mut len);
		take_owned(arr, len)
	}
}
pub fn make_circle_edge(ax: f64, ay: f64, az: f64, radius: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::make_circle_edge(ax, ay, az, radius)) }
}
pub fn make_line_edge(ax: f64, ay: f64, az: f64, bx: f64, by: f64, bz: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::make_line_edge(ax, ay, az, bx, by, bz)) }
}
pub fn make_arc_edge(sx: f64, sy: f64, sz: f64, mx: f64, my: f64, mz: f64, ex: f64, ey: f64, ez: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::make_arc_edge(sx, sy, sz, mx, my, mz, ex, ey, ez)) }
}
#[allow(clippy::too_many_arguments)]
pub fn make_bspline_edge(coords: &[f64], end_kind: u32, sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::make_bspline_edge(coords.as_ptr(), coords.len(), end_kind, sx, sy, sz, ex, ey, ez)) }
}
pub fn edge_endpoints(edge: &TopoDS_Edge, sx: &mut f64, sy: &mut f64, sz: &mut f64, ex: &mut f64, ey: &mut f64, ez: &mut f64) {
	unsafe { raw::edge_endpoints(edge, sx, sy, sz, ex, ey, ez) }
}
pub fn edge_tangents(edge: &TopoDS_Edge, sx: &mut f64, sy: &mut f64, sz: &mut f64, ex: &mut f64, ey: &mut f64, ez: &mut f64) {
	unsafe { raw::edge_tangents(edge, sx, sy, sz, ex, ey, ez) }
}
pub fn edge_is_closed(edge: &TopoDS_Edge) -> bool {
	unsafe { raw::edge_is_closed(edge) }
}
#[allow(clippy::too_many_arguments)]
pub fn edge_project_point(edge: &TopoDS_Edge, px: f64, py: f64, pz: f64, cpx: &mut f64, cpy: &mut f64, cpz: &mut f64, tx: &mut f64, ty: &mut f64, tz: &mut f64) -> bool {
	unsafe { raw::edge_project_point(edge, px, py, pz, cpx, cpy, cpz, tx, ty, tz) }
}
pub fn deep_copy_edge(edge: &TopoDS_Edge) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::deep_copy_edge(edge)) }
}
pub fn translate_edge(edge: &TopoDS_Edge, tx: f64, ty: f64, tz: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::translate_edge(edge, tx, ty, tz)) }
}
pub fn rotate_edge(edge: &TopoDS_Edge, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::rotate_edge(edge, ox, oy, oz, dx, dy, dz, angle)) }
}
pub fn scale_edge(edge: &TopoDS_Edge, cx: f64, cy: f64, cz: f64, factor: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::scale_edge(edge, cx, cy, cz, factor)) }
}
pub fn mirror_edge(edge: &TopoDS_Edge, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> Owned<TopoDS_Edge> {
	unsafe { Owned::from_raw(raw::mirror_edge(edge, ox, oy, oz, nx, ny, nz)) }
}
pub fn make_extrude(profile_edges: &[*const TopoDS_Edge], dx: f64, dy: f64, dz: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_extrude(profile_edges.as_ptr(), profile_edges.len(), dx, dy, dz)) }
}
#[allow(clippy::too_many_arguments)]
pub fn make_pipe_shell(all_edges: &[*const TopoDS_Edge], spine_edges: &[*const TopoDS_Edge], orient: u32, ux: f64, uy: f64, uz: f64, aux_spine_edges: &[*const TopoDS_Edge]) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_pipe_shell(all_edges.as_ptr(), all_edges.len(), spine_edges.as_ptr(), spine_edges.len(), orient, ux, uy, uz, aux_spine_edges.as_ptr(), aux_spine_edges.len())) }
}
pub fn make_loft(all_edges: &[*const TopoDS_Edge], ruled: bool) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_loft(all_edges.as_ptr(), all_edges.len(), ruled)) }
}
pub fn make_sewn_solid(faces: &[*const TopoDS_Face], tolerance: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_sewn_solid(faces.as_ptr(), faces.len(), tolerance)) }
}
pub fn make_offset_shape(shape: &TopoDS_Shape, offset: f64, tolerance: f64) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_offset_shape(shape, offset, tolerance)) }
}
pub fn make_bspline_solid(coords: &[f64], nu: u32, nv: u32, u_periodic: bool) -> Owned<TopoDS_Shape> {
	unsafe { Owned::from_raw(raw::make_bspline_solid(coords.as_ptr(), coords.len(), nu, nv, u_periodic)) }
}

// ---- Colored STEP I/O ----

#[cfg(feature = "color")]
pub fn read_step_color_stream(reader: &mut RustReader, out_ids: &mut Vec<u64>, out_rgb: &mut Vec<f32>) -> Owned<TopoDS_Shape> {
	let c = reader.as_c();
	let mut ids_p: *mut u64 = std::ptr::null_mut();
	let mut ids_l: usize = 0;
	let mut rgb_p: *mut f32 = std::ptr::null_mut();
	let mut rgb_l: usize = 0;
	unsafe {
		let s = raw::read_step_color_stream(&c, &mut ids_p, &mut ids_l, &mut rgb_p, &mut rgb_l);
		out_ids.extend(take_pod(ids_p, ids_l));
		out_rgb.extend(take_pod(rgb_p, rgb_l));
		Owned::from_raw(s)
	}
}

#[cfg(feature = "color")]
pub fn write_step_color_stream(shape: &TopoDS_Shape, ids: &[u64], rgb: &[f32], writer: &mut RustWriter) -> bool {
	let c = writer.as_c();
	unsafe { raw::write_step_color_stream(shape, ids.as_ptr(), ids.len(), rgb.as_ptr(), rgb.len(), &c) }
}
