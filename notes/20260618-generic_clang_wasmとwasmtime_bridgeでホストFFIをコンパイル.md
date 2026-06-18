# generic clang.wasm ＋ Rust(wasmtime) bridge crate でホストの実 FFI をコンパイル（#226）

## 結論

**generic な clang.wasm（OCCT/wrapper を焼かず、sysroot だけ焼き込み）を Rust の `wasmtime` crate で in-process
実行し、bridge 経由で host のファイルを読み書きして、cadrum の実 FFI（`wrapper.cpp`＋`ffi.rs.cc`＋OCCT）を
native clang とバイト等価のオブジェクトにコンパイルできた。** 焼き込み無し・docker 無し・外部 wasmtime/node/wasi-sdk
無し（実行は Windows native の cargo から）。

| 成果物 | 値 |
|---|---|
| generic clang.wasm | 101.6MB（clang＋sysroot33MB＋bridge）、**import env=0 / wasi=14**（bridge の path_open/fd_readdir/fd_prestat_* も全て wasi_snapshot_preview1） |
| `wrapper.o`（-Os） | **311,398 B**（#210 native release 311,511 と 113B 差） |
| `ffi.rs.o`（-Os） | **33,772 B**（#210 native release と完全一致） |
| 実 FFI `.a` | 419,258 B（wrapper.o＋ffi.rs.o、#210 release .a 419,488 とほぼ一致） |
| 1 回の compile | ~15s（OCCT 44MB の MEMFS copy-in が支配。cold 46s） |

## bridge（核心・新規）

emscripten WASMFS は wasmtime の `--dir` preopen を見ない（バックエンドが MEMFS/NODEFS/OPFS/… のみで WASI
バックエンドが無い）。だが **WASI import を直接叩けば preopen fd に触れる**（GATE 1a で実証）。これを使い
clang.wasm に C の bridge を内蔵：

- **constructor**：全 preopen を raw WASI（`fd_prestat_get`/`fd_prestat_dir_name`/`path_open`/`fd_readdir`/`fd_read`）で
  再帰列挙し MEMFS へコピー。以後 clang は通常の `fopen` で host のソース/ヘッダを読める（焼き込み不要）。
- **destructor**：出力（guest "/out"）配下の MEMFS を raw `path_open`+`fd_write` で host preopen へ書き戻す
  （file write は 0x00 を化けさせない＝#223 の stdout 破損を回避、hex 不要）。

リンクの肝：`<wasi/api.h>` の `__wasi_*` は「libc 実装」を期待し emscripten が path_open/fd_readdir を提供しないため
リンク不可 → **`import_module("wasi_snapshot_preview1")` を明示した自前宣言**＋ビルド側 `-sERROR_ON_UNDEFINED_SYMBOLS=0`
で通す。これで該当 import が増えても全て純 WASI（env=0 維持）。

## crate `clang-wasm`（独立 crate、後で cadrum の build-dep に）

- `src/lib.rs`：`wasmtime`（39）＋`wasmtime-wasi` の **p1**(preview1) API。`run_clang(wasm, preopens, args)`。
  exnref 実行のため `Config::wasm_function_references/gc/exceptions(true)`。clang の `proc_exit` は `I32Exit` で受ける。
- `src/main.rs`：`clangw <wasm> [--dir HOST::GUEST].. -- <clang args>` の薄い CLI。
- `bridge/wasi_bridge.c`：上記 bridge（clang.wasm にリンク）。

## 検証段階

- **GATE 1a**：最小 wasm で raw `__wasi_path_open` が wasmtime preopen の host ファイルを読めることを実証（`READ ... [HELLO-FROM-HOST-PREOPEN]`）。
- **GATE 1**：`clangw`（wasmtime crate）で host の trivial `.cpp` を preopen→bridge→generic clang.wasm→host に valid `.o`（380B, magic `0061736d`）。
- **GATE 2**：実 `wrapper.cpp`＋OCCT include＋cxx glue を preopen して `-Os` で `wrapper.o`（311,398B）。さらに `ffi.rs.cc` も `ffi.rs.o`（33,772B）。両方 native とバイト等価、実 OCCT シンボル（TopoDS/BRepBndLib/opencascade::handle、`_GLOBAL__sub_I_wrapper.cpp`）。

