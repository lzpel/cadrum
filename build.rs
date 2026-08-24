use std::env;
use std::path::{Path, PathBuf};

/// OCCT release used by cadrum. Update this tag when bumping OCCT versions.
/// `release_name()` derives the GitHub Release tag, the prebuilt tarball, and the
/// cache directory name from this.
const OCCT_VERSION: &str = "V8_0_1";

/// Build revision for prebuilt tarballs. Update this when making non-OCCT-breaking changes that require cache invalidation (e.g. patch updates, build script changes, EH encoding changes, etc).
const BUILD_REVISION: &str = "rev1";

/// Release tag / tarball / cache-dir name (#203). Fields are separated by `-` and
/// characters within a field by `_`, so the name parses by splitting on `-` (the
/// target's hyphens are underscored too). `has_version` appends the cadrum crate
/// version for the per-crate FFI artifact.
///
/// - `release_name(None, false)`    → `occt-8_0_1_rev1`                              (GitHub Release タグ)
/// - `release_name(Some(t), false)` → `occt-8_0_1_rev1-wasm32_unknown_unknown`       (OCCT tarball / cache dir)
/// - `release_name(Some(t), true)`  → `occt-8_0_1_rev1-wasm32_unknown_unknown-cadrum-0_8_13` (FFI tarball)
fn release_name(target: Option<&str>) -> String {
	let occt = OCCT_VERSION.trim_start_matches(['V', 'v']);
	let mut name = format!("occt-{}_{}", occt, BUILD_REVISION);
	if let Some(target) = target {
		name.push('-');
		name.push_str(&target.replace('-', "_"));
	}
	name
}

fn main() {
	println!("cargo:rerun-if-env-changed=OCCT_ROOT");
	println!("cargo:rerun-if-env-changed=CADRUM_PREBUILT_URL");
	println!("cargo:rerun-if-env-changed=CADRUM_BUNDLE_RUNTIME");

	if env::var("DOCS_RS").is_ok() {
		return;
	}

	let target = env::var("TARGET").unwrap();

	// Get occt_root directory whether the binary exist or not. If not, download or compile it.
	let effective_root = env::var("OCCT_ROOT")
		.map(|r| {
			let p = PathBuf::from(r);
			if p.is_relative() {
				env::current_dir().unwrap().join(p)
			} else {
				p
			}
		})
		.unwrap_or(cargo_target_dir(&target).join(release_name(Some(&target))));

	let occt = resolve_occt(&effective_root, &target);
	let [occt_include, occt_lib]= [&occt[0], &occt[1]];

	// Prebuilt tarball 作成時のみ host toolchain runtime を OCCT lib dir に同梱 (#89 / #147 対策)。
	// gate を切らないと source user 全員のホストランタイムが静的取り込みされてしまう。
	// 現 policy: mingw のみ GCC runtime を同梱 (Windows には安定した system libstdc++ が無い)。
	// Linux GNU は manylinux base gcc で古いシンボルのみ参照するので消費者の system libstdc++ に
	// 動的リンクさせ非同梱 (#147)。Windows MSVC は MSVC ランタイム、Mac は別系統。
	#[cfg(feature = "source")]
	if env::var("CADRUM_BUNDLE_RUNTIME").is_ok() {
		if target.ends_with("windows-gnu") {
			bundle_runtime_libs(&occt_lib, &["libstdc++.a", "libgcc.a", "libgcc_eh.a"], true);
		} else if target.starts_with("wasm32") {
			// -fwasm-exceptions ビルドの OCCT/libc++ が要求する eh 版ランタイムを同梱し prebuilt を
			// 自己完結化する（実測: 4 つ同梱すれば RUSTFLAGS 無しでリンク・実行できる）。
			// c++abi/unwind/c は外部 -l が出ないので prefix=true で walkdir に明示リンクさせる。
			// libc++ は link-cplusplus の `-l c++` が実名を要求するので prefix=false で実名のまま置く。
			bundle_runtime_libs(&occt_lib, &["libc++abi.a", "libunwind.a", "libc.a"], true);
			bundle_runtime_libs(&occt_lib, &["libc++.a"], false);
		}
	}

	link_occt_libraries(&occt_include, &occt_lib, &target);
}

/// Derive the cargo target directory from `OUT_DIR`.
///
/// `OUT_DIR` layout:
///   `<target_dir>/<profile>/build/<pkg>-<hash>/out`            (no `--target`)
///   `<target_dir>/<triple>/<profile>/build/<pkg>-<hash>/out`   (with `--target`)
fn cargo_target_dir(target: &str) -> PathBuf {
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	let above_profile = out_dir.ancestors().nth(4).expect("unexpected OUT_DIR layout");
	if above_profile.file_name().map_or(false, |n| n == target) {
		above_profile.parent().unwrap().to_path_buf()
	} else {
		above_profile.to_path_buf()
	}
}

/// Resolve `[include_dir, lib_dir]` for OCCT.
///
///   1. Cache hit → use it
///   2. Cache miss + `source` → build from upstream sources
///   3. Cache miss otherwise → download prebuilt tarball
fn resolve_occt(effective_root: &Path, target: &str) -> Vec<PathBuf> {
	println!("cargo:rerun-if-changed={}", effective_root.display());

	match find_occt_whitelist(effective_root) {
		Some(dirs) => return dirs,
		None => {
			#[cfg(feature = "source")]
			{
				eprintln!("cargo:warning=OCCT cache miss at {} — building from source (this may take 10-30 minutes)", effective_root.display());
				return source::occt_from_source(effective_root).expect(&format!(
					"\nFailed to build OCCT from source for target `{}`.\n\
					 Check that a C/C++ toolchain and CMake are installed and on PATH,\n\
					 then re-run:\n\
					 \n    cargo build --features source\n",
					target
				));
			}
			#[cfg(not(feature = "source"))]
			{
				return occt_from_prebuilt(effective_root, target).expect(&format!(
					"\nFailed to download prebuilt OCCT for target `{}`.\n\
					 See README for the list of supported prebuilt targets, or enable\n\
					 the `source` feature to build OCCT from upstream sources:\n\
					 \n    cargo build --features source\n",
					target
				));
			}
		}
	}
}

