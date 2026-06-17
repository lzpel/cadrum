# approach 2 実走スパイク: GATE 1（clang.wasm 入手）でブロック

#215 approach 2（wasm 化 clang で user 側コンパイル）を実走しようとした記録。
メモリ関門は別ノートで実証済み(~163MB)。本回は **clang.wasm の入手**で停止。

## 試したこと

1. **wasmer 7.1.0** を導入（Windows 公式インストーラ、`~/.wasmer/bin`）。`--volume HOST:GUEST` で
   ホストディレクトリをマップ（Windows のドライブコロン回避のため PowerShell からリポジトリ root を
   `--volume ".:/src"` でマップ＝MSYS のパス変換も回避）。
2. `wasmer run clang/clang -- --version` → **clang 16.0.0 / target wasm32-unknown-wasi**。
3. ヘッダ不要の最小 `throw.cpp`（`try{throw 1;}catch(int){}`）を exnref フラグ
   `-fwasm-exceptions -fexceptions -mllvm -wasm-use-legacy-eh=false -c` で wasm `.o` 化を試行。

## 結果（2つの致命的ブロッカー）

- **(A) clang 16 は exnref 非対応。** 新 wasm EH（try_table/throw_ref, `-wasm-use-legacy-eh=false`）は
  LLVM 19+ 以降。OCCT prebuilt は wasi-sdk-33（LLVM 22）の **exnref** でビルドされており(#204)、
  16 が出す legacy EH とは混在不可。
- **(B) `clang/clang` package は WASI 上で `-c` コンパイルできない。**
  `error: unknown integrated tool '-cc1'` ―clang driver が cc1 を別プロセスで起動しようとするが
  WASI に exec/spawn が無く失敗。この package は実質コンパイラとして駆動できない。

## 他候補も不適

- **wasix-clang**（wasix-org/wasix-clang v0.0.15）: `wasix-llvm.tar.xz` の `bin/clang` を
  **ネイティブ実行**で検証している＝**ネイティブ cross toolchain（clang.wasm ではない）**。
  我々は既に native cross（wasi-sdk-33）を持つので approach 2 の目的（clang を wasm として動かす）に
  寄与しない。

## 結論

「recent-LLVM(exnref対応) ＋ wasm ランタイムで cc1 まで動く ＋ wasi-sdk-33 と ABI 整合」を満たす
**prebuilt な clang.wasm は現状入手できない**。approach 2 を実走するには **wasi-sdk-33 の LLVM(22) から
clang を wasm32-wasi 向けに自前ビルド**する必要があり（数時間・CI 行き、cc1 を in-process で動く構成に
する必要あり）、ローカルでの即時実証は不可。

= 承認済み計画の「不成立時は GATE で停止＋報告」に該当。メモリ関門(別ノート)は clear 済みなので、
approach 2 の残る本質的ハードルは **適切な clang.wasm の生成（CI）** に集約される。

## 環境メモ
- wasmer: `C:\Users\smith\.wasmer\bin\wasmer.exe`（7.1.0）。`--volume ".:/src"` を PowerShell から。
- 再現コマンド例（PowerShell, repo root）:
  `& wasmer run --volume ".:/src" clang/clang -- --target=wasm32-wasip1 -fwasm-exceptions -fexceptions -mllvm -wasm-use-legacy-eh=false -std=c++17 -O1 -c /src/<path>/throw.cpp -o /src/<path>/throw.o`
