LLVM = clang+llvm-18.1.8-x86_64-pc-windows-msvc
WASISDK = wasi-sdk-32
CMAKE = cmake-3.31.12
comma := ,
export PATH := $(CURDIR)/../out/$(LLVM)/bin:$(CURDIR)/../out/$(CMAKE)/bin:$(PATH)

# pure rust. I got wasm successfully.
run: run-raw-pure
# with simple cpp. such as "double add(double a, double b) { return a + b; }".
run-cc: run-raw-cc
# cc + libc (wasi-libc). __has_include(<math.h>) becomes true so add() returns sin(a+b).
run-cc-libc: run-raw-cc-libc
run-cxx: run-raw-cxx
# with cadrum. This is that target of this sandbox.
run-cadrum: run-raw-cadrum
run-raw-%: download # build wasm, and run it with node. hyphens in % become comma-separated features (e.g. run-raw-cc-libc -> --features cc,libc).
	cargo build --target wasm32-unknown-unknown --features $(subst -,$(comma),$*)
	../out/bin/wasm-pack build . -d ./target --features $(subst -,$(comma),$*)
	node -e "import('./target/wasm_experiment.js').then(m=>console.log(m.print_volume()))"
generate: download
	# build out/wasi-sdk-32
	ls ../out/wasi-sysroot || cmake -G "Unix Makefiles" -S ../out/$(WASISDK) -B ../out/$(WASISDK)/build -DCMAKE_C_COMPILER=clang -DCMAKE_AR=llvm-ar -DCMAKE_RANLIB=llvm-ranlib -DCMAKE_NM=llvm-nm -DCMAKE_INSTALL_PREFIX=$(CURDIR)/../out/wasi-sysroot && cmake --build ../out/$(WASISDK)/build && cmake --install ../out/$(WASISDK)/build
download: download-cmake # download llvm for clang++ and wasi-sdk for libc.
	cargo install --root ../out wasm-pack
	ls ../out/$(LLVM) || ( cd ../out && curl -L -O https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/$(LLVM).tar.xz && tar xf $(LLVM).tar.xz )
	ls ../out/$(WASISDK) || ( cd ../out && curl -L -O https://github.com/WebAssembly/wasi-libc/archive/refs/tags/$(WASISDK).tar.gz && tar xf $(WASISDK).tar.gz --transform="s,^[^/]+,$(WASISDK),x" )
ifeq ($(OS),Windows_NT)
  CMAKE_SRC = $(CMAKE)-windows-x86_64
  CMAKE_ARCHIVE = $(CMAKE_SRC).zip
  CMAKE_EXTRACT = unzip -q $(CMAKE_ARCHIVE)
else
  CMAKE_SRC = $(CMAKE)-linux-x86_64
  CMAKE_ARCHIVE = $(CMAKE_SRC).tar.gz
  CMAKE_EXTRACT = tar xf $(CMAKE_ARCHIVE)
endif
download-cmake: # download appropriate cmake binary. wasi-libc requires cmake 3.31 or later ( occt 8.0.0 requires cmake 3.31 or lower, so 3.31 is good choice ). 
	ls ../out/$(CMAKE) || ( cd ../out && curl -L -O https://github.com/Kitware/CMake/releases/download/v3.31.12/$(CMAKE_ARCHIVE) && $(CMAKE_EXTRACT) && mv $(CMAKE_SRC) $(CMAKE) )
	cmake --version