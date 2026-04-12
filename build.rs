mod build_delegation;

use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

fn main() {
	println!("cargo:rerun-if-env-changed=OCCT_ROOT");
	println!("cargo:rerun-if-changed=src/traits.rs");
	println!("cargo:rerun-if-changed=build_delegation.rs");

	if env::var("DOCS_RS").is_ok() {
		return;
	}

	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	build_delegation::build_delegation(include_str!("src/traits.rs"), &out_dir);

	let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

	let occt_root = env::var("OCCT_ROOT").map(PathBuf::from).unwrap_or_else(|_| manifest_dir.join("target").join("occt"));

	let lib_dir = find_occt_lib_dir(&occt_root);
	println!("cargo:rerun-if-changed={}", lib_dir.display());
	let (occt_include, occt_lib_dir) = if lib_dir.exists() {
		// Libraries found — link only, no rebuild
		(find_occt_include_dir(&occt_root), lib_dir)
	} else {
		// No libraries found — build OCCT from source (this may take 10-30 minutes)
		eprintln!("cargo:warning=OCCT not found at {}. Building from source — this may take 10-30 minutes.", occt_root.display());
		build_occt_from_source(&out_dir, &occt_root)
	};

	link_occt_libraries(&occt_include, &occt_lib_dir, cfg!(feature = "color"));
}

