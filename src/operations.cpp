#include "cadrum/src/ffi.rs.h"
#include <BRepCheck_Analyzer.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <BRepCheck_Result.hxx>
#include <BRep_Tool.hxx>
#include <TopExp.hxx>
#include <TopoDS.hxx>
#include <NCollection_IndexedMap.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <Standard_Failure.hxx>
#include <stdexcept>

#define OP_CATCH catch (const Standard_Failure& e) { throw std::runtime_error(e.what()); }

namespace cadrum {
ValidationData op_validate(const TopoDS_Shape& shape) try {
    ValidationData out{};
    BRepCheck_Analyzer analyzer(shape);
    out.valid = analyzer.IsValid();
    out.closed = BRep_Tool::IsClosed(shape);
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> map;
    TopExp::MapShapes(shape, map);
    for (int i = 1; i <= map.Extent(); ++i) {
        const auto& s = map(i);
        const auto& result = analyzer.Result(s);
        if (result.IsNull()) continue;
        for (const auto status : result->Status()) {
            if (status != BRepCheck_NoError) out.issues.push_back(ValidationEntry{uint32_t(i), 0, uint32_t(s.ShapeType()), uint32_t(status)});
        }
        for (result->InitContextIterator(); result->MoreShapeInContext(); result->NextShapeInContext()) {
            const int context = map.FindIndex(result->ContextualShape());
            for (const auto status : result->StatusOnShape()) {
                if (status != BRepCheck_NoError) out.issues.push_back(ValidationEntry{uint32_t(i), uint32_t(context), uint32_t(s.ShapeType()), uint32_t(status)});
            }
        }
    }
    return out;
} OP_CATCH
}

#include <BRepBuilderAPI_Copy.hxx>
#include <ShapeFix_Shape.hxx>
#include <ShapeFix_Shell.hxx>
#include <ShapeFix_Solid.hxx>
#include <ShapeFix_Face.hxx>
#include <ShapeFix_Wire.hxx>
#include <TopExp_Explorer.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_List.hxx>

namespace cadrum {
static double maximum_tolerance(const TopoDS_Shape& s) {
    double value = 0.0;
    for (TopExp_Explorer ex(s, TopAbs_VERTEX); ex.More(); ex.Next()) value = std::max(value, BRep_Tool::Tolerance(TopoDS::Vertex(ex.Current())));
    for (TopExp_Explorer ex(s, TopAbs_EDGE); ex.More(); ex.Next()) value = std::max(value, BRep_Tool::Tolerance(TopoDS::Edge(ex.Current())));
    for (TopExp_Explorer ex(s, TopAbs_FACE); ex.More(); ex.Next()) value = std::max(value, BRep_Tool::Tolerance(TopoDS::Face(ex.Current())));
    return value;
}
static size_t boundary_edges(const TopoDS_Shape& s) {
    NCollection_IndexedDataMap<TopoDS_Shape, NCollection_List<TopoDS_Shape>, TopTools_ShapeMapHasher> map;
    TopExp::MapShapesAndAncestors(s, TopAbs_EDGE, TopAbs_FACE, map);
    size_t count = 0;
    for (int i = 1; i <= map.Extent(); ++i) if (map.FindFromIndex(i).Extent() == 1 && !BRep_Tool::IsClosed(TopoDS::Edge(map.FindKey(i)), TopoDS::Face(map.FindFromIndex(i).First()))) ++count;
    return count;
}
RepairData op_repair(const TopoDS_Shape& shape, double precision, double max_tolerance, double max_deviation, rust::Slice<const bool> flags) try {
    if (flags.size() != 5) throw std::runtime_error("repair: invalid flags");
    BRepBuilderAPI_Copy copier(shape);
    const TopoDS_Shape copy = copier.Shape();
    ShapeFix_Shape fix(copy);
    fix.SetPrecision(precision);
    fix.SetMaxTolerance(max_tolerance);
    fix.FixFreeWireMode() = flags[0];
    fix.FixFreeFaceMode() = flags[1];
    fix.FixFreeShellMode() = flags[2];
    fix.FixSolidMode() = flags[3];
    fix.FixShellTool()->FixFaceMode() = flags[1];
    fix.FixFaceTool()->FixWireMode() = flags[0];
    fix.FixVertexTolMode() = 0;
    RepairData out{};
    out.changed = fix.Perform();
    const TopoDS_Shape result = fix.Shape();
    out.tolerance_before = maximum_tolerance(shape);
    out.tolerance_after = maximum_tolerance(result);
    out.boundary_before = boundary_edges(shape);
    out.boundary_after = boundary_edges(result);
    if (out.tolerance_after > max_tolerance * (1.0 + 1e-10)) throw std::runtime_error("repair: maximum tolerance exceeded");
    if (!flags[4] && out.boundary_after < out.boundary_before) throw std::runtime_error("repair: closing a boundary is not permitted");
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> original;
    TopExp::MapShapes(shape, original);
    auto measure = [&](const gp_Pnt& point) {
        BRepExtrema_DistShapeShape distance(BRepBuilderAPI_MakeVertex(point).Vertex(), result);
        if (!distance.IsDone()) throw std::runtime_error("repair deviation measurement failed");
        out.maximum_deviation = std::max(out.maximum_deviation, distance.Value()); ++out.sample_count;
        if (out.maximum_deviation > max_deviation) throw std::runtime_error("repair sampled deviation exceeds limit");
    };
    for (int i=1;i<=original.Extent();++i) {
        const auto copied=copier.ModifiedShape(original(i));
        const auto repaired=fix.Context()->Apply(copied);
        if (!copied.IsSame(repaired) || maximum_tolerance(original(i)) != maximum_tolerance(repaired)) out.changed_subshapes.push_back(i);
        if (original(i).ShapeType()==TopAbs_VERTEX) measure(BRep_Tool::Pnt(TopoDS::Vertex(original(i))));
        if (original(i).ShapeType()==TopAbs_EDGE && !BRep_Tool::Degenerated(TopoDS::Edge(original(i)))) {
            BRepAdaptor_Curve curve(TopoDS::Edge(original(i)));
            if (std::isfinite(curve.FirstParameter()) && std::isfinite(curve.LastParameter())) for(int j=0;j<=4;++j) measure(curve.Value(curve.FirstParameter()+(curve.LastParameter()-curve.FirstParameter())*j/4.0));
        }
    }
    out.shape = std::make_unique<TopoDS_Shape>(result);
    return out;
} OP_CATCH
}

