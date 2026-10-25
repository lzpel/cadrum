#include "cadrum/src/ffi.rs.h"

// ==================== OCCT headers (impl only — not exposed via wrapper.h) ====================
//
// Grouped by responsibility. Anything used in wrapper.h is included there;
// here we only pull in what the implementations need.

// --- Standard / exceptions ---
#include <Standard_Failure.hxx>
#include <stdexcept>

// --- Topology types & navigation ---
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopLoc_Location.hxx>
#include <NCollection_IndexedMap.hxx>
#include <NCollection_DataMap.hxx>
#include <NCollection_Map.hxx>
#include <NCollection_List.hxx>
#include <TopTools_ShapeMapHasher.hxx>

// --- Geometry primitives (gp / Geom / 2d) ---
#include <gp_Ax1.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Pln.hxx>
#include <gp_Sphere.hxx>
#include <gp_Torus.hxx>
#include <gp_Trsf.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_ConicalSurface.hxx>
#include <Geom_SphericalSurface.hxx>
#include <Geom_ToroidalSurface.hxx>
#include <Geom2d_Line.hxx>
#include <Geom_RectangularTrimmedSurface.hxx>
#include <Geom_Line.hxx>
#include <Geom_Circle.hxx>
#include <Geom_Ellipse.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <GeomConvert.hxx>
#include <GeomAdaptor_Curve.hxx>
#include <gp_Elips.hxx>
#include <gp_Lin.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <GC_MakeArcOfCircle.hxx>

// --- BRep builders (faces / wires / edges / solid primitives) ---
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <BRepLib.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <ShapeFix_Edge.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Dir2d.hxx>
#include <ShapeFix_Face.hxx>
#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
#include <BRepExtrema_ExtPF.hxx>
#include <BRepLProp_SLProps.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeHalfSpace.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>

// --- Boolean operations & shape cleanup ---
#include <BOPAlgo_CellsBuilder.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <BRepTools_History.hxx>

// --- Sweep / pipe / loft ---
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepOffsetAPI_MakeOffsetShape.hxx>
#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepOffset_Mode.hxx>
#include <GeomAbs_JoinType.hxx>
#include <GeomAbs_SurfaceType.hxx>

// --- Mesh, classification, mass / surface properties ---
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <Poly_Triangulation.hxx>
#include <BRepBndLib.hxx>
#include <Bnd_Box.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>

// --- Curve adaptation / approximation ---
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_CompCurve.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <GeomAPI_Interpolate.hxx>
#include <Extrema_ExtPC.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BSplineSurface.hxx>
#include <NCollection_Array2.hxx>
#include <NCollection_HArray1.hxx>
#include <Precision.hxx>

// --- I/O (BREP / STEP / progress) ---
#include <BinTools.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_Writer.hxx>
#include <Message_ProgressRange.hxx>
#include <Message.hxx>

// --- C++ standard library ---
#include <istream>
#include <ostream>
#include <sstream>
#include <streambuf>
#include <cmath>
#include <cstring>
#include <cstdint>
#include <algorithm>
#include <unordered_map>
#include <unordered_set>
#include <set>
#include <array>

namespace cadrum {

// OCCT defaults to a stdout printer that emits "Statistics on Transfer" banners on STEP read/write.
// Clear all printers at load time per the documented recommendation.
// ******        Statistics on Transfer (Write)                 ******
static const int _silence_occt_default_printer = []() {
    Message::DefaultMessenger()->ChangePrinters().Clear();
    return 0;
}();

// ==================== Solid <-> TopoDS_Shape narrowing ====================

// Narrow to the SOLID the bridge promises; a container yields its first solid.
static std::unique_ptr<TopoDS_Solid> to_solid(const TopoDS_Shape& shape) {
    if (!shape.IsNull() && shape.ShapeType() == TopAbs_SOLID) return std::make_unique<TopoDS_Solid>(TopoDS::Solid(shape));
    if (!shape.IsNull()) {
        TopExp_Explorer ex(shape, TopAbs_SOLID);
        if (ex.More()) return std::make_unique<TopoDS_Solid>(TopoDS::Solid(ex.Current()));
    }
    throw std::runtime_error("result contains no solid");
}

// The solid under `shape`, or its one closed shell upgraded to a positively oriented
// solid. Face handles are kept, so face history computed on `shape` stays valid.
static std::unique_ptr<TopoDS_Solid> solid_from_shell(const TopoDS_Shape& shape) {
    if (TopExp_Explorer(shape, TopAbs_SOLID).More()) return to_solid(shape);
    TopExp_Explorer ex(shape, TopAbs_SHELL);
    if (!ex.More()) throw std::runtime_error("result contains no shell");
    TopoDS_Shell shell = TopoDS::Shell(ex.Current());
    ex.Next();
    if (ex.More()) throw std::runtime_error("result contains more than one shell");
    if (!BRep_Tool::IsClosed(shell)) throw std::runtime_error("the shell is not closed");
    shell.Closed(true);
    BRepBuilderAPI_MakeSolid solid_maker(shell);
    if (!solid_maker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeSolid did not complete");
    TopoDS_Solid solid = solid_maker.Solid();
    BRepLib::OrientClosedSolid(solid);
    return std::make_unique<TopoDS_Solid>(solid);
}

// Every SOLID under `shape` as shallow handle copies; anything else is dropped.
static std::unique_ptr<std::vector<TopoDS_Solid>> solids_of(const TopoDS_Shape& shape) {
    auto out = std::make_unique<std::vector<TopoDS_Solid>>();
    for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next()) {
        out->push_back(TopoDS::Solid(ex.Current()));
    }
    return out;
}

// The compound OCCT's writers and mesher take in place of a list of solids.
static TopoDS_Compound compound_of(const std::vector<TopoDS_Solid>& solids) {
    BRep_Builder builder;
    TopoDS_Compound compound;
    builder.MakeCompound(compound);
    for (const auto& s : solids) builder.Add(compound, s);
    return compound;
}

// ==================== Identity ====================

// A sub-shape's id as seen from its owner: the TShape address, mixed with the hash of
// its location relative to the owner when that location is not the identity.
static uint64_t sub_id(const TopoDS_Shape& sub, const TopLoc_Location& owner_loc) {
    const uint64_t ptr = static_cast<uint64_t>(reinterpret_cast<uintptr_t>(sub.TShape().get()));
    const TopLoc_Location rel = sub.Location().Predivided(owner_loc);
    if (rel.IsIdentity()) return ptr;
    return ptr ^ (static_cast<uint64_t>(rel.HashCode()) * 0x9E3779B97F4A7C15ULL);
}

// A shape's own id: its TShape address, whatever its location.
static uint64_t own_id(const TopoDS_Shape& shape) {
    return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(shape.TShape().get()));
}

// ==================== History ====================
//
// Every builder reports how the faces and edges of its operands relate to those of
// its result as [kind, src, post] triples (kinds below; a Deleted entry has post 0).
// `src` is the sub-shape's id relative to its operand, `post` relative to the result
// solid that holds it, so both match what `Solid::iter_face` / `iter_edge` report.

enum : uint64_t { LINEAGE_KEPT = 0, LINEAGE_MODIFIED = 1, LINEAGE_GENERATED = 2, LINEAGE_DELETED = 3 };

using IdMap = NCollection_DataMap<TopoDS_Shape, uint64_t, TopTools_ShapeMapHasher>;

// One operand sub-shape: the id the caller knows it by, and the shape to ask the
// algorithm about. `copied` marks a probe that is already a copy of the caller's
// shape, so finding it unchanged in the result is a Modified, not a Kept.
struct SrcEntry {
    uint64_t id;
    TopoDS_Shape probe;
    bool copied = false;
};

// The faces and edges of `owner`, ids relative to it.
static std::vector<SrcEntry> src_of(const TopoDS_Shape& owner) {
    std::vector<SrcEntry> out;
    NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> seen;
    for (TopAbs_ShapeEnum type : {TopAbs_FACE, TopAbs_EDGE}) {
        for (TopExp_Explorer ex(owner, type); ex.More(); ex.Next()) {
            if (seen.Add(ex.Current())) out.push_back({sub_id(ex.Current(), owner.Location()), ex.Current()});
        }
    }
    return out;
}

// {absolute face or edge → id relative to the result solid holding it} over every
// solid under `result`.
static IdMap index_result(const TopoDS_Shape& result) {
    IdMap out;
    auto add = [&](const TopoDS_Shape& owner) {
        for (TopAbs_ShapeEnum type : {TopAbs_FACE, TopAbs_EDGE}) {
            for (TopExp_Explorer ex(owner, type); ex.More(); ex.Next()) {
                if (!out.IsBound(ex.Current())) out.Bind(ex.Current(), sub_id(ex.Current(), owner.Location()));
            }
        }
    };
    if (result.ShapeType() == TopAbs_SOLID) add(result);
    for (TopExp_Explorer ex(result, TopAbs_SOLID); ex.More(); ex.Next()) add(ex.Current());
    return out;
}

// Emit the lineage of every `src` entry: Kept when the probe itself is in the result,
// Modified / Generated for each image `algo` reports there, and Deleted when the shape
// itself has no image (a Generated face does not keep its source edge alive).
// `Algo` is anything with `Modified(S)` and `Generated(S)` returning shape lists.
template <typename Algo>
static void relay_history(Algo& algo, const std::vector<SrcEntry>& src, const IdMap& post, rust::Vec<uint64_t>& out)
{
    for (const SrcEntry& e : src) {
        std::set<std::pair<uint64_t, uint64_t>> seen;
        auto emit = [&](uint64_t kind, uint64_t p) {
            if (!seen.insert({kind, p}).second) return;
            out.push_back(kind);
            out.push_back(e.id);
            out.push_back(p);
        };
        if (const uint64_t* p = post.Seek(e.probe)) emit(e.copied ? LINEAGE_MODIFIED : LINEAGE_KEPT, *p);
        for (uint64_t kind : {LINEAGE_MODIFIED, LINEAGE_GENERATED}) {
            NCollection_List<TopoDS_Shape> images;
            // Builders differ in what they accept; an unknown shape is simply no image.
            try {
                images = kind == LINEAGE_MODIFIED ? algo.Modified(e.probe) : algo.Generated(e.probe);
            } catch (const Standard_Failure&) {
            }
            for (const TopoDS_Shape& image : images) {
                if (const uint64_t* p = post.Seek(image)) emit(kind, *p);
            }
        }
        const bool survives = std::any_of(seen.begin(), seen.end(), [](const auto& kp) { return kp.first != LINEAGE_GENERATED; });
        if (!survives) {
            out.push_back(LINEAGE_DELETED);
            out.push_back(e.id);
            out.push_back(0);
        }
    }
}

// An algorithm that changed nothing: every operand sub-shape found in the result is Kept.
struct NoAlgo {
    NCollection_List<TopoDS_Shape> none;
    const NCollection_List<TopoDS_Shape>& Modified(const TopoDS_Shape&) { return none; }
    const NCollection_List<TopoDS_Shape>& Generated(const TopoDS_Shape&) { return none; }
};

// MakePrism / MakeRevol report the side face as Generated from a profile edge; the
// edge's image on the far cap is `LastShape(edge)`, added here as Generated too.
template <typename Sweep>
struct SweepEnds {
    Sweep& sweep;
    NCollection_List<TopoDS_Shape> list;
    const NCollection_List<TopoDS_Shape>& Modified(const TopoDS_Shape& s) { return sweep.Modified(s); }
    const NCollection_List<TopoDS_Shape>& Generated(const TopoDS_Shape& s) {
        list = sweep.Generated(s);
        const TopoDS_Shape last = sweep.LastShape(s);
        if (!last.IsNull()) list.Append(last);
        return list;
    }
};

// BRepBuilderAPI_Sewing has its own history accessors; this gives them the common shape.
struct SewingHistory {
    BRepBuilderAPI_Sewing& sewing;
    NCollection_List<TopoDS_Shape> list;
    const NCollection_List<TopoDS_Shape>& Modified(const TopoDS_Shape& s) {
        list.Clear();
        if (sewing.IsModified(s)) list.Append(sewing.Modified(s));
        else if (sewing.IsModifiedSubShape(s)) list.Append(sewing.ModifiedSubShape(s));
        return list;
    }
    const NCollection_List<TopoDS_Shape>& Generated(const TopoDS_Shape&) { return list = NCollection_List<TopoDS_Shape>(); }
};

// ==================== Wires from edges ====================

// Copies of the caller's edges chained into wires. The copies take the vertex and
// pcurve fixes the builders apply, so the caller's edges stay untouched; `src` maps
// each caller edge, by its own id, to the copy the wire holds.
struct Chained {
    std::vector<TopoDS_Wire> wires;
    std::vector<SrcEntry> src;
};

// Each edge is taken whichever way meets the chain within `tolerance`. A wire closes
// when it returns to its start; trailing edges make an open last wire.
static Chained chain(const std::vector<TopoDS_Edge>& edges, double tolerance)
{
    auto near = [tolerance](const gp_Pnt& p, const gp_Pnt& q) { return p.Distance(q) <= tolerance; };
    Chained out;
    std::vector<std::pair<TopoDS_Edge, size_t>> link;
    gp_Pnt start, end;
    auto flush = [&]() {
        BRepBuilderAPI_MakeWire maker;
        for (const auto& [e, i] : link) {
            maker.Add(e);
            if (!maker.IsDone()) throw std::runtime_error("edges do not connect into a wire");
            // MakeWire may have copied the edge again to share a vertex; Edge() is what it holds.
            out.src.push_back({own_id(edges[i]), maker.Edge(), true});
        }
        out.wires.push_back(maker.Wire());
        link.clear();
    };
    for (size_t i = 0; i < edges.size(); ++i) {
        // An edge runs along its curve, or against it when a face's loop reversed it.
        BRepAdaptor_Curve curve(edges[i]);
        const bool reversed = edges[i].Orientation() == TopAbs_REVERSED;
        const gp_Pnt a = curve.Value(reversed ? curve.LastParameter() : curve.FirstParameter()), b = curve.Value(reversed ? curve.FirstParameter() : curve.LastParameter());
        TopoDS_Edge e = TopoDS::Edge(BRepBuilderAPI_Copy(edges[i], false).Shape().Oriented(reversed ? TopAbs_REVERSED : TopAbs_FORWARD));
        if (link.empty()) {
            start = a;
            end = b;
        } else {
            // The first edge's direction is settled by the second.
            if (link.size() == 1 && !near(end, a) && !near(end, b) && (near(start, a) || near(start, b))) {
                link.front().first.Reverse();
                std::swap(start, end);
            }
            if (near(end, a)) {
                end = b;
            } else if (near(end, b)) {
                e.Reverse();
                end = a;
            } else {
                throw std::runtime_error("an edge meets neither end of the previous one");
            }
        }
        link.push_back({e, i});
        if (near(end, start)) flush();
    }
    if (!link.empty()) flush();
    return out;
}

