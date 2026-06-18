#!/usr/bin/env bash
# emscripten-standalone で LLVM+clang を wasm へビルドする。
#   stage=smoke : configure + LLVMSupport だけビルド（wasi 経路が詰まった POSIX 依存 TU=
#                 CrashRecoveryContext / Signals / Program(fork,execve) / Process が
#                 emscripten musl で改変なしに通るかの fail-fast ゲート）
#   stage=full  : 上に加えて clang 本体をリンクし clang.wasm を得る（数時間）
#
# 成果物・ビルドツリーは /work（ホスト out/wasm-clang を mount）配下に置き再開可能にする。
set -euo pipefail
source /opt/emsdk/emsdk_env.sh >/dev/null 2>&1 || true

STAGE="${1:-smoke}"
LLVM_TAG="${LLVM_TAG:-llvmorg-22.1.0}"
SRC=/work/llvm-project
NATIVE=/work/build-native
EMB=/work/build-emcc
JOBS="$(nproc)"

echo "=== build-clang-wasm stage=$STAGE tag=$LLVM_TAG jobs=$JOBS ==="
emcc --version | head -1

# --- 0. ソース取得（depth1, tag）。tag が無ければ release/22.x にフォールバック ---
if [ ! -d "$SRC/llvm" ]; then
    echo "--- cloning llvm-project ($LLVM_TAG) ---"
    git clone --depth 1 --branch "$LLVM_TAG" https://github.com/llvm/llvm-project "$SRC" 2>/dev/null \
      || git clone --depth 1 --branch release/22.x https://github.com/llvm/llvm-project "$SRC"
fi
( cd "$SRC" && git log -1 --format='llvm-project @ %h %d' || true )

COMMON_CMAKE=(
    -G Ninja
    -DCMAKE_BUILD_TYPE=Release
    -DLLVM_ENABLE_PROJECTS=clang
    -DLLVM_TARGETS_TO_BUILD=WebAssembly
    -DLLVM_INCLUDE_TESTS=OFF
    -DLLVM_INCLUDE_BENCHMARKS=OFF
    -DLLVM_INCLUDE_EXAMPLES=OFF
    -DLLVM_ENABLE_ZLIB=OFF
    -DLLVM_ENABLE_ZSTD=OFF
    -DLLVM_ENABLE_LIBXML2=OFF
    -DLLVM_ENABLE_TERMINFO=OFF
    -DLLVM_ENABLE_LIBEDIT=OFF
    -DLLVM_ENABLE_LIBPFM=OFF
)

# --- 1. native tablegen（ホスト clang/gcc） ---
if [ ! -x "$NATIVE/bin/llvm-tblgen" ]; then
    echo "--- configure native tablegen ---"
    cmake -S "$SRC/llvm" -B "$NATIVE" "${COMMON_CMAKE[@]}"
    echo "--- build native tblgen ---"
    ninja -C "$NATIVE" llvm-tblgen clang-tblgen llvm-min-tblgen
fi