#include <BRepBuilderAPI_GTransform.hxx>
#include <gp_GTrsf.hxx>
#include <gp_Mat.hxx>
namespace cadrum {
template<class Modified, class Generated, class Deleted>
static OperationData collect_history(const TopoDS_Shape& result, const std::vector<TopoDS_Shape>& inputs, Modified modified_shapes, Generated generated_shapes, Deleted is_deleted) {
    if (result.IsNull()) throw std::runtime_error("kernel produced null topology");
    OperationData out{};
    out.shape = std::make_unique<TopoDS_Shape>(result);
    out.complete = true;
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> targets;
    TopExp::MapShapes(result, targets);
    for (size_t operand = 0; operand < inputs.size(); ++operand) {
        NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> sources;
        TopExp::MapShapes(inputs[operand], sources);
        for (int i = 1; i <= sources.Extent(); ++i) {
            const auto& source = sources(i);
            if (source.ShapeType() != TopAbs_VERTEX && source.ShapeType() != TopAbs_EDGE && source.ShapeType() != TopAbs_FACE) continue;
            bool found = false;
            auto add = [&](const TopoDS_Shape& target, uint32_t relation) {
                const int index = targets.FindIndex(target);
                if (index == 0) return;
                out.history.push_back(HistoryEntry{uint32_t(operand), uint32_t(i), uint32_t(index), relation, uint32_t(source.ShapeType()), uint32_t(target.ShapeType()), reinterpret_cast<uint64_t>(source.TShape().get()), reinterpret_cast<uint64_t>(target.TShape().get())});
                found = true;
            };
            for (const auto& modified : modified_shapes(source)) add(modified, 0);
            for (const auto& generated : generated_shapes(source)) add(generated, 1);
            if (targets.Contains(source)) add(source, 3);
            if (!found) {
                const bool removed = is_deleted(source);
                out.history.push_back(HistoryEntry{uint32_t(operand), uint32_t(i), 0, uint32_t(removed ? 2 : 4), uint32_t(source.ShapeType()), 0, reinterpret_cast<uint64_t>(source.TShape().get()), 0});
                if (!removed) out.complete = false;
            }
        }
    }
    return out;
}
template<class Builder>
static OperationData operation_result(Builder& builder, const std::vector<TopoDS_Shape>& inputs) {
    return collect_history(builder.Shape(), inputs,
        [&](const TopoDS_Shape& s) { return builder.Modified(s); },
        [&](const TopoDS_Shape& s) { return builder.Generated(s); },
        [&](const TopoDS_Shape& s) { return builder.IsDeleted(s); });
}

OperationData op_affine(const TopoDS_Shape& s, rust::Slice<const double> v) try {
    if (v.size() != 12) throw std::runtime_error("affine: expected 12 matrix coefficients");
    gp_GTrsf t(gp_Mat(v[0],v[1],v[2],v[4],v[5],v[6],v[8],v[9],v[10]), gp_XYZ(v[3],v[7],v[11]));
    BRepBuilderAPI_GTransform builder(s, t, true);
    if (!builder.IsDone()) throw std::runtime_error("affine: transform failed");
    return operation_result(builder, {s});
} OP_CATCH
}

