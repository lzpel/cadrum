#pragma once

#include "rust/cxx.h"

// Types used directly in function signatures — keep minimal so that
// the cxx-generated bridge objects do not compile heavy OCCT headers.
#include <TopoDS_Shape.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Edge.hxx>

#include <cstdint>
#include <streambuf>
#include <memory>
#include <vector>

namespace cadrum {

// Type aliases to bring OCCT global types into cadrum namespace.
// Required because the cxx bridge uses namespace = "cadrum".
using TopoDS_Shape = ::TopoDS_Shape;
using TopoDS_Face = ::TopoDS_Face;
using TopoDS_Edge = ::TopoDS_Edge;

// Forward-declare the Rust opaque types (defined by cxx in ffi.rs.h)
struct RustReader;
struct RustWriter;

// Forward-declare shared structs (defined by cxx in ffi.rs.h)
struct MeshData;
// ==================== Streambuf bridges ====================

// std::streambuf subclass that reads from a Rust `dyn Read` via FFI callback
class RustReadStreambuf : public std::streambuf {
public:
    explicit RustReadStreambuf(RustReader& reader) : reader_(reader) {}

protected:
    int_type underflow() override;
    // Override to keep the vtable slot resolved within wrapper.o instead of
    // referencing `std::basic_streambuf<char>::seekpos`, whose mangling depends
    // on `std::fpos<mbstate_t>` — and `mbstate_t` is a typedef to the internal
    // `_Mbstatet` on gcc 15 mingw but a different name on gcc 14, so the
    // external symbol fails to resolve when the prebuilt ships gcc 14
    // libstdc++.a but downstream links with gcc 15.
    pos_type seekpos(pos_type sp, std::ios_base::openmode which = std::ios_base::in | std::ios_base::out) override;

private:
    RustReader& reader_;
    char buf_[8192];
};

// std::streambuf subclass that writes to a Rust `dyn Write` via FFI callback
class RustWriteStreambuf : public std::streambuf {
public:
    explicit RustWriteStreambuf(RustWriter& writer) : writer_(writer) {}

    ~RustWriteStreambuf() override {
        sync();
    }

protected:
    int_type overflow(int_type ch) override;
    std::streamsize xsputn(const char* s, std::streamsize count) override;
    int sync() override;
    // See RustReadStreambuf::seekpos — same gcc 14/15 `_Mbstatet` mangling fix.
    pos_type seekpos(pos_type sp, std::ios_base::openmode which = std::ios_base::in | std::ios_base::out) override;

private:
    bool flush_buf();

