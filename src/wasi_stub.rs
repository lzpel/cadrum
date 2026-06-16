//! No-op import shims for the `wasm32-unknown-unknown` build.
//!
//! libc++'s `<iostream>` static initialization and OCCT (getenv / STEP I/O /
//! `Standard_ErrorHandler`'s setjmp) drag in imports from `wasi_snapshot_preview1`
//! (and `env` for setjmp/longjmp) that have no runtime on `wasm32-unknown-unknown`.
//! Defining those symbols here as no-ops makes cadrum's wasm output self-contained
//! (no downstream WASI shim). Signatures match the WASI ABI (i32/i64/pointers).
//!
//! These replace the former `cpp/wasi_stub.c` (compiled via `cc` and linked
//! `+whole-archive`). In Rust the symbols live in cadrum's rlib, so they would be
//! dropped before libc's late references are resolved unless something reachable
//! pulls them in — that is what [`anchor`] does, invoked from `cadrum::wasm_start!`.
//! `__cxa_atexit` is intentionally NOT defined: libc owns its real definition and a
//! second one would be a duplicate symbol (cadrum is a cdylib, so static dtors do
//! not auto-run anyway).
#![allow(clippy::missing_safety_doc)]

use core::ffi::c_void;

// --- WASI ABI errno values used by the called-at-runtime stubs ---
const ERRNO_BADF: i32 = 8; // invalid file descriptor
const ERRNO_NOENT: i32 = 44; // no such file or directory

// stdio (iostream static init). Normal runs never write to stdout/stderr, so no-op.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_write(_fd: i32, _iovs: i32, _iovs_len: i32, _nwritten: i32) -> i32 {
	0
}
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_seek(_fd: i32, _offset: i64, _whence: i32, _newoffset: i32) -> i32 {
	0
}
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_close(_fd: i32) -> i32 {
	0
}
// terminate / abort path from libc++abi. Not reached in normal runs.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_proc_exit(_code: i32) {}
// startup preopen scan (`__wasilibc_populate_preopens`) calls fd_prestat_get with
// increasing fds until an error; return BADF immediately to end the scan.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_prestat_get(_fd: i32, _buf: i32) -> i32 {
	ERRNO_BADF
}
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_prestat_dir_name(_fd: i32, _path: i32, _path_len: i32) -> i32 {
	ERRNO_BADF
}
// environment imports dragged in by OCCT's getenv. Treat the environment as empty.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_environ_sizes_get(count: *mut u32, buf_size: *mut u32) -> i32 {
	unsafe {
		*count = 0;
		*buf_size = 0;
	}
	0
}
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_environ_get(_environ: i32, _buf: i32) -> i32 {
	0
}
// stdio init's isatty etc. Return BADF to treat the fd as invalid.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_fdstat_get(_fd: i32, _stat: i32) -> i32 {
	ERRNO_BADF
}
// fd_read: report 0 bytes read (EOF).
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_read(_fd: i32, _iovs: i32, _iovs_len: i32, nread: *mut u32) -> i32 {
	unsafe {
		*nread = 0;
	}
	0
}
// STEP write/read time + file imports. Time is 0; path lookups return NOENT so OCCT
// falls back to its built-in defaults.
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_clock_time_get(_id: i32, _precision: i64, time: *mut u64) -> i32 {
	unsafe {
		*time = 0;
	}
	0
}
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_fd_fdstat_set_flags(_fd: i32, _flags: i32) -> i32 {
	0
}
#[no_mangle]
pub extern "C" fn __imported_wasi_snapshot_preview1_path_filestat_get(_fd: i32, _flags: i32, _path: i32, _path_len: i32, _buf: i32) -> i32 {
	ERRNO_NOENT
}
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn __imported_wasi_snapshot_preview1_path_open(_fd: i32, _dirflags: i32, _path: i32, _path_len: i32, _oflags: i32, _rights_base: i64, _rights_inheriting: i64, _fdflags: i32, _opened_fd: i32) -> i32 {
	ERRNO_NOENT
}

// OCCT's `Standard_ErrorHandler` references setjmp/longjmp; on wasm these become `env`
// imports. cadrum uses C++ exceptions (not signal-based unwinding), so setjmp just
// returns 0 (fall through the protected block) and longjmp is never reached (trap if it is).
#[no_mangle]
pub extern "C" fn setjmp(_env: *mut c_void) -> i32 {
	0
}
#[no_mangle]
pub extern "C" fn longjmp(_env: *mut c_void, _val: i32) {
	core::arch::wasm32::unreachable()
}

/// Force every stub above into the final wasm module.
///
/// In Rust the stubs sit in cadrum's rlib; rustc only links that object — and only
/// keeps each `#[no_mangle]` symbol under LTO — if reachable Rust code references it.
/// libc's references arrive too late to pull the rlib member (the same reason the old
/// C stub needed `+whole-archive`). Referencing every stub from this reachable
/// function makes each one a link root. The calls live behind an opaque-false branch,
/// so they are retained but never executed (the null/zero arguments are never used).
#[doc(hidden)]
pub fn anchor() {
	if core::hint::black_box(false) {
		// Calling these `extern "C"` *definitions* is safe; the unsafe pointer work
		// lives inside them and never runs (this branch is opaque-false).
		let _ = __imported_wasi_snapshot_preview1_fd_write(0, 0, 0, 0);
		let _ = __imported_wasi_snapshot_preview1_fd_seek(0, 0, 0, 0);
		let _ = __imported_wasi_snapshot_preview1_fd_close(0);
		__imported_wasi_snapshot_preview1_proc_exit(0);
		let _ = __imported_wasi_snapshot_preview1_fd_prestat_get(0, 0);
		let _ = __imported_wasi_snapshot_preview1_fd_prestat_dir_name(0, 0, 0);
		let _ = __imported_wasi_snapshot_preview1_environ_sizes_get(core::ptr::null_mut(), core::ptr::null_mut());
		let _ = __imported_wasi_snapshot_preview1_environ_get(0, 0);
		let _ = __imported_wasi_snapshot_preview1_fd_fdstat_get(0, 0);
		let _ = __imported_wasi_snapshot_preview1_fd_read(0, 0, 0, core::ptr::null_mut());
		let _ = __imported_wasi_snapshot_preview1_clock_time_get(0, 0, core::ptr::null_mut());
		let _ = __imported_wasi_snapshot_preview1_fd_fdstat_set_flags(0, 0);
		let _ = __imported_wasi_snapshot_preview1_path_filestat_get(0, 0, 0, 0, 0);
		let _ = __imported_wasi_snapshot_preview1_path_open(0, 0, 0, 0, 0, 0, 0, 0, 0);
		let _ = setjmp(core::ptr::null_mut());
		longjmp(core::ptr::null_mut(), 0);
	}
}
