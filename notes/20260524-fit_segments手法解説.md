# fit_segments 手法解説

`sandbox-sdf/src/region2.rs` の `fit_segments` 関数は、SDF 零等位線から marching squares + Newton 射影で得られた**点列**を、`Segment::Line` / `Segment::Circle` の**幾何プリミティブ列**に集約する役割を持つ。Douglas–Peucker のような点間引きではなく、「セグメントを切る位置を最初に固定し、隣接 run を bottom-up で結合していく」アプローチを取っているのが本質的特徴。

以下、手順を分解して解説する。

## 1. 入出力

```rust
fn fit_segments(
    pts: &[Vec2],
    sdf: &impl Fn(Vec2) -> f32,
    tol: f32,                  // フィット残差の許容値 (bbox 対角線比 0.003)
    min_circle_radius: f32,    // Circle 採用の最小半径 (bbox 対角線比 0.003)
) -> EdgeLoop                  // = Vec<Segment>
```

入力 `pts` は閉ループの点列（Newton 射影済みで SDF=0 上に乗っている）。出力は `Segment::Line { point, direction }` または `Segment::Circle { center, radius }` の列。

## 2. コーナー検出（barrier 配列）

```rust
let grad = |i: usize| distance_nabla_laplacian(pts[i], sdf).1;
let barrier: Vec<bool> = (0..n).map(|i| {
    let a = grad(i).normalize_or_zero();
    let b = grad((i + 1) % n).normalize_or_zero();
    a.dot(b) < CORNER_COS_THRESHOLD     // cos(0.3 rad) ≈ 0.955
}).collect();
```

- SDF の勾配 `∇d` は零等位線では法線方向そのもの。
- 隣接 2 点 `i`, `i+1` の単位法線同士の内積（= cos 角度）が閾値 0.955 を下回ったら、その辺 `(i, i+1)` を **barrier**（マージ禁止境界）としてマークする。
- 角度差にして約 17° 以上で barrier。

これによって「四角形の角」や「ポリゴンの頂点」などのコーナーが事前に検出される。bottom-up merge では barrier をまたぐ結合だけ強制的に却下するので、`fit_line` / `fit_circle` の残差判定だけでは見逃しがちな鋭角コーナーを確実に切ることができる。

ポイント: **コーナー判定を「フィット残差」ではなく「勾配の不連続」で行っている**。点列だけ見ると鈍角コーナーは曲線と区別しづらいが、SDF 勾配のジャンプは離散的に出るので閾値判定が安定する。

## 3. run の双方向リンクリスト表現

n 点を初期状態で「1 点 = 1 run」とみなし、双方向リンクリスト + 各 run の開始位置・長さ・生存フラグで管理する。

```rust
let start_idx: Vec<usize> = (0..n).collect();   // run i の開始点 index
let mut run_len: Vec<usize> = vec![1; n];        // run i の長さ
let mut prev_arr: Vec<usize> = ...;              // 循環 prev
let mut next_arr: Vec<usize> = ...;              // 循環 next
let mut alive: Vec<bool> = vec![true; n];        // 生存フラグ
let mut merge_res: Vec<f32> = vec![f32::INFINITY; n]; // run i と next(i) のマージ残差
```

run の点列は `run_points(i, ...)` で `start_idx[i]` から `run_len[i]` 個の点を循環取得する。マージ操作は配列の連結 O(run_len) ではなく、リンクリストの付け替え O(1) ＋ `run_len` 加算で済む。

## 4. bottom-up merge ループ

`merge_residual(i, ...)` は「run i と run next(i) を連結した点列を `fit_line` / `fit_circle` の両方で試した最良残差」を返す。barrier をまたぐ場合は `INFINITY`。

```rust
let mut merged = run_points(i, ...);
merged.extend(run_points(j, ...));
let (_, _, lr) = fit_line(&merged);
let cr = fit_circle(&merged)
    .filter(|&(_, r, _)| r >= min_circle_radius)
    .map(|(_, _, res)| res)
    .unwrap_or(f32::INFINITY);
lr.min(cr)
```

メインループは **「現存する全 run の merge_res のうち最小値、かつ ≤ tol」のペアを 1 つ選んで連結**することを繰り返す:

```rust
loop {
    let mut best: Option<(usize, f32)> = None;
    for i in 0..n {
        if !alive[i] { continue; }
        let r = merge_res[i];
        if r <= tol && best.map_or(true, |(_, br)| r < br) {
            best = Some((i, r));
        }
    }
    let i = match best { Some((i, _)) => i, None => break };
    // i に j = next(i) を吸収
    let j = next_arr[i];
    run_len[i] += run_len[j];
    let nx = next_arr[j];
    next_arr[i] = nx;
    prev_arr[nx] = i;
    alive[j] = false;
    merge_res[j] = f32::INFINITY;

    // i と prev(i) の merge_res を更新（他は変化しないので再計算不要）
    merge_res[i] = ...;
    merge_res[prev_arr[i]] = ...;
}
```

差分更新のミソ:
- マージしたのは `i` と `j` だけなので、影響を受ける `merge_res` は **`i` 自身**（次のペアが変わった）と **`prev(i)`**（後続が i に変わった）の 2 件だけ。
- その他の `merge_res[k]` は値が変わらないのでそのまま使い回す。
- 「最小値の線形走査」自体は O(n) なので全体は O(n²) だが、`merge_residual` の再計算は O(merged) しか発生しない。

なぜ bottom-up なのか:
- top-down (Douglas–Peucker 系) だと「最初にどこで切るか」を残差で決めることになるが、SDF 零等位線では曲線パッチがコーナーで切れるかどうかは勾配の方が確実で、フィット残差だけだと「角を含んだ円弧」に過大フィットしやすい。
- bottom-up + barrier 事前計算なら、各点間に切れ目を入れるかどうかを最初に決めてしまえるので、コーナーが残差判定に紛れ込まない。

