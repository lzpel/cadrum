fn main() {
	// cc feature: ヘッダを含まない純 C をコンパイル（libc 不要のはず）。
	if std::env::var("CARGO_FEATURE_CC").is_ok() {
		let mut build = cc::Build::new();
		build.file("src/ffi.c");
		if std::env::var("CARGO_FEATURE_LIBC").is_ok() {
			// ターゲットは wasm32-unknown-unknown のまま。先に生成した wasi-libc の
			// sysroot があれば、ヘッダ(-I)と libc.a(-lc) を足す。これで
			// __has_include(<math.h>) が true になり ffi.c は sin 分岐を採る。
			// sin は top-half(純粋計算)なので unknown でもリンクでき、WASI import を出さない。
			let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
			let sysroot = format!("{manifest}/../out/wasi-sysroot");
			let inc = format!("{sysroot}/include/wasm32-wasip1");
			build.include(&inc);
			println!("cargo:rustc-link-search=native={sysroot}/lib/wasm32-wasip1");
			println!("cargo:rustc-link-lib=static=c");
		}
		build.compile("sandbox_cc");
		println!("cargo:rerun-if-changed=src/ffi.c");
		println!("cargo:rerun-if-changed=src/ffi.h");
	}
	// cxx feature: cxx bridge 経由で C++ をコンパイル。
	if std::env::var("CARGO_FEATURE_CXX").is_ok() {
		cxx_build::bridge("src/lib.rs")
			.file("src/cpp.cpp")
			.include("src")
			.std("c++17")
			.compile("sandbox_cxx");
		println!("cargo:rerun-if-changed=src/ffi.cpp");
		println!("cargo:rerun-if-changed=src/ffi.h");
	}
	println!("cargo:rerun-if-changed=src/lib.rs");
}