fn find_occt_whitelist(occt_root: &Path) -> Option<Vec<PathBuf>>{
	let pick = |cands: &[PathBuf]| cands.iter().find(|p| p.exists()).cloned();
	let contains=|parent: &Path, prefx: &str| -> Option<PathBuf>{
		std::fs::read_dir(parent).ok()?.filter_map(Result::ok).find_map(|e| e.file_name().to_string_lossy().contains(prefx).then_some(e.path()))
    };
	Some([
		pick(&[
			occt_root.join("include").join("opencascade"),
			occt_root.join("inc"),
			occt_root.join("include")
		])?,
		pick(&[
			occt_root.join("lib"),
			occt_root.join("win64").join("gcc").join("lib"),
			occt_root.join("win64").join("clang").join("lib"),
			occt_root.join("win64").join("vc14").join("lib")
		])?,
		contains(&occt_root, "OCCT")?,
		contains(&occt_root, "LICENSE")?,
		contains(&occt_root, "EXCEPTION")?,
	].to_vec())
}

/// OCCT toolkits to link against (OCCT 7.8+ / 8.x naming).
const OCC_LIBS: &[&str] = &[
	"TKernel",
	"TKMath",
	"TKBRep",
	"TKTopAlgo",
	"TKPrim",
	"TKBO",
	"TKBool",
	"TKShHealing",
	"TKMesh",
	"TKGeomBase",
	"TKGeomAlgo",
	"TKG3d",
	"TKG2d",
	"TKBin",
	"TKXSBase",
	"TKDE",
	"TKDECascade",
	"TKOffset",
	"TKFillet",
	"TKDESTEP",
	#[cfg(feature = "color")]
	"TKLCAF",
	#[cfg(feature = "color")]
	"TKXCAF",
	#[cfg(feature = "color")]
	"TKCAF",
	#[cfg(feature = "color")]
	"TKCDF",
];

/// Apply target-conditional C++ compiler flags through `apply`, which forwards each flag
/// to the concrete builder (`cc::Build::flag` for the wrapper, `cmake::Config::cxxflag` for
/// the OCCT source build). Shared so the wrapper and OCCT get identical flags — in particular
/// the same wasm EH encoding, which must match the legacy-built wasi-sdk eh sysroot (#199, #233).
fn apply_compiler_flags(mut apply: impl FnMut(&str)) {
	// MSVC: compile sources as UTF-8.
	if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
		apply("/utf-8");
	}
	// wasm: force the legacy EH encoding, matching the legacy eh sysroot the cross image
	// self-builds (exnref needs a runtime opt-in; #233). No-op without -fwasm-exceptions.
	if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
		apply("-mllvm");
		apply("-wasm-use-legacy-eh=true");
	}
}

fn link_occt_libraries(occt_include: &Path, occt_lib_dir: &Path, target: &str) {
	println!("cargo:rustc-link-search=native={}", occt_lib_dir.display());
	for lib in OCC_LIBS {
		println!("cargo:rustc-link-lib=static={}", lib);
	}

	// Tarball 側が "cadrum" を含む名前の static library を同梱していれば拾う。
	// 典型: mingw 向けに libcadrum_stdc++.a / libcadrum_gcc.a 等を同梱して、
	// ホスト側 GCC のバージョン差による libstdc++ ABI ミスマッチを回避する (#89)。
	// OCC_LIBS ループの後に置くので OCCT libs の未解決 symbol を後段で満たす順序になる。
	for entry in walkdir::WalkDir::new(occt_lib_dir).min_depth(1).max_depth(1).into_iter().flatten() {
		let Some(name) = entry.file_name().to_str() else { continue };
		if name.contains("cadrum") {
			let name = name.strip_prefix("lib").unwrap_or(name).strip_suffix(".a").or(name.strip_suffix(".lib")).unwrap_or(name);
			println!("cargo:rustc-link-lib=static={}", name);
		}
	}

	let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
	let is_mingw_like = target_env == "gnu" || target_env == "gnullvm";
	if is_mingw_like {
		println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
	}

	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && is_mingw_like {
		println!("cargo:rustc-link-arg=-static");
	}

	let mut build = cxx_build::bridge("src/ffi.rs");
	build.file("src/ffi.cpp").include(occt_include).std("c++17").define("_USE_MATH_DEFINES", None);

	apply_compiler_flags(|s| {
		build.flag(s);
	});

	// Mirror every enabled cargo feature as a FEATURE_<NAME> define so C++
	for name in env::vars().filter_map(|kv| kv.0.strip_prefix("CARGO_FEATURE_").map(str::to_owned)) {
		build.define(&format!("FEATURE_{name}"), None);
	}
	build.compile(&release_name(Some(target)));
	println!("cargo:rerun-if-changed=src/ffi.rs");
	println!("cargo:rerun-if-changed=src/ffi.h");
	println!("cargo:rerun-if-changed=src/ffi.cpp");
}

