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
#include <BRepBuilderAPI_Copy.hxx>
#include <NCollection_List.hxx>
#include "progress.h"

#define OP_CATCH                                                                                             \
    catch (const Standard_Failure& e) {                                                                      \
        throw std::runtime_error(e.what());                                                                  \
    }

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
        if (result.IsNull()) {
            continue;
        }
        for (const auto status : result->Status()) {
            if (status != BRepCheck_NoError) {
                out.issues.push_back(
                    ValidationEntry{uint32_t(i), 0, uint32_t(s.ShapeType()), uint32_t(status)});
            }
        }
        for (result->InitContextIterator(); result->MoreShapeInContext(); result->NextShapeInContext()) {
            const int context = map.FindIndex(result->ContextualShape());
            for (const auto status : result->StatusOnShape()) {
                if (status != BRepCheck_NoError) {
                    out.issues.push_back(ValidationEntry{uint32_t(i), uint32_t(context),
                                                         uint32_t(s.ShapeType()), uint32_t(status)});
                }
            }
        }
    }
    return out;
}
OP_CATCH

template <class Modified, class Generated, class Deleted>
static OperationData collect_history(const TopoDS_Shape& result, const std::vector<TopoDS_Shape>& inputs,
                                     Modified modified_shapes, Generated generated_shapes,
                                     Deleted is_deleted) {
    if (result.IsNull()) {
        throw std::runtime_error("kernel produced null topology");
    }
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
            if (source.ShapeType() != TopAbs_VERTEX && source.ShapeType() != TopAbs_EDGE &&
                source.ShapeType() != TopAbs_FACE) {
                continue;
            }
            bool found = false;
            auto add = [&](const TopoDS_Shape& target, uint32_t relation) {
                const int index = targets.FindIndex(target);
                if (index == 0) {
                    return;
                }
                out.history.push_back(HistoryEntry{uint32_t(operand), uint32_t(i), uint32_t(index), relation,
                                                   uint32_t(source.ShapeType()), uint32_t(target.ShapeType()),
                                                   reinterpret_cast<uint64_t>(source.TShape().get()),
                                                   reinterpret_cast<uint64_t>(target.TShape().get())});
                found = true;
            };
            for (const auto& modified : modified_shapes(source)) {
                add(modified, 0);
            }
            for (const auto& generated : generated_shapes(source)) {
                add(generated, 1);
            }
            if (targets.Contains(source)) {
                add(source, 3);
            }
            if (!found) {
                const bool removed = is_deleted(source);
                out.history.push_back(HistoryEntry{uint32_t(operand), uint32_t(i), 0,
                                                   uint32_t(removed ? 2 : 4), uint32_t(source.ShapeType()), 0,
                                                   reinterpret_cast<uint64_t>(source.TShape().get()), 0});
                if (!removed) {
                    out.complete = false;
                }
            }
        }
    }
    return out;
}
template <class Builder>
static OperationData operation_result(Builder& builder, const std::vector<TopoDS_Shape>& inputs) {
    return collect_history(
        builder.Shape(), inputs, [&](const TopoDS_Shape& s) { return builder.Modified(s); },
        [&](const TopoDS_Shape& s) { return builder.Generated(s); },
        [&](const TopoDS_Shape& s) { return builder.IsDeleted(s); });
}

bool NativeProgress::UserBreak() {
    return operation_cancelled(control);
}
void NativeProgress::Show(const Message_ProgressScope&, const bool) {
    operation_progress(control, GetPosition());
}
OperationData op_copy(const TopoDS_Shape& s) try {
    BRepBuilderAPI_Copy builder(s);
    return operation_result(builder, {s});
}
OP_CATCH
std::unique_ptr<TopoDS_Shape> op_topology_at(const TopoDS_Shape& s, uint32_t index) try {
    NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher> map;
    TopExp::MapShapes(s, map);
    if (index < 1 || index > uint32_t(map.Extent())) {
        throw std::runtime_error("topology index out of bounds");
    }
    return std::make_unique<TopoDS_Shape>(map(int(index)));
}
OP_CATCH
}
