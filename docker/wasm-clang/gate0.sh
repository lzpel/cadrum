#!/usr/bin/env bash
# GATE 0: #220 の純WASIパターン(env import=0)を再確認し、さらに clang.wasm が使う
# stdin-tar → WasmFS → stdout の実 I/O 経路が bare wasmtime で成立することを検証する。
# 多時間の clang ビルドに進む前の fail-fast ゲート。
set -euo pipefail
source /opt/emsdk/emsdk_env.sh >/dev/null 2>&1 || true

HERE="$(cd "$(dirname "$0")" && pwd)"
OUT=/work/gate0
mkdir -p "$OUT"
cd "$OUT"

echo "=== versions ==="
emcc --version | head -1
wasmtime --version
echo

# #220 で env=0 を出した純WASI構成のフラグ（--embed-file は使わない）。
EMFLAGS="-O1 -sWASMFS -sSTANDALONE_WASM -sWASM_LEGACY_EXCEPTIONS=0 -fwasm-exceptions -sSUPPORT_LONGJMP=wasm -mllvm -wasm-use-legacy-eh=false"
WT="wasmtime run -W exceptions=y -W function-references=y -W gc=y"

# import 検査: env import が 1 つでもあれば純WASIではない。
assert_env0() {
    local wasm="$1"
    local env_n wasi_n
    env_n=$(wasm-objdump -x "$wasm" | grep -c '<- env\.' || true)
    wasi_n=$(wasm-objdump -x "$wasm" | grep -c '<- wasi_snapshot_preview1\.' || true)
    echo "  imports: wasi=$wasi_n env=$env_n"
    if [ "$env_n" -ne 0 ]; then
        echo "  FAIL: $wasm has $env_n env import(s) — not pure WASI"
        wasm-objdump -x "$wasm" | grep '<- env\.' || true
        return 1
    fi
}

echo "=== GATE 0.1: minimal WasmFS RW + exnref EH ==="
em++ $EMFLAGS -x c++ "$HERE/gate0_min_fs.c" -o min_fs.wasm
assert_env0 min_fs.wasm
OUT1=$($WT min_fs.wasm)
echo "$OUT1"
echo "$OUT1" | grep -q 'READBACK\[/x.txt\]=\[INWASM-FS-OK\]' || { echo "FAIL: in-wasm FS RW (/x.txt)"; exit 1; }
echo "$OUT1" | grep -q 'READBACK\[/tmp/x.txt\]=\[INWASM-FS-OK\]' || { echo "FAIL: in-wasm FS RW (/tmp)"; exit 1; }
echo "$OUT1" | grep -q 'EH=7' || { echo "FAIL: exnref EH"; exit 1; }
echo "  PASS 0.1"
echo

echo "=== GATE 0.2: stdin-tar -> WasmFS -> stdout (clang の実 I/O 経路) ==="
emcc $EMFLAGS "$HERE/gate0_tar_io.c" -o tar_io.wasm
assert_env0 tar_io.wasm
# サンプル tar を作る（ustar）。main.cpp を読み戻して stdout に出させる。
mkdir -p sample/sub
printf 'CONTENT-OF-main.cpp-line1\nline2\n' > sample/main.cpp
printf 'aux-header-data\n' > sample/sub/aux.h
( cd sample && bsdtar --format ustar -cf ../sample.tar main.cpp sub/aux.h )
OUT2=$($WT tar_io.wasm main.cpp < sample.tar)
echo "--- stdout ---"; echo "$OUT2"
echo "$OUT2" | grep -q 'CONTENT-OF-main.cpp-line1' || { echo "FAIL: stdout did not return extracted member"; exit 1; }
echo "  PASS 0.2"
echo
echo "##### GATE 0 PASS: pure-WASI (env=0) + in-wasm FS + exnref + stdin-tar/stdout all work under bare wasmtime #####"