/// Provide OCCT into `effective_root` by downloading a prebuilt tarball for `target`.
/// Paired with `occt_from_source` (selected by the `source` feature) in `resolve_occt`.
#[cfg(not(feature = "source"))]
fn occt_from_prebuilt(effective_root: &Path, target: &str) -> Option<Vec<PathBuf>> {
	let top_name = release_name(Some(target));
	let tarball_name = format!("{}.tar.gz", top_name);
	let url = env::var("CADRUM_PREBUILT_URL").unwrap_or_else(|_| format!("https://github.com/lzpel/cadrum/releases/download/{}/{}", release_name(None), tarball_name));

	eprintln!("cargo:warning=Downloading prebuilt OCCT from {}", url);

	let parent = effective_root.parent()?;
	std::fs::create_dir_all(parent).ok()?;

	if let Err(e) = download_and_extract_tar_gz(&url, parent) {
		eprintln!("cargo:warning=prebuilt fetch failed: {}", e);
		return None;
	}

	let extracted = parent.join(&top_name);
	if !extracted.is_dir() {
		eprintln!("cargo:warning=prebuilt tarball missing expected top-level dir `{}`", top_name);
		return None;
	}

	if extracted != *effective_root {
		let _ = std::fs::remove_dir_all(effective_root);
		if let Err(e) = std::fs::rename(&extracted, effective_root) {
			eprintln!("cargo:warning=failed to move extracted OCCT into {}: {}", effective_root.display(), e);
			return None;
		}
	}

	find_occt_whitelist(effective_root)
}

fn download_and_extract_tar_gz(url: &str, dest: &Path) -> Result<(), String> {
	let gz = libflate::gzip::Decoder::new(fetch(url)?).map_err(|e| format!("gzip decode failed: {e}"))?;
	tar::Archive::new(gz).unpack(dest).map_err(|e| format!("tar unpack failed: {e}"))
}

fn fetch(url: &str) -> Result<Box<dyn std::io::Read>, String> {
	match url.strip_prefix("file://") {
		Some(rest) => {
			let path: PathBuf = if rest.len() >= 3 && rest.starts_with('/') && rest.as_bytes()[2] == b':' { PathBuf::from(&rest[1..]) } else { PathBuf::from(rest) };
			Ok(Box::new(std::fs::File::open(&path).map_err(|e| format!("read {}: {}", path.display(), e))?))
		}
		None => Ok(Box::new(minreq::get(url).send_lazy().map_err(|e| e.to_string())?)),
	}
}

/// Bundle host toolchain runtime archives (`libs`) into the OCCT lib dir.
/// Triggered by `CADRUM_BUNDLE_RUNTIME` only — used by the prebuilt-tarball makefile
/// recipe so end users running source don't get their host runtime silently bundled.
/// See the call sites in `main`.
///
/// `prefix` controls the destination name:
/// - `true`  → copy as `libcadrum_*.a` so `link_occt_libraries`'s walkdir links it
///   explicitly (`-l cadrum_*`). This both avoids the linker auto-grabbing the host
///   runtime (#89) and is the *only* thing that links the archive in a clean
///   prebuilt-consumer build. Required for anything no external `-l` already requests
///   (GCC runtime, wasm libc++abi/unwind/c).
/// - `false` → copy under the real name so an externally-emitted `-l <name>` resolves it
///   via the occt lib-dir search path. Used for wasm `libc++.a`: cxx's link-cplusplus
///   unconditionally emits `-l c++`, which needs the real file name (renaming breaks it).
#[cfg(feature = "source")]
fn bundle_runtime_libs(occt_lib_dir: &Path, libs: &[&str], prefix: bool) {
	let compiler = cc::Build::new().get_compiler();
	// `to_command()` で target/sysroot 等のフラグごと probe する。wasm は --target/--sysroot 無しだと
	// -print-file-name が wasi-sysroot を解決できない（GNU は素の probe でも絶対パスが返る）。
	let probe = |lib: &str| -> PathBuf {
		let out = compiler.to_command().arg(format!("-print-file-name={}", lib)).output().expect("compiler probe failed");
		PathBuf::from(std::str::from_utf8(&out.stdout).unwrap().trim())
	};
	// wasm の wasi-sysroot は C++ ランタイムを lib/<triple>/{noeh,eh}/ に分け、-print-file-name は
	// noeh を返す（libunwind は解決すらできない）。OCCT は -fwasm-exceptions ビルドなので eh 版が要る。
	// triple 直下にあり確実に引ける libc.a から eh ディレクトリを導出し、各 lib は eh を優先する。
	// GNU 系は eh サブディレクトリが無いので probe 結果（従来通り）にフォールバックする。
	let libc = probe("libc.a");
	let eh_dir = libc.parent().map(|d| d.join("eh")).filter(|d| d.is_dir());
	for &lib in libs {
		let src = eh_dir.as_ref().map(|d| d.join(lib)).filter(|p| p.exists()).unwrap_or_else(|| probe(lib));
		// `-print-file-name=` は名前が見つからない時に lib 名そのものを返すので存在チェック必須
		if src.is_absolute() && src.exists() {
			let dst_name = if prefix { lib.replace("lib", "libcadrum_") } else { lib.to_string() };
			std::fs::copy(&src, occt_lib_dir.join(&dst_name)).unwrap();
		} else {
			eprintln!("cargo:warning=runtime lib not found: {}", lib);
		}
	}
}

// ---------------------------------------------------------------------------
// source: build OCCT from upstream sources.
// Dependencies on cmake and walkdir live here only.
// ---------------------------------------------------------------------------
#[cfg(feature = "source")]
mod source {
	use super::{download_and_extract_tar_gz, find_occt_whitelist, OCCT_VERSION, OCC_LIBS};
	use std::env;
	use std::path::{Path, PathBuf};

