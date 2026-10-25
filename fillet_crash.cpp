// A square with its left side bowed out into an arc, extruded 5 along Z, then a 2.5
// fillet on two edges that share no vertex: the bottom right edge and the arc's top edge.
// Each edge alone fillets fine; both together crash inside ChFi3d_StripeEdgeInter.
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <iostream>

// The edge of `shape` nearest to `p`.
static TopoDS_Edge nearest_edge(const TopoDS_Shape& shape, const gp_Pnt& p)
{
    TopoDS_Edge best;
    double best_d = 1e100;
    const TopoDS_Shape v = BRepBuilderAPI_MakeVertex(p).Shape();
    for (TopExp_Explorer ex(shape, TopAbs_EDGE); ex.More(); ex.Next()) {
        BRepExtrema_DistShapeShape dist(v, ex.Current());
        if (dist.IsDone() && dist.Value() < best_d) {
            best_d = dist.Value();
            best = TopoDS::Edge(ex.Current());
        }
    }
    return best;
}

int main()
{
    const gp_Pnt p0(0, 0, 0), p1(22.5, 0, 0), p2(22.5, 22.5, 0), p3(0, 22.5, 0);
    BRepBuilderAPI_MakeWire wire;
    wire.Add(BRepBuilderAPI_MakeEdge(p0, p1).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(p1, p2).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(p2, p3).Edge());
    wire.Add(BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(p3, gp_Pnt(-7.5, 11.25, 0), p0).Value()).Edge());
    const TopoDS_Face face = BRepBuilderAPI_MakeFace(wire.Wire()).Face();  // image 1: the profile face
    const TopoDS_Shape solid = BRepPrimAPI_MakePrism(face, gp_Vec(0, 0, 5)).Shape();  // image 2: the prism

    BRepFilletAPI_MakeFillet fillet(solid);
    fillet.Add(2.5, nearest_edge(solid, gp_Pnt(22.5, 11.25, 0)));  // image 2: bottom right edge (red)
    fillet.Add(2.5, nearest_edge(solid, gp_Pnt(-7.5, 11.25, 5)));  // image 2: arc's top edge (red)
    std::cout << "contours: " << fillet.NbContours() << std::endl;
    std::cout << "building..." << std::endl;
    fillet.Build();  // access violation in ChFi3d_StripeEdgeInter; image 3 is the result expected here
    std::cout << "done: " << fillet.IsDone() << std::endl;
    return 0;
}