#include <Geom_BezierCurve.hxx>
#include <Geom_BSplineCurve.hxx>
#include <Geom_BezierSurface.hxx>
#include <Geom_BSplineSurface.hxx>
#include <Geom_Ellipse.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepTools.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_Array2.hxx>
namespace cadrum {
template<class T> static NCollection_Array1<T> array1(const rust::Vec<T>& v) {
    NCollection_Array1<T> out(1, int(v.size())); for (int i=1;i<=out.Length();++i) out(i)=v[i-1]; return out;
}
static Handle(Geom_Curve) authored_curve(const SplineData& d) {
    NCollection_Array1<gp_Pnt> p(1,int(d.u_count));
    for(int i=1;i<=p.Length();++i) p(i)=gp_Pnt(d.poles[3*(i-1)],d.poles[3*(i-1)+1],d.poles[3*(i-1)+2]);
    if(d.bezier) return new Geom_BezierCurve(p,array1(d.weights));
    return new Geom_BSplineCurve(p,array1(d.weights),array1(d.u_knots),array1(d.u_mults),int(d.u_degree),d.u_periodic);
}
std::unique_ptr<TopoDS_Edge> op_authored_curve(const SplineData& d,rust::Slice<const double> trim) try {
    const auto c=authored_curve(d);
    double a=c->FirstParameter(),b=c->LastParameter();
    if(trim.size()==2) { if(trim[0]<a || trim[1]>b) throw std::runtime_error("curve trim outside domain"); a=trim[0]; b=trim[1]; }
    BRepBuilderAPI_MakeEdge builder(c,a,b);
    if(!builder.IsDone()) throw std::runtime_error("authored curve construction failed");
    return std::make_unique<TopoDS_Edge>(builder.Edge());
} OP_CATCH
std::unique_ptr<TopoDS_Face> op_authored_surface(const SplineData& d,rust::Slice<const double> trim) try {
    NCollection_Array2<gp_Pnt> p(1,int(d.u_count),1,int(d.v_count));
    NCollection_Array2<double> w(1,int(d.u_count),1,int(d.v_count));
    for(int i=1;i<=int(d.u_count);++i) for(int j=1;j<=int(d.v_count);++j) {
        const size_t k=size_t(i-1)*d.v_count+j-1; p(i,j)=gp_Pnt(d.poles[3*k],d.poles[3*k+1],d.poles[3*k+2]); w(i,j)=d.weights[k];
    }
    Handle(Geom_Surface) surface;
    if(d.bezier) surface=new Geom_BezierSurface(p,w);
    else surface=new Geom_BSplineSurface(p,w,array1(d.u_knots),array1(d.v_knots),array1(d.u_mults),array1(d.v_mults),d.u_degree,d.v_degree,d.u_periodic,d.v_periodic);
    double a,b,c,e; surface->Bounds(a,b,c,e);
    if(trim.size()==4) { if(trim[0]<a||trim[1]>b||trim[2]<c||trim[3]>e) throw std::runtime_error("surface trim outside domain"); a=trim[0];b=trim[1];c=trim[2];e=trim[3]; }
    BRepBuilderAPI_MakeFace builder(surface,a,b,c,e,1e-7);
    if(!builder.IsDone()) throw std::runtime_error("authored surface construction failed");
    return std::make_unique<TopoDS_Face>(builder.Face());
} OP_CATCH
std::unique_ptr<TopoDS_Edge> op_ellipse(rust::Slice<const double> f,double major,double minor,rust::Slice<const double> trim) try {
    Handle(Geom_Ellipse) c=new Geom_Ellipse(gp_Ax2(gp_Pnt(f[0],f[1],f[2]),gp_Dir(f[3],f[4],f[5]),gp_Dir(f[6],f[7],f[8])),major,minor);
    const double a=trim.empty()?0.0:trim[0],b=trim.empty()?2.0*M_PI:trim[1];
    if(b-a>2.0*M_PI+1e-12) throw std::runtime_error("ellipse arc exceeds one turn");
    BRepBuilderAPI_MakeEdge builder(c,a,b);
    if(!builder.IsDone()) throw std::runtime_error("ellipse construction failed");
    return std::make_unique<TopoDS_Edge>(builder.Edge());
} OP_CATCH
static void append_xyz(rust::Vec<double>& out,const gp_XYZ& p) { out.push_back(p.X());out.push_back(p.Y());out.push_back(p.Z()); }
SplineData op_curve_control(const TopoDS_Edge& edge) try {
    double a,b; const Handle(Geom_Curve) c=BRep_Tool::Curve(edge,a,b);
    const auto bez=Handle(Geom_BezierCurve)::DownCast(c); const auto bs=Handle(Geom_BSplineCurve)::DownCast(c);
    if(bez.IsNull()&&bs.IsNull()) throw std::runtime_error("curve is neither Bezier nor B-spline");
    SplineData d{};d.bezier=!bez.IsNull();d.u_count=d.bezier?bez->NbPoles():bs->NbPoles();d.v_count=1;
    for(int i=1;i<=int(d.u_count);++i) { append_xyz(d.poles,(d.bezier?bez->Pole(i):bs->Pole(i)).XYZ()); d.weights.push_back(d.bezier?bez->Weight(i):bs->Weight(i)); }
    d.u_degree=d.bezier?bez->Degree():bs->Degree();
    if(!d.bezier) { d.u_periodic=bs->IsPeriodic(); for(int i=1;i<=bs->NbKnots();++i) {d.u_knots.push_back(bs->Knot(i));d.u_mults.push_back(bs->Multiplicity(i));} }
    return d;
} OP_CATCH
SplineData op_surface_control(const TopoDS_Face& face) try {
    const auto surface=BRep_Tool::Surface(face); const auto bez=Handle(Geom_BezierSurface)::DownCast(surface);const auto bs=Handle(Geom_BSplineSurface)::DownCast(surface);
    if(bez.IsNull()&&bs.IsNull()) throw std::runtime_error("surface is neither Bezier nor B-spline");
    SplineData d{};d.bezier=!bez.IsNull();d.u_count=d.bezier?bez->NbUPoles():bs->NbUPoles();d.v_count=d.bezier?bez->NbVPoles():bs->NbVPoles();
    for(int i=1;i<=int(d.u_count);++i) for(int j=1;j<=int(d.v_count);++j) {append_xyz(d.poles,(d.bezier?bez->Pole(i,j):bs->Pole(i,j)).XYZ());d.weights.push_back(d.bezier?bez->Weight(i,j):bs->Weight(i,j));}
    d.u_degree=d.bezier?bez->UDegree():bs->UDegree();d.v_degree=d.bezier?bez->VDegree():bs->VDegree();
    if(!d.bezier) { d.u_periodic=bs->IsUPeriodic();d.v_periodic=bs->IsVPeriodic();for(int i=1;i<=bs->NbUKnots();++i){d.u_knots.push_back(bs->UKnot(i));d.u_mults.push_back(bs->UMultiplicity(i));}for(int i=1;i<=bs->NbVKnots();++i){d.v_knots.push_back(bs->VKnot(i));d.v_mults.push_back(bs->VMultiplicity(i));} }
    return d;
} OP_CATCH
rust::Vec<double> op_curve_range(const TopoDS_Edge& edge) try { BRepAdaptor_Curve c(edge);return {c.FirstParameter(),c.LastParameter()}; } OP_CATCH
rust::Vec<double> op_curve_evaluate(const TopoDS_Edge& edge,double t) try { BRepAdaptor_Curve c(edge);gp_Pnt p;gp_Vec d1,d2;c.D2(t,p,d1,d2);rust::Vec<double> out;append_xyz(out,p.XYZ());append_xyz(out,d1.XYZ());append_xyz(out,d2.XYZ());return out; } OP_CATCH
std::unique_ptr<TopoDS_Edge> op_trim_edge(const TopoDS_Edge& edge,rust::Slice<const double> trim) try { double a,b;auto c=BRep_Tool::Curve(edge,a,b);BRepBuilderAPI_MakeEdge builder(c,trim[0],trim[1]);if(!builder.IsDone())throw std::runtime_error("edge trim failed");return std::make_unique<TopoDS_Edge>(builder.Edge()); } OP_CATCH
rust::Vec<double> op_surface_range(const TopoDS_Face& face) try {double a,b,c,d;BRepTools::UVBounds(face,a,b,c,d);return {a,b,c,d};} OP_CATCH
rust::Vec<double> op_surface_evaluate(const TopoDS_Face& face,double u,double v) try {
    BRepAdaptor_Surface s(face);gp_Pnt p;gp_Vec du,dv,duu,dvv,duv;s.D2(u,v,p,du,dv,duu,dvv,duv);
    gp_Vec normal=du.Crossed(dv);if(normal.SquareMagnitude()>1e-30) normal.Normalize();if(face.Orientation()==TopAbs_REVERSED)normal.Reverse();
    rust::Vec<double> out;for(const auto& xyz:{p.XYZ(),du.XYZ(),dv.XYZ(),duu.XYZ(),duv.XYZ(),dvv.XYZ(),normal.XYZ()})append_xyz(out,xyz);return out;
} OP_CATCH
std::unique_ptr<TopoDS_Face> op_trim_surface(const TopoDS_Face& face,rust::Slice<const double> trim) try {
    auto s=BRep_Tool::Surface(face);double a,b,c,d;s->Bounds(a,b,c,d);if(trim[0]<a||trim[1]>b||trim[2]<c||trim[3]>d)throw std::runtime_error("surface trim outside domain");
    BRepBuilderAPI_MakeFace builder(s,trim[0],trim[1],trim[2],trim[3],1e-7);if(!builder.IsDone())throw std::runtime_error("surface trim failed");return std::make_unique<TopoDS_Face>(builder.Face());
} OP_CATCH
}

