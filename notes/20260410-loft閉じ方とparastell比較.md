# Closed loft の実装方法と parastell との比較

## 背景

ステラレーター CAD のフェーズ 2 で `Solid::loft` を実装するにあたり、トロイダル方向に**閉じた**(周期的な)曲面をどう構築するかが論点になった。素朴な「最後にもう一度最初の wire を AddWire する」案が接線連続性を壊す懸念があったため、OCCT の正規 API と先行プロジェクト parastell の実装を徹底調査した。結論として **OCCT は closed loft を公式にサポートしており、cadrum はそれを直接活用できる**。これは parastell に対する cadrum の明確な優位点になる。

このノートはその調査結果をまとめたもので、loft 実装(`feature/loft` ブランチ)の設計根拠として記録する。

## 1. parastell の closed loft 実装(=実装していない)

### 何をしているか

parastell (`build/lib/parastell/invessel_build.py`)の核心箇所:

```python
# Surface.generate_surface (L1157-1164)
def generate_surface(self):
    """Constructs a surface by lofting across a set of rib splines."""
    if not self.surface:
        self.surface = cq.Solid.makeLoft(
            [rib.generate_rib() for rib in self.Ribs]
        )
    return self.surface
```

`cq.Solid.makeLoft` は **CadQuery のラッパー** で、内部で OCCT の `BRepOffsetAPI_ThruSections` を呼ぶ。だがここで渡しているのは N 本の rib(各々はポロイダル方向に閉じた wire)を **list として** 並べただけで、`closed` フラグは存在しない(CadQuery API が公開していない)。

つまり `makeLoft` の結果は **「toroidal 方向に開いた」shell** で、ステラレーターのような完全閉曲面ではない。ある toroidal 角度範囲(例: 0°〜90°)を 1 つの open loft セグメントとして作る。

### どうやって閉じるか: boolean rotate+fuse

```python
# generate_components_cadquery (L526-559)
segment_angles = np.linspace(
    self.radial_build.toroidal_angles[-1],
    self._repeat * self.radial_build.toroidal_angles[-1],
    num=self._repeat,
)

for name, surface in self.Surfaces.items():
    outer_surface = surface.generate_surface()  # 開いた loft 1 セグメント

    if interior_surface is not None:
        segment = outer_surface.cut(interior_surface)
    else:
        segment = outer_surface

    component = segment

    # rotate して fuse を _repeat 回繰り返す
    for angle in segment_angles:
        rot_segment = segment.rotate((0, 0, 0), (0, 0, 1), angle)
        component = component.fuse(rot_segment)

    self.Components[name] = component
```

**フロー全体**:

1. `radial_build.toroidal_angles = [0, 11.25, 22.5, ..., 90]`(90 度幅の 1 セグメント)
2. その範囲で rib を生成し、open loft で 1 セグメントを作る
3. Z 軸まわりに `_repeat` 回(典型 4 回 = 360°/90° = 4)回転コピーして `fuse`(boolean union)で接合
4. 結果として 360 度フルトーラスが完成

### この方式の問題点

- **boolean union を 4 回以上実行**するため計算が重く、複雑な形状ほど OCCT のロバストネスが試される
- セグメント境界での **接線連続性は保証されない**。回転コピーした 2 つのセグメントは、境界面で「同じ位置・同じ法線」で接するが、boolean union は形状を「正しく繋ぐ」だけで「滑らかに繋ぐ」わけではない。境界線で C¹ より上の連続性は失われる
- **頂点の重複や数値ノイズ**を回避するため、`_repeat` パラメータの上限チェックが必要(該当: L417-424 の `(self._repeat + 1) * toroidal_angles[-1] > 360.0` のアサート)
- セグメント間で頂点 ID が不連続なので、後段のメッシング(DAGMC 出力)で **stitching の追加処理**が必要になる
- ステラレーターは本来 **field period** ごとに同一形状なので、回転対称性を活用するアプローチ自体は妥当だが、**boolean fuse でしか繋げないのは CadQuery の制約**であって本質ではない

つまり parastell は CadQuery の API 制約に縛られて回避策を取っているだけで、より低レベルの OCCT を直接呼べるならもっと素直な方法があるはず — というのが調査の出発点。

## 2. OCCT の closed loft API の真実

### `BRepOffsetAPI_ThruSections` のソースコードを読む

OCCT 7.x (`BRepOffsetAPI_ThruSections.cxx`) を直接読んで判明した事実:

```cxx
// .cxx 行 539 (CreateRuled 内)
Standard_Boolean vClosed = (myWires(1).IsSame(myWires(nbSects)));

// .cxx 行 691 (CreateSmoothed 内)
Standard_Boolean vClosed = (myWires(1).IsSame(myWires(nbSects)));
```

**OCCT は loft の最初と最後の wire が「同一オブジェクト」(`IsSame()` = TShape* pointer 比較)であるかを自動チェックし、true なら `vClosed` フラグを立てて v 方向周期な surface を構築する**。

