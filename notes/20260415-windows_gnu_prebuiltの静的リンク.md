# windows-gnu prebuilt の静的リンク

## 課題

`docker/Dockerfile_x86_64-pc-windows-gnu` が生成する OCCT prebuilt を `stable-x86_64-pc-windows-gnu` ユーザーが取り込んでできる exe を、配布時に追加 DLL を同梱せずとも動作する「自己完結バイナリ」にしたい。具体的には `libgcc_s_seh-1.dll` / `libstdc++-6.dll` / `libwinpthread-1.dll` の 3 つの mingw ランタイム DLL を全て静的に吸収し、最終 exe の runtime dep を `msvcrt.dll` (OS 同梱) と Win32 API DLL のみに絞る。

あわせて、`x86_64-w64-mingw32-gcc` 無印参照によってスレッドモデルが Debian の `update-alternatives` に依存している曖昧さも除去したい。

## 原因

単純に `build.rs` から `cargo:rustc-link-arg=-static` を emit するだけでは libstdc++ が吸収できない。調査の結果、次の二重の障害があることが判明した:

### 障害 1: rustc がネイティブライブラリ列挙前に `-Wl,-Bdynamic` をハードコード

rustc が windows-gnu 向けに生成するリンカコマンドは次の構造:

```
... (object files)
-Wl,-Bstatic
... (Rust rlib 群)
-Wl,-Bdynamic       <-- ここで強制的に Bdynamic に戻される
-lstdc++            <-- link-cplusplus が emit、ここで dynamic 解決
-lkernel32 -lgcc_eh -l:libpthread.a -lmsvcrt -lmingwex -lmingw32 -lgcc -lmsvcrt ...
-Wl,--allow-multiple-definition
-static             <-- build.rs から cargo:rustc-link-arg=-static
```

`-static` は gcc ドライバレベルのフラグで、コマンドライン全体を走査して `-lgcc` / `-lwinpthread` を静的変種へ書き換える。実際これで libgcc と libwinpthread は静的吸収される。しかし `-lstdc++` は既に `-Wl,-Bdynamic` で ld の状態が dynamic に切り替わった後に出現するため、`-static` が手を付ける前に `libstdc++.dll.a` (import library) で解決されて終わる。

`-static-libstdc++` も同じ理由で効かない。`-Wl,-Bstatic -lstdc++ -Wl,-Bdynamic` を末尾に追加しても、link-cplusplus 由来の先行 `-lstdc++` (dynamic) が既に libstdc++-6.dll import を作成済みなので DLL 依存は消えない。

gcc で `x86_64-w64-mingw32-g++-posix /tmp/t.cpp -static-libgcc -static-libstdc++` を直接実行すると期待通り静的リンクできるのに、rustc 経由では同じ結果にならないのはこのためである。

### 障害 2: link-cplusplus の emit を切り替えるには CXXSTDLIB 経由、検索パスは別途 RUSTFLAGS 経由

`cxx` が間接的に依存する `link-cplusplus` クレートは、`cc` crate 経由で `CXXSTDLIB` / `CXXSTDLIB_<target>` / `TARGET_CXXSTDLIB` の環境変数を読み、その値をそのまま `cargo:rustc-link-lib={value}` として emit する。value に rustc の link-lib 修飾子 (`static=stdc++` など) を渡すとそれが直接 `-l static=stdc++` として渡るので、`CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++` を設定すれば link-cplusplus の emit が static 扱いになる。

ただし、rustc は `#[link(name="stdc++", kind="static")]` を含むクレート (link-cplusplus 本体) をコンパイルする時点で `libstdc++.a` が `-L` 検索パス上に存在することをチェックする。link-cplusplus は cxx → cadrum のビルドチェーンで cadrum よりずっと先にビルドされるため、cadrum の `build.rs` から `cargo:rustc-link-search=native=...` を emit してもそのときには既にチェックが終わっており、error: `could not find native static library 'stdc++'` で link-cplusplus 自体のコンパイルが失敗する。

したがって libstdc++.a の検索パスは build.rs より早いタイミングで、全クレートに対して与えなければならない。実質 `RUSTFLAGS="-L <dir>"` を環境変数として cargo 起動前に与えるのが唯一の経路となる。

## 解決策

3 つの仕掛けを組み合わせる:

### 1. `build.rs`: `-static` を windows-gnu のみに emit

```rust
if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
{
    println!("cargo:rustc-link-arg=-static");
}
```

これで libgcc と libwinpthread が吸収される。linux-gnu で誤爆しないよう OS+ENV 両方で gate する (linux で `-static` は glibc を静的リンクしようとして失敗する)。

### 2. `Dockerfile`: `CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++`

```dockerfile
ENV CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++
```

link-cplusplus の cc crate 経由 emit を `cargo:rustc-link-lib=static=stdc++` に切り替える。これにより rustc が生成するリンカコマンドで `-lstdc++` が `-Wl,-Bstatic -lstdc++ -Wl,-Bdynamic` のブラケットで囲まれ、`libstdc++.a` に解決される。

### 3. `Dockerfile`: entrypoint wrapper で `RUSTFLAGS=-L <dir>` を baked-in

