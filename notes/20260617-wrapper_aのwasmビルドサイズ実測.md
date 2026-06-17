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

## #2 の可否: ユーザー環境＋最低限の道具で `libcadrum_cpp.a` を生成できるか（Web 調査 2026-06）

「wasi-sdk-33（Windows 625 MB）をユーザーに強いず、ユーザー環境＋最低限の道具で wrapper の `.a` を生成する」案 (#2) の可否を調査した。

### 結論
**実質的に「最低限」にはならない。** 本質的ブロッカーは **exnref EH 結合**。

### 根拠
- `cpp/wrapper.cpp` は例外を多用（`try`/`catch`/`throw` 系 74 箇所、OCCT `Standard_Failure` をエラー変換）→ **`-fwasm-exceptions` 必須**。`-fno-exceptions` では成立しない。
- cadrum は **新 exnref エンコーディング**を強制している（`build.rs:170` の `-wasm-use-legacy-eh=false`）。この指定は **LLVM ≥ 20.1** でのみ有効（legacy EH は LLVM 20.1 で `-wasm-use-legacy-eh` として残置され、ブラウザ既定の都合で当面 ON）。
- legacy EH と exnref EH のオブジェクトは**同一モジュールに混在不可**。よってローカルビルドの wrapper は、prebuilt OCCT（wasi-sdk-33 / clang 22, exnref）と一致する **exnref 対応 clang（≥20.1）** が必須。

### wasi-sdk-33 リリース実測サイズ（GitHub API）
| アセット | サイズ |
|---------|-------|
| フル SDK x86_64-windows | **625 MB** |
| フル SDK x86_64-linux | 184 MB |
| フル SDK x86_64-macos | 175 MB |
| **wasi-sysroot 単体**（ヘッダ + prebuilt libc/libc++ eh 版） | **118 MB** |
| prebuilt wrapper `.a`（#1, release） | **0.42 MB** |

### 選択肢の評価
| 経路 | 必要なローカル道具 | DL 量 | 判定 |
|------|------------------|------|------|
| フル wasi-sdk（現状） | なし | 625 MB(win) | 動くが重い |
| **#2a** system clang≥20.1 + sysroot のみ | clang≥20.1 on PATH | 118 MB | clang を持つ環境なら可。Windows は稀。exnref バージョン差リスク |
| **#2b** zig `zig c++` 自己完結 | zig (~50–80 MB) | ~50–80 MB | wasm32-wasi の C++ 例外が未成熟（zig/LLVM の未解決 issue が 2026 も継続）。未実証 |
| **#2c** clang を wasm 化して実行 | wasm runtime | sysroot + clang.wasm | 異色・低速、結局 118 MB sysroot が必要 |
| **#1** prebuilt wrapper.a | なし | 0.42 MB | 極小。命名 scaffolding 済み（`build.rs:19/28` の `…-cadrum-0_8_11`） |

### 含意
- #2 は最小でも **118 MB sysroot + exnref 対応 clang(≥20.1)** が必要で「最低限の道具」とは呼べない。最小自己完結の zig は、よりによって wasm の C++ 例外が壊れている。
- libc++ ABI は sysroot ヘッダ由来なので、#2a でも **wasi-sdk-33 の sysroot を使えば** prebuilt OCCT と ABI 整合する（フロントエンドの clang が別物でも可）。残るリスクは exnref のクロスバージョンリンク（LLVM 20↔22）。
- 対して #1 は **0.42 MB を配るだけ**で、OCCT を既に prebuilt 配布している現行モデルの自然な延長（wrapper は cadrum バージョンで鍵付けされる 1 アーティファクトが増えるだけ）。**#1 は「ワークアラウンド」ではなく、ユーザー負担（625 MB→0.42 MB）の観点ではむしろ #2 より軽い。**

### 出典
- wasi-sdk releases（アセットサイズ）: <https://github.com/WebAssembly/wasi-sdk/releases/tag/wasi-sdk-33>
- LLVM 20.1.0 Release Notes（`-wasm-use-legacy-eh`）: <https://releases.llvm.org/20.1.0/docs/ReleaseNotes.html>
- WebAssembly EH proposal（exnref）: <https://github.com/WebAssembly/exception-handling/blob/main/proposals/exception-handling/Exceptions.md>
- zig / LLVM の wasm C++ 例外 issue: <https://github.com/ziglang/zig/issues/22629> , <https://github.com/llvm/llvm-project/issues/188077>
- wasi-sdk README（stock clang + sysroot で wasm ターゲット可）: <https://github.com/WebAssembly/wasi-sdk>
- clang を wasm 化（参考）: <https://wasmer.io/posts/clang-in-browser>
