fn main() {
	// cc feature: ffi.c をコンパイル。wasi-sysroot のヘッダ(-isystem)と libc.a(-L/-lc)は
	// makefile の CFLAGS_wasm32_unknown_unknown / CARGO_TARGET_*_RUSTFLAGS から供給される。
	// これで __has_include(<math.h>) が true になり ffi.c は sin 分岐を採る。
	if std::env::var("CARGO_FEATURE_CC").is_ok() {
		cc::Build::new().file("src/ffi.c").compile("sandbox_cc");
		println!("cargo:rerun-if-changed=src/ffi.c");
		println!("cargo:rerun-if-changed=src/ffi.h");
	}
	// cxx feature: cxx bridge 経由で C++ をコンパイル。
	if std::env::var("CARGO_FEATURE_CXX").is_ok() {
		let mut build = cxx_build::bridge("src/lib.rs");
		build.file("src/ffi.cpp").include("src").std("c++17");
		// 例外/RTTI 関連フラグ(-fwasm-exceptions 等)は makefile の
		// CXXFLAGS_wasm32_unknown_unknown が cc-rs 経由で全 C++ TU(ffi.cpp/cxx.cc)へ届ける。
		// libcxx feature: 生成した wasi-sysroot の libc++/libc ヘッダ(-isystem)と
		// libc++/libc++abi/libc(.a) を足す。target は wasm32-unknown-unknown のまま
		// にして __wasi__ を定義させず、libc++ の WASI bottom-half 経路（実 import を
		// 出す）に化けるのを避ける（cc+libc と同じ方式）。
		if std::env::var("CARGO_FEATURE_LIBCXX").is_ok() {
			let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
			let sysroot = format!("{manifest}/../out/wasi-sdk-33/share/wasi-sysroot");
			// ヘッダ(-isystem)と __wasi__ rune table は makefile の CXXFLAGS_<target>
			// (--target=wasm32-wasip1 --sysroot=...) が供給する。ここはリンクのみ。
			// eh: -fwasm-exceptions ビルドに対応する libc++ / libc++abi。例外巻き戻しに
			// libunwind が要る。
			println!("cargo:rustc-link-search=native={sysroot}/lib/wasm32-wasip1/eh");
			println!("cargo:rustc-link-search=native={sysroot}/lib/wasm32-wasip1");
			println!("cargo:rustc-link-lib=static=c++");
			println!("cargo:rustc-link-lib=static=c++abi");
			println!("cargo:rustc-link-lib=static=unwind");
			println!("cargo:rustc-link-lib=static=c");
		}
		build.compile("sandbox_cxx");
		println!("cargo:rerun-if-changed=src/ffi.cpp");
		println!("cargo:rerun-if-changed=src/ffi.h");
	}
	// libcxx 単独実験は、libc++ の静的初期化(iostream 等)が引きずる wasi_snapshot_preview1
	// import を no-op スタブで潰す。正常系では実 I/O しない。スタブシンボルは libc.a 処理時に
	// 初めて undefined になるので whole-archive で確実に取り込む。
	// cadrum feature は cadrum 本体(cpp/wasi_stub.c)が同じスタブを自前で +whole-archive リンク
	// するので、ここでは焼かない（焼くとシンボル二重定義でリンクエラー）。
	let links_libcxx = std::env::var("CARGO_FEATURE_LIBCXX").is_ok();
	if links_libcxx {
		let out_dir = std::env::var("OUT_DIR").unwrap();
		cc::Build::new().file("src/wasi_stub.c").cargo_metadata(false).compile("wasi_stub");
		println!("cargo:rustc-link-search=native={out_dir}");
		println!("cargo:rustc-link-lib=static:+whole-archive=wasi_stub");
		println!("cargo:rerun-if-changed=src/wasi_stub.c");
	}
	println!("cargo:rerun-if-changed=src/lib.rs");
}
