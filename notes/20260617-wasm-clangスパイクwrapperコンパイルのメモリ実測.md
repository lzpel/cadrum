# wasm-clang spike: wrapper.cpp コンパイルのメモリ/時間実測（#207 approach 2）

## 背景

#207「rustc だけで wasm を通す」の approach 2 = **wasm 製 clang を配布し、user 側で
`cpp/wrapper.cpp` をコンパイル（compile only → `.o`、リンクは host の rust-lld）→ `.a` 化**。
最大の不確実性は **wasm32 の 4GB 線形メモリ上限内で OCCT 多用の `wrapper.cpp` を
コンパイルできるか**。これを実証する spike。

## 方法（Tier 1: native clang のピーク RSS を proxy 計測）

wasm 上 clang のメモリは native clang のピーク RSS と同オーダー（やや増）になるため、
まず手元の wasi-sdk-33 **native** clang で `wrapper.cpp` を wasm 向けにコンパイルし、
ピーク working set を実測。clang.wasm 不要で即判定できる主ゲート。

- コンパイラ: `out/wasi-sdk-33/bin/clang++`（native exe、target=wasm32-wasip1）
- フラグ: build.rs/Dockerfile 準拠
  `--target=wasm32-wasip1 --sysroot=… -fwasm-exceptions -fexceptions -mllvm -wasm-use-legacy-eh=false -std=c++17 -D_USE_MATH_DEFINES -DCADRUM_COLOR -D_WASI_EMULATED_*`
- include: `target/cxxbridge`（cxx 生成 ffi.rs.h / rust/cxx.h）, `..`（実 `cadrum/cpp/wrapper.h`）,
  `target/occt-8_0_0_rev2-wasm32_unknown_unknown/include/opencascade`
- 計測: PowerShell `Start-Process -PassThru` → `PeakWorkingSet64` をポーリング（単調増加なので真のピーク）, `Stopwatch` で時間
- `-fsyntax-only` 事前検証 → exit 0（include 構成・パース OK）

## 結果

| 最適化 | ピーク RSS | 時間 | 生成 .o |
|---|---|---|---|
| `-O1` | **163.1 MB** | 4.6 s | 346 KB |
| `-O0` | **160.8 MB** | 2.7 s | 978 KB |

## 結論

- **メモリ関門は明確にクリア。** ピーク ~163 MB は wasm32 の 4GB 上限に対し **約25倍の余裕**。
  `wrapper.cpp` は OCCT を呼ぶだけの薄い glue TU で、OCCT 本体は prebuilt のため、
  ヘッダを多数 include してもコンパイル時フットプリントは小さい。
- wasm 上 clang のオーバーヘッド（経験的に native の <2倍程度）を見ても <1GB に収まる見込み。
- よって **approach 2 はメモリ面で成立** = go。

## 残課題（feasibility ではなくロジ/エンジニアリング）

- **Tier 2 未実施**: 実 clang.wasm を wasm ランタイム（wasmtime 等）で実走させ、
  (a) exnref EH 付きコンパイルの成功 (b) wasm インスタンス内メモリ (c) 時間 を確認する段は未了。
  ローカルに wasm ランタイムも clang.wasm も無いため。これは「clang.wasm の入手/自前ビルド」という
  ロジ課題で、メモリ可否の主判断は Tier 1 で確定済み。
- approach 2 本実装に進む場合の構成（別計画）: clang.wasm 配布（LLVM 版ごと、低頻度）＋
  compile-time ヘッダ同梱＋`build.rs` を cxx-gen（glue 生成のみ）＋`wasmtime` build-dep で
  clang.wasm 実行＋`ar` クレートで `.a` 化。consumer は rustc のみ。
- approach 1（PR #208 の prebuilt `cadrum_cpp.a`）は既に低リスクで成立済み。当面のデフォルト維持。