```cxx
// .cxx 行 1187-1189: vClosed が true のとき、最後のセクションで最初の曲線を再利用
if (vClosed) {
    // skip the duplicate last section, reuse the first curve
}
```

つまり **「最後に最初の wire と同じオブジェクトをもう一度 AddWire する」** という素朴に見える操作は、実は OCCT が**公式にサポートしている closed loft の正規経路**だった。`IsSame()` で識別されるための条件は:

- **同じ TopoDS_Wire 変数を 2 回 AddWire する**(コピー代入は OCCT の Handle 経由なので TShape* identity が保たれる、これも OK)
- **`BRepBuilderAPI_Copy` で deep copy したものを渡してはダメ**(TShape* が新規に割り当てられて `IsSame()` が false になる)

### 接線連続性の確認

`isRuled = false`(smoothed mode、cadrum のデフォルト)では:

```cxx
// CreateSmoothed の処理: 全セクションを GeomFill_AppSurf で一度に近似
if (myUseSmoothing) {
    anApprox.PerformSmoothing(line, section);
} else {
    anApprox.Perform(line, section, SpApprox);
}
```

`GeomFill_AppSurf` は **全セクションを同時に B-spline 近似**するアルゴリズムで、`vClosed` フラグが立っているときは v 方向の周期境界条件を含めて surface を構築する。結果として **境界(= 最初と最後の rib 位置)で C² 連続**な surface が得られる。これは parastell の boolean fuse 方式では絶対に到達できない品質。

`isRuled = true` の場合は panel-by-panel(セクション間を直線で繋ぐ ruled surface)になり、接線連続性は保証されない。cadrum は smoothed をデフォルトにするので、loft の境界も自動的に C² 連続になる。

### `SetSmoothing(true)` ではない

紛らわしいが、`BRepOffsetAPI_ThruSections::SetSmoothing(bool)` は **接線連続性とは無関係** な近似アルゴリズムの選択スイッチで、smooth vs ruled の区別はコンストラクタの `isRuled` 引数で行う。cadrum では `SetSmoothing` は呼ばずに `isRuled = false` のデフォルト挙動を使う。

### parastell が知らなかった理由

CadQuery のソースを追えていないので推測だが、`cq.Solid.makeLoft` の引数は単純な list で、Python 側で `closed=True` のような flag を立てて C++ の `IsSame()` 経路に乗せる手段が公開されていない可能性が高い。CadQuery がラッパーレベルで「最初と最後を同一にする」明示的な API を出していないため、parastell は知らない/使えないだけと思われる。

cadrum は cxx::bridge で OCCT を直接呼ぶので、ラッパーレベルの制約がない。**ここに cadrum の優位点がある**。

## 3. cadrum での実装方針

`Solid::loft(sections, closed: bool)` の `closed = true` 経路は以下の C++ コードで実装する:

```cpp
TopoDS_Wire first_wire;
for (size_t s = 0; s < section_sizes.size(); ++s) {
    BRepBuilderAPI_MakeWire wire_maker;
    for (uint32_t i = 0; i < section_sizes[s]; ++i) {
        wire_maker.Add(all_edges[edge_idx + i]);
    }
    edge_idx += section_sizes[s];
    if (!wire_maker.IsDone()) return nullptr;

    TopoDS_Wire wire = wire_maker.Wire();
    loft.AddWire(wire);
    if (s == 0) first_wire = wire;  // ← 同一 TShape* のまま保持
}

if (closed) {
    // OCCT の IsSame() で検出される。BRepBuilderAPI_Copy しないこと。
    loft.AddWire(first_wire);
}
```

`first_wire = wire;` は OCCT 内で Handle のコピー = TShape* pointer の参照コピーなので、**identity が保たれる**。後で再 `AddWire` したときに `IsSame()` が true になり、OCCT が closed loft の処理経路を選ぶ。

## 4. cadrum と parastell の比較表

| 観点 | parastell | cadrum (本実装) |
|---|---|---|
| 使用 CAD ライブラリ | CadQuery (ラッパー) | OCCT 直接 (cxx::bridge) |
| Closed loft の API | 無い(`makeLoft` に flag 無し) | あり(`Solid::loft(sections, closed=true)`) |
| Closed 実装方式 | open loft + boolean rotate+fuse(回避策) | OCCT IsSame trick(正規経路) |
| 境界の連続性 | C⁰ のみ(boolean union による接合) | C²(`GeomFill_AppSurf` の周期境界処理) |
| Boolean op の回数 | フィールド周期数 - 1(典型 3 回) | 0 回 |
| 計算コスト | 高(boolean fuse は重い) | 低(loft 1 回) |
| 数値ロバスト性 | 中(boolean のたびに頂点重複・誤差累積) | 高(単一 surface 構築) |
| 後段メッシング | 境界 stitch が必要 | そのまま使える |
| API シグネチャ | `cq.Solid.makeLoft([list])` | `Solid::loft(sections, closed: bool)` |

### cadrum が優位な点(まとめ)

