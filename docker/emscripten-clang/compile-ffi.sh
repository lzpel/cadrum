#!/usr/bin/env bash
# GATE C: 自前ビルドの clang.wasm を bare wasmtime で動かし、cadrum の FFI
# (cpp/wrapper.cpp + cxx glue) を wasm32-wasip1 の .o にコンパイルできるか検証する。
#
# WASMFS は WASI preopen(--dir)を見ず、standalone の stdin は大入力で破綻し、clang の raw
# バイナリ stdout 書き込みは 0x00 が化ける（emscripten #23724 / #21335）。よって:
#  - 入力一式（ヘッダ＋ソース）を pack.py で 1 本の blob にまとめ #embed で clang.wasm に焼き込み、
#    起動時に constructor(embed_data.c) が WASMFS へ展開する。
#  - clang は WASMFS 上の /cadrum/cpp/wrapper.cpp を `-o /tmp/cadrum_out.o` でファイル出力し、
#    destructor がそれを hex で stdout に吐く → ホストで xxd -r -p して .o を復元する。
set -euo pipefail
source /opt/emsdk/emsdk_env.sh >/dev/null 2>&1 || true

SRC=/src
EMB=/work/build-emcc
HERE="$SRC/docker/emscripten-clang"
LLVMNM=/opt/emsdk/upstream/bin/llvm-nm
OBJDUMP=/opt/emsdk/upstream/bin/llvm-objdump
WT="wasmtime run -W exceptions=y -W function-references=y -W gc=y"

