PATH_DOCS=out/markdown
generate: # prepare for deploy
	mkdir -p out
	find . -maxdepth 1 -name .gitignore | xargs -IX sed '/^#\s*EOF_DOCKERIGNORE.*/q' X > .dockerignore
test: # test all
	cargo test
big: # list top 20 largest blobs in git history (bytes, path) — includes deleted files; use to find repo-bloating commits
	git rev-list --objects --all | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' | awk '/^blob/ {size=$$3; $$1=$$2=$$3=""; sub(/^ +/, ""); printf "%12d  %s\n", size, $$0}' | sort -n
deploy: generate # generate out/markdown from examples, then build out/html
	cargo install --root out mdbook --version 0.4.50
	cargo run --example codegen -- src/traits.rs src/lib.rs
	cargo run --example markdown -- $(PATH_DOCS)/SUMMARY.md ./README.md
	./out/bin/mdbook build
publish: deploy # publish to crates.io
	cargo publish
cadrum-occt: generate # build occt from source natively
	cargo clean
	# CADRUM_BUNDLE_RUNTIME=1 で OCCT lib dir に C++ runtime を libcadrum_* として同梱する:
	# native GNU は libstdc++/libgcc/libgcc_eh (ホスト GCC ABI 不整合回避 #89 / #147)、
	# wasm32 は wasi-sysroot eh の libc++/libc++abi/libunwind/libc (consumer が rustc だけで通る #207)。
	# pipefail is required so tee's exit code does not mask a cargo build failure
	bash -c "set -o pipefail && CADRUM_BUNDLE_RUNTIME=1 cargo build --example 01_primitives --release --features source 2>&1 | tee out/log.txt"
	find target -maxdepth 1 -type d -name 'occt*' | xargs -IX sh -c 'tar -czf out/$$(basename X).tar.gz -C $$(dirname X) $$(basename X)'
cadrum-ffi: generate # build the prebuilt cxx wrapper archive libcadrum_cpp.a for $(CARGO_BUILD_TARGET) and tar it (#207)
	cargo clean
	# Builds cadrum (compiles cpp/wrapper.cpp + cxx glue -> libcadrum_cpp.a). --features source
	# builds OCCT too so the wrapper matches the prebuilt's headers/flags/cxx/color.
	bash -c "set -o pipefail && cargo build --example 01_primitives --release --features source 2>&1 | tee out/log.txt"
	# Names must match build.rs release_name(Some(target),true) / (None,true): occt-8_0_0_rev2 is
	# tied to OCCT_VERSION/BUILD_REVISION in build.rs; the crate version comes from Cargo.toml.
	bash -c 'set -e; \
		VER=$$(grep -m1 "^version" Cargo.toml | sed -E "s/.*\"(.*)\".*/\1/" | tr . _); \
		TOP="occt-8_0_0_rev2-$(subst -,_,$(CARGO_BUILD_TARGET))-cadrum-$$VER"; \
		A=$$(find target/$(CARGO_BUILD_TARGET)/release/build/cadrum-*/out -name libcadrum_cpp.a | head -1); \
		test -n "$$A" || { echo "libcadrum_cpp.a not found under target/$(CARGO_BUILD_TARGET)"; exit 1; }; \
		mkdir -p out/$$TOP/lib; cp "$$A" out/$$TOP/lib/libcadrum_cpp.a; \
		tar -czf out/$$TOP.tar.gz -C out $$TOP'
cadrum-ffi-%: # build the FFI wrapper archive in the cross docker image (e.g. cadrum-ffi-wasm32-unknown-unknown)
	docker build -f docker/Dockerfile_$(*) -t cadrum-occt-$(*) .
	docker run --rm -v $(PWD)/out/$(*):/src/out cadrum-occt-$(*) make cadrum-ffi
cadrum-occt-%: # build occt from source in cross ( = native build in container ) cadrum-occt-aarch64-unknown-linux-gnu cadrum-occt-x86_64-pc-windows-gnu cadrum-occt-x86_64-unknown-linux-gnu
	docker build -f docker/Dockerfile_$(*) -t cadrum-occt-$(*) .
	docker run --rm -v $(PWD)/out/$(*):/src/out cadrum-occt-$(*) make cadrum-occt
cadrum-occt-%-check: # validate the built occt: link+run a host binary, or for wasm build+run sandbox-wasm under node
	$(MAKE) cadrum-occt-$*
	mkdir -p target
	find out -maxdepth 2 -type f -name '*.tar.gz' | xargs -IX tar -xzf X -C target
	if [ "$*" = "wasm32-unknown-unknown" ]; then \
		$(MAKE) -C sandbox-wasm check-cadrum; \
	else \
		timeout 300 cargo run --example 01_primitives; \
	fi