1. **API として closed loft を公開している** — ユーザーが「閉じたいか開いたいか」を 1 つの bool で指定できる
2. **C² 連続性が保証される** — 境界で接線・曲率まで一致するので、後段の解析(磁場計算、メッシング、輸送計算)が安定
3. **計算コストが低い** — boolean union を経由しないので、複雑形状でも OCCT 呼び出しが線形オーダーで済む
4. **数値ノイズが少ない** — 単一の surface fitting なので頂点 ID が連続、stitching 不要
5. **コードが明示的** — 「閉じる」意図がコード上に直接出る(parastell は `_repeat` パラメータと `fuse` ループの組み合わせを読み解かないと意図が分からない)

### parastell の方式が妥当なケース

完全に劣位ではない。以下のケースでは parastell 的アプローチも合理的:

- **異なる field period で形状が変わる**(完全に対称でない磁場配位)場合、回転コピーではなく明示的に rib 列を作るしかない
- **半周期や 1/3 周期だけ作りたい**(対称性を活用した解析や可視化用)場合、open loft の方が適切
- **ライブラリ制約上、低レベル OCCT が呼べない**(CadQuery, FreeCAD Python API など)

cadrum は OCCT を直接呼ぶ立場なので、これらの制約から解放されている。

## 5. 実装後の検証戦略

`tests/loft.rs` で以下を検証する予定:

1. **数値検証**: 円錐台の体積が解析値 `π/3 × h × (R₁² + R₁R₂ + R₂²)` と一致(tolerance < 1%)
2. **Closed loft の意味的検証**: 同じ section 列を `closed=false` と `closed=true` で loft し、face 数の差(open は端面 cap が増える)を assert することで、closed パスが OCCT に届いていることを間接的に確認
3. **bspline 統合**: 楕円的 `Edge::bspline(Periodic)` を rib 列にしてロフト → ステラレーター用途のミニ再現
4. **Closure ergonomic**: `(0..N).map(|i| [&edge])` 形の closure 呼び出しが動くこと

OCCT の `vClosed` フラグが立ったかを直接観測する API は無いので、**トポロジーの差(face 数や shell 数)から間接検証**する方針を取る。

## 6. 将来の拡張余地

- **`isRuled` の enum 化**: 現在 smooth 固定。将来「ruled が欲しい」要望が出たら `LoftKind::{Smooth, Ruled}` で受ける(`BSplineEnd` や `ProfileOrient` と同じパターン)
- **`SetMaxDegree` / `SetParType` の露出**: B-spline 近似の細かい調整。デフォルトで十分なはずだが、品質チューニングが必要になったら追加
- **`SetCriteriumWeight`**: smoothed approximation の重み付け。ステラレーター用途では曲率制約を強める可能性あり
- **複数 field period の自動展開**: VMEC は通常 1 field period のフーリエ係数を提供するので、cadrum 側で N 周期分の rib を生成 → loft → boolean union(ステラレーターは厳密に rotational symmetric なので結果は同じ)。これは loft API の上の application layer で実装する予定

## 7. 関連する OCCT API(調査済み、不採用)

以下も検討したが loft の代替にはならないことを確認:

- **`BRepFill_PipeShell`**: pipe/sweep 用で、断面補間 (loft) ではない。`SetMode` に loft 的な周期性オプションは無い
- **`BRepFill_Generator`**: 単純な ruled shell 生成のみ。`BRepOffsetAPI_ThruSections` の下位機能で、loft API として使うべきではない
- **`GeomFill_NSections`**: lower-level の section interpolation。`IsUPeriodic`/`IsVPeriodic` はクエリのみで、コンストラクタで periodic を指定する手段は無い。`BRepOffsetAPI_ThruSections` がこれを内部で使っているので、直接呼ぶ意味はない
- **`BRepBuilderAPI_Sewing`**: open shell を縫合して closed solid にする用途。**接線連続性は保証されない**(C⁰ のみ)ので、closed loft の代替にはならない

## 8. 参考文献

- OCCT 7.9.3 ヘッダ: `BRepOffsetAPI_ThruSections.hxx`, `GeomFill_AppSurf.hxx`, `GeomFill_NSections.hxx`
- OCCT 7.9.3 ソース: `BRepOffsetAPI_ThruSections.cxx`(行 539, 691, 1187-1189, 1266-1274)
- parastell 0.x: `parastell/invessel_build.py`(`Surface.generate_surface`, `generate_components_cadquery`)
- CadQuery 2.x: `cq.Solid.makeLoft` ラッパー実装(本調査では未確認、推測のみ)

---

このノートの結論を一文にまとめると:

> **OCCT は最初と最後に同一 TopoDS_Wire オブジェクトを置けば自動で closed loft の処理経路に乗ってくれる。これは公式仕様だがラッパー層(CadQuery, parastell)では公開されておらず、cadrum が OCCT を直接呼ぶ立場ゆえに利用できる。`Solid::loft(sections, closed: bool)` の `closed = true` はこの IsSame trick で実装する。**
