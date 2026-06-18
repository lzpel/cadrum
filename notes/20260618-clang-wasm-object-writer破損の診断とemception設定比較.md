# clang.wasm: wasm object writer だけが破損 / 診断と emception 設定比較

#220 (A) の到達点と、唯一残る破損の精密診断。

## 到達（再掲）
- **純 WASI な clang.wasm（63MB, import=WASI のみ, env=0）を emscripten standalone+WasmFS でビルド**、bare wasmtime で起動。
- フロントエンド/コード生成は正常: **`-S`(asm) も `-emit-llvm`(bitcode, magic `42 43 c0 de`) も完全に正しい**。
- 取り出し機構も実装済み: driver.cpp パッチ（`CADRUM_EMIT_STDOUT`）で「clang→/tmp の WasmFS ファイル→stdout へ fwrite」。
  setjmp/longjmp(CrashRecoveryContext) は `__EMSCRIPTEN__` で無効化済み（唯一の longjmp 利用箇所）。

## 唯一の破損: `-c`（wasm binary object）だけ壊れる
- `-c` 出力の先頭が `00 61 73 6d 01 00 00 00…` → `0a 61 73 6d 01 0a 0a 01…`（**0x00 が 0x0a 化**）。llvm-objdump で invalid。
- 切り分け（すべて実証）:
  - stdout への putchar/fwrite（**0x00 含む**、clang.wasm と同一リンク設定でも）→ **クリーン**。
  - WasmFS ファイルの write/read、**pwrite/lseek-write（object writer のパッチ相当）→ クリーン**。
  - `-S`(text)・`-emit-llvm`(bitcode) → **クリーン**。
  - `-o -`(非シーク stdout) と `-o /tmp/o.o`(シーク可 WasmFS) が **同一の壊れ方** → 破損は出力チャネルより前の
    **in-memory バイト構築**に在る（FS/seek 問題ではない）= **WasmObjectWriter 限定のミスコンパイル**。

## emception（既知の動作品）との設定差
emception の `build-llvm.sh`（LLVM `d5a963a`, clang;lld, Release, THREADS=OFF, native tblgen）は:
- **standalone でも WASMFS でもない**。**JS モード**（`-sEXPORTED_RUNTIME_METHODS=FS,PROXYFS,…`, `--js-library fsroot.js`,
  `ALLOW_MEMORY_GROWTH`）で **node/browser 実行**。exnref/SUPPORT_LONGJMP も不使用。
- パッチは llvm-project.patch（cc1 のプロセス分離を無効化 = 当方は `-DCLANG_SPAWN_CC1=OFF` 相当）と `-Dwait4=__syscall_wait4`。
- → emception の clang は **valid object を生成（lld がリンクできている）が node 前提**。当方の破損は
  **standalone / WASMFS / exnref(sjlj) いずれかのフラグ**に起因する可能性が高い（emception はこれらを使わない）。

## 次の実験候補（各々フルリビルド=数時間）
1. **exnref/sjlj を外す**: clang は `-fno-exceptions`、longjmp 利用は patch 済み → `-sSUPPORT_LONGJMP=wasm` /
   `-sWASM_LEGACY_EXCEPTIONS=0` 不要。外せば exnref も消え、ミスコンパイル要因なら解消＋wasmtime も `-W` 不要に。
2. それでも破損 → **standalone/WASMFS 起因**を疑い、MC/object 関連 lib のみ `-O1`/`-O0` で再ビルドして切り分け。
3. 妥協案: emception 同様 **JS モード（node 実行）**にすれば valid object は得られる（rustc-only は崩れ node 依存）。

## 資産
- `C:\Users\smith\wasi-sdk-build`: clang.wasm(63MB) / emsdk / wasmtime / probe 群(so2..so5, bctest, gatec2*) /
  build スクリプト(emcc-clang-cfg.sh, ninja-clang-emcc.sh, relink-clang.sh, relink-and-test.sh)。
- LLVM パッチ: CrashRecoveryContext.cpp(setjmp/longjmp 無効化), driver.cpp(CADRUM_EMIT_STDOUT で出力ファイル→stdout), bit.h(__wasi__ endian)。
- 関連: #215 #216 #219 #220、notes（純WASI化の実証 / 実ビルド成功 / GATE A FS壁 / メモリ~163MB）。

## 追記: exnref/sjlj を外しても破損継続（=要因はそれらでない）
`-sSUPPORT_LONGJMP=wasm` / `-sWASM_LEGACY_EXCEPTIONS=0` を外して再リンク（clang は `-fno-exceptions`＋longjmp patch 済みで
不要）→ imports は WASI のみのまま（exnref 不要化に成功、wasmtime も `-W` 不要に）。しかし **`-c` object は同一に破損**
（`0a 61 73 6d…`, objdump invalid）。→ **EH/sjlj は無関係**。残る差は emception(JS モード, standalone/WASMFS 不使用) に対する
**standalone / WASMFS / -O3 のいずれか**。
次の決定的実験: emception 同等の **JS モード**で clang をビルドし `-c` object が valid か確認（valid なら standalone/WASMFS が要因と確定。
ただし JS モード=node 実行）。並行候補: `-fno-strict-aliasing` 等での全再ビルド（-O3 UB 起因の検証）。いずれも数時間級。