fn link_occt_libraries(occt_include: &Path, occt_lib_dir: &Path, color: bool) {
	// Required OCC toolkit libraries to link against (OCCT 7.8+ / 7.9.x naming).
	// In OCCT 7.8+: TKSTEP*/TKBinTools/TKShapeUpgrade were reorganized into
	// TKDESTEP/TKBin/TKShHealing respectively.
	let mut occ_libs = [
		"TKernel",
		"TKMath",
		"TKBRep",
		"TKTopAlgo",
		"TKPrim",
		"TKBO",
		"TKBool",
		"TKShHealing", // includes former TKShapeUpgrade
		"TKMesh",
		"TKGeomBase",
		"TKGeomAlgo",
		"TKG3d",
		"TKG2d",
		"TKBin", // was TKBinTools
		"TKXSBase",
		"TKDE",        // DE framework base (OCCT 7.8+)
		"TKDECascade", // DE cascade bridge (OCCT 7.8+)
		"TKOffset",    // BRepOffsetAPI_MakePipeShell (helix sweep)
		"TKDESTEP",    // was TKSTEP + TKSTEP209 + TKSTEPAttr + TKSTEPBase
		               // TKService is NOT linked here: it contains Image_AlienPixMap (WIC image I/O)
		               // which pulls in ole32/windowscodecs on Windows, but image I/O is unused in
		               // the base API.  TKService is added below only when "color" is enabled because
		               // TKXCAF references Graphic3d_* symbols that live in TKService.
	].to_vec();


	// XDE (XDE-based STEP with color) requires ApplicationFramework libs.
	// OCCT library layout (verified by nm):
	//   TKLCAF   — TDocStd_Document, TDocStd_Application (NewDocument / Close)
	//   TKXCAF   — XCAFApp_Application, XCAFDoc_ColorTool, XCAFDoc_ShapeTool,
	//              XCAFDoc_DocumentTool
	//   TKCAF    — TNaming_NamedShape, TNaming_Builder (needed by TKXCAF's XCAFDoc)
	//   TKCDF    — CDM_Document, CDM_Application (needed by TKLCAF's TDocStd_Document)
	//   TKDESTEP — STEPCAFControl_Reader / Writer (already in OCC_LIBS above)
	//   TKService — Graphic3d_* symbols referenced by TKXCAF (XCAFDoc_VisMaterial etc.)
	if color {
		occ_libs.extend(["TKLCAF", "TKXCAF", "TKCAF", "TKCDF"]);
	}

	// Link OCC libraries
	println!("cargo:rustc-link-search=native={}", occt_lib_dir.display());
	for lib in occ_libs {
		println!("cargo:rustc-link-lib=static={}", lib);
	}

	// Safety-net: suppress any residual duplicate-symbol errors when linking
	// against OCCT static libraries on MinGW.  The primary fix is the
	// OCC_CONVERT_SIGNALS define added below to the cxx_build step.
	// Guard to GNU only: -Wl,... is GCC/ld syntax and is invalid on MSVC link.exe.
	if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
		println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
	}

	// advapi32 / user32: no longer needed — patch_occt_sources() stubs the OSD
	// files (OSD_WNT, OSD_File, OSD_Protection, OSD_signal) that reference them.

	// Build cxx bridge + C++ wrapper
	let mut build = cxx_build::bridge("src/occt/ffi.rs");
	build.file("cpp/wrapper.cpp").include(occt_include).std("c++17").define("_USE_MATH_DEFINES", None);

	// --- Vendored occ_gordon (Apache-2.0, DLR / rainman110) ---
	// Provides a working Gordon surface implementation that OCCT's own
	// GeomFill_Gordon fails on for toroidal curve networks.
	let og_root = PathBuf::from("cpp/third_party/occ_gordon/src");
	let og_internal = og_root.join("internal");
	build.include(&og_root).include(&og_internal);
	// Inject the compat shim into every .cpp so legacy `Handle_<Type>`
	// typedefs are visible without modifying upstream sources.
	let compat_header = og_internal.join("occ_gordon_compat.h");
	let compat_str = compat_header.to_string_lossy().into_owned();
	build.flag("-include").flag(&compat_str);
	for f in [
		"occ_gordon/occ_gordon.cpp",
		"internal/BSplineAlgorithms.cpp",
		"internal/BSplineApproxInterp.cpp",
		"internal/CurveNetworkSorter.cpp",
		"internal/CurvesToSurface.cpp",
		"internal/Error.cpp",
		"internal/GordonSurfaceBuilder.cpp",
		"internal/InterpolateCurveNetwork.cpp",
		"internal/IntersectBSplines.cpp",
		"internal/PointsToBSplineInterpolation.cpp",
	] {
		build.file(og_root.join(f));
	}
	println!("cargo:rerun-if-changed=cpp/third_party/occ_gordon/src");

	// Define CADRUM_COLOR for C++ when the "color" feature is enabled.
	if color {
		build.define("CADRUM_COLOR", None);
	}

	// On MinGW (Windows GNU toolchain), GCC at -O0 emits inline C++ methods
	// (from Standard_ErrorHandler.hxx) as strong (non-COMDAT) symbols in wrapper.o.
	// TKernel.a unconditionally defines the same methods in Standard_ErrorHandler.cxx.obj.
	// This causes "multiple definition" link errors.
	//
	// Standard_ErrorHandler.hxx only generates the inline stubs when OCC_CONVERT_SIGNALS
	// is NOT defined (via `#if !defined(OCC_CONVERT_SIGNALS)`).  Defining it here
	// suppresses those stubs in wrapper.o so only TKernel.a's implementation is linked.
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
		build.define("OCC_CONVERT_SIGNALS", None);
	}

	build.compile("cadrum_cpp");

	println!("cargo:rerun-if-changed=src/occt/ffi.rs");
	println!("cargo:rerun-if-changed=cpp/wrapper.h");
	println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
}