## 5. 終了条件と corner artifact 捨て

「`tol` 以下でマージ可能なペアが 1 つもない」状態になったらループを抜ける。

```rust
let mut segs = Vec::new();
let first = (0..n).find(|&i| alive[i]);
if let Some(s) = first {
    let mut i = s;
    loop {
        if run_len[i] >= 3 {
            let run_pts = run_points(i, ...);
            segs.push(best_fit_segment(&run_pts, tol, min_circle_radius));
        }
        i = next_arr[i];
        if i == s { break; }
    }
}
```

`run_len[i] >= 3` のガードは、コーナー直近で barrier に挟まれた 1〜2 点の極小 run を **corner artifact** として捨てるためのもの。marching squares のセル境界補間と Newton 射影の組み合わせで、コーナー付近にどうしても 1〜2 点の「曲がっているように見える」点が残ってしまうが、これを真面目に Line/Circle にしてしまうと EdgeLoop に微小セグメントが混入する。3 点未満は無視するという素朴な閾値で十分捌けている。

## 6. best_fit_segment（Occam バイアス）

```rust
fn best_fit_segment(pts: &[Vec2], tol: f32, min_circle_radius: f32) -> Segment {
    let (lp, ld, lr) = fit_line(pts);
    let circle = fit_circle(pts).filter(|&(_, r, _)| r >= min_circle_radius);
    match circle {
        None => Segment::Line { ... },
        Some((cc, cr, cres)) => {
            if lr <= tol || cres >= lr {
                Segment::Line { ... }       // Line が tol 以下なら問答無用で Line
            } else {
                Segment::Circle { ... }     // それ以外で Circle が勝つときだけ Circle
            }
        }
    }
}
```

Occam バイアス: **直線で間に合うなら直線を選ぶ**。具体的には:
1. Line 残差 `lr` が `tol` 以下なら Circle 残差が小さくても Line を採用。
2. Circle 残差 `cres` が Line 残差 `lr` 以上なら Line。
3. それ以外（Circle が明確に勝つ）のときだけ Circle。

加えて `min_circle_radius` 以下の極小半径は corner straddle の誤フィット扱いで Circle 候補から除外。これで「ほとんど直線な部分を半径数百の巨大円としてフィットしてしまう」事故を防ぐ。

## 7. フィット本体

### fit_line: PCA / TLS

```rust
// 2x2 共分散行列の最大固有ベクトル = 主軸方向
let trace = sxx + syy;
let disc = (trace * trace - 4.0 * (sxx * syy - sxy * sxy)).max(0.0).sqrt();
let lambda_max = 0.5 * (trace + disc);
let dir = ...; // (sxy, lambda_max - sxx) を正規化
```

直交残差（Total Least Squares）を最小化する方向。OLS（縦の残差最小化）ではなく TLS を採用しているのは、点列が垂直に近いケースで OLS が破綻するため。残差 `max_res` は法線方向の最大絶対値（最悪値ノルム）。

### fit_circle: Kasa 線形最小二乗（中心化版）

```rust
// 点を mean シフトしてから x^2+y^2 を z として線形回帰
sxx, sxy, syy, sxz, syz, sz を集計
det = sxx * syy - sxy * sxy
a = (syy * sxz - sxy * syz) / det
b = (sxx * syz - sxy * sxz) / det
c = sz / n
r^2 = c + 0.25 * (a^2 + b^2)
```

Kasa 法は `(x - cx)^2 + (y - cy)^2 = r^2` を `x^2 + y^2 = 2 cx x + 2 cy y + (r^2 - cx^2 - cy^2)` と展開して `(x, y, 1)` で線形回帰する古典的手法。中心化（mean シフト）で数値条件を改善している。`det` が極端に小さい（共線）場合は `None`。

残差は最悪値ノルム `max | |p - center| - radius |`。

## 8. 全体の流れまとめ

```
SDF
 │
 ▼ marching_squares
点列ループ (生)
 │
 ▼ project_loop (Newton射影)
点列ループ (SDF=0上)
 │
 ▼ fit_segments
 │   1. 勾配ジャンプで barrier 計算 (コーナー検出)
 │   2. 1点=1run の双方向リンクリスト初期化
 │   3. bottom-up merge: 最小残差 ≤ tol を貪欲に連結
 │   4. run_len < 3 は corner artifact として捨てる
 │   5. 各 run を best_fit_segment で Line/Circle 化 (Occam)
 ▼
Segment 列 (EdgeLoop)
```

設計上の要点を一言で言うと: **「どこで切るか」を SDF 勾配で先に決め、「どう繋ぐか」を残差ベースの貪欲マージで後から決める**。点列だけ見て両方同時に解こうとすると角を曲線に取り込むなどの誤フィットが起きるが、SDF が提供する勾配情報を切断判断に専用化することで責任分離している。

## 関連定数

- `RES = 1024` — marching squares のセル分割数
- `NEWTON_ITERS = 8` — 射影ニュートン反復回数
- `CORNER_COS_THRESHOLD = 0.955` — コーナー閾値 (cos 0.3 rad ≈ 17°)
- `FIT_TOL_REL = 0.003` — フィット残差許容 (bbox 対角線比)
- `MIN_CIRCLE_RADIUS_REL = 0.003` — Circle 採用最小半径 (bbox 対角線比)

すべてベンチ用 SDF (circle / rectangle / pentagon の単体テスト) で `< 1e-7` 精度に収まる調整。