	/// Provide OCCT into `effective_root`: download source, patch, build with CMake, paired with `occt_from_prebuilt` (selected by the `source` feature) in `resolve_occt`.
	pub fn occt_from_source(effective_root: &Path) -> Option<Vec<PathBuf>> {
		if let Some(dirs) = find_occt_whitelist(effective_root) {
			return Some(dirs);
		}

		let occt_version = OCCT_VERSION;
		let occt_url = format!("https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/{}.tar.gz", occt_version);

		if !walkdir::WalkDir::new(effective_root).max_depth(2).into_iter().any(|e| e.ok().is_some_and(|e| e.file_name() == "OCCT_LGPL_EXCEPTION.txt")) {
			eprintln!("Downloading OCCT {} from {} ...", occt_version, occt_url);
			download_and_extract_tar_gz(&occt_url, effective_root).expect("Failed to download/extract OCCT source tarball");
			eprintln!("OCCT source extracted successfully.");
		}

		let source_dir = std::fs::read_dir(effective_root).expect("Failed to read effective_root directory").flatten().find(|e| e.file_name().to_string_lossy().starts_with("OCCT") && e.path().is_dir()).map(|e| e.path()).expect("OCCT source directory not found after extraction");
		let mut patches = patch_from_files(&source_dir, &[include_str!("patches/pr1.patch"), include_str!("patches/pr2.patch")]);
		walk_occt_sources(&source_dir, |path| {
			if let Some(patched) = patch_or_none(path) {
				patches.insert(path.strip_prefix(&source_dir).expect("walked path escapes source_dir").to_path_buf(), patched);
			}
		});
		for (path, contents) in &patches {
			std::fs::write(source_dir.join(path), contents).expect("patch write failed");
			eprintln!("Patched {}", path.display());
		}

		eprintln!("Building OCCT with CMake (this may take a while)...");

		let mut cfg = cmake::Config::new(&source_dir);
		cfg.profile("Release")
			.define("BUILD_LIBRARY_TYPE", "Static")
			.define("CMAKE_INSTALL_PREFIX", effective_root.to_str().unwrap())
			.define("USE_FREETYPE", "OFF")
			.define("USE_FREEIMAGE", "OFF")
			.define("USE_OPENVR", "OFF")
			.define("USE_FFMPEG", "OFF")
			.define("USE_TBB", "OFF")
			.define("USE_VTK", "OFF")
			.define("USE_RAPIDJSON", "OFF")
			.define("USE_DRACO", "OFF")
			.define("USE_TK", "OFF")
			.define("USE_TCL", "OFF")
			.define("USE_XLIB", "OFF")
			.define("USE_OPENGL", "OFF")
			.define("USE_GLES2", "OFF")
			.define("USE_EGL", "OFF")
			.define("USE_D3D", "OFF")
			.define("BUILD_MODULE_FoundationClasses", "ON")
			.define("BUILD_MODULE_ModelingData", "ON")
			.define("BUILD_MODULE_ModelingAlgorithms", "ON")
			.define("BUILD_MODULE_DataExchange", "ON")
			.define("BUILD_MODULE_Visualization", "OFF")
			.define("BUILD_MODULE_ApplicationFramework", "OFF")
			.define("BUILD_MODULE_Draw", "OFF")
			.define("BUILD_DOC_Overview", "OFF")
			.define("BUILD_DOC_RefMan", "OFF")
			.define("BUILD_YACCLEX", "OFF")
			.define("BUILD_RESOURCES", "OFF")
			.define("BUILD_SAMPLES_MFC", "OFF")
			.define("BUILD_SAMPLES_QT", "OFF")
			.define("BUILD_Inspector", "OFF")
			.define("BUILD_ENABLE_FPE_SIGNAL_HANDLER", "OFF")
			.define("CMAKE_RC_FLAGS_INIT", "-C 1252");

		// cmake クレートは cc-rs の CC_<target>/CXX_<target> を CMAKE_C/CXX_COMPILER へ転送しない。
		// クロスツールチェインを env で差せるよう、ここで橋渡しする（target 非依存）。汎用(CC/CXX)
		// → target 固有(CC_<target>) の順で後勝ち。env が無い target(native 等)は何もせず cmake の
		// 既定探索に任せる。generator は CMAKE_GENERATOR env、target/sysroot 等は CFLAGS_/CXXFLAGS_<target>。
		// AR は転送しない: cmake は CMAKE_AR を PATH 解決せず bare 名だとリンクが壊れる。
		// CMAKE_C_COMPILER を指定すれば cmake が compiler prefix から正しい ar を自動導出する
		// （AR_<target> は cc-rs が src/ffi.cpp の archive に使うので無駄にはならない）。
		let tgt = env::var("TARGET").unwrap_or_default().replace('-', "_");
		for (cmake_key, base) in [("CMAKE_C_COMPILER", "CC"), ("CMAKE_CXX_COMPILER", "CXX")] {
			for name in [base.to_string(), format!("{base}_{tgt}")] {
				if let Ok(v) = env::var(&name) {
					cfg.define(cmake_key, &v);
				}
			}
		}

		// Same target-conditional flags as the wrapper (MSVC `/utf-8`, wasm exnref EH) so
		// OCCT's EH encoding matches the wrapper and the exnref eh sysroot (#199).
		super::apply_compiler_flags(|s| {
			cfg.cxxflag(s);
		});

		let occt_whitelist:Vec<PathBuf>={
			let built=cfg.build();
			eprintln!("OCCT built at: {}", built.display());
			let wl = find_occt_whitelist(effective_root)?;
			eprintln!("Whitelist entries ({}):", wl.len());
			for (i, w) in wl.iter().enumerate() {
				eprintln!("  [{}] {}", i, w.display());
			}
			wl
		};

		fn prune_except(dir: &Path, whitelist: &[PathBuf]) -> std::io::Result<()> {
			for p in std::fs::read_dir(dir)?.filter_map(Result::ok).map(|v| v.path()) {
				// p が whitelist のどれかの下にあるか、または whitelist そのものであるか
				let is_whitelisted = whitelist.iter().any(|w| w == &p || p.starts_with(w));
				if is_whitelisted && p.is_dir() {
					prune_except(&p, whitelist)?;
				} else if !is_whitelisted {
					if p.is_dir() {
						std::fs::remove_dir_all(&p)?;
					} else {
						std::fs::remove_file(&p)?;
					}
				}
			}
			Ok(())
		}
		// dir 以下を再帰的に走査し、whitelist以外を削除
		prune_except(&effective_root, &occt_whitelist).unwrap_or_default();
		// lib/ 以下でリンクしないものを削除。whitelist に含む lib のみをホワイトリストとして残す。
		let lib_whitelist: Vec<PathBuf> = OCC_LIBS.iter()
			.filter_map(|lib| {
				std::fs::read_dir(&occt_whitelist[1]).ok()?
					.filter_map(Result::ok)
					.find_map(|e| {
						let p = e.path();
						p.file_stem().and_then(|s| s.to_str()).map(|s| s.ends_with(lib)).unwrap_or(false)
							.then_some(p)
					})
			})
			.collect();
		prune_except(&occt_whitelist[1], &lib_whitelist).unwrap_or_default();
		// LGPL 2.1 §2: keep only patched files; remove everything else.
		std::fs::remove_dir_all(&source_dir).expect("failed to clear OCCT source tree");
		for (path, contents) in &patches {
			let path = source_dir.join(path);
			std::fs::create_dir_all(path.parent().unwrap()).expect("failed to create patched source dir");
			std::fs::write(&path, contents).expect("patched source write failed");
		}
		Some(occt_whitelist)
	}