/// Download OCCT 8.0.0-rc5 source, patch, and build with CMake into `install_prefix`.
fn build_occt_from_source(out_dir: &Path, install_prefix: &Path) -> (PathBuf, PathBuf) {
	let occt_version = "V8_0_0_rc5";
	let occt_url = format!("https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/{}.tar.gz", occt_version);

	let download_dir = out_dir.join("occt-source");

	// Use a sentinel file to track successful extraction.
	let extraction_sentinel = download_dir.join(".extraction_done");

	if !extraction_sentinel.exists() {
		std::fs::create_dir_all(&download_dir).unwrap();

		// Clean up any partial extraction from a previous failed attempt
		if let Ok(entries) = std::fs::read_dir(&download_dir) {
			for entry in entries.flatten() {
				let name = entry.file_name();
				if name.to_string_lossy().starts_with("OCCT") && entry.path().is_dir() {
					eprintln!("Removing partial OCCT extraction: {:?}", name);
					let _ = std::fs::remove_dir_all(entry.path());
				}
			}
		}

		eprintln!("Downloading OCCT {} ...", occt_version);

		// Download using ureq (pure Rust HTTP client)
		let response = ureq::get(&occt_url).call().expect("Failed to download OCCT source tarball");

		let mut body = Vec::new();
		response.into_body().into_reader().read_to_end(&mut body).expect("Failed to read OCCT download response body");

		eprintln!("Downloaded {} bytes. Extracting...", body.len());

		// Extract using libflate + tar (pure Rust)
		let gz_decoder = libflate::gzip::Decoder::new(&body[..]).expect("Failed to initialize gzip decoder");
		let mut archive = tar::Archive::new(gz_decoder);
		archive.unpack(&download_dir).expect("Failed to extract OCCT source tarball");

		// Write sentinel to mark successful extraction
		std::fs::write(&extraction_sentinel, "done").unwrap();
		eprintln!("OCCT source extracted successfully.");
	}

	// Auto-detect the extracted OCCT directory name
	// (GitHub archives name it OCCT-{tag}, e.g. OCCT-V8_0_0_rc5)
	let source_dir = std::fs::read_dir(&download_dir).expect("Failed to read occt-source directory").flatten().find(|e| e.file_name().to_string_lossy().starts_with("OCCT") && e.path().is_dir()).map(|e| e.path()).expect("OCCT source directory not found after extraction");

	// Patch OCCT sources to remove TKService (Visualization) dependencies.
	// XCAFDoc_VisMaterial.cxx and XCAFPrs_Texture.cxx reference Graphic3d_* symbols
	// that live in TKService, which we don't build (BUILD_MODULE_Visualization=OFF).
	// The non-visualization TDF_Attribute methods (GetID, Restore, Paste, …) are
	// kept intact; only FillMaterialAspect / FillAspect are emptied.
	patch_occt_sources(&source_dir);

	let occt_root = install_prefix;

	// Determine lib path (CMake on Windows/MinGW installs to win64/gcc/lib)
	let lib_dir = find_occt_lib_dir(&occt_root);

	// Build with CMake only if not already installed
	if !lib_dir.exists() {
		eprintln!("Building OCCT with CMake (this may take a while)...");

		let built = cmake::Config::new(&source_dir)
			.profile("Release")
			.define("BUILD_LIBRARY_TYPE", "Static")
			.define("CMAKE_INSTALL_PREFIX", occt_root.to_str().unwrap())
			// Disable optional dependencies we don't need
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
			// Only build the modules we need
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
			.build();

		eprintln!("OCCT built at: {}", built.display());
	}

	// Re-resolve lib dir after build (in case it was just created)
	let lib_dir = find_occt_lib_dir(&occt_root);
	let include_dir = find_occt_include_dir(&occt_root);

	(include_dir, lib_dir)
}

/// Find the OCCT include directory, checking common install layouts.
fn find_occt_include_dir(occt_root: &Path) -> PathBuf {
	let candidates = [occt_root.join("include").join("opencascade"), occt_root.join("inc"), occt_root.join("include")];
	for dir in &candidates {
		if dir.exists() {
			return dir.clone();
		}
	}
	// Default fallback
	occt_root.join("include")
}

/// Find the OCCT lib directory, checking common install layouts.
/// CMake on Windows/MinGW installs to win64/gcc/lib; on Linux to lib.
fn find_occt_lib_dir(occt_root: &Path) -> PathBuf {
	let candidates = [occt_root.join("lib"), occt_root.join("win64").join("gcc").join("lib"), occt_root.join("win64").join("vc14").join("lib")];
	for dir in &candidates {
		if dir.exists() {
			return dir.clone();
		}
	}
	// Default fallback
	occt_root.join("lib")
}

