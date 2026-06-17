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
# #215 検証用: consumer が wasi-sdk を install せず、cadrum が再配布する最小成果物だけで
# wasm 化できるかを実証するための「bundle」。BUNDLE 配下にヘッダ(include/wasm32-wasip1 一式)と
# eh 版ランタイム .a を harvest し、installed sysroot を一切参照せずにビルドする(check-cadrum-bundle)。
# RESOURCE は clang の builtin ヘッダ(stddef.h 等)。これはコンパイラ側の所有物で、#215 では
# clang.wasm に同梱される類なので bundle ではなく compiler 同梱として -isystem 末尾に置く
# (-nostdinc が builtin 経路を切るため明示が要る。libc++ の #include_next <stddef.h> 解決用)。
BUNDLE := $(CURDIR)/bundle/sysroot
RESOURCE := $(firstword $(wildcard $(CURDIR)/../out/$(call stem,$(WASI_SDK))/lib/clang/*/include))
# OCCT の wasm クロスビルドのツールチェインを env で差す（Dockerfile の CC_<target> と同じ流儀）。
# build.rs が CC_<target>/CXX_<target> を CMAKE_C/CXX_COMPILER へ転送する。
# PATH 先頭が wasi-sdk/bin なので `clang`/`clang++` は wasi-sdk のものに解決される。
export CC_wasm32_unknown_unknown := clang
export CXX_wasm32_unknown_unknown := clang++
# generator は env で渡せる（cmake は CMAKE_GENERATOR を尊重）。Windows 既定の VS generator を避ける。
export CMAKE_GENERATOR := Unix Makefiles
# C/C++ のコンパイルだけ --target=wasm32-wasip1 にすると、clang が --sysroot から
# include/wasm32-wasip1 と（-fwasm-exceptions 時は）eh/c++/v1 を自動解決するので -isystem 不要。
# __wasi__ が定義され libc++ の __locale も正規の rune table を選ぶため rune table hack も不要。
# OCCT は例外/RTTI を多用するので -fwasm-exceptions（eh 版 libc++）で通す。RTTI は既定で有効。
# 最終リンクは rustc が wasm32-unknown-unknown で行う（オブジェクトは wasm32 で互換）。
# OCCT が引く wasi-libc 条件付きヘッダ（sys/times.h 等）を通すための emulation マクロ。
# OSD はスタブ化して実体は呼ばないので、対応する -lwasi-emulated-* のリンクは不要。
WASI_EMU := -D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_MMAN -D_WASI_EMULATED_GETPID
# cc-rs(cxx-build) は既定で -fno-exceptions を先頭付近に付ける。-fwasm-exceptions は EH
# モデルを選ぶだけで例外を再有効化しないので、後勝ちの env 末尾で -fexceptions を足す。
export CXXFLAGS_wasm32_unknown_unknown := --target=wasm32-wasip1 --sysroot=$(SYSROOT) -fwasm-exceptions -fexceptions $(WASI_EMU)
export CFLAGS_wasm32_unknown_unknown := --target=wasm32-wasip1 --sysroot=$(SYSROOT) $(WASI_EMU)
# libc.a のリンクは rustc(rust-lld)側。静的アーカイブは遅延リンクなので、libc を参照しない
# pure ビルドには取り込まれない。これで build.rs から sysroot/libc 記述を排除できる。
# eh: 例外あり libc++ の場所。c++ は link-cplusplus(CXXSTDLIB=c++)が出すので search path を
# 足すだけ。OCCT/libc++ が要求する c++abi・unwind も繋ぐ（未参照なら遅延リンクで無害）。
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS := -L native=$(SYSROOT)/lib/wasm32-wasip1/eh -L native=$(SYSROOT)/lib/wasm32-wasip1 -l static=c++abi -l static=unwind -l static=c

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
check-cadrum: download # build the cadrum wasm sample (--target web, like the browser) against ../target's prebuilt OCCT and run it under node; surfaces the WASI-import / EH-mix / __wasm_call_ctors errors
	cargo clean
	OCCT="$$(find $(CURDIR)/../target -maxdepth 1 -type d -name 'occt-*-wasm32_unknown_unknown' | head -1)"; \
	test -n "$$OCCT" || { echo "no extracted wasm OCCT under ../target — run 'make check-wasm32-unknown-unknown' first"; exit 1; }; \
	OCCT_ROOT="$$OCCT" ../out/bin/wasm-pack build . -d ./target --target web --features cadrum
	node --experimental-wasm-exnref run_node.mjs
download: # download wasi-sdk (clang + prebuilt sysroot with libc / libc++).
	cargo install --root ../out wasm-pack
	cd ../out && ( ls $(call stem,$(WASI_SDK))/bin/clang.exe || ( curl -L -O $(WASI_SDK) && tar xf $(notdir $(WASI_SDK)) --transform="s,^[^/]+,$(call stem,$(WASI_SDK)),x" ) )
bundle: download # harvest the minimal redistributable set into ./bundle: the wasm32-wasip1 header tree + the eh-variant runtime .a. This is what cadrum would ship so a consumer needs NO installed wasi-sdk (see #215). The whole-triple header copy keeps include-resolution identical to a real sysroot (a -M minimized subset breaks #include_next ordering).
	rm -rf bundle/sysroot
	mkdir -p bundle/sysroot/include bundle/sysroot/lib/wasm32-wasip1/eh
	cp -r $(SYSROOT)/include/wasm32-wasip1 bundle/sysroot/include/
	cp $(SYSROOT)/lib/wasm32-wasip1/eh/libc++.a $(SYSROOT)/lib/wasm32-wasip1/eh/libc++abi.a $(SYSROOT)/lib/wasm32-wasip1/eh/libunwind.a bundle/sysroot/lib/wasm32-wasip1/eh/
	cp $(SYSROOT)/lib/wasm32-wasip1/libc.a bundle/sysroot/lib/wasm32-wasip1/
	@echo "bundle ready: $$(du -sh bundle/sysroot/include | cut -f1) headers + $$(du -sh bundle/sysroot/lib | cut -f1) runtime .a"
check-cadrum-bundle: bundle bundle-build # #215 feasibility: harvest ./bundle then build the cadrum wasm sample using ONLY clang.exe + ./bundle (NO installed wasi-sysroot). Replaces --sysroot with -nostdinc + explicit -isystem into the bundle, and links the bundled eh runtime .a via rust-lld. Proves the remaining wasi-sdk dependency is just the compiler binary (which #215 ships as clang.wasm). Verify isolation with: make check-cadrum-bundle-isolated
bundle-build: # the bundle-only build+run itself (assumes ./bundle and ../target's wasm OCCT already exist). Touches only ./bundle + the compiler resource dir, never share/wasi-sysroot — which is what check-cadrum-bundle-isolated exploits.
	cargo clean
	OCCT="$$(find $(CURDIR)/../target -maxdepth 1 -type d -name 'occt-*-wasm32_unknown_unknown' | head -1)"; \
	test -n "$$OCCT" || { echo "no extracted wasm OCCT under ../target — run 'make -C .. check-wasm32-unknown-unknown' first"; exit 1; }; \
	CXXFLAGS_wasm32_unknown_unknown="--target=wasm32-wasip1 -fwasm-exceptions -fexceptions $(WASI_EMU) -nostdinc -nostdinc++ -isystem $(BUNDLE)/include/wasm32-wasip1/eh/c++/v1 -isystem $(BUNDLE)/include/wasm32-wasip1 -isystem $(RESOURCE)" \
	CFLAGS_wasm32_unknown_unknown="--target=wasm32-wasip1 $(WASI_EMU) -nostdinc -isystem $(BUNDLE)/include/wasm32-wasip1 -isystem $(RESOURCE)" \
	CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="-L native=$(BUNDLE)/lib/wasm32-wasip1/eh -L native=$(BUNDLE)/lib/wasm32-wasip1 -l static=c++abi -l static=unwind -l static=c" \
	OCCT_ROOT="$$OCCT" ../out/bin/wasm-pack build . -d ./target --target web --features cadrum
	node --experimental-wasm-exnref run_node.mjs
check-cadrum-bundle-isolated: bundle # the falsifiable proof: harvest the bundle WHILE the sysroot is present, then temporarily hide the installed wasi-sysroot so any leak (a path still read from it) fails bundle-build, then restore.
	mv $(SYSROOT) $(SYSROOT).hidden
	$(MAKE) bundle-build; status=$$?; mv $(SYSROOT).hidden $(SYSROOT); exit $$status
