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
# C/C++ のコンパイルだけ --target=wasm32-wasip1 にすると、clang が --sysroot から
# include/wasm32-wasip1 と（-fno-exceptions 時は）noeh/c++/v1 を自動解決するので -isystem 不要。
# __wasi__ が定義され libc++ の __locale も正規の rune table を選ぶため rune table hack も不要。
# 最終リンクは rustc が wasm32-unknown-unknown で行う（オブジェクトは wasm32 で互換）。
export CXXFLAGS_wasm32_unknown_unknown := --target=wasm32-wasip1 --sysroot=$(SYSROOT) -fno-exceptions -fno-rtti -DRUST_CXX_NO_EXCEPTIONS
export CFLAGS_wasm32_unknown_unknown := --target=wasm32-wasip1 --sysroot=$(SYSROOT)
# libc.a のリンクは rustc(rust-lld)側。静的アーカイブは遅延リンクなので、libc を参照しない
# pure ビルドには取り込まれない。これで build.rs から sysroot/libc 記述を排除できる。
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS := -L native=$(SYSROOT)/lib/wasm32-wasip1 -l static=c

run: run-pure run-cc run-cxx
run-pure: run-raw-pure
# cc + libc (wasi-sysroot). __has_include(<math.h>) is true so add() returns sin(a+b).
run-cc: run-raw-cc
run-cxx: run-raw-cxx-libcxx
# with cadrum. This is that target of this sandbox.
run-cadrum: run-raw-cadrum
run-raw-%: # build wasm, and run it with node. hyphens in % become comma-separated features (e.g. run-raw-cc-libc -> --features cc,libc).
	cargo build --target wasm32-unknown-unknown --features "$(subst -, ,$*)"
	../out/bin/wasm-pack build . -d ./target --features "$(subst -, ,$*)"
	node -e "import('./target/wasm_experiment.js').then(m=>console.log(m.print_volume()))"
download: # download wasi-sdk (clang + prebuilt sysroot with libc / libc++).
	cargo install --root ../out wasm-pack
	cd ../out && ( ls $(call stem,$(WASI_SDK))/bin/clang.exe || ( curl -L -O $(WASI_SDK) && tar xf $(notdir $(WASI_SDK)) --transform="s,^[^/]+,$(call stem,$(WASI_SDK)),x" ) )