#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepClass_FaceClassifier.hxx>
#include <BRep_Builder.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Iterator.hxx>
#include <gp_Pln.hxx>
namespace cadrum {
OperationData op_profile(const std::vector<TopoDS_Shape>& wires,double tolerance,bool explicit_outer) try {
    if(wires.empty())throw std::runtime_error("profile: no boundary wires");
    std::vector<TopoDS_Face> disks;
    std::vector<TopoDS_Wire> normalized;
    gp_Pln plane;
    for(size_t i=0;i<wires.size();++i) {
        const auto& wire=TopoDS::Wire(wires[i]);
        if(!BRep_Tool::IsClosed(wire)||!BRepCheck_Analyzer(wire).IsValid())throw std::runtime_error("profile: invalid or open wire");
        BRepBuilderAPI_MakeFace maker(wire,true);
        if(!maker.IsDone()||!BRepCheck_Analyzer(maker.Face()).IsValid())throw std::runtime_error("profile: non-planar or self-intersecting wire");
        const gp_Pln local=BRepAdaptor_Surface(maker.Face()).Plane();
        if(i==0)plane=local;
        if(std::abs(local.Axis().Direction().Dot(plane.Axis().Direction()))<1.0-1e-10||plane.Distance(local.Location())>tolerance)throw std::runtime_error("profile: wires are not coplanar");
        BRepBuilderAPI_MakeFace on_plane(plane,wire,true);
        disks.push_back(on_plane.Face());normalized.push_back(BRepTools::OuterWire(on_plane.Face()));
    }
    std::vector<std::vector<bool>> inside(wires.size(),std::vector<bool>(wires.size(),false));
    std::vector<int> depth(wires.size(),0);
    for(size_t i=0;i<wires.size();++i) for(size_t j=i+1;j<wires.size();++j) {
        BRepExtrema_DistShapeShape distance(wires[i],wires[j]);
        if(!distance.IsDone()||distance.Value()<=tolerance)throw std::runtime_error("profile: loops intersect or touch");
    }
    for(size_t i=0;i<wires.size();++i) {
        TopExp_Explorer vertex(wires[i],TopAbs_VERTEX);if(!vertex.More())throw std::runtime_error("profile: wire has no vertex");
        const gp_Pnt point=BRep_Tool::Pnt(TopoDS::Vertex(vertex.Current()));
        for(size_t j=0;j<wires.size();++j) if(i!=j) { BRepClass_FaceClassifier classifier(disks[j],point,tolerance);inside[i][j]=classifier.State()==TopAbs_IN;if(inside[i][j])++depth[i]; }
    }
    if(explicit_outer) { if(depth[0]!=0)throw std::runtime_error("face: outer wire is inside a hole");for(size_t i=1;i<wires.size();++i)if(depth[i]!=1||!inside[i][0])throw std::runtime_error("face: hole is outside outer wire or nested"); }
    BRep_Builder b;TopoDS_Compound compound;b.MakeCompound(compound);TopoDS_Shape only;size_t count=0;
    for(size_t i=0;i<wires.size();++i) if(depth[i]%2==0) {
        BRepBuilderAPI_MakeFace face(plane,normalized[i],true);
        for(size_t j=0;j<wires.size();++j)if(depth[j]==depth[i]+1&&inside[j][i])face.Add(TopoDS::Wire(normalized[j].Reversed()));
        if(!face.IsDone()||!BRepCheck_Analyzer(face.Face()).IsValid())throw std::runtime_error("profile: invalid nested region");
        only=face.Face();b.Add(compound,only);++count;
    }
    OperationData out{};out.shape=std::make_unique<TopoDS_Shape>(count==1?only:compound);out.complete=false;return out;
} OP_CATCH
static void require_profile(const TopoDS_Shape& s) {
    if(s.ShapeType()==TopAbs_FACE)return;
    if(s.ShapeType()!=TopAbs_COMPOUND)throw std::runtime_error("unsupported: expected face or compound of faces");
    for(TopoDS_Iterator it(s);it.More();it.Next())require_profile(it.Value());
}
OperationData op_prism(const TopoDS_Shape& s,double x,double y,double z) try {
    require_profile(s);if(!BRepCheck_Analyzer(s).IsValid())throw std::runtime_error("extrude: invalid profile");
    BRepPrimAPI_MakePrism builder(s,gp_Vec(x,y,z),true);if(!builder.IsDone())throw std::runtime_error("extrude failed");return operation_result(builder,{s});
} OP_CATCH
OperationData op_revolution(const TopoDS_Shape& s,rust::Slice<const double> v) try {
    require_profile(s);if(!BRepCheck_Analyzer(s).IsValid())throw std::runtime_error("revolve: invalid profile");
    BRepPrimAPI_MakeRevol builder(s,gp_Ax1(gp_Pnt(v[0],v[1],v[2]),gp_Dir(v[3],v[4],v[5])),v[6],true);if(!builder.IsDone())throw std::runtime_error("revolve failed");return operation_result(builder,{s});
} OP_CATCH
}

#include <BRepBuilderAPI_MakePolygon.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepLib.hxx>
namespace cadrum {
std::unique_ptr<TopoDS_Shape> op_faceted_mesh(rust::Slice<const double> positions,rust::Slice<const uint32_t> indices,double tolerance,bool solid) try {
    BRepBuilderAPI_Sewing sewing(tolerance);
    for(size_t i=0;i<indices.size();i+=3){
        BRepBuilderAPI_MakePolygon polygon;
        for(int j=0;j<3;++j){size_t k=size_t(indices[i+j])*3;polygon.Add(gp_Pnt(positions[k],positions[k+1],positions[k+2]));}polygon.Close();
        BRepBuilderAPI_MakeFace face(polygon.Wire());if(!face.IsDone())throw std::runtime_error("triangle face construction failed");sewing.Add(face.Face());
    }
    if(indices.empty())return native_build({},TopAbs_COMPOUND);
    sewing.Perform();TopoDS_Shape result=sewing.SewedShape();
    if(result.IsNull())throw std::runtime_error("mesh sewing returned null topology");
    if(solid){
        BRep_Builder b;TopoDS_Compound compound;b.MakeCompound(compound);TopoDS_Shape only;size_t count=0,face_count=0;
        for(TopExp_Explorer ex(result,TopAbs_SHELL);ex.More();ex.Next()){
            auto shell=TopoDS::Shell(ex.Current());if(!BRep_Tool::IsClosed(shell))throw std::runtime_error("reconstructed shell is open");
            TopoDS_Solid body=BRepBuilderAPI_MakeSolid(shell).Solid();if(!BRepLib::OrientClosedSolid(body)||!BRepCheck_Analyzer(body).IsValid())throw std::runtime_error("reconstructed solid is invalid");
            for(TopExp_Explorer f(body,TopAbs_FACE);f.More();f.Next())++face_count;
            b.Add(compound,body);only=body;++count;
        }
        if(face_count!=indices.size()/3||count==0)throw std::runtime_error("solid reconstruction would lose faces");
        result=count==1?only:compound;
    }else if(result.ShapeType()==TopAbs_FACE){BRep_Builder b;TopoDS_Shell shell;b.MakeShell(shell);b.Add(shell,result);result=shell;}
    if(!BRepCheck_Analyzer(result).IsValid())throw std::runtime_error("reconstructed topology is invalid");
    return std::make_unique<TopoDS_Shape>(result);
} OP_CATCH
}

