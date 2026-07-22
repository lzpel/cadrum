#pragma once

// Pure C ABI between Rust (src/occt/ffi.rs) and the OCCT wrapper
// (cpp/wrapper.cpp). The Rust-side bindings are committed at src/occt/ffi.rs
// and kept in sync with this header by hand; the header stays bindgen-parseable
// (plain C when __cplusplus is absent) so they can be regenerated mechanically.

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
#include <TopoDS_Shape.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Edge.hxx>
#include <vector>
typedef std::vector<TopoDS_Shape> ShapeVec;
typedef std::vector<TopoDS_Face> FaceVec;
typedef std::vector<TopoDS_Edge> EdgeVec;
extern "C" {
#else
#include <stdbool.h>
typedef struct TopoDS_Shape TopoDS_Shape;
typedef struct TopoDS_Face TopoDS_Face;
typedef struct TopoDS_Edge TopoDS_Edge;
typedef struct ShapeVec ShapeVec;
typedef struct FaceVec FaceVec;
typedef struct EdgeVec EdgeVec;
#endif

// ==================== Rust-side callbacks (defined in src/occt) ====================
// `vec` points at a Rust `Vec<T>`; `reader`/`writer` point at `RustReader` /
// `RustWriter`. C++ only appends / reads / writes through these — nothing else.

void cadrum_u32_extend(void* vec, const uint32_t* data, size_t len);
void cadrum_u64_extend(void* vec, const uint64_t* data, size_t len);
void cadrum_f32_extend(void* vec, const float* data, size_t len);
void cadrum_f64_extend(void* vec, const double* data, size_t len);
size_t cadrum_reader_read(void* reader, uint8_t* buf, size_t len);
size_t cadrum_writer_write(void* writer, const uint8_t* buf, size_t len);

// ==================== Ownership ====================
// Pointers returned by constructors / `*_new` transfer ownership to Rust and
// must be freed with the matching `*_delete`. Pointers returned by `*_get`
// are borrows owned by the parent vector. All functions returning a pointer
// return NULL on failure.

void cadrum_shape_delete(TopoDS_Shape* shape);
void cadrum_face_delete(TopoDS_Face* face);
void cadrum_edge_delete(TopoDS_Edge* edge);
void cadrum_shape_vec_delete(ShapeVec* v);
void cadrum_face_vec_delete(FaceVec* v);
void cadrum_edge_vec_delete(EdgeVec* v);

// ==================== Shape I/O (streambuf callback) ====================

#ifndef CADRUM_COLOR
// Plain STEP I/O — only built without CADRUM_COLOR; with color, STEP goes
// through XCAF (`cadrum_read_step_color_stream` etc.) instead.
TopoDS_Shape* cadrum_read_step_stream(void* reader);
bool cadrum_write_step_stream(const TopoDS_Shape* shape, void* writer);
#endif
// `out_consumed` = length of the BinTools payload, where Rust's color trailer
// begins. Written ONLY on success; on failure NULL comes back and it is untouched.
TopoDS_Shape* cadrum_read_brep_stream(const uint8_t* data, size_t data_len, size_t* out_consumed);
bool cadrum_write_brep_stream(const TopoDS_Shape* shape, void* writer);

// ==================== Shape Constructors ====================

TopoDS_Shape* cadrum_make_half_space(
    double ox, double oy, double oz,
    double nx, double ny, double nz);

TopoDS_Shape* cadrum_make_box(
    double x1, double y1, double z1,
    double x2, double y2, double z2);

TopoDS_Shape* cadrum_make_cylinder(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double radius, double height);

TopoDS_Shape* cadrum_make_sphere(
    double cx, double cy, double cz,
    double radius);

TopoDS_Shape* cadrum_make_cone(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2, double height);

TopoDS_Shape* cadrum_make_torus(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2);

TopoDS_Shape* cadrum_make_empty(void);
TopoDS_Shape* cadrum_deep_copy(const TopoDS_Shape* shape);

// ==================== Builders (solid → solid with history) ====================
//
// Functions in this section take one or more solid inputs, rebuild topology,
// and append flat [post_id, src_id, ...] face derivation pairs to
// `out_history` (a Rust `Vec<u64>` filled via `cadrum_u64_extend`).

// Evaluate an arbitrary boolean expression on N solids in a single pass
// using BOPAlgo_CellsBuilder. The expression is encoded as DIMACS-flat DNF:
//   - clauses は signed literal の列、`0` で 1 clause 終端 (末尾 0 必須)
//   - `+i` (i≥1) は solids[i-1] を AddToResult の toTake に
//   - `-i`         は solids[i-1] を toAvoid に
// 例: (A ∪ B) − C → solids=[A,B,C], clauses=[1,-3, 0, 2,-3, 0]
// 全 clause で同一 material を使い RemoveInternalBoundaries() で内部境界を除去。
TopoDS_Shape* cadrum_builder_cells(
    const ShapeVec* solids,
    const int64_t* clauses, size_t clauses_len,
    void* out_history);

// Unify shared faces / collinear edges via ShapeUpgrade_UnifySameDomain.
// `out_history` encodes how each old face maps onto the unified result.
// Rust uses it to remap the colormap when the `color` feature is enabled.
TopoDS_Shape* cadrum_builder_clean(
    const TopoDS_Shape* shape,
    void* out_history);

// Shell (hollow) the solid by removing `open_faces` and offsetting the
// remaining faces by `thickness` via BRepOffsetAPI_MakeThickSolid. Negative
// thickness hollows inward, positive thickens outward. Returns NULL on
// failure (e.g. self-intersecting offset at sharp corners).
//
// `out_history`: flat [post_id, src_id] face-derivation pairs (Modified(),
// identity for pass-through). Generated walls have no face source, absent.
TopoDS_Shape* cadrum_builder_thick_solid(
    const TopoDS_Shape* solid,
    const FaceVec* open_faces,
    double thickness,
    void* out_history);

// Fillet the given edges of `solid` with a uniform radius using
// BRepFilletAPI_MakeFillet. Empty `edges` is a no-op (returns a shallow
// copy of `solid`). Returns NULL on OCCT failure (radius too large,
// tangent discontinuity, edges not belonging to `solid`, etc.).
TopoDS_Shape* cadrum_builder_fillet(
    const TopoDS_Shape* solid,
    const EdgeVec* edges,
    double radius,
    void* out_history);

// Chamfer (symmetric bevel) the given edges of `solid` with a uniform
// distance using BRepFilletAPI_MakeChamfer. Empty `edges` is a no-op.
TopoDS_Shape* cadrum_builder_chamfer(
    const TopoDS_Shape* solid,
    const EdgeVec* edges,
    double distance,
    void* out_history);

// ==================== Transforms (solid → solid, no history) ====================
//
// 3D transforms. translate/rotate use TopLoc_Location and preserve TShape*
// (Rust side keeps colormap and history intact). scale/mirror rebuild
// topology via BRepBuilderAPI_Transform; OCCT does not expose a face
// derivation table, so out_history is intentionally absent and the Rust
// side clears Solid::history (colormap is remapped by face order instead).

TopoDS_Shape* cadrum_transform_translate(
    const TopoDS_Shape* shape, double tx, double ty, double tz);
TopoDS_Shape* cadrum_transform_rotate(
    const TopoDS_Shape* shape,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle);
TopoDS_Shape* cadrum_transform_scale(
    const TopoDS_Shape* shape,
    double cx, double cy, double cz,
    double factor);
TopoDS_Shape* cadrum_transform_mirror(
    const TopoDS_Shape* shape,
    double ox, double oy, double oz,
    double nx, double ny, double nz);

// ==================== Shape Queries ====================

bool cadrum_shape_is_null(const TopoDS_Shape* shape);
bool cadrum_shape_is_solid(const TopoDS_Shape* shape);
double cadrum_shape_volume(const TopoDS_Shape* shape);
double cadrum_shape_surface_area(const TopoDS_Shape* shape);
void cadrum_shape_center_of_mass(const TopoDS_Shape* shape,
    double* x, double* y, double* z);
void cadrum_shape_inertia_tensor(const TopoDS_Shape* shape,
    double* m00, double* m01, double* m02,
    double* m10, double* m11, double* m12,
    double* m20, double* m21, double* m22);
bool cadrum_shape_contains_point(const TopoDS_Shape* shape, double x, double y, double z);
void cadrum_shape_bounding_box(const TopoDS_Shape* shape,
    double* xmin, double* ymin, double* zmin,
    double* xmax, double* ymax, double* zmax);

// ==================== Compound Decompose/Compose ====================

ShapeVec* cadrum_decompose_into_solids(const TopoDS_Shape* shape);
void cadrum_compound_add(TopoDS_Shape* compound, const TopoDS_Shape* child);

// ==================== Meshing ====================

// Triangulate `shape`. On success fills the four Rust `Vec`s (flat xyz
// vertices, per-vertex normals, triangle indices, per-triangle TShape* ids)
// and returns true; on failure returns false leaving the vecs untouched.
bool cadrum_mesh_shape(const TopoDS_Shape* shape,
    double linear, double angular, bool relative,
    void* out_vertices, void* out_normals,
    void* out_indices, void* out_face_ids);

// ==================== Topology enumeration ====================

// One-shot enumeration of unique sub-shapes. `cadrum_shape_edges` deduplicates
// edges shared between faces (so each edge appears exactly once).
// Callers typically cache the result in a Rust-side OnceLock<Vec<Edge>>.
EdgeVec* cadrum_shape_edges(const TopoDS_Shape* shape);
FaceVec* cadrum_shape_faces(const TopoDS_Shape* shape);

// One-shot enumeration of the boundary edges of a single face. Edges shared
// between this face's wires are deduplicated so each edge appears once.
EdgeVec* cadrum_face_edges(const TopoDS_Face* face);

// Shallow handle clone — C++ copy-ctor shares the underlying TShape via
// OCCT's ref count. Needed when Rust materializes owned `Shape` / `Edge` /
// `Face` wrappers from the borrows yielded by `*_vec_get`. Distinct from
// `cadrum_deep_copy` / `cadrum_deep_copy_edge` which create new TShapes.
TopoDS_Shape* cadrum_clone_shape_handle(const TopoDS_Shape* shape);
TopoDS_Edge* cadrum_clone_edge_handle(const TopoDS_Edge* edge);
TopoDS_Face* cadrum_clone_face_handle(const TopoDS_Face* face);

// ==================== Face Methods ====================

// These return the underlying TopoDS_TShape* address as a u64 — used to
// track face/solid/edge identity across boolean ops, color maps, and BREP I/O.
uint64_t cadrum_face_tshape_id(const TopoDS_Face* face);
uint64_t cadrum_shape_tshape_id(const TopoDS_Shape* shape);
uint64_t cadrum_edge_tshape_id(const TopoDS_Edge* edge);

// Project a 3D point onto `face`. Sister of `cadrum_edge_project_point`.
// Returns the closest point on the (trimmed) face surface and the outward
// face normal there. `nx/ny/nz` is the zero vector when the projector
// cannot define a normal at the closest hit (degenerate surface point).
// Returns false on catastrophic OCCT failure.
bool cadrum_face_project_point(const TopoDS_Face* face,
    double px, double py, double pz,
    double* cpx, double* cpy, double* cpz,
    double* nx, double* ny, double* nz);

// ==================== Edge Methods ====================

// Approximate an edge as a polyline. Takes independent angular/chord
// deflection bounds. Fills `out` (a Rust `Vec<f64>`) with flat xyz triples.
void cadrum_edge_approximation_segments(
    const TopoDS_Edge* edge, double linear, double angular, bool relative,
    void* out);

// Construct a single helical edge on a cylindrical surface centered at the
// world origin. `axis` is the cylinder axis direction; `x_ref` is the
// reference direction that anchors the local +X axis of the cylindrical
// frame. The helix starts at `radius * normalize(x_ref - project_on(axis))`
// (i.e. at the +X side of the local frame, z=0) and rises by `height` over
// `height/pitch` turns. `x_ref` must not be parallel to `axis`.
TopoDS_Edge* cadrum_make_helix_edge(
    double ax, double ay, double az,
    double xrx, double xry, double xrz,
    double radius, double pitch, double height);

// Build a closed polygon from `coords` (flat xyz triples, ≥3 points) and
// return its constituent edges in order. The closing edge from the last
// point back to the first is included. Failure returns an EMPTY vector.
EdgeVec* cadrum_make_polygon_edges(const double* coords, size_t coords_len);

// Construct a closed circular edge of `radius` centered at the world origin,
// lying in the plane normal to `axis`. The local +X axis of the circle's
// frame (which determines the parametric start point) is chosen by OCCT
// from an arbitrary orthogonal direction to `axis`.
TopoDS_Edge* cadrum_make_circle_edge(
    double ax, double ay, double az, double radius);

// Construct a straight line segment edge from point a to point b.
TopoDS_Edge* cadrum_make_line_edge(
    double ax, double ay, double az,
    double bx, double by, double bz);

// Construct a circular arc edge through three points (start, mid, end).
// `mid` must not be collinear with `start` and `end`. On degenerate input
// OCCT returns NULL.
TopoDS_Edge* cadrum_make_arc_edge(
    double sx, double sy, double sz,
    double mx, double my, double mz,
    double ex, double ey, double ez);

// Cubic B-spline edge interpolating data points.
//
// `coords` is a flat array of xyz triples (length must be a multiple of 3
// and ≥ 6). Each (x, y, z) is one interpolation target — the resulting
// curve passes through every input point exactly. `end_kind` selects the
// end-condition variant of `BSplineEnd`:
//   0 = Periodic (C² periodic; tangent args ignored)
//   1 = NotAKnot (open, OCCT default; tangent args ignored)
//   2 = Clamped  (open, explicit start/end tangents in (sx,sy,sz)/(ex,ey,ez))
// Returns NULL on any failure.
TopoDS_Edge* cadrum_make_bspline_edge(
    const double* coords, size_t coords_len,
    uint32_t end_kind,
    double sx, double sy, double sz,
    double ex, double ey, double ez);

// Edge query helpers.
void cadrum_edge_endpoints(const TopoDS_Edge* edge,
    double* sx, double* sy, double* sz,
    double* ex, double* ey, double* ez);
void cadrum_edge_tangents(const TopoDS_Edge* edge,
    double* sx, double* sy, double* sz,
    double* ex, double* ey, double* ez);
bool cadrum_edge_is_closed(const TopoDS_Edge* edge);

// Project a world point onto the edge's underlying curve. Returns false if
// the curve is missing or the projector cannot converge (leaves outputs 0).
bool cadrum_edge_project_point(const TopoDS_Edge* edge,
    double px, double py, double pz,
    double* cpx, double* cpy, double* cpz,
    double* tx, double* ty, double* tz);

// Edge clone (deep copy of underlying TShape).
TopoDS_Edge* cadrum_deep_copy_edge(const TopoDS_Edge* edge);

// Edge spatial transforms. Mirror the shape-level helpers but operate on
// TopoDS_Edge directly so the Rust wrapper can stay edge-typed.
TopoDS_Edge* cadrum_translate_edge(
    const TopoDS_Edge* edge, double tx, double ty, double tz);
TopoDS_Edge* cadrum_rotate_edge(
    const TopoDS_Edge* edge,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle);
TopoDS_Edge* cadrum_scale_edge(
    const TopoDS_Edge* edge,
    double cx, double cy, double cz,
    double factor);
TopoDS_Edge* cadrum_mirror_edge(
    const TopoDS_Edge* edge,
    double ox, double oy, double oz,
    double nx, double ny, double nz);

// ==================== Sweeps / lofts / offsets ====================

// Extrude a closed profile wire into a solid using BRepPrimAPI_MakePrism.
// Internally builds Wire → Face → Prism.
TopoDS_Shape* cadrum_make_extrude(
    const EdgeVec* profile_edges,
    double dx, double dy, double dz);

// Sweep closed profile wires (from `all_edges`, sections separated by
// null-edge sentinels) along a spine wire (built from `spine_edges`) using
// BRepOffsetAPI_MakePipeShell. Supports single-profile sweep and
// multi-profile morphing.
//
// `orient` selects how the profile is oriented along the spine:
//   0 = Fixed   — fix the trihedron to the spine-start frame (no rotation)
//   1 = Torsion — raw Frenet trihedron (helices, springs)
//   2 = Up      — keep `(ux, uy, uz)` as the constant binormal direction
//   3 = Auxiliary — use `aux_spine_edges` as auxiliary spine for twist control
// Any other value falls back to Torsion.
TopoDS_Shape* cadrum_make_pipe_shell(
    const EdgeVec* all_edges,
    const EdgeVec* spine_edges,
    uint32_t orient,
    double ux, double uy, double uz,
    const EdgeVec* aux_spine_edges);

// Loft (skin) a solid through N cross-section wires.
// Sections in `all_edges` are separated by null-edge sentinels.
// `ruled=false` interpolates a smooth B-spline surface through all sections;
// `ruled=true` connects adjacent sections with straight ruled panels.
TopoDS_Shape* cadrum_make_loft(
    const EdgeVec* all_edges,
    bool ruled);

// Sew (stitch) free faces into a single closed shell and upgrade it to a
// solid via BRepBuilderAPI_MakeSolid. The sewn result must contain exactly
// one closed shell — gaps wider than `tolerance` (open shell), leftover free
// faces, or multiple disconnected shells all return NULL. The solid is
// oriented with BRepLib::OrientClosedSolid so the enclosed volume is
// positive regardless of input face orientation.
TopoDS_Shape* cadrum_make_sewn_solid(
    const FaceVec* faces,
    double tolerance);

// Offset every face of `shape` by signed `offset` (positive = outward,
// negative = inward) using BRepOffsetAPI_MakeOffsetShape (PerformByJoin,
// BRepOffset_Skin, GeomAbs_Arc). A SHELL/compound result is upgraded to a
// solid when it contains exactly one closed shell or one solid. Returns
// NULL when OCCT rejects the offset — typically a self-intersecting
// result (|offset| ≥ half the local wall thickness of a thin feature, or a
// concave slot narrower than 2*offset pinching shut).
TopoDS_Shape* cadrum_make_offset_shape(
    const TopoDS_Shape* shape,
    double offset,
    double tolerance);

// Build a B-spline surface solid from a 2D point grid.
// `coords` is a flat array of xyz triples, length = 3 * nu * nv.
// V direction (cross-section, j index) is always periodic.
// U direction (longitudinal, i index) is periodic iff `u_periodic=true`
// (producing a torus); otherwise the U-ends are capped with planar faces
// (producing a pipe). Returns NULL on any OCCT failure.
TopoDS_Shape* cadrum_make_bspline_solid(
    const double* coords, size_t coords_len,
    uint32_t nu, uint32_t nv,
    bool u_periodic);

// ==================== C++ vector helpers ====================
// Rust-side construction and iteration of std::vector<TopoDS_*>.

EdgeVec* cadrum_edge_vec_new(void);
void cadrum_edge_vec_push(EdgeVec* v, const TopoDS_Edge* e);
// Push a null edge — the section separator sentinel for pipe shell / loft.
void cadrum_edge_vec_push_null(EdgeVec* v);
size_t cadrum_edge_vec_len(const EdgeVec* v);
const TopoDS_Edge* cadrum_edge_vec_get(const EdgeVec* v, size_t i);

FaceVec* cadrum_face_vec_new(void);
void cadrum_face_vec_push(FaceVec* v, const TopoDS_Face* f);
size_t cadrum_face_vec_len(const FaceVec* v);
const TopoDS_Face* cadrum_face_vec_get(const FaceVec* v, size_t i);

ShapeVec* cadrum_shape_vec_new(void);
void cadrum_shape_vec_push(ShapeVec* v, const TopoDS_Shape* s);
size_t cadrum_shape_vec_len(const ShapeVec* v);
const TopoDS_Shape* cadrum_shape_vec_get(const ShapeVec* v, size_t i);

#ifdef CADRUM_COLOR

// ==================== Colored STEP I/O ====================

// `out_ids` (Rust `Vec<u64>`) = TShape* of each colored sub-shape, `out_rgb`
// (Rust `Vec<f32>`) = flat [r,g,b,...] in OCC native space. An id is a FACE's
// or a SOLID's — a styled_item targets either. Returns NULL on failure.
TopoDS_Shape* cadrum_read_step_color_stream(
    void* reader,
    void* out_ids,
    void* out_rgb);

// A solid id is written as one styled_item on that solid; a face style, being
// the more specific one, overrides it.
bool cadrum_write_step_color_stream(
    const TopoDS_Shape* shape,
    const uint64_t* ids, size_t ids_len,
    const float* rgb, size_t rgb_len,
    void* writer);

#endif // CADRUM_COLOR

#ifdef __cplusplus
} // extern "C"
#endif