```dockerfile
RUN LIBSTDCXX_DIR="$(dirname "$(x86_64-w64-mingw32-g++-posix -print-file-name=libstdc++.a)")" && \
    printf '#!/bin/sh\nexport RUSTFLAGS="-L %s ${RUSTFLAGS:-}"\nexec /entrypoint.sh "$@"\n' \
        "$LIBSTDCXX_DIR" > /entrypoint-wrapper.sh && \
    chmod +x /entrypoint-wrapper.sh

ENTRYPOINT ["/entrypoint-wrapper.sh"]
```

イメージビルド時に `g++-posix -print-file-name=libstdc++.a` で sysroot 内のパスを動的に取得し、それを含む `RUSTFLAGS` を固定値としてラッパースクリプトに焼き込む。Debian が gcc を bump したらイメージ再ビルド時に自動で追従する。`cargo:rustc-link-search` を build.rs から emit しても link-cplusplus の rlib コンパイル時には間に合わないので、この経路が実質唯一の解。

### 4. `Dockerfile`: posix サフィックスを明示

```dockerfile
ENV CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
ENV CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix
```

Debian `update-alternatives` の既定は現状 posix だが、将来 win32 に変わると silently ABI が壊れる。defensive に明示しておく (`ar` はスレッドモデルに依存しないのでサフィックスなし)。

## 検証結果

`01_primitives` を prebuilt Docker イメージ内で `cargo build --release --example` し、`x86_64-w64-mingw32-objdump -p` で DLL import table を確認:

```
DLL Name: KERNEL32.dll
DLL Name: msvcrt.dll
DLL Name: ntdll.dll
DLL Name: USERENV.dll
DLL Name: WS2_32.dll
DLL Name: api-ms-win-core-synch-l1-2-0.dll
DLL Name: bcryptprimitives.dll
```

全て OS 同梱 DLL のみ。`libgcc_s_seh-1.dll` / `libstdc++-6.dll` / `libwinpthread-1.dll` はいずれも imports に現れない。

## downstream ユーザーへの波及

cadrum を依存に持つ外部クレートから windows-gnu ビルドする場合、**同じ環境変数 2 点を downstream 側の build 環境にも設定する必要がある**:

```bash
export CXXSTDLIB_x86_64_pc_windows_gnu=static=stdc++
export RUSTFLAGS="-L /usr/lib/gcc/x86_64-w64-mingw32/14-posix"  # mingw gcc バージョンに応じて
```

または `.cargo/config.toml` で等価な設定を書く。これは `build.rs` からは制御不能な領域 (別クレートのビルドスクリプト環境には踏み込めないため)。README に記載する形で UX を補う。

環境変数を設定しない downstream では、リンク自体は成功するが最終 exe が `libstdc++-6.dll` に動的依存する状態になる (cadrum が `build.rs` で `-static` を emit しているので libgcc / libwinpthread は引き続き静的吸収される)。配布時に libstdc++-6.dll を同梱する前提なら十分実用的。

## 却下した案

- **UCRT に寄せる (`x86_64-pc-windows-gnullvm` + llvm-mingw)**: 理想的だが Rust ターゲット triple 変更を伴い、既存の gnu ユーザーを切り捨てる。release ワークフロー / Dockerfile ファイル名 / prebuilt tarball 名の大規模変更が必要。今回の fix スコープでは過剰。
- **Debian trixie の `gcc-mingw-w64-ucrt64` で OCCT だけ UCRT 化**: Rust libstd が msvcrt 決め打ちなので二重 CRT になり、`__acrt_iob_func` 等の UCRT 固有シンボルが未解決になる。
- **`-static-libgcc -static-libstdc++ -Wl,-Bstatic -lwinpthread -Wl,-Bdynamic` のみ**: rustc のハードコードされた `-Wl,-Bdynamic` を上書きできないため libstdc++ が吸収されない。
- **`cargo:rustc-link-search=native=...` を build.rs から emit**: link-cplusplus の rlib コンパイル時には間に合わず、error: `could not find native static library 'stdc++'` で失敗する。
- **`.cargo/config.toml` で RUSTFLAGS 設定**: cadrum ワークスペース内のビルドにしか効かず、downstream ユーザーには届かない (そもそも downstream UX は環境変数で要求する前提に割り切った)。

## ソース

- [How can I statically link libstdc++-6 when cross compiling to x86_64-pc-windows-gnu — rust-lang forum](https://users.rust-lang.org/t/how-can-i-statically-link-libstdc-6-when-cross-compilint-to-x86-64-pc-windows-gnu-from-linux/106587)
- [rustc -C link-args=-static-libgcc does not work on Windows — rust-lang/rust#15420](https://github.com/rust-lang/rust/issues/15420)
- [Statically link libstdc++ on windows-gnu (rustc 内部 PR) — rust-lang/rust#65911](https://github.com/rust-lang/rust/pull/65911)
- [Consider dynamically link to libgcc_s when targeting windows-gnu — rust-lang/rust#89919](https://github.com/rust-lang/rust/issues/89919)
- [link-cplusplus crate docs](https://docs.rs/link-cplusplus/latest/link_cplusplus/)
- [Binding c++ with cxx error on windows-gnu — rust-lang/rust#137301](https://github.com/rust-lang/rust/issues/137301)
- 本リポジトリ PR: [lzpel/cadrum#60](https://github.com/lzpel/cadrum/pull/60)
