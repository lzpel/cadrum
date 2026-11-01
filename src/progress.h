#pragma once
#include <Message_ProgressIndicator.hxx>
namespace cadrum {
struct OperationControl;
class NativeProgress final : public Message_ProgressIndicator {
    const OperationControl& control;

  public:
    explicit NativeProgress(const OperationControl& value) : control(value) {
    }
    bool UserBreak() override;
    void Show(const Message_ProgressScope&, const bool) override;
};
}