# --- 1. 入力一式を WASMFS パスに staging（ヘッダ＋ソース） ---
echo "=== stage FFI inputs (headers + source) ==="
CXXB="$(ls -d "$SRC"/sandbox-wasm/target/wasm32-unknown-unknown/release/build/cadrum-*/out/cxxbridge/include | head -1)"
test -n "$CXXB" || { echo "FAIL: cxxbridge include not found (build sandbox-wasm first)"; exit 1; }
rm -rf /work/s1 && mkdir -p /work/s1/cadrum/cpp /work/s1/cxxbridge/include /work/s1/occt /work/s1/sysroot/include /work/s1/res/include
cp "$SRC"/cpp/wrapper.cpp "$SRC"/cpp/wrapper.h /work/s1/cadrum/cpp/
cp -r "$CXXB"/. /work/s1/cxxbridge/include/
cp -r "$SRC"/target/occt-8_0_0_rev2-wasm32_unknown_unknown/include/opencascade /work/s1/occt/opencascade
cp -r "$SRC"/sandbox-wasm/bundle/sysroot/include/wasm32-wasip1 /work/s1/sysroot/include/wasm32-wasip1
cp -r "$EMB"/lib/clang/*/include/. /work/s1/res/include/
echo "staged: $(du -sh /work/s1 | cut -f1)"

# --- 2. blob にまとめる ---
python3 "$HERE/pack.py" /work/s1 > /work/data.bin
echo "blob: $(du -h /work/data.bin | cut -f1)"

# --- 3. #embed で焼き込む .c を生成（clang 22 / C23。wasm の .section 構文を避ける） ---
cat > /work/embed_data.c <<EOF
const unsigned char cadrum_blob_start[] = {
#embed "/work/data.bin"
};
const unsigned long cadrum_blob_len = sizeof(cadrum_blob_start);
EOF

# --- 4. constructor と blob を clang にリンク ---
echo "=== relink clang.wasm with embedded blob ==="
emcc -O2 -c "$HERE/embed_data.c" -o /work/embed.o \
    -sWASMFS -fwasm-exceptions -sWASM_LEGACY_EXCEPTIONS=0 -sSUPPORT_LONGJMP=wasm
emcc -std=gnu23 -O0 -c /work/embed_data.c -o /work/embed_data.o
# embed*.o は EXE_LINKER_FLAGS 経由で ninja が依存追跡しない → 出力を消して強制再リンク。
rm -f "$EMB"/bin/clang.wasm "$EMB"/bin/clang.js-22 "$EMB"/bin/clang.js
EMBED_OBJ="/work/embed.o /work/embed_data.o" bash "$HERE/build-clang-wasm.sh" full >/work/relink.log 2>&1 \
    || { echo "FAIL: relink (see /work/relink.log)"; tail -20 /work/relink.log; exit 1; }
cp "$EMB/bin/clang.wasm" /work/clang-ffi.wasm
echo "clang-ffi.wasm = $(stat -c %s /work/clang-ffi.wasm) bytes"
echo "imports: env=$(wasm-objdump -x /work/clang-ffi.wasm 2>/dev/null | grep -c '<- env\.' || true) wasi=$(wasm-objdump -x /work/clang-ffi.wasm 2>/dev/null | grep -c '<- wasi_snapshot_preview1\.' || true)"

# --- 5. clang.wasm で wrapper.cpp をコンパイル（stdin 不使用、.o はファイル→hex で取り出し） ---
echo "=== GATE C: compile cadrum FFI wrapper.cpp under bare wasmtime ==="
WASI_EMU="-D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_MMAN -D_WASI_EMULATED_GETPID"
# clang はファイルへ出力（raw バイナリ stdout は 0x00 が化けるため）。destructor が
# /tmp/cadrum_out.o を hex で stdout に吐くので、ホストで xxd -r -p して復元する。
set +e
$WT /work/clang-ffi.wasm \
    -c /cadrum/cpp/wrapper.cpp -o /tmp/cadrum_out.o \
    --target=wasm32-wasip1 -fwasm-exceptions -fexceptions -mllvm -wasm-use-legacy-eh=false \
    -std=c++17 -D_USE_MATH_DEFINES -DCADRUM_COLOR $WASI_EMU \
    -nostdinc -nostdinc++ -nobuiltininc \
    -isystem /sysroot/include/wasm32-wasip1/eh/c++/v1 \
    -isystem /sysroot/include/wasm32-wasip1 \
    -isystem /res/include \
    -I /occt/opencascade -I /cxxbridge/include -I / \
    > /work/wrapper.hex 2>/work/c1.err
rc=$?
set -e
xxd -r -p /work/wrapper.hex > /work/wrapper.o 2>/dev/null || true
echo "--- clang stderr (head) ---"; head -30 /work/c1.err
echo "exit=$rc  wrapper.o=$(stat -c %s /work/wrapper.o 2>/dev/null || echo 0) bytes (hex=$(stat -c %s /work/wrapper.hex 2>/dev/null || echo 0))"

MAGIC=$(head -c4 /work/wrapper.o | xxd -p)
echo "wrapper.o magic = $MAGIC (valid wasm object = 0061736d)"
if [ "$rc" -eq 0 ] && [ "$MAGIC" = "0061736d" ]; then
    echo "=== inspect wrapper.o ==="
    "$OBJDUMP" --section-headers /work/wrapper.o 2>&1 | head -20 || true
    echo "--- defined text symbols (sample) ---"
    "$LLVMNM" /work/wrapper.o 2>&1 | grep -iE ' [TtWw] ' | head -10 || true
    echo "--- symbol count ---"; "$LLVMNM" /work/wrapper.o 2>&1 | wc -l
    # 既存 .a を消してから作る（/work 永続なので ar 追記で同一オブジェクトが二重格納されるのを防ぐ）。
    rm -f /work/libcadrum_ffi_wasmclang.a
    ( cd /work && /opt/emsdk/upstream/bin/llvm-ar rcs libcadrum_ffi_wasmclang.a wrapper.o )
    echo "archived: $(stat -c %s /work/libcadrum_ffi_wasmclang.a) bytes"
    echo "##### GATE C PASS: clang.wasm compiled cadrum FFI wrapper.cpp -> valid wasm32 object under bare wasmtime #####"
else
    echo "##### GATE C FAIL: wrapper.cpp compile did not produce a valid object (see /work/c1.err) #####"
    exit 1
fi
