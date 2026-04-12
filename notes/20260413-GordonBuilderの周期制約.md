# GeomFill_GordonBuilder の周期 BSpline 拒否制約

## 結論

OCCT 8.0 rc5 の `GeomFill_GordonBuilder` は、**profile / guide のどちらか一方でも
周期 BSpline を含むと Init / Perform の段階で例外を投げて失敗する**。
Header で公開されている `theIsUClosed` / `theIsVClosed` フラグは実装が追従しておらず、
宣言だけの飾りになっている。

トーラス (stellarator / tokamak) のように両方向が本質的に周期である形状を、
**GordonBuilder 経由で厳密に扱う経路は存在しない**。

## 実測データ

`examples/08_gordon_surface.rs` (10×10 torus) と `examples/09_gordon_probe.rs`
(3×3 〜 6×6 の grid sweep) で、`cpp/wrapper.cpp::make_gordon_direct` に
sample 数 × periodic フラグの全組み合わせを試した probe を仕掛けて計 2091 件の
attempts を回した結果:

| (prof_periodic, guide_periodic) | passes / attempts (small grid) | passes / attempts (10×10) |
|---|---|---|
| `(false, false)` | **1050 / 1260** | **324 / 400** |
| `(true,  false)` | 0 / 1092 | 0 / 351 |
| `(false, true)`  | 0 / 1014 | 0 / 338 |
| `(true,  true)`  | 0 / 1014 | 0 / 338 |

**周期フラグが片側でも立つと pass 件数は常に 0**。非周期ペアのみが通過する。

失敗時の例外メッセージはほぼ全て `Geom_BSplineSurface: # U Poles and degree mismatch`
で、一部 `pP=0 pG=1` で empty throw も混じる。

## メカニズムの推測

`GeomFill_GordonBuilder.hxx` によると GordonBuilder は 3 枚の中間サーフェスを作る:

- `S_profiles` — profile 群を V 方向にスキン
- `S_guides`   — guide 群を U 方向にスキン
- `S_tensor`   — profile × guide の交点格子を tensor 補間

その後 `unifySurfaces()` が 3 枚の knot / pole を揃え、Boolean sum
`S = S_profiles + S_guides − S_tensor` で最終結果を出す。

OCCT BSpline の poles / knots / degree 関係式は:
- **非周期**: `n_poles = n_knots - degree - 1`
- **周期**  : `n_poles = n_knots + degree - 1`

関係式が異なるため、`unifySurfaces` が中間サーフェスを再構築するときに
片方を周期扱い・もう片方を非周期扱いで組もうとすると必ず `# U Poles and
degree mismatch` で破綻する。両方周期でも壊れるのは、`buildTensorSurface`
が内部で非周期補間 (`GeomAPI_PointsToBSplineSurface` など) を使って格子点を
interpolate しているため、tensor 側が非周期で固定される一方
profile / guide 側が周期になり整合しないため、と推測される。

header が `isUClosed` / `isVClosed` フラグを受け取る設計なのに実装が
周期 BSpline をサポートしていないのは、**公開 API の未完成部分**と見てよい。

## 実用上の影響

- `cpp/wrapper.cpp::make_gordon_direct` は profile / guide を強制的に
  `make_compatible_bspline(..., periodic=false)` で非周期化して GordonBuilder
  に渡している。周期円の場合、非周期補間の再サンプリングで
  「sample[0] と sample[nSamples-1] が幾何的に重なる縮退曲線」になる。
- 結果、`examples/08_gordon_surface` (R=3, r=1 の torus) で得られる volume は
  理論値 6π² ≈ 59.22 に対して 34〜40 程度 (65〜67%) に留まる。seam の chord
  が失われて体積欠損が出る。
- profile を「等値曲線として厳密に再現」するという目標は GordonBuilder 経路
  では**原理的に達成不能**。

## 回避策の候補

1. **pole を手組みして `Geom_BSplineSurface` を直接構築**
   - `Geom_BSplineSurface` のコンストラクタ `(poles[2D], uKnots, vKnots,
     uMults, vMults, uDeg, vDeg, uPeriodic, vPeriodic)` に torus 用の制御点
     配列を与えれば、周期性を保ったまま厳密な torus を作れる。
   - Gordon アルゴリズムは使わないが、入力 profile/guide の pole を control
     net として直接流用できる限り、幾何的な厳密性は保てる。
2. **rainman110/occ_gordon を patching** (upstream の TODO:
   "both u-directional splines and v-directional splines are closed" を埋める)
   - doubly-periodic の tensor interpolation を循環行列ソルバーで実装する
     必要あり。数日〜数週間の作業。
3. **現状の approx 経路** (`GeomAPI_PointsToBSplineSurface` ベース) に戻す
   - 過去の実装で volume 精度は 53.27 / 59.18 と理論値に近かった。
   - profile は point-cloud 再補間で近似だが GordonBuilder の 65% よりは
     はるかに精度が高い。contract [30, 90] は余裕で満たす。

## 参考

- probe 実装: `cpp/wrapper.cpp::make_gordon_direct` (2025-04 時点で
  combinatorial sweep 付き実装) — 最終的には単一の「受理される条件」に固定する。
- probe log: セッション中の `probe.log` / `probe_08.log` に全 attempt 記録あり
  (commit には含めていない)。
- 関連: `notes/20260411-Solid_gordon実装.md` (Gordon 採用の動機と初期実装方針)
