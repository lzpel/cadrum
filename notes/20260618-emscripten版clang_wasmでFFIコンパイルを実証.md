# emscripten-standalone な純WASI clang.wasm を作り、bare wasmtime で cadrum FFI をコンパイル（#220 approach 2 = 実証 GO）

## 結論

自前ビルドした **emscripten-standalone・純WASI（env import=0）の clang.wasm** を **bare wasmtime** で動かし、
cadrum の **`cpp/wrapper.cpp`（OCCT ヘッダ＋cxx glue＋libc++ 込み）を wasm32-wasip1 の正規オブジェクトにコンパイル**できた。
→ #215 approach 2 / #219 / #220 の「動く clang.wasm を入手して consumer 側 wrapper をコンパイルする」路線は**成立（GO）**。

- 生成 `wrapper.o` = **1,001,976 bytes**、magic `0061736d`（valid wasm）、`Arch: wasm32`、**2,994 シンボル**
  （実 OCCT/cadrum シンボル: `TopoDS_Edge` の ctor/dtor、`NCollection` type descriptor、`_GLOBAL__sub_I_wrapper.cpp` 等）。
- EH 種別 = **exnref**（`target_features` に `+exception-handling` ＋ `+reference-types`）→ OCCT prebuilt の exnref（#204）と一致。
- `llvm-ar` で `libcadrum_ffi_wasmclang.a`（2.28MB）化。

## 環境

docker `ubuntu:24.04` ＋ **emsdk(emcc/clang 6.0.0 = LLVM 22.1.0)** ＋ **wasmtime 45.0.2** ＋ ninja/cmake。
ビルド対象は **llvm-project `llvmorg-22.1.0`**（exnref を emit 可能。OCCT prebuilt と同系）。
OCCT は released prebuilt `occt-8_0_0_rev2-wasm32_unknown_unknown`、sysroot ヘッダは #215 の `sandbox-wasm/bundle`。
ホスト 32CPU/16.6GB。

## なぜ emscripten 経路か（前段の確定事項）

- メモリ関門は別途 clear 済み（native proxy で wrapper.cpp ピーク ~163MB、4GB の約25倍余裕）。
- **wasi-sdk LLVM の wasm クロスビルドは失敗**：wasi-libc が `sigaction`/`fork`/`execve` を欠き `CrashRecoveryContext.cpp` で停止。
- prebuilt clang.wasm も不適（wasmer の clang は LLVM16=exnref 非対応、WASI に exec 無で `-cc1` 不可）。
- → **emscripten の musl が `sigaction` 等をシンボル提供**するので、LLVM の POSIX 依存コードをソース改変なしでコンパイルできる。

## 段階 GATE と知見

### GATE 0 — 純WASI I/O 経路（分）
最小プログラムで、`-sWASMFS -sSTANDALONE_WASM -sWASM_LEGACY_EXCEPTIONS=0 -fwasm-exceptions -sSUPPORT_LONGJMP=wasm`（`--embed-file` 無し）→
**import は wasi のみ（env=0）**、bare wasmtime（`-W exceptions=y -W function-references=y -W gc=y`、`--dir` 無し）で in-wasm WasmFS RW＋exnref EH 成立。

### GATE B-smoke — POSIX 依存 TU の壁（分水嶺）
LLVM+clang を **2 段 cmake**（native tablegen ＋ emscripten target、`LLVM_ENABLE_THREADS=OFF`・`CLANG_SPAWN_CC1=OFF`・
`LLVM_TARGETS_TO_BUILD=WebAssembly`）で configure し、**`LLVMSupport` をビルド**。wasi 経路を殺した
`CrashRecoveryContext`/`Signals`/`Program`(fork,execve)/`Process` を含め **183/183 改変なしでコンパイル成功**＝
emscripten musl で sigaction/fork の壁が消えることを実証（最大の未知をクリア）。

### GATE B — full clang.wasm（数時間）
`ninja clang` で **clang.wasm（67MB）** 生成。env=0 を保つための要点（ハマり所）:
- **`-sSUPPORT_LONGJMP=wasm` は compile 時に必須**（既定の emscripten longjmp は JS=env import を出す）。
- **`EXIT_RUNTIME` は STANDALONE_WASM では明示不可**（main 有=自動 True）→ 付けない。
- **`INITIAL_MEMORY` は clang 静的データ(~24MB)以上**が要る（既定16MBだと wasm-ld が拒否）。
- **`ALLOW_MEMORY_GROWTH` は `env.emscripten_notify_memory_growth` を import する**（WASI に成長通知 API 無し）→
  成長を切り **固定 2GB メモリ**に（wrapper.cpp peak ~163MB なので十分）。これで **import = wasi のみ（env=0）**。
