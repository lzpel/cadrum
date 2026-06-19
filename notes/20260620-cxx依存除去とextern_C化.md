# cxx 依存除去と extern "C" 化 — 目的と達成内容

PR #232。OCCT FFI を `cxx` クレートから手書き `extern "C"` へ置き換えた作業の記録。

## 何がしたかったか（背景・動機）

- **`cxx` を依存に置く限り、cxx 自身の `src/cxx.cc` が全 consumer ビルドでコンパイルされる。**
  `cxx.cc` は `<iostream>` / `<string>` 等の libc++ ヘッダを include しており、これが
  wasm ビルドで wasi-sdk（libc++ ヘッダ）を要求し続ける一因だった。OCCT を prebuilt
  `.a` 化しても、wrapper.cpp を prebuilt 化しても、**cxx.cc のコンパイルだけは consumer
  ビルドに残る**。
- さらに **publish ライブラリとして downstream の cxx ビルドを制御する手段が無い**。
  `.cargo/config.toml [env]` も `[patch.crates-io]` も「ビルド root」でしか効かず、
  依存パッケージ側の設定は読まれない。build.rs から依存（cxx）の build.rs へ env を
  注入することも不可能（実行順序＝依存先が先、かつ API も無い）。
- 結論として、**downstream まで効かせて wasm 用 C++ コンパイラを選べるようにするには、
  cxx を依存から外して全 C++ コンパイルを cadrum 自身の `cc::Build` に集約するしかない。**

## 何を達成したか

- `cxx` / `cxx-build` の依存を**完全に削除**。
- OCCT FFI を手書き `extern "C"` に置換。
- **全 C++ コンパイルが cadrum の build.rs（`cc::Build`）に集約**され、wasm 向けコンパイラを
  build.rs ロジックで選べる土台ができた（AGENTS.md「依存を減らす」にも合致）。

## 設計

低リスクを優先し、**OCCT ロジック本体（`namespace cadrum` の関数群）はほぼ無改変**で、
marshaling を薄い shim 層に集約する方針を採った。

### C++ 側

- **`cpp/ffi.h`（新規）** — 手書き C ABI。
  - `CReader` / `CWriter`: streambuf コールバック（`{ ctx: void*, fn ポインタ }`）。
  - `CMeshData`: mesh 結果の POD 構造体。
  - `cadrum_free`（malloc 配列用 = free）/ `cadrum_shape_free` 等（所有ハンドル用 = delete）。
  - 戻り値が可変長のものは `(ptr, len)` を malloc して返す規約。
- **`cpp/wrapper.{h,cpp}`**
  - OCCT ロジックは温存。`rust::Vec<T>&` → `std::vector<T>&`、`rust::Slice<const T>` →
    `const std::vector<T>&`、`rust::Vec<T>` 戻り → `std::vector<T>` 戻り に置換しただけ
    （`.push_back` / `.size()` / `[i]` / `.clear()` は std::vector で同一なので本体は不変）。
  - 末尾に `extern "C"` **shim 層**を追加し、生 C ABI ↔ `cadrum::` 関数を marshaling。
  - streambuf は `rust_reader_read`/`rust_writer_write` 呼びを `CReader/CWriter` の
    fn ポインタ呼びに置換。
  - 不要化した **vec ヘルパ 7 個**（`*_vec_new`/`push`/`push_null`）と
    **`clone_edge_handle` / `clone_face_handle`** を削除（`clone_shape_handle` は残置）。

### Rust 側

- **`src/occt/ffi.rs`** — `#[cxx::bridge]` を撤去。
  - 不透明型 `TopoDS_Shape/Face/Edge`（`[u8;0]` + `PhantomData` イディオム）。
  - `Owned<T>`: `cxx::UniquePtr<T>` 相当の所有スマートポインタ。`Deref`（call site の
    deref 強制で `&Owned<T>` → `&T`）と `is_null`、`Drop` で `cadrum_*_free`。
  - 生 `extern "C"`（`mod raw`）+ 旧 bridge と同名・同シグネチャの安全ラッパ
    （→ 下流の `ffi::*` 呼び出しはほぼ無改変で済む）。
  - `MeshData` は平の Rust struct。
- **`src/occt/stream.rs`** — `#[repr(C)]` の `CReader`/`CWriter` と `extern "C"`
  トランポリンを追加（既存の reader/writer ロジックを再利用）。
- **caller 群**（`solid`/`compound`/`edge`/`face`/`io`）
  - wrapper struct のフィールド `cxx::UniquePtr` → `ffi::Owned`。
  - `CxxVector` 入力は `Vec<*const TopoDS_*>` を組んで渡す。**null ポインタ = loft/pipe の
    断面区切り**（旧 null-edge sentinel の置き換え）。
  - `CxxVector` 返り値は `Vec<Owned<T>>` を直接消費（要素ごとの clone 廃止）。

### 例外

- OCCT 呼び出しは既存の `try/catch(Standard_Failure)` が **36 箇所すべて**を覆っており、
  例外が FFI 境界を越えない。追加の境界実装は不要だった。

## 検証

- `cargo test`（default: color+png）green。
- `cargo test --no-default-features --lib --tests`（color off）green。
- `cargo build --examples`（default）OK、`cargo fmt` 適用。
- `cargo build --target wasm32-unknown-unknown`（wasi-sdk-33 + prebuilt wasm OCCT）で
  `wrapper.cpp` が clang++ でコンパイルされ rlib ビルド成功。

native のテストは全 FFI 関数を実リンク・実行するので ABI を end-to-end で検証済み。
wasm ビルドは wasi-sdk 下での C++ コンパイル成立を確認するもの。

## ハマり所メモ

- **wasm で `cstdint file not found`** が出たが、これは検証時に `--sysroot` を
  **Unix 形式パス（`/c/...`）** で渡したため native Windows clang が解決できなかっただけ。
  `cygpath -w` で Windows 形式（`C:\...`）に直すと解決。**コード側の問題ではない。**
  wasi-sdk の `clang++.cfg` が相対 sysroot を提供しており、デフォルト target
  `wasm32-unknown-wasip1` + `-fwasm-exceptions` で `wasm32-wasip1/eh/c++/v1` を自動解決する。
  `sandbox-wasm/makefile` の `SYSROOT := $(CURDIR)/...` も同じ落とし穴があり得る（要留意・本PR対象外）。
- `cargo test --no-default-features` で examples 09/10/12 がコンパイルエラーになるのは
  `.color()`/`write_png` を使う例に `required-features` が無い**既存問題**で、本変更とは無関係。

## 補足: 取らなかった代替案

- **cxx を fork（`sandbox-cxx`）して build.rs に wasm 分岐**: `package` リネームで `::cxx::`
  パスを維持すれば downstream にも伝播でき技術的には可能。ただし cxx fork の追従保守が
  発生し、かつ「consumer ビルドで C++ をコンパイルし続ける」点は変わらない。
- 本 PR の extern "C" 化なら **consumer ビルドの C++ を将来 prebuilt `.a` 化して
  toolchain ゼロにする**道まで開ける（cxx fork ではそこに届かない）。
