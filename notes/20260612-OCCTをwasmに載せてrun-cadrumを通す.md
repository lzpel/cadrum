# OCCT を wasm に載せて run-cadrum を通す

`make -C sandbox-wasm run-cadrum` で OCCT(OpenCASCADE 8.0.0) を `wasm32` 上で動かし、
実 B-rep 幾何（`BRepPrimAPI_MakeBox` → `GProp_GProps`）で `Solid::cube(0,(10,20,30)).volume()`
= **6000** を node で出力できた。`20260329-wasmビルド方針_jp.md` の「本命」が実現。

最終確認:
```
run-pure   -> Solid volume: 1
run-cc     -> Solid volume: -0.9589242746631385   (wasi-libc の sin)
run-cxx    -> Solid volume: 5                       (libc++ + wasm EH)
run-cadrum -> Solid volume: 6000                    (OCCT in wasm)
```

## 全体構成（重要な前提）

- **コンパイル target は `wasm32-wasip1`、最終リンクの rustc target は `wasm32-unknown-unknown`**。
  オブジェクトは同じ wasm32 で互換。wasip1 でコンパイルすると `--sysroot` が
  `include/wasm32-wasip1`（例外時は `eh/c++/v1`）を自動解決し、`__wasi__` で libc++ の
  rune table も正規に選ばれる。一方リンクは rustc(rust-lld) が unknown-unknown で行い、
  node/wasm-bindgen が WASI ランタイム無しで読める形にする。
- ツールチェインは wasi-sdk-33 の Windows SDK 1 個（`bin/clang` ＋ 同梱 `share/wasi-sysroot`）。

## フェーズ別の要点（ドライブ・バイ・エラーで反復）

### Phase 0: 例外(eh)ツールチェイン
- OCCT は例外(`Standard_Failure`)/RTTI 多用 → `noeh`/`-fno-exceptions` から
  **`-fwasm-exceptions`** へ。`eh` 版 `libc++ / libc++abi / libunwind` をリンク。
- 罠: cc-rs(cxx-build) が先頭付近に `-fno-exceptions` を付ける。`-fwasm-exceptions` は
  EH モデルを選ぶだけで例外を再有効化しない。**env 末尾(後勝ち)で `-fexceptions` を明示**して解決。
  - 実証: `clang++ -fno-exceptions -fwasm-exceptions` は throw NG、`... -fexceptions` を足すと OK。
- まず最小の `run-cxx` で EH×wasm-bindgen×node を通してから OCCT へ。

### Phase 1: ルート `build.rs` に wasm クロスビルド分岐
`cadrum/build.rs` の `source-build`(cmake) に wasm 分岐を注入（`TARGET` が wasm32 のとき）:
```
.generator("Unix Makefiles")          # ninja 無し / "MinGW Makefiles" は sh.exe を嫌うため
.define("CMAKE_SYSTEM_NAME", "Generic")
.define("CMAKE_C/CXX_COMPILER", wasi-sdk clang/clang++)
.define("CMAKE_AR/RANLIB", llvm-ar/llvm-ranlib)
.define("CMAKE_C/CXX_COMPILER_WORKS", "1")   # クロスで test バイナリのリンク/実行が不可
```
- target/sysroot/`-fwasm-exceptions`/emulation マクロは makefile の `CFLAGS_/CXXFLAGS_<target>`
  経由で cmake クレートが拾い、OCCT の compile flags に流れる。
- bin パスは makefile が `export WASI_SDK_BIN := .../wasi-sdk-33/bin` で渡す。
- 依存縮小: sandbox の cadrum を `default-features=false, features=["source-build","color"]`
  （`png`/tiny-skia を外す。`.color()` 用に `color` は残す）。
- 結果: OCCT の cmake configure は `Generic`+wasi-sdk で**そのまま通った**（OCCT 側パッチ不要）。

### Phase 2: OCCT の POSIX 依存ファイルをスタブ
既存の `patch_or_none`(これまで Windows 用) を wasm にも拡張。`stub_content(path,true)` で
**シグネチャを残しボディだけ潰す**（リンク用シンボルは残す）。sandbox は cube/volume しか
叩かず OSD は実行時未使用。
- `src` 全体を grep して、wasi-libc に無い POSIX ヘッダを include する `.cxx` を一括特定:
  `OSD_*`(File/Directory/Process/Host/Disk/signal/Chronometer/MemInfo/SharedLibrary 等)、
  `Message_PrinterSystemLog.cxx`(syslog)、`STEPConstruct_AP203Context.cxx`(pwd)。
- 罠: ボディを潰しても `#include <netdb.h>` 等が残ると fatal。
  **存在しないヘッダ(`netdb.h`,`dlfcn.h`,`syslog.h`,…)は `comment_out_include_in` で除去**。