	/// unified diff 群を `source_dir` 下のソースに順に当て、(source_dir 相対パス, 適用後の内容)
	/// を返す。同じファイルを触る diff が続けば前の適用結果に重ねるので、上流でスタックした
	/// PR をそのまま並べられる。当たらなければ panic する: 黙って未適用の prebuilt を配ると
	/// 原因の切り分けができない。上流に取り込まれたら該当の一行を消すだけで撤退できる。
	fn patch_from_files(source_dir: &Path, diffs: &[&str]) -> std::collections::HashMap<PathBuf, String> {
		let mut patches = std::collections::HashMap::new();
		for chunk in diffs.iter().flat_map(|diff| diff.split("\ndiff --git ")) {
			let Some(head) = chunk.lines().find(|line| line.starts_with("+++ ")) else { continue };
			let target = head[4..].split('\t').next().unwrap_or_default().trim();
			let target = PathBuf::from(target.strip_prefix("b/").unwrap_or(target));
			let base = match patches.get(&target) {
				Some(patched) => String::clone(patched),
				None => std::fs::read_to_string(source_dir.join(&target)).unwrap_or_else(|e| panic!("patch target {} unreadable: {e}", target.display())),
			};
			let patched = diff_apply(&base, chunk).unwrap_or_else(|| panic!("patch does not apply to {}", target.display()));
			patches.insert(target, patched);
		}
		patches
	}

	/// unified diff を `content` に適用する。どれか一つでもハンクが当たらなければ `None`。
	/// ハンクの行番号は目安として扱い、位置がずれていれば近傍を探し直す(GNU patch 相当)。
	fn diff_apply(content: &str, diff: &str) -> Option<String> {
		let lines: Vec<&str> = content.lines().collect();
		let mut out: Vec<&str> = Vec::new();
		let mut cur = 0usize;
		let mut it = diff.lines().peekable();

		while let Some(header) = it.next() {
			let Some(rest) = header.strip_prefix("@@ -") else { continue };
			let hint: usize = rest.split([',', ' ']).next()?.parse().ok()?;

			let (mut old, mut new) = (Vec::new(), Vec::new());
			while let Some(body) = it.peek() {
				match body.as_bytes().first() {
					Some(b'-') => old.push(&body[1..]),
					Some(b'+') => new.push(&body[1..]),
					// 文脈行。git は末尾空白を落とすので長さ 0 の行も空の文脈行として扱う。
					Some(b' ') | None => {
						old.push(body.get(1..).unwrap_or(""));
						new.push(body.get(1..).unwrap_or(""));
					}
					Some(b'\\') => {} // "\ No newline at end of file"
					_ => break,
				}
				it.next();
			}

			// hint 位置から外側へ探す。直前のハンクの終端 `cur` より前には戻らない。
			let hint = hint.saturating_sub(1).max(cur);
			let find = |needle: &[&str]| {
				let last = lines.len().checked_sub(needle.len()).filter(|last| *last >= cur)?;
				let hint = hint.min(last);
				(0..=last - cur).flat_map(|d| [hint + d, hint.wrapping_sub(d)]).find(|&i| (cur..=last).contains(&i) && lines[i..i + needle.len()] == needle[..])
			};
			// `old` が無く `new` があるハンクは適用済み。読み飛ばして冪等にする(patch -N 相当)。
			let (at, taken) = match find(&old) {
				Some(at) => (at, old.len()),
				None => (find(&new)?, new.len()),
			};

			out.extend_from_slice(&lines[cur..at]);
			out.extend_from_slice(&new);
			cur = at + taken;
		}

		out.extend_from_slice(&lines[cur..]);
		let mut patched = out.join("\n");
		if content.ends_with('\n') {
			patched.push('\n');
		}
		Some(patched)
	}

