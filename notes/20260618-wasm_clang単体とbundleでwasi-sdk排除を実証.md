# wasm: clang バイナリ＋bundle のみで wasi-sdk install を排除できることの実証（#215 feasibility = GO）

## 背景と検証した命題

#215 は「wasm 化した clang を配布し、consumer から wasi-sdk を完全排除する」提案。本実装（clang.wasm を wasmtime で実走）に踏む前に、ホストの native `clang.exe`（clang.wasm の proxy）で feasibility をホスト上で確定させた。

当初案「`clang.exe` 単体（sysroot 完全抜き）で ffi をコンパイル → 全体 wasm 化」は**不正確**だった：

- **compile** には libc++/libc の**ヘッダ**（sysroot `include/`）が要る。`cpp/wrapper.cpp` は OCCT 経由で大量に、`cpp/wrapper.h` も `<vector><memory><streambuf><cstdint>` を引く。`clang.exe` バイナリ単体には libc++/libc ヘッダは入っていない（入っているのは clang の builtin resource ヘッダ＝`lib/clang/<ver>/include` の stddef.h 等だけ）。
- 「全体の wasm 化」= compile + **link**。link には eh 版 runtime `.a`（sysroot `lib/`）が要る。

→ 正しい命題は「sysroot を**使わない**」ではなく「**consumer が wasi-sdk を install しない**（cadrum が必要な小部分集合＝ヘッダ＋`.a` を再配布する）」。本検証はこれを falsifiable に示した。

## 検証手順（sandbox-wasm 上、再現可能）

OCCT は released prebuilt `occt-8_0_0_rev2-wasm32_unknown_unknown.tar.gz`（#214 前なので `libcadrum_*.a` 非同梱のクリーン版）を `../target` に展開して `OCCT_ROOT` で供給。wasi-sdk-33（`clang.exe` 22.1.0 / wasip1）使用。

- **Stage 0** baseline：既存 `make check-cadrum`（installed sysroot フル使用）が green（`Solid volume: 6000`）。
- **Stage 0 実測**：`clang++ -M cpp/wrapper.cpp`（cxx 生成 glue 込み）で依存ヘッダを列挙。
- **Stage 1** harvest：sandbox-wasm/bundle に必要物を copy。
- **Stage 2** bundle-only build：`--sysroot` を外し `-nostdinc -nostdinc++` ＋明示 `-isystem`（bundle）＋ runtime `.a` の `-L`（bundle）でフルビルド → node で green。
- **Stage 3** isolation（本体の証明）：installed `share/wasi-sysroot` を一時リネームで隠してビルド → **それでも green**。leak（隠した sysroot をまだ読んでいる経路）が無いことを示す。

## 結果（go/no-go）

| Stage | 結果 |
|---|---|
| Stage 2 bundle-only build | **green**（`Solid volume: 6000`） |
| Stage 3 sysroot 隠し（isolated） | **green** → leak 無し |

→ **#215 feasibility = GO**。bundle のみ（installed wasi-sysroot 不在）で wasm 化が完結する。残る wasi-sdk 依存は「**実行される clang バイナリ**」だけで、これがまさに #215 で clang.wasm に置換する対象。compile のメモリは別途 Tier 1（native proxy）で ~163MB（4GB の約25倍余裕）と実測済み。

## bundle 内容と規模（#215 の配布コスト指標）

cadrum が再配布する想定物。consumer 側 wasi-sdk install（数百 MB）を以下で置換できる：

| 区分 | 中身 | 規模 |
|---|---|---|
| **compile ヘッダ（最小・実参照）** | `-M` が指す wasi-sysroot ヘッダ 758 ファイル（libc++ eh 707 ＋ libc 51） | **6.6 MB** |
| **compile ヘッダ（自己完結 triple）** | `include/wasm32-wasip1` 一式（採用。下記の理由） | **34 MB** |
| （参考）full sysroot include | 6 triple 分 | 168 MB |
| **link runtime `.a`（eh 版）** | `libc++.a / libc++abi.a / libunwind.a`（eh）＋ `libc.a` | **13 MB** |

- libc++ ヘッダは `include/wasm32-wasip1/eh/c++/v1/`（target別・**eh別**・**拡張子なし**）に解決される。`-isystem` はこれを最優先に置く。
- clang の builtin resource ヘッダ（`lib/clang/<ver>/include`）は**コンパイラ側の所有物**。#215 では clang.wasm に同梱される類なので bundle ではなく compiler 同梱として扱い、`-isystem` 末尾に置く（`-nostdinc` が builtin 経路を切るため明示が要る。libc++ の `#include_next <stddef.h>` 解決用）。
- link 側 runtime `.a` の bundle は **#214 で既に approach 1 として main にマージ済み**（prebuilt OCCT に `libcadrum_c++abi/unwind/c.a` として同梱）。本検証は残る **compile 側（ヘッダ）** が bundle 可能であることを補完的に示すもの。両者で consumer は rustc + clang(.wasm) のみになる。

## 設計上の判断（最小集合 vs 全 triple）

- `-M` 最小集合（6.6 MB）＋ `-isystem` は、include 解決順が installed sysroot と変わり libc++ の `#include_next <float.h>` が別経路を辿って `bits/float.h` 欠落で落ちた。
- `--sysroot=bundle`（最小集合）は wasi-sdk の eh 選択が内部 config/multilib 依存のため relocate 先で eh `c++/v1` を自動解決できず `<algorithm>` not found。
- → **`include/wasm32-wasip1` を triple 丸ごと copy ＋ `-nostdinc -nostdinc++` ＋明示 `-isystem`** が relocate に最も強く、安定して通った（34 MB）。最小化（6.6 MB 台）は #215 本実装での最適化に回す。

## 再現コマンド

```sh
# 前提: ../target に wasm OCCT prebuilt を展開（OCCT_ROOT 供給用）、wasi-sdk-33 を ../out に展開
cd sandbox-wasm
make check-cadrum            # baseline（installed sysroot フル使用）
make check-cadrum-bundle     # clang.exe + ./bundle のみでビルド（--sysroot 不使用）
make check-cadrum-bundle-isolated  # installed wasi-sysroot を隠して同ビルド → leak 検出
```

`bundle` target が `include/wasm32-wasip1` ＋ eh `.a` を `sandbox-wasm/bundle/` に harvest（`.gitignore` 済み）。`check-cadrum-bundle-isolated` は harvest を sysroot 表示中に済ませ、`bundle-build`（sysroot 非依存）だけを隠した状態で実行する。

## 関連

#215（本提案）, #214（wasm eh runtime `.a` を prebuilt OCCT に bundle＝link 側 approach 1）, #207（self-contained wasm prebuilt）, #199/#204（exnref EH 統一）, #205（wasi_stub / `wasm_start!`）。
