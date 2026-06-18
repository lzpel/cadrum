//! clangw — generic clang.wasm を host から叩く薄い CLI（検証用）。
//!
//! 使い方:
//!   clangw <generic-clang.wasm> [--dir HOST::GUEST]... -- <clang args...>
//!
//! 例:
//!   clangw generic-clang.wasm --dir ./in::/in --dir ./out::/out -- \
//!     clang -c /in/t.cpp -o /out/t.o --target=wasm32-wasip1 -nostdinc -nostdinc++ -nobuiltininc
use anyhow::{bail, Result};
use clang_wasm::{run_clang, Preopen};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut a = std::env::args().skip(1);
    let wasm = PathBuf::from(a.next().ok_or_else(|| anyhow::anyhow!("usage: clangw <wasm> [--dir H::G].. -- <args>"))?);
    let mut preopens = Vec::new();
    let mut clang_args = Vec::new();
    let mut after_sep = false;
    while let Some(tok) = a.next() {
        if after_sep {
            clang_args.push(tok);
        } else if tok == "--" {
            after_sep = true;
        } else if tok == "--dir" {
            let spec = a.next().ok_or_else(|| anyhow::anyhow!("--dir needs HOST::GUEST"))?;
            let (h, g) = spec.split_once("::").ok_or_else(|| anyhow::anyhow!("--dir HOST::GUEST"))?;
            preopens.push(Preopen { host: PathBuf::from(h), guest: g.to_string() });
        } else {
            bail!("unexpected arg before --: {tok}");
        }
    }
    if clang_args.is_empty() {
        bail!("no clang args after --");
    }
    let code = run_clang(&wasm, &preopens, &clang_args)?;
    std::process::exit(code);
}
