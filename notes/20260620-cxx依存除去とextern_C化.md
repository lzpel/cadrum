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

## フォローアップ案: ffi.h / ffi.rs の片側自動生成

手書き化で失ったのは「1 定義から両側を生成する単一の真実の源」と「ABI 不一致のコンパイル時検証」。
これを取り戻す（=ミニ cxx を自前で持つ）案。

### 前提: proc-macro 単体では不可

`cpp/ffi.h` は **build.rs が wrapper.cpp をコンパイルする時点**で必要だが、proc-macro の展開は
build.rs より後（クレート本体コンパイル時）。間に合わない。生成するなら **build.rs 時点**で行う。
これは cxx が `cxx-build`（build.rs から `.rs` を再パースする別クレート）を持つ理由と同じ。

### 案A: build.rs で ffi.rs を解析して ffi.h を生成（自前ミニ cxx-build）

`syn` 等で `mod raw { extern "C" {…} }` を読み、Rust 型→C 型（`*mut TopoDS_Shape`→`TopoDS_Shape*`、
`usize`→`size_t`、`*mut *mut u64`→`uint64_t**` 等）に写して `ffi.h` を出力。

- 利点: 真実の源を `ffi.rs` に一本化。
- 欠点: `syn` を build-dep に復活（cxx-build を消した意義と一部相殺）。`#[cfg(feature=color)]` の
  選別や、extern ブロック外の `CReader/CWriter/CMeshData` を別途扱う必要。
  → 「自前 cxx-build」になりがちで、cxx を外した意義と一番ぶつかる。

### 案B: bindgen で ffi.h から Rust の raw ブロックを生成

`ffi.h`（手書き）を真実の源にし、build.rs で `bindgen` を回して `mod raw` と
`CReader/CWriter/CMeshData` を生成。手書きの raw ブロックは削除。

- 利点: C ABI の自然な源は C ヘッダ。bindgen は枯れている。**clang は wrapper.cpp ビルドで
  既に必須**なので追加ツール要求は実質ゼロ。最小実装で堅牢。
- 欠点: `bindgen`(+`clang-sys`) という重めの build-dep。color の `#ifdef` は bindgen 実行時の
  define で揃える必要。

### 案C: build.rs 内の「関数テーブル」から両方を生成（依存ゼロ）

関数定義を build.rs 内の Rust データ（`(name, ret, [args])` のリスト）として 1 か所に書き、
build.rs が **ffi.h（C 文字列）と raw.rs（`include!` する Rust extern）の両方**を文字列整形で出力。

- 利点: 真実の源が 1 つ、**新規依存なし**（「依存を減らす」方針に最も合致）。
- 欠点: テーブル DSL と 2 つのエミッタを自作・保守。

### 共通効果と見立て

- どの案でも「片側を生成」すれば、**ffi.h と wrapper.cpp の shim 定義の不一致を C++ コンパイラが
  弾く**（宣言 vs 定義）。生成側が ffi.rs/ffi.h 由来なので **Rust↔C++ の食い違いがコンパイル時に
  検出され**、cxx の安全性の大半が戻る（残る穴は型マッピング表自体の正しさのみ）。生成されるのは
  宣言だけで、shim の中身と Rust 安全ラッパは引き続き手書き。
- 今回の規模（FFI 面 1 つ・~75 関数・変更頻度低）なら手書き二面のままでも十分。ドリフトが
  保守の痛みになったら **案C（依存ゼロのテーブル生成）**が方針に最も合う。clang 前提を許容できるなら
  **案B（bindgen）**が最小実装で堅牢。**案A は非推奨**（自前 cxx-build 化）。