// ==================== NURBS encoding ====================
//
// A B-spline crosses the bridge as one flat double vector; the layouts are mirrored
// by `NurbsSurface::to_flat` / `NurbsCurve::to_flat` in src/common/geometry.rs.

static int as_int(double v) { return static_cast<int>(std::llround(v)); }

// Reads successive values of a flat vector, throwing once the data runs out.
struct FlatReader {
    rust::Slice<const double> data;
    size_t at = 0;
    double next() {
        if (at >= data.size()) throw std::runtime_error("truncated NURBS data");
        return data[at++];
    }
    int count() { return as_int(next()); }
};

// [udeg, vdeg, uper, vper, nuk, nvk, nu, nv, uknots, umults, vknots, vmults, poles xyz (u-major), weights]
static void encode_bspline_surface(const Handle(Geom_BSplineSurface)& s, rust::Vec<double>& out) {
    const int nu = s->NbUPoles(), nv = s->NbVPoles(), nuk = s->NbUKnots(), nvk = s->NbVKnots();
    for (double v : {double(s->UDegree()), double(s->VDegree()), s->IsUPeriodic() ? 1.0 : 0.0, s->IsVPeriodic() ? 1.0 : 0.0, double(nuk), double(nvk), double(nu), double(nv)}) out.push_back(v);
    for (double k : s->UKnots()) out.push_back(k);
    for (int m : s->UMultiplicities()) out.push_back(m);
    for (double k : s->VKnots()) out.push_back(k);
    for (int m : s->VMultiplicities()) out.push_back(m);
    const NCollection_Array2<gp_Pnt>& poles = s->Poles();
    const NCollection_Array2<double>* weights = s->Weights();  // null when not rational
    for (int i = 1; i <= nu; ++i) for (int j = 1; j <= nv; ++j) { const gp_Pnt& p = poles(i, j); out.push_back(p.X()); out.push_back(p.Y()); out.push_back(p.Z()); }
    for (int i = 1; i <= nu; ++i) for (int j = 1; j <= nv; ++j) out.push_back(weights ? (*weights)(i, j) : 1.0);
}

static Handle(Geom_BSplineSurface) decode_bspline_surface(rust::Slice<const double> data) {
    FlatReader r{data};
    const int udeg = r.count(), vdeg = r.count();
    const bool uper = r.next() != 0.0, vper = r.next() != 0.0;
    const int nuk = r.count(), nvk = r.count(), nu = r.count(), nv = r.count();
    if (nuk < 2 || nvk < 2 || nu < 2 || nv < 2) throw std::runtime_error("a B-spline surface needs at least two knots and two poles per direction");
    NCollection_Array1<double> uk(1, nuk), vk(1, nvk);
    NCollection_Array1<int> um(1, nuk), vm(1, nvk);
    for (int i = 1; i <= nuk; ++i) uk(i) = r.next();
    for (int i = 1; i <= nuk; ++i) um(i) = r.count();
    for (int i = 1; i <= nvk; ++i) vk(i) = r.next();
    for (int i = 1; i <= nvk; ++i) vm(i) = r.count();
    NCollection_Array2<gp_Pnt> poles(1, nu, 1, nv);
    for (int i = 1; i <= nu; ++i) for (int j = 1; j <= nv; ++j) { const double x = r.next(), y = r.next(), z = r.next(); poles(i, j) = gp_Pnt(x, y, z); }
    NCollection_Array2<double> weights(1, nu, 1, nv);
    for (int i = 1; i <= nu; ++i) for (int j = 1; j <= nv; ++j) weights(i, j) = r.next();
    return new Geom_BSplineSurface(poles, weights, uk, vk, um, vm, udeg, vdeg, uper, vper);
}

// [deg, periodic, nk, np, knots, mults, poles xyz, weights]
static void encode_bspline_curve(const Handle(Geom_BSplineCurve)& c, rust::Vec<double>& out) {
    const int nk = c->NbKnots(), np = c->NbPoles();
    for (double v : {double(c->Degree()), c->IsPeriodic() ? 1.0 : 0.0, double(nk), double(np)}) out.push_back(v);
    for (double k : c->Knots()) out.push_back(k);
    for (int m : c->Multiplicities()) out.push_back(m);
    const NCollection_Array1<gp_Pnt>& poles = c->Poles();
    const NCollection_Array1<double>* weights = c->Weights();  // null when not rational
    for (int i = 1; i <= np; ++i) { out.push_back(poles(i).X()); out.push_back(poles(i).Y()); out.push_back(poles(i).Z()); }
    for (int i = 1; i <= np; ++i) out.push_back(weights ? (*weights)(i) : 1.0);
}

static Handle(Geom_BSplineCurve) decode_bspline_curve(rust::Slice<const double> data) {
    FlatReader r{data};
    const int deg = r.count();
    const bool periodic = r.next() != 0.0;
    const int nk = r.count(), np = r.count();
    if (nk < 2 || np < 2) throw std::runtime_error("a B-spline curve needs at least two knots and two poles");
    NCollection_Array1<double> knots(1, nk);
    NCollection_Array1<int> mults(1, nk);
    for (int i = 1; i <= nk; ++i) knots(i) = r.next();
    for (int i = 1; i <= nk; ++i) mults(i) = r.count();
    NCollection_Array1<gp_Pnt> poles(1, np);
    for (int i = 1; i <= np; ++i) { const double x = r.next(), y = r.next(), z = r.next(); poles(i) = gp_Pnt(x, y, z); }
    NCollection_Array1<double> weights(1, np);
    for (int i = 1; i <= np; ++i) weights(i) = r.next();
    return new Geom_BSplineCurve(poles, weights, knots, mults, deg, periodic);
}

// Append a placement as [origin, z, x] and two parameters.
static void push_frame(const gp_Pnt& o, const gp_Dir& z, const gp_Dir& x, double p1, double p2, rust::Vec<double>& out) {
    for (double v : {o.X(), o.Y(), o.Z(), z.X(), z.Y(), z.Z(), x.X(), x.Y(), x.Z(), p1, p2}) out.push_back(v);
}

// ==================== Shape Constructors ====================

std::unique_ptr<TopoDS_Solid> make_half_space(
    double ox, double oy, double oz,
    double nx, double ny, double nz)
{
    gp_Pnt origin(ox, oy, oz);
    gp_Dir normal(nx, ny, nz);
    gp_Pln plane(origin, normal);

    BRepBuilderAPI_MakeFace face_maker(plane);
    TopoDS_Face face = face_maker.Face();

    // Reference point is on the SAME side as the normal.
    // BRepPrimAPI_MakeHalfSpace fills the ref_point side,
    // so the solid occupies the half-space where the normal points.
    double len = std::sqrt(nx*nx + ny*ny + nz*nz);
    gp_Pnt ref_point(ox + nx/len, oy + ny/len, oz + nz/len);

    BRepPrimAPI_MakeHalfSpace maker(face, ref_point);
    return std::make_unique<TopoDS_Solid>(maker.Solid());
}

static Handle(Geom_Surface) surface_of(uint32_t kind, double ox, double oy, double oz, double zx, double zy, double zz, double xx, double xy, double xz, double p1, double p2);

std::unique_ptr<TopoDS_Face> make_face(
    uint32_t kind,
    double ox, double oy, double oz,
    double zx, double zy, double zz,
    double xx, double xy, double xz,
    double p1, double p2,
    rust::Slice<const double> data,
    const std::vector<TopoDS_Edge>& edges)
{
    const std::vector<TopoDS_Wire> wires = chain(edges, Precision::Confusion()).wires;
    Handle(Geom_Surface) surface = kind == 6 ? Handle(Geom_Surface)(decode_bspline_surface(data)) : surface_of(kind, ox, oy, oz, zx, zy, zz, xx, xy, xz, p1, p2);
    if (!wires.empty() && !BRep_Tool::IsClosed(wires.back())) throw std::runtime_error("trailing edges do not close a loop");
    if (wires.empty()) {
        // Only a sphere, a torus or a B-spline is bounded without a boundary.
        if (kind != 4 && kind != 5 && kind != 6) throw std::runtime_error("empty profile on an unbounded surface");
        return std::make_unique<TopoDS_Face>(BRepBuilderAPI_MakeFace(surface, Precision::Confusion()).Face());
    }
    BRep_Builder builder;
    TopoDS_Face bare;
    builder.MakeFace(bare, surface, Precision::Confusion());
    for (const TopoDS_Wire& w : wires) builder.Add(bare, w);
    ShapeFix_Edge edge_fixer;
    for (TopExp_Explorer x(bare, TopAbs_EDGE); x.More(); x.Next()) edge_fixer.FixAddPCurve(TopoDS::Edge(x.Current()), bare, false);
    if (kind == 3) {
        // A single ring round a cone closes at the apex, which ShapeFix does not
        // add as it does a sphere's pole: give it a degenerate edge there.
        int rings = 0;
        for (const TopoDS_Wire& w : wires) {
            double du = 0;
            for (TopExp_Explorer x(w, TopAbs_EDGE); x.More(); x.Next()) {
                double f, l;
                Handle(Geom2d_Curve) c = BRep_Tool::CurveOnSurface(TopoDS::Edge(x.Current()), bare, f, l);
                du += c->Value(l).X() - c->Value(f).X();
            }
            if (std::abs(std::abs(du) - 2 * M_PI) < 0.1) ++rings;
        }
        if (rings == 1) {
            Handle(Geom_ConicalSurface) cone = Handle(Geom_ConicalSurface)::DownCast(surface);
            TopoDS_Edge apex;
            builder.MakeEdge(apex);
            builder.Degenerated(apex, true);
            builder.UpdateEdge(apex, new Geom2d_Line(gp_Pnt2d(0, -cone->RefRadius() / std::sin(cone->SemiAngle())), gp_Dir2d(1, 0)), bare, Precision::Confusion());
            builder.Range(apex, bare, 0, 2 * M_PI);
            TopoDS_Vertex v;
            builder.MakeVertex(v, cone->Apex(), Precision::Confusion());
            builder.Add(apex, v.Oriented(TopAbs_FORWARD));
            builder.Add(apex, v.Oriented(TopAbs_REVERSED));
            TopoDS_Wire w;
            builder.MakeWire(w);
            builder.Add(w, apex);
            builder.Add(bare, w);
        }
    }
    // A loop round a periodic direction needs a seam, and Perform alters the
    // wires so that FixMissingSeam no longer recognises them; seam first.
    ShapeFix_Face fixer(bare);
    fixer.FixMissingSeam();
    // As in STEP, the face lies left of each loop seen from the surface normal.
    fixer.FixOrientationMode() = 0;
    fixer.Perform();
    TopoDS_Shape fixed = fixer.Result();
    if (fixed.IsNull() || fixed.ShapeType() != TopAbs_FACE) throw std::runtime_error("loops bound no single region");
    const TopoDS_Face& face = TopoDS::Face(fixed);
    GProp_GProps props;
    BRepGProp::SurfaceProperties(face, props);
    if (!(props.Mass() > 0) || !BRepCheck_Analyzer(face).IsValid()) throw std::runtime_error("loops bound no finite region: wind each loop counterclockwise about the surface normal round the face, so holes run clockwise");
    // The fix widens an edge's tolerance to cover its distance from the surface.
    if (BRep_Tool::MaxTolerance(face, TopAbs_EDGE) > 1.0e-4) throw std::runtime_error("profile does not lie on the surface");
    return std::make_unique<TopoDS_Face>(face);
}

// Same placement and kind codes as face_surface, so a face round-trips through it.
static Handle(Geom_Surface) surface_of(
    uint32_t kind,
    double ox, double oy, double oz,
    double zx, double zy, double zz,
    double xx, double xy, double xz,
    double p1, double p2)
{
    // OCCT only rejects negative radii; a zero one gives a face with no area.
    const bool has_radius = kind != 1 && kind != 3;
    if (has_radius && !(p1 > Precision::Confusion())) throw std::runtime_error("radius must be positive");
    if (kind == 5 && !(p2 > Precision::Confusion())) throw std::runtime_error("minor radius must be positive");
    const gp_Ax3 pos(gp_Pnt(ox, oy, oz), gp_Dir(zx, zy, zz), gp_Dir(xx, xy, xz));
    switch (kind) {
        case 1: return new Geom_Plane(pos);
        case 2: return new Geom_CylindricalSurface(pos, p1);
        case 3: return new Geom_ConicalSurface(pos, p2, p1);
        case 4: return new Geom_SphericalSurface(pos, p1);
        case 5: return new Geom_ToroidalSurface(pos, p1, p2);
        default: throw std::runtime_error("unknown surface kind");
    }
}

std::unique_ptr<TopoDS_Solid> make_box(
    double x1, double y1, double z1,
    double x2, double y2, double z2)
{
    double minx = std::min(x1, x2);
    double miny = std::min(y1, y2);
    double minz = std::min(z1, z2);
    double maxx = std::max(x1, x2);
    double maxy = std::max(y1, y2);
    double maxz = std::max(z1, z2);

    gp_Pnt p_min(minx, miny, minz);
    double dx = maxx - minx;
    double dy = maxy - miny;
    double dz = maxz - minz;
    if (dx < Precision::Confusion() || dy < Precision::Confusion() || dz < Precision::Confusion()) throw std::runtime_error("the box has a zero extent");

    BRepPrimAPI_MakeBox maker(p_min, dx, dy, dz);
    return std::make_unique<TopoDS_Solid>(maker.Solid());
}

std::unique_ptr<TopoDS_Solid> make_cylinder(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double radius, double height)
{
    if (!(radius > 0.0)) throw std::runtime_error("the radius must be positive");
    if (!(height > Precision::Confusion())) throw std::runtime_error("the height must be positive");
    gp_Pnt center(px, py, pz);
    gp_Dir direction(dx, dy, dz);
    gp_Ax2 axis(center, direction);

    BRepPrimAPI_MakeCylinder maker(axis, radius, height);
    return std::make_unique<TopoDS_Solid>(maker.Solid());
}

std::unique_ptr<TopoDS_Solid> make_sphere(
    double cx, double cy, double cz,
    double radius)
{
    if (!(radius > 0.0)) throw std::runtime_error("the radius must be positive");
    gp_Pnt center(cx, cy, cz);
    BRepPrimAPI_MakeSphere maker(center, radius);
    return std::make_unique<TopoDS_Solid>(maker.Solid());
}

std::unique_ptr<TopoDS_Solid> make_cone(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2, double height)
{
    if (r1 < 0.0 || r2 < 0.0 || !(r1 > 0.0 || r2 > 0.0)) throw std::runtime_error("a cone needs non-negative radii, one of them positive");
    if (std::abs(r1 - r2) < Precision::Confusion()) throw std::runtime_error("equal radii make a cylinder, not a cone");
    if (!(height > Precision::Confusion())) throw std::runtime_error("the height must be positive");
    gp_Pnt center(px, py, pz);
    gp_Dir direction(dx, dy, dz);
    gp_Ax2 axis(center, direction);
    BRepPrimAPI_MakeCone maker(axis, r1, r2, height);
    return std::make_unique<TopoDS_Solid>(maker.Solid());
}

