### 0.8.17

- #268 Add solid_sweep tests for a closed periodic spine with an auxiliary guide
- #267 Reorganize tests under a solid_* prefix and drop the integration test files
- #263 Stream the OCCT download and make patch application idempotent
- #262 Vendor upstream-bound OCCT fixes as patch files
- #261 Bump OCCT to 8.0.1
- #260 Stop leaking STEPControl_Reader in read_step_stream
- #258 Build linux-gnu prebuilts with the manylinux base gcc, drop the runtime bundle
- #256 Remove the sandbox-wasm experiment directory
- #255 Inline the streambuf bridges into ffi.cpp and slim ffi.h
- #254 Define FEATURE_<NAME> for every enabled cargo feature

### 0.8.16

- #247 Read and write STEP's solid-level colour keyed by the solid's id, drop BRep text I/O, rename read_brep_binary/write_brep_binary to read_brep/write_brep, and rework the BRep colour trailer
- #244 Emit NORMAL in glTF and drop KHR_materials_unlit
- #243 Give Mesh per-vertex surface normals, drop the unused uvs
- #233 Roll back the wasm exception model from exnref to the legacy encoding
- #203 Adopt the occt-<version>_<rev>-<target> prebuilt artifact naming scheme
- #199 Make the wasm prebuilt self-contained, dropping the wasi-sdk dependency
- #182 Rename the source-build feature to source
- #147 Build Linux prebuilts with the manylinux base gcc so they link on older-GCC distros without a bundled C++ runtime
- OCCT bumped to 8.0.1

### 0.8.10

- #193 Slim the prebuilt OCCT to just the include and lib that cadrum links
- #187 Build OCCT for wasm32-unknown-unknown so models can be built in the browser
- Aggregates changes since 0.8.0

### 0.8.0

- Restrict the boolean API to Solid::boolean_union/boolean_subtract/boolean_intersect, removing Solid, Vec<Solid> and Compound union/subtract/intersect (see notes/20260514-boolean演算は単体x単体のみ公開する方針.md)
- Add Add/Sub/Mul operators on &Solid and Sum/Product folding on Result<Solid, Error>, with new Error::OneFailed(usize)
- OCCT bumped to 8.0.0 final; no source changes required
- Rename tests/subtract.rs and tests/union.rs to tests/boolean_subtract.rs and tests/boolean_union.rs
- Fix supertrait extraction in examples/codegen.rs misreading where clauses with HRTBs

### 0.7.6

- Reduce src/lib.rs to #![doc = include_str!("../README.md")], making the README the single crate-root doc
- Emit rust,no_run fences in examples/markdown.rs so README examples are not slow doctests
- Center the README top section and add a docs.rs build-status badge
- Add CODE_OF_CONDUCT.md and CONTRIBUTING.md at the repo root
- Extract CHANGELOG.md from the README's Release Notes section
- Normalize examples/codegen.rs region indent to tabs by brace depth
- Documentation-only release with no public API changes

### 0.7.5

- #147 Bundle libstdc++.a, libgcc.a and libgcc_eh.a into the Linux prebuilt so linked binaries no longer depend on the host libstdc++
- #145 Move the I/O methods onto Solid: write_step, write_brep_binary, write_brep_text, read_step, read_brep
- #143 Add Edge::id and Face::iter_edge for face-edge incidence
- #142 Add Face::project and rename tshape_id to Edge/Face/Solid::id
- #120 Fix the C¹-discontinuous U=0 seam in periodic Solid::bspline
- OCCT bumped to 8.0.0-beta1
- Drop the relabeled x86_64-pc-windows-gnullvm prebuilt
- Aggregated changes since 0.7.2

### 0.7.2

- #130 Drop the *_with_metadata boolean variants in favor of Solid::iter_history
- #129 Recover SolveSpace-style multi-color STEP files that read as zero solids by sewing duplicated edges
- #127 Add up_dir parameter to Mesh::write_svg and Mesh::to_svg
- #125 Update the top README image to the alphastell stellarator render
- #111 Fix the docs.rs build by generating trait delegation before the DOCS_RS early-return
- #107 Drop the unsupported x86_64-pc-windows-msvc target from docs.rs
- #97 Silence OCCT's Statistics on Transfer stdout output on STEP read and write
- #94 #95 Re-export glam types from the crate root so downstream code needs no glam dependency
- #91 Hide the Transform trait behind Compound and Wire forwarders
- #89 Bundle libstdc++.a and libgcc.a into the mingw prebuilt so windows-gnu executables need no MinGW runtime DLLs
- Add Solid::shell(thickness, open_faces) via BRepOffsetAPI_MakeThickSolid
- Add Solid::fillet_edges and Solid::chamfer_edges for uniform fillet and chamfer on selected edges
- Add Solid::area, Solid::center and Solid::inertia, replacing shell_count
- Add Wire::project closest-point and tangent query on edges
- Add Edge::end_point and Edge::end_tangent
- Add Solid::iter_edge and Solid::iter_face backed by OnceLock caches
- Add Solid::history and Solid::iter_history face-derivation pairs from boolean ops and clean()
- Add example 08_shell.rs and renumber 08_bspline.rs to 09_bspline.rs
- Aggregated changes since 0.6.0

### 0.6.0

- Gate cmake and walkdir behind the source-build feature as optional build-dependencies
- Add the x86_64-pc-windows-gnu prebuilt via Docker cross-compilation with Debian mingw-w64, statically absorbing the MinGW runtime DLLs
- Retain only the ~9 patched OCCT source files alongside the .a libraries for LGPL 2.1 §2 compliance
- Resolve relative OCCT_ROOT via env::current_dir so the --target flag works
- Restructure build.rs around resolve_occt with cfg-separated source-build code
- Move the README build section after usage with a prebuilt target table

### 0.5.1

- Re-release of 0.4.5, whose version number sorted below the already-published 0.5.0; prefer 0.5.1 over 0.4.5
- Add Solid::bspline(grid, periodic) building a periodic B-spline solid from a 2D control-point grid
- Add a shading flag to write_svg and Mesh::to_svg for opt-in Lambertian shading
- Rewrite examples/08_bspline.rs as a 2 field-period stellarator-like torus
- Add tests/bspline.rs verifying 180° point symmetry via half-space intersections
- Add Error::BsplineFailed(String)
- Resolve OCCT 8.0.0 deprecation warnings in make_bspline_edge and make_bspline_solid
