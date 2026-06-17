# build.rs の各関数とOCCT_ROOT への影響

`build.rs` の各関数が負う責任と、それによって `OCCT_ROOT`（= コード中の `effective_root` ディレクトリ）の中身に**書き込み／削除等の影響を与えるか否か**を関数単位で列挙する。

前提:
- `OCCT_ROOT` 環境変数が指すディレクトリ（未指定なら `<target_dir>/<release_name>`）が `effective_root`。`find_occt_dirs` 等が探すのはこのディレクトリ配下の `include*`／`inc`／`lib`／`win64/.../lib`。
- 「影響あり」= そのディレクトリの中にファイル／ディレクトリを作成・移動・削除する、の意。

---

## 影響しない（純粋・読み取り・メタ出力のみ）関数

### `release_name(target, has_version)`
- **責任**: `OCCT_VERSION` + `BUILD_REVISION`（+ target +crate version）から、GitHub Release タグ / tarball 名 / キャッシュディレクトリ名を導出する純粋関数。
- **OCCT_ROOT への影響**: なし。文字列を組み立てるだけ。ただし戻り値が `effective_root` の**既定パス名**（`cargo_target_dir/release_name(...)`）を決めるので、間接的に「どこを OCCT_ROOT とみなすか」を左右する。

### `cargo_target_dir(target)`
- **責任**: `OUT_DIR` の階層から cargo の target ディレクトリを逆算する純粋関数。
- **OCCT_ROOT への影響**: なし。パス計算のみ。`OCCT_ROOT` 未指定時の既定ルートの**親**を決めるだけで、中身には触れない。

### `find_occt_dirs(occt_root)`
- **責任**: 与えられたルート配下から include ディレクトリと lib ディレクトリの候補を探し、両方存在すれば `[include, lib]` を返す（プローブ）。
- **OCCT_ROOT への影響**: なし。`exists()` で探索するだけ（読み取り専用）。

### `apply_compiler_flags(apply)`
- **責任**: target 条件付き C++ コンパイラフラグ（MSVC `/utf-8`、wasm の exnref EH エンコーディング）を `apply` クロージャ経由で wrapper / OCCT 両方に同一適用する。
- **OCCT_ROOT への影響**: なし。コンパイラフラグを渡すだけ。

### `download_and_extract_tar_gz(url, dest)`
- **責任**: URL から取得した gzip+tar を `dest` に展開する汎用ヘルパ。
- **OCCT_ROOT への影響**: `dest` の中身に**書き込む**（展開）。ただし呼び出し側が渡す `dest` 次第。`occt_from_prebuilt` からは `effective_root.parent()`、`occt_from_source` からは `effective_root` 直下に展開される（→ 呼び出し側の項で扱う）。

### `fetch_bytes(url)`
- **責任**: `file://` ならローカル読み取り、それ以外は HTTP GET でバイト列を取得する。
- **OCCT_ROOT への影響**: なし。ネットワーク／読み取りのみ。

---

## 影響しうる／影響する関数

### `main()`
- **責任**: build 全体のエントリ。rerun トリガ出力、`DOCS_RS` 早期 return、`OCCT_ROOT` の解決（相対→絶対化）、`resolve_occt` → `link_occt_libraries` の呼び出し。
- **OCCT_ROOT への影響**: 自身では書かないが、`effective_root` を確定して下流に渡す。`OCCT_ROOT` 環境変数があればそれを（相対なら CWD 起点で絶対化して）使い、無ければ既定キャッシュパスを採用する。

### `resolve_occt(effective_root, target)`
- **責任**: include/lib の解決戦略の分岐点。
  1. キャッシュヒット（`find_occt_dirs` 成功）→ そのまま使う
  2. ミス + `source` feature → `source::occt_from_source` でソースビルド
  3. ミス + feature なし → `occt_from_prebuilt` でプリビルド tarball 取得
- **OCCT_ROOT への影響**:
  - キャッシュヒット時: **なし**（既存をそのまま返す）。
  - ミス時: 下流（`occt_from_source` / `occt_from_prebuilt`）経由で `effective_root` に**生成・展開する**。
  - 同梱 (`bundle_runtime_libs`) はここではなく `main` に移動済み（`resolve_occt` は dirs を返すだけ）。

