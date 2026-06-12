# stem: filename up to the first dot. e.g. $(call stem,.../libcxx-18.1.8.src.tar.xz) -> libcxx-18
stem = $(firstword $(subst ., ,$(notdir $(1))))
CMAKE = https://github.com/Kitware/CMake/releases/download/v3.31.12/cmake-3.31.12-windows-x86_64.zip
CLANG = https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/clang+llvm-18.1.8-x86_64-pc-windows-msvc.tar.xz
LLVM = https://github.com/llvm/llvm-project/archive/refs/tags/llvmorg-18.1.8.tar.gz
WASI = https://github.com/WebAssembly/wasi-libc/archive/refs/tags/wasi-sdk-32.tar.gz
# LLVM runtimes (libc++) build hard-requires a real Python3; portable embeddable build avoids a system install.
PYTHON = https://www.python.org/ftp/python/3.12.7/python-3.12.7-embed-amd64.zip
export PATH := $(CURDIR)/../out/$(call stem,$(CLANG))/bin:$(CURDIR)/../out/python:$(CURDIR)/../out/$(CMAKE)/bin:$(PATH)

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
	cd ../out && ( ls wasi-sysroot || ( cmake -G "Unix Makefiles" -S $(call stem,$(WASI)) -B $(call stem,$(WASI))/build -DCMAKE_C_COMPILER=clang -DCMAKE_AR=llvm-ar -DCMAKE_RANLIB=llvm-ranlib -DCMAKE_NM=llvm-nm -DCMAKE_C_COMPILER_WORKS=1 -DCMAKE_INSTALL_PREFIX=$(CURDIR)/../out/wasi-sysroot && cmake --build $(call stem,$(WASI))/build && cmake --install $(call stem,$(WASI))/build ) )
	# build libc++ / libc++abi for wasm32-wasip1 into the same wasi-sysroot
	ls $(CURDIR)/../out/wasi-sysroot/lib/wasm32-wasip1/libc++.a || ( cmake -G "Unix Makefiles" \
		-S ../out/$(call stem,$(LLVM))/runtimes -B ../out/$(call stem,$(LLVM))/build-libcxx \
		-DCMAKE_C_COMPILER=$(CURDIR)/../out/$(call stem,$(CLANG))/bin/clang.exe -DCMAKE_CXX_COMPILER=$(CURDIR)/../out/$(call stem,$(CLANG))/bin/clang++.exe \
		-DCMAKE_AR=$(CURDIR)/../out/$(call stem,$(CLANG))/bin/llvm-ar.exe -DCMAKE_RANLIB=$(CURDIR)/../out/$(call stem,$(CLANG))/bin/llvm-ranlib.exe -DCMAKE_NM=$(CURDIR)/../out/$(call stem,$(CLANG))/bin/llvm-nm.exe \
		-DCMAKE_C_COMPILER_WORKS=1 -DCMAKE_CXX_COMPILER_WORKS=1 \
		-DCMAKE_SYSTEM_NAME=Generic -DCMAKE_SYSTEM_PROCESSOR=wasm32 \
		-DCMAKE_C_COMPILER_TARGET=wasm32-wasip1 -DCMAKE_CXX_COMPILER_TARGET=wasm32-wasip1 \
		-DCMAKE_SYSROOT=$(CURDIR)/../out/wasi-sysroot -DCMAKE_INSTALL_PREFIX=$(CURDIR)/../out/wasi-sysroot \
		-DLLVM_ENABLE_RUNTIMES="libcxx;libcxxabi" \
		-DLIBCXX_ENABLE_SHARED=OFF -DLIBCXXABI_ENABLE_SHARED=OFF \
		-DLIBCXX_ENABLE_THREADS=OFF -DLIBCXXABI_ENABLE_THREADS=OFF \
		-DLIBCXX_ENABLE_FILESYSTEM=OFF \
		-DLIBCXX_CXX_ABI=libcxxabi -DLIBCXXABI_USE_LLVM_UNWINDER=OFF \
		-DLIBCXX_INCLUDE_TESTS=OFF -DLIBCXX_INCLUDE_BENCHMARKS=OFF \
		-DLIBCXX_INSTALL_LIBRARY_DIR=lib/wasm32-wasip1 -DLIBCXXABI_INSTALL_LIBRARY_DIR=lib/wasm32-wasip1 \
		-DLIBCXX_INSTALL_INCLUDE_DIR=include/c++/v1 -DLIBCXX_INSTALL_INCLUDE_TARGET_DIR=include/c++/v1 \
		&& cmake --build ../out/$(call stem,$(LLVM))/build-libcxx \
		&& cmake --install ../out/$(call stem,$(LLVM))/build-libcxx )
download: # download llvm for clang++ and wasi-sdk for libc.
	cargo install --root ../out wasm-pack
	cd ../out && ( ls $(notdir $(CMAKE)) || ( curl -L -O $(CMAKE) && unzip -q $(notdir $(CMAKE)) ) )
	cd ../out && ( ls $(notdir $(CLANG)) || ( curl -L -O $(CLANG) && tar xf $(notdir $(CLANG)) --transform="s,^[^/]+,$(call stem,$(CLANG)),x" ) )
	cd ../out && ( ls $(notdir $(LLVM)) || ( curl -L -O $(LLVM) && tar xf $(notdir $(LLVM)) --transform="s,^[^/]+,$(call stem,$(LLVM)),xSH" --exclude='*/clang/test/*' ) ) # avoid pathes like ../../opt/cuda*
	cd ../out && ( ls $(notdir $(WASI)) || ( curl -L -O $(WASI) && tar xf $(notdir $(WASI)) --transform="s,^[^/]+,$(call stem,$(WASI)),x" ) )
	cd ../out && ( ls python/python.exe || ( curl -L -O $(PYTHON) && unzip -q $(notdir $(PYTHON)) -d python ) )
	cmake --version
	python --version