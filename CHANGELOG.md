# Changelog

All notable changes to `cadrum` will be documented in this file.

This document is written according to the [Keep a Changelog][kac] style.

1. [Version 0](#version-0)
	1. [0.8.0](#080)
	1. [0.7.6](#076)
	1. [0.7.5](#075)
	1. [0.7.2](#072)
	1. [0.6.0](#060)
	1. [0.5.1](#051)

## Version 0

`cadrum` is in the `0.x` series. Minor-version bumps may include breaking
changes until `1.0`.

### 0.8.0

#### Breaking

- **Boolean API を単体×単体 + FFI 直通に限定。** OCCT の General BOP
  (multi-args × multi-tools) は集合論的な `op(∪args, ∪tools)` を実装して
  おらず、グループ内自己交差は未定義/破綻する (tools 自己交差で体積が
  物理的に意味を失い、multi-tools intersect は連結結果でなく per-pair
  piece を返す)。詳細は `notes/20260514-boolean演算は単体x単体のみ公開する方針.md`。
  - **削除**: `Solid::union/subtract/intersect`、`Vec<Solid>::union/...`、
    `Compound::union/subtract/intersect`。
  - **追加**: `Solid::boolean_union/boolean_subtract/boolean_intersect`
    (multi-args × multi-tools 通る唯一のパス、FFI 直通)。
  - **追加**: `&Solid: Add/Sub/Mul` 演算子 (戻り値 `Result<Solid, Error>`)。
    結果 Solid 数が 1 でないとき `Error::OneFailed(n)` を返す。
  - **追加**: `Result<Solid, Error>: Sum<&Solid> + Product<&Solid>`。
    `iter.sum::<Result<Solid, _>>()` で union 畳み込み、`product` で
    intersect 畳み込み。中間は `Vec<Solid>` を保持し、終端でだけ
    `exactly_one` 判定するため、途中で多ピースになるが最終的に 1 個に
    連結するケース (オリンピックの輪を out-of-order で fold する等) も
    正常に成功する。
  - **追加**: `Error::OneFailed(usize)`。
  - `SolidStruct` の HRTB で `for<'a> &'a Self: Add + Sub + Mul` と
    `for<'a> Result<Self, Error>: Sum<&'a Self> + Product<&'a Self>` を
    強制 → backend が boolean を実装する契約に演算子が含まれる。

  **Migration**:

  ```rust
  // 旧 API → 新 API
  a.union(&b)            → Solid::boolean_union([&a], [&b])      // または (&a + &b)?
  a.subtract(&[hole])    → Solid::boolean_subtract([&a], [&hole]) // または (&a - &hole)?
  vec.union(&tools)      → Solid::boolean_union(&vec, &tools)
  // 複数を 1 個に畳む
  vec.iter().sum::<Result<Solid, _>>()?
  ```

#### Changes

- **OCCT bumped to 8.0.0** (final release; was 8.0.0-beta1). No source
  changes required — APIs deprecated/removed in V8_0_0 (`Standard_Mutex`,
  OCCT math wrappers like `::Sin` / `::Cos`, `NCollection_BasePointerVector`,
  `TColGeom`, `PLib_Base`, `BRepMesh_PluginMacro`) are unused by cadrum's
  `cpp/wrapper.cpp`. The toolkit list (`TKernel`, `TKMath`, ..., `TKDESTEP`)
  is unchanged.
- `tests/subtract.rs` → `tests/boolean_subtract.rs`、`tests/union.rs` →
  `tests/boolean_union.rs` にリネーム (boolean 系の検証テストを命名で
  集約)。

#### Fixes

- `examples/codegen.rs`: trait header の supertrait 抽出が `where` 句を
  含めて誤読していた問題を修正。HRTB を含む where 句 (`+` を含む
  trait bound) があると、`Compound` 等の supertrait が `"Compound where
  for<'a> ..."` という文字列にぶら下がって認識されず、`impl Solid` への
  forwarder 生成が大量に欠落していた。` where ` 以降を切り落とすように
  修正。

### 0.7.6

#### Notes

Documentation-only release. No public API changes.

The README is now the single source of truth for both GitHub and the
docs.rs landing page, mirroring the [bitvec][bitvec-docs] convention.

#### Changes

- `src/lib.rs` reduced to `#![doc = include_str!("../README.md")]`. The
  crate-root prose now lives in `README.md`.
- `examples/markdown.rs` emits ` ```rust,no_run ` fences for example
  programs so the `include_str!`'d README does not turn each example
  into a slow doctest.
- README's top section centered with `<div align="center">`, badges and
  links converted to reference-style definitions, new `docs.rs`
  build-status badge.
- `CODE_OF_CONDUCT.md` (Rust CoC) and `CONTRIBUTING.md` added at the repo
  root.
- `CHANGELOG.md` extracted from the previous `## Release Notes` section
  of the README.
- `examples/codegen.rs` region indent normalized to tabs based on brace
  depth so regenerated `impl` blocks honor the project's tab-indent
  convention.

### 0.7.5

#### Notes

Aggregated changes since 0.7.2 (no separate entries were written for
0.7.3 / 0.7.4).

#### Changes

- **OCCT bumped to 8.0.0-beta1** ahead of the May 7 final release.
  Inherits upstream perf gains (STEP read up to ~75% faster vs 7.7) and
  the Shape-Healing / `BRepFill_PipeShell` crash fixes.
- **Linux prebuilts are now self-contained** (#147): `libstdc++.a` /
  `libgcc.a` / `libgcc_eh.a` are bundled into the tarball, so binaries
  linked against the prebuilt no longer depend on the host distro's
  libstdc++ runtime — fixes link-time `__cxa_call_terminate` undefined
  errors on Amazon Linux 2023 and other distros with older default GCC.
  Same self-contained guarantee that mingw already had since 0.7.2 (#89).
- **`x86_64-pc-windows-gnullvm` prebuilt dropped.** The prior "support"
  was a relabeled `windows-gnu` artifact, not a real llvm-mingw build.
  Use `--features source-build` or switch to the `windows-gnu` toolchain.
- **I/O methods relocated to `Solid` impl** (#145):
  `Solid::write_step / write_brep_binary / write_brep_text / read_step / read_brep`.
  The free-standing `cadrum::write_*` re-exports are gone.
  **Breaking vs 0.7.4**: `cadrum::write_step(...)` →
  `Solid::write_step(...)`, etc.
- **`Edge::id()` / `Face::id()` / `Solid::id()`** (#142, #143):
  TShape-pointer-based identity exposed as a stable `u64` for cross-shape
  correspondence (e.g. before/after boolean ops). Replaces the
  underscored `tshape_id`. **Breaking** for callers that named the old
  method.
- **`Face::iter_edge() -> impl Iterator<Item = &Edge>`** (#143):
  face-edge incidence query without going through the Solid boundary
  explorer.
- **`Face::project(point)`** (#142): closest-point + normal query on a
  face via `BRepExtrema_DistShapeShape`. Sibling to the existing
  `Edge::project` / `Wire::project`.
- **C¹-periodic B-spline seam fix** (#120):
  `Solid::bspline(_, periodic=true)` no longer emits a discontinuous
  U=0 seam — surfaces that previously showed dents at the seam now
  interpolate smoothly. Regression test in `tests/bspline.rs`.

### 0.7.2

#### Notes

Aggregated changes since 0.6.0 (no separate entries were written for
0.6.1 – 0.7.1).

#### Changes

- **`Solid::shell(thickness, open_faces)`** — hollow a solid via
  `BRepOffsetAPI_MakeThickSolid`. Empty `open_faces` produces a sealed
  internal void (cavity). Example: `examples/08_shell.rs`.
- **`Solid::fillet_edges(radius, edges)` /
  `Solid::chamfer_edges(distance, edges)`** — uniform fillet / chamfer
  on selected edges via `BRepFilletAPI_MakeFillet` / `MakeChamfer`.
- **`Solid::area()` / `Solid::center()` / `Solid::inertia()`** — surface
  area, center of mass, inertia tensor. Replaces the previous
  `shell_count` query.
- **`Wire::project(point)`** — closest-point + tangent query on
  `Edge` / `Vec<Edge>` / `[Edge; N]` via `GeomAPI_ProjectPointOnCurve`.
- **`Edge::end_point()` / `Edge::end_tangent()`** — added as siblings
  to the existing `start_*` accessors.
- **`Solid::iter_edge()` / `Solid::iter_face()`** — yield `&Edge` /
  `&Face` references through internal `OnceLock` caches; first call
  populates, subsequent calls are free.
- **`Solid::history` + `Solid::iter_history()`** — face-derivation pairs
  `[post_id, src_id]` populated by boolean ops and `clean()`. Lets
  callers select result faces by their original input membership.
- **Multi-color STEP read recovery (#129).** SolveSpace-style multi-color
  STEP files (which duplicate `EDGE_CURVE` entities at face boundaries
  instead of sharing them) used to land as `Compound{Shell×N}` with zero
  solids, breaking every downstream op. A `BRepBuilderAPI_Sewing`
  post-process now stitches coincident edges, promotes the result to one
  valid `Solid`, and remaps the colormap. The same STEP file is
  currently unfixable in CadQuery — see
  `sandbox-cadquery/read_step_fillet.py`.
- **`Mesh::write_svg` / `Mesh::to_svg` gained `up_dir: DVec3`** between
  `view: DVec3` and `hidden_lines: bool` (#127). **Breaking vs 0.7.0**:
  pass `DVec3::Z` to reproduce earlier output.
- **`Transform` trait no longer in the public prelude** (#91) — its
  methods reach you via `Compound` / `Wire` forwarders, so
  `use cadrum::{Compound, Wire};` is enough for every transform call.
  **Breaking vs 0.7.0** for code that imported `Transform` explicitly.
- **`*_with_metadata` boolean variants removed** (#130) — the same
  information is now available via `Solid::iter_history()` on the
  result solid. **Breaking** for callers that consumed the metadata
  tuple.
- **glam types re-exported from the crate root** (#94, #95) — downstream
  code no longer needs its own `glam` dependency for `DVec3` etc.
- **OCCT `Statistics on Transfer` stdout chatter silenced** on every
  STEP read / write (#97).
- **mingw prebuilt is now self-contained** (#89): bundles the
  container's `libstdc++.a` / `libgcc.a`, so user-built
  `x86_64-pc-windows-gnu` executables do not depend on MinGW runtime
  DLLs at link time.
- **docs.rs build restored** (#107, #111): dropped the unsupported
  `x86_64-pc-windows-msvc` target and reordered `build.rs` so trait
  delegation generation runs before the DOCS_RS early-return.
- New example `08_shell.rs` (hollow torus carved by halfspace-cut
  openings); old `08_bspline.rs` renumbered to `09_bspline.rs`. Top
  README image updated to the alphastell stellarator render (#125).

### 0.6.0

#### Changes

- **`source-build` feature now gates `cmake`/`walkdir` as optional
  build-dependencies.** Default `cargo build` no longer compiles them,
  significantly reducing build time on prebuilt targets. Users on
  unsupported targets must enable `--features source-build` (behavior
  unchanged — previously these targets also failed, just with a
  download error instead of a clear message).
- **`x86_64-pc-windows-gnu` prebuilt added** via Docker
  cross-compilation with Debian mingw-w64 (posix thread model). All
  MinGW runtime DLLs are statically absorbed — the resulting exe
  depends only on Windows OS DLLs.
- **LGPL 2.1 §2 compliance:** source builds now retain only the ~9
  patched OCCT source files alongside the `.a` libraries, removing the
  unmodified bulk (~88 MB of data/dox/tests). The patched files carry
  timestamped headers per §2(a).
- **`OCCT_ROOT` relative path handling fixed:** resolved via
  `env::current_dir()` instead of the unreliable `CARGO_TARGET_DIR`
  heuristic. `--target <triple>` flag now works correctly.
- **`build.rs` restructured:** `resolve_occt` uses match chains with
  `#[cfg]` for source-build vs prebuilt paths. Source-build code lives
  in `#[cfg(feature = "source-build")] mod source`.
  `patch_occt_sources` split into `walk_occt_sources` + `patch_or_none`
  (side-effect-free).
- **README simplified:** Build section moved after Usage with a
  prebuilt target table + OS icons.

### 0.5.1

#### Notes

> 0.4.5 was published briefly but its version number was lower than the
> already-published 0.5.0 (OCCT 7.9.3, older feature set), so
> `cargo add cadrum` would silently pick up 0.5.0 instead of the newer
> 0.4.5 code. Re-released as 0.5.1 with identical contents. Prefer
> 0.5.1 over 0.4.5.

#### Changes

- **`Solid::bspline<const M, const N>(grid, periodic)`** — new
  constructor: build a periodic B-spline solid from a 2D control-point
  grid. V (cross-section) is always periodic; U (longitudinal) is
  controlled by the `periodic` flag (torus when `true`, capped pipe
  when `false`). Implemented via
  `GeomAPI_PointsToBSplineSurface::Interpolate` over an augmented grid
  plus `SetUPeriodic`/`SetVPeriodic`.
- **`write_svg` / `Mesh::to_svg` now take `shading: bool`** — opt-in
  Lambertian shading with head-on light. When `true`, triangles are
  tinted by `0.5 + 0.5 * (normal · dir)` so curved/organic shapes read
  clearly; `false` reproduces the pre-0.5.1 flat rendering. **Breaking
  vs 0.5.0**: existing callers must add the flag (pass `false` to
  preserve earlier output).
- **`examples/08_bspline.rs`** rewritten: 2 field-period stellarator-like
  torus with twisted + vertically undulating elliptic cross-sections,
  exercising `Solid::bspline` and `shading=true`.
- **`tests/bspline.rs`** added: verifies 180° point symmetry of the
  stellarator shape via XZ/YZ half-space intersection (s1 ≈ s3,
  s2 ≈ s4).
- **`Error::BsplineFailed(String)`** new variant. **Breaking** for
  downstream code that does exhaustive `match` on `Error`.
- OCCT 8.0.0 deprecation warnings resolved in `make_bspline_edge` and
  `make_bspline_solid` (`NCollection_HArray1<gp_Pnt>` via local `using`
  alias to bypass the `Handle()` macro comma-splitting issue;
  `NCollection_Array2<gp_Pnt>` directly).

[bitvec-docs]: https://docs.rs/bitvec/latest/bitvec/
[kac]: https://keepachangelog.com/
