# WASM対応とビルドシステムの将来構想

## 現状のビルド構成

chijinの`bundled`featureは以下の流れでOCCTをビルドする：

1. `ureq`でOCCT 7.9.3ソースをダウンロード（pure Rust）
2. `cmake` crateがシステムのCMakeバイナリを呼び出す
3. CMakeが`CMakeLists.txt`を読み、Makefile/Ninja/VS projectを生成（Configure + Generate）
4. 生成されたビルドシステム（Make, Ninja等）がGCC/Clang/MSVCを呼んでコンパイル・リンク（Build）
5. `cxx_build`（内部で`cc` crateを使用）がwrapper.cppをコンパイル

つまりCMake自体はコンパイラを直接呼ばず、間にビルドシステム（Make/Ninja）が挟まる。

### 各コンポーネントの役割

| コンポーネント | 役割 |
|---|---|
| CMake | CMakeLists.txtからMakefile/Ninjaファイルを生成 |
| Make / Ninja | 依存関係を見てコンパイラを呼ぶ（NinjaはMakeの高速版） |
| `cmake` crate | Rustのbuild.rsからシステムのCMakeバイナリを呼ぶラッパー |
| `cc` crate | Rustのbuild.rsからGCC/Clang/MSVCを適切に呼ぶ抽象化 |

## 問題：CMakeが外部バイナリ依存

- `cmake` crateはシステムにインストール済みのCMakeバイナリを呼ぶだけ
- CMakeをダウンロードしてくれるcrateや、pure RustのCMake互換は存在しない
- ユーザーはCMakeを事前にインストールする必要がある

### 検討した代替案

1. **CMakeバイナリを自動ダウンロード** — build.rsでGitHub ReleasesからCMakeを取得し、`CMAKE`環境変数にセット。Windows版は`.zip`配布なので`zip` crateの追加が必要
2. **プリビルド済み静的ライブラリを配布** — CMake不要だがプラットフォームごとにバイナリ管理が必要
3. **現状維持** — READMEにCMake必須を明記

現時点では3（現状維持）を選択。

## 将来構想：WASM対応と純粋Rust CMake互換

### 動機

OCCTをEmscripten等でWASMコンパイルするには、CMakeビルドプロセスへの大量の介入が必要：
- `add_definitions`の差し替え
- リンクフラグの書き換え
- 特定ソースファイルの除外
- ツールチェインファイルの調整

外部のcmakeバイナリに依存していると、これらの介入が「CMakeLists.txtにパッチを当てる」形になり汚くなる。

### 構想

純粋RustでCMakeLists.txtをパースして評価できれば、ビルドグラフをRust側で自由に操作してからコンパイルを実行できる。hackが構造化される。

```
CMakeLists.txt → [純粋Rust CMakeパーサー] → ビルドグラフ → [hack/変換] → cc crateの呼び出し列
```

中間のビルドシステム（Make/Ninja）を飛ばし、CMakeLists.txtから直接`cc` crate経由でコンパイラを呼ぶ形が自然。

### 現実的なスコープ

CMake言語の仕様は巨大（変数スコープ、generator expressions、toolchain files等）で完全互換は非現実的。ただしOCCTのCMakeLists.txtが使うCMakeサブセットだけ解釈する限定実装なら射程圏内。