## GATE 3（make check）— 追加で判明した productionization 要件

`build.rs` に **env-gated cxx_build-skip** を追加（`CADRUM_PREBUILT_FFI=lib<release_name>.a` のとき C++ コンパイルを
丸ごと飛ばし、その `.a` をリンク）。これで cadrum 側の wrapper/ffi の C++ コンパイルは消えるが、**`make check` green には
さらに2点**が要ると判明：

1. **`cxx` crate 自身が `cxx.cc`（cxx ランタイム）を cc-rs でコンパイルする** → cadrum の cxx_build を skip しても
   `clang++` を探して失敗（`error occurred in cc-rs: failed to find tool "clang++"`）。真の rustc-only には、cxx.cc も
   clang.wasm で通す **`CXX=clangw` 透過 shim**（cc-rs の argv から preopen を組み立てる launcher）が追加で必要。
2. **version 一致**：本セッション中に main が 0.8.11→0.8.12（rev3, #221）へ drift。`src/occt/ffi.rs` も
   `Solid::sew/offset_surface/thru_sections` 追加で変化 → 旧 glue の `.a` は symbol 不足。FFI `.a` は cadrum 版に追従が要る。

→ GATE 3 は「両 FFI TU を rustc-only でリンクして run」の最後の配線（cxx.cc 用透過 shim ＋ 版一致）が残課題で、
**コンパイル能力自体は GATE 1/2 で完全に実証済み**。

## 主な詰まり所（記録）
- wasmtime-wasi 39 の API は `preview1`→**`p1`**、`WasiCtxBuilder`/`I32Exit`/`DirPerms`/`FilePerms` は top-level。
- bridge の raw WASI を `<wasi/api.h>` で書くとリンク不可（emscripten 未提供）→ import 属性付き自前宣言＋`-sERROR_ON_UNDEFINED_SYMBOLS=0`。
- **`walk_dir` の `dbuf` が `static`** だと再帰で外側ループが壊れ、preopen ツリーを取りこぼす（`rust/cxx.h` not found）→ 非 static に。
- Windows native `clangw.exe` を git-bash から叩くと `/in/t.cpp` が MSYS パス変換で化ける → `MSYS_NO_PATHCONV=1`。

## 再現
```sh
# 1) generic clang.wasm（docker, sysroot焼き込み＋bridge）
docker run --rm -v "$PWD:/src" -v "$PWD/out/wasm-clang:/work" wasm-clang \
  bash /src/docker/wasm-clang/build-generic.sh        # → out/wasm-clang/generic-clang.wasm

# 2) crate ビルド（Windows native）
cd clang-wasm && cargo build --release

# 3) host のソースを preopen 経由でコンパイル（例: 実 FFI を -Os）
MSYS_NO_PATHCONV=1 ./target/release/clangw.exe <generic-clang.wasm> \
  --dir <cpp>::/cadrum/cpp --dir <cxxbridge>::/cxxbridge --dir <occt>/include/opencascade::/occt/opencascade --dir <out>::/out -- \
  clang -c /cadrum/cpp/wrapper.cpp -o /out/wrapper.o -Os --target=wasm32-wasip1 -fwasm-exceptions \
    -mllvm -wasm-use-legacy-eh=false -std=c++17 -D_USE_MATH_DEFINES -DCADRUM_COLOR -D_WASI_EMULATED_* \
    -nostdinc -nostdinc++ -nobuiltininc -isystem /sysroot/include/wasm32-wasip1/eh/c++/v1 \
    -isystem /sysroot/include/wasm32-wasip1 -isystem /res/include -I /occt/opencascade -I /cxxbridge/include -I /
```

関連: #226（本方針）, #223（純WASI clang.wasm で実 FFI を bare wasmtime コンパイル）, #215, #219, #220, #210, #224。