	/// Walk the OCCT source tree.
	/// - `src/` and `adm/`: recurse and yield every **file**
	/// - other top-level directories: yield the **directory** itself
	/// - top-level files: skipped
	fn walk_occt_sources(source_dir: &Path, mut f: impl FnMut(&Path)) {
		for entry in walkdir::WalkDir::new(source_dir).min_depth(1).max_depth(1).into_iter().flatten() {
			match entry {
				entry if "src|adm".contains(&*entry.file_name().to_string_lossy()) => {
					for child in walkdir::WalkDir::new(entry.path()).into_iter().flatten() {
						if child.file_type().is_file() {
							f(child.path());
						}
					}
				}
				entry if entry.file_type().is_dir() => f(entry.path()),
				_ => {}
			}
		}
	}

	/// OS 依存(OSD)層など OS API を直接使う OCCT 実装ファイル。全 target で body-stub 化し
	/// （シグネチャは残しリンク用シンボルを維持）、STUB_DROP_HEADERS の #include を外す。
	/// cadrum の公開 I/O はストリームベースで OSD ファイル層を通らず、テストも std::fs で
	/// バイト列を読み書きするので、source にこのスタブを当てても cargo test で検証できる。
	/// 性能の要 OSD_ThreadPool / OSD_Parallel(_Threads/_TBB) / OSD_Thread は意図的に非対象。
	const OSD_POSIX_STUBS: &[&str] = &["OSD_File.cxx", "OSD_Directory.cxx", "OSD_DirectoryIterator.cxx", "OSD_FileIterator.cxx", "OSD_FileNode.cxx", "OSD_Path.cxx", "OSD_Protection.cxx", "OSD_Process.cxx", "OSD_Host.cxx", "OSD_Disk.cxx", "OSD_Environment.cxx", "OSD_signal.cxx", "OSD_Chronometer.cxx", "OSD_MemInfo.cxx", "OSD_SharedLibrary.cxx", "Message_PrinterSystemLog.cxx", "STEPConstruct_AP203Context.cxx"];

	/// 上記スタブから外す、環境により不在のヘッダ（wasm に無い等）。
	/// native では既存ヘッダを消すだけで無害（body-stub 済みなので参照されない）。
	const STUB_DROP_HEADERS: &[&str] = &["netdb.h", "sys/socket.h", "arpa/inet.h", "net/if.h", "ifaddrs.h", "pwd.h", "grp.h", "dlfcn.h", "sys/statvfs.h", "sys/mount.h", "syslog.h"];

	/// Return the patched content for a file if it needs patching, `None` otherwise.
	/// Pure function — does not write to disk. 全 target 共通（target 分岐なし）。
	fn patch_or_none(path: &Path) -> Option<String> {
		let name = path.file_name()?.to_str()?;

		match name {
			"XCAFDoc_VisMaterial.cxx" => Some(stub_content(path, true)),
			"XCAFPrs_Texture.cxx" => Some(stub_content(path, false)),

			"Standard_StackTrace.cxx" => {
				let stubbed = stub_content(path, true);
				Some(comment_out_include_in(&stubbed, "execinfo.h"))
			}

			// OSD/POSIX 依存ファイル: body-stub + 不在ヘッダ除去（全 target 共通）。
			n if OSD_POSIX_STUBS.contains(&n) => {
				let mut s = stub_content(path, true);
				for h in STUB_DROP_HEADERS {
					s = comment_out_include_in(&s, h);
				}
				Some(s)
			}

			// Windows 専用 OSD 実装。windows.h + SEH(__try/__except) を含み body-stub できない
			// ため空ファイル化（Windows 以外ではコンパイルされず無害）。
			"OSD_WNT.cxx" => Some(stub_content(path, false)),

			// OCC_CONVERT_SIGNALS(signal→例外変換) を全 target で無効化。OSD_signal スタブ化と整合。
			//
			// このアームは「dead」ではなく実際に生きている: occt_defs_flags.cmake は OCCT 8.0.1 に
			// 存在し `add_definitions(-DOCC_CONVERT_SIGNALS)` を含むため、ここで実際にコメントアウトされる。
			// #209 はこれを dead と判断して削除したが、検証が wasm のみで、windows-gnu (mingw) ビルドでは
			// 経路が生きていた。外すと OCCT が setjmp/longjmp ベースの Standard_ErrorHandler を使い、
			// mingw 新版が廃止した `_setjmp` への未定義参照が出てリンク不能になる (rev3 の windows-gnu 回帰)。
			"occt_defs_flags.cmake" => {
				let content = std::fs::read_to_string(path).ok()?;
				let needle = "add_definitions(-DOCC_CONVERT_SIGNALS)";
				let replacement = "# add_definitions(-DOCC_CONVERT_SIGNALS)  # patched out by cadrum build.rs";
				if content.contains(needle) {
					Some(content.replace(needle, replacement))
				} else if content.contains(replacement) {
					Some(content) // already patched — keep as-is
				} else {
					None
				}
			}

			_ => None,
		}
	}

	/// Generate stubbed content for a C++ source file without writing to disk.
	fn stub_content(path: &Path, keep_signatures: bool) -> String {
		let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs().to_string()).unwrap_or_else(|_| "unknown".to_string());
		let description = if keep_signatures { "method bodies stubbed" } else { "file emptied" };
		let header = format!("// Stubbed by cadrum build.rs at unix={unix}: {description}.\n");

