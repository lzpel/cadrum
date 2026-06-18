# approach 2: 純WASI な clang.wasm を bare wasmtime で動かし wasm object を生成する（全記録）

issue #220 (#215 の approach 2)。**wasm 化した clang を配布し、consumer 側で `cpp/wrapper.cpp` を
コンパイル → wasi-sdk 不要**にする試みの全記録。前段（wasi-libc 経路の行き詰まり・メモリ実証 ~163MB）は
#216 / 別 notes 参照。本書は **emscripten 経路**で「純WASI な clang.wasm が bare wasmtime 上で
valid な wasm object を生成・取り出せる（GATE C2）」までを、**上から下へ問題解決に近づく順**で記す。

## 0. 前提・環境
- docker `ubuntu:24.04`、`/work`=`C:\Users\smith\wasi-sdk-build`（mount, 再開用に残置）。
- emsdk（emcc 6.0.0）/ wasmtime 45.0.2 を `/work` 配下に導入。LLVM/clang ソースは wasi-sdk 同梱の
  `wasi-sdk/src/llvm-project`（LLVM 22.1.0）。
- import 列挙は emsdk 同梱 node ＋ `--experimental-wasm-exnref`（exnref を使うモジュール用）。
- 検証は `wasi-sdk-linux/bin/llvm-objdump -h`。

なぜ emscripten か: wasi-libc は POSIX を欠く（sigaction/fork 等）ため mainline LLVM を wasm-host へ
移植すると多数パッチが要り停止（#216）。**emscripten の musl は POSIX を持つ**ので LLVM をソース改変
ほぼ無しでビルドできる。実行は exnref EH 対応の wasmtime(2025) を用いる。

---

## 1. GATE A — 最初の壁: standalone の FS が wasmtime preopen に繋がらない
最小 `gatea.cpp`（fopen→read→`try{throw}catch`→write）で検証:

| 構成 | 実行 | fopen |
|---|---|---|
| 既定(legacy EH) | 不可 | wasmtime45 は新 exnref のみ（"exceptions proposal not enabled"/"legacy_exceptions required"） |
| `-sWASM_LEGACY_EXCEPTIONS=0`(exnref)＋`-W exceptions/function-references/gc` | **起動・EH OK** | 全パス失敗 |
| default FS, `--dir=/work` 他3方式 | 起動 OK | 一律 **errno=63 (WASI EPERM)** |
| `-sWASMFS`, `--dir` | 起動 OK | 一律 **errno=44 (WASI ENOENT)** |

- パス候補 × preopen 方式を総当たりしても開けない＝ **emscripten-standalone は wasmtime の `--dir`
  preopen を自身の FS に接続しない**（公式注記「standalone の FS は basic、wasmtime には wasi-sdk 推奨」の実証）。
- ただし **exnref EH は wasmtime で動く**ことを確認（ブロッカーは EH でなく FS）。
- 含意: 入力ヘッダは preopen では渡せない → **wasm 内に焼き込む / stdin で流す**必要がある。

---

## 2. 純WASI化の実証 — `--embed-file` を避ければ env import 0、in-wasm FS は動く
- `memfs_probe.cpp`（in-wasm WasmFS に fopen("w")→write→read、preopen 不使用）を
  `-sSTANDALONE_WASM -sWASMFS`(他EH等)でビルド → **imports = `wasi_snapshot_preview1` のみ（env=0）**、
  bare `wasmtime`（`--dir` 無し）で `READBACK[...]=[INWASM-FS-OK]` `EH=7` 成立。
- 以前観測した `env`(7)（`_emscripten_fs_load_embedded_files`/`_wasmfs_jsimpl_*`）は **`--embed-file` 由来**。
  外すと WasmFS は **純 wasm の memory backend** を使い JS hook が消える。
- `probeB.cpp`: データを `xxd -i` で **C 配列に焼き込み**→起動時に in-wasm で WasmFS へ展開→読取も
  **env=0 維持**で成功。