### `link_occt_libraries(occt_include, occt_lib_dir)`
- **責任**: `rustc-link-search` / `rustc-link-lib` の出力、lib ディレクトリ内の `cadrum` を含む static lib の追加リンク、mingw 向けリンクフラグ、`cxx_build` による `cpp/wrapper.cpp` のコンパイル（`cadrum_cpp`）。
- **OCCT_ROOT への影響**: **なし（読み取りのみ）**。`occt_lib_dir` は `WalkDir` で**走査して読むだけ**。wrapper のコンパイル成果物は `OUT_DIR` 側に出るので OCCT_ROOT は汚さない。

### `occt_from_prebuilt(effective_root, target)`  〔`not(feature = "source")`〕
- **責任**: target 用プリビルド tarball を URL（既定は GitHub Release、`CADRUM_PREBUILT_URL` で上書き可）から `effective_root` の親に展開し、トップレベルディレクトリを `effective_root` へ rename する。
- **OCCT_ROOT への影響**: **大きい**。
  - `effective_root.parent()` を `create_dir_all` で作成。
  - 親に tarball を展開（`occt-..._<target>/` ができる）。
  - 展開ディレクトリが `effective_root` と異なれば、**既存 `effective_root` を `remove_dir_all` で消してから** rename で置き換える。
  - 結果として `OCCT_ROOT`（=effective_root）の中身が tarball の内容で**新規生成／上書き**される。

### `bundle_runtime_libs(occt_lib_dir, libs)`  〔`feature = "source"`、`main` から呼ばれる〕
- **責任**: ホストツールチェインのランタイム（現 policy: GCC の `libstdc++.a` / `libgcc.a` / `libgcc_eh.a`）をコンパイラの `-print-file-name` で見つけ、OCCT lib ディレクトリへ `libcadrum_*.a` としてコピーする。`CADRUM_BUNDLE_RUNTIME` 指定 + GNU target のときのみ（prebuilt tarball 作成 recipe 専用）。`resolve_occt` ではなく `main` で `resolve_occt` の戻り値 lib に対して実行され、prebuilt/source 両分岐に対して対称的な位置になった。
- **OCCT_ROOT への影響**: **あり**。`occt_lib_dir`（= OCCT_ROOT 配下の lib）へ `libcadrum_stdc++.a` 等を**追加コピー**する。

---

## `mod source`（`feature = "source"` 時のみ）

### `source::occt_from_source(effective_root)`
- **責任**: OCCT ソースを DL → パッチ適用 → CMake ビルド → slim 化（不要物削除）→ LGPL 2.1 §2 のため改変ファイルのみ残す、までの一連。
- **OCCT_ROOT への影響**: **最も大きい。effective_root 全体を作り込む**。
  - 既にキャッシュがあれば（先頭の `find_occt_dirs`）早期 return = 影響なし。
  - sentinel `.occt_extraction_done` が無ければ: `effective_root` を作成、既存の `OCCT*` 部分展開ディレクトリを**削除**、ソース tarball を `effective_root` 直下に**展開**、sentinel を**書き込む**。
  - `walk_occt_sources` 経由でソースファイルを**パッチ（上書き）**。
  - CMake が `CMAKE_INSTALL_PREFIX=effective_root` で **include/lib 等をインストール**。
  - slim 化: `effective_root/share`, `effective_root/bin` を**削除**。
  - lib ディレクトリから `OCC_LIBS` に一致しないもの（cmake/pkgconfig 含む）を**削除**。
  - LGPL: 改変していないソースファイル／ディレクトリを**削除**し、パッチ済みファイルのみ残す。

### `source::walk_occt_sources(source_dir, f)`
- **責任**: OCCT ソースツリーの走査ルール（`src/`・`adm/` は全ファイル、他トップレベルはディレクトリ自身、トップレベルファイルはスキップ）を提供し、各対象に `f` を適用する。
- **OCCT_ROOT への影響**: 自身は走査するだけ（**なし**）。ただし呼び出し側が渡す `f` が書き込み・削除を行う（パッチ書き込み／非改変ファイル削除）。

