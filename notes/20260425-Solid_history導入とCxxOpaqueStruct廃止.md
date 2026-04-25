# Solid::history 導入と Cxx Opaque Struct 廃止

## 背景

これまで boolean 演算 (`union` / `subtract` / `intersect`) は派生情報を戻り値タプルの第 2 要素 `[Vec<u64>; 2]` で返す `*_with_metadata` API を持っていた:

```rust
// 旧 API (src/traits.rs)
fn union_with_metadata(...) -> Result<(Vec<Self::Elem>, [Vec<u64>; 2]), Error>;
fn union(...) -> Result<Vec<Self::Elem>, Error> { Ok(self.union_with_metadata(tool)?.0) }
```

第 0 要素が `from_self` (a 側由来 Face)、第 1 要素が `from_tool` (b 側由来 Face) で、それぞれ flat な `[post_id, src_id, ...]` ペア列。さらに C++ 側に `class BooleanShape { TopoDS_Shape shape; std::vector<uint64_t> from_a; std::vector<uint64_t> from_b; }` が存在し、FFI 関数 3 本で個別取り出しを行っていた。同じパターンの `class CleanShape` も clean 操作用に存在。

## 問題

- 戻り値型 `(Vec<Solid>, [Vec<u64>; 2])` がマジックインデックス的 (`metadata[0]` / `metadata[1]`)
- 派生情報を後段に持ち回るとき毎回タプルを通さないといけない
- C++ 側の opaque struct (`BooleanShape` / `CleanShape`) 1 個につき FFI 関数が 3 本に膨らむ
- `*_with_metadata` と `*` の二重 API でメソッド数が多い
- 既存 3 call sites (`08_shell.rs`, `tests/shape.rs` × 2) すべて tool 側 (`[1]`) しか見ていない → self/tool 二分割は実需が無い

## 解決方針

### 方針 1: Solid に派生情報を state として持たせる

`Solid::history: Vec<u64>` フィールドを追加 (cfg ゲートなし、`colormap` と違い常に有効)。flat `[post_id, src_id, ...]` で内部表現。公開 API は `iter_history(&self) -> impl Iterator<Item = [u64; 2]>` 一本。

#### 議論したが採用しなかった案

1. **`Vec<FaceLineage { post: u64, src: u64 }>`** — 型で意味を明示できるが、cxx は POD struct を直接 Vec で受け渡せない (`ExternType` 経由の追加層が必要)。`Vec<u64>` なら cxx の primitive Vec サポートで FFI から直接編集可能。`iter_history` accessor で型安全性は確保できるので、内部 `Vec<u64>` + 出口で `[u64; 2]` の二層分業が最良。
2. **`history` を累積 (世代をまたぐ)** — `A.subtract(B).subtract(C)` で `r.history` を全祖先累積する案。Vec が肥大化、post_id の名前空間が世代をまたいで曖昧化、設計規模が一段大きくなる。「直近の操作で生えた面に対して何かやりたい人のための情報源」という用途には上書きで十分。
3. **self/tool 区別タグ (`Vec<[u64; 3]>` で input_idx も持つ)** — 表現複雑化、cxx の `Vec<u64>` 直接編集の利点が失われる。実需も無い (call site 全て tool 側のみ参照)。
4. **scale / mirror / Clone で `remap_history_by_order` を書いて preserve** — colormap 用の `remap_*_by_order` をテンプレに同等のものを作る案。実装は可能だが、新しい topology を作る操作で history を持ち回す意味が薄い。`Default::default()` で捨てて将来「直近の builder の派生情報」として再利用できる空き枠にしておく方が筋が良い。

#### 採用したルール

| 操作 | history |
|---|---|
| 不変系: `translate` / `rotate` / `color` / `color_clear` | preserve |
| トポロジ rebuild 系: `scale` / `mirror` / `Clone` | `Default::default()` (clear) |
| 全プリミティブ + builder (`extrude` / `sweep` / `loft` / `bspline` / `shell` / `fillet` / `chamfer`) | `Default::default()` (現状) |
| boolean (`union` / `subtract` / `intersect`) | C++ 側で from_a + from_b を flat union して populate |
| `read_step` / `read_brep_*` | `Default::default()` |
| シリアライズ | しない (ephemeral) |