std::unique_ptr<TopoDS_Solid> make_torus(
    double px, double py, double pz,
    double dx, double dy, double dz,
    double r1, double r2)
{
    if (!(r2 > 0.0) || !(r1 > r2)) throw std::runtime_error("a torus needs 0 < minor radius < major radius");
    gp_Pnt center(px, py, pz);
    gp_Dir direction(dx, dy, dz);
    gp_Ax2 axis(center, direction);
    BRepPrimAPI_MakeTorus maker(axis, r1, r2);
    return std::make_unique<TopoDS_Solid>(maker.Solid());
}

// ==================== STEP read post-processing ====================

// Recover Solids from a STEP-read Compound that has disjoint shells / loose
// faces (multi-color export from SolveSpace etc.). Returns the original
// compound if no orphan faces are found (= valid STEP, zero overhead).
//
// If `colorMap` is non-null, remaps its keys for faces whose TShape* changed
// during sewing (only applicable when called from the color path).
//
// See #129 for the reproducer and root-cause analysis.
//
// Design notes:
//   - has_orphans を残す理由: SewedShape().IsNull() でも判定可能だが、
//     (a) 空入力 Perform() を回避、(b) 「sewing 不要」と「sewing 失敗」を区別、
//     の 2 点で明示フラグ優位。
//   - Solid 配下の face を sewer に入れない理由: 既存 valid Solid の face は
//     TShape* preserve したい (colormap キー保持 + 既存挙動互換)。
//   - TopoDS_Iterator (immediate children) ではなく TopExp_Explorer (再帰) を
//     使う理由: STEP のツリー構造で valid Solid が深い所に埋まり、その兄弟に
//     orphan face がある混在ケース (例: Compound { sub { Solid + face×6 } })
//     を救うため。
//   - tolerance = Precision::Confusion(): 重複 EDGE_CURVE は座標完全一致なので
//     最厳設定で十分。緩めると意図しない縫合リスクが増す。
static TopoDS_Shape try_sew_orphan_faces(
    const TopoDS_Shape& compound,
    std::unordered_map<uint64_t, std::array<float, 3>>* colorMap)
{
    // 1. 既存 Solid と配下 face TShape* 集合を回収
    std::unordered_set<const TopoDS_TShape*> in_solid;
    std::vector<TopoDS_Shape> existing_solids;
    for (TopExp_Explorer sx(compound, TopAbs_SOLID); sx.More(); sx.Next()) {
        existing_solids.push_back(sx.Current());
        for (TopExp_Explorer fx(sx.Current(), TopAbs_FACE); fx.More(); fx.Next()) {
            in_solid.insert(fx.Current().TShape().get());
        }
    }

    // 2. 孤立 face を Sewing に投入
    BRepBuilderAPI_Sewing sewer(Precision::Confusion());
    bool has_orphans = false;
    std::vector<TopoDS_Shape> orphan_faces;  // color remap 用に保持
    for (TopExp_Explorer fx(compound, TopAbs_FACE); fx.More(); fx.Next()) {
        if (in_solid.count(fx.Current().TShape().get()) == 0) {
            sewer.Add(fx.Current());
            orphan_faces.push_back(fx.Current());
            has_orphans = true;
        }
    }

    // 3. 正常 STEP は素通し (= zero-overhead)
    if (!has_orphans) return compound;

    // 4. 縫合 → Shell ごとに MakeSolid → 新 compound 構築
    sewer.Perform();
    TopoDS_Shape sewn = sewer.SewedShape();

    BRep_Builder bb;
    TopoDS_Compound new_compound;
    bb.MakeCompound(new_compound);
    for (const auto& s : existing_solids) bb.Add(new_compound, s);
    for (TopExp_Explorer sx(sewn, TopAbs_SHELL); sx.More(); sx.Next()) {
        BRepBuilderAPI_MakeSolid mk(TopoDS::Shell(sx.Current()));
        if (mk.IsDone()) bb.Add(new_compound, mk.Solid());
    }

    // 5. colormap キー remap (color path のみ)
    if (colorMap) {
        for (const auto& old_face : orphan_faces) {
            uint64_t old_id = reinterpret_cast<uint64_t>(old_face.TShape().get());
            auto it = colorMap->find(old_id);
            if (it == colorMap->end()) continue;
            if (sewer.IsModified(old_face)) {
                uint64_t new_id = reinterpret_cast<uint64_t>(
                    sewer.Modified(old_face).TShape().get());
                (*colorMap)[new_id] = it->second;
            }
        }
    }

    return new_compound;
}

// ==================== Builders (solid → solid with history) ====================

// Evaluate any boolean expression in DNF on N solids via BOPAlgo_CellsBuilder.
// 1 回の Perform() で全交差を計算し、clause ごとに AddToResult を呼ぶ。
std::unique_ptr<std::vector<TopoDS_Solid>> builder_cells(
    const std::vector<TopoDS_Solid>& solids,
    rust::Slice<const int64_t> clauses,
    rust::Vec<uint64_t>& out_history)
{
    if (solids.empty() || clauses.size() == 0) throw std::runtime_error("no solids or clauses");

    std::vector<SrcEntry> src;
    for (const auto& s : solids) {
        for (SrcEntry& e : src_of(s)) src.push_back(std::move(e));
    }

    // BOPAlgo_CellsBuilder は引数 N≥2 を想定するため、単一 solid の場合は
    // そのまま返す (DNF clause が `[+1, 0]` の単純ケース)。
    if (solids.size() == 1 && clauses.size() == 2 && clauses[0] == 1 && clauses[1] == 0) {
        NoAlgo none;
        relay_history(none, src, index_result(solids[0]), out_history);
        return std::make_unique<std::vector<TopoDS_Solid>>(solids);
    }

    BOPAlgo_CellsBuilder cb;
    NCollection_List<TopoDS_Shape> args;
    for (const auto& s : solids) args.Append(s);
    cb.SetArguments(args);
    // The operands are shared with the caller, so they must come out untouched.
    cb.SetNonDestructive(true);
    cb.Perform();
    if (cb.HasErrors()) throw std::runtime_error("BOPAlgo_CellsBuilder reported errors");

    const int material = 1;
    NCollection_List<TopoDS_Shape> take, avoid;
    for (size_t i = 0; i < clauses.size(); ++i) {
        int64_t lit = clauses[i];
        if (lit == 0) {
            if (!take.IsEmpty()) {
                cb.AddToResult(take, avoid, material);
            }
            take.Clear();
            avoid.Clear();
            continue;
        }
        int64_t idx = (lit > 0 ? lit : -lit) - 1;
        if (idx < 0 || idx >= static_cast<int64_t>(solids.size())) throw std::runtime_error("clause literal out of range");
        if (lit > 0) take.Append(solids[static_cast<size_t>(idx)]);
        else         avoid.Append(solids[static_cast<size_t>(idx)]);
    }
    cb.RemoveInternalBoundaries();

    // The result shares every untouched face with its operand, so those are Kept.
    relay_history(cb, src, index_result(cb.Shape()), out_history);
    return solids_of(cb.Shape());
}

// Unify shared faces / collinear edges. `out_history` is populated with
// flat [new_id, old_id, ...] pairs covering every old face that survived
// (either unchanged, or merged into a result face); identical layout to
// `builder_boolean`'s history.
std::unique_ptr<TopoDS_Solid> builder_clean(
    const TopoDS_Solid& shape,
    rust::Vec<uint64_t>& out_history)
{
    ShapeUpgrade_UnifySameDomain unifier(shape, true, true, true);
    unifier.AllowInternalEdges(false);
    unifier.Build();

    auto result = to_solid(unifier.Shape());
    Handle(BRepTools_History) history = unifier.History();
    if (history.IsNull()) {
        NoAlgo none;
        relay_history(none, src_of(shape), index_result(*result), out_history);
    } else {
        relay_history(*history, src_of(shape), index_result(*result), out_history);
    }
    return result;
}

// ==================== Transforms (solid → solid, no history) ====================

std::unique_ptr<TopoDS_Solid> transform_translate(
    const TopoDS_Solid& shape,
    double tx, double ty, double tz)
{
    gp_Trsf trsf;
    trsf.SetTranslation(gp_Vec(tx, ty, tz));
    return to_solid(shape.Moved(TopLoc_Location(trsf)));
}

std::unique_ptr<TopoDS_Solid> transform_rotate(
    const TopoDS_Solid& shape,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle)
{
    gp_Trsf trsf;
    trsf.SetRotation(gp_Ax1(gp_Pnt(ox, oy, oz), gp_Dir(dx, dy, dz)), angle);
    return to_solid(shape.Moved(TopLoc_Location(trsf)));
}

std::unique_ptr<TopoDS_Solid> transform_scale(
    const TopoDS_Solid& shape,
    double cx, double cy, double cz,
    double factor,
    rust::Vec<uint64_t>& out_history)
{
    if (std::abs(factor) < Precision::Confusion()) throw std::runtime_error("the scale factor is zero");
    gp_Trsf trsf;
    trsf.SetScale(gp_Pnt(cx, cy, cz), factor);
    BRepBuilderAPI_Transform transform(shape, trsf, true);
    auto out = to_solid(transform.Shape());
    relay_history(transform, src_of(shape), index_result(*out), out_history);
    return out;
}

std::unique_ptr<TopoDS_Solid> transform_mirror(
    const TopoDS_Solid& shape,
    double ox, double oy, double oz,
    double nx, double ny, double nz,
    rust::Vec<uint64_t>& out_history)
{
    gp_Trsf trsf;
    trsf.SetMirror(gp_Ax2(gp_Pnt(ox, oy, oz), gp_Dir(nx, ny, nz)));
    BRepBuilderAPI_Transform transform(shape, trsf, true);
    auto out = to_solid(transform.Shape());
    relay_history(transform, src_of(shape), index_result(*out), out_history);
    return out;
}

// ==================== Shape Queries ====================

double shape_volume(const TopoDS_Solid& shape) {
    GProp_GProps props;
    BRepGProp::VolumeProperties(shape, props);
    return props.Mass();
}

void shape_center_of_mass(const TopoDS_Solid& shape,
    double& x, double& y, double& z)
{
    GProp_GProps props;
    BRepGProp::VolumeProperties(shape, props);
    gp_Pnt com = props.CentreOfMass();
    x = com.X(); y = com.Y(); z = com.Z();
}

void shape_inertia_tensor(const TopoDS_Solid& shape,
    double& m00, double& m01, double& m02,
    double& m10, double& m11, double& m12,
    double& m20, double& m21, double& m22)
{
    // OCCT's MatrixOfInertia() is expressed about the center of mass, but the
    // Rust-side API returns the tensor about the world origin so collections
    // can aggregate by plain matrix sum (parallel-axis theorem is already
    // folded in). Shift here with I_world = I_com + m·(|d|² I - d⊗d),
    // where d = COM vector from world origin, m = volume (uniform density).
    GProp_GProps props;
    BRepGProp::VolumeProperties(shape, props);
    gp_Mat ic = props.MatrixOfInertia();
    gp_Pnt com = props.CentreOfMass();
    double mass = props.Mass();
    double dx = com.X(), dy = com.Y(), dz = com.Z();
    double d2 = dx*dx + dy*dy + dz*dz;
    m00 = ic.Value(1,1) + mass * (d2 - dx*dx);
    m11 = ic.Value(2,2) + mass * (d2 - dy*dy);
    m22 = ic.Value(3,3) + mass * (d2 - dz*dz);
    m01 = ic.Value(1,2) - mass * dx * dy;
    m02 = ic.Value(1,3) - mass * dx * dz;
    m12 = ic.Value(2,3) - mass * dy * dz;
    m10 = m01; m20 = m02; m21 = m12;
}

bool shape_contains_point(const TopoDS_Solid& shape, double x, double y, double z) {
    BRepClass3d_SolidClassifier classifier(shape, gp_Pnt(x, y, z), 1e-6);
    return classifier.State() == TopAbs_IN;
}

void shape_bounding_box(const TopoDS_Solid& shape,
    double& xmin, double& ymin, double& zmin,
    double& xmax, double& ymax, double& zmax)
{
    Bnd_Box box;
    BRepBndLib::Add(shape, box);
    box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
}

// ==================== Meshing ====================

MeshData mesh_shape(const std::vector<TopoDS_Solid>& solids, double linear, double angular, bool relative) {
    MeshData result;
    result.success = false;
    TopoDS_Compound shape = compound_of(solids);
    // Face ids are owner-relative, so compute them from each solid, not from the compound.
    NCollection_DataMap<TopoDS_Shape, uint64_t, TopTools_ShapeMapHasher> ids;
    for (const auto& s : solids) {
        for (TopExp_Explorer ex(s, TopAbs_FACE); ex.More(); ex.Next()) ids.Bind(ex.Current(), sub_id(ex.Current(), s.Location()));
    }

    // BRepMesh writes its triangulation into the faces it is given, so it runs on a
    // topology copy (geometry shared) and the caller's shapes are never written to.
    TopoDS_Shape meshed = BRepBuilderAPI_Copy(shape, false, false).Shape();
    BRepTools::Clean(meshed);
    // BRepMesh_IncrementalMesh(shape, linDeflection, isRelative, angDeflection, isInParallel)
    BRepMesh_IncrementalMesh mesher(meshed, linear, relative, angular, false);
    if (!mesher.IsDone()) {
        return result;
    }

    uint32_t global_vertex_offset = 0;

    // The copy keeps the traversal order, so its faces pair with the originals' ids.
    TopExp_Explorer original(shape, TopAbs_FACE);
    for (TopExp_Explorer explorer(meshed, TopAbs_FACE); explorer.More() && original.More(); explorer.Next(), original.Next()) {
        TopoDS_Face face = TopoDS::Face(explorer.Current());
        TopLoc_Location location;
        Handle(Poly_Triangulation) triangulation = BRep_Tool::Triangulation(face, location);

        // Nodal normals, taken from the underlying surface (GeomLib::NormEstim at
        // each UV node) rather than averaged from the triangles, so curved faces
        // carry their exact normal. OCCT falls back to averaging adjacent triangle
        // normals at singular nodes (cone apex, sphere pole) and on faces without
        // UV nodes. NOT Poly_Triangulation::ComputeNormals, which only averages
        // triangle normals and would throw the surface away.
        //
        // Safe on a null handle, and every other path allocates the array, so the
        // guard below rejects exactly the faces with nothing to emit: no
        // triangulation at all, or a triangulation with no nodes.
        BRepLib_ToolTriangulatedShape::ComputeNormals(face, triangulation);
        if (triangulation.IsNull() || !triangulation->HasNormals()) {
            continue;
        }

        int nb_nodes = triangulation->NbNodes();
        int nb_triangles = triangulation->NbTriangles();

        // Shared by the nodal normals and the index winding below.
        bool reversed = (face.Orientation() == TopAbs_REVERSED);

        // Position, surface parameters and normal of every node in one pass.
        const bool has_uv = triangulation->HasUVNodes();
        for (int i = 1; i <= nb_nodes; i++) {
            gp_Pnt p = triangulation->Node(i);
            p.Transform(location.Transformation());
            result.vertices.push_back(p.X());
            result.vertices.push_back(p.Y());
            result.vertices.push_back(p.Z());
            const gp_Pnt2d uv = has_uv ? triangulation->UVNode(i) : gp_Pnt2d(std::nan(""), std::nan(""));
            result.uvs.push_back(uv.X());
            result.uvs.push_back(uv.Y());

            // ComputeNormals ignores face orientation and works in the
            // triangulation's local frame, so apply the location and the REVERSED
            // flip here — the same rule the index winding below uses.
            gp_Dir n = triangulation->Normal(i);
            n.Transform(location.Transformation());
            if (reversed) n.Reverse();
            result.normals.push_back(n.X());
            result.normals.push_back(n.Y());
            result.normals.push_back(n.Z());
        }

        // Indices
        const uint64_t face_id = ids.Find(original.Current());
        for (int i = 1; i <= nb_triangles; i++) {
            const Poly_Triangle& tri = triangulation->Triangle(i);

            int n1, n2, n3;
            tri.Get(n1, n2, n3);

            // OCC indices are 1-based, convert to 0-based + global offset
            if (reversed) {
                result.indices.push_back(global_vertex_offset + n1 - 1);
                result.indices.push_back(global_vertex_offset + n3 - 1);
                result.indices.push_back(global_vertex_offset + n2 - 1);
            } else {
                result.indices.push_back(global_vertex_offset + n1 - 1);
                result.indices.push_back(global_vertex_offset + n2 - 1);
                result.indices.push_back(global_vertex_offset + n3 - 1);
            }
            result.face_tshape_ids.push_back(face_id);
        }

        global_vertex_offset += nb_nodes;
    }

    result.success = true;
    return result;
}