# --- 2. emscripten クロス configure ---
# THREADS=OFF（単スレ clang）, CLANG_SPAWN_CC1=OFF（WASI に exec 無→cc1 in-process; 既定 OFF だが明示）,
# tablegen は native を流用, HOST_TRIPLE は emscripten, DEFAULT_TARGET は wasm32-wasip1。
#
# env import を 0 に保つ鍵: SUPPORT_LONGJMP=wasm は **compile 時** に要る（既定の emscripten
# longjmp は JS=env import を出す。LLVM の CrashRecoveryContext 等が setjmp を使う）。
# 最終 clang は STANDALONE_WASM + WASMFS(memory backend) + 例外/ longjmp を wasm 命令で。
# clang はパーサが深いので STACK_SIZE を拡張、メモリは grow 可能に。
EM_COMPILE="-sSUPPORT_LONGJMP=wasm -fwasm-exceptions -sWASM_LEGACY_EXCEPTIONS=0 -mllvm -wasm-use-legacy-eh=false"
# EXIT_RUNTIME は STANDALONE_WASM では自動決定（main あり=True）なので明示禁止 → 付けない。
# 純WASI(env=0)を守るため ALLOW_MEMORY_GROWTH は使わない（成長は env.emscripten_notify_memory_growth
# を import するため。WASI にメモリ成長通知 API が無い → #22211/wazero#601）。代わりに固定大メモリ。
# clang の wrapper.cpp コンパイル peak は native 実測 ~163MB なので 2GB 固定で十分余裕。
EM_LINK="-sSTANDALONE_WASM -sWASMFS -sSUPPORT_LONGJMP=wasm -fwasm-exceptions -sWASM_LEGACY_EXCEPTIONS=0 -sINITIAL_MEMORY=2147483648 -sSTACK_SIZE=16777216"
# GATE C: stdin-tar 展開 constructor の .o を clang にリンクし、起動時に WASMFS へ
# プロジェクト/ヘッダを展開できる clang.wasm を作る（EMBED_OBJ 指定時のみ）。
[ -n "${EMBED_OBJ:-}" ] && EM_LINK="$EM_LINK ${EMBED_OBJ}"
# 再configure は安価かつ冪等（compile/link フラグを反映させるため毎回流す）。
echo "--- emcmake configure (emscripten target) ---"
emcmake cmake -S "$SRC/llvm" -B "$EMB" "${COMMON_CMAKE[@]}" \
    -DLLVM_ENABLE_THREADS=OFF \
    -DCLANG_SPAWN_CC1=OFF \
    -DLLVM_TABLEGEN="$NATIVE/bin/llvm-tblgen" \
    -DCLANG_TABLEGEN="$NATIVE/bin/clang-tblgen" \
    -DLLVM_HOST_TRIPLE=wasm32-unknown-emscripten \
    -DLLVM_DEFAULT_TARGET_TRIPLE=wasm32-wasip1 \
    -DLLVM_BUILD_TOOLS=OFF \
    -DCLANG_BUILD_TOOLS=ON \
    -DCMAKE_C_FLAGS="$EM_COMPILE" \
    -DCMAKE_CXX_FLAGS="$EM_COMPILE" \
    -DCMAKE_EXE_LINKER_FLAGS="$EM_LINK" \
    -DCMAKE_EXECUTABLE_SUFFIX=.wasm

# --- 3. smoke: POSIX 依存の壁(LLVMSupport)を先に通す ---
echo "--- build LLVMSupport (POSIX wall smoke) ---"
ninja -C "$EMB" -j "$JOBS" LLVMSupport
echo "##### SMOKE OK: LLVMSupport built under emscripten (sigaction/fork wall cleared) #####"

if [ "$STAGE" = "full" ]; then
    echo "--- build clang (full, hours) ---"
    ninja -C "$EMB" -j "$JOBS" clang
    echo "--- locate clang wasm ---"
    find "$EMB/bin" -maxdepth 1 -name 'clang*.wasm' -printf '%p %s bytes\n' | head
    CLANG_WASM="$(find "$EMB/bin" -maxdepth 1 -name 'clang*.wasm' ! -name '*-*' | head -1)"
    [ -n "$CLANG_WASM" ] || CLANG_WASM="$(find "$EMB/bin" -maxdepth 1 -name 'clang*.wasm' | head -1)"
    echo "CLANG_WASM=$CLANG_WASM"
    echo "--- imports (must be env=0 for pure WASI) ---"
    wasm-objdump -x "$CLANG_WASM" | grep -c '<- env\.' | sed 's/^/  env imports: /' || true
    wasm-objdump -x "$CLANG_WASM" | grep -c '<- wasi_snapshot_preview1\.' | sed 's/^/  wasi imports: /' || true
    echo "--- run clang --version under bare wasmtime ---"
    wasmtime run -W exceptions=y -W function-references=y -W gc=y "$CLANG_WASM" -- --version 2>&1 | head -5 || true
    # 安定版コピーを /work 直下に置く（GATE C で使う）
    cp "$CLANG_WASM" /work/clang.wasm && echo "copied -> /work/clang.wasm ($(stat -c %s /work/clang.wasm) bytes)"
fi
