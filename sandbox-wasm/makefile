# pure rust. I got wasm successfully.
run: run-raw-pure
# with simple cpp. such as "double add(double a, double b) { return a + b; }".
run-cpp: run-raw-cpp
# with cadrum. This is that target of this sandbox.
run-cadrum: run-raw-cadrum
run-raw-%: generate
	cargo build --target wasm32-unknown-unknown --features $*
	../out/bin/wasm-pack build . -d ../out --features $*
generate:
	cargo install --root ../out wasm-pack
	