		if keep_signatures {
			let content = std::fs::read_to_string(path).expect("Failed to read file for stubbing");
			header + &stub_all_top_level_bodies(&content)
		} else {
			header
		}
	}

	/// Comment out `#include <header>` in a string and return the result.
	fn comment_out_include_in(content: &str, header: &str) -> String {
		let needle = format!("#include <{}>", header);
		let replacement = format!("// {} (patched out by cadrum build.rs)", needle);
		content.replace(&needle, &replacement)
	}

	fn lex_normalize(content: &str) -> String {
		let bytes = content.as_bytes();
		let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
		let mut i = 0;
		let mut at_line_start = true;

		let push_blank = |out: &mut Vec<u8>, b: u8| {
			out.push(if b == b'\n' { b'\n' } else { b' ' });
		};

		while i < bytes.len() {
			let c = bytes[i];

			if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
				while i < bytes.len() && bytes[i] != b'\n' {
					out.push(b' ');
					i += 1;
				}
				continue;
			}
			if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
				out.push(b' ');
				out.push(b' ');
				i += 2;
				while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
					push_blank(&mut out, bytes[i]);
					i += 1;
				}
				if i + 1 < bytes.len() {
					out.push(b' ');
					out.push(b' ');
					i += 2;
				} else {
					while i < bytes.len() {
						push_blank(&mut out, bytes[i]);
						i += 1;
					}
				}
				continue;
			}
			if c == b'"' {
				out.push(b' ');
				i += 1;
				while i < bytes.len() && bytes[i] != b'"' {
					if bytes[i] == b'\\' && i + 1 < bytes.len() {
						out.push(b' ');
						push_blank(&mut out, bytes[i + 1]);
						i += 2;
					} else {
						push_blank(&mut out, bytes[i]);
						i += 1;
					}
				}
				if i < bytes.len() {
					out.push(b' ');
					i += 1;
				}
				continue;
			}
			if c == b'\'' {
				out.push(b' ');
				i += 1;
				while i < bytes.len() && bytes[i] != b'\'' {
					if bytes[i] == b'\\' && i + 1 < bytes.len() {
						out.push(b' ');
						out.push(b' ');
						i += 2;
					} else {
						out.push(b' ');
						i += 1;
					}
				}
				if i < bytes.len() {
					out.push(b' ');
					i += 1;
				}
				continue;
			}
			if at_line_start && c == b'#' {
				while i < bytes.len() {
					if bytes[i] == b'\n' {
						let mut k = i;
						while k > 0 && (bytes[k - 1] == b' ' || bytes[k - 1] == b'\t') {
							k -= 1;
						}
						let continued = k > 0 && bytes[k - 1] == b'\\';
						out.push(b'\n');
						i += 1;
						if !continued {
							break;
						}
					} else {
						out.push(b' ');
						i += 1;
					}
				}
				at_line_start = true;
				continue;
			}

			if c == b'\n' {
				at_line_start = true;
			} else if !c.is_ascii_whitespace() {
				at_line_start = false;
			}
			out.push(c);
			i += 1;
		}

		debug_assert_eq!(out.len(), bytes.len(), "lex_normalize must preserve byte length");
		String::from_utf8(out).expect("lex_normalize produced invalid utf-8")
	}

	/// `)` の直後（`rest`）が「引数リストを閉じた後の末尾」かどうかを判定する。
	/// 末尾修飾子（`const`/`volatile`/`noexcept`/`override`/`final`/`mutable`/`&`/`&&`、
	/// および `noexcept(...)`/`throw(...)` の例外指定）だけを読み飛ばしてシグネチャ終端に
	/// 達するか、`->`（末尾戻り値型）で始まれば true。返り値型中の `(`（`Handle(X)` 等）は
	/// `)` の後ろに関数名が続くので false になる。
	fn is_parameter_list_tail(rest: &str) -> bool {
		let mut s = rest.trim_start();
		loop {
			if s.is_empty() || s.starts_with("->") {
				return true;
			}
			// コンストラクタ初期化リスト（`) : Base(x), member(y)`）が続く `)` も引数リスト。
			// `::`（名前修飾）と区別するため単独 `:` のみを見る。これを引数リストと認めないと
			// 初期化リスト内の `(...)` を引数リストと誤認し、コンストラクタに `{ return {}; }` を
			// 付けて MSVC C2534/C2562 になる。
			if s.starts_with(':') && !s.starts_with("::") {
				return true;
			}
			// `noexcept(...)` / `throw(...)`: 括弧ごと読み飛ばす。
			let mut consumed = false;
			for kw in ["noexcept", "throw"] {
				if let Some(after) = s.strip_prefix(kw) {
					let after = after.trim_start();
					if let Some(inner) = after.strip_prefix('(') {
						let b = inner.as_bytes();
						let mut depth = 1usize;
						let mut k = 0;
						while k < b.len() && depth > 0 {
							match b[k] {
								b'(' => depth += 1,
								b')' => depth -= 1,
								_ => {}
							}
							k += 1;
						}
						s = inner[k..].trim_start();
						consumed = true;
						break;
					}
				}
			}
			if consumed {
				continue;
			}
			// 単独の末尾修飾子キーワード / 参照修飾子（長い `&&` を先に判定）。
			let mut stripped = false;
			for kw in ["const", "volatile", "noexcept", "override", "final", "mutable", "&&", "&"] {
				if let Some(after) = s.strip_prefix(kw) {
					let boundary = after.chars().next().map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_'));
					if boundary {
						s = after.trim_start();
						stripped = true;
						break;
					}
				}
			}
			if !stripped {
				return false;
			}
		}
	}

	fn stub_body_for_sig(sig: &str) -> &'static str {
		let sig_norm: String = {
			let mut s = sig.to_string();
			loop {
				let next = s.replace(" ::", "::").replace(":: ", "::");
				if next == s {
					break s;
				}
				s = next;
			}
		};

		// 引数リストの `(` を構造的に探す。返り値型に現れる `(`（`Handle(X)`・`decltype(...)`・
		// ALL_CAPS マクロ・`operator()` 等）を関数引数の `(` と誤認しないよう、対応する `)` の
		// 後続が「末尾修飾子だけ→終端／`->`」になっているものを引数リストと判定する。返り値型側の
		// `(` はその `)` の後ろに必ず関数名（修飾子でない識別子）が続くので除外される。マクロ名の
		// 列挙に依存しないためコンパイラ非依存。
		let paren_pos = {
			let bytes = sig_norm.as_bytes();
			let mut cursor = 0;
			loop {
				let Some(off) = sig_norm[cursor..].find('(') else {
					return "{}";
				};
				let pos = cursor + off;
				let mut depth = 1;
				let mut j = pos + 1;
				while j < bytes.len() && depth > 0 {
					match bytes[j] {
						b'(' => depth += 1,
						b')' => depth -= 1,
						_ => {}
					}
					j += 1;
				}
				if is_parameter_list_tail(&sig_norm[j..]) {
					break pos;
				}
				cursor = j;
			}
		};
		let head_full = sig_norm[..paren_pos].trim();
		// シグネチャは物理行をまたいで折り返されることがある（clang-format は長い
		// `ReturnType Class::Method` を `::` 直後や返り値型の後で改行する）。最終行だけを
		// 採ると返り値型が失われ、値を返す関数を void と誤判定して `{}` を出力し
		// MSVC C4716「must return a value」になる。全空白を 1 スペースに畳み、`::` 周りを
		// 詰めて 1 論理行にしてから処理する。
		let head_joined = head_full.split_whitespace().collect::<Vec<_>>().join(" ");
		let head_tight = {
			let mut s = head_joined;
			loop {
				let next = s.replace(" ::", "::").replace(":: ", "::");
				if next == s {
					break s;
				}
				s = next;
			}
		};
		let head = head_tight.as_str();
		if head.is_empty() {
			return "{}";
		}

		let hb = head.as_bytes();
		let mut start = hb.len();
		while start > 0 {
			let c = hb[start - 1];
			if c.is_ascii_alphanumeric() || c == b'_' || c == b':' || c == b'~' {
				start -= 1;
			} else {
				break;
			}
		}
		let name = &head[start..];
		let return_part = head[..start].trim();

		if name.contains('~') {
			return "{}";
		}
		let segs: Vec<&str> = name.split("::").collect();
		if segs.len() >= 2 && segs[segs.len() - 1] == segs[segs.len() - 2] {
			return "{}";
		}
		if return_part.is_empty() {
			return "{}";
		}

		let rb = return_part.as_bytes();
		let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
		let mut idx = 0;
		while let Some(off) = return_part[idx..].find("void") {
			let pos = idx + off;
			let end = pos + 4;
			let before_ok = pos == 0 || !is_ident(rb[pos - 1]);
			let after_ok = end >= rb.len() || !is_ident(rb[end]);
			if before_ok && after_ok {
				let mut j = end;
				while j < rb.len() && rb[j].is_ascii_whitespace() {
					j += 1;
				}
				if j >= rb.len() || (rb[j] != b'*' && rb[j] != b'&') {
					return "{}";
				}
			}
			idx = end;
		}

		"{ return {}; }"
	}

	fn stub_all_top_level_bodies(content: &str) -> String {
		let normalized = lex_normalize(content);
		let nb = normalized.as_bytes();
		let mut result = String::new();
		let mut depth = 0usize;
		let mut i = 0;
		let mut last_end = 0;

		while i < nb.len() {
			match nb[i] {
				b'{' if depth == 0 => {
					let brace_pos = i;
					let prefix_norm = &normalized[last_end..brace_pos];
					let sig = prefix_norm.rfind(|c| c == ';' || c == '}').map(|p| &prefix_norm[p + 1..]).unwrap_or(prefix_norm);

					let trimmed = sig.trim_end();
					let last_line = trimmed.rsplit('\n').next().unwrap_or(trimmed).trim();
					let is_function = {
						let mut t = last_line;
						loop {
							let prev_len = t.len();
							for kw in ["const", "override", "final", "noexcept", "mutable", "volatile", "= 0", "=0"] {
								if t.ends_with(kw) {
									t = t[..t.len() - kw.len()].trim_end();
									break;
								}
							}
							if t.len() == prev_len {
								break;
							}
						}
						t.ends_with(')')
					};
					let is_var_init = trimmed.ends_with('=') || !is_function;

					depth = 1;
					i += 1;
					while i < nb.len() && depth > 0 {
						match nb[i] {
							b'{' => depth += 1,
							b'}' => depth -= 1,
							_ => {}
						}
						i += 1;
					}

					if is_var_init {
						continue;
					}

					let stub_body = stub_body_for_sig(sig);
					result.push_str(&content[last_end..brace_pos]);
					result.push_str(stub_body);
					last_end = i;
					continue;
				}
				b'{' => depth += 1,
				b'}' => {
					if depth > 0 {
						depth -= 1;
					}
				}
				_ => {}
			}
			i += 1;
		}
		result.push_str(&content[last_end..]);
		result
	}
}
