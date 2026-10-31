#include "cadrum/src/ffi.rs.h"
#include <BRepCheck_Analyzer.hxx>
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
RepairData op_repair(const TopoDS_Shape& shape, double precision, double max_tolerance, rust::Slice<const bool> flags) try {
    if (flags.size() != 5) throw std::runtime_error("repair: invalid flags");
    const TopoDS_Shape copy = BRepBuilderAPI_Copy(shape).Shape();
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
    out.shape = std::make_unique<TopoDS_Shape>(result);
    return out;
} OP_CATCH
}

#include <BRepBuilderAPI_GTransform.hxx>
#include <gp_GTrsf.hxx>
#include <gp_Mat.hxx>
namespace cadrum {
template<class Builder>
static OperationData operation_result(Builder& builder, const std::vector<TopoDS_Shape>& inputs) {
    const TopoDS_Shape result = builder.Shape();
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
            for (const auto& modified : builder.Modified(source)) add(modified, 0);
            for (const auto& generated : builder.Generated(source)) add(generated, 1);
            if (targets.Contains(source)) add(source, 3);
            if (!found) {
                const bool removed = builder.IsDeleted(source);
                out.history.push_back(HistoryEntry{uint32_t(operand), uint32_t(i), 0, uint32_t(removed ? 2 : 4), uint32_t(source.ShapeType()), 0, reinterpret_cast<uint64_t>(source.TShape().get()), 0});
                if (!removed) out.complete = false;
            }
        }
    }
    return out;
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
