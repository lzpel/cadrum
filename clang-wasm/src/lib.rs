//! generic clang.wasm を wasmtime で in-process 実行し、host の preopen 越しに C/C++ を
//! wasm へコンパイルする。clang.wasm 内蔵の bridge が起動時に preopen を MEMFS へコピーし、
//! 出力（guest "/out"）を host へ書き戻す。外部 wasmtime/node/wasi-sdk は不要。
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::{DirPerms, FilePerms, I32Exit, WasiCtxBuilder};

/// 1 つの preopen（host ディレクトリ → guest パス）。
pub struct Preopen {
    pub host: PathBuf,
    pub guest: String,
}

/// generic clang.wasm を実行する。
/// - `wasm`: generic clang.wasm のパス
/// - `preopens`: host dir を guest パスにマップ（入力 dir 群＋出力 dir は guest "/out"）
/// - `args`: clang の引数（argv[0] は "clang" を渡す）
///
/// 返り値は clang のプロセス終了コード。
pub fn run_clang(wasm: &Path, preopens: &[Preopen], args: &[String]) -> Result<i32> {
    // exnref EH を実行するため function-references / gc / exceptions を有効化（CLI の -W 群と等価）。
    let mut config = Config::new();
    config.wasm_function_references(true);
    config.wasm_gc(true);
    config.wasm_exceptions(true);
    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, wasm).with_context(|| format!("load {}", wasm.display()))?;

    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    p1::add_to_linker_sync(&mut linker, |t| t)?;

    let mut builder = WasiCtxBuilder::new();
    builder.inherit_stderr();
    builder.args(args);
    for p in preopens {
        builder
            .preopened_dir(&p.host, &p.guest, DirPerms::all(), FilePerms::all())
            .with_context(|| format!("preopen {} -> {}", p.host.display(), p.guest))?;
    }
    let mut store = Store::new(&engine, builder.build_p1());

    let instance = linker.instantiate(&mut store, &module)?;
    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
    match start.call(&mut store, ()) {
        Ok(()) => Ok(0),
        Err(e) => {
            // clang は終了時 proc_exit を呼ぶ → wasmtime は I32Exit で trap する。
            if let Some(exit) = e.downcast_ref::<I32Exit>() {
                Ok(exit.0)
            } else {
                Err(e)
            }
        }
    }
}