#include <BRepBuilderAPI_Transform.hxx>
#include <BRepAlgoAPI_BooleanOperation.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <BRepTools_History.hxx>
#include <sstream>
#include "progress.h"
namespace cadrum {
bool NativeProgress::UserBreak() { return operation_cancelled(control); }
void NativeProgress::Show(const Message_ProgressScope&, const bool) { operation_progress(control,GetPosition()); }
}
namespace cadrum {
OperationData op_copy(const TopoDS_Shape& s) try { BRepBuilderAPI_Copy builder(s);return operation_result(builder,{s}); } OP_CATCH
OperationData op_transform(const TopoDS_Shape& s,uint32_t kind,rust::Slice<const double> v) try {
    gp_Trsf t;
    if(kind==0)t.SetTranslation(gp_Vec(v[0],v[1],v[2]));
    else if(kind==1)t.SetRotation(gp_Ax1(gp_Pnt(v[0],v[1],v[2]),gp_Dir(v[3],v[4],v[5])),v[6]);
    else if(kind==2)t.SetScale(gp_Pnt(v[0],v[1],v[2]),v[3]);
    else t.SetMirror(gp_Ax2(gp_Pnt(v[0],v[1],v[2]),gp_Dir(v[3],v[4],v[5])));
    BRepBuilderAPI_Transform builder(s,t,true);return operation_result(builder,{s});
} OP_CATCH
OperationData op_boolean(const std::vector<TopoDS_Shape>& inputs,size_t argument_count,uint32_t operation,const BooleanConfig& config,const OperationControl& control) try {
    if(operation_cancelled(control))throw std::runtime_error("cancelled");
    NCollection_List<TopoDS_Shape> args,tools;for(size_t i=0;i<argument_count;++i)args.Append(inputs[i]);for(size_t i=argument_count;i<inputs.size();++i)tools.Append(inputs[i]);
    BRepAlgoAPI_BooleanOperation builder;builder.SetArguments(args);builder.SetTools(tools);builder.SetOperation(static_cast<BOPAlgo_Operation>(operation));builder.SetNonDestructive(config.non_destructive);builder.SetRunParallel(config.parallel);builder.SetFuzzyValue(config.fuzzy);builder.SetGlue(static_cast<BOPAlgo_GlueEnum>(config.glue));
    Handle(NativeProgress) progress=new NativeProgress(control);builder.Build(progress->Start());
    if(operation_cancelled(control))throw std::runtime_error("cancelled");
    if(builder.HasErrors()){std::ostringstream error;builder.DumpErrors(error);throw std::runtime_error(error.str());}
    auto out=operation_result(builder,inputs);if(builder.HasWarnings()){std::ostringstream warnings;builder.DumpWarnings(warnings);out.warnings.push_back(warnings.str());}
    std::ostringstream effective;effective<<"fuzzy="<<builder.FuzzyValue()<<" glue="<<config.glue<<" non_destructive="<<config.non_destructive<<" parallel="<<config.parallel;out.diagnostics.push_back(effective.str());return out;
} OP_CATCH

OperationData op_blend(const TopoDS_Shape& s,const std::vector<TopoDS_Shape>& edges,double radius,bool chamfer) try {
    NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> members;TopExp::MapShapes(s,TopAbs_EDGE,members);
    for(const auto& edge:edges)if(!members.Contains(edge))throw std::runtime_error("selected edge is not a member of the input shape");
    if(edges.empty())throw std::runtime_error("no selected edges");
    if(chamfer){BRepFilletAPI_MakeChamfer builder(s);for(const auto& edge:edges)builder.Add(radius,TopoDS::Edge(edge));builder.Build();if(!builder.IsDone())throw std::runtime_error("chamfer failed");return operation_result(builder,{s});}
    BRepFilletAPI_MakeFillet builder(s);for(const auto& edge:edges)builder.Add(radius,TopoDS::Edge(edge));builder.Build();if(!builder.IsDone())throw std::runtime_error("fillet failed");return operation_result(builder,{s});
} OP_CATCH
OperationData op_unify(const TopoDS_Shape& s,double linear,double angular) try {
    ShapeUpgrade_UnifySameDomain builder(s,true,true,false);builder.SetLinearTolerance(linear);builder.SetAngularTolerance(angular);builder.Build();const auto& history=builder.History();
    return collect_history(builder.Shape(),{s},[&](const TopoDS_Shape& v){return history->Modified(v);},[&](const TopoDS_Shape& v){return history->Generated(v);},[&](const TopoDS_Shape& v){return history->IsRemoved(v);});
} OP_CATCH
OperationData op_sew_history(const std::vector<TopoDS_Shape>& inputs,double tolerance) try {
    if(inputs.empty()){OperationData out{};out.shape=native_build({},TopAbs_COMPOUND);out.complete=true;return out;}
    BRepBuilderAPI_Sewing sewing(tolerance);for(const auto& s:inputs){if(!native_loose(s)->empty())throw std::runtime_error("unsupported: sewing requires face-based topology");sewing.Add(s);}sewing.Perform();
    auto modified=[&](const TopoDS_Shape& s){NCollection_List<TopoDS_Shape> out;if(sewing.IsModifiedSubShape(s))out.Append(sewing.ModifiedSubShape(s));else if(sewing.IsModified(s))out.Append(sewing.Modified(s));return out;};
    return collect_history(sewing.SewedShape(),inputs,modified,[](const TopoDS_Shape&){return NCollection_List<TopoDS_Shape>();},[](const TopoDS_Shape&){return false;});
} OP_CATCH
std::unique_ptr<TopoDS_Shape> op_topology_at(const TopoDS_Shape& s,uint32_t index) try {NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> map;TopExp::MapShapes(s,map);if(index<1||index>uint32_t(map.Extent()))throw std::runtime_error("topology index out of bounds");return std::make_unique<TopoDS_Shape>(map(int(index)));} OP_CATCH
}

