fn main() {
	// cc feature: ヘッダを含まない純 C をコンパイル（libc 不要のはず）。
	if std::env::var("CARGO_FEATURE_CC").is_ok() {
		cc::Build::new().file("src/ffi.c").compile("sandbox_cc");
		println!("cargo:rerun-if-changed=src/ffi.c");
		println!("cargo:rerun-if-changed=src/cpp.h");
	}
	// cxx feature: cxx bridge 経由で C++ をコンパイル。
	if std::env::var("CARGO_FEATURE_CXX").is_ok() {
		cxx_build::bridge("src/lib.rs")
			.file("src/cpp.cpp")
			.include("src")
			.std("c++17")
			.compile("sandbox_cxx");
		println!("cargo:rerun-if-changed=src/cpp.cpp");
		println!("cargo:rerun-if-changed=src/cpp.h");
	}
	println!("cargo:rerun-if-changed=src/lib.rs");
}