/// Patch OCCT source files to remove unwanted link dependencies:
///
/// 1. TKService (Visualization) — even with BUILD_MODULE_Visualization=OFF:
///    - XCAFDoc_VisMaterial.cxx: stub bodies that use Graphic3d_* types.
///    - XCAFPrs_Texture.cxx: empty entirely (inherits Graphic3d_Texture2D).
///
/// 2. advapi32 / user32 (Windows system libs) — TKernel's OSD package:
///    - OSD_WNT.cxx: empty entirely (static initialiser calls AllocateAndInitializeSid).
///    - OSD_File.cxx: stub bodies (OpenProcessToken, SetSecurityDescriptorDacl, etc.).
///    - OSD_Protection.cxx: stub bodies (EqualSid, LookupAccountNameW, etc.).
///    - OSD_signal.cxx: stub bodies (MessageBoxA / MessageBeep on MSVC).
///    - OSD_FileNode.cxx: stub bodies (SetFileSecurityW + OSD_WNT helpers).
///    - OSD_Process.cxx: stub bodies (OpenProcessToken, GetUserNameW, EqualSid).
fn patch_occt_sources(source_dir: &Path) {
	// OCCT 8.0.0 moved sources under src/<Module>/<Toolkit>/<Package>/ hierarchy.
	// Try the new layout first, fall back to the legacy flat layout (≤7.9.x).
	let vis_material = ["src/DataExchange/TKXCAF/XCAFDoc/XCAFDoc_VisMaterial.cxx", "src/XCAFDoc/XCAFDoc_VisMaterial.cxx"];
	let prs_texture  = ["src/DataExchange/TKXCAF/XCAFPrs/XCAFPrs_Texture.cxx",    "src/XCAFPrs/XCAFPrs_Texture.cxx"];

	let find = |candidates: &[&str]| candidates.iter().map(|p| source_dir.join(p)).find(|p| p.exists());

	// Stub method bodies only: keep #includes and signatures, empty the bodies.
	if let Some(path) = find(&vis_material) {
		stub_out_methods(&path, true);
	}
	// Empty the entire file: the initializer list references the base class, so
	// body stubs alone cannot cut the TKService dependency.
	if let Some(path) = find(&prs_texture) {
		stub_out_methods(&path, false);
	}

	// --- Eliminate advapi32 / user32 dependencies from TKernel's OSD package ---
	let osd = |name: &str| [
		format!("src/FoundationClasses/TKernel/OSD/{name}"),
		format!("src/OSD/{name}"),
	];
	let find_osd = |name: &str| {
		let candidates = osd(name);
		candidates.into_iter().map(|p| source_dir.join(p)).find(|p| p.exists())
	};

	// OSD_WNT.cxx: static initialiser calls AllocateAndInitializeSid (advapi32).
	// Module-internal only — no external interface needed.
	if let Some(path) = find_osd("OSD_WNT.cxx") {
		stub_out_methods(&path, false);
	}
	// OSD_File.cxx: OpenProcessToken, SetSecurityDescriptorDacl, etc. (advapi32).
	if let Some(path) = find_osd("OSD_File.cxx") {
		stub_out_methods(&path, true);
	}
	// OSD_Protection.cxx: EqualSid, LookupAccountNameW, etc. (advapi32).
	if let Some(path) = find_osd("OSD_Protection.cxx") {
		stub_out_methods(&path, true);
	}
	// OSD_signal.cxx: MessageBoxA / MessageBeep (user32) on MSVC.
	if let Some(path) = find_osd("OSD_signal.cxx") {
		stub_out_methods(&path, true);
	}
	// OSD_FileNode.cxx: SetFileSecurityW (advapi32) + OSD_WNT helpers.
	if let Some(path) = find_osd("OSD_FileNode.cxx") {
		stub_out_methods(&path, true);
	}
	// OSD_Process.cxx: OpenProcessToken, GetUserNameW, EqualSid (advapi32).
	if let Some(path) = find_osd("OSD_Process.cxx") {
		stub_out_methods(&path, true);
	}
}