#include <BRepAlgoAPI_Section.hxx>
#include <BOPAlgo_Splitter.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
namespace cadrum {
std::unique_ptr<TopoDS_Shape> op_plane(rust::Slice<const double> v) try {return std::make_unique<TopoDS_Shape>(BRepBuilderAPI_MakeFace(gp_Pln(gp_Pnt(v[0],v[1],v[2]),gp_Dir(v[3],v[4],v[5]))).Face());} OP_CATCH
OperationData op_section(const TopoDS_Shape& a,const TopoDS_Shape& b,double tolerance,rust::Slice<const bool> flags) try {
    BRepAlgoAPI_Section builder(a,b,false);builder.Approximation(flags[0]);builder.ComputePCurveOn1(flags[1]);builder.ComputePCurveOn2(flags[2]);builder.SetFuzzyValue(tolerance);builder.SetNonDestructive(true);builder.Build();
    if(builder.HasErrors()){std::ostringstream error;builder.DumpErrors(error);throw std::runtime_error(error.str());}return operation_result(builder,{a,b});
} OP_CATCH
OperationData op_split(const std::vector<TopoDS_Shape>& inputs,double tolerance) try {
    BOPAlgo_Splitter builder;builder.AddArgument(inputs[0]);for(size_t i=1;i<inputs.size();++i)builder.AddTool(inputs[i]);builder.SetFuzzyValue(tolerance);builder.SetNonDestructive(true);builder.Perform();
    if(builder.HasErrors()){std::ostringstream error;builder.DumpErrors(error);throw std::runtime_error(error.str());}return operation_result(builder,inputs);
} OP_CATCH
std::unique_ptr<std::vector<TopoDS_Shape>> op_children(const TopoDS_Shape& s) try {auto out=native_vec_new();for(TopoDS_Iterator it(s);it.More();it.Next())out->push_back(it.Value());return out;} OP_CATCH
rust::Vec<double> op_centroid(const TopoDS_Shape& s) try {
    GProp_GProps p;
    if(TopExp_Explorer(s,TopAbs_SOLID).More())BRepGProp::VolumeProperties(s,p,1e-9);
    else if(TopExp_Explorer(s,TopAbs_FACE).More())BRepGProp::SurfaceProperties(s,p,1e-9);
    else BRepGProp::LinearProperties(s,p);
    if(std::abs(p.Mass())<1e-30){TopExp_Explorer ex(s,TopAbs_VERTEX);if(!ex.More())return {0.0,0.0,0.0};const auto v=BRep_Tool::Pnt(TopoDS::Vertex(ex.Current()));return {v.X(),v.Y(),v.Z()};}
    const auto v=p.CentreOfMass();return {v.X(),v.Y(),v.Z()};
} OP_CATCH
rust::Vec<uint32_t> op_member_indices(const TopoDS_Shape& parent,const TopoDS_Shape& child) try {
    NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> all,members;TopExp::MapShapes(parent,all);TopExp::MapShapes(child,members);rust::Vec<uint32_t> out;
    for(int i=1;i<=members.Extent();++i){const int index=all.FindIndex(members(i));if(index>0)out.push_back(index);}return out;
} OP_CATCH
OperationData op_surface_offset(const TopoDS_Shape& s,double distance,double tolerance,bool intersection_join,bool intersect_all,bool thicken) try {
    BRepOffset_MakeOffset builder;builder.Initialize(s,distance,tolerance,BRepOffset_Skin,intersect_all,false,intersection_join?GeomAbs_Intersection:GeomAbs_Arc,thicken);builder.MakeOffsetShape();
    if(!builder.IsDone()||builder.Shape().IsNull())throw std::runtime_error("offset failed");
    TopoDS_Shape result=builder.Shape();
    if(thicken){if(result.ShapeType()!=TopAbs_SOLID)throw std::runtime_error("thickening did not produce a solid");auto solid=TopoDS::Solid(result);if(!BRepLib::OrientClosedSolid(solid))throw std::runtime_error("cannot orient thickened solid");result=solid;}
    if(!BRepCheck_Analyzer(result).IsValid())throw std::runtime_error("invalid offset result");
    return collect_history(result,{s},[&](const TopoDS_Shape& v){return builder.Modified(v);},[&](const TopoDS_Shape& v){return builder.Generated(v);},[&](const TopoDS_Shape& v){return builder.IsDeleted(v);});
} OP_CATCH
}

#include <STEPControl_Reader.hxx>
#include <STEPControl_Writer.hxx>
namespace cadrum {
OperationData op_read_step(rust::Slice<const uint8_t> data,const OperationControl& control) try {
    std::istringstream is(std::string(reinterpret_cast<const char*>(data.data()),data.size()));STEPControl_Reader reader;
    if(reader.ReadStream("memory.step",is)!=IFSelect_RetDone)throw std::runtime_error("STEP parse failed");
    if(operation_cancelled(control))throw std::runtime_error("cancelled");
    Handle(NativeProgress) progress=new NativeProgress(control);
    const int roots=reader.NbRootsForTransfer();const int transferred=reader.TransferRoots(progress->Start());
    if(operation_cancelled(control))throw std::runtime_error("cancelled");
    if(reader.OneShape().IsNull())throw std::runtime_error("STEP contains no transferred geometry");
    OperationData out{};out.shape=std::make_unique<TopoDS_Shape>(reader.OneShape());out.complete=false;
    if(transferred<roots)out.warnings.push_back("STEP transfer was partial");
    return out;
} OP_CATCH
rust::Vec<uint8_t> op_write_step(const TopoDS_Shape& s,const OperationControl& control) try {
    if(operation_cancelled(control))throw std::runtime_error("cancelled");
    Handle(NativeProgress) progress=new NativeProgress(control);
    STEPControl_Writer writer;if(writer.Transfer(s,STEPControl_AsIs,true,progress->Start())!=IFSelect_RetDone)throw std::runtime_error("STEP transfer failed");
    if(operation_cancelled(control))throw std::runtime_error("cancelled");
    std::ostringstream os;if(writer.WriteStream(os)!=IFSelect_RetDone)throw std::runtime_error("STEP write failed");
    rust::Vec<uint8_t> out;for(unsigned char c:os.str())out.push_back(c);return out;
} OP_CATCH
}

