# 基本方針

- OCCTの仕様とSTEPファイルの仕様でどちらに忠実な表現にするか迷うときはSTEPファイルの仕様を優先する
- 関数や構造体を増やす方向の検討より減らす方向の検討を優先する
- OCCTのモジュールを減らす方向の検討を優先する
- **ユーザーに誤解を招くか、僅かな手間を強いるかで迷ったら後者を取る**。例えば `impl IntoIterator<Item = &Edge>` を期待する API に対して `impl IntoIterator for &Edge` を足せば `func(&edge)` と単一要素で書けるようになるが、そうすると「この関数はコレクションを受け取る」というシグネチャの意図がユーザーから見えなくなる。**単一要素でも `&[edge]` や `[&edge]` と書かせて、「これは集合を受け取る関数だ」という事実を型レベルで認識してもらう方を優先する**。sugar でシグネチャの本来の意図を覆い隠すより、呼び出し側に3文字余分に書かせる方が長期的な理解に繋がる。

# ディレクトリ構成

- cpp/wrapper.h/cpp
    - occtとrust間のバインディング
- notes/YYYYMMDD-日本語タイトル.md
    - 設計方針などを記録
- examples/00_*.rs
    - このリポジトリのサンプルコードです。実行するとカレントディレクトリに00_*.svg/stepが生成されます。この命名規則に従う出力ファイルはbook.rsによりドキュメント内からリンクされます。
- examples/codegen.rs
    - 引数で渡された各 .rs ファイルを「trait 定義のソース」かつ「マーカ書き換え先」として扱い、`////////// codegen.rs` マーカ領域を in-place で再生成する
    - 使い方: `cargo run --example codegen -- src/traits.rs src/lib.rs`
    - 全入力ファイルからトレイト定義を pool して、その union で各ファイルのマーカを書き換える。trait 定義と consumer が別ファイルでも同一ファイルに merge されていても動く
    - traits.rs の公開トレイト定義を変更したら走らせて差分をコミットする（examples/markdown.rs が README.md を改変するのと同じ運用）
    - マーカ仕様:
        - `////////// codegen.rs` (タグなし): 囲みが `impl X { ... }` なら `XStruct` チェーンの inherent methods、囲みが `pub trait X: Y, Z { ... }` なら親 trait Y, Z の forwarder default methods を生成
        - `////////// codegen.rs <Tag>` (モジュールレベル): `<Tag>Module` の free fn を生成
    - 領域はマーカ次行から「囲みブロックの閉じ `}`」または EOF まで。マーカ自身は保存される
- examples/markdown.rs
    - 番号付きexample (NN_*.rs) を実行し、mdbook用markdownとREADMEのExamples節を生成する
    - 使い方: `cargo run --example markdown -- out/markdown/SUMMARY.md ./README.md`
    - 第1引数: SUMMARY.mdパス → mdbook用markdown一式を出力
    - 第2引数: README.mdパス → ## Examples節を最新のソースコードと生成物で更新（画像は GitHub Pages 上の `https://lzpel.github.io/cadrum/<name>.svg` を参照）
- src/traits.rs
    - traits.rsはバックエンド共通のトレイト定義（pub(crate)、ユーザーに非公開）
    - トレイト名は`<Type>Struct`の命名規則に従う（SolidStruct→Solid, FaceStruct→Face等）
    - fnシグネチャは1行、#[cfg]は直前1行のみ認識、ライフタイム/where句は1行に収めれば対応