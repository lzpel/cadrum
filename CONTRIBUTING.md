# Contributing Guide

Contributions, feature requests, usage questions, and general contacts of any kind are absolutely welcome.

## Contact

- GitHub Issues: <https://github.com/lzpel/cadrum/issues> — preferred for
  bugs, feature requests, and design discussions.
- GitHub: [@lzpel](https://github.com/lzpel)

## Submitting Changes

If you have a patch you think is worth inspecting right away, opening a pull request without prelude is fine, although an accompanying explanation of what the patch does and why is appreciated.

For larger or design-affecting changes, please open an issue first to discuss the approach. The trait surface in `src/traits.rs` and the codegen pipeline in `make update` interact in non-obvious ways, so a quick alignment saves rework.

## Environment

The project's build, test, and release commands are driven by `make`:

```sh
make test     # cargo test (unit + integration + doc tests)
make update   # regenerate codegen/README/markdown output and build the mdbook site
make publish  # publish to crates.io (for maintainers), including upload releases.
```

### Before opening a PR

`make test` — runs unit, integration, and doc tests.