### `source::patch_or_none(path)`
- **責任**: ファイル名に応じてパッチ後の内容を返す純粋関数（OSD/POSIX 依存・StackTrace・VisMaterial・Texture・OSD_WNT 等を body-stub / 空ファイル化、不在ヘッダの `#include` をコメントアウト）。`None` ならパッチ不要。
- **OCCT_ROOT への影響**: **なし**（ディスクに書かない純粋関数）。判定結果が呼び出し側の書き込み／残置／削除を駆動する。

### `source::stub_content(path, keep_signatures)`
- **責任**: C++ ソースのスタブ済み内容を生成（署名維持なら本体だけ `stub_all_top_level_bodies` で潰す、そうでなければヘッダコメントのみの空ファイル）。
- **OCCT_ROOT への影響**: **なし**。元ファイルを**読む**ことはあるが書き込まない。

### `source::comment_out_include_in(content, header)`
- **責任**: `#include <header>` をコメント化した文字列を返す純粋関数。
- **OCCT_ROOT への影響**: なし。

### `source::lex_normalize(content)`
- **責任**: コメント・文字列・文字・プリプロセッサ行を空白に潰しつつバイト長を保つ字句正規化（本体検出用）。純粋関数。
- **OCCT_ROOT への影響**: なし。

### `source::is_parameter_list_tail(rest)`
- **責任**: `)` の直後が引数リスト終端（末尾修飾子／初期化リスト／`->`）かを判定する純粋関数。
- **OCCT_ROOT への影響**: なし。

### `source::stub_body_for_sig(sig)`
- **責任**: シグネチャから適切なスタブ本体（`{}` か `{ return {}; }`）を決める純粋関数。
- **OCCT_ROOT への影響**: なし。

### `source::stub_all_top_level_bodies(content)`
- **責任**: トップレベル関数本体をスタブに置換した文字列を生成する純粋関数（変数初期化はそのまま）。
- **OCCT_ROOT への影響**: なし。

---

## まとめ表

| 関数 | OCCT_ROOT への影響 | 種別 |
|------|--------------------|------|
| `release_name` | なし | パス名導出 |
| `cargo_target_dir` | なし | パス計算 |
| `find_occt_dirs` | なし | 読み取りプローブ |
| `apply_compiler_flags` | なし | フラグ適用 |
| `fetch_bytes` | なし | 取得のみ |
| `download_and_extract_tar_gz` | **dest に展開**（呼び出し側依存） | 書き込み |
| `main` | 自身は無。`effective_root` を確定 | 制御 |
| `resolve_occt` | ミス時に下流で**生成**（dirs を返すだけ） | 分岐 |
| `link_occt_libraries` | なし（lib を走査して読むだけ） | リンク出力 |
| `occt_from_prebuilt` | **effective_root を削除→展開→rename で上書き生成** | 書き込み/削除 |
| `bundle_runtime_libs` | **lib に `libcadrum_*.a` 追加** | 書き込み |
| `source::occt_from_source` | **effective_root 全体を生成・slim 化・削除** | 書き込み/削除 |
| `source::walk_occt_sources` | 自身は無（`f` 経由で書込・削除） | 走査 |
| `source::patch_or_none` | なし（純粋） | 判定 |
| `source::stub_content` | なし（読みのみ） | 生成 |
| `source::comment_out_include_in` | なし | 純粋 |
| `source::lex_normalize` | なし | 純粋 |
| `source::is_parameter_list_tail` | なし | 純粋 |
| `source::stub_body_for_sig` | なし | 純粋 |
| `source::stub_all_top_level_bodies` | なし | 純粋 |

**要点**: OCCT_ROOT の中身を実際に作る／変える責任を持つのは `occt_from_prebuilt`・`bundle_runtime_libs`・`source::occt_from_source`（と汎用ヘルパ `download_and_extract_tar_gz`）の4つだけ。残りはパス計算・読み取り・純粋関数で、OCCT_ROOT を一切汚さない。`occt_from_prebuilt` と `occt_from_source` は `resolve_occt` で `source` feature により排他選択される対の関数で、第一引数はどちらも `effective_root`。同梱 `bundle_runtime_libs` は `main` に格上げされ、`CADRUM_BUNDLE_RUNTIME`（旧 `CADRUM_BUNDLE_GCC_RUNTIME`）+ GNU target のときのみ発動する。