- `clang --version` が bare wasmtime で動作（`Target: wasm32-unknown-wasip1`）。
  ※ wasmtime のゲスト引数は**モジュール名の後にそのまま**渡す（`--` を入れると clang が全フラグをファイル名扱いして壊れる）。

### GATE C — cadrum FFI をコンパイル（検証本体）
clang.wasm へどう入力を渡すかが鍵。standalone WASMFS の制約を順に回避:
- **WASI preopen(`--dir`)は WASMFS が見ない**（"/" は memory backend 固定）→ ホスト dir を mount できない。
- **standalone の stdin は大入力で破綻**（4.7MB は読めるが 60MB は 115B で切れる。emscripten #23724/#21335）。
- **clang の raw バイナリ stdout は 0x00 が 0x0a に化ける**（`putchar` 経由は無事＝raw fd 書き込み経路の問題）。
- → 採った方式（#220「次段 A」の正攻法）:
  1. 入力一式（OCCT/sysroot/clang-resource ヘッダ＋cxx glue＋wrapper.cpp）を `pack.py` で 1 本の blob 化（60MB）、
     **`#embed`（C23/clang22）で clang.wasm に焼き込み**、起動時に **constructor が WASMFS へ展開**（10,973 ファイル）。
  2. clang は WASMFS の `/cadrum/cpp/wrapper.cpp` を **`-o /tmp/cadrum_out.o`** でファイル出力。
  3. **destructor が `/tmp/cadrum_out.o` を hex で stdout に吐く**（hex は 0x00 を含まず安全）→ ホストで `xxd -r -p` 復元。
- 結果 = 上記「結論」のとおり **valid な exnref wasm32 オブジェクト**。

## 成果物（再現用）

`docker/Dockerfile_emscripten-clang` ＋ `docker/emscripten-clang/`:
- `gate0.sh` ＋ `gate0_min_fs.c` / `gate0_tar_io.c` — GATE 0
- `build-clang-wasm.sh` — LLVM/clang を smoke/full でビルド（llvmorg-22.1.0 を /work に clone）
- `embed_data.c` — #embed blob を WASMFS へ展開する constructor ＋ 出力 hex 化 destructor
- `pack.py` — 入力を blob 化（`<path>\t<size>\n<data>` 連結）
- `compile-ffi.sh` — GATE C 一式

再現（リポジトリ root、要 docker。ビルドツリー/巨大成果物は `out/emscripten-clang/`=gitignore）:
```sh
docker build -t emcc-clang - < docker/Dockerfile_emscripten-clang
R="-v $PWD:/src -v $PWD/out/emscripten-clang:/work"
docker run --rm $R emcc-clang bash /src/docker/emscripten-clang/gate0.sh            # GATE 0
docker run --rm $R emcc-clang bash /src/docker/emscripten-clang/build-clang-wasm.sh full   # clang.wasm（数時間）
docker run --rm $R emcc-clang bash /src/docker/emscripten-clang/compile-ffi.sh      # GATE C: wrapper.o 生成
```
※ GATE C は OCCT prebuilt を `target/occt-…-wasm32_unknown_unknown` に、sysroot を `sandbox-wasm/bundle` に、
cxx glue を sandbox-wasm の release ビルドで用意しておくこと（#215 の手順）。

## 残課題（feasibility ではなく productionization）

- **consumer 経路**: 生成 `.a` を cxx_build の代わりに link する build.rs 分岐（cxx_build skip + prebuilt FFI link）→
  `make check-wasm32-unknown-unknown` を green に。FFI 名は `release_name(Some(t), true)` で予約済み。
- **clang.wasm の配布形**: 67MB(clang) ＋ sysroot 焼き込み。焼き込みヘッダは #215 の最小集合（6.6MB）に絞れる。
- **実行系**: build.rs に `wasmtime` を build-dep として in-process 実行（外部 wasmtime/node 不要）。
- **ビルドの CI 化**: full clang.wasm は数時間。
- 出力取り出しの hex は暫定（raw stdout の 0x00 化け回避）。WASI 直書き等に置換余地。

関連: #220（本方針）, #219, #215, #204（exnref 統一）, #205。
