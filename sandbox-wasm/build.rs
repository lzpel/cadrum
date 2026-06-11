fn main() {
	// cpp feature のときだけ C++ をコンパイルする。
	if std::env::var("CARGO_FEATURE_CPP").is_ok() {
		cxx_build::bridge("src/lib.rs")
			.file("src/cpp.cpp")
			.include("src")
			.std("c++17")
			.compile("sandbox_cpp");
		println!("cargo:rerun-if-changed=src/cpp.cpp");
		println!("cargo:rerun-if-changed=src/cpp.h");
	}
	println!("cargo:rerun-if-changed=src/lib.rs");
}
