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

## Experiment 0: ok

make -C sandbox-wasm run-pure

## Experiment 1

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