// ==================== Topology enumeration ====================

// Unique edges of `shape` (TopExp_Explorer visits a shared edge once per face) with
// ids relative to `owner`.
static std::unique_ptr<std::vector<TopoDS_Edge>> edges_of(const TopoDS_Shape& shape, const TopLoc_Location& owner, rust::Vec<uint64_t>& out_ids) {
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> edgeMap;
    TopExp::MapShapes(shape, TopAbs_EDGE, edgeMap);
    auto out = std::make_unique<std::vector<TopoDS_Edge>>();
    out->reserve(edgeMap.Extent());
    for (int i = 1; i <= edgeMap.Extent(); i++) {
        out->push_back(TopoDS::Edge(edgeMap(i)));
        out_ids.push_back(sub_id(edgeMap(i), owner));
    }
    return out;
}

std::unique_ptr<std::vector<TopoDS_Edge>> solid_edges(const TopoDS_Solid& shape, rust::Vec<uint64_t>& out_ids) {
    return edges_of(shape, shape.Location(), out_ids);
}

std::unique_ptr<std::vector<TopoDS_Face>> solid_faces(const TopoDS_Solid& shape, rust::Vec<uint64_t>& out_ids) {
    auto out = std::make_unique<std::vector<TopoDS_Face>>();
    for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next()) {
        out->push_back(TopoDS::Face(ex.Current()));
        out_ids.push_back(sub_id(ex.Current(), shape.Location()));
    }
    return out;
}

// `local` is `face` placed relative to its owner, so the owner sits at `face / local`.
std::unique_ptr<std::vector<TopoDS_Edge>> face_loops(const TopoDS_Face& face, const TopoDS_Face& local, rust::Vec<uint64_t>& out_ids, rust::Vec<uint32_t>& out_counts)
{
    const TopLoc_Location owner = face.Location().Divided(local.Location());
    auto out = std::make_unique<std::vector<TopoDS_Edge>>();
    // Loops run with the face on the left seen from the surface normal, whichever way
    // the face itself is oriented in its shell, as `make_face` reads them.
    const TopoDS_Face forward = TopoDS::Face(face.Oriented(TopAbs_FORWARD));
    const TopoDS_Wire outer = BRepTools::OuterWire(forward);
    std::vector<TopoDS_Wire> wires;
    if (!outer.IsNull()) wires.push_back(outer);
    for (TopExp_Explorer ex(forward, TopAbs_WIRE); ex.More(); ex.Next()) {
        if (outer.IsNull() || !ex.Current().IsSame(outer)) wires.push_back(TopoDS::Wire(ex.Current()));
    }
    for (const TopoDS_Wire& wire : wires) {
        std::vector<TopoDS_Edge> loop;
        for (BRepTools_WireExplorer wx(wire, forward); wx.More(); wx.Next()) loop.push_back(wx.Current());
        // The wire explorer stops early on a wire it cannot order; fall back to storage order.
        size_t total = 0;
        for (TopoDS_Iterator it(wire); it.More(); it.Next()) ++total;
        if (loop.size() != total) {
            loop.clear();
            for (TopoDS_Iterator it(wire); it.More(); it.Next()) loop.push_back(TopoDS::Edge(it.Value()));
        }
        for (const TopoDS_Edge& e : loop) {
            out->push_back(e);
            out_ids.push_back(sub_id(e, owner));
        }
        out_counts.push_back(static_cast<uint32_t>(loop.size()));
    }
    return out;
}

rust::Vec<double> solid_vertices(const TopoDS_Solid& shape) {
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> map;
    TopExp::MapShapes(shape, TopAbs_VERTEX, map);
    rust::Vec<double> out;
    for (int i = 1; i <= map.Extent(); i++) {
        const gp_Pnt p = BRep_Tool::Pnt(TopoDS::Vertex(map(i)));
        out.push_back(p.X());
        out.push_back(p.Y());
        out.push_back(p.Z());
    }
    return out;
}

std::unique_ptr<TopoDS_Face> local_face(const TopoDS_Face& face, const TopoDS_Solid& owner) {
    return std::make_unique<TopoDS_Face>(TopoDS::Face(face.Located(face.Location().Predivided(owner.Location()))));
}

bool shape_is_valid(const TopoDS_Solid& shape) {
    return BRepCheck_Analyzer(shape).IsValid();
}

std::unique_ptr<TopoDS_Solid> clone_solid_handle(const TopoDS_Solid& solid) {
    return std::make_unique<TopoDS_Solid>(solid);
}

std::unique_ptr<TopoDS_Edge> clone_edge_handle(const TopoDS_Edge& edge) {
    return std::make_unique<TopoDS_Edge>(edge);
}

std::unique_ptr<TopoDS_Face> clone_face_handle(const TopoDS_Face& face) {
    return std::make_unique<TopoDS_Face>(face);
}

// ==================== Face Methods ====================

uint64_t face_tshape_id(const TopoDS_Face& face) {
    return own_id(face);
}

uint64_t solid_tshape_id(const TopoDS_Solid& shape) {
    return own_id(shape);
}

uint64_t edge_tshape_id(const TopoDS_Edge& edge) {
    return own_id(edge);
}

double face_surface_area(const TopoDS_Face& face) {
    GProp_GProps props;
    BRepGProp::SurfaceProperties(face, props);
    return props.Mass();
}

uint32_t face_surface(const TopoDS_Face& face, rust::Vec<double>& out)
{
    uint32_t kind = 0;
    gp_Ax3 pos;
    double p1 = 0.0, p2 = 0.0;
    Handle(Geom_BSplineSurface) bspline;
    BRepAdaptor_Surface surf(face);
    switch (surf.GetType()) {
        case GeomAbs_Plane: {
            kind = 1; pos = surf.Plane().Position();
            break;
        }
        case GeomAbs_Cylinder: {
            const gp_Cylinder cyl = surf.Cylinder();
            kind = 2; pos = cyl.Position(); p1 = cyl.Radius();
            break;
        }
        case GeomAbs_Cone: {
            const gp_Cone cone = surf.Cone();
            kind = 3; pos = cone.Position();
            p1 = cone.RefRadius(); p2 = cone.SemiAngle();
            break;
        }
        case GeomAbs_Sphere: {
            const gp_Sphere sph = surf.Sphere();
            kind = 4; pos = sph.Position(); p1 = sph.Radius();
            break;
        }
        case GeomAbs_Torus: {
            const gp_Torus tor = surf.Torus();
            kind = 5; pos = tor.Position();
            p1 = tor.MajorRadius(); p2 = tor.MinorRadius();
            break;
        }
        default: {
            // BRep_Tool::Surface applies the face's location. Anything that is not a
            // B-spline already is converted over the face's own parameter range.
            Handle(Geom_Surface) s = BRep_Tool::Surface(face);
            bspline = Handle(Geom_BSplineSurface)::DownCast(s);
            if (bspline.IsNull()) {
                Handle(Geom_RectangularTrimmedSurface) trimmed = Handle(Geom_RectangularTrimmedSurface)::DownCast(s);
                if (!trimmed.IsNull()) bspline = Handle(Geom_BSplineSurface)::DownCast(trimmed->BasisSurface());
            }
            if (bspline.IsNull()) {
                double u1, u2, v1, v2;
                BRepTools::UVBounds(face, u1, u2, v1, v2);
                bspline = GeomConvert::SurfaceToBSplineSurface(new Geom_RectangularTrimmedSurface(s, u1, u2, v1, v2));
            }
            kind = 6;
            break;
        }
    }
    gp_Dir x = pos.XDirection();
    // Normalise a mirrored placement, so axis x X is always its Y direction.
    if (!pos.Direct()) x.Reverse();
    push_frame(pos.Location(), pos.Direction(), x, p1, p2, out);
    if (kind == 6) encode_bspline_surface(bspline, out);
    return kind;
}

void face_center_of_mass(const TopoDS_Face& face,
    double& x, double& y, double& z)
{
    GProp_GProps props;
    BRepGProp::SurfaceProperties(face, props);
    gp_Pnt center = props.CentreOfMass();
    x = center.X(); y = center.Y(); z = center.Z();
}

bool face_project_point(const TopoDS_Face& face,
    double px, double py, double pz,
    double& cpx, double& cpy, double& cpz,
    double& nx, double& ny, double& nz)
{
    // Default normal = zero. Returned when BRepLProp can't define a normal
    // at the closest hit (e.g. degenerate surface point or zero first
    // derivative). Caller can detect via `normal.length() == 0`.
    nx = 0.0; ny = 0.0; nz = 0.0;

    const gp_Pnt target(px, py, pz);
    bool has_uv = false;
    double best_d2 = Precision::Infinite(), bu = 0.0, bv = 0.0;
    gp_Pnt best_p;

    // Orthogonal projections onto the face interior. BRepExtrema_ExtPF
    // respects the trim: it keeps only extrema classified inside the face.
    TopoDS_Vertex vert = BRepBuilderAPI_MakeVertex(target);
    BRepExtrema_ExtPF ext(vert, face);
    if (ext.IsDone()) {
        for (int i = 1; i <= ext.NbExt(); ++i) {
            const double d2 = ext.SquareDistance(i);
            if (d2 >= best_d2) continue;
            best_d2 = d2;
            best_p = ext.Point(i);
            ext.Parameter(i, bu, bv);
            has_uv = true;
        }
    }

    // Nearest points on the boundary wires. An interior extremum can be
    // farther than the boundary (a 270° cylindrical face keeps only the far
    // wall), and outside the trim there is no interior extremum at all, so
    // both sets must be compared rather than one used as a fallback.
    for (TopExp_Explorer it(face, TopAbs_EDGE); it.More(); it.Next()) {
        const TopoDS_Edge& edge = TopoDS::Edge(it.Current());
        BRepAdaptor_Curve curve(edge);
        const double first = curve.FirstParameter();
        const double last = curve.LastParameter();
        Extrema_ExtPC extrema(target, curve, first, last);
        double u = first;
        if (extrema.IsDone() && extrema.NbExt() > 0) {
            int k = 1;
            for (int i = 2; i <= extrema.NbExt(); ++i) {
                if (extrema.SquareDistance(i) < extrema.SquareDistance(k)) k = i;
            }
            u = extrema.Point(k).Parameter();
        }
        // The endpoints are candidates too: an interior extremum of the
        // curve can be farther than either end.
        double pf = 0.0, pl = 0.0;
        Handle(Geom2d_Curve) pcurve = BRep_Tool::CurveOnSurface(edge, face, pf, pl);
        for (const double t : {u, first, last}) {
            const gp_Pnt p = curve.Value(t);
            const double d2 = target.SquareDistance(p);
            if (d2 >= best_d2) continue;
            best_d2 = d2;
            best_p = p;
            // The edge shares its parameter with its pcurve, so the same t
            // gives the (u, v) the normal needs.
            has_uv = !pcurve.IsNull();
            if (has_uv) {
                const gp_Pnt2d uv = pcurve->Value(t);
                bu = uv.X();
                bv = uv.Y();
            }
        }
    }

    if (Precision::IsInfinite(best_d2)) throw std::runtime_error("face has no interior extremum and no boundary");
    cpx = best_p.X();
    cpy = best_p.Y();
    cpz = best_p.Z();
    if (!has_uv) return true;  // cp valid, normal stays 0.

    BRepAdaptor_Surface surf(face);
    BRepLProp_SLProps props(surf, bu, bv, /*derivOrder=*/1, Precision::Confusion());
    if (!props.IsNormalDefined()) return true;  // cp valid, normal stays 0.

    gp_Dir n = props.Normal();
    // BRepLProp returns the surface-orientation normal; flip when the
    // face is REVERSED in its enclosing shell so the caller always
    // sees an outward-pointing direction.
    if (face.Orientation() == TopAbs_REVERSED) n.Reverse();
    nx = n.X();
    ny = n.Y();
    nz = n.Z();
    return true;
}

// ==================== Edge Methods ====================

rust::Vec<double> edge_approximation_segments(
    const TopoDS_Edge& edge, double linear, double angular, bool relative)
{
    rust::Vec<double> out;
    // Mirror mesh_shape's relative semantics: when relative, scale the chord
    // by the edge's bounding-box max dimension (OCCT BRepMesh convention).
    double eff_chord = linear;
    if (relative) {
        Bnd_Box box;
        BRepBndLib::Add(edge, box);
        if (!box.IsVoid()) {
            double xmin, ymin, zmin, xmax, ymax, zmax;
            box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
            eff_chord = linear * std::max(xmax - xmin, std::max(ymax - ymin, zmax - zmin));
        }
    }
    BRepAdaptor_Curve curve(edge);
    GCPnts_TangentialDeflection approx(curve, angular, eff_chord);

    const int nb_points = approx.NbPoints();
    const bool reversed = edge.Orientation() == TopAbs_REVERSED;
    for (int k = 1; k <= nb_points; k++) {
        gp_Pnt p = approx.Value(reversed ? nb_points + 1 - k : k);
        out.push_back(p.X());
        out.push_back(p.Y());
        out.push_back(p.Z());
    }
    return out;
}

