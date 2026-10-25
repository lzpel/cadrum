# 三層契約：ビルダー・履歴・探索（#317 項目 1、PR #325）

## 背景

cadrum は OCCT のビルダー群を薄く包むだけで、CAD アプリの土台に要る三つの情報が欠けていた。issue #317 の項目 1 が指摘したのは、面や辺の同一性、ビルダー前後の履歴、失敗の形の三つで、いずれも機能ではなく型の設計に属する。SolidCraft の移行では、これらを「中心と面積が一致する面を同じとみなす」「側面の点を profile に戻す」といった幾何的推測で補っていた。

本 PR は三層を明示し、下二層を全ビルダー共通の契約にした。第一層のビルダー（`extrude`・`fillet_edges`・`Boolean` など）は被演算子から新しい `Solid` を作り、被演算子に書き込まない。第二層の履歴は結果の面と辺が被演算子のどれから来たかを `Lineage` で報告する。第三層の探索は `iter_face` / `iter_loop` / `iter_edge` / `iter_vertex` でトポロジへ、`Face::surface` / `Edge::curve` で NURBS 係数まで幾何へ到達し、`Face::face` / `Edge::edge` で作り直す。

## 失敗の形

全 FFI 関数が cxx の `Result` を返し、OCCT の例外はプロセスを落とさない。OCCT 8 では `Standard_Failure` が `std::exception` を継承するので、cxx 既定の `trycatch` が `Result` ブリッジ関数内の例外をすべて `cxx::Exception` に変える。例外を握りつぶして 0 や false を返していた関数個別の `catch` は取り除き、失敗を伝播させた。

Rust 側は縮退入力を `Err` にし、panic を論理バグに限った。プリミティブは `Error::Validation`、`rotate/scale/mirror/align_*` は `Error::Transform`、`project` と `surface` は `Error::Query` を返す。計測系（`volume` など）は戻り型を変えず、カーネルが拒む solid では `NaN`（`contains` は `false`）を返し、`is_valid` が事前に判定を与える。`translate` は `Moved` で失敗し得ないので `expect` を残し、文書で論理バグと明記した。

`is_valid` は fillet の門番にしなかった。`BRepCheck_Analyzer` は輸入・縫合した solid で偽陰性を出すため、fillet は OCCT 自身のエラーで失敗させる方が正直である。項目 28 のアクセス違反は Rust から捕まえられないと文書化した。

## 同一性

面・辺の id は TShape ポインタと「所属 solid に対する相対 Location」のハッシュを混ぜた 64 bit 値である。`TopLoc_Location::Predivided` で `abs = owner * rel` から `rel` を厳密に取り出せ、探索器が合成する Location は正規形で相殺されるので、押し出しの底面（恒等）と天面（プリズムの datum）は別の id になる。恒等 Location の id は生ポインタのままなので、従来のテストはそのまま通る。

`translate`・`rotate`・色付け・浅い `Clone` は全 id を保つ。`Moved` は solid 自身の Location だけを変え、サブシェイプの相対 Location は変わらないからである。`Clone` は OCCT の `TopoDS_Shape` と同じ浅いハンドルコピーにし、深いコピーの `deep_copy` は削除した。トポロジは不変なので深いコピーを公開する理由がない。

id は solid 間で比較しない。boolean で手つかずに残った面は TShape は同じでも結果に対する相対 Location が被演算子に対するものと異なるため、履歴が `Kept { src, post }` で両方の id を明示する。`Face` は絶対位置の面と所属相対の面（`local`）を持ち、`face_loops` が後者から辺の所属相対 id を計算して `solid.iter_edge()` の id と一致させる。

## 履歴

