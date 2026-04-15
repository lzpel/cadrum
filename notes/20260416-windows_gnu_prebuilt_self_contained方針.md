# windows-gnu prebuilt で self-contained exe を目指す方針

## 発端: mandolin の DLL 依存観察

純 Rust の `cargo install mandolin` で作られた `.exe` を `objdump -p` で覗くと、windows-gnu ツールチェーン産でも下記のような極めてクリーンな外部依存で済んでいた。

```text
api-ms-win-core-synch-l1-2-0.dll
bcryptprimitives.dll
kernel32.dll / KERNEL32.dll
api-ms-win-crt-environment-l1-1-0.dll
api-ms-win-crt-heap-l1-1-0.dll
api-ms-win-crt-locale-l1-1-0.dll
api-ms-win-crt-math-l1-1-0.dll
api-ms-win-crt-private-l1-1-0.dll
api-ms-win-crt-runtime-l1-1-0.dll
api-ms-win-crt-stdio-l1-1-0.dll
api-ms-win-crt-string-l1-1-0.dll
ntdll.dll
USERENV.dll
WS2_32.dll
```

すべて OS 同梱 DLL ないし UCRT shim。`libstdc++-6.dll` / `libgcc_s_seh-1.dll` / `libwinpthread-1.dll` は一切出てこない。

**cadrum の prebuilt を使った downstream の `.exe` もこのレベルの依存セットに揃えたい** というのが本メモの主題。

---

## 層 1: CRT を合わせる

mandolin の依存に `api-ms-win-crt-*` が並んでいることは、そのバイナリが **UCRT バインド**である決定的な証拠。`msvcrt.dll` は自前の export セットを持っており、`api-ms-win-crt-*` には forward しない。つまりユーザ環境の Rust windows-gnu toolchain は、rustup が同梱する self-contained mingw (llvm-mingw 系) 経由で **UCRT にバインドされている** (Rust 1.80 前後以降の動向と一致)。

一方、cadrum の現 `docker/Dockerfile_x86_64-pc-windows-gnu` は Debian apt の `mingw-w64-x86-64-posix` を使っており、これは **msvcrt バインド**。cadrum prebuilt と downstream Rust の CRT がズレる → `FILE*` などの CRT 構造体を C++ 側から触る箇所で未定義動作の危険がある。OCCT は内部で `std::fstream` / `std::FILE*` を触るので無視できない。

### 対応案

- **(A) llvm-mingw ベースに乗り換え**
  Dockerfile の base image を `mstorsjo/llvm-mingw` に切り替える。UCRT + 全ランタイム静的化が既定で、Rust 公式の self-contained mingw と同じ世界に着地する。CRT ミスマッチも unwind model ミスマッチも一発で消える。**第一候補**。

- **(B) Debian apt の UCRT 版 mingw を使う**
  `gcc-mingw-w64-x86-64-posix` の代わりに UCRT 対応パッケージを入れる。Debian trixie での提供状況・安定性は未調査。

- **(C) 現状維持 (msvcrt)**
  downstream Rust が msvcrt バインドのユーザだけが安全に使える縛り。今のユーザ環境で既に危険。非推奨。

---

## 層 2: 全 MinGW ランタイムを静的吸収する

`libstdc++-6.dll` / `libgcc_s_seh-1.dll` / `libwinpthread-1.dll` を最終 `.exe` から消すには、それぞれの `.a` を最終リンクに静的に吸わせる必要がある。

- **libgcc / libwinpthread**: 既に `build.rs` の `cargo:rustc-link-arg=-static` で処理済み。downstream にも自動適用される。
- **libstdc++**: 現状は Dockerfile の RUSTFLAGS bake で cadrum 自身のテストビルド (`01_primitives`) だけが救われている状態。**downstream の `cargo add cadrum` ユーザには効かない**。

llvm-mingw (層 1 案 A) に乗り換えた場合、C++ 標準ライブラリは `libc++.a` 相当になり、llvm-mingw のデフォルトで静的吸収される傾向が強いので、libstdc++ 問題の大部分は副産物として解ける。

msvcrt/Debian apt 路線を維持する場合は:

