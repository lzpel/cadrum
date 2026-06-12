# stem: filename up to the first dot. e.g. $(call stem,.../libcxx-18.1.8.src.tar.xz) -> libcxx-18
stem = $(firstword $(subst ., ,$(notdir $(1))))
CMAKE = https://github.com/Kitware/CMake/releases/download/v3.31.12/cmake-3.31.12-windows-x86_64.zip
CLANG = https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/clang+llvm-18.1.8-x86_64-pc-windows-msvc.tar.xz
LIBCXX = https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/libcxx-18.1.8.src.tar.xz
WASISDK = https://github.com/WebAssembly/wasi-libc/archive/refs/tags/wasi-sdk-32.tar.gz
export PATH := $(CURDIR)/../out/$(call stem,$(CLANG))/bin:$(CURDIR)/../out/$(CMAKE)/bin:$(PATH)

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
	cargo build --target wasm32-unknown-unknown --features "$(subst -, ,$*)"
	../out/bin/wasm-pack build . -d ./target --features "$(subst -, ,$*)"
	node -e "import('./target/wasm_experiment.js').then(m=>console.log(m.print_volume()))"
generate: download
	# build out/wasi-sdk-32
	ls ../out/wasi-sysroot || cmake -G "Unix Makefiles" -S ../out/$(call stem,$(WASISDK)) -B ../out/$(call stem,$(WASISDK))/build -DCMAKE_C_COMPILER=clang -DCMAKE_AR=llvm-ar -DCMAKE_RANLIB=llvm-ranlib -DCMAKE_NM=llvm-nm -DCMAKE_C_COMPILER_WORKS=1 -DCMAKE_INSTALL_PREFIX=$(CURDIR)/../out/wasi-sysroot && cmake --build ../out/$(call stem,$(WASISDK))/build && cmake --install ../out/$(call stem,$(WASISDK))/build
download: # download llvm for clang++ and wasi-sdk for libc.
	cargo install --root ../out wasm-pack
	cd ../out && ( ls $(notdir $(CMAKE)) || ( curl -L -O $(CMAKE) && unzip -q $(notdir $(CMAKE)) ) )
	cd ../out && ( ls $(notdir $(CLANG)) || ( curl -L -O $(CLANG) && tar xf $(notdir $(CLANG)) --transform="s,^[^/]+,$(call stem,$(CLANG)),x" ) )
	cd ../out && ( ls $(notdir $(LIBCXX)) || ( curl -L -O $(LIBCXX) && tar xf $(notdir $(LIBCXX)) --transform="s,^[^/]+,$(call stem,$(LIBCXX)),x" ) )
	cd ../out && ( ls $(notdir $(WASISDK)) || ( curl -L -O $(WASISDK) && tar xf $(notdir $(WASISDK)) --transform="s,^[^/]+,$(call stem,$(WASISDK)),x" ) )
	cmake --version