# GATE C2 突破: 純WASI clang.wasm が bare wasmtime で valid な wasm object を生成

#220 (A/B)。**純WASI な clang.wasm（emscripten standalone+WasmFS, env=0, 63MB）が bare wasmtime 上で
C++ を valid な wasm relocatable object にコンパイルし、host が取り出せる**ことを実証。

## 結果
- `int f(){return 7;}` を stdin から compile → **378B の valid object**（magic `00 61 73 6d 01 00 00 00`、
  llvm-objdump で `file format wasm`、TYPE/IMPORT/FUNCTION/CODE/linking/producers/target_features セクション）。
- clang.wasm の imports は `wasi_snapshot_preview1` のみ（env=0）。wasmtime は `-W` 不要（exnref も外せた）。

## 根本原因（長時間の切り分けの結論）
- **emscripten WASMFS は stdio fd(fd1/fd2) 上の raw バイナリを壊す**（バイト欠落＋改行 0x0a 絡みの破損）。
  - 実証: 大バッファ raw write を fd1 に出すと 8192→6986 にバイト欠落＋00→0a。
  - 一方 **WASMFS のファイル write/read/pwrite/lseek は完全クリーン**（8KB往復一致）。
  - `fprintf`(テキスト)で **hex を fd2 に出すのはクリーン**（hex 文字は 0x0a を含まない）。
- 以前「object writer が壊す」と見えたのは、**取り出しが壊れた fd 経路を通っていた**ため。clang 自身・object writer・
  WASMFS ファイル書込はすべて正常（DIAG で /tmp の実体が valid 378B と確認）。
- 補足の決定打: 同一 clang を JS/NODERAWFS にリンクし直すと object は最初から valid（=runtime FS の問題と確定）。

## 動く構成（再現）
- clang.wasm: emscripten で LLVM/clang を `-sSTANDALONE_WASM -sWASMFS -sINITIAL_MEMORY=1GiB -sSTACK_SIZE=64MiB`、
  `LLVM_ENABLE_THREADS=OFF` `CLANG_SPAWN_CC1=OFF`、native tblgen、Release。exnref/sjlj 不要（CrashRecoveryContext の
  setjmp/longjmp は `__EMSCRIPTEN__` で無効化済み）。
- LLVM/clang パッチ:
  - `bit.h`: `__wasi__` を endian 分岐に追加（前段の wasi 試行由来、無害）。
  - `CrashRecoveryContext.cpp`: setjmp/longjmp を `__EMSCRIPTEN__` で無効化。
  - `clang/tools/driver/driver.cpp`: 末尾に「`CADRUM_EMIT_FD2`=<path> の object ファイルを
    `CADRUM_OBJ_BEGIN`/`END` で挟んで **hex テキストで fd2 へ fprintf**」。
- 実行: `wasmtime run --env CADRUM_EMIT_FD2=/tmp/out.o clang.wasm --target=wasm32-wasip1 -O1 -w -c -x c++ - -o /tmp/out.o`
  （source=stdin）。host: stderr から `CADRUM_OBJ_BEGIN..END` の hex を抽出→デコード = valid object。
- 検証ツール: `wasi-sdk-linux/bin/llvm-objdump -h`。

## 次（A3）
- 実 `cpp/wrapper.cpp` をコンパイルするには OCCT include＋cxx glue を **WASMFS に載せる**必要（読込はクリーン）。
  方式: 必要ファイルを tar→C 配列で焼き込み（or stdin tar）→ 起動フックで WASMFS 展開 → compile → object を hex で取り出し。
- その後 `.a` 化（host）→ rust-lld リンク（OCCT prebuilt＋eh runtime）→ `make check-wasm32-unknown-unknown`。
- 最終: build.rs に wasmtime クレート埋め込みで in-process 実行＝rustc-only。

## 資産
- `C:\Users\smith\wasi-sdk-build`: clang.wasm(63MB, build-emcc/bin) / emsdk / wasmtime / probe(so2..so8) /
  スクリプト(emcc-clang-cfg.sh, ninja-clang-emcc.sh, fd2test.sh, hextest.sh 等) / dec.o(検証済 valid object)。
- 関連: #215 #216 #219 #220、notes（純WASI化の実証 / 実ビルド成功 / object-writer診断 / GATE A FS壁 / メモリ~163MB）。