/// Neutralize a C++ source file at `path`.
///
/// # Arguments
/// - `keep_signatures` — `true`: keep `#include`s and signatures, replace only the
///   top-level method bodies with empty stubs. Use when the signature types are still
///   needed by the compiler.
///   `false`: empty the entire file (header comment only). Use when the initializer
///   list references a base class and body stubs alone cannot cut the dependency.
///
/// Stub body rules:
/// - `void` return / constructor / destructor → `{}`
/// - anything else → `{ return {}; }` (value-initialize)
///
/// # Note
/// `keep_signatures: true` cannot be used on files that have top-level
/// `namespace {}` or `extern "C" {}` blocks. Intended for `.cxx` implementation files only.
fn stub_out_methods(path: &Path, keep_signatures: bool) {
	if !path.exists() {
		return;
	}

	// Build the header comment that records the stub operation.
	let timestamp = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| {
			let secs = d.as_secs();
			let (h, mi, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
			format!("{:02}:{:02}:{:02} UTC", h, mi, s)
		})
		.unwrap_or_else(|_| "unknown".to_string());
	let description = if keep_signatures { "all method bodies replaced with empty stubs" } else { "file emptied" };
	let header = format!(
		"// Stubbed by cadrum build.rs: {}.\n\
		 // Stubbed at: {}\n",
		description, timestamp
	);

	let patched = if keep_signatures {
		let content = std::fs::read_to_string(path).expect("Failed to read file for stubbing");
		// Replace all top-level brace blocks with empty stubs.
		header + &stub_all_top_level_bodies(&content)
	} else {
		// Empty the file (header comment only).
		header
	};

	std::fs::write(path, patched).expect("Failed to write stubbed file");
	eprintln!("Stubbed {}", path.file_name().unwrap().to_string_lossy());
}

/// Replace every top-level (brace depth 0) `{ … }` block in `content` with
/// `{}` or `{ return {}; }` and return the result.
///
/// Brace-initialised variables (`static int x{0};`, `std::atomic<T> y{...};`)
/// are preserved verbatim: a `{` preceded by `=`, or whose prefix line has no
/// `(` (not a function signature), is treated as a variable initialiser and
/// skipped rather than stubbed.
fn stub_all_top_level_bodies(content: &str) -> String {
	let bytes = content.as_bytes();
	let mut result = String::new();
	let mut depth = 0usize;
	let mut i = 0;
	let mut last_end = 0;

	while i < bytes.len() {
		match bytes[i] {
			b'{' if depth == 0 => {
				let prefix = &content[last_end..i];

				// Detect brace-initialised variables: if the non-whitespace
				// character immediately before '{' is '=' or the prefix since
				// the last statement terminator contains no '(' (i.e. it is
				// not a function/method signature), treat as variable init
				// and skip the block without stubbing.
				let sig = prefix.rfind(|c| c == ';' || c == '}').map(|p| &prefix[p + 1..]).unwrap_or(prefix);
				let trimmed = sig.trim_end();
				let is_var_init = trimmed.ends_with('=') || !sig.contains('(');

				if is_var_init {
					// Walk forward to find the matching '}' and preserve verbatim.
					depth = 1;
					i += 1;
					while i < bytes.len() && depth > 0 {
						match bytes[i] {
							b'{' => depth += 1,
							b'}' => depth -= 1,
							_ => {}
						}
						i += 1;
					}
					// Keep the original text (prefix + braced block).
					continue;
				}

				// Function/method body: stub it.
				// Always use "{}" — "{ return {}; }" would fail on constructors,
				// and the CMake build uses -w to suppress missing-return warnings.
				let stub_body = "{}";

				// Walk forward with brace counting to find the matching closing brace.
				depth = 1;
				i += 1;
				while i < bytes.len() && depth > 0 {
					match bytes[i] {
						b'{' => depth += 1,
						b'}' => depth -= 1,
						_ => {}
					}
					i += 1;
				}
				// i now points one past the closing '}'.
				result.push_str(prefix);
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

