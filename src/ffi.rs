use std::io::{Read, Write};

#[cxx::bridge(namespace = "cadrum")]
mod ffi_bridge {

	// Shared struct for mesh data returned from C++
	struct MeshData {
		vertices: Vec<f64>, // flat xyz
		normals: Vec<f64>,  // flat xyz, one per vertex
		indices: Vec<u32>,
		face_tshape_ids: Vec<u64>, // per-triangle TShape* address
		success: bool,
	}

	// Expose Rust stream types to C++ for streambuf callbacks
	extern "Rust" {
		type RustReader;
		type RustWriter;

		fn rust_reader_read(reader: &mut RustReader, buf: &mut [u8]) -> usize;
		fn rust_writer_write(writer: &mut RustWriter, buf: &[u8]) -> usize;
	}

	unsafe extern "C++" {
		include!("cadrum/src/ffi.h");

		// Opaque C++ types (accessed as cadrum::TopoDS_Solid etc. via using aliases)
		type TopoDS_Solid;
		type TopoDS_Face;
		type TopoDS_Edge;

		// ==================== Shape I/O (streambuf callback) ====================

		// Plain STEP I/O — used only without `color` feature.
		#[cfg(not(feature = "color"))]
		fn read_step_stream(reader: &mut RustReader) -> Result<UniquePtr<CxxVector<TopoDS_Solid>>>;
		#[cfg(not(feature = "color"))]
		fn write_step_stream(shape: &CxxVector<TopoDS_Solid>, writer: &mut RustWriter) -> Result<()>;
		// `out_consumed` = payload length, where the color trailer begins. Written only
		// when the returned pointer is non-null.
		fn read_brep_stream(data: &[u8], out_consumed: &mut usize) -> Result<UniquePtr<CxxVector<TopoDS_Solid>>>;
		fn write_brep_stream(shape: &CxxVector<TopoDS_Solid>, writer: &mut RustWriter) -> Result<()>;

		// ==================== Shape Constructors ====================

		fn make_half_space(ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> UniquePtr<TopoDS_Solid>;

		fn make_box(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> UniquePtr<TopoDS_Solid>;

		fn make_cylinder(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, radius: f64, height: f64) -> UniquePtr<TopoDS_Solid>;

		fn make_sphere(cx: f64, cy: f64, cz: f64, radius: f64) -> UniquePtr<TopoDS_Solid>;

		fn make_cone(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64, height: f64) -> UniquePtr<TopoDS_Solid>;

		fn make_torus(px: f64, py: f64, pz: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64) -> UniquePtr<TopoDS_Solid>;

		fn deep_copy(shape: &TopoDS_Solid) -> UniquePtr<TopoDS_Solid>;

		// ==================== Colored STEP I/O (color feature only) ====================

		#[cfg(feature = "color")]
		fn read_step_color_stream(reader: &mut RustReader, out_ids: &mut Vec<u64>, out_rgb: &mut Vec<f32>) -> Result<UniquePtr<CxxVector<TopoDS_Solid>>>;

		#[cfg(feature = "color")]
		fn write_step_color_stream(shape: &CxxVector<TopoDS_Solid>, ids: &[u64], rgb: &[f32], writer: &mut RustWriter) -> Result<()>;

		// ==================== Builders (solid → solid with history) ====================

		// Evaluate any boolean expression on N solids via BOPAlgo_CellsBuilder.
		// `clauses` は DIMACS-flat DNF (`+i` = solids[i-1] を take、`-i` = avoid、`0` = clause 終端)。
		// `out_history` の形式は builder_boolean と同じ。
		fn builder_cells(solids: &CxxVector<TopoDS_Solid>, clauses: &[i64], out_history: &mut Vec<u64>) -> Result<UniquePtr<CxxVector<TopoDS_Solid>>>;

		// Unify shared faces. `out_history` receives flat [new_id, old_id, ...]
		// pairs (same layout as `builder_boolean`), used by Solid::clean to populate
		// `Solid::history` and remap the colormap when color is enabled.
		fn builder_clean(shape: &TopoDS_Solid, out_history: &mut Vec<u64>) -> Result<UniquePtr<TopoDS_Solid>>;

		// shell/fillet/chamfer fill `out_history` with flat [post_id, src_id]
		// pairs (same layout as builder_cells) → Solid::history + colormap remap.
		fn builder_thick_solid(solid: &TopoDS_Solid, open_faces: &CxxVector<TopoDS_Face>, thickness: f64, out_history: &mut Vec<u64>) -> Result<UniquePtr<TopoDS_Solid>>;
		fn builder_fillet(solid: &TopoDS_Solid, edges: &CxxVector<TopoDS_Edge>, radius: f64, out_history: &mut Vec<u64>) -> Result<UniquePtr<TopoDS_Solid>>;
		fn builder_chamfer(solid: &TopoDS_Solid, edges: &CxxVector<TopoDS_Edge>, distance: f64, out_history: &mut Vec<u64>) -> Result<UniquePtr<TopoDS_Solid>>;

		// ==================== Transforms (solid → solid, no history) ====================

		fn transform_translate(shape: &TopoDS_Solid, tx: f64, ty: f64, tz: f64) -> UniquePtr<TopoDS_Solid>;

		fn transform_rotate(shape: &TopoDS_Solid, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> Result<UniquePtr<TopoDS_Solid>>;

		fn transform_scale(shape: &TopoDS_Solid, cx: f64, cy: f64, cz: f64, factor: f64) -> Result<UniquePtr<TopoDS_Solid>>;

		fn transform_mirror(shape: &TopoDS_Solid, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> Result<UniquePtr<TopoDS_Solid>>;

		// ==================== Shape Queries ====================
		fn shape_volume(shape: &TopoDS_Solid) -> f64;
		fn shape_center_of_mass(shape: &TopoDS_Solid, x: &mut f64, y: &mut f64, z: &mut f64);
		fn shape_inertia_tensor(shape: &TopoDS_Solid, m00: &mut f64, m01: &mut f64, m02: &mut f64, m10: &mut f64, m11: &mut f64, m12: &mut f64, m20: &mut f64, m21: &mut f64, m22: &mut f64);
		fn shape_contains_point(shape: &TopoDS_Solid, x: f64, y: f64, z: f64) -> bool;
		fn shape_bounding_box(shape: &TopoDS_Solid, xmin: &mut f64, ymin: &mut f64, zmin: &mut f64, xmax: &mut f64, ymax: &mut f64, zmax: &mut f64);

		// ==================== Meshing ====================

		fn mesh_shape(shape: &CxxVector<TopoDS_Solid>, linear: f64, angular: f64, relative: bool) -> MeshData;

		// ==================== Topology enumeration ====================

		fn solid_edges(shape: &TopoDS_Solid) -> UniquePtr<CxxVector<TopoDS_Edge>>;
		fn solid_faces(shape: &TopoDS_Solid) -> UniquePtr<CxxVector<TopoDS_Face>>;
		fn face_edges(face: &TopoDS_Face) -> UniquePtr<CxxVector<TopoDS_Edge>>;

		fn clone_solid_handle(solid: &TopoDS_Solid) -> UniquePtr<TopoDS_Solid>;
		fn clone_edge_handle(edge: &TopoDS_Edge) -> UniquePtr<TopoDS_Edge>;
		fn clone_face_handle(face: &TopoDS_Face) -> UniquePtr<TopoDS_Face>;

		// ==================== Face Methods ====================

		fn face_tshape_id(face: &TopoDS_Face) -> u64;
		fn solid_tshape_id(solid: &TopoDS_Solid) -> u64;
		fn edge_tshape_id(edge: &TopoDS_Edge) -> u64;

		fn face_surface(face: &TopoDS_Face, ox: &mut f64, oy: &mut f64, oz: &mut f64, zx: &mut f64, zy: &mut f64, zz: &mut f64, xx: &mut f64, xy: &mut f64, xz: &mut f64, p1: &mut f64, p2: &mut f64) -> u32;
		fn face_surface_area(face: &TopoDS_Face) -> f64;
		fn face_center_of_mass(face: &TopoDS_Face, x: &mut f64, y: &mut f64, z: &mut f64);
		fn face_project_point(face: &TopoDS_Face, px: f64, py: f64, pz: f64, cpx: &mut f64, cpy: &mut f64, cpz: &mut f64, nx: &mut f64, ny: &mut f64, nz: &mut f64) -> bool;

		// ==================== Edge Methods ====================

		fn edge_approximation_segments(edge: &TopoDS_Edge, linear: f64, angular: f64, relative: bool) -> Vec<f64>;

		fn make_helix_edge(ax: f64, ay: f64, az: f64, xrx: f64, xry: f64, xrz: f64, radius: f64, pitch: f64, height: f64) -> Result<UniquePtr<TopoDS_Edge>>;
		fn make_polygon_edges(coords: &[f64]) -> Result<UniquePtr<CxxVector<TopoDS_Edge>>>;
		fn make_circle_edge(ax: f64, ay: f64, az: f64, radius: f64) -> Result<UniquePtr<TopoDS_Edge>>;
		fn make_line_edge(ax: f64, ay: f64, az: f64, bx: f64, by: f64, bz: f64) -> Result<UniquePtr<TopoDS_Edge>>;
		fn make_arc_edge(sx: f64, sy: f64, sz: f64, mx: f64, my: f64, mz: f64, ex: f64, ey: f64, ez: f64) -> Result<UniquePtr<TopoDS_Edge>>;
		fn make_bspline_edge(coords: &[f64], end_kind: u32, sx: f64, sy: f64, sz: f64, ex: f64, ey: f64, ez: f64) -> Result<UniquePtr<TopoDS_Edge>>;

		fn edge_endpoints(edge: &TopoDS_Edge, sx: &mut f64, sy: &mut f64, sz: &mut f64, ex: &mut f64, ey: &mut f64, ez: &mut f64);
		fn edge_tangents(edge: &TopoDS_Edge, sx: &mut f64, sy: &mut f64, sz: &mut f64, ex: &mut f64, ey: &mut f64, ez: &mut f64);
		fn precision_confusion() -> f64;
		fn edge_project_point(edge: &TopoDS_Edge, px: f64, py: f64, pz: f64, cpx: &mut f64, cpy: &mut f64, cpz: &mut f64, tx: &mut f64, ty: &mut f64, tz: &mut f64) -> bool;

		fn deep_copy_edge(edge: &TopoDS_Edge) -> Result<UniquePtr<TopoDS_Edge>>;

		fn translate_edge(edge: &TopoDS_Edge, tx: f64, ty: f64, tz: f64) -> Result<UniquePtr<TopoDS_Edge>>;
		fn rotate_edge(edge: &TopoDS_Edge, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> Result<UniquePtr<TopoDS_Edge>>;
		fn scale_edge(edge: &TopoDS_Edge, cx: f64, cy: f64, cz: f64, factor: f64) -> Result<UniquePtr<TopoDS_Edge>>;
		fn mirror_edge(edge: &TopoDS_Edge, ox: f64, oy: f64, oz: f64, nx: f64, ny: f64, nz: f64) -> Result<UniquePtr<TopoDS_Edge>>;

		fn make_extrude(profile_edges: &CxxVector<TopoDS_Edge>, dx: f64, dy: f64, dz: f64) -> Result<UniquePtr<TopoDS_Solid>>;
		fn make_revolve(profile_edges: &CxxVector<TopoDS_Edge>, ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64, angle: f64) -> Result<UniquePtr<TopoDS_Solid>>;
		fn make_pipe_shell(all_edges: &CxxVector<TopoDS_Edge>, spine_edges: &CxxVector<TopoDS_Edge>, orient: u32, ux: f64, uy: f64, uz: f64, aux_spine_edges: &CxxVector<TopoDS_Edge>) -> Result<UniquePtr<TopoDS_Solid>>;
		fn make_loft(all_edges: &CxxVector<TopoDS_Edge>, ruled: bool) -> Result<UniquePtr<TopoDS_Solid>>;
		fn make_sewn_solid(faces: &CxxVector<TopoDS_Face>, tolerance: f64) -> Result<UniquePtr<TopoDS_Solid>>;
		fn make_offset(shape: &TopoDS_Solid, faces: &CxxVector<TopoDS_Face>, offset: f64, tolerance: f64) -> Result<UniquePtr<TopoDS_Solid>>;
		fn make_bspline_solid(coords: &[f64], nu: u32, nv: u32, u_periodic: bool) -> Result<UniquePtr<TopoDS_Solid>>;

		fn edge_vec_new() -> UniquePtr<CxxVector<TopoDS_Edge>>;
		fn edge_vec_push(v: Pin<&mut CxxVector<TopoDS_Edge>>, e: &TopoDS_Edge);
		fn edge_vec_push_null(v: Pin<&mut CxxVector<TopoDS_Edge>>);

		fn face_vec_new() -> UniquePtr<CxxVector<TopoDS_Face>>;
		fn face_vec_push(v: Pin<&mut CxxVector<TopoDS_Face>>, f: &TopoDS_Face);

		fn shape_vec_new() -> UniquePtr<CxxVector<TopoDS_Solid>>;
		fn shape_vec_push(v: Pin<&mut CxxVector<TopoDS_Solid>>, s: &TopoDS_Solid);

	}
}

// Re-export all bridge items so other modules can use `ffi::TopoDS_Solid` etc.
pub use ffi_bridge::*;

// ==================== Stream wrappers ====================
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

/// Wrapper around `dyn Write` passed to C++ as an opaque extern Rust type.
///
/// C++ calls `rust_writer_write()` to push bytes into the Rust writer,
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

/// FFI callback: read up to `buf.len()` bytes from the RustReader.
/// Returns the number of bytes actually read (0 = EOF).
pub fn rust_reader_read(reader: &mut RustReader, buf: &mut [u8]) -> usize {
	unsafe { (*reader.inner).read(buf).unwrap_or(0) }
}

/// FFI callback: write bytes into the RustWriter.
/// Returns the number of bytes actually written.
pub fn rust_writer_write(writer: &mut RustWriter, buf: &[u8]) -> usize {
	unsafe { (*writer.inner).write(buf).unwrap_or(0) }
}

// cxx opaque types default to `!Send + !Sync`. We mark them `Send` here so
// that `UniquePtr<TopoDS_Solid>` (and friends) become `Send`, which in turn
// makes our wrapper types (`Shape`, `Solid`, `Face`, `Edge`) auto-Send.
//
// Safety rationale:
//   - `UniquePtr` gives exclusive ownership — no aliasing is possible.
//   - These values are never shared across threads simultaneously; they are
//     only *moved* to another thread, which is what `Send` permits.
//   - `Sync` is intentionally NOT implemented: OCC's `Handle<Geom_XXX>`
//     reference counts are non-atomic, so concurrent `&T` access across
//     threads would be unsound.
unsafe impl Send for TopoDS_Solid {}
unsafe impl Send for TopoDS_Face {}
unsafe impl Send for TopoDS_Edge {}
