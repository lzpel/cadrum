# 純 WASI 化の実証: emscripten-standalone で env import 0、in-wasm FS が bare wasmtime で動く

#220 の方針（emscripten clang.wasm を純 WASI 化）の検証フェーズ。**両ゲート合格**。

## 環境
- docker ubuntu:24.04、`/work`=`C:\Users\smith\wasi-sdk-build`。emsdk(emcc 6.0.0) / wasmtime 45.0.2 は `/work` 配下。
- import 列挙は emsdk 同梱 node ＋ `--experimental-wasm-exnref`（モジュールが exnref を使うため node も要求）。

## GATE A' — `--embed-file` を使わなければ env import が 0（純 WASI）

`memfs_probe.cpp`（in-wasm WasmFS に fopen("w")→write→fopen("r")→read、host/preopen 不使用）を
`emcc -sSTANDALONE_WASM -sWASMFS -sWASM_LEGACY_EXCEPTIONS=0 -sSUPPORT_LONGJMP=wasm -fwasm-exceptions` でビルド:

- **imports = `wasi_snapshot_preview1` (5): clock_time_get, fd_read, fd_write, proc_exit, random_get のみ。`env` = 0。**
- bare `wasmtime run -W exceptions=y -W function-references=y -W gc=y`（**`--dir` 無し**）で:
  `READBACK[/x.txt]=[INWASM-FS-OK]` `READBACK[/tmp/x.txt]=[INWASM-FS-OK]` `EH=7` → **file RW＋例外が成立**。

→ 先に出た `env`(7)（`_emscripten_fs_load_embedded_files` / `_wasmfs_jsimpl_*`）は **`--embed-file` 由来**だった。
  `--embed-file` を外すと WasmFS は **純 wasm の memory backend** を使い JS hook が消える。

## GATE B' — read-only データの in-wasm 焼き込みも env 0 を維持

`probeB.cpp`: データを `xxd -i` で C 配列化して wasm に焼き込み、起動時に in-wasm で WasmFS へ write→read（`--embed-file` 不使用）:

- **imports = `wasi_snapshot_preview1` (5) のみ（env=0 維持）。**
- bare wasmtime（`--dir` 無し）で `VIA_CARRAY=[SYSROOT-STUB-DATA-via-Carray] len=28` → 焼き込みデータを読めた。

## 結論（純 WASI 化は成立）

emscripten-standalone でも、**`--embed-file` を避け WasmFS の in-wasm memory backend ＋ C 配列焼き込み**にすれば:
- **import は `wasi_snapshot_preview1` のみ（env=0）= 純 WASI** → bare wasmtime CLI でも build.rs 埋め込み wasmtime でも動く。
- in-wasm FS の read/write、WASI stdio、exnref 例外、すべて bare wasmtime で動作。

→ clang 本体に適用する道筋:
1. clang を `-sWASMFS -sSTANDALONE_WASM`（`--embed-file` なし）＋ exnref で emscripten ビルド。
2. wasi-sdk-33 の sysroot/ヘッダを **tar→C 配列**で焼き込み、起動フックで WasmFS に展開（env 0 維持）。
3. 入力 `.cpp` は stdin、出力 `.o` は stdout（WASI）。`clang -cc1`（integrated, fork 不要）。
4. build.rs に `wasmtime` クレートを埋め込み in-process 実行 → FFI 生成 → cxx_build skip ＝ consumer は rustc-only。

## リスク（後続フェーズ）
- clang の実 FS 利用（多数 open/stat、ヘッダ探索）が memory backend ＋ 焼き込み sysroot で満たせるか（本丸）。
- 焼き込み sysroot サイズ（数 MB〜）と clang 本体 emscripten ビルドの時間/容量。
- 起動フック（main 前）で sysroot 展開する仕組み（C++ static ctor / emscripten preinit）。

## 資産（再現）
`C:\Users\smith\wasi-sdk-build`: emsdk / wasmtime / `memfs_probe.cpp` `probeA.sh` `probeB.cpp` `probeB.sh` `li2.js`。
関連: #215 #216 #219 #220、notes（GATE A の FS 壁・wasi-libc 移植停止・メモリ ~163MB）。