- **build.rs から `cargo:rustc-link-arg=-static-libstdc++ -static-libgcc` を emit** する
  - gcc ドライバが直接解釈するオプションなので、最終リンクのみに作用する。rlib embedding のタイミング問題を踏まない。
  - downstream ユーザは環境変数を設定せずに自動的に静的リンクの恩恵を受ける。
  - CXXSTDLIB override 経由で link-cplusplus を静的化する必要がなくなる → RUSTFLAGS -L のラッパーも不要になる。
  - 副作用: link-cplusplus が emit する dynamic `-lstdc++` と競合する可能性。`--allow-multiple-definition` で抑えられるが実測必須。

**downstream 自動化の鍵は build.rs 側で完結させること**。現在の Dockerfile ラッパーは cadrum 自身のビルドしか救わないので、配布戦略としては不完全。

---

## 層 3: C++ 例外と unwind model

mandolin が軽いのは「Rust が C++ 例外に触れない」ため。純 Rust では libstdc++ の typeinfo / `__cxa_*` が参照されないので `-lstdc++` が undefined references に現れない。

cadrum は OCCT (大量の C++) を取り込むので、downstream の `.exe` には C++ 例外ハンドラが埋め込まれる。これ自体は DLL 依存を増やさない (`libgcc_eh.a` / llvm-mingw の `libunwind.a` を静的吸収できれば)。

ただし gcc と llvm-mingw は unwind の実装系が異なり、**cadrum prebuilt と downstream Rust が同じ unwind model (SEH / Dwarf2) を使う必要がある**。混在すると C++ 例外が Rust の panic boundary を跨ぐ箇所で崩壊する。

llvm-mingw は SEH、Rust の self-contained mingw も SEH、gcc mingw-w64 (Debian apt) は SEH。x86_64 Windows ではどの経路も SEH に揃うので、実害は層 1 の CRT ミスマッチほど派手ではないが、llvm-mingw に統一しておけば将来の例外系まわりの揺れを一元化できる。

---

## 推奨ロードマップ

1. **事実確認**
   ユーザ側 Windows で `rustc --version --verbose` を実行し、Rust の host triple と release channel を確認。合わせて C++ を 1 箇所でも噛ませた小さな Rust プログラムをビルドし、`objdump -p` で mandolin と同じ依存セットに収まるか、それとも libstdc++ が顔を出すかを観測。

2. **Dockerfile を llvm-mingw に載せ替え**
   `docker/Dockerfile_x86_64-pc-windows-gnu` の base image を `mstorsjo/llvm-mingw` に切り替え、Debian apt mingw を廃止。`CC` / `CXX` / `AR` / `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` を llvm-mingw のバイナリ名に揃える。

3. **build.rs で downstream 自動化**
   `cargo:rustc-link-arg=-static-libgcc -static-libstdc++` (llvm-mingw では `-static-libgcc -static-libstdc++` equivalent) を windows-gnu ブランチで emit。これで downstream の最終リンクが常に静的吸収される。

4. **Dockerfile ラッパーの撤去**
   現 Dockerfile の CXXSTDLIB override + RUSTFLAGS -L 用 `/usr/local/bin/with-rustflags` ラッパーを削除。build.rs 側で完結するので不要。

5. **検証**
   完成した prebuilt を使って downstream で `cargo install` → 生成された `.exe` を `objdump -p` で観察し、mandolin と同じ (あるいは OCCT 由来の OS 同梱 DLL `ADVAPI32.dll` / `GDI32.dll` などが増える程度の) クリーンな依存セットに収まっていることを確認。

**ゴールの明示**: 層 1 + 層 2 + 層 3 が同時に成立したとき、cadrum を依存に含めた downstream の `.exe` は mandolin と同水準の portable binary として配布できる状態になる。現状はどの層も半分ずつ欠けている。

---

## 現状整理 (2026-04-16 時点)

- 層 1: **未対応**。Debian apt msvcrt mingw を使用中。Rust 側 UCRT とのミスマッチリスクあり。
- 層 2: **部分対応**。libgcc / libwinpthread は静的吸収済み (`build.rs` `-static`)。libstdc++ は Dockerfile の RUSTFLAGS bake で cadrum 自身のビルドだけが通る状態。downstream 未対応。
- 層 3: **偶然揃っている**。SEH unwind は Debian gcc mingw / llvm-mingw / Rust self-contained mingw すべて共通なので現状は事故らない。ただし将来の toolchain 変動に対して脆弱。

次アクションとしては「層 1 の事実確認」→「Dockerfile を llvm-mingw へ」→「build.rs に `-static-libstdc++` 追記」の順で詰めるのが最短。