- → **純WASI 化は成立**: `--embed-file` を避け WasmFS memory backend ＋ C配列焼き込み ＋ WASI stdio に
  すれば、import は WASI のみ。bare wasmtime でも build.rs 埋め込み wasmtime でも動く。

---

## 3. clang.wasm 実ビルド成功（GATE C1）
- emscripten で LLVM/clang 22.1.0 を **standalone+WasmFS** クロスビルド成功
  （`build-emcc/bin/clang.wasm`, **63MB**）。
  - `emcmake` の configure 成功（musl が POSIX を満たし、wasi-libc で詰まった sigaction 等の壁を回避）。
  - 設定: `LLVM_ENABLE_PROJECTS=clang` / `LLVM_TARGETS_TO_BUILD=WebAssembly` /
    `LLVM_ENABLE_THREADS=OFF` / `CLANG_SPAWN_CC1=OFF` / native `llvm-tblgen`,`clang-tblgen` / Release。
  - リンク唯一の undefined `emscripten_longjmp`（`CrashRecoveryContext.cpp`）→ 同所の setjmp/longjmp を
    `#ifdef __EMSCRIPTEN__` で無効化（crash recovery 不要）して解決。
  - `-sALLOW_MEMORY_GROWTH` を外し固定メモリ（`-sINITIAL_MEMORY=1GiB -sSTACK_SIZE=64MiB`）→ 唯一の env
    import `emscripten_notify_memory_growth` も除去 → **imports = `wasi_snapshot_preview1` のみ＝純WASI**。
- bare wasmtime で **`-S`（テキスト asm）を stdin→stdout で完全に正しく出力**（フロントエンド＋コード生成は健全）。

---

