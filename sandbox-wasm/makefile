# stem: filename up to the first dot. e.g. $(call stem,.../libcxx-18.1.8.src.tar.xz) -> libcxx-18
stem = $(firstword $(subst ., ,$(notdir $(1))))
# wasi-sdk Windows SDK 1 個で完結: bin/clang.exe 等のツールと、prebuilt sysroot
# (wasm 互換の libc / libc++) を share/wasi-sysroot/ に同梱している。
WASI_SDK = https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-33/wasi-sdk-33.0-x86_64-windows.tar.gz
export PATH := $(CURDIR)/../out/$(call stem,$(WASI_SDK))/bin:$(PATH)
# cxx crate は cxx.cc を自前の cc::Build でコンパイルするので、libc++ ヘッダ(-isystem)と
# 例外無効フラグを cc crate が読む CXXFLAGS_<target> 経由で全 C++ TU に届ける。
# MSYS/Git-bash は env 値中の 2 つ目の `C:` をパスリスト区切りと誤認し `C;` に化けさせる
# ので、MSYS_NO_PATHCONV=1 でパス変換を無効化する。
export MSYS_NO_PATHCONV := 1
# cxx が依存する link-cplusplus は wasm では既定で libstdc++ をリンクしようとする。
# 我々は libc++ を使うので CXXSTDLIB=c++ に上書きする。
export CXXSTDLIB := c++
SYSROOT := $(CURDIR)/../out/$(call stem,$(WASI_SDK))/share/wasi-sysroot
export CXXFLAGS_wasm32_unknown_unknown := -isystem$(SYSROOT)/include/wasm32-wasip1/noeh/c++/v1 -isystem$(SYSROOT)/include/wasm32-wasip1 -fno-exceptions -fno-rtti -DRUST_CXX_NO_EXCEPTIONS -D_LIBCPP_PROVIDES_DEFAULT_RUNE_TABLE

# pure rust. I got wasm successfully.
run: run-raw-pure
# with simple cpp. such as "double add(double a, double b) { return a + b; }".
run-cc: run-raw-cc
# cc + libc (wasi-libc). __has_include(<math.h>) becomes true so add() returns sin(a+b).
run-cc-libc: run-raw-cc-libc
run-cxx: run-raw-cxx-libcxx
# with cadrum. This is that target of this sandbox.
run-cadrum: run-raw-cadrum
run-raw-%: # build wasm, and run it with node. hyphens in % become comma-separated features (e.g. run-raw-cc-libc -> --features cc,libc).
	cargo build --target wasm32-unknown-unknown --features "$(subst -, ,$*)"
	../out/bin/wasm-pack build . -d ./target --features "$(subst -, ,$*)"
	node -e "import('./target/wasm_experiment.js').then(m=>console.log(m.print_volume()))"
# prebuilt sysroot を同梱する SDK を落とすだけ。from-source ビルドは不要。
generate: download
download: # download wasi-sdk (clang + prebuilt sysroot with libc / libc++).
	cargo install --root ../out wasm-pack
	cd ../out && ( ls $(call stem,$(WASI_SDK))/bin/clang.exe || ( curl -L -O $(WASI_SDK) && tar xf $(notdir $(WASI_SDK)) --transform="s,^[^/]+,$(call stem,$(WASI_SDK)),x" ) )