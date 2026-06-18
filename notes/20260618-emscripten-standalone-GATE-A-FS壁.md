# approach 2（emscripten 経路）GATE A: exnref EH は OK、standalone FS が wasmtime で繋がらず停止

#219 の方針（Emscripten-standalone clang.wasm を wasmtime で実行）の最初の関門 GATE A
（standalone wasm が wasmtime preopen で file I/O できるか）を実機検証した記録。

## 環境
- docker ubuntu:24.04、`/work`=`C:\Users\smith\wasi-sdk-build` mount。
- emsdk（emcc 6.0.0）と wasmtime 45.0.2 を `/work` 配下に導入（再利用可）。
- 最小プログラム `gatea.cpp`: 複数パス候補を fopen → 読取 → `try{throw}catch` → 別ファイルへ書込。

## 結果

| 構成 | 実行 | fopen 結果 |
|---|---|---|
| 既定フラグ（legacy EH） | **不可** | wasmtime: "exceptions proposal not enabled" / "legacy_exceptions required"（45 は新 exnref のみ） |
| `-sWASM_LEGACY_EXCEPTIONS=0`（exnref）＋ `-W exceptions=y -W function-references=y -W gc=y` | **起動・EH 動作 OK** | 全パス失敗 |
| default FS, `--dir=/work` ほか3方式 | 起動 OK | 全候補 **errno=63（WASI EPERM）** |
| `-sWASMFS`, `--dir=/work`/`.` | 起動 OK | 全候補 **errno=44（WASI ENOENT）** |

- パス候補（`/work/...`, `gatea_in.txt`, `./...`, `/...`）× preopen 方式（`--dir=/work`, `::/`, `.::/work`）を総当たりしても、
  **default FS は一律 EPERM、WasmFS は一律 ENOENT**。
- = **emscripten-standalone は wasmtime の `--dir` preopen を自身の FS に接続しない**（公式注記「standalone の FS は basic、
  wasmtime なら wasi-sdk 推奨」の実証）。

## 確定した本質的トレードオフ

- ✅ **exnref EH は wasmtime で動く**（`-sWASM_LEGACY_EXCEPTIONS=0` ＋ `-W exceptions/function-references/gc`）。
  ユーザ指摘どおり「standalone＋wasmtime＋exnref」の実行前提は成立。**ブロッカーは EH ではなく FS**。
- ❌ **emscripten clang は実 FS に node が必要**（standalone FS が wasmtime preopen 非対応 → NODERAWFS/JS FS＝node）。
  emception が JS モードなのもこの理由。
- 対して **wasi-sdk clang は wasmtime 上 FS が堅牢**（wasi-libc が preopen を populate）だが、**mainline LLVM の
  ビルドに POSIX 移植が必要**（前回 notes: sigaction 等）。
- → 「ビルド＝musl POSIX が欲しい」「実行時 FS＝wasi-libc が欲しい」が単一既製経路で両立しない。

## 選択肢

1. **emscripten clang を node 実行**（emception 流, JS/NODERAWFS）: FS 実績あり・最短で FFI/`make check` まで到達見込み。
   ただし build 時に **node 依存**（埋め込み wasmtime/rustc-only では無い）。cadrum-wasm-example は既に node 前提。
2. **wasi-sdk clang ＋ LLVM POSIX パッチ**（前回経路の継続）: wasmtime FS 堅牢・埋め込み wasmtime/rustc-only に最適だが、
   sigaction 等を要パッチ（ファイル数は未確定だが各々小）。
3. 記録して停止。

## 状態
- ビルド資産（emsdk/wasmtime/各 gatea.* ）は `C:\Users\smith\wasi-sdk-build` に残置（再開可）。
- 関連: #215, #216, #219、前回 notes（wasi-libc 移植停止 / メモリ ~163MB）。
