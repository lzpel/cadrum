The objective target is that `make -C sandbox-wasm run-cadrum` generate some wasm file.

I think that wasm32-unknown-unknown has no standard C and standard C++, so I should add C and C++ header and implementation which are independent from OS.

- C++:
    - llvm's libc++ https://github.com/llvm/llvm-project/tree/main/libcxx/include
- C
   - wasm-libc's top-half https://github.com/WebAssembly/wasi-libc/blob/main/libc-top-half/musl/include/math.h
      - this is from musl(https://musl.libc.org/)

- wasi-sdk bundle prebuilt libc and libc++ for wasm. (3~9MB)

```
$ curl -L -O https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-33/wasi-sdk-33.0-x86_64-linux.tar.gz &&
$ tar xzf wasi-sdk-33.0-x86_64-linux.tar.gz
$ find . \( -name libc.a -or -name libc++.a \) -exec ls -lh {} +
-rw-r--r-- 1 smith 197121 8.4M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi-threads/eh/libc++.a
-rw-r--r-- 1 smith 197121 9.6M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi-threads/eh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 3.3M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi-threads/libc.a
-rw-r--r-- 1 smith 197121 5.7M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi-threads/llvm-lto/22.1.0-wasi-sdk/libc.a
-rw-r--r-- 1 smith 197121 7.7M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi-threads/noeh/libc++.a
-rw-r--r-- 1 smith 197121 9.0M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi-threads/noeh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 8.4M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi/eh/libc++.a
-rw-r--r-- 1 smith 197121 9.6M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi/eh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 3.0M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi/libc.a
-rw-r--r-- 1 smith 197121 5.3M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi/llvm-lto/22.1.0-wasi-sdk/libc.a
-rw-r--r-- 1 smith 197121 7.8M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi/noeh/libc++.a
-rw-r--r-- 1 smith 197121 9.0M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasi/noeh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 8.4M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1-threads/eh/libc++.a
-rw-r--r-- 1 smith 197121 9.6M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1-threads/eh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 3.3M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1-threads/libc.a
-rw-r--r-- 1 smith 197121 5.7M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1-threads/llvm-lto/22.1.0-wasi-sdk/libc.a
-rw-r--r-- 1 smith 197121 7.8M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1-threads/noeh/libc++.a
-rw-r--r-- 1 smith 197121 9.0M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1-threads/noeh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 8.4M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1/eh/libc++.a
-rw-r--r-- 1 smith 197121 9.6M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1/eh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 3.0M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1/libc.a
-rw-r--r-- 1 smith 197121 5.3M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1/llvm-lto/22.1.0-wasi-sdk/libc.a
-rw-r--r-- 1 smith 197121 7.8M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1/noeh/libc++.a
-rw-r--r-- 1 smith 197121 9.0M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip1/noeh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 8.4M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip2/eh/libc++.a
-rw-r--r-- 1 smith 197121 9.6M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip2/eh/llvm-lto/22.1.0-wasi-sdk/libc++.a
-rw-r--r-- 1 smith 197121 3.5M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip2/libc.a
-rw-r--r-- 1 smith 197121 6.1M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip2/llvm-lto/22.1.0-wasi-sdk/libc.a
-rw-r--r-- 1 smith 197121 7.8M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip2/noeh/libc++.a
-rw-r--r-- 1 smith 197121 9.0M May  1 02:08 ./wasi-sdk-33.0-x86_64-linux/share/wasi-sysroot/lib/wasm32-wasip2/noeh/llvm-lto/22.1.0-wasi-sdk/libc++.a
```

## Experiment 0: pure ✅ok

make -C sandbox-wasm run-pure

## Experiment 1: cc ✅ok

make minimum crate sandbox_wasm_add which includes simple C `int add(int a, int b){return a+b;}` and built into wasm.
provide C and C++ from llvm's libc++ and wasi-libc

make -C sandbox-wasm run-cpp

❌ Failed — failed to find tool "clang++"

I should download llvm like https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/clang+llvm-18.1.8-x86_64-pc-windows-msvc.tar.xz

My rust toolchain is `1.93.0-x86_64-pc-windows-gnu` and llvm release contains no binaries for  windows-gnu, has only for windows-msvc.

Q. Should I change my toolchain for the release A. No, that means clang.exe is built with msvc, does not mean the target is msvc.

#### I move one step further

```
0107411268@SIEDL6YJ54 MINGW64 ~/cadrum (feature/sandbox-wasm)
$ make -C sandbox-wasm run-cpp
make: Entering directory 'C:/Users/0107411268/cadrum/sandbox-wasm'
cargo install --root ../out wasm-pack
    Updating crates.io index
     Ignored package `wasm-pack v0.15.0` is already installed, use --force to override
warning: be sure to add `C:\Users\0107411268\cadrum\out\bin` to your PATH to be able to run the installed binaries
ls ../out/clang+llvm-18.1.8-x86_64-pc-windows-msvc || ( cd ../out && curl -L -O https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/clang+llvm-18.1.8-x86_64-pc-windows-msvc.tar.xz && tar xf clang+llvm-18.1.8-x86_64-pc-windows-msvc.tar.xz )
bin  include  lib  libexec  share
cargo build --target wasm32-unknown-unknown --features cpp
   Compiling link-cplusplus v1.0.12
   Compiling cxx v1.0.194                                                                                                                                                                                                                                                 
warning: cxx@1.0.194: In file included from C:\Users\0107411268\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\cxx-1.0.194\src/cxx.cc:1:                                                                                                                            
warning: cxx@1.0.194: C:\Users\0107411268\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\cxx-1.0.194\src\../include/cxx.h:2:10: fatal error: 'algorithm' file not found
warning: cxx@1.0.194:     2 | #include <algorithm>
warning: cxx@1.0.194:       |          ^~~~~~~~~~~
warning: cxx@1.0.194: 1 error generated.
error: failed to run custom build command for `cxx v1.0.194`
...
```

#### Building with c source is success.

```
make -C sandbox-wasm run-cc
```

## Experiment 2: cc + libc ✅ok

```
make -C sandbox-wasm run-cc-libc
```

I would like to know the difference among follwings
https://github.com/kraj/musl/tree/kraj/master/include
https://github.com/llvm/llvm-project/tree/main/libc
https://github.com/WebAssembly/wasi-libc

```
● 3つはどれも「C標準ライブラリ(libc)」ですが、「移植可能な上半分(関数の実装)」と「OSに触る下半分(システムコール)」をどう持つかで性格が全く違います。

  それぞれの正体

  1. musl (kraj/musl)

  - Linux 向けの libc 実装。kraj/musl は本家 musl の GitHub ミラーで、include/ は標準ヘッダ群（math.h/stdio.h…）。
  - 軽量・クリーンで有名だが、下半分が Linux のシステムコール ABI に直結している。
  - → wasm にはそのままでは使えない（Linux syscall が無いため）。ただしコードの「上半分」（文字列処理・math・printf 整形・malloc 等、OS非依存の部分）は移植性が高い。

  2. llvm-libc (llvm-project/libc)

  - LLVM がゼロから書き直している新しい libc。musl とは別系統の独立コードベース。
  - モジュール化・overlay/full-build 対応を狙うが、まだ実験的で未完成。
  - wasm 向けの定番ではない。今回の用途で第一選択にはならない。

  3. wasi-libc (WebAssembly/wasi-libc)

  - wasm/WASI 専用の libc。これが実際に wasm32-wasi をビルドするとき使うもの。
  - 構造が肝で、ハイブリッドになっている:
    - 上半分（top-half）= musl 由来（issue 本文の「this is from musl」がこれ）。math・string・stdio 整形・dlmalloc など OS非依存部分を musl から流用。
    - 下半分（bottom-half）= WASI 実装。Linux syscall の代わりに WASI の API に対して OS インタフェースを実装。
  - つまり 「musl の移植可能な部分 + WASI のOS層」 = wasm のための libc。

  関係を一言で

  musl        : (musl上半分) + (Linux syscall 下半分)   → Linux 用
  wasi-libc   : (musl上半分) + (WASI       下半分)      → wasm/WASI 用  ← これを使う
  llvm-libc   : 全部 LLVM が独自実装（別物・実験的）     → 今は非推奨
```

### libc供給用に make -C sandbox-wasm generate で wasi-libcがビルドされるようになった

### make -C sandbox-wasm run-cc-libcでfeature cc,libcが渡り sinが動くことを確認

## Experiment 3: cxx

wasi-sdkのなかにllvm libcxx のコンパイル設定が書いてある

https://github.com/WebAssembly/wasi-sdk/blob/5faf80805397ae2a96ab224d1f103798af06dd92/cmake/wasi-sdk-sysroot.cmake#L241


```
# =============================================================================
# libcxx build logic
# =============================================================================

execute_process(
  COMMAND ${CMAKE_C_COMPILER} -dumpversion
  OUTPUT_VARIABLE llvm_version
  OUTPUT_STRIP_TRAILING_WHITESPACE)

function(define_libcxx_sub target target_suffix extra_target_flags extra_libdir_suffix exceptions)
  if(${target} MATCHES threads)
    set(pic OFF)
    set(target_flags -pthread)
  else()
    set(pic ON)
    set(target_flags "")
  endif()
  if(${target_suffix} MATCHES lto)
    set(pic OFF)
  endif()
  list(APPEND target_flags ${extra_target_flags})

  set(runtimes "libcxx;libcxxabi")

  get_property(dir_compile_opts DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR} PROPERTY COMPILE_OPTIONS)
  get_property(dir_link_opts DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR} PROPERTY LINK_OPTIONS)
  set(extra_flags
    ${WASI_SDK_CPU_CFLAGS}
    ${target_flags}
    --target=${target}
    ${dir_compile_opts}
    ${dir_link_opts}
    --sysroot ${wasi_sysroot}
    -resource-dir ${wasi_resource_dir})

  set(exnsuffix "")

  if (exceptions)
    # TODO: lots of builds fail with shared libraries and `-fPIC`. Looks like
    # things are maybe changing in llvm/llvm-project#159143 but otherwise I'm at
    # least not really sure what the state of shared libraries and exceptions
    # are. For now shared libraries are disabled and supporting them is left for
    # a future endeavor.
    set(pic OFF)
    set(runtimes "libunwind;${runtimes}")
    list(APPEND extra_flags -fwasm-exceptions -mllvm -wasm-use-legacy-eh=false)
    if (WASI_SDK_EXCEPTIONS STREQUAL "DUAL")
      set(exnsuffix "/eh")
    endif()
  else()
    if (WASI_SDK_EXCEPTIONS STREQUAL "DUAL")
      set(exnsuffix "/noeh")
    endif()
  endif()

  # The `wasm32-wasi` target is deprecated in clang, so ignore the deprecation
  # warnings for now.
  if(${target} STREQUAL wasm32-wasi OR ${target} STREQUAL wasm32-wasi-threads)
    list(APPEND extra_flags -Wno-deprecated)
  endif()

  # `shared` is computed here, after the exceptions branch above may have forced
  # pic OFF, so that LIBCXX_ENABLE_SHARED/LIBCXXABI_ENABLE_SHARED/LIBUNWIND_ENABLE_SHARED
  # stay consistent with the final value of CMAKE_POSITION_INDEPENDENT_CODE.
  if(WASI_SDK_BUILD_SHARED AND pic)
    set(shared ON)
  else()
    set(shared OFF)
  endif()

  set(extra_cflags_list ${CMAKE_C_FLAGS} ${extra_flags})
  list(JOIN extra_cflags_list " " extra_cflags)
  set(extra_cxxflags_list ${CMAKE_CXX_FLAGS} ${extra_flags})
  list(JOIN extra_cxxflags_list " " extra_cxxflags)

  ExternalProject_Add(libcxx-${target}${target_suffix}-build
    SOURCE_DIR ${llvm_proj_dir}/runtimes
    CMAKE_ARGS
      ${default_cmake_args}
      # Ensure headers are installed in a target-specific path instead of a
      # target-generic path.
      -DCMAKE_INSTALL_INCLUDEDIR=${wasi_sysroot}/include/${target}${exnsuffix}
      -DCMAKE_STAGING_PREFIX=${wasi_sysroot}
      -DCMAKE_POSITION_INDEPENDENT_CODE=${pic}
      -DLIBCXX_ENABLE_THREADS:BOOL=ON
      -DLIBCXX_HAS_PTHREAD_API:BOOL=ON
      -DLIBCXX_HAS_EXTERNAL_THREAD_API:BOOL=OFF
      -DLIBCXX_HAS_WIN32_THREAD_API:BOOL=OFF
      -DLLVM_COMPILER_CHECKED=ON
      -DLIBCXX_ENABLE_SHARED:BOOL=${shared}
      -DLIBCXX_ENABLE_EXCEPTIONS:BOOL=${exceptions}
      -DLIBCXX_ENABLE_FILESYSTEM:BOOL=ON
      -DLIBCXX_ENABLE_ABI_LINKER_SCRIPT:BOOL=OFF
      -DLIBCXX_CXX_ABI=libcxxabi
      -DLIBCXX_HAS_MUSL_LIBC:BOOL=OFF
      -DLIBCXX_ABI_VERSION=2
      -DLIBCXXABI_ENABLE_EXCEPTIONS:BOOL=${exceptions}
      -DLIBCXXABI_ENABLE_SHARED:BOOL=${shared}
      -DLIBCXXABI_SILENT_TERMINATE:BOOL=ON
      -DLIBCXXABI_ENABLE_THREADS:BOOL=ON
      -DLIBCXXABI_HAS_PTHREAD_API:BOOL=ON
      -DLIBCXXABI_HAS_EXTERNAL_THREAD_API:BOOL=OFF
      -DLIBCXXABI_HAS_WIN32_THREAD_API:BOOL=OFF
      -DLIBCXXABI_USE_LLVM_UNWINDER:BOOL=${exceptions}
      -DLIBUNWIND_ENABLE_SHARED:BOOL=${shared}
      -DLIBUNWIND_ENABLE_THREADS:BOOL=ON
      -DLIBUNWIND_USE_COMPILER_RT:BOOL=ON
      -DLIBUNWIND_INCLUDE_TESTS:BOOL=OFF
      -DUNIX:BOOL=ON
      -DCMAKE_C_FLAGS=${extra_cflags}
      -DCMAKE_ASM_FLAGS=${extra_cflags}
      -DCMAKE_CXX_FLAGS=${extra_cxxflags}
      -DLIBCXX_LIBDIR_SUFFIX=/${target}${exnsuffix}${extra_libdir_suffix}
      -DLIBCXXABI_LIBDIR_SUFFIX=/${target}${exnsuffix}${extra_libdir_suffix}
      -DLIBUNWIND_LIBDIR_SUFFIX=/${target}${exnsuffix}${extra_libdir_suffix}
      -DLIBCXX_INCLUDE_TESTS=OFF
      -DLIBCXX_INCLUDE_BENCHMARKS=OFF

    # See https://www.scivision.dev/cmake-externalproject-list-arguments/ for
    # why this is in `CMAKE_CACHE_ARGS` instead of above
    CMAKE_CACHE_ARGS
      -DLLVM_ENABLE_RUNTIMES:STRING=${runtimes}
    DEPENDS
      wasi-libc-${target}
      compiler-rt
    EXCLUDE_FROM_ALL ON
    USES_TERMINAL_CONFIGURE ON
    USES_TERMINAL_BUILD ON
    USES_TERMINAL_INSTALL ON
    USES_TERMINAL_PATCH ON
    PATCH_COMMAND
      ${CMAKE_COMMAND} -E chdir .. bash -c
        "git apply ${CMAKE_SOURCE_DIR}/src/llvm-pr-168449.patch || git apply ${CMAKE_SOURCE_DIR}/src/llvm-pr-168449.patch -R --check"
    COMMAND
      ${CMAKE_COMMAND} -E chdir .. bash -c
        "git apply ${CMAKE_SOURCE_DIR}/src/llvm-pr-186054.patch || git apply ${CMAKE_SOURCE_DIR}/src/llvm-pr-186054.patch -R --check"
    COMMAND
      ${CMAKE_COMMAND} -E chdir .. bash -c
        "git apply ${CMAKE_SOURCE_DIR}/src/llvm-pr-185770.patch || git apply ${CMAKE_SOURCE_DIR}/src/llvm-pr-185770.patch -R --check"
  )
  add_dependencies(libcxx-${target} libcxx-${target}${target_suffix}-build)
endfunction()

function(define_libcxx_and_lto target target_suffix exceptions)
  define_libcxx_sub(${target} "${target_suffix}" "" "" ${exceptions})
  if (WASI_SDK_LTO)
    # Note: clang knows this /llvm-lto/${llvm_version} convention.
    # https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/clang/lib/Driver/ToolChains/WebAssembly.cpp#L204-L210
    define_libcxx_sub(${target} ${target_suffix}-lto "-flto=full" "/llvm-lto/${llvm_version}" ${exceptions})
  endif()
endfunction()

function(define_libcxx target)
  add_custom_target(libcxx-${target})

  # For dual-mode exceptions-and-not there are two versions of libcxx which are
  # compiled and placed into the sysroot. They're named slightly differently to
  # have unique CMake rules.
  #
  # Otherwise there's only one build of libcxx and it's either got exceptions or
  # it doesn't depending on configuration.
  if (WASI_SDK_EXCEPTIONS STREQUAL "DUAL")
    define_libcxx_and_lto(${target} "" OFF)
    define_libcxx_and_lto(${target} "-exn" ON)
  elseif(WASI_SDK_EXCEPTIONS STREQUAL "ON")
    define_libcxx_and_lto(${target} "" ON)
  else()
    define_libcxx_and_lto(${target} "" OFF)
  endif()

  # As of this writing, `clang++` will ignore the target-specific include dirs
  # unless this one also exists:
  add_custom_target(libcxx-${target}-extra-dir
    COMMAND ${CMAKE_COMMAND} -E make_directory ${wasi_sysroot}/include/c++/v1
    COMMENT "creating libcxx-specific header file folder")
  add_dependencies(libcxx-${target} libcxx-${target}-extra-dir)
endfunction()

foreach(target IN LISTS WASI_SDK_TARGETS)
  define_libcxx(${target})
endforeach()
```

### cxx ビルド完了 ✅ok

`cxx` crate 経由の C++ を libc++ とリンクして wasm 化、node で実行できるようになった。

```
make -C sandbox-wasm generate   # wasi-libc + libc++/libc++abi を out/wasi-sysroot にビルド（初回のみ）
make -C sandbox-wasm run-cxx     # => Solid volume: 5
```

`run-cxx` は `run-raw-cxx-libcxx`（`--features "cxx libcxx"`）に張り替えてある。`cc`/`libc` の関係と同様、`cxx` が C++ ソース、`libcxx` が libc++ sysroot 供給を担当する。bridge は些末な `double add(double,double)` で、`volume() = add(2,3) = 5`。

#### libc++ のビルド（generate）

`out/wasi-sysroot`（target `wasm32-wasip1`）に wasi-libc と libc++ を同居させる。libc++ は LLVM monorepo の `runtimes` を入口に `libcxx;libcxxabi` をビルド。詰まった点:

- **`CMAKE_SYSTEM_NAME=WASI` は LLVM が知らない**（`HandleLLVMOptions` は Win32/UNIX/Generic のみ）→ `Generic` を使う。wasm のコード生成は `--target=wasm32-wasip1` が駆動する。
- **`runtimes` は実 Python3 必須**（`find_package(Python3 REQUIRED)`）→ Store スタブではダメ。`out/python` に公式 embeddable を落として PATH に通す。
- **`CMAKE_AR=llvm-ar`（相対名）は CWD 前置で誤絶対化**されアーカイブで `no such file or directory` → AR/RANLIB/NM/clang を絶対パスで渡す。
- threads/filesystem は OFF、localization と例外は ON（既定）。`__config_site` に `_LIBCPP_HAS_NO_THREADS` / `_LIBCPP_HAS_NO_FILESYSTEM` が入る。

> 補足: wasi-sdk の prebuilt（このファイル冒頭の表）を使えば libc++ ビルドを丸ごと省略できる。ただし配布版は eh/noeh など構成が固定なので、例外の扱いを自前で選びたいなら source ビルドのほうが融通が利く。

#### 実装中の発見（run-cxx を通すまで）

`wasm32-unknown-unknown`（Rust 側）のまま、C++ TU だけ libc++ ヘッダを食わせる方針（`cc-libc` と同じ）。`--target=wasm32-wasip1` にすると `__wasi__` が定義され libc++/libc が WASI bottom-half 経路（実 import を出す）に化けるので避ける。

1. **`cxx` crate は `cxx.cc`（C++ ランタイム）を自前の `cc::Build` でコンパイル**する。ここに libc++ ヘッダを届けるには `build.rs` の `.flag()` では不足で、`cc` crate が読む env `CXXFLAGS_wasm32_unknown_unknown` を **Makefile から export** する必要がある。
2. **MSYS/Git-bash の env パス変換**が、値中 2 つ目の `C:` をパスリスト区切りと誤認し `C;` に化けさせる → `MSYS2_ENV_CONV_EXCL` でその変数を変換対象外に。
3. **`__wasi__` 未定義だと libc++ の `__locale` が ctype マスク（`alpha`/`digit`…）を定義できない**（`#elif defined(__wasi__)` ブランチを外すため）。同じ musl ロケールを選ぶ **`-D_LIBCPP_HAS_MUSL_LIBC`** で回避。target は unknown-unknown のまま。
4. **`cxx::bridge` は `mod` 形式必須**（`cxx_build::bridge()` が「expected a module」）。`#[cxx::bridge] mod ffi { unsafe extern "C++" { ... } }` に直し、呼び出しを `ffi::add` に。
5. **`link-cplusplus`（cxx の依存）は wasm 既定で `-lstdc++`** を要求 → 我々は libc++ なので `CXXSTDLIB=c++`。

#### 例外の扱い（今後）

今は **例外オフ**で通している:

- bridge fn `add` は `f64` 返却 → cxx は `noexcept` トランポリンを生成し try/catch を出さない。
- `cxx.cc` の唯一の `throw` は `-DRUST_CXX_NO_EXCEPTIONS` で `abort()` に切り替わる。
- よって `-fno-exceptions -fno-rtti` でコンパイルでき、libc++ を `-fwasm-exceptions` で作り直す必要がない。正常系は決して throw しないので実害なし。

**OCCT を載せる段階では例外が要る**（`Standard_Failure` 等）。その時は wasi-sdk の `define_libcxx`（このファイル上部）が示すとおり、

- libc++/libc++abi/**libunwind** を `-fwasm-exceptions -mllvm -wasm-use-legacy-eh=true`（legacy EH, #233）で**作り直し**、
- cadrum/cxx 側も同じ `-fwasm-exceptions` でコンパイルし（ABI 一致）、
- 実行する node が wasm EH 対応（最近の Node は既定で可）、

が必要になる。wasi-sdk が eh/noeh の 2 系統を別ディレクトリに分けているのはこのため。`RUST_CXX_NO_EXCEPTIONS` / `-fno-exceptions` は外すことになる。

#### libc bottom-half（WASI import）の扱い

`wasm32-unknown-unknown` には WASI ランタイムが無い。ところが `cxx.cc` の `#include <iostream>` が生成する `std::ios_base::Init` 静的初期化子が stdio を参照し、リンク後の wasm に **`wasi_snapshot_preview1` への import（`fd_write`/`fd_seek`/`fd_close`）が残る**（グローバルコンストラクタなので `--gc-sections` で消えない）。node は ESM 解決でこれを package と誤認し `ERR_MODULE_NOT_FOUND` になる。

対処は **bottom-half import を no-op スタブで定義して消す**（`src/wasi_stub.c`）:

- 実 import シンボルは `__wasi_fd_write` ではなく **`__imported_wasi_snapshot_preview1_fd_*`**（`__wasi_*` は libc.a 側で定義済みの wrapper）。`llvm-nm libc.a` で確認した。
- スタブは静的アーカイブで、実 import シンボルは libc.a 処理時に初めて undefined になりリンク順で取りこぼすため、**`+whole-archive` で強制リンク**する。
- 正常系で stdout に書かないので no-op で実害なし。最終的に残る import は wasm-bindgen の glue のみ（`node -e "...WebAssembly.Module.imports..."` で確認可）。

将来 wasm 内で本当に stdout 等を使いたくなったら、スタブを外して node 側に WASI shim を渡す（`wasi_snapshot_preview1` を import object で供給）か、wasm-bindgen のターゲット設定を変える方向になる。

#### 懸念点 / 残課題

- **cmake が project 内の `out/cmake-3.31.12` でなく PATH 上の別 cmake を使っている**（PATH 行が `$(CMAKE)`=URL で壊れている）。動くが再現性のため要修正。
- stale な `build-libcxx` が「Device or resource busy」で `rm` 不可になることがある（AV/エクスプローラのハンドル）。`CMakeCache.txt` 削除で回避できる。
- `wasm32-unknown-unknown` の C++ オブジェクトと `wasm32-wasip1` ビルドの libc++/libc を混在リンクしている。今は純計算経路のみで成立しているが、使う libc++ 機能が増えると新たな bottom-half import が出る可能性がある（その都度スタブを足す or wasip1 + shim へ移行）。
- 次は `make -C sandbox-wasm run-cadrum`（本命）。OCCT は例外多用なので、上記「例外の扱い（今後）」の eh 版 libc++ ビルドが前提になる。