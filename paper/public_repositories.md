# Public repositories that depend on cadrum

This list is the evidence for the statement in `paper.md` (section "Use outside the author's projects") on how many public repositories by other developers depend on cadrum. It extends the table posted in the JOSS pre-review, [openjournals/joss-reviews#11355](https://github.com/openjournals/joss-reviews/issues/11355#issuecomment-5902443003), which was checked on 2026-09-30.

## Method

Every repository below was checked on 2026-10-21 by reading the `cadrum` line of the Cargo manifest named in the table. Candidates came from the 2026-09-30 table, from a GitHub code search for `cadrum filename:Cargo.toml`, and from repositories reported by hand, which the code search may not have indexed yet. Repositories owned by the author (lzpel) are left out, and so are repositories that only mention the name without depending on the crate.

## Repositories (17)

| Repository | Use | Manifest | cadrum requirement |
|---|---|---|---|
| [GHawk1124/Valurile](https://github.com/GHawk1124/Valurile) | Lattice Boltzmann flow solver (STEP input, per-face boundary conditions) | `valurile-core/Cargo.toml` | `0.8.13` |
| [saurlax/oxiprep](https://github.com/saurlax/oxiprep) | CAE preprocessor (geometry and meshing) | `Cargo.toml` | `0.8.16` |
| [sabatesduran/volum](https://github.com/sabatesduran/volum) | Desktop 3D model library | `src-tauri/Cargo.toml` | `0.8.20` |
| [verylowfreq/stepconverter](https://github.com/verylowfreq/stepconverter) | STEP to STL converter | `Cargo.toml` | `0.8.13` |
| [shadowbrok3r/RingDesigner](https://github.com/shadowbrok3r/RingDesigner) | Ring design | `crates/ringdesign-occt/Cargo.toml` | `=0.8.20` |
| [shtttt6/voice-ring](https://github.com/shtttt6/voice-ring) | Ring design (Tauri app) | `src-tauri/Cargo.toml` | `0.8.18` |
| [stoorps/stoorps.dev](https://github.com/stoorps/stoorps.dev) | Furniture model generation | `Cargo.toml` | `=0.8.20` |
| [Ontixa/worldos](https://github.com/Ontixa/worldos) | CAD kernel adapter crate over cadrum | `crates/worldos-adapter-cadrum/Cargo.toml` | `=0.8.20` |
| [MFarabi619/MFarabi619](https://github.com/MFarabi619/MFarabi619) | Robot mechanism generation | `apps/robot/mech/Cargo.toml` | `0.9` |
| [elasticdotventures/\_b00t\_](https://github.com/elasticdotventures/_b00t_) | CAD module | `b00t-cad/Cargo.toml` | `0.8.13` |
| [liuming_8912/STPtoDXF_AI](https://gitcode.com/liuming_8912/STPtoDXF_AI) (GitCode) | Converter from STEP into DXF drawings | `Cargo.toml` | `0.8` |
| [1hp-left/Fission180](https://github.com/1hp-left/Fission180) | Desktop CAD app modeled on Autodesk Fusion | `crates/openfusion-core/Cargo.toml` | `=0.9.0` |
| [mac999/cadrust](https://github.com/mac999/cadrust) | Fusion-style parametric modeler for the browser (WebAssembly) and the command line | `crates/cadrust-core/Cargo.toml` | `0.9` |
| [runinit/boardstudio](https://github.com/runinit/boardstudio) | Keyboard design workbench (mechanical assemblies) | `cad/wasm/Cargo.toml` | `=0.8.20` |
| [Teased-oChroid-orrA/powershell_tool](https://github.com/Teased-oChroid-orrA/powershell_tool) | Engineering toolbench with modelling and finite-element analysis | `solid-kernel/Cargo.toml` | `0.9` (optional) |
| [base698/ferrender](https://github.com/base698/ferrender) | Fusion-style parametric CAD with an MCP command interface | `crates/fr-core/Cargo.toml` | `=0.8.20` |
| [cameronprops/BuilderCraft](https://github.com/cameronprops/BuilderCraft) | Design environment (Worldwright); cadrum in a kernel evaluation crate | `evaluations/cadrum/Cargo.toml` | `=0.8.20` |

## Changes since the 2026-09-30 table

- Added: the six repositories from 1hp-left/Fission180 down. base698/ferrender was reported by hand, and the code search found the others. The GitCode repository was already appended to the original table.
- Removed: [kevinx-key/keyboard-dev-toolkit](https://github.com/kevinx-key/keyboard-dev-toolkit) dropped cadrum on 2026-10-12 (commit `2b3878d`, which moved STEP handling to a cloud service).
- Updated: MFarabi619/MFarabi619 now requires `0.9` instead of `^0.8`.

## Not counted

- [nevenincs/cadrumo](https://github.com/nevenincs/cadrumo) matches the search by name only and does not depend on cadrum.
- The crates.io reverse dependencies list one crate, `alphastell`, which is the author's own.