#include <GCPnts_AbscissaPoint.hxx>
#include <BRepClass3d_SolidClassifier.hxx>
namespace cadrum {
static void append_frame(rust::Vec<double>& out,const gp_Ax2& f){append_xyz(out,f.Location().XYZ());append_xyz(out,f.Direction().XYZ());append_xyz(out,f.XDirection().XYZ());}
GeometryInfoData op_curve_info(const TopoDS_Edge& edge) try {
    BRepAdaptor_Curve c(edge);GeometryInfoData d{};d.kind=c.GetType();d.range={c.FirstParameter(),c.LastParameter()};d.u_periodic=c.IsPeriodic();if(d.u_periodic)d.u_period=c.Period();
    switch(c.GetType()){
    case GeomAbs_Line:append_xyz(d.frame,c.Line().Location().XYZ());append_xyz(d.frame,c.Line().Direction().XYZ());break;
    case GeomAbs_Circle:append_frame(d.frame,c.Circle().Position());d.parameters.push_back(c.Circle().Radius());break;
    case GeomAbs_Ellipse:append_frame(d.frame,c.Ellipse().Position());d.parameters={c.Ellipse().MajorRadius(),c.Ellipse().MinorRadius()};break;
    default:break;}return d;
} OP_CATCH
GeometryInfoData op_surface_info(const TopoDS_Face& face) try {
    BRepAdaptor_Surface s(face);GeometryInfoData d{};d.kind=s.GetType();double a,b,c,e;BRepTools::UVBounds(face,a,b,c,e);d.range={a,b,c,e};d.u_periodic=s.IsUPeriodic();d.v_periodic=s.IsVPeriodic();if(d.u_periodic)d.u_period=s.UPeriod();if(d.v_periodic)d.v_period=s.VPeriod();
    switch(s.GetType()){
    case GeomAbs_Plane:append_frame(d.frame,s.Plane().Position().Ax2());break;
    case GeomAbs_Cylinder:append_frame(d.frame,s.Cylinder().Position().Ax2());d.parameters={s.Cylinder().Radius()};break;
    case GeomAbs_Cone:append_frame(d.frame,s.Cone().Position().Ax2());d.parameters={s.Cone().RefRadius(),s.Cone().SemiAngle()};break;
    case GeomAbs_Sphere:append_frame(d.frame,s.Sphere().Position().Ax2());d.parameters={s.Sphere().Radius()};break;
    case GeomAbs_Torus:append_frame(d.frame,s.Torus().Position().Ax2());d.parameters={s.Torus().MajorRadius(),s.Torus().MinorRadius()};break;
    default:break;}return d;
} OP_CATCH
double op_curve_length(const TopoDS_Edge& edge,double tolerance) try {BRepAdaptor_Curve c(edge);return GCPnts_AbscissaPoint::Length(c,c.FirstParameter(),c.LastParameter(),tolerance);} OP_CATCH
rust::Vec<double> op_curve_abscissae(const TopoDS_Edge& edge,uint32_t count,double tolerance) try {
    BRepAdaptor_Curve c(edge);const double length=GCPnts_AbscissaPoint::Length(c,tolerance);rust::Vec<double> out;out.push_back(c.FirstParameter());
    for(uint32_t i=1;i+1<count;++i){GCPnts_AbscissaPoint point(tolerance,c,length*double(i)/double(count-1),c.FirstParameter());if(!point.IsDone())throw std::runtime_error("arc-length inversion failed");out.push_back(point.Parameter());}out.push_back(c.LastParameter());return out;
} OP_CATCH
uint32_t op_classify_uv(const TopoDS_Face& face,double u,double v,double tolerance) try {BRepClass_FaceClassifier classifier(face,gp_Pnt2d(u,v),tolerance);return classifier.State();} OP_CATCH
std::unique_ptr<std::vector<TopoDS_Shape>> op_boundary_wires(const TopoDS_Face& face) try {
    auto out=native_vec_new();const auto outer=BRepTools::OuterWire(face);if(!outer.IsNull())out->push_back(outer);
    NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> map;TopExp::MapShapes(face,TopAbs_WIRE,map);for(int i=1;i<=map.Extent();++i)if(!map(i).IsSame(outer))out->push_back(map(i));return out;
} OP_CATCH
rust::Vec<double> op_properties(const TopoDS_Shape& s,uint32_t kind,double accuracy) try {
    GProp_GProps p;double error=0.0;
    if(kind==0)BRepGProp::LinearProperties(s,p);
    else if(kind==1)error=BRepGProp::SurfaceProperties(s,p,accuracy);
    else {if(!TopExp_Explorer(s,TopAbs_SOLID).More()&&!native_empty(s))throw std::runtime_error("unsupported: volume requires solid topology");error=BRepGProp::VolumeProperties(s,p,accuracy,true);}
    const auto c=p.CentreOfMass();const auto i=p.MatrixOfInertia();return {p.Mass(),c.X(),c.Y(),c.Z(),i(1,1),i(1,2),i(1,3),i(2,1),i(2,2),i(2,3),i(3,1),i(3,2),i(3,3),error};
} OP_CATCH
uint32_t op_classify_point(const TopoDS_Shape& s,double x,double y,double z,double tolerance) try {
    bool found=false,boundary=false;for(TopExp_Explorer ex(s,TopAbs_SOLID);ex.More();ex.Next()){found=true;BRepClass3d_SolidClassifier classifier(ex.Current(),gp_Pnt(x,y,z),tolerance);if(classifier.State()==TopAbs_IN)return TopAbs_IN;if(classifier.State()==TopAbs_ON)boundary=true;}
    if(!found)throw std::runtime_error("unsupported: point containment requires solids");
    return boundary?TopAbs_ON:TopAbs_OUT;
} OP_CATCH
DistanceData op_distance(const TopoDS_Shape& a,const TopoDS_Shape& b,double tolerance) try {
    if(native_empty(a)||native_empty(b))throw std::runtime_error("unsupported: distance to empty topology");
    BRepExtrema_DistShapeShape distance(a,b,tolerance);if(!distance.IsDone())throw std::runtime_error("closest-point search failed");
    DistanceData out{};out.distance=distance.Value();out.inner_solution=distance.InnerSolution();out.first=native_vec_new();out.second=native_vec_new();
    for(int i=1;i<=distance.NbSolution();++i){append_xyz(out.points,distance.PointOnShape1(i).XYZ());append_xyz(out.points,distance.PointOnShape2(i).XYZ());out.first->push_back(distance.SupportOnShape1(i));out.second->push_back(distance.SupportOnShape2(i));}return out;
} OP_CATCH
std::unique_ptr<std::vector<TopoDS_Shape>> op_ancestors(const TopoDS_Shape& parent,const TopoDS_Shape& subshape,uint32_t kind) try {
    NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> all;TopExp::MapShapes(parent,all);if(!all.Contains(subshape))throw std::runtime_error("foreign subshape in adjacency query");
    NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> ancestors;TopExp::MapShapes(parent,static_cast<TopAbs_ShapeEnum>(kind),ancestors);auto out=native_vec_new();
    for(int i=1;i<=ancestors.Extent();++i){NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> members;TopExp::MapShapes(ancestors(i),members);if(members.Contains(subshape))out->push_back(ancestors(i));}return out;
} OP_CATCH
}

