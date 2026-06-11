LLVM = clang+llvm-18.1.8-x86_64-pc-windows-msvc
export PATH := $(CURDIR)/../out/$(LLVM)/bin:$(PATH)

# pure rust. I got wasm successfully.
run: run-raw-pure
# with simple cpp. such as "double add(double a, double b) { return a + b; }".
run-cc: run-raw-cc
run-cxx: run-raw-cxx
# with cadrum. This is that target of this sandbox.
run-cadrum: run-raw-cadrum
run-raw-%: generate
	cargo build --target wasm32-unknown-unknown --features $*
	../out/bin/wasm-pack build . -d ../out --features $*
generate:
	cargo install --root ../out wasm-pack
	ls ../out/$(LLVM) || ( cd ../out && curl -L -O https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/$(LLVM).tar.xz && tar xf $(LLVM).tar.xz )
test:
	clang++ --version
	