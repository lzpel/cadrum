PATH_DOCS=out/markdown
generate: # prepare for deploy
	mkdir -p out
	find . -maxdepth 1 -name .gitignore | xargs -IX sed '/^#\s*EOF_DOCKERIGNORE.*/q' X > .dockerignore
test: # test all
	cargo test
big: # list top 20 largest blobs in git history (bytes, path) — includes deleted files; use to find repo-bloating commits
	git rev-list --objects --all | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' | awk '/^blob/ {size=$$3; $$1=$$2=$$3=""; sub(/^ +/, ""); printf "%12d  %s\n", size, $$0}' | sort -n
update: generate # regenerate codegen/README/markdown from examples, then build out/html
	cargo install --root out mdbook --version 0.4.50
	cargo fmt
	cargo run --example codegen -- src/traits.rs src/lib.rs
	cargo run --example markdown -- $(PATH_DOCS)/SUMMARY.md ./README.md
	./out/bin/mdbook build
publish-ready: # guard: HEAD must be on main and match remote main's tip
	git branch --show-current | grep -qx main # on main
	gh api repos/lzpel/cadrum/commits/main --jq .sha | grep -q $(shell git rev-parse --short HEAD) # latest
	cargo publish --dry-run # not-durty
publish: update publish-ready # publish to crates.io
	# build (and wasm-smoke-test) the cross-wasm32-unknown-unknown image published below
	$(MAKE) cross-wasm32-unknown-unknown GOAL=generate
	# publish the wasm cross-build toolchain image (already built above by the
	# cross-wasm32-unknown-unknown step) so users can run
	# `docker run ghcr.io/lzpel/cross-wasm32-unknown-unknown cargo build` without installing wasi-sdk.
	# Requires a one-time `docker login ghcr.io -u lzpel` beforehand (token cached).
	docker tag cross-wasm32-unknown-unknown ghcr.io/lzpel/cross-wasm32-unknown-unknown:latest
	docker push ghcr.io/lzpel/cross-wasm32-unknown-unknown:latest
	# publish the crate sources to crates.io
	cargo publish
occt: generate # output out/occt-<rev>-<target>.tar.gz from source natively
	cargo clean
	# CADRUM_BUNDLE_RUNTIME=1 は windows-gnu / wasm でのみランタイムを同梱する (#89)。native linux-gnu は base gcc ビルドで古いシンボルのみ参照し消費者の system libstdc++ に動的リンクするので no-op (#147)。
	# pipefail is required so tee's exit code does not mask a cargo build failure
	bash -c "set -o pipefail && CADRUM_BUNDLE_RUNTIME=1 cargo build --example 01_primitives --release --features source 2>&1 | tee out/log.txt"
	find $(or $(CARGO_TARGET_DIR),target) -maxdepth 1 -type d -name 'occt*' | xargs -IX sh -c 'tar -czf out/$$(basename X).tar.gz -C $$(dirname X) $$(basename X)'
cadrum: generate # output out/libocct-<rev>-<target>-cadrum-<version>.a (wrapper compiled against the RELEASED prebuilt OCCT)
	# cargo clean wipes any source-built OCCT cache (from `make occt`) so build.rs is
	# forced down the prebuilt path: it downloads the RELEASED prebuilt OCCT and compiles
	# the wrapper against exactly that. --release without --features source selects the
	# prebuilt path; if that target's prebuilt is not released yet the download fails and
	# so does this recipe -- never silently build/stage an archive against a missing/stale OCCT.
	cargo clean
	cargo build --example 01_primitives --release
	find $(or $(CARGO_TARGET_DIR),target) -name 'libocct-*-cadrum*.a' -exec cp {} out/ \;
