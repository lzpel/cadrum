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
    ValidationData out;
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
    RepairData out;
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
    OperationData out;
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
