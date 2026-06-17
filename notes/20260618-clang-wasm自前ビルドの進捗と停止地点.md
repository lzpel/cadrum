# clang.wasm 自前ビルド（LLVM→wasm クロス）: 進捗と停止地点

#215 approach 2 を実走するため、wasi-sdk-33 の llvm-project を **wasm ホストへクロスビルド**して
clang.wasm を得る試み。**クロス configure まで成功**し、LLVMSupport を ~55/2811 までコンパイル
した時点で、**wasi-libc が POSIX を欠く（sigaction/fork/execve 等）**ため mainline LLVM の
多ファイル移植が必要と判明し、ユーザ判断で一旦停止。

## 環境 / 再現手順

- ソース: `git clone --recursive https://github.com/WebAssembly/wasi-sdk` → checkout
  `c10c0507deb3e5aad506f1f9f32084e49a21834b`（submodule: config, wasi-libc, llvm-project=**LLVM 22.1.0**）。
  ローカル配置: `C:\Users\smith\wasi-sdk-build\wasi-sdk`（cadrum レポ外。再開用に残置）。
- ビルド: docker `ubuntu:24.04`、`/work`=上記ソース mount。ビルドコンパイラは
  **wasi-sdk-33(Linux) の clang-22**（`/work/wasi-sdk-linux`、builtins・sysroot・cmake toolchain 同梱）。
- 2 段構成:
  1. **native tablegen**: `cmake -S llvm -B build-native -DLLVM_ENABLE_PROJECTS=clang -DLLVM_TARGETS_TO_BUILD=WebAssembly` → `ninja llvm-tblgen clang-tblgen`。
  2. **wasm クロス**: `CMAKE_TOOLCHAIN_FILE`=wasi-sdk の `wasi-sdk-p1.cmake`（`WASI_SDK_PREFIX` 指定、`Platform/WASI.cmake` を module path に）、
     `CMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY`、`LLVM_HOST_TRIPLE=wasm32-wasip1`、
     `LLVM_TABLEGEN/CLANG_TABLEGEN`=native、`LLVM_ENABLE_THREADS=OFF`、`CLANG_SPAWN_CC1=OFF`
     （WASI に exec 無 → cc1 を in-process 化）、`LLVM_TARGETS_TO_BUILD=WebAssembly`、
     `_WASI_EMULATED_{SIGNAL,MMAN,PROCESS_CLOCKS,GETPID}` を CXXFLAGS、対応 emulated lib をリンク。

## 必要だったパッチ（クロス成立のため）

1. **`cmake/Platform/WASI.cmake` に `set(UNIX 1)`**: LLVM `HandleLLVMOptions.cmake` が
   WIN32/UNIX/CYGWIN/Generic 以外を "Unable to determine platform" で拒否。WASI は POSIX 風なので UNIX 扱い。
2. **`llvm/include/llvm/ADT/bit.h`**: endianness 分岐の最初の `#if` に `|| defined(__wasi__)` を追加。
   未対応だと `#else` で不在の `<machine/endian.h>` を引く（wasi-libc は `<endian.h>` を持つ）。

## 到達点

- ✅ **クロス configure 成功**（`CONFIGURE_OK`、Clang 22.1.0 / Targeting WebAssembly）。
- ✅ `bit.h` パッチ後、LLVMSupport を **~55/2811** までコンパイル。
- ❌ `lib/Support/CrashRecoveryContext.cpp` で停止: `struct sigaction` 不完全 + `sigaction()` 未提供。
  wasi-sysroot の `signal.h` で **`sigaction()` は `#ifdef __wasilibc_unmodified_upstream /* WASI has no signals */` 内**
  ＝ **wasi-libc は sigaction を提供しない**。`_WASI_EMULATED_SIGNAL` でも `signal()`/`raise()` のみ。

## 結論 / 推奨

- mainline LLVM 22 の Unix サポート層（`CrashRecoveryContext`・`Signals.inc`・`Program.inc` の fork/execve・
  `Process.inc`・`Path.inc`・`Memory.inc`・`DynamicLibrary` 等）は wasi-libc に無い POSIX API を多用。
  **wasm ホスト化には多数のソースパッチ＝ mainline LLVM の wasi 移植**が必要（Wasmer が fork で維持する規模）。
  emulated shim（signal/mman/getpid/clocks）では sigaction/fork/exec は埋まらない。
- 推奨: ①既存の **LLVM-WASI fork/パッチ群（Wasmer 等）** を採用して clang.wasm を得る、または
  ②本移植を独立した大規模タスクとして計画・実施。**メモリ関門と consumer 経路（cxx_build skip + prebuilt FFI link）
  は別途実証/設計済み**なので、適切な clang.wasm さえ得られれば approach 2 は完成可能。

## 状態
- ビルドツリー `C:\Users\smith\wasi-sdk-build`（数 GB）は再開用に残置。スクリプト:
  `wasi-sdk/cross-configure.sh`, `wasi-sdk/reconfig-and-build.sh`, `wasi-sdk/build-clang.sh`。
- 関連: 別ノート（メモリ実測 ~163MB / GATE1 ブロック）、#215, PR #216。
