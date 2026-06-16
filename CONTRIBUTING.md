# Contributing Guide

Contributions, feature requests, usage questions, and general contacts of any
kind are absolutely welcome.

Note that this is a hobby project worked on in spare time. I can only give a
best-effort promise of availability or responsiveness, but please do reach
out with anything you need.

## Contact

- GitHub Issues: <https://github.com/lzpel/cadrum/issues> — preferred for
  bugs, feature requests, and design discussions.
- GitHub: [@lzpel](https://github.com/lzpel)

## Preconditions

You should be comfortable with `cargo` and the basics of building Rust
projects. The first build downloads a prebuilt OCCT 8.0.0 tarball for
supported targets and links it statically; on unsupported targets you will
need CMake plus a C++17 compiler and `cargo build --features source`.
See the [README](./README.md#build) for the full build matrix.

The project uses **tab indentation** (`U+0009`) throughout. `rustfmt.toml`
sets `hard_tabs = true`, so running `cargo fmt` keeps formatting consistent.

This project aims to expose OpenCASCADE through an idiomatic Rust API, so
contributions to documentation, examples, and ergonomic tweaks are equally
welcome alongside patches to the FFI core.

## Submitting Changes

If you have a patch you think is worth inspecting right away, opening a pull
request without prelude is fine, although an accompanying explanation of
what the patch does and why is appreciated.

For larger or design-affecting changes, please open an issue first to
discuss the approach. The trait surface in `src/traits.rs` and the codegen
pipeline in `examples/codegen.rs` interact in non-obvious ways, so a quick
alignment saves rework.

If you have questions, bugs, suggestions, or any other contributions that
do not immediately touch the codebase, please open an issue or reach out
informally on GitHub.

## Environment

The project's build, test, and release commands are driven by `make`:

```sh
make test     # cargo test (unit + integration + doc tests)
make deploy   # regenerate examples/markdown output and build the mdbook site
make publish  # publish to crates.io
```

### Regenerating derived files

When you change `src/traits.rs`, regenerate the inherent-method delegations
in `src/lib.rs`:

```sh
cargo run --example codegen -- src/traits.rs src/lib.rs
```

When you add or modify a numbered example (`examples/NN_*.rs`), regenerate
the README's `## Examples` section and the mdbook source:

```sh
cargo run --example markdown -- out/markdown/SUMMARY.md ./README.md
```

Both regenerators produce deterministic output. Commit the resulting diffs
together with the source change that motivated them.

### Before opening a PR

1. `cargo fmt` — keep formatting consistent (`hard_tabs = true`).
2. `cargo test` — runs unit, integration, and doc tests.
3. If you touched `src/traits.rs` or `examples/NN_*.rs`, run the relevant
   regenerator above and commit the diff.