将来的には builder 系 (fillet / chamfer / sweep / extrude / loft / bspline / shell) も `BRepBuilderAPI_MakeShape::Modified()` / `Generated()` を使って history を populate できるよう、API は今の形で安定。

### 方針 2: cxx opaque struct の out-parameter 化

`BooleanShape` と `CleanShape` をそれぞれ廃止し、`std::unique_ptr<TopoDS_Shape>` を return + `rust::Vec<uint64_t>& out_*` を out-parameter として受ける形に倒す:

```cpp
// 旧
std::unique_ptr<BooleanShape> boolean_op(const TopoDS_Shape&, const TopoDS_Shape&, uint32_t);
std::unique_ptr<TopoDS_Shape> boolean_shape_shape(const BooleanShape&);
rust::Vec<uint64_t> boolean_shape_from_a(const BooleanShape&);
rust::Vec<uint64_t> boolean_shape_from_b(const BooleanShape&);

// 新
std::unique_ptr<TopoDS_Shape> boolean_op(
    const TopoDS_Shape&, const TopoDS_Shape&, uint32_t,
    rust::Vec<uint64_t>& out_history);
```

C++ 側 4 個の FFI 表面が 1 個に集約。`unsafe impl Send for BooleanShape {}` / `for CleanShape {}` も自動的に消える。同パターンを `CleanShape` (`clean_shape_full` / `clean_shape_get` / `clean_shape_mapping`) にも適用。

#### 適用条件

- C++ 側で当該 opaque struct を参照する関数が「生成 + getter のみ」で、Rust 側 caller が「生成 → 即座に getter 1 回ずつ → 破棄」で済んでいること
- 遅延評価や intermediate state 共有がないこと

→ `BooleanShape` も `CleanShape` も両方この条件を満たす。

## API ergonomics 上の trade-off

`08_shell.rs` の cutter 由来 Face 抽出が、self/tool 二分割の喪失で 3 行ほど冗長になった:

```rust
// 旧 (3 行)
let (mut halves, [_, from_cutter]) = torus.intersect_with_metadata(&[cutter])?;
let half = halves.pop().ok_or(Error::BooleanOperationFailed)?;
half.shell(thickness, half.iter_face().filter(|f| from_cutter.contains(&f.tshape_id())))

// 新 (cutter の Face ID を別途キャプチャしてから filter_map)
let cutter_face_ids: HashSet<u64> = cutter.iter_face().map(|f| f.tshape_id()).collect();
let halves = torus.intersect(&[cutter])?;
let half = halves.into_iter().next().ok_or(Error::BooleanOperationFailed)?;
let from_cutter: HashSet<u64> = half.iter_history()
    .filter_map(|[post, src]| cutter_face_ids.contains(&src).then_some(post))
    .collect();
half.shell(thickness, half.iter_face().filter(|f| from_cutter.contains(&f.tshape_id())))
```

許容範囲。AGENTS.md「ユーザーに誤解を招くか僅かな手間を強いるかで迷ったら後者を取る」と整合。

## 結果

12 ファイル変更、+223/-192 行。

- C++ opaque struct 2 個削除 (`BooleanShape`, `CleanShape`) → FFI 表面 −6 関数 / −2 type
- `Solid::history` field と `iter_history()` accessor 追加
- `*_with_metadata` × 3 + `is_tool_face` / `is_shape_face` helper 削除
- 全 Solid::new 呼び出し (約 25 ヵ所) を `Default::default()` で history 初期化するよう更新
- 検証: `cargo test` 全 pass、11 example 全実行、touched code clippy clean

## 関連

- `notes/20260420-OCCTトポロジ不変性と設計含意.md` — 不変性ルールと colormap の preserve/clear 方針 (history も同型)