- `sys/times.h` は `#error` ガード付き → makefile で
  `-D_WASI_EMULATED_PROCESS_CLOCKS`(他に SIGNAL/MMAN/GETPID) を定義して通す
  （スタブ済みで実体は呼ばないので `-lwasi-emulated-*` のリンクは不要）。

### Phase 3: リンク・WASI import・実行
- リンク: `-lc++` が見つからない → libc++ は `lib/wasm32-wasip1/eh/` サブディレクトリ。
  makefile の `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS` に `eh` の `-L` と
  `c++abi`/`unwind` を追加（未参照なら遅延リンクで pure 等には無害）。
- wasm-opt(binaryen) が EH wasm の Precompute pass で**クラッシュ** →
  `Cargo.toml` の `[package.metadata.wasm-pack.profile.release] wasm-opt=false`。
- node 実行に **`--experimental-wasm-exnref`** が必要（新 exnref EH モデル）。
- WASI import を `src/wasi_stub.c` の no-op スタブで全消し。OCCT は 10 個を要求:
  `environ_get/environ_sizes_get/fd_{close,fdstat_get,prestat_get,prestat_dir_name,read,seek,write}/proc_exit`。
  - 重要: `wasi_stub.c` は従来 cxx+libcxx feature でしかリンクされていなかった。
    `build.rs` を直し **cadrum feature でも whole-archive リンク**するように。
- `env/setjmp`(OCCT `Standard_ErrorHandler`) も import に残る → C++ 例外を使い signal 経路は
  不要なので `setjmp` は 0 を返すだけ、`longjmp` は trap のスタブで解決。
- 実行直後 `__wasm_call_dtors → __funcs_on_exit` で `null function / signature mismatch` クラッシュ
  （wasm-bindgen の command モデルが呼び出し毎に静的 dtor を走らせ、OCCT 静的オブジェクトの
  dtor が不正な間接呼び出しをする）。**`__cxa_atexit` を no-op 化**して静的 dtor 登録自体を抑止
  （一回限り実行なので終了時クリーンアップ不要）→ 解決。

## 触ったファイル
- `sandbox-wasm/makefile` … eh/-fexceptions、emulation マクロ、eh の `-L`、`WASI_SDK_BIN`、
  node の `--experimental-wasm-exnref`。
- `cadrum/build.rs` … wasm cmake クロス分岐、`patch_or_none` の wasm POSIX スタブ＋ヘッダ除去。
- `sandbox-wasm/build.rs` … eh リンク、`wasi_stub` を cadrum でもリンク。
- `sandbox-wasm/src/wasi_stub.c` … WASI import 10 種＋`setjmp/longjmp`＋`__cxa_atexit` スタブ。
- `sandbox-wasm/Cargo.toml` … cadrum features、`wasm-opt=false`。

## メモリ経由の STEP I/O は動く（追記）
- cadrum の I/O は完全にストリームベース（`Solid::write_step<W: Write>` / `read_step<R: Read>`、
  C++ 側は `RustReader/RustWriter` streambuf → `std::istream/ostream`）。スタブ化した `OSD_File`
  層を通らないので **メモリ往復が可能**。sandbox の `volume()` を
  cube→`write_step(Vec<u8>)`→`read_step(Cursor)`→`volume()` に変えても `run-cadrum` は **6000**。
  color(XDE) 込みでも動作（フォールバック不要だった）。
- 追加で必要だった WASI スタブ: `clock_time_get`(STEP ヘッダ時刻)・`fd_fdstat_set_flags`・
  `path_filestat_get`・`path_open`（path 系は NOENT を返して OCCT を内蔵既定へフォールバック）。
- つまり「不可」なのは **ファイルパス指定の I/O のみ**。メモリ⇄Solid は OK。

## 既知の制限 / 今後
- OCCT の OSD 層はスタブ。**ファイルパス指定の I/O・スレッド・ディスク上のリソース/スキーマ参照は不可**
  （`path_open` は NOENT 固定）。メモリ経由の STEP/BRep I/O と純幾何は動く。
  実ファイルや環境依存リソースを使うなら実 wasi シム or JS 側 WASI が要る。
- wasm-opt 無効（最適化のみ。機能影響なし）。生成 wasm は ~3.8MB。
- OCCT ソースビルドは初回数分。以後 `sandbox-wasm/target/cadrum-occt-v800-wasm32-unknown-unknown` を再利用。
- スレッド系(`OSD_Thread/ThreadPool/Parallel`)は今回は未参照で済んだが、並列を使う算法を載せると
  pthread リンクの扱いが必要になる可能性。