    RustWriter& writer_;
    char buf_[8192];
    size_t pos_ = 0;
};

// ==================== Shape I/O (streambuf callback) ====================

// Plain STEP I/O — only built without CADRUM_COLOR; with color, STEP goes
// through XCAF (`read_step_color_stream` etc.) instead.
#ifndef CADRUM_COLOR
std::unique_ptr<TopoDS_Shape> read_step_stream(RustReader& reader);
bool write_step_stream(const TopoDS_Shape& shape, RustWriter& writer);
#endif
std::unique_ptr<TopoDS_Shape> read_brep_bin_stream(RustReader& reader);
bool write_brep_bin_stream(const TopoDS_Shape& shape, RustWriter& writer);
std::unique_ptr<TopoDS_Shape> read_brep_text_stream(RustReader& reader);
bool write_brep_text_stream(const TopoDS_Shape& shape, RustWriter& writer);

// ==================== Shape Constructors ====================

std::unique_ptr<TopoDS_Shape> make_half_space(
    double ox, double oy, double oz,
    double nx, double ny, double nz);

std::unique_ptr<TopoDS_Shape> make_box(
    double x1, double y1, double z1,
    double x2, double y2, double z2);

std::unique_ptr<TopoDS_Shape> make_cylinder(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double radius, double height);

std::unique_ptr<TopoDS_Shape> make_sphere(
    double cx, double cy, double cz,
    double radius);

std::unique_ptr<TopoDS_Shape> make_cone(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2, double height);

std::unique_ptr<TopoDS_Shape> make_torus(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2);

std::unique_ptr<TopoDS_Shape> make_empty();
std::unique_ptr<TopoDS_Shape> deep_copy(const TopoDS_Shape& shape);

// ==================== Boolean Operations ====================

// Unified boolean operation: 0=Fuse(union), 1=Cut(a−b), 2=Common(intersect).
//
// `out_history` receives flat [post_id, src_id, post_id, src_id, ...] pairs
// for every result face derived from either input (a or b). post_id is the
// TShape* of the result face; src_id is the TShape* of the original face in
// whichever input it came from. Self/tool distinction is intentionally
// collapsed — TShape* pointers are globally unique so callers can filter by
// matching src_id against either input's face id set.
std::unique_ptr<TopoDS_Shape> boolean_op(
    const TopoDS_Shape& a, const TopoDS_Shape& b, uint32_t op_kind,
    rust::Vec<uint64_t>& out_history);

// ==================== Shape Methods ====================

// Unify shared faces / collinear edges via ShapeUpgrade_UnifySameDomain.
// `out_history` is appended with flat [new_id, old_id, ...] pairs encoding
// how each old face maps onto the unified result (mirrors `boolean_op`'s
// history layout). The Rust side stores these in `Solid::history` and uses
// them to remap the colormap when the `color` feature is enabled.
std::unique_ptr<TopoDS_Shape> clean_shape(
    const TopoDS_Shape& shape,
    rust::Vec<uint64_t>& out_history);
std::unique_ptr<TopoDS_Shape> translate_shape(
    const TopoDS_Shape& shape, double tx, double ty, double tz);
std::unique_ptr<TopoDS_Shape> rotate_shape(
    const TopoDS_Shape& shape,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle);
std::unique_ptr<TopoDS_Shape> scale_shape(
    const TopoDS_Shape& shape,
    double cx, double cy, double cz,
    double factor);
std::unique_ptr<TopoDS_Shape> mirror_shape(
    const TopoDS_Shape& shape,
    double ox, double oy, double oz,
    double nx, double ny, double nz);
bool shape_is_null(const TopoDS_Shape& shape);
bool shape_is_solid(const TopoDS_Shape& shape);
double shape_volume(const TopoDS_Shape& shape);
double shape_surface_area(const TopoDS_Shape& shape);
void shape_center_of_mass(const TopoDS_Shape& shape,
    double& x, double& y, double& z);
void shape_inertia_tensor(const TopoDS_Shape& shape,
    double& m00, double& m01, double& m02,
    double& m10, double& m11, double& m12,
    double& m20, double& m21, double& m22);
bool shape_contains_point(const TopoDS_Shape& shape, double x, double y, double z);
void shape_bounding_box(const TopoDS_Shape& shape,
    double& xmin, double& ymin, double& zmin,
    double& xmax, double& ymax, double& zmax);

// ==================== Compound Decompose/Compose ====================

std::unique_ptr<std::vector<TopoDS_Shape>> decompose_into_solids(const TopoDS_Shape& shape);
void compound_add(TopoDS_Shape& compound, const TopoDS_Shape& child);

// ==================== Meshing ====================

MeshData mesh_shape(const TopoDS_Shape& shape, double tolerance);

// ==================== Topology enumeration ====================

// One-shot enumeration of unique sub-shapes. `shape_edges` deduplicates
// edges shared between faces (so each edge appears exactly once).
// Callers typically cache the result in a Rust-side OnceLock<Vec<Edge>>.
std::unique_ptr<std::vector<TopoDS_Edge>> shape_edges(const TopoDS_Shape& shape);
std::unique_ptr<std::vector<TopoDS_Face>> shape_faces(const TopoDS_Shape& shape);

// Shallow handle clone — C++ copy-ctor shares the underlying TShape via
// OCCT's ref count. Needed when Rust materializes owned `Shape` / `Edge` /
// `Face` wrappers from the `&TopoDS_*` references yielded by
// `CxxVector::iter()`. Distinct from `deep_copy` / `deep_copy_edge` which
// create new TShapes.
std::unique_ptr<TopoDS_Shape> clone_shape_handle(const TopoDS_Shape& shape);
std::unique_ptr<TopoDS_Edge> clone_edge_handle(const TopoDS_Edge& edge);
std::unique_ptr<TopoDS_Face> clone_face_handle(const TopoDS_Face& face);

// ==================== Edge Methods ====================

// Approximate an edge as a polyline. Takes independent angular/chord
// deflection bounds. Returns a flat xyz `Vec<f64>` (length = 3 * point count).
rust::Vec<double> edge_approximation_segments(
    const TopoDS_Edge& edge, double angular, double chord);

// Construct a single helical edge on a cylindrical surface centered at the
// world origin. `axis` is the cylinder axis direction; `x_ref` is the
// reference direction that anchors the local +X axis of the cylindrical
// frame. The helix starts at `radius * normalize(x_ref - project_on(axis))`
// (i.e. at the +X side of the local frame, z=0) and rises by `height` over
// `height/pitch` turns. `x_ref` must not be parallel to `axis`.
std::unique_ptr<TopoDS_Edge> make_helix_edge(
    double ax, double ay, double az,
    double xrx, double xry, double xrz,
    double radius, double pitch, double height);

// Build a closed polygon from `coords` (flat xyz triples, ≥3 points) and
// return its constituent edges in order. The closing edge from the last
// point back to the first is included.
std::unique_ptr<std::vector<TopoDS_Edge>> make_polygon_edges(
    rust::Slice<const double> coords);

// Construct a closed circular edge of `radius` centered at the world origin,
// lying in the plane normal to `axis`. The local +X axis of the circle's
// frame (which determines the parametric start point) is chosen by OCCT
// from an arbitrary orthogonal direction to `axis`.
std::unique_ptr<TopoDS_Edge> make_circle_edge(
    double ax, double ay, double az, double radius);

// Construct a straight line segment edge from point a to point b.
std::unique_ptr<TopoDS_Edge> make_line_edge(
    double ax, double ay, double az,
    double bx, double by, double bz);

// Construct a circular arc edge through three points (start, mid, end).
// `mid` must not be collinear with `start` and `end`. On degenerate input
// OCCT returns nullptr.
std::unique_ptr<TopoDS_Edge> make_arc_edge(
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
// Returns nullptr on any failure.
std::unique_ptr<TopoDS_Edge> make_bspline_edge(
    rust::Slice<const double> coords,
    uint32_t end_kind,
    double sx, double sy, double sz,
    double ex, double ey, double ez);

// Edge query helpers.
void edge_endpoints(const TopoDS_Edge& edge,
    double& sx, double& sy, double& sz,
    double& ex, double& ey, double& ez);
void edge_tangents(const TopoDS_Edge& edge,
    double& sx, double& sy, double& sz,
    double& ex, double& ey, double& ez);
bool edge_is_closed(const TopoDS_Edge& edge);

// Project a world point onto the edge's underlying curve. Returns false if
// the curve is missing or the projector cannot converge (leaves outputs 0).
bool edge_project_point(const TopoDS_Edge& edge,
    double px, double py, double pz,
    double& cpx, double& cpy, double& cpz,
    double& tx, double& ty, double& tz);

// Edge clone (deep copy of underlying TShape).
std::unique_ptr<TopoDS_Edge> deep_copy_edge(const TopoDS_Edge& edge);

// Edge spatial transforms. Mirror the shape-level helpers but operate on
// TopoDS_Edge directly so the Rust wrapper can stay edge-typed.
std::unique_ptr<TopoDS_Edge> translate_edge(
    const TopoDS_Edge& edge, double tx, double ty, double tz);
std::unique_ptr<TopoDS_Edge> rotate_edge(
    const TopoDS_Edge& edge,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle);
std::unique_ptr<TopoDS_Edge> scale_edge(
    const TopoDS_Edge& edge,
    double cx, double cy, double cz,
    double factor);
std::unique_ptr<TopoDS_Edge> mirror_edge(
    const TopoDS_Edge& edge,
    double ox, double oy, double oz,
    double nx, double ny, double nz);

// Extrude a closed profile wire into a solid using BRepPrimAPI_MakePrism.
// Internally builds Wire → Face → Prism.
std::unique_ptr<TopoDS_Shape> make_extrude(
    const std::vector<TopoDS_Edge>& profile_edges,
    double dx, double dy, double dz);

// Sweep a closed profile wire (built from `profile_edges`) along a spine
// wire (built from `spine_edges`) using BRepOffsetAPI_MakePipeShell. The
// profile is wrapped in a face before sweeping so the result is a Solid.
// Unified MakePipeShell wrapper.  Supports single-profile sweep and
// multi-profile morphing.  Profile sections in `all_edges` are separated
// by null-edge sentinels (TopoDS_Edge().IsNull() == true).
//
// `orient` selects how the profile is oriented along the spine:
//   0 = Fixed   — fix the trihedron to the spine-start frame (no rotation)
//   1 = Torsion — raw Frenet trihedron (helices, springs)
//   2 = Up      — keep `(ux, uy, uz)` as the constant binormal direction
//   3 = Auxiliary — use `aux_spine_edges` as auxiliary spine for twist control
// Any other value falls back to Torsion.
std::unique_ptr<TopoDS_Shape> make_pipe_shell(
    const std::vector<TopoDS_Edge>& all_edges,
    const std::vector<TopoDS_Edge>& spine_edges,
    uint32_t orient,
    double ux, double uy, double uz,
    const std::vector<TopoDS_Edge>& aux_spine_edges);

// Helpers for the Rust side to construct a std::vector<TopoDS_Edge>.
std::unique_ptr<std::vector<TopoDS_Edge>> edge_vec_new();
void edge_vec_push(std::vector<TopoDS_Edge>& v, const TopoDS_Edge& e);
void edge_vec_push_null(std::vector<TopoDS_Edge>& v);

// Helpers for the Rust side to construct a std::vector<TopoDS_Face>.
std::unique_ptr<std::vector<TopoDS_Face>> face_vec_new();
void face_vec_push(std::vector<TopoDS_Face>& v, const TopoDS_Face& f);

// Shell (hollow) the solid by removing `open_faces` and offsetting the
// remaining faces by `thickness` via BRepOffsetAPI_MakeThickSolid. Negative
// thickness hollows inward, positive thickens outward. Returns nullptr on
// failure (e.g. self-intersecting offset at sharp corners).
std::unique_ptr<TopoDS_Shape> make_thick_solid(
    const TopoDS_Shape& solid,
    const std::vector<TopoDS_Face>& open_faces,
    double thickness);

// Fillet the given edges of `solid` with a uniform radius using
// BRepFilletAPI_MakeFillet. Empty `edges` is a no-op (returns a shallow
// copy of `solid`). Returns nullptr on OCCT failure (radius too large,
// tangent discontinuity, edges not belonging to `solid`, etc.).
std::unique_ptr<TopoDS_Shape> make_fillet(
    const TopoDS_Shape& solid,
    const std::vector<TopoDS_Edge>& edges,
    double radius);

// Chamfer (symmetric bevel) the given edges of `solid` with a uniform
// distance using BRepFilletAPI_MakeChamfer. Empty `edges` is a no-op
// (returns a shallow copy of `solid`). Returns nullptr on OCCT failure
// (distance too large, tangent discontinuity, edges not belonging to
// `solid`, etc.).
std::unique_ptr<TopoDS_Shape> make_chamfer(
    const TopoDS_Shape& solid,
    const std::vector<TopoDS_Edge>& edges,
    double distance);

// Loft (skin) a smooth solid through N cross-section wires.
// Sections in `all_edges` are separated by null-edge sentinels.
std::unique_ptr<TopoDS_Shape> make_loft(
    const std::vector<TopoDS_Edge>& all_edges);

// Build a B-spline surface solid from a 2D point grid.
// `coords` is a flat array of xyz triples, length = 3 * nu * nv.
// V direction (cross-section, j index) is always periodic.
// U direction (longitudinal, i index) is periodic iff `u_periodic=true`
// (producing a torus); otherwise the U-ends are capped with planar faces
// (producing a pipe). Returns nullptr on any OCCT failure.
std::unique_ptr<TopoDS_Shape> make_bspline_solid(
    rust::Slice<const double> coords,
    uint32_t nu, uint32_t nv,
    bool u_periodic);

// ==================== Face Methods ====================

// Both helpers return the underlying TopoDS_TShape* address as a u64 — used
// to track face/solid identity across boolean ops, color maps, and BREP I/O.
uint64_t face_tshape_id(const TopoDS_Face& face);
uint64_t shape_tshape_id(const TopoDS_Shape& shape);

} // namespace cadrum

#ifdef CADRUM_COLOR

namespace cadrum {

// ==================== Colored STEP I/O ====================

// Read a colored STEP stream. Returns the geometry shape; appends to
// `out_ids` the TShape* address of each colored face, and to `out_rgb` its
// RGB components in OCC native space (0.0–1.0) as a flat
// [r0,g0,b0, r1,g1,b1, ...] sequence (length = 3 * out_ids.size()).
// Returns nullptr on failure.
std::unique_ptr<TopoDS_Shape> read_step_color_stream(
    RustReader&          reader,
    rust::Vec<uint64_t>& out_ids,
    rust::Vec<float>&    out_rgb);

// Write a colored STEP stream. `ids` lists TShape* of colored faces; `rgb`
// is the matching flat [r,g,b,...] sequence (length = 3 * ids.size()).
bool write_step_color_stream(
    const TopoDS_Shape&         shape,
    rust::Slice<const uint64_t> ids,
    rust::Slice<const float>    rgb,
    RustWriter&                 writer);

} // namespace cadrum

#endif // CADRUM_COLOR