`Lineage` は `Kept`・`Modified`・`Generated`・`Deleted` の四種で、面と辺の両方を扱う。C++ の `relay_history` は一本のテンプレートで、`Modified(S)` と `Generated(S)` を持つ任意のアルゴリズムから三つ組 `[kind, src, post]` を出す。probe（アルゴリズムに問い合わせる形状）が結果に含まれれば `Kept`、像が結果に含まれれば `Modified` / `Generated`、`Kept` も `Modified` も無ければ `Deleted` とする。`Generated` だけでは生き残りとみなさないので、フィレットで消えた辺は面を生やしつつ `Deleted` になる。

profile の辺は複製してからワイヤにする。`BRepBuilderAPI_MakeWire` は頂点を共有させるために辺を複製することがあり、`ShapeFix_Edge::FixAddPCurve` は辺に pcurve を書き込む。利用者の辺を先に `BRepBuilderAPI_Copy` し、`maker.Edge()` で実際に挿入された辺を記録することで、入力は不変になり、利用者の辺から結果への対応も取れる。複製を probe にした `Kept` は利用者から見れば `Modified` なので、`SrcEntry::copied` で言い換える。

boolean は非破壊にして結果の最終コピーをやめた。`BOPAlgo_CellsBuilder::SetNonDestructive(true)` で被演算子は書き換えられず、結果は手つかずの面を被演算子と共有するので `Kept` で報告できる。`RemoveInternalBoundaries` の履歴は `LocModified` が考慮する。

OCCT の二つの限界はそのまま伝える。`BRepFilletAPI_MakeFillet` と `ShapeUpgrade_UnifySameDomain` は全面を作り直すので、手つかずの面も `Modified` で返る。フィレットで短くなった境界辺の履歴は OCCT が出さない。

## 探索と到達

`Face::iter_loop` は外周ループを先に、各ループの辺を「面を左に見る向き」で返す。`BRepTools::OuterWire` と `BRepTools_WireExplorer` を FORWARD 向きの面に対して使い、面がシェル内で REVERSED でも巻き方向が曲面法線基準で揃うようにした。これは `Face::face` が読む巻き方向と同じなので、`Face::face(&face.surface()?, loop)` が面を復元する。

辺は STEP の oriented_edge と同じく向きを持つ。`start_point` / `end_point` / 接線 / 近似点列 / `project` の接線はすべて辺の向きに従い、`chain()` も辺の向きで始点を決める。この変更が無いと、面のループから取った辺の半分は曲線方向と逆向きになり、復元した面が裏返る。

`Face::surface` は全域の曲面を返す。初等曲面は従来どおり、それ以外は `GeomConvert::SurfaceToBSplineSurface` を UV 範囲でトリムした曲面に掛けて `SurfaceKind::Bspline(NurbsSurface)` にする。`Edge::curve` は直線・円・楕円・NURBS を `Curve` で返し、縮退辺（球の極）では `None` になる。NURBS は STEP の `b_spline_*_with_knots` と `rational_b_spline_*` に揃えた構造体で、FFI は平坦な `Vec<f64>` 一本で運ぶ。

`Solid::mesh` はトポロジのコピーに対して三角形分割する。`BRepBuilderAPI_Copy(copyGeom=false)` で幾何を共有したままトポロジだけ複製し、結果の面を元の面と走査順で対応させて id を付ける。呼び手の形状に書き込まないので `write_brep` のバイト列はメッシュ化で変わらず、`&self` メソッドはすべて読み取り専用になった。`Mesh::uvs` は各頂点の曲面パラメータで、`Face::surface` の NURBS と組んで GPU で曲面を評価できる。

## 残した判断

`Sync` は宣言していない。cadrum 側の書き込みは無くしたが、OCCT のフィレットやオフセットが入力に書かないことは未監査で、README の表がその監査対象を示す。`Surface` と `SurfaceKind` は `Bspline` が `Vec` を持つため `Copy` ではなくなり、テストに数か所 `.clone()` が入った。lump の順序、ドラフト角、可変半径フィレット、XDE、`AddOptimal`、`GProp` の `Eps`、shell の join、sweep の遷移モード、頂点への loft、split / defeaturing、IGES、wasm は issue #317 の後続項目として残る。