#include <BRepOffsetAPI_MakePipeShell.hxx>
#include <Law_Interpol.hxx>
namespace cadrum {
SweepData op_sweep(const std::vector<TopoDS_Shape>& inputs,size_t section_count,const SweepConfig& config,const OperationControl& control) try {
    if(operation_cancelled(control))throw std::runtime_error("cancelled");
    const auto spine=TopoDS::Wire(inputs[0]);BRepOffsetAPI_MakePipeShell builder(spine);builder.SetIsBuildHistory(config.law.empty());
    builder.SetTransitionMode(static_cast<BRepBuilderAPI_TransitionMode>(config.transition));builder.SetTolerance(config.tolerance,config.boundary_tolerance,config.angular_tolerance);builder.SetForceApproxC1(config.force_c1);builder.SetMaxDegree(config.max_degree);builder.SetMaxSegments(config.max_segments);
    switch(config.mode){
    case 0:builder.SetMode(false);break;case 1:builder.SetMode(true);break;
    case 2:builder.SetMode(gp_Ax2(gp_Pnt(config.frame[0],config.frame[1],config.frame[2]),gp_Dir(config.frame[3],config.frame[4],config.frame[5]),gp_Dir(config.frame[6],config.frame[7],config.frame[8])));break;
    case 3:builder.SetMode(gp_Dir(config.frame[0],config.frame[1],config.frame[2]));break;
    case 4:builder.SetDiscreteMode();break;
    case 5:if(!builder.SetMode(inputs[config.guide]))throw std::runtime_error("unsupported: spine lacks pcurves on the selected support");break;
    case 6:builder.SetMode(TopoDS::Wire(inputs[config.guide]),config.curvilinear,static_cast<BRepFill_TypeOfContact>(config.auxiliary_contact));break;
    }
    NCollection_IndexedMap<TopoDS_Shape,TopTools_ShapeMapHasher> vertices;TopExp::MapShapes(spine,TopAbs_VERTEX,vertices);
    for(size_t i=0;i<section_count;++i){
        const auto& profile=inputs[i+1];const int location=config.locations[i];if(location>=0&&!vertices.Contains(inputs[location]))throw std::runtime_error("sweep: explicit section location is not a spine vertex");
        if(!config.law.empty()){
            NCollection_Array1<gp_Pnt2d> samples(1,int(config.law.size()/2));for(int j=1;j<=samples.Length();++j)samples(j)=gp_Pnt2d(config.law[2*(j-1)],config.law[2*(j-1)+1]);Handle(Law_Interpol) law=new Law_Interpol;law->Set(samples,spine.Closed());
            if(location>=0)builder.SetLaw(profile,law,TopoDS::Vertex(inputs[location]),config.contact[i],config.correction[i]);else builder.SetLaw(profile,law,config.contact[i],config.correction[i]);
        }else if(location>=0)builder.Add(profile,TopoDS::Vertex(inputs[location]),config.contact[i],config.correction[i]);else builder.Add(profile,config.contact[i],config.correction[i]);
    }
    if(!builder.IsReady())throw std::runtime_error("sweep: pipe is not ready");
    SweepData out{};out.sections=native_vec_new();if(config.simulation>0){NCollection_List<TopoDS_Shape> result;builder.Simulate(config.simulation,result);for(const auto& shape:result)out.sections->push_back(shape);}
    Handle(NativeProgress) progress=new NativeProgress(control);builder.Build(progress->Start());if(operation_cancelled(control))throw std::runtime_error("cancelled");
    if(!builder.IsDone())throw std::runtime_error("sweep: pipe status "+std::to_string(int(builder.GetStatus())));
    if(config.solid&&!builder.MakeSolid())throw std::runtime_error("sweep: open profile cannot produce a solid");
    if(!BRepCheck_Analyzer(builder.Shape()).IsValid())throw std::runtime_error("sweep: invalid result");
    if(config.law.empty())out.operation=operation_result(builder,inputs);else{out.operation.shape=std::make_unique<TopoDS_Shape>(builder.Shape());out.operation.complete=false;out.operation.warnings.push_back("OCCT scaling-law history is unavailable; native history building is disabled to avoid its kernel crash");}
    out.status=builder.GetStatus();out.error=builder.ErrorOnSurface();return out;
} OP_CATCH
}

#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepAlgoAPI_Cut.hxx>
namespace cadrum {
static std::unique_ptr<BRepOffsetAPI_ThruSections> loft_piece(const std::vector<TopoDS_Shape>& sections,const LoftConfig& config,const OperationControl& control){
    auto builder=std::make_unique<BRepOffsetAPI_ThruSections>(config.solid,config.ruled,config.tolerance);builder->CheckCompatibility(config.compatibility);builder->SetMutableInput(false);builder->SetSmoothing(config.smoothing);builder->SetParType(static_cast<Approx_ParametrizationType>(config.parametrization));builder->SetContinuity(static_cast<GeomAbs_Shape>(config.continuity));builder->SetMaxDegree(config.max_degree);builder->SetCriteriumWeight(config.weights[0],config.weights[1],config.weights[2]);
    bool has_wire=false;for(const auto& s:sections){if(s.ShapeType()==TopAbs_VERTEX)builder->AddVertex(TopoDS::Vertex(s));else{has_wire=true;if(config.solid&&!BRep_Tool::IsClosed(s))throw std::runtime_error("solid loft requires closed wires");builder->AddWire(TopoDS::Wire(s));}}
    if(!has_wire)throw std::runtime_error("loft requires at least one wire section");
    Handle(NativeProgress) progress=new NativeProgress(control);builder->Build(progress->Start());if(operation_cancelled(control))throw std::runtime_error("cancelled");
    if(!builder->IsDone()||builder->Shape().IsNull())throw std::runtime_error("loft construction failed");
    return builder;
}
OperationData op_loft(const std::vector<TopoDS_Shape>& sections,const LoftConfig& config,const OperationControl& control) try {
    bool simple=true;for(const auto& s:sections)if(s.ShapeType()!=TopAbs_WIRE&&s.ShapeType()!=TopAbs_VERTEX)simple=false;
    if(simple){auto builder=loft_piece(sections,config,control);if(!BRepCheck_Analyzer(builder->Shape()).IsValid())throw std::runtime_error("invalid loft result");return operation_result(*builder,sections);}
    std::vector<std::vector<TopoDS_Face>> profiles;
    for(const auto& s:sections){require_profile(s);std::vector<TopoDS_Face> faces;for(TopExp_Explorer ex(s,TopAbs_FACE);ex.More();ex.Next())faces.push_back(TopoDS::Face(ex.Current()));if(faces.empty()||(!profiles.empty()&&faces.size()!=profiles[0].size()))throw std::runtime_error("loft profiles have incompatible region counts");profiles.push_back(faces);}
    BRep_Builder b;TopoDS_Compound compound;b.MakeCompound(compound);TopoDS_Shape only;size_t count=0;
    for(size_t region=0;region<profiles[0].size();++region){
        std::vector<std::vector<TopoDS_Shape>> loops;
        for(const auto& profile:profiles){auto wires=op_boundary_wires(profile[region]);if(!loops.empty()&&wires->size()!=loops[0].size())throw std::runtime_error("loft profiles have incompatible hole counts");loops.push_back(*wires);}
        TopoDS_Shape body;
        for(size_t loop=0;loop<loops[0].size();++loop){std::vector<TopoDS_Shape> wires;for(const auto& section:loops)wires.push_back(section[loop]);auto builder=loft_piece(wires,config,control);TopoDS_Shape result=builder->Shape();
            if(config.solid){auto solid=TopoDS::Solid(result);if(!BRepLib::OrientClosedSolid(solid))throw std::runtime_error("cannot orient profile loft");result=solid;if(loop==0)body=result;else{BRepAlgoAPI_Cut cut(body,result);if(!cut.IsDone()||cut.HasErrors())throw std::runtime_error("hole loft subtraction failed");body=cut.Shape();}}
            else{if(loop>0)result.Reverse();b.Add(compound,result);only=result;++count;}
        }
        if(config.solid){b.Add(compound,body);only=body;++count;}
    }
    const TopoDS_Shape result=count==1?only:compound;if(!BRepCheck_Analyzer(result).IsValid())throw std::runtime_error("invalid composite loft");OperationData out{};out.shape=std::make_unique<TopoDS_Shape>(result);out.complete=false;out.warnings.push_back("composite profile loft history is incomplete");return out;
} OP_CATCH
}