sample-wasm: # write out the throwaway cdylib (CHECK_WASM_* at the end of this file) and build it for wasm. Must run inside the cross image: only it has the legacy-EH wasi-sysroot the prebuilt was built against. --target-dir points at cadrum's own target/ (overriding the container's CARGO_TARGET_DIR), which both puts the .wasm where the host can see it and lets build.rs find the extracted prebuilt OCCT at its default location -- no OCCT_ROOT needed.
	mkdir -p target/check-wasm/src
	printf '%s\n' "$$CHECK_WASM_CARGO_TOML" > target/check-wasm/Cargo.toml
	printf '%s\n' "$$CHECK_WASM_LIB_RS" > target/check-wasm/src/lib.rs
	cd target/check-wasm && cargo build --release --target-dir $(CURDIR)/target
cross-%: # run `make $(GOAL)` for target % inside its Docker cross env. GOAL is required. Source is bind-mounted at run time (image is a project-agnostic toolchain); CARGO_TARGET_DIR is redirected into the container (/tmp/target) so the build never touches/pollutes the host target/, and outputs still land in out/<target>/.
	@test -n "$(GOAL)" || { echo "GOAL is required: make cross-$* GOAL=occt|cadrum"; exit 1; }
	docker build -f docker/Dockerfile_$(*) -t cross-$(*) .
	docker run --rm -v $(PWD):/src -w /src -e CARGO_TARGET_DIR=/tmp/target -v $(PWD)/out/$(*):/src/out cross-$(*) make $(GOAL)
check-%: # validate the cross-built prebuilt OCCT runs on the host (extract -> run example / wasm check)
	$(MAKE) cross-$* GOAL=occt
	mkdir -p target
	find out -maxdepth 2 -type f -name '*.tar.gz' | xargs -IX tar -xzf X -C target
	# wasm: link inside the image (only it has the legacy-EH sysroot), run here. Passing no import
	# object is the point: a leftover WASI import fails instantiation. legacy EH needs no node flag.
	if [ "$*" = "wasm32-unknown-unknown" ]; then \
		$(MAKE) cross-$* GOAL=sample-wasm; \
		node -e "const fs=require('fs');WebAssembly.instantiate(fs.readFileSync('target/wasm32-unknown-unknown/release/check_wasm.wasm')).then(({instance})=>{instance.exports.start();const v=instance.exports.volume();console.log('Solid volume:',v);process.exit(v===6000?0:1)})"; \
	else \
		timeout 300 cargo run --example 01_primitives; \
	fi
# The throwaway cdylib `sample-wasm` writes out. cadrum needs no wasm-bindgen: its WASI stubs live
# in the rlib (src/wasi_stub.rs), so with __anchor_wasi_stub() pulling them in, the module has zero
# imports and node can instantiate it raw. Comments inside a `define` are kept verbatim, and the
# body reaches the recipe as an exported environment variable, so make never re-expands the
# `#[...]` attributes.
define CHECK_WASM_CARGO_TOML
[package]
name = "check-wasm"
version = "0.0.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
cadrum = { path = "../..", default-features = false, features = ["color"] }

# An empty [workspace] keeps this a standalone workspace root, without which cargo would ignore the
# [profile] below. The optimizations only reach the Rust side -- the OCCT/libc++ archives are
# prebuilt, so their size is untouched by any of this.
[workspace]

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
strip = true
endef
export CHECK_WASM_CARGO_TOML
define CHECK_WASM_LIB_RS
use cadrum::{DVec3, Solid};

// A cdylib links with --no-entry, so OCCT's C++ global constructors never run on their own.
#[unsafe(no_mangle)]
pub extern "C" fn start() {
	cadrum::__anchor_wasi_stub();
	unsafe extern "C" {
		fn __wasm_call_ctors();
	}
	unsafe { __wasm_call_ctors() };
}

#[unsafe(no_mangle)]
pub extern "C" fn volume() -> f64 {
	let solid = Solid::cube(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0)).color("#4a90d9");
	// Round-trip STEP through memory, never a file, so the OSD_File stub layer stays out of it.
	let mut bytes: Vec<u8> = Vec::new();
	Solid::write_step([&solid], &mut bytes).expect("write_step to memory failed");
	let mut cursor = std::io::Cursor::new(&bytes);
	let solids = Solid::read_step(&mut cursor).expect("read_step from memory failed");
	solids.first().expect("no solid after round-trip").volume()
}
endef
export CHECK_WASM_LIB_RS