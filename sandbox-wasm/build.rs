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
			let sysroot = format!("{manifest}/../out/wasi-sdk-33/share/wasi-sysroot");
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
		let mut build = cxx_build::bridge("src/lib.rs");
		build
			.file("src/ffi.cpp")
			.include("src")
			.std("c++17")
			// add は f64 を返すので生成トランポリンは noexcept、cxx.cc の throw も
			// RUST_CXX_NO_EXCEPTIONS で abort() に切り替わる。正常系は throw しないので
			// 例外/RTTI を無効化して libc++ を -fwasm-exceptions 無しのまま使う。
			.flag_if_supported("-fno-exceptions")
			.flag_if_supported("-fno-rtti")
			.define("RUST_CXX_NO_EXCEPTIONS", None);
		// libcxx feature: 生成した wasi-sysroot の libc++/libc ヘッダ(-isystem)と
		// libc++/libc++abi/libc(.a) を足す。target は wasm32-unknown-unknown のまま
		// にして __wasi__ を定義させず、libc++ の WASI bottom-half 経路（実 import を
		// 出す）に化けるのを避ける（cc+libc と同じ方式）。
		if std::env::var("CARGO_FEATURE_LIBCXX").is_ok() {
			let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
			let sysroot = format!("{manifest}/../out/wasi-sdk-33/share/wasi-sysroot");
			build.flag(format!("-isystem{sysroot}/include/wasm32-wasip1/noeh/c++/v1"));
			build.flag(format!("-isystem{sysroot}/include/wasm32-wasip1"));
			// target は unknown-unknown のままなので __wasi__ が未定義。wasi-sdk-33 の
			// libc++ は __locale の rune table を __wasi__ 等で選ぶが該当分岐が無く
			// ctype マスクが未定義になる。libc++ 自前のデフォルト rune table を選ぶ
			// _LIBCPP_PROVIDES_DEFAULT_RUNE_TABLE を定義して回避する。
			build.define("_LIBCPP_PROVIDES_DEFAULT_RUNE_TABLE", None);
			// noeh: -fno-exceptions ビルドに対応する libc++ / libc++abi バリアント。
			println!("cargo:rustc-link-search=native={sysroot}/lib/wasm32-wasip1/noeh");
			println!("cargo:rustc-link-search=native={sysroot}/lib/wasm32-wasip1");
			println!("cargo:rustc-link-lib=static=c++");
			println!("cargo:rustc-link-lib=static=c++abi");
			println!("cargo:rustc-link-lib=static=c");
			// <iostream> の静的初期化子が引きずる wasi_snapshot_preview1 import を
			// no-op スタブで定義して消す（正常系では stdout に書かない）。スタブの実
			// import シンボルは libc.a 処理時に初めて undefined になるので、リンク順に
			// 依らず確実に取り込むため whole-archive で強制リンクする。
			let out_dir = std::env::var("OUT_DIR").unwrap();
			cc::Build::new()
				.file("src/wasi_stub.c")
				.cargo_metadata(false)
				.compile("wasi_stub");
			println!("cargo:rustc-link-search=native={out_dir}");
			println!("cargo:rustc-link-lib=static:+whole-archive=wasi_stub");
			println!("cargo:rerun-if-changed=src/wasi_stub.c");
		}
		build.compile("sandbox_cxx");
		println!("cargo:rerun-if-changed=src/ffi.cpp");
		println!("cargo:rerun-if-changed=src/ffi.h");
	}
	println!("cargo:rerun-if-changed=src/lib.rs");
}
