PATH_DOCS=out/markdown
# Cargo's build dir. Defaults to `target`; the `cross-%` Docker recipe overrides it
# (via `docker run -e`) to an in-container path so a bind-mounted source tree's build
# stays isolated from the host `target/`. Recipes that locate build outputs use this var.
CARGO_TARGET_DIR ?= target
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
publish: publish-ready update # publish to crates.io
	# build the wasm prebuilt FFI wrapper .a (consumer-side download code is TBD)
	$(MAKE) cross-wasm32-unknown-unknown GOAL=cadrum
	# upload to the EXISTING OCCT release (tag = occt-<rev>, derived from the .a name); fails if the release is absent
	find out/wasm32-unknown-unknown -maxdepth 1 -name 'libocct-*-cadrum*.a' | while read -r f; do \
		tag=$$(basename "$$f" .a | sed 's/^lib//' | cut -d- -f1,2); \
		gh release upload "$$tag" "$$f" --clobber; \
	done
	# publish the crate sources to crates.io
	cargo publish
occt: generate # output out/occt-<rev>-<target>.tar.gz from source natively
	cargo clean
	# CADRUM_BUNDLE_RUNTIME=1 で OCCT lib dir に libstdc++.a / libgcc.a / libgcc_eh.a を libcadrum_* として同梱し、ホスト側 GCC との ABI 不整合を回避する (#89 / #147 対策)。
	# pipefail is required so tee's exit code does not mask a cargo build failure
	bash -c "set -o pipefail && CADRUM_BUNDLE_RUNTIME=1 cargo build --example 01_primitives --release --features source 2>&1 | tee out/log.txt"
	find $(CARGO_TARGET_DIR) -maxdepth 1 -type d -name 'occt*' | xargs -IX sh -c 'tar -czf out/$$(basename X).tar.gz -C $$(dirname X) $$(basename X)'
cadrum: generate # output out/libocct-<rev>-<target>-cadrum-<version>.a (wrapper compiled against the RELEASED prebuilt OCCT)
	# cargo clean wipes any source-built OCCT cache (from `make occt`) so build.rs is
	# forced down the prebuilt path: it downloads the RELEASED prebuilt OCCT and compiles
	# the wrapper against exactly that. --release without --features source selects the
	# prebuilt path; if that target's prebuilt is not released yet the download fails and
	# so does this recipe -- never silently build/stage an archive against a missing/stale OCCT.
	cargo clean
	cargo build --example 01_primitives --release
	find $(CARGO_TARGET_DIR) -name 'libocct-*-cadrum*.a' -exec cp {} out/ \;
cross-%: # run `make $(GOAL)` for target % inside its Docker cross env. GOAL is required. Source is bind-mounted at run time (image is a project-agnostic toolchain); CARGO_TARGET_DIR is redirected into the container (/tmp/target) so the build never touches/pollutes the host target/, and outputs still land in out/<target>/.
	@test -n "$(GOAL)" || { echo "GOAL is required: make cross-$* GOAL=occt|cadrum"; exit 1; }
	docker build -f docker/Dockerfile_$(*) -t cross-$(*) .
	docker run --rm -v $(PWD):/src -w /src -e CARGO_TARGET_DIR=/tmp/target -v $(PWD)/out/$(*):/src/out cross-$(*) make $(GOAL)
check-%: # validate the cross-built prebuilt OCCT runs on the host (extract -> run example / wasm check)
	$(MAKE) cross-$* GOAL=occt
	mkdir -p target
	find out -maxdepth 2 -type f -name '*.tar.gz' | xargs -IX tar -xzf X -C target
	if [ "$*" = "wasm32-unknown-unknown" ]; then \
		$(MAKE) -C sandbox-wasm check-cadrum; \
	else \
		timeout 300 cargo run --example 01_primitives; \
	fi