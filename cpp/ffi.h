#pragma once

// Hand-written C ABI for the cadrum OCCT wrapper.
//
// This replaces the cxx-generated bridge: `wrapper.cpp` keeps its OCCT logic in
// `namespace cadrum` (using `std::unique_ptr` / `std::vector`), and a thin
// `extern "C"` shim layer (defined at the bottom of wrapper.cpp) marshals
// between those C++ types and the raw pointer/length ABI declared here. The Rust
// side declares matching `extern "C"` signatures in `src/occt/ffi.rs`.
//
// Ownership conventions:
//   - `TopoDS_*` returned by value are heap objects owned by Rust; free with the
//     matching `cadrum_*_free` (delete).
//   - `T**` arrays and POD (`double*`/`uint32_t*`/`uint64_t*`/`float*`) buffers
//     are allocated with `malloc`; Rust copies out then frees with `cadrum_free`.
//     For `T**` arrays each element is an owned `TopoDS_*` (free individually).

#include <cstdint>
#include <cstddef>

// OCCT topology types — only used behind pointers across the ABI, so incomplete
// forward declarations suffice (avoids pulling heavy OCCT headers into the ABI).
class TopoDS_Shape;
class TopoDS_Face;
class TopoDS_Edge;

extern "C" {

// ==================== Stream callbacks ====================
// Bridge a Rust `dyn Read`/`dyn Write` into a C++ std::streambuf. `ctx` is an
// opaque pointer to the Rust-side reader/writer; `read`/`write` is a Rust
// trampoline with C linkage.
typedef size_t (*cadrum_read_fn)(void* ctx, uint8_t* buf, size_t len);
typedef size_t (*cadrum_write_fn)(void* ctx, const uint8_t* buf, size_t len);

struct CReader {
	void* ctx;
	cadrum_read_fn read;
};
struct CWriter {
	void* ctx;
	cadrum_write_fn write;
};

// ==================== Mesh result ====================
// POD arrays allocated with malloc (free each with cadrum_free).
struct CMeshData {
	double* vertices;
	size_t vertices_len;
	double* uvs;
	size_t uvs_len;
	uint32_t* indices;
	size_t indices_len;
	uint64_t* face_ids;
	size_t face_ids_len;
	bool success;
};

// ==================== Deallocators ====================
void cadrum_free(void* p);              // free()  — POD arrays / T** arrays
void cadrum_shape_free(TopoDS_Shape* p); // delete — owned shape
void cadrum_face_free(TopoDS_Face* p);   // delete — owned face
void cadrum_edge_free(TopoDS_Edge* p);   // delete — owned edge

// ==================== Shape I/O (streambuf callback) ====================
#ifndef CADRUM_COLOR
TopoDS_Shape* read_step_stream(const CReader* reader);
bool write_step_stream(const TopoDS_Shape* shape, const CWriter* writer);
#endif
TopoDS_Shape* read_brep_bin_stream(const CReader* reader);
bool write_brep_bin_stream(const TopoDS_Shape* shape, const CWriter* writer);
TopoDS_Shape* read_brep_text_stream(const CReader* reader);
bool write_brep_text_stream(const TopoDS_Shape* shape, const CWriter* writer);

// ==================== Shape Constructors ====================
TopoDS_Shape* make_half_space(double ox, double oy, double oz, double nx, double ny, double nz);
TopoDS_Shape* make_box(double x1, double y1, double z1, double x2, double y2, double z2);
TopoDS_Shape* make_cylinder(double px, double py, double pz, double dx, double dy, double dz, double radius, double height);
TopoDS_Shape* make_sphere(double cx, double cy, double cz, double radius);
TopoDS_Shape* make_cone(double px, double py, double pz, double dx, double dy, double dz, double r1, double r2, double height);
TopoDS_Shape* make_torus(double px, double py, double pz, double dx, double dy, double dz, double r1, double r2);
TopoDS_Shape* make_empty();
TopoDS_Shape* deep_copy(const TopoDS_Shape* shape);

// ==================== Builders (solid -> solid with history) ====================
// `out_history`/`out_history_len` receive a malloc'd flat [post_id, src_id, ...]
// buffer (free with cadrum_free).
TopoDS_Shape* builder_cells(const TopoDS_Shape* const* solids, size_t n_solids, const int64_t* clauses, size_t n_clauses, uint64_t** out_history, size_t* out_history_len);
TopoDS_Shape* builder_clean(const TopoDS_Shape* shape, uint64_t** out_history, size_t* out_history_len);
TopoDS_Shape* builder_thick_solid(const TopoDS_Shape* solid, const TopoDS_Face* const* open_faces, size_t n_faces, double thickness, uint64_t** out_history, size_t* out_history_len);
TopoDS_Shape* builder_fillet(const TopoDS_Shape* solid, const TopoDS_Edge* const* edges, size_t n_edges, double radius, uint64_t** out_history, size_t* out_history_len);
TopoDS_Shape* builder_chamfer(const TopoDS_Shape* solid, const TopoDS_Edge* const* edges, size_t n_edges, double distance, uint64_t** out_history, size_t* out_history_len);

// ==================== Transforms (solid -> solid, no history) ====================
TopoDS_Shape* transform_translate(const TopoDS_Shape* shape, double tx, double ty, double tz);
TopoDS_Shape* transform_rotate(const TopoDS_Shape* shape, double ox, double oy, double oz, double dx, double dy, double dz, double angle);
TopoDS_Shape* transform_scale(const TopoDS_Shape* shape, double cx, double cy, double cz, double factor);
TopoDS_Shape* transform_mirror(const TopoDS_Shape* shape, double ox, double oy, double oz, double nx, double ny, double nz);

// ==================== Shape Queries ====================
bool shape_is_null(const TopoDS_Shape* shape);
bool shape_is_solid(const TopoDS_Shape* shape);
double shape_volume(const TopoDS_Shape* shape);
double shape_surface_area(const TopoDS_Shape* shape);
void shape_center_of_mass(const TopoDS_Shape* shape, double* x, double* y, double* z);
void shape_inertia_tensor(const TopoDS_Shape* shape, double* m00, double* m01, double* m02, double* m10, double* m11, double* m12, double* m20, double* m21, double* m22);
bool shape_contains_point(const TopoDS_Shape* shape, double x, double y, double z);
void shape_bounding_box(const TopoDS_Shape* shape, double* xmin, double* ymin, double* zmin, double* xmax, double* ymax, double* zmax);

// ==================== Compound Decompose/Compose ====================
// Returns a malloc'd array of owned TopoDS_Shape* (free array with cadrum_free,
// each element with cadrum_shape_free).
TopoDS_Shape** decompose_into_solids(const TopoDS_Shape* shape, size_t* out_len);
void compound_add(TopoDS_Shape* compound, const TopoDS_Shape* child);

// ==================== Meshing ====================
CMeshData mesh_shape(const TopoDS_Shape* shape, double linear, double angular, bool relative);

// ==================== Topology enumeration ====================
// Each returns a malloc'd array of owned handles (free array with cadrum_free,
// each element with the matching cadrum_*_free).
TopoDS_Edge** shape_edges(const TopoDS_Shape* shape, size_t* out_len);
TopoDS_Face** shape_faces(const TopoDS_Shape* shape, size_t* out_len);
TopoDS_Edge** face_edges(const TopoDS_Face* face, size_t* out_len);

TopoDS_Shape* clone_shape_handle(const TopoDS_Shape* shape);

// ==================== Face Methods ====================
uint64_t face_tshape_id(const TopoDS_Face* face);
uint64_t shape_tshape_id(const TopoDS_Shape* shape);
uint64_t edge_tshape_id(const TopoDS_Edge* edge);
bool face_project_point(const TopoDS_Face* face, double px, double py, double pz, double* cpx, double* cpy, double* cpz, double* nx, double* ny, double* nz);

// ==================== Edge Methods ====================
// Returns a malloc'd flat xyz buffer (free with cadrum_free).
double* edge_approximation_segments(const TopoDS_Edge* edge, double linear, double angular, bool relative, size_t* out_len);

TopoDS_Edge* make_helix_edge(double ax, double ay, double az, double xrx, double xry, double xrz, double radius, double pitch, double height);
TopoDS_Edge** make_polygon_edges(const double* coords, size_t n_coords, size_t* out_len);
TopoDS_Edge* make_circle_edge(double ax, double ay, double az, double radius);
TopoDS_Edge* make_line_edge(double ax, double ay, double az, double bx, double by, double bz);
TopoDS_Edge* make_arc_edge(double sx, double sy, double sz, double mx, double my, double mz, double ex, double ey, double ez);
TopoDS_Edge* make_bspline_edge(const double* coords, size_t n_coords, uint32_t end_kind, double sx, double sy, double sz, double ex, double ey, double ez);

void edge_endpoints(const TopoDS_Edge* edge, double* sx, double* sy, double* sz, double* ex, double* ey, double* ez);
void edge_tangents(const TopoDS_Edge* edge, double* sx, double* sy, double* sz, double* ex, double* ey, double* ez);
bool edge_is_closed(const TopoDS_Edge* edge);
bool edge_project_point(const TopoDS_Edge* edge, double px, double py, double pz, double* cpx, double* cpy, double* cpz, double* tx, double* ty, double* tz);

TopoDS_Edge* deep_copy_edge(const TopoDS_Edge* edge);
TopoDS_Edge* translate_edge(const TopoDS_Edge* edge, double tx, double ty, double tz);
TopoDS_Edge* rotate_edge(const TopoDS_Edge* edge, double ox, double oy, double oz, double dx, double dy, double dz, double angle);
TopoDS_Edge* scale_edge(const TopoDS_Edge* edge, double cx, double cy, double cz, double factor);
TopoDS_Edge* mirror_edge(const TopoDS_Edge* edge, double ox, double oy, double oz, double nx, double ny, double nz);

// Edge collections passed in as pointer arrays; a null entry is a section
// separator (replaces the cxx null-edge sentinel) for pipe_shell / loft.
TopoDS_Shape* make_extrude(const TopoDS_Edge* const* profile_edges, size_t n_profile, double dx, double dy, double dz);
TopoDS_Shape* make_pipe_shell(const TopoDS_Edge* const* all_edges, size_t n_all, const TopoDS_Edge* const* spine_edges, size_t n_spine, uint32_t orient, double ux, double uy, double uz, const TopoDS_Edge* const* aux_spine_edges, size_t n_aux);
TopoDS_Shape* make_loft(const TopoDS_Edge* const* all_edges, size_t n_all, bool ruled);
TopoDS_Shape* make_sewn_solid(const TopoDS_Face* const* faces, size_t n_faces, double tolerance);
TopoDS_Shape* make_offset_shape(const TopoDS_Shape* shape, double offset, double tolerance);
TopoDS_Shape* make_bspline_solid(const double* coords, size_t n_coords, uint32_t nu, uint32_t nv, bool u_periodic);

// ==================== Colored STEP I/O (color feature only) ====================
#ifdef CADRUM_COLOR
// out_ids / out_rgb receive malloc'd POD buffers (free with cadrum_free).
TopoDS_Shape* read_step_color_stream(const CReader* reader, uint64_t** out_ids, size_t* out_ids_len, float** out_rgb, size_t* out_rgb_len);
bool write_step_color_stream(const TopoDS_Shape* shape, const uint64_t* ids, size_t n_ids, const float* rgb, size_t n_rgb, const CWriter* writer);
#endif

} // extern "C"