std::unique_ptr<TopoDS_Edge> make_helix_edge(
    double ax, double ay, double az,
    double xrx, double xry, double xrz,
    double radius, double pitch, double height)
{
    if (radius < Precision::Confusion()) throw std::runtime_error("radius must be positive");
    if (pitch < Precision::Confusion()) throw std::runtime_error("pitch must be positive");
    if (height < Precision::Confusion()) throw std::runtime_error("height must be positive");

    // Build a deterministic local frame: the cylinder's local +X is the
    // user-supplied x_ref (orthogonalized against axis by gp_Ax2). The
    // helix then starts at (radius, 0, 0) in this frame, which is
    // origin + radius * normalize(x_ref ⊥ axis) in world coordinates.
    gp_Dir axis_dir(ax, ay, az);
    gp_Dir x_ref(xrx, xry, xrz);
    if (axis_dir.IsParallel(x_ref, Precision::Angular())) throw std::runtime_error("x_ref is parallel to the axis");
    gp_Ax2 ax2(gp_Pnt(0.0, 0.0, 0.0), axis_dir, x_ref);
    Handle(Geom_CylindricalSurface) cylinder =
        new Geom_CylindricalSurface(ax2, radius);

    double turns = height / pitch;
    double total_angle = turns * 2.0 * M_PI;
    gp_Pnt2d line_origin(0.0, 0.0);
    gp_Dir2d line_dir(total_angle, height);
    Handle(Geom2d_Line) line2d = new Geom2d_Line(line_origin, line_dir);

    double param_end = std::sqrt(total_angle * total_angle + height * height);

    BRepBuilderAPI_MakeEdge edgeMaker(line2d, cylinder, 0.0, param_end);
    if (!edgeMaker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeEdge did not complete");
    TopoDS_Edge edge = edgeMaker.Edge();
    BRepLib::BuildCurve3d(edge);
    return std::make_unique<TopoDS_Edge>(edge);
}

std::unique_ptr<std::vector<TopoDS_Edge>> make_polygon_edges(rust::Slice<const double> coords) {
    if (coords.size() < 9 || coords.size() % 3 != 0) throw std::runtime_error("need at least 3 points");
    BRepBuilderAPI_MakePolygon poly;
    for (size_t i = 0; i + 2 < coords.size(); i += 3) {
        poly.Add(gp_Pnt(coords[i], coords[i + 1], coords[i + 2]));
    }
    poly.Close();
    if (!poly.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakePolygon did not complete");
    auto out = std::make_unique<std::vector<TopoDS_Edge>>();
    for (TopExp_Explorer ex(poly.Wire(), TopAbs_EDGE); ex.More(); ex.Next()) {
        out->push_back(TopoDS::Edge(ex.Current()));
    }
    return out;
}

std::unique_ptr<TopoDS_Edge> make_circle_edge(
    double ax, double ay, double az, double radius)
{
    if (radius < Precision::Confusion()) throw std::runtime_error("radius must be positive");
    gp_Dir axis_dir(ax, ay, az);
    // Single-arg gp_Ax2(origin, N): OCCT picks an arbitrary X direction
    // orthogonal to the normal. The circle's parametric start is then at
    // (radius, 0, 0) in that implicit local frame. Callers that need a
    // specific start direction should rotate the result into place.
    gp_Ax2 ax2(gp_Pnt(0.0, 0.0, 0.0), axis_dir);
    gp_Circ circ(ax2, radius);
    BRepBuilderAPI_MakeEdge edgeMaker(circ);
    if (!edgeMaker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeEdge did not complete");
    return std::make_unique<TopoDS_Edge>(edgeMaker.Edge());
}

std::unique_ptr<TopoDS_Edge> make_line_edge(
    double ax, double ay, double az,
    double bx, double by, double bz)
{
    gp_Pnt a(ax, ay, az);
    gp_Pnt b(bx, by, bz);
    if (a.Distance(b) < Precision::Confusion()) throw std::runtime_error("zero-length segment");
    BRepBuilderAPI_MakeEdge edgeMaker(a, b);
    if (!edgeMaker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeEdge did not complete");
    return std::make_unique<TopoDS_Edge>(edgeMaker.Edge());
}

std::unique_ptr<TopoDS_Edge> make_arc_edge(
    double sx, double sy, double sz,
    double mx, double my, double mz,
    double ex, double ey, double ez)
{
    gp_Pnt p_start(sx, sy, sz);
    gp_Pnt p_mid(mx, my, mz);
    gp_Pnt p_end(ex, ey, ez);
    // false: do not wrap around; the arc goes from start through
    // mid to end on the unique circle defined by those three points.
    GC_MakeArcOfCircle maker(p_start, p_mid, p_end);
    if (!maker.IsDone()) throw std::runtime_error("GC_MakeArcOfCircle did not complete (collinear or coincident points)");
    BRepBuilderAPI_MakeEdge edgeMaker(maker.Value());
    if (!edgeMaker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeEdge did not complete");
    return std::make_unique<TopoDS_Edge>(edgeMaker.Edge());
}

// Cubic B-spline edge interpolating the given data points.
//
// `coords` is a flat array of xyz triples (length must be a multiple of 3
// and ≥ 6). Each (x, y, z) triple is one interpolation target — the
// resulting curve passes through every input point.
//
// `end_kind` selects the end-condition variant of `BSplineEnd`:
//   0 = Periodic — wraps around with C² continuity. Periodic is encoded
//       in the basis; the caller must NOT duplicate the first point at
//       the end. Needs ≥ 3 points (Rust side validates).
//   1 = NotAKnot — open curve, OCCT default end condition (the boundary
//       cubic is fit to 3 data points instead of being constrained by
//       an artificial derivative). Needs ≥ 2 points.
//   2 = Clamped — open curve with explicit start/end tangent vectors
//       passed in (sx, sy, sz) and (ex, ey, ez). Needs ≥ 2 points.
//
// For end_kind 0 and 1, the tangent arguments are ignored.
//
// Returns null on any failure (out-of-range end_kind, OCCT internal
// failure, degenerate point distribution).
std::unique_ptr<TopoDS_Edge> make_bspline_edge(
    rust::Slice<const double> coords,
    uint32_t end_kind,
    double sx, double sy, double sz,
    double ex, double ey, double ez)
{
    if (coords.size() < 6 || coords.size() % 3 != 0) throw std::runtime_error("need at least 2 points");
    // Local alias: `Handle(NCollection_HArray1<gp_Pnt>)` は Handle マクロが
    // template 内のカンマで引数を分割してしまうので、using alias を噛ませて
    // 単一トークン化する(コミット a72e330 で deprecated 型に戻した時の回避策)。
    using HPntArray = NCollection_HArray1<gp_Pnt>;
    const int n = static_cast<int>(coords.size() / 3);
    Handle(HPntArray) pts = new HPntArray(1, n);
    for (int i = 0; i < n; ++i) {
        pts->SetValue(i + 1, gp_Pnt(coords[i * 3], coords[i * 3 + 1], coords[i * 3 + 2]));
    }

    const bool periodic = (end_kind == 0) ? true : false;
    GeomAPI_Interpolate interp(pts, periodic, Precision::Confusion());

    if (end_kind == 2) {
        // Clamped: load explicit start and end tangent vectors.
        // Scale = true lets OCCT scale the tangent magnitude
        // by the chord length, which usually gives more intuitive
        // pull strength than the raw vector magnitude.
        gp_Vec start_tan(sx, sy, sz);
        gp_Vec end_tan(ex, ey, ez);
        interp.Load(start_tan, end_tan, true);
    } else if (end_kind != 0 && end_kind != 1) {
        // Unknown end_kind; fail rather than silently picking a default.
        throw std::runtime_error("unknown end kind");
    }

    interp.Perform();
    if (!interp.IsDone()) throw std::runtime_error("GeomAPI_Interpolate did not complete");

    Handle(Geom_BSplineCurve) curve = interp.Curve();
    if (curve.IsNull()) throw std::runtime_error("GeomAPI_Interpolate produced no curve");

    BRepBuilderAPI_MakeEdge edgeMaker(curve);
    if (!edgeMaker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeEdge did not complete");
    return std::make_unique<TopoDS_Edge>(edgeMaker.Edge());
}

uint32_t edge_curve(const TopoDS_Edge& edge, rust::Vec<double>& out)
{
    double first, last;
    // The 3-argument overload applies the edge's location; null on a degenerate edge.
    Handle(Geom_Curve) curve = BRep_Tool::Curve(edge, first, last);
    if (curve.IsNull()) return 0;
    GeomAdaptor_Curve adaptor(curve, first, last);
    uint32_t kind = 0;
    gp_Ax2 pos;
    double p1 = 0.0, p2 = 0.0;
    Handle(Geom_BSplineCurve) bspline;
    switch (adaptor.GetType()) {
        case GeomAbs_Line: {
            const gp_Lin line = adaptor.Line();
            kind = 1; pos = gp_Ax2(line.Location(), line.Direction());
            break;
        }
        case GeomAbs_Circle: {
            const gp_Circ circle = adaptor.Circle();
            kind = 2; pos = circle.Position(); p1 = circle.Radius();
            break;
        }
        case GeomAbs_Ellipse: {
            const gp_Elips ellipse = adaptor.Ellipse();
            kind = 3; pos = ellipse.Position(); p1 = ellipse.MajorRadius(); p2 = ellipse.MinorRadius();
            break;
        }
        case GeomAbs_BSplineCurve: {
            kind = 4; bspline = adaptor.BSpline();
            break;
        }
        default: {
            // Bezier, parabola, hyperbola, offset: converted over the edge's own range.
            kind = 4; bspline = GeomConvert::CurveToBSplineCurve(new Geom_TrimmedCurve(curve, first, last));
            first = bspline->FirstParameter(); last = bspline->LastParameter();
            break;
        }
    }
    push_frame(pos.Location(), pos.Direction(), pos.XDirection(), p1, p2, out);
    out.push_back(first);
    out.push_back(last);
    if (kind == 4) encode_bspline_curve(bspline, out);
    return kind;
}

std::unique_ptr<TopoDS_Edge> make_curve_edge(
    uint32_t kind,
    double ox, double oy, double oz,
    double zx, double zy, double zz,
    double xx, double xy, double xz,
    double p1, double p2, double t0, double t1,
    rust::Slice<const double> data)
{
    if (!(t1 > t0)) throw std::runtime_error("parameter range is empty");
    const gp_Pnt origin(ox, oy, oz);
    const gp_Dir z(zx, zy, zz);
    BRepBuilderAPI_MakeEdge maker;
    switch (kind) {
        case 1: maker.Init(new Geom_Line(origin, z), t0, t1); break;
        case 2: maker.Init(new Geom_Circle(gp_Ax2(origin, z, gp_Dir(xx, xy, xz)), p1), t0, t1); break;
        case 3: maker.Init(new Geom_Ellipse(gp_Ax2(origin, z, gp_Dir(xx, xy, xz)), p1, p2), t0, t1); break;
        case 4: maker.Init(decode_bspline_curve(data), t0, t1); break;
        default: throw std::runtime_error("unknown curve kind");
    }
    if (!maker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeEdge did not complete");
    return std::make_unique<TopoDS_Edge>(maker.Edge());
}

void edge_endpoints(const TopoDS_Edge& edge,
    double& sx, double& sy, double& sz,
    double& ex, double& ey, double& ez)
{
    BRepAdaptor_Curve curve(edge);
    gp_Pnt start = curve.Value(curve.FirstParameter());
    gp_Pnt end = curve.Value(curve.LastParameter());
    // A reversed edge runs against its curve.
    if (edge.Orientation() == TopAbs_REVERSED) std::swap(start, end);
    sx = start.X(); sy = start.Y(); sz = start.Z();
    ex = end.X();   ey = end.Y();   ez = end.Z();
}

void edge_tangents(const TopoDS_Edge& edge,
    double& sx, double& sy, double& sz,
    double& ex, double& ey, double& ez)
{
    sx = 0.0; sy = 0.0; sz = 0.0;
    ex = 0.0; ey = 0.0; ez = 0.0;
    BRepAdaptor_Curve curve(edge);
    gp_Pnt p;
    gp_Vec vs, ve;
    curve.D1(curve.FirstParameter(), p, vs);
    curve.D1(curve.LastParameter(), p, ve);
    // A reversed edge runs against its curve: the ends swap and the tangents turn round.
    if (edge.Orientation() == TopAbs_REVERSED) {
        std::swap(vs, ve);
        vs.Reverse();
        ve.Reverse();
    }
    if (vs.Magnitude() > Precision::Confusion()) {
        vs.Normalize();
        sx = vs.X(); sy = vs.Y(); sz = vs.Z();
    }
    if (ve.Magnitude() > Precision::Confusion()) {
        ve.Normalize();
        ex = ve.X(); ey = ve.Y(); ez = ve.Z();
    }
}

double precision_confusion() {
    return Precision::Confusion();
}

bool edge_project_point(const TopoDS_Edge& edge,
    double px, double py, double pz,
    double& cpx, double& cpy, double& cpz,
    double& tx, double& ty, double& tz)
{
    cpx = 0.0; cpy = 0.0; cpz = 0.0;
    tx = 0.0; ty = 0.0; tz = 0.0;
    // BRepAdaptor_Curve は 3D 曲線を持たない縮退エッジ (球の極・円錐の頂点) でも
    // pcurve + サーフェス経由で評価できる。BRep_Tool::Curve を直接引くと null で
    // 落ちるので使わない。ファイル内の他の edge_* 関数もこのアダプタ経由。
    BRepAdaptor_Curve curve(edge);
    const double first = curve.FirstParameter();
    const double last = curve.LastParameter();
    gp_Pnt target(px, py, pz);
    Extrema_ExtPC extrema(target, curve, first, last);
    double u;
    if (extrema.IsDone() && extrema.NbExt() > 0) {
        int best = 1;
        for (int i = 2; i <= extrema.NbExt(); i++) {
            if (extrema.SquareDistance(i) < extrema.SquareDistance(best)) best = i;
        }
        u = extrema.Point(best).Parameter();
    } else {
        // 区間内に極値なし — 距離が単調 (端点の外側にある線分など)。近い端点にクランプ。
        double d_first = target.Distance(curve.Value(first));
        double d_last  = target.Distance(curve.Value(last));
        u = (d_first <= d_last) ? first : last;
    }
    gp_Pnt p;
    gp_Vec v;
    curve.D1(u, p, v);
    if (edge.Orientation() == TopAbs_REVERSED) v.Reverse();
    cpx = p.X(); cpy = p.Y(); cpz = p.Z();
    if (v.Magnitude() > Precision::Confusion()) {
        v.Normalize();
        tx = v.X(); ty = v.Y(); tz = v.Z();
    }
    return true;
}

// Scale and mirror rebuild the edge via BRepBuilderAPI_Transform; translate and
// rotate only move it, keeping its TShape and so its id.
static std::unique_ptr<TopoDS_Edge> transform_edge_impl(
    const TopoDS_Edge& edge, const gp_Trsf& trsf)
{
    BRepBuilderAPI_Transform transform(edge, trsf, true);
    return std::make_unique<TopoDS_Edge>(TopoDS::Edge(transform.Shape()));
}

std::unique_ptr<TopoDS_Edge> translate_edge(
    const TopoDS_Edge& edge, double tx, double ty, double tz)
{
    gp_Trsf trsf;
    trsf.SetTranslation(gp_Vec(tx, ty, tz));
    return std::make_unique<TopoDS_Edge>(TopoDS::Edge(edge.Moved(TopLoc_Location(trsf))));
}

std::unique_ptr<TopoDS_Edge> rotate_edge(
    const TopoDS_Edge& edge,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle)
{
    gp_Trsf trsf;
    trsf.SetRotation(gp_Ax1(gp_Pnt(ox, oy, oz), gp_Dir(dx, dy, dz)), angle);
    return std::make_unique<TopoDS_Edge>(TopoDS::Edge(edge.Moved(TopLoc_Location(trsf))));
}

std::unique_ptr<TopoDS_Edge> scale_edge(
    const TopoDS_Edge& edge,
    double cx, double cy, double cz,
    double factor)
{
    if (std::abs(factor) < Precision::Confusion()) throw std::runtime_error("the scale factor is zero");
    gp_Trsf trsf;
    trsf.SetScale(gp_Pnt(cx, cy, cz), factor);
    return transform_edge_impl(edge, trsf);
}

std::unique_ptr<TopoDS_Edge> mirror_edge(
    const TopoDS_Edge& edge,
    double ox, double oy, double oz,
    double nx, double ny, double nz)
{
    gp_Trsf trsf;
    trsf.SetMirror(gp_Ax2(gp_Pnt(ox, oy, oz), gp_Dir(nx, ny, nz)));
    return transform_edge_impl(edge, trsf);
}

std::unique_ptr<std::vector<TopoDS_Edge>> edge_vec_new() {
    return std::make_unique<std::vector<TopoDS_Edge>>();
}


void edge_vec_push(std::vector<TopoDS_Edge>& v, const TopoDS_Edge& e) {
    v.push_back(e);
}

std::unique_ptr<std::vector<TopoDS_Face>> face_vec_new() {
    return std::make_unique<std::vector<TopoDS_Face>>();
}

void face_vec_push(std::vector<TopoDS_Face>& v, const TopoDS_Face& f) {
    v.push_back(f);
}

std::unique_ptr<std::vector<TopoDS_Solid>> shape_vec_new() {
    return std::make_unique<std::vector<TopoDS_Solid>>();
}

void shape_vec_push(std::vector<TopoDS_Solid>& v, const TopoDS_Solid& s) {
    v.push_back(s);
}

std::unique_ptr<TopoDS_Solid> builder_thick_solid(
    const TopoDS_Solid& solid,
    const std::vector<TopoDS_Face>& open_faces,
    double thickness,
    rust::Vec<uint64_t>& out_history)
{
    // Empty open_faces: MakeThickSolidByJoin degenerates to a plain offset
    // shape (no cavity) because it needs at least one removed face to
    // build the first wall W1. Instead, build the solid explicitly as
    // outer_shell + reversed inner_shell so the result is a sealed solid
    // with an internal void (OCCT permits multi-shell solids).
    if (open_faces.empty()) {
        BRepOffsetAPI_MakeOffsetShape offset;
        offset.PerformByJoin(
            solid, thickness,
            /*tolerance=*/ 1.0e-6,
            /*mode=*/ BRepOffset_Skin,
            /*intersection=*/ false,
            /*selfInter=*/ false,
            /*join=*/ GeomAbs_Arc);
        if (!offset.IsDone()) throw std::runtime_error("BRepOffsetAPI_MakeOffsetShape did not complete");
        TopoDS_Shape offset_shape = offset.Shape();

        // A negative thickness shrinks inward, so the original shell stays outer.
        TopoDS_Shell original_shell = TopoDS::Shell(TopoDS_Iterator(solid).Value());
        TopoDS_Shell offset_shell = TopoDS::Shell(TopoDS_Iterator(*to_solid(offset_shape)).Value());
        TopoDS_Shell outer = thickness < 0.0 ? original_shell : offset_shell;
        TopoDS_Shell inner = thickness < 0.0 ? offset_shell : original_shell;

        BRepBuilderAPI_MakeSolid solid_maker(outer);
        solid_maker.Add(TopoDS::Shell(inner.Reversed()));
        if (!solid_maker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeSolid did not complete");
        auto out = std::make_unique<TopoDS_Solid>(solid_maker.Solid());
        // The original faces are kept; the offset walls are Generated from them.
        relay_history(offset, src_of(solid), index_result(*out), out_history);
        return out;
    }

    NCollection_List<TopoDS_Shape> faces_to_remove;
    for (const auto& f : open_faces) faces_to_remove.Append(f);

    BRepOffsetAPI_MakeThickSolid builder;
    builder.MakeThickSolidByJoin(
        solid, faces_to_remove, thickness,
        /*tolerance=*/ 1.0e-6,
        /*mode=*/ BRepOffset_Skin,
        /*intersection=*/ false,
        /*selfInter=*/ false,
        /*join=*/ GeomAbs_Arc);
    builder.Build();
    if (!builder.IsDone()) throw std::runtime_error("BRepOffsetAPI_MakeThickSolid did not complete");
    auto out = to_solid(builder.Shape());
    relay_history(builder, src_of(solid), index_result(*out), out_history);
    return out;
}

std::unique_ptr<TopoDS_Solid> builder_fillet(
    const TopoDS_Solid& solid,
    const std::vector<TopoDS_Edge>& edges,
    double radius,
    rust::Vec<uint64_t>& out_history)
{
    if (edges.empty()) {
        NoAlgo none;
        relay_history(none, src_of(solid), index_result(solid), out_history);
        return std::make_unique<TopoDS_Solid>(solid);
    }
    BRepFilletAPI_MakeFillet mk(solid);
    for (const TopoDS_Edge& e : edges) {
        if (e.IsNull()) continue;
        mk.Add(radius, e);
    }
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("BRepFilletAPI_MakeFillet did not complete");
    TopoDS_Shape result = mk.Shape();
    if (result.IsNull()) throw std::runtime_error("BRepFilletAPI_MakeFillet produced no shape");
    // MakeFillet can wrap the solid in a compound even if it contains only one solid.
    auto out = to_solid(result);
    relay_history(mk, src_of(solid), index_result(*out), out_history);
    return out;
}

std::unique_ptr<TopoDS_Solid> builder_chamfer(
    const TopoDS_Solid& solid,
    const std::vector<TopoDS_Edge>& edges,
    double distance,
    rust::Vec<uint64_t>& out_history)
{
    if (edges.empty()) {
        NoAlgo none;
        relay_history(none, src_of(solid), index_result(solid), out_history);
        return std::make_unique<TopoDS_Solid>(solid);
    }
    BRepFilletAPI_MakeChamfer mk(solid);
    for (const TopoDS_Edge& e : edges) {
        if (e.IsNull()) continue;
        mk.Add(distance, e);
    }
    mk.Build();
    if (!mk.IsDone()) throw std::runtime_error("BRepFilletAPI_MakeChamfer did not complete");
    TopoDS_Shape result = mk.Shape();
    if (result.IsNull()) throw std::runtime_error("BRepFilletAPI_MakeChamfer produced no shape");
    // Like MakeFillet, MakeChamfer may wrap the result in a compound even if it contains only one solid.
    auto out = to_solid(result);
    relay_history(mk, src_of(solid), index_result(*out), out_history);
    return out;
}


// Build the profile face shared by make_extrude and make_revolve from closed
// loops; the first is the outer boundary.
static TopoDS_Face face_from_loops(const std::vector<TopoDS_Wire>& wires)
{
    if (wires.empty()) throw std::runtime_error("empty profile");
    if (!wires.empty() && !BRep_Tool::IsClosed(wires.back())) throw std::runtime_error("trailing edges do not close a loop");

    auto face_area = [](const TopoDS_Face& f) {
        GProp_GProps props;
        BRepGProp::SurfaceProperties(f, props);
        return props.Mass();
    };
    BRepBuilderAPI_MakeFace outer_only(wires.front());
    if (!outer_only.IsDone()) throw std::runtime_error("outer loop is not planar");
    const double outer_area = face_area(outer_only.Face());

    BRepBuilderAPI_MakeFace face_maker(wires.front());
    if (!face_maker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeFace did not complete");
    // A hole must wind against the outer loop or its area adds instead of
    // subtracting, and MakeFace::Add takes the wire as given. Rather than demand a
    // winding from the caller, add whichever orientation removes material.
    for (size_t i = 1; i < wires.size(); ++i) {
        BRepBuilderAPI_MakeFace trial(wires.front());
        trial.Add(wires[i]);
        if (!trial.IsDone()) throw std::runtime_error("hole loop is not planar");
        face_maker.Add(face_area(trial.Face()) < outer_area ? wires[i] : TopoDS::Wire(wires[i].Reversed()));
    }
    if (!face_maker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeFace did not complete");
    TopoDS_Face face = face_maker.Face();

    if (wires.size() > 1) {
        // Nothing above checks that a hole lies inside the outer loop; ShapeFix_Face
        // returns a Shell rather than a Face when the loops bound disjoint areas.
        ShapeFix_Face fixer(face);
        fixer.FixAddNaturalBoundMode() = 0;
        fixer.Perform();
        TopoDS_Shape fixed = fixer.Result();
        if (fixed.IsNull() || fixed.ShapeType() != TopAbs_FACE) throw std::runtime_error("a hole lies outside the outer loop");
        face = TopoDS::Face(fixed);
    }
    return face;
}

std::unique_ptr<TopoDS_Solid> make_extrude(
    const std::vector<TopoDS_Edge>& edges,
    double dx, double dy, double dz,
    rust::Vec<uint64_t>& out_history)
{
    // MakePrism accepts a zero vector and returns a degenerate solid.
    const gp_Vec dir(dx, dy, dz);
    if (dir.Magnitude() < Precision::Confusion()) throw std::runtime_error("direction is zero-length");
    Chained profile = chain(edges, Precision::Confusion());
    TopoDS_Face face = face_from_loops(profile.wires);
    BRepPrimAPI_MakePrism prism(face, dir);
    prism.Build();
    if (!prism.IsDone()) throw std::runtime_error("BRepPrimAPI_MakePrism did not complete");
    auto out = to_solid(prism.Shape());
    SweepEnds<BRepPrimAPI_MakePrism> ends{prism, {}};
    relay_history(ends, profile.src, index_result(*out), out_history);
    return out;
}

std::unique_ptr<TopoDS_Solid> make_revolve(
    const std::vector<TopoDS_Edge>& edges,
    double ox, double oy, double oz,
    double dx, double dy, double dz,
    double angle,
    rust::Vec<uint64_t>& out_history)
{
    const gp_Vec axis_vec(dx, dy, dz);
    if (axis_vec.Magnitude() < Precision::Confusion()) throw std::runtime_error("axis is zero-length");
    // MakeRevol takes an angle in [0, 2pi]; a negative angle turns the other way.
    if (std::abs(angle) < Precision::Angular() || std::abs(angle) > 2.0 * M_PI + Precision::Angular()) throw std::runtime_error("angle must be within (0, 2pi]");
    Chained profile = chain(edges, Precision::Confusion());
    TopoDS_Face face = face_from_loops(profile.wires);
    gp_Ax1 axis(gp_Pnt(ox, oy, oz), gp_Dir(axis_vec));
    if (angle < 0.0) axis.Reverse();
    BRepPrimAPI_MakeRevol revol(face, axis, std::abs(angle));
    revol.Build();
    if (!revol.IsDone()) throw std::runtime_error("BRepPrimAPI_MakeRevol did not complete");
    auto out = to_solid(revol.Shape());
    SweepEnds<BRepPrimAPI_MakeRevol> ends{revol, {}};
    relay_history(ends, profile.src, index_result(*out), out_history);
    return out;
}

// Unified MakePipeShell wrapper.  Handles both single-profile sweep and
// multi-profile morphing sweep: each wire of `profile` is a section.
// `spine_wires` and, when orient == 3 (Auxiliary), `aux_spine` hold one wire.
std::unique_ptr<TopoDS_Solid> make_pipe_shell(
    const std::vector<TopoDS_Edge>& profile_edges,
    const std::vector<TopoDS_Edge>& spine_edges,
    uint32_t orient,
    double ux, double uy, double uz,
    const std::vector<TopoDS_Edge>& aux_edges,
    rust::Vec<uint64_t>& out_history)
{
    Chained profiled = chain(profile_edges, Precision::Confusion());
    const std::vector<TopoDS_Wire>& profile = profiled.wires;
    const std::vector<TopoDS_Wire> spine_wires = chain(spine_edges, Precision::Confusion()).wires;
    const std::vector<TopoDS_Wire> aux_spine = chain(aux_edges, Precision::Confusion()).wires;
    if (profile.empty()) throw std::runtime_error("empty profile");
    if (spine_wires.size() != 1) throw std::runtime_error("spine edges do not connect into one wire");
    const TopoDS_Wire& spine = spine_wires.front();

    BRepOffsetAPI_MakePipeShell shell(spine);

    // Configure trihedron law.
    switch (orient) {
        case 0: {
            // Fixed: lock the trihedron to the spine-start frame.
            BRepAdaptor_CompCurve curve(spine);
            gp_Pnt start_pnt;
            gp_Vec start_tan;
            curve.D1(curve.FirstParameter(), start_pnt, start_tan);
            if (start_tan.Magnitude() < Precision::Confusion()) throw std::runtime_error("spine has no tangent at its start");
            gp_Dir tdir(start_tan);
            gp_Dir xref = (std::abs(tdir.X()) < 0.9) ? gp_Dir(1, 0, 0) : gp_Dir(0, 1, 0);
            gp_Ax2 fixed_ax2(start_pnt, tdir, xref);
            shell.SetMode(fixed_ax2);
            break;
        }
        case 1: {
            // Torsion: raw Frenet.
            shell.SetMode(true);
            break;
        }
        case 2: {
            // Up(v): fix the binormal direction.
            gp_Vec up_vec(ux, uy, uz);
            if (up_vec.Magnitude() < Precision::Confusion()) throw std::runtime_error("up vector is zero-length");
            shell.SetMode(gp_Dir(up_vec));
            break;
        }
        case 3: {
            // Auxiliary: build aux spine wire and use it for twist control.
            if (aux_spine.size() != 1) throw std::runtime_error("auxiliary spine edges do not connect into one wire");
            shell.SetMode(aux_spine.front(), true);
            break;
        }
        default: {
            shell.SetMode(true);
            break;
        }
    }

    for (const TopoDS_Wire& w : profile) shell.Add(w, false, false);

    shell.Build();
    if (!shell.IsDone()) throw std::runtime_error("BRepOffsetAPI_MakePipeShell did not complete");
    if (!shell.MakeSolid()) throw std::runtime_error("profile is not closed, so the sweep is a shell rather than a solid");
    auto out = to_solid(shell.Shape());
    relay_history(shell, profiled.src, index_result(*out), out_history);
    return out;
}

// Loft (skin) a solid through a sequence of cross-section wires.
//
// `wires` are the sections; ≥2 required. Built via
// BRepOffsetAPI_ThruSections with isSolid=true (cap open ends with planar
// faces). `ruled=false` gives B-spline / C² smoothed interpolation through
// all sections; `ruled=true` gives per-panel ruled (straight-line) surfaces
// between adjacent sections. Both pass through every section wire exactly.
std::unique_ptr<TopoDS_Solid> make_loft(
    const std::vector<TopoDS_Edge>& edges,
    uint32_t sections,
    bool ruled,
    rust::Vec<uint64_t>& out_history)
{
    BRepOffsetAPI_ThruSections loft(
        /*isSolid=*/true,
        /*isRuled=*/ruled,
        Precision::Confusion());

    if (sections < 2) throw std::runtime_error("need at least 2 sections");
    // Each section closes on itself, so the flattened edges chain back into one wire per section.
    Chained chained = chain(edges, Precision::Confusion());
    if (chained.wires.size() != sections) throw std::runtime_error(std::to_string(sections) + " sections chain into " + std::to_string(chained.wires.size()) + " wires; each section must close a loop");
    for (const TopoDS_Wire& w : chained.wires) loft.AddWire(w);

    loft.Build();
    if (!loft.IsDone()) throw std::runtime_error("BRepOffsetAPI_ThruSections did not complete");
    auto out = to_solid(loft.Shape());
    relay_history(loft, chained.src, index_result(*out), out_history);
    return out;
}

// Sew (stitch) free faces into a single closed shell and upgrade it to a
// solid. BRepBuilderAPI_Sewing merges boundary edges that coincide within
// `tolerance`; the sewn result must contain exactly one closed shell —
// gaps (open shell), leftover free faces, or multiple disconnected shells
// all return nullptr. The solid is oriented with BRepLib::OrientClosedSolid
// so the enclosed volume is positive regardless of input face orientation.
std::unique_ptr<TopoDS_Solid> make_sewn_solid(
    const std::vector<TopoDS_Face>& faces,
    rust::Slice<const uint64_t> ids,
    double tolerance,
    rust::Vec<uint64_t>& out_history)
{
    if (faces.empty()) throw std::runtime_error("no faces");
    BRepBuilderAPI_Sewing sewing(tolerance);
    for (const auto& f : faces) sewing.Add(f);
    sewing.Perform();
    const TopoDS_Shape& sewn = sewing.SewedShape();
    if (sewn.IsNull()) throw std::runtime_error("BRepBuilderAPI_Sewing produced no shape");
    // Partial sewing leaves stray faces beside the shell; requiring one closed shell rejects them.
    auto out = solid_from_shell(sewn);
    // The caller knows its faces by the ids it passed, whatever solid they came from.
    std::vector<SrcEntry> src;
    for (size_t i = 0; i < faces.size(); ++i) src.push_back({ids[i], faces[i]});
    SewingHistory history{sewing, {}};
    relay_history(history, src, index_result(*out), out_history);
    return out;
}

// Offset the given faces of `shape` by signed `offset` along their normals; a
// SHELL/compound result is upgraded so Rust always receives a TopAbs_SOLID.
std::unique_ptr<TopoDS_Solid> make_offset(
    const TopoDS_Solid& shape,
    const std::vector<TopoDS_Face>& faces,
    double offset,
    double tolerance,
    rust::Vec<uint64_t>& out_history)
{
    // Per-face offsets need the Intersection join; Arc rejects them.
    BRepOffset_MakeOffset offsetter;
    offsetter.Initialize(
        shape, /*offset=*/ 0.0, tolerance,
        /*mode=*/ BRepOffset_Skin,
        /*intersection=*/ true,
        /*selfInter=*/ false,
        /*join=*/ GeomAbs_Intersection);
    for (const TopoDS_Face& f : faces) offsetter.SetOffsetOnFace(f, offset);
    offsetter.MakeOffsetShape();
    if (!offsetter.IsDone()) throw std::runtime_error("BRepOffset_MakeOffset did not complete");
    TopoDS_Shape result = offsetter.Shape();
    if (result.IsNull()) throw std::runtime_error("BRepOffset_MakeOffset produced no shape");
    // solid_from_shell keeps the face handles, so the result can be indexed for the history.
    auto out = solid_from_shell(result);
    relay_history(offsetter, src_of(shape), index_result(*out), out_history);
    return out;
}

// Interpolate each column of `points` along its first index at uniform
// parameters and return the poles transposed, so applying this twice yields the
// tensor-product pole grid of a surface interpolating the grid in both
// directions. GeomAPI_Interpolate's periodic solve gives C^(degree-1) seams;
// the non-periodic interpolator picks independent end derivatives, so wrapping
// an open interpolation with SetUPeriodic would leave a C^0 seam.
static NCollection_Array2<gp_Pnt> interpolate_columns(
    const NCollection_Array2<gp_Pnt>& points, bool periodic, Handle(Geom_BSplineCurve)& sample)
{
    const int n = points.NbRows(), m = points.NbColumns();
    Handle(NCollection_HArray1<double>) params = new NCollection_HArray1<double>(1, n + periodic);
    for (int k = 1; k <= params->Length(); ++k) params->SetValue(k, double(k - 1) / n);
    NCollection_Array2<gp_Pnt> poles;
    for (int j = 1; j <= m; ++j) {
        Handle(NCollection_HArray1<gp_Pnt>) column = new NCollection_HArray1<gp_Pnt>(1, n);
        for (int i = 1; i <= n; ++i) column->SetValue(i, points(i, j));
        GeomAPI_Interpolate interp(column, params, periodic, Precision::Confusion());
        interp.Perform();
        if (!interp.IsDone()) throw std::runtime_error("GeomAPI_Interpolate did not complete");
        sample = interp.Curve();
        if (j == 1) poles.Resize(1, m, 1, sample->NbPoles(), false);
        for (int i = 1; i <= sample->NbPoles(); ++i) poles.SetValue(j, i, sample->Pole(i));
    }
    return poles;
}

std::unique_ptr<TopoDS_Solid> make_bspline_solid(
    rust::Slice<const double> coords,
    uint32_t nu, uint32_t nv,
    bool u_periodic, bool v_periodic)
{
    if (u_periodic && !v_periodic) throw std::runtime_error("a U-periodic grid needs a periodic V; only open U ends are capped");

    NCollection_Array2<gp_Pnt> grid(1, nu, 1, nv);
    for (int i = 1; i <= int(nu); ++i) {
        for (int j = 1; j <= int(nv); ++j) {
            const double* p = &coords[(size_t(i - 1) * nv + (j - 1)) * 3];
            grid.SetValue(i, j, gp_Pnt(p[0], p[1], p[2]));
        }
    }
    Handle(Geom_BSplineCurve) u_sample, v_sample;
    NCollection_Array2<gp_Pnt> poles = interpolate_columns(interpolate_columns(grid, u_periodic, u_sample), v_periodic, v_sample);
    Handle(Geom_BSplineSurface) surface = new Geom_BSplineSurface(
        poles, u_sample->Knots(), v_sample->Knots(), u_sample->Multiplicities(), v_sample->Multiplicities(),
        u_sample->Degree(), v_sample->Degree(), u_periodic, v_periodic);

    BRepBuilderAPI_MakeFace face_maker(surface, Precision::Confusion());
    if (!face_maker.IsDone()) throw std::runtime_error("BRepBuilderAPI_MakeFace did not complete");
    TopoDS_Face side = face_maker.Face();

    // Open in both directions: the solid is the side the normal du x dv points to.
    if (!u_periodic && !v_periodic) {
        double u1, u2, v1, v2;
        surface->Bounds(u1, u2, v1, v2);
        gp_Pnt p;
        gp_Vec du, dv;
        surface->D1((u1 + u2) / 2, (v1 + v2) / 2, p, du, dv);
        gp_Vec n = du.Crossed(dv);
        if (n.Magnitude() < Precision::Confusion()) throw std::runtime_error("degenerate normal at the middle of the surface");
        Bnd_Box box;
        BRepBndLib::Add(side, box);
        BRepPrimAPI_MakeHalfSpace maker(side, p.Translated(n.Normalized() * (1e-3 * std::sqrt(box.SquareExtent()))));
        return std::make_unique<TopoDS_Solid>(maker.Solid());
    }

    // Cap each open U end with a plane on the side face's own boundary edge; the
    // seam edges are skipped, so a torus is the side face alone.
    BRep_Builder builder;
    TopoDS_Shell shell;
    builder.MakeShell(shell);
    builder.Add(shell, side);
    for (TopExp_Explorer ex(side, TopAbs_EDGE); ex.More(); ex.Next()) {
        const TopoDS_Edge& edge = TopoDS::Edge(ex.Current());
        if (BRep_Tool::IsClosed(edge, side)) continue;
        BRepBuilderAPI_MakeFace cap_maker(BRepBuilderAPI_MakeWire(edge).Wire(), /*OnlyPlane=*/true);
        if (!cap_maker.IsDone()) throw std::runtime_error("could not cap the U boundaries");
        TopoDS_Face cap = cap_maker.Face();
        // The shared edge must run opposite ways in the two faces.
        if (TopExp_Explorer(cap, TopAbs_EDGE).Current().Orientation() == edge.Orientation()) cap.Reverse();
        builder.Add(shell, cap);
    }
    return solid_from_shell(shell);
}

// ==================== Streambuf bridges (ffi.cpp-internal) ====================

// std::streambuf subclass that reads from a Rust `dyn Read` via FFI callback
class RustReadStreambuf : public std::streambuf {
public:
    explicit RustReadStreambuf(RustReader& reader) : reader_(reader) {}

protected:
    int_type underflow() override {
        rust::Slice<uint8_t> slice(
            reinterpret_cast<uint8_t*>(buf_), sizeof(buf_));
        size_t n = rust_reader_read(reader_, slice);
        if (n == 0) return traits_type::eof();
        setg(buf_, buf_, buf_ + n);
        return traits_type::to_int_type(*gptr());
    }

    // Override to keep the vtable slot resolved within this TU instead of
    // referencing `std::basic_streambuf<char>::seekpos`, whose mangling depends
    // on `std::fpos<mbstate_t>` — and `mbstate_t` is a typedef to the internal
    // `_Mbstatet` on gcc 15 mingw but a different name on gcc 14, so the
    // external symbol fails to resolve when the prebuilt ships gcc 14
    // libstdc++.a but downstream links with gcc 15.
    pos_type seekpos(pos_type, std::ios_base::openmode = std::ios_base::in | std::ios_base::out) override {
        return pos_type(off_type(-1));
    }

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
    int_type overflow(int_type ch) override {
        if (ch != traits_type::eof()) {
            buf_[pos_++] = static_cast<char>(ch);
            if (pos_ >= sizeof(buf_)) {
                if (!flush_buf()) return traits_type::eof();
            }
        }
        return ch;
    }

    std::streamsize xsputn(const char* s, std::streamsize count) override {
        std::streamsize written = 0;
        while (written < count) {
            std::streamsize space = sizeof(buf_) - pos_;
            std::streamsize chunk = std::min(count - written, space);
            std::memcpy(buf_ + pos_, s + written, chunk);
            pos_ += static_cast<size_t>(chunk);
            written += chunk;
            if (pos_ >= sizeof(buf_)) {
                if (!flush_buf()) return written;
            }
        }
        return written;
    }

    int sync() override {
        return flush_buf() ? 0 : -1;
    }

    // See RustReadStreambuf::seekpos — same gcc 14/15 `_Mbstatet` mangling fix.
    pos_type seekpos(pos_type, std::ios_base::openmode = std::ios_base::in | std::ios_base::out) override {
        return pos_type(off_type(-1));
    }

private:
    bool flush_buf() {
        if (pos_ == 0) return true;
        rust::Slice<const uint8_t> slice(
            reinterpret_cast<const uint8_t*>(buf_), pos_);
        size_t n = rust_writer_write(writer_, slice);
        if (n < pos_) return false;
        pos_ = 0;
        return true;
    }

    RustWriter& writer_;
    char buf_[8192];
    size_t pos_ = 0;
};



std::unique_ptr<std::vector<TopoDS_Solid>> read_brep_stream(
    rust::Slice<const uint8_t> data, size_t& out_consumed)
{
    // istringstream because BinTools::Read seeks backwards to shared sub-shapes.
    std::istringstream iss(
        std::string(reinterpret_cast<const char*>(data.data()), data.size()));

    TopoDS_Shape shape;
    try {
        BinTools::Read(shape, iss);
    } catch (const Standard_Failure& f) {
        // OCCT raises this one without a message; out_consumed stays untouched.
        throw std::runtime_error(std::string("BinTools::Read failed: ") + f.what());
    }
    if (shape.IsNull()) throw std::runtime_error("BinTools::Read produced no shape");

    // clear() is load-bearing: a payload read to its last byte leaves eofbit set, and
    // tellg()'s sentry then turns that into failbit and returns -1.
    iss.clear();
    out_consumed = static_cast<size_t>(iss.tellg());
    return solids_of(shape);
}

void write_brep_stream(const std::vector<TopoDS_Solid>& solids, RustWriter& writer) {
    RustWriteStreambuf sbuf(writer);
    std::ostream os(&sbuf);
    // Without triangles: a stored triangulation tightens BRepBndLib boxes after a read.
    BinTools::Write(compound_of(solids), os, false, false, BinTools_FormatVersion_CURRENT);
    if (!os.good()) throw std::runtime_error("stream write failed");
}

} // namespace cadrum

#include <StepVisual_OverRidingStyledItem.hxx>
#include <StepShape_ShapeRepresentation.hxx>
#include <Transfer_TransientProcess.hxx>
#include <Interface_Graph.hxx>
#include <Interface_EntityIterator.hxx>
#include <Quantity_Color.hxx>
#include <NCollection_Sequence.hxx>
#include <STEPControl_Controller.hxx>
#include <STEPControl_ActorWrite.hxx>
#include <STEPConstruct.hxx>
#include <STEPConstruct_Styles.hxx>
#include <STEPConstruct_RenderingProperties.hxx>
#include <StepVisual_MechanicalDesignGeometricPresentationRepresentation.hxx>
#include <StepVisual_StyledItem.hxx>
#include <StepVisual_Colour.hxx>
#include <StepVisual_ColourRgb.hxx>
#include <StepRepr_RepresentationContext.hxx>
#include <StepData_StepModel.hxx>
#include <Transfer_FinderProcess.hxx>
#include <Transfer_TransientListBinder.hxx>
#include <TransferBRep.hxx>
#include <TransferBRep_ShapeMapper.hxx>
#include <XSControl_WorkSession.hxx>
#include <XSControl_TransferWriter.hxx>
#include <StepShape_ShapeDefinitionRepresentation.hxx>
#include <StepRepr_PropertyDefinition.hxx>
#include <StepBasic_ProductDefinition.hxx>
#include <StepBasic_ProductDefinitionFormation.hxx>
#include <StepBasic_Product.hxx>
#include <TCollection_HAsciiString.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <NCollection_Map.hxx>
#include <NCollection_DataMap.hxx>
#include <TCollection_AsciiString.hxx>
#include <optional>

namespace cadrum {

// IsOverriden in STEPCAFControl_Reader.cxx: a root style yields only to another root style.
static bool is_overridden(const Interface_Graph& graph, const Handle(StepVisual_StyledItem)& style, bool root)
{
    for (Interface_EntityIterator it = graph.Sharings(style); it.More(); it.Next()) {
        Handle(StepVisual_OverRidingStyledItem) over = Handle(StepVisual_OverRidingStyledItem)::DownCast(it.Value());
        if (over.IsNull()) continue;
        if (!root) return true;
        Handle(Standard_Transient) item = over->ItemAP242().Value();
        if (!item.IsNull() && item->IsKind(STANDARD_TYPE(StepShape_ShapeRepresentation))) return true;
    }
    return false;
}

// SetStyle in STEPCAFControl_Reader.cxx reduced to surface colors, the only ones cadrum keeps.
// Instances share their TShape with the part, so component styles need no instance lookup.
static void paint(const STEPConstruct_Styles& styles, const Handle(StepVisual_StyledItem)& style,
                  std::vector<std::pair<TopoDS_Shape, Quantity_Color>>& painted)
{
    if (style.IsNull()) return;
    Handle(StepVisual_OverRidingStyledItem) over = Handle(StepVisual_OverRidingStyledItem)::DownCast(style);
    if (!over.IsNull()) paint(styles, over->OverRiddenStyle(), painted);

    Handle(StepVisual_Colour) surf, bound, curve;
    STEPConstruct_RenderingProperties props;
    bool component = false;
    styles.GetColors(style, surf, bound, curve, props, component);
    // Quantity_Color holds sRGB as stored, bypassing DecodeColor's sRGB-to-linear step.
    Quantity_Color color;
    Handle(StepVisual_ColourRgb) rgb = Handle(StepVisual_ColourRgb)::DownCast(surf);
    if (!rgb.IsNull()) {
        color.SetValues(rgb->Red(), rgb->Green(), rgb->Blue(), Quantity_TOC_RGB);
    } else {
        if (!surf.IsNull()) STEPConstruct_Styles::DecodeColor(surf, color);
        else if (props.IsDefined()) color = props.GetRGBAColor().GetRGB();
        else return;
        double r, g, b;
        color.Values(r, g, b, Quantity_TOC_sRGB);
        color.SetValues(r, g, b, Quantity_TOC_RGB);
    }

    const Handle(Transfer_TransientProcess)& tp = styles.TransientProcess();
    int index = tp->MapIndex(style->ItemAP242().Value());
    if (index <= 0) return;
    TopoDS_Shape shape = TransferBRep::ShapeResult(tp->MapItem(index));
    if (!shape.IsNull()) painted.emplace_back(shape, color);
}

// Face and solid keys share one map: a TShape* is unique across shape types. A color on a
// compound reaches its solids unless a solid has its own; it is not expanded onto faces.
static void collect_colors(
    const std::vector<std::pair<TopoDS_Shape, Quantity_Color>>& painted,
    std::unordered_map<uint64_t, std::array<float, 3>>& colorMap)
{
    std::unordered_map<uint64_t, std::array<float, 3>> inherited;
    for (const auto& [shape, color] : painted) {
        std::array<float, 3> rgb = {(float)color.Red(), (float)color.Green(), (float)color.Blue()};
        TopAbs_ShapeEnum type = shape.ShapeType();
        if (type == TopAbs_FACE || type == TopAbs_SOLID) {
            colorMap[reinterpret_cast<uint64_t>(shape.TShape().get())] = rgb;
            continue;
        }
        for (TopExp_Explorer ex(shape, TopAbs_SOLID); ex.More(); ex.Next()) {
            inherited[reinterpret_cast<uint64_t>(ex.Current().TShape().get())] = rgb;
        }
    }
    colorMap.insert(inherited.begin(), inherited.end());
}

std::unique_ptr<std::vector<TopoDS_Solid>> read_step_color_stream(
    RustReader&          reader,
    rust::Vec<uint64_t>& out_ids,
    rust::Vec<float>&    out_rgb)
{
    STEPControl_Reader step_reader;
    RustReadStreambuf sbuf(reader);
    std::istream is(&sbuf);
    if (step_reader.ReadStream("stream", is) != IFSelect_RetDone) throw std::runtime_error("STEPControl_Reader could not read the stream");
    step_reader.TransferRoots(Message_ProgressRange());

    BRep_Builder builder;
    TopoDS_Compound compound;
    builder.MakeCompound(compound);
    for (int i = 1; i <= step_reader.NbShapes(); i++) {
        builder.Add(compound, step_reader.Shape(i));
    }

    // Root styles first, as leaf styles override them; a later style wins on the same shape.
    std::vector<std::pair<TopoDS_Shape, Quantity_Color>> painted;
    STEPConstruct_Styles styles(step_reader.WS());
    if (styles.LoadStyles()) {
        const Interface_Graph& graph = styles.Graph();
        for (int i = 1; i <= styles.NbRootStyles(); i++) {
            if (!is_overridden(graph, styles.RootStyle(i), true)) paint(styles, styles.RootStyle(i), painted);
        }
        for (int i = 1; i <= styles.NbStyles(); i++) {
            if (!is_overridden(graph, styles.Style(i), false)) paint(styles, styles.Style(i), painted);
        }
    }

    std::unordered_map<uint64_t, std::array<float, 3>> colorMap;
    collect_colors(painted, colorMap);

    // Recover Solids from disjoint shells / loose faces (#129); also remaps
    // colorMap keys for faces whose TShape* changed during sewing.
    TopoDS_Shape post = try_sew_orphan_faces(compound, &colorMap);

    // Walk the POST-processed shape so sewing's new TShape* are picked up, and
    // entries it no longer holds are dropped by not being reached.
    auto emit = [&](const TopoDS_Shape& sub, uint64_t id) {
        auto it = colorMap.find(own_id(sub));
        if (it == colorMap.end()) return;
        out_ids.push_back(id);
        out_rgb.push_back(it->second[0]);
        out_rgb.push_back(it->second[1]);
        out_rgb.push_back(it->second[2]);
    };
    for (TopExp_Explorer sx(post, TopAbs_SOLID); sx.More(); sx.Next()) {
        for (TopExp_Explorer ex(sx.Current(), TopAbs_FACE); ex.More(); ex.Next()) {
            emit(ex.Current(), sub_id(ex.Current(), sx.Current().Location()));
        }
        emit(sx.Current(), own_id(sx.Current()));
    }

    return solids_of(post);
}

// STEPCAFControl_ActorWrite with no registered assembly: a compound is never
// split into an assembly, so the solids share one shape representation.
class FlatActorWrite : public STEPControl_ActorWrite {
public:
    bool IsAssembly(const Handle(StepData_StepModel)&, TopoDS_Shape&) const override { return false; }
};

class FlatController : public STEPControl_Controller {
public:
    FlatController() { myAdaptorWrite = new FlatActorWrite; }
};

// Copy of FindEntities in STEPCAFControl_Writer.cxx: falls back to the pieces a
// shape was split into during shape processing.
static void find_entities(const Handle(Transfer_FinderProcess)& fp, const TopoDS_Shape& shape,
                          NCollection_Sequence<Handle(StepRepr_RepresentationItem)>& items)
{
    TopLoc_Location loc;
    Handle(StepRepr_RepresentationItem) item = STEPConstruct::FindEntity(fp, shape, loc);
    if (!item.IsNull()) {
        items.Append(item);
        return;
    }
    Handle(Transfer_Binder) binder = fp->Find(TransferBRep::ShapeMapper(fp, shape));
    if (binder.IsNull()) return;
    Handle(Transfer_TransientListBinder) list = Handle(Transfer_TransientListBinder)::DownCast(binder);
    if (list.IsNull()) {
        for (TopoDS_Iterator it(shape); it.More(); it.Next()) {
            Handle(StepRepr_RepresentationItem) sub = STEPConstruct::FindEntity(fp, it.Value(), loc);
            if (!sub.IsNull()) items.Append(sub);
        }
        return;
    }
    for (int i = 1; i <= list->NbTransients(); i++) {
        Handle(StepRepr_RepresentationItem) sub = Handle(StepRepr_RepresentationItem)::DownCast(list->Transient(i));
        if (!sub.IsNull()) items.Append(sub);
    }
}

struct StyleWriter {
    STEPConstruct_Styles& styles;
    const std::unordered_map<uint64_t, Quantity_Color>& colors;
    NCollection_Map<TopoDS_Shape, TopTools_ShapeMapHasher> seen;
    NCollection_DataMap<TCollection_AsciiString, Handle(Standard_Transient)> dpdcs;
    NCollection_DataMap<gp_Pnt, Handle(Standard_Transient)> rgbs;

    // MakeSTEPStyles in STEPCAFControl_Writer.cxx for a visible, non-component shape
    // with surface colors only: a style on a solid overrides into its faces.
    void walk(const TopoDS_Shape& shape, const Handle(StepVisual_StyledItem)& over, const Quantity_Color* inherit, const TopLoc_Location& owner) {
        if (!seen.Add(shape)) return;
        std::optional<Quantity_Color> color;
        if (inherit) color = *inherit;
        TopAbs_ShapeEnum type = shape.ShapeType();
        // Face colours are keyed relative to the enclosing solid.
        const TopLoc_Location here = type == TopAbs_SOLID ? shape.Location() : owner;
        if (type == TopAbs_SOLID || type == TopAbs_FACE) {
            auto it = colors.find(type == TopAbs_SOLID ? own_id(shape) : sub_id(shape, here));
            if (it != colors.end()) color = it->second;
        }
        Handle(StepVisual_StyledItem) style = over;
        bool own = color.has_value();
        if (own && type != TopAbs_COMPOUND) {
            // EncodeColor gamma-shifts COLOUR_RGB, so it only writes 0/1 colors: the eight
            // pre-defined ones, which gamma leaves unchanged.
            gp_Pnt key(color->Red(), color->Green(), color->Blue());
            auto unit = [](double c) { return c == 0.0 || c == 1.0; };
            Handle(Standard_Transient) surf;
            if (unit(key.X()) && unit(key.Y()) && unit(key.Z())) {
                surf = STEPConstruct_Styles::EncodeColor(*color, dpdcs, rgbs);
            } else if (!rgbs.Find(key, surf)) {
                Handle(StepVisual_ColourRgb) rgb = new StepVisual_ColourRgb;
                rgb->Init(new TCollection_HAsciiString(""), key.X(), key.Y(), key.Z());
                surf = *rgbs.Bound(key, rgb);
            }
            NCollection_Sequence<Handle(StepRepr_RepresentationItem)> items;
            find_entities(styles.FinderProcess(), shape, items);
            for (const Handle(StepRepr_RepresentationItem)& item : items) {
                Handle(StepVisual_PresentationStyleAssignment) psa =
                    styles.MakeColorPSA(item, Handle(StepVisual_Colour)::DownCast(surf), nullptr, STEPConstruct_RenderingProperties());
                style = styles.AddStyle(item, psa, over);
                own = false;
            }
        }
        if (type == TopAbs_EDGE) return;
        for (TopoDS_Iterator it(shape); it.More(); it.Next()) {
            walk(it.Value(), style, own ? &*color : nullptr, here);
        }
    }
};

void write_step_color_stream(
    const std::vector<TopoDS_Solid>& solids,
    rust::Slice<const uint64_t>      ids,
    rust::Slice<const float>         rgb,
    RustWriter&                      writer)
{
    TopoDS_Compound shape = compound_of(solids);
    std::unordered_map<uint64_t, Quantity_Color> colors;
    for (size_t i = 0; i < ids.size(); i++) {
        colors[ids[i]] = Quantity_Color(rgb[3*i], rgb[3*i+1], rgb[3*i+2], Quantity_TOC_RGB);
    }

    STEPControl_Writer step_writer;
    Handle(XSControl_WorkSession) ws = step_writer.WS();
    ws->SetController(new FlatController);
    if (step_writer.Transfer(shape, STEPControl_AsIs, false) != IFSelect_RetDone) throw std::runtime_error("STEPControl_Writer could not transfer the shape");
    ws->ComputeGraph(true);

    // XCAF names a shape label after its type, and STEPCAFControl_Writer writes that name to the product.
    Handle(Transfer_FinderProcess) fp = ws->TransferWriter()->FinderProcess();
    Handle(StepShape_ShapeDefinitionRepresentation) sdr;
    if (fp->FindTypedTransient(TransferBRep::ShapeMapper(fp, shape), STANDARD_TYPE(StepShape_ShapeDefinitionRepresentation), sdr)) {
        Handle(StepBasic_Product) product = sdr->Definition().PropertyDefinition()->Definition().ProductDefinition()->Formation()->OfProduct();
        Handle(TCollection_HAsciiString) name = new TCollection_HAsciiString("COMPOUND");
        product->SetId(name);
        product->SetName(name);
    }

    STEPConstruct_Styles styles(ws);
    Handle(StepRepr_RepresentationContext) context = styles.FindContext(shape);
    if (!context.IsNull()) {
        StyleWriter{styles, colors, {}, {}, {}}.walk(shape, nullptr, nullptr, TopLoc_Location());
        Handle(StepData_StepModel) model = step_writer.Model();
        Handle(StepVisual_MechanicalDesignGeometricPresentationRepresentation) mdgpr;
        if (styles.CreateMDGPR(context, mdgpr, model)) model->AddWithRefs(mdgpr);
    }
    ws->ComputeGraph(true);

    RustWriteStreambuf sbuf(writer);
    std::ostream os(&sbuf);
    if (step_writer.WriteStream(os) != IFSelect_RetDone) throw std::runtime_error("STEPControl_Writer could not write the stream");
}

} // namespace cadrum

