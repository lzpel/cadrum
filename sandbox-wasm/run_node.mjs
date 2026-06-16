// CLI validation of the --target web path the browser (cadrum-wasm-example) uses.
// node cannot fetch file: URLs, so we hand the raw .wasm bytes to the wasm-bindgen init.
import { readFile } from "node:fs/promises";
import init, { print_volume } from "./target/wasm_experiment.js";

const wasmBytes = await readFile(new URL("./target/wasm_experiment_bg.wasm", import.meta.url));

// `cadrum::wasm_start!()` in the consumer emits a #[wasm_bindgen(start)] shim, which
// wasm-bindgen's init() calls — so OCCT's C++ global constructors run here automatically,
// with no manual __wasm_call_ctors() call from JS.
await init({ module_or_path: wasmBytes });

const v = print_volume();
console.log(v);
if (!/Solid volume:\s*[0-9]/.test(v)) process.exit(1);
