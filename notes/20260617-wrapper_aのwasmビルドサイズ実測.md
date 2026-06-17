# wrapper の wasm `.a` ビルドサイズ実測（debug / release）

## 背景

cadrum の `wasm32-unknown-unknown` ビルドは、`cpp/wrapper.h` / `cpp/wrapper.cpp` を
wasm アーキの静的ライブラリ（`build.compile("cadrum_cpp")` → `libcadrum_cpp.a`）に
コンパイルするため、ユーザー環境に **wasi-sdk-33** を要求する。
OCCT ランタイム（libc/libc++/libunwind 等、~120 MB）は wasi-sdk-33 ビルド済みを配布済み。

SDK ダウンロードをユーザーに強いない案として:
- **#1** `wrapper` の `.a` を GitHub Release で配布する
- **#2** 最小 llvm/clang をユーザー側で動かし `build.rs` で `.a` を生成する

#1 の判断材料として「`libcadrum_cpp.a` が何バイトか」を実測した記録。

## 計測手順

```sh
# 1. wasm ツールチェイン取得（wasi-sdk-33 Windows tarball + wasm-pack を out/ へ）
make -C sandbox-wasm download

# 2. sandbox-wasm/makefile の env を再現し、prebuilt OCCT を OCCT_ROOT で指定
#    （PATH 先頭に out/wasi-sdk-33/bin、SYSROOT 等は makefile 33-39 行と同一）
export PATH="$(pwd)/out/wasi-sdk-33/bin:$PATH"
export SYSROOT=".../out/wasi-sdk-33/share/wasi-sysroot"
export CC_wasm32_unknown_unknown=clang CXX_wasm32_unknown_unknown=clang++ CXXSTDLIB=c++
export CXXFLAGS_wasm32_unknown_unknown="--target=wasm32-wasip1 --sysroot=$SYSROOT -fwasm-exceptions -fexceptions -D_WASI_EMULATED_*"
export OCCT_ROOT=".../target/occt-8_0_0_rev2-wasm32_unknown_unknown"   # prebuilt、OCCT 再ビルド回避

cd sandbox-wasm
cargo build              --target wasm32-unknown-unknown --features cadrum   # debug
cargo build --release    --target wasm32-unknown-unknown --features cadrum   # release
```

生成物: `sandbox-wasm/target/wasm32-unknown-unknown/{debug,release}/build/cadrum-*/out/libcadrum_cpp.a`

`.a` は `wrapper.cpp` 単体ではなく **`wrapper.o` + cxx 自動生成ブリッジ glue `ffi.rs.o`** の 2 オブジェクト構成。
両者とも wasm マジック `00 61 73 6d` を確認済み。

## 結果（wasi-sdk-33 / clang 22.1.0, 2026-06-17）

| ビルド | `libcadrum_cpp.a` 全体 | `wrapper.o` | `ffi.rs.o` | DWARF |
|--------|----------------------:|------------:|-----------:|:-----:|
| debug                    | **3,765,242 B（≈3.59 MiB）** | 2,999,282 | 422,143 | あり |
| debug + `--strip-debug`  | 1,464,524 B（≈1.40 MiB）     | 1,002,358 | 118,332 | 除去 |
| **release**              | **419,488 B（≈0.40 MiB）**   |   311,511 |  33,772 | **なし** |

## 重要な発見: `--release` は C++ wrapper にも効く

「`--release` は Rust 側のリリースビルドであって C(++) には無関係では?」という疑問を検証した結果、**無関係ではない**。

- release の `.a` は debug の **約 1/9**（3.77 MB → 0.42 MB）。
- release オブジェクトには `.debug_*` セクションが **0 個**（debug は多数）。`--strip-debug` しても release はサイズ不変＝元から DWARF なし。
- 理由: **cxx-build が内部で使う cc crate（cc-rs）が、Cargo がプロファイル毎に設定する `OPT_LEVEL` / `DEBUG` を読んで C/C++ コンパイラのフラグへ翻訳する**。
  - debug: `OPT_LEVEL=0` → `-O0`、`DEBUG=true` → `-g`（DWARF 込み）
  - release: `[profile.release] opt-level = "s"` → `-Os`、`DEBUG=false` → `-g` なし
  - したがって `wrapper.cpp` / `ffi.rs.cc` の wasm オブジェクトは debug と release で別物になる。
- 補足: `[profile.release]` の `strip = true` / `lto = true` は **最終 Rust 成果物（.wasm）** に効くもので、中間生成物の `.a` には作用しない。`.a` が小さいのは上記 `-Os` + `-g` なしの効果。

## #1（wrapper.a を Release 配布）への含意

- サイズは release で ~0.42 MB、debug でも ~3.8 MB と、OCCT ランタイム（~120 MB）に比べ桁違いに小さい。
  **配布サイズは #1 のボトルネックにならない。**
- 真のボトルネックは依然として「`wrapper.cpp` の更新頻度（OCCT バージョンアップより高頻度）に追従して `.a` を再生成する CI/CD」。
- 配布するなら release（`-Os`・DWARF なし）の ~0.42 MB を基準に考えればよい。