## 4. 長い切り分け: 「object(`-c`) だけ壊れる」謎
- `-c -o -`（binary object）で先頭が `00 61 73 6d…` → `0a 61 73 6d…` に化け、llvm-objdump で invalid。
- 順次つぶした（すべて実証）:
  - `-S`(asm)・`-emit-llvm`(bitcode, magic `42 43 c0 de`) は **クリーン** → 出力全般・メモリは健全。
  - stdout への putchar/fwrite（0x00 含む, clang と同一リンク設定でも）→ クリーン。
  - WasmFS ファイルの write/read/**pwrite/lseek**（object writer のパッチ相当）→ クリーン。
  - `lseek(stdout)` は正しく ESPIPE → seek 誤検出ではない。
  - exnref/`-sSUPPORT_LONGJMP=wasm` を外しても破損継続 → **EH/sjlj は無関係**（外せたので以後 exnref 不要・`-W` 不要）。
- emception（既知の動作品, `build-llvm.sh`）との差分: emception は **JS モード**（`-sEXPORTED_RUNTIME_METHODS=FS,PROXYFS`,
  `--js-library fsroot.js`, ALLOW_MEMORY_GROWTH, node 実行）で **standalone/WasmFS を使わない**＝valid object を生成。
  → 当方の破損は **standalone/WASMFS 由来**と絞れた。
- 決定打: 同一 clang を **JS/NODERAWFS にリンクし直すと object は最初から valid**（`00 61 73 6d…`, objdump OK,
  378B, セクション健全）。compile は同一なので **compile-time でなく runtime(FS) の問題**と確定。

---

## 5. 根本原因の特定と回避（GATE C2 突破＝解決）
- 大バッファの raw write を **WASMFS の fd1(stdout) に出すと 8192→6986 にバイト欠落＋00→0a** で破損。
  一方 **WASMFS のファイル write/read/pwrite/lseek は 8KB 往復まで完全クリーン**、**fd2 への fprintf(テキスト)も
  クリーン**（hex 文字は 0x0a を含まない）。
- = **emscripten WASMFS は stdio fd 上の raw バイナリを壊す**（改行 0x0a 絡みの欠落）が、**ファイルと
  fd2-テキストは無傷**。「object writer が壊す」と見えていたのは、**取り出しが壊れた fd 経路を通っていた**ため。
  DIAG で clang が書いた `/tmp/out.o` の実体は **valid 378B**（`00 61 73 6d 01 00 00 00…`）と確認。
- **回避策**: clang に object を WasmFS ファイル(`-o /tmp/out.o`, シーク可・クリーン)へ書かせ、`driver.cpp` の
  パッチで **その内容を hex テキスト化して fd2 へ**（`CADRUM_OBJ_BEGIN`/`END` で囲む）。host は stderr から
  マーカー間 hex を抽出→デコード。
- **結果（GATE C2 突破）**: `int f(){return 7;}` を stdin から compile → host で **valid な 378B wasm object**
  を取得。`llvm-objdump -h` で `file format wasm`、TYPE/IMPORT/FUNCTION/CODE/linking/producers/target_features。
  clang.wasm の imports は **WASI のみ（env=0）**、wasmtime は `-W` 不要。
  → **純WASI な clang.wasm が bare wasmtime 上で C++ を valid な wasm object にコンパイルし、host が取り出せる**。

---

## 6. 動く再現構成（決定版）
- **clang.wasm ビルド**: emscripten で LLVM/clang を
  `-sSTANDALONE_WASM -sWASMFS -sINITIAL_MEMORY=1073741824 -sSTACK_SIZE=67108864`、
  `LLVM_ENABLE_THREADS=OFF` `CLANG_SPAWN_CC1=OFF`、native tblgen、Release。exnref/sjlj 不要。
- **LLVM/clang パッチ**:
  - `llvm/include/llvm/ADT/bit.h`: endian 分岐に `|| defined(__wasi__)`（前段 wasi 試行由来・無害）。
  - `llvm/lib/Support/CrashRecoveryContext.cpp`: setjmp/longjmp を `__EMSCRIPTEN__` で無効化。
  - `clang/tools/driver/driver.cpp`: 末尾で env `CADRUM_EMIT_FD2`=<path> の object を
    `CADRUM_OBJ_BEGIN`/`END` で挟み **hex で fd2 へ fprintf**（WASMFS の fd バイナリ破損回避）。
- **実行**: `wasmtime run --env CADRUM_EMIT_FD2=/tmp/out.o clang.wasm --target=wasm32-wasip1 -O1 -w -c -x c++ - -o /tmp/out.o`
  （source=stdin）。host: stderr の `CADRUM_OBJ_BEGIN..END` を抽出→hex デコード = valid object。
- **資産**: `C:\Users\smith\wasi-sdk-build`（clang.wasm 63MB / emsdk / wasmtime / probe so2..so8 /
  スクリプト emcc-clang-cfg.sh, ninja-clang-emcc.sh, fd2test.sh, hextest.sh 等 / 検証済 dec.o）。

---

## 7. 残り（A3 以降）
1. **実 `cpp/wrapper.cpp` のコンパイル**: clang が読む **wasi-sdk-33 sysroot＋libc++ ヘッダ / OCCT include /
   cxx glue / wrapper.cpp** を **WASMFS に投入**（読込はクリーン）。tar→C配列 焼き込み（or stdin tar）＋起動展開。重い実コンパイル（~163MB）。
2. `.o`→`.a`（host の ar）→ rust-lld で OCCT prebuilt＋eh runtime とリンク（ABI 確認）。
3. `make check-wasm32-unknown-unknown`（node で `Solid volume: 6000` / NODETEST:OK）。
4. 最終: build.rs に `wasmtime` クレートを埋め込み in-process 実行 ＝ **consumer は rustc-only**。

残リスク: ヘッダ群の WASMFS 投入規模・起動展開フック、重い実コンパイルの時間/メモリ、生成 `.a` の ABI 整合。

関連: #215 / #216（前半・wasi-libc 停止・メモリ~163MB）/ #217(マージ済) / #219・#220（方針）/ #224（本記録の PR）。
