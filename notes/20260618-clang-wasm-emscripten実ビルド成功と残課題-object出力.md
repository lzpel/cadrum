# clang.wasm（emscripten standalone, 純WASI）実ビルド成功 / 残課題は binary object の取り出し

#220 (A) の進捗。**純 WASI な clang.wasm を実ビルドし、bare wasmtime 上で C++ を実際にコンパイルできた**。
残るは「生成 object(.o) を非シーク stdout 経由で壊さず取り出す」点のみ。

## 達成（GATE C1 + C2 の大部分）

- **clang 22.1.0 を emscripten で standalone+WasmFS ビルド成功**（`/work/wasi-sdk/build-emcc/bin/clang.wasm`, 63MB）。
  - emcmake configure 成功（musl が POSIX を満たし、前回 wasi-libc で詰まった sigaction 等の壁を回避）。
  - リンク時の唯一の undefined `emscripten_longjmp`（CrashRecoveryContext.cpp）→ 同ファイルの setjmp/longjmp を
    `#ifdef __EMSCRIPTEN__` で無効化（crash recovery 不要）して解決。
  - `-sALLOW_MEMORY_GROWTH` を外し固定メモリ化（`-sINITIAL_MEMORY=1GiB -sSTACK_SIZE=64MiB`）して
    **唯一の env import `emscripten_notify_memory_growth` も除去** → **import は `wasi_snapshot_preview1`(9) のみ＝純 WASI**。
- **bare wasmtime（`-W exceptions=y -W function-references=y -W gc=y`）で clang.wasm が起動・動作**:
  - `-S`（テキスト asm）を **stdin→stdout で完全に正しく出力**（`int f(){return 1;}` → 正しい wasm asm）。
    = フロントエンド＋コード生成は健全。

## 残課題（唯一）: binary object を stdout で取り出すと壊れる

- `-c -o -`（または `-o /dev/stdout`）で **object の先頭マジック等が壊れる**（`00 61 73 6d…` → `0a 61 73 6d…`、
  llvm-objdump で invalid）。
- 切り分け結果（すべて実証）:
  - stdout への putchar/fwrite（**0x00 を含むバッファ**）は**クリーン**。
  - WasmFS ファイルへの 0x00 含む write→read も**クリーン**。
  - `-S` テキスト出力は stdout でクリーン。
  - `lseek(stdout)` は**正しく ESPIPE(-1)** を返す（seek 誤検出ではない）。
- → 壊れるのは **LLVM の binary object writer が非シーク stdout へ書く最終経路に限定**。
  シーク可能な **WasmFS ファイルへ `-o /out.o`** すれば正しい object が得られるはず（要・取り出し）。

## 取り出しの設計（次段）

- emscripten の WasmFS は **wasmtime の `--dir` preopen に繋がらない**（GATE A で実証）。埋め込み wasmtime でも同様。
  よって host が object を受け取る口は **stdout（fwrite はクリーン）** に限る。
- 解: **custom entry** — clang を呼んで `/out.o`(WasmFS, シーク可) に書かせ、その後 `/out.o` を読んで **fwrite で stdout** へ。
  - clang を関数として呼ぶ必要（clang をライブラリ/driver として組み込む。emception 類似）。
  - これで `clang.wasm < src > out.o` 相当が成立。最終的に build.rs の埋め込み wasmtime が stdin(tar)/stdout を供給。

## 状態・資産
- `C:\Users\smith\wasi-sdk-build`: emsdk / wasmtime / `wasi-sdk/build-emcc/bin/clang.wasm`（純WASI 63MB）/
  各 probe（so*.cpp, lseek_probe, gatec2*.sh）/ build スクリプト（emcc-clang-cfg.sh, ninja-clang-emcc.sh, relink-clang.sh）。
- LLVM パッチ: `wasi-sdk/src/llvm-project/.../CrashRecoveryContext.cpp`（setjmp/longjmp を emscripten 無効化）、
  `bit.h`(__wasi__ endian, 前回)。
- 関連: #215 #216 #219 #220、notes（純WASI化の実証 / GATE A FS壁 / wasi-libc 移植停止 / メモリ~163MB）。

## 残リスク
- custom entry（clang を in-process 呼び出し＋出力ファイルを stdout へ）の実装コスト。
- その後 A3（wrapper.cpp を実コンパイル: OCCT ヘッダ等を stdin-tar で WasmFS 展開）→ clang の多数 open/stat が成立するか。
