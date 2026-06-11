#include "ffi.h"

// math.h が利用可能なら sin(a+b)、無ければ a+b を返す。
// __has_include は clang/gcc のプリプロセッサ演算子で、ヘッダが include
// 可能か（= sysroot/libc が供給されているか）をコンパイル時に判定する。
// bare wasm32-unknown-unknown では <math.h> が無いので a+b、
// wasi-libc 等を供給すると sin(a+b) に切り替わる。
#if defined(__has_include) && __has_include(<math.h>)
#include <math.h>
double add(double a, double b) { return sin(a + b); }
#else
double add(double a, double b) { return a + b; }
#endif
