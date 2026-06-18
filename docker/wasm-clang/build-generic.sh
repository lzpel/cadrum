#!/usr/bin/env bash
# generic clang.wasm を作る：clang 本体 ＋ wasi-sysroot/clang-resource ヘッダだけ #embed 焼き込み
# （OCCT/wrapper は焼かない）＋ wasi_bridge.o をリンク。
# bridge は起動時に wasmtime preopen を raw WASI で MEMFS へコピーするので、ソースや
# プロジェクト include は焼き込み無しで host から渡せる（clang-wasm crate が preopen で供給）。
# 出力 /work/generic-clang.wasm。import が env=0（純WASI）であることを確認する。
set -euo pipefail
source /opt/emsdk/emsdk_env.sh >/dev/null 2>&1 || true
SRC=/src
EMB=/work/build-emcc
HERE="$SRC/docker/wasm-clang"

echo "=== stage sysroot + clang resource (baked, stable) ==="
rm -rf /work/sgen && mkdir -p /work/sgen/sysroot/include /work/sgen/res/include
cp -r "$SRC"/sandbox-wasm/bundle/sysroot/include/wasm32-wasip1 /work/sgen/sysroot/include/wasm32-wasip1
cp -r "$EMB"/lib/clang/*/include/. /work/sgen/res/include/
python3 "$HERE/pack.py" /work/sgen > /work/gdata.bin
echo "baked blob: $(du -h /work/gdata.bin | cut -f1)"

cat > /work/embed_data.c <<EOF
const unsigned char cadrum_blob_start[] = {
#embed "/work/gdata.bin"
};
const unsigned long cadrum_blob_len = sizeof(cadrum_blob_start);
EOF

echo "=== compile embed (sysroot extractor) + data + bridge ==="
emcc -O2 -c "$HERE/embed_data.c" -o /work/embed.o \
    -sWASMFS -fwasm-exceptions -sWASM_LEGACY_EXCEPTIONS=0 -sSUPPORT_LONGJMP=wasm
emcc -std=gnu23 -O0 -c /work/embed_data.c -o /work/embed_data.o
emcc -O2 -c "$SRC/clang-wasm/bridge/wasi_bridge.c" -o /work/bridge.o \
    -sWASMFS -fwasm-exceptions -sWASM_LEGACY_EXCEPTIONS=0 -sSUPPORT_LONGJMP=wasm

echo "=== relink clang with embed + bridge (force) ==="
rm -f "$EMB"/bin/clang.wasm "$EMB"/bin/clang.js-22 "$EMB"/bin/clang.js
# bridge が叩く raw WASI import(path_open/fd_readdir 等)を emscripten は標準提供しないので、
# import 属性付き宣言＋ERROR_ON_UNDEFINED_SYMBOLS=0 でリンクを通す（import は全て wasi=env0）。
EMBED_OBJ="/work/embed.o /work/embed_data.o /work/bridge.o" \
EM_LINK_EXTRA="-sERROR_ON_UNDEFINED_SYMBOLS=0" \
    bash "$HERE/build-clang-wasm.sh" full >/work/relink.log 2>&1 \
    || { echo "FAIL relink:"; tail -25 /work/relink.log; exit 1; }
cp "$EMB/bin/clang.wasm" /work/generic-clang.wasm
echo "generic-clang.wasm = $(stat -c %s /work/generic-clang.wasm) bytes"
echo "imports: env=$(wasm-objdump -x /work/generic-clang.wasm 2>/dev/null | grep -c '<- env\.' || true) wasi=$(wasm-objdump -x /work/generic-clang.wasm 2>/dev/null | grep -c '<- wasi_snapshot_preview1\.' || true)"
echo "wasi import names:"; wasm-objdump -x /work/generic-clang.wasm 2>/dev/null | grep '<- wasi' | sed 's/.*<- //' | sort -u | tr '\n' ' '; echo
