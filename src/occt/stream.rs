use std::ffi::c_void;
use std::io::{Read, Write};

/// C ABI mirror of `CReader` in `cpp/ffi.h`: an opaque context plus a function
/// pointer the C++ streambuf calls to pull bytes. `ctx` points at a
/// [`RustReader`]; `read` is [`read_trampoline`].
#[repr(C)]
pub(crate) struct CReader {
	ctx: *mut c_void,
	read: extern "C" fn(*mut c_void, *mut u8, usize) -> usize,
}

/// C ABI mirror of `CWriter` in `cpp/ffi.h`.
#[repr(C)]
pub(crate) struct CWriter {
	ctx: *mut c_void,
	write: extern "C" fn(*mut c_void, *const u8, usize) -> usize,
}

/// Trampoline: recover the `RustReader` from the opaque ctx and read into `buf`.
extern "C" fn read_trampoline(ctx: *mut c_void, buf: *mut u8, len: usize) -> usize {
	// SAFETY: `ctx` is the `&mut RustReader` handed to `RustReader::as_c`, alive
	// for the duration of the synchronous C++ call.
	let reader = unsafe { &mut *(ctx as *mut RustReader) };
	let slice = unsafe { std::slice::from_raw_parts_mut(buf, len) };
	rust_reader_read(reader, slice)
}

/// Trampoline: recover the `RustWriter` from the opaque ctx and write `buf`.
extern "C" fn write_trampoline(ctx: *mut c_void, buf: *const u8, len: usize) -> usize {
	// SAFETY: see `read_trampoline`.
	let writer = unsafe { &mut *(ctx as *mut RustWriter) };
	let slice = unsafe { std::slice::from_raw_parts(buf, len) };
	rust_writer_write(writer, slice)
}

/// Wrapper around `dyn Read` passed to C++ as an opaque extern Rust type.
///
/// C++ calls `rust_reader_read()` to pull bytes from the Rust reader,
/// feeding them into a `std::streambuf` subclass that OCC reads from.
///
/// # Safety
/// The lifetime is erased internally. The caller must ensure the reader
/// outlives the FFI call (which is always the case since C++ calls are
/// synchronous and blocking).
pub struct RustReader {
	inner: *mut dyn Read,
}

impl RustReader {
	/// Create a new RustReader wrapping the given reader.
	///
	/// # Safety
	/// The caller must ensure that the resulting `RustReader` is not used
	/// after `reader` is dropped. In practice, this is guaranteed because
	/// the C++ FFI call is synchronous.
	pub fn from_ref<'a>(reader: &'a mut (dyn Read + 'a)) -> Self {
		// SAFETY: Caller must ensure `reader` outlives this RustReader.
		// The `'static` bound is required by the raw pointer type, so we
		// use transmute to erase the lifetime (lifetimes are compile-time only).
		RustReader { inner: unsafe { std::mem::transmute::<*mut (dyn Read + 'a), *mut (dyn Read + 'static)>(reader as *mut (dyn Read + 'a)) } }
	}

	/// Build the C ABI view (`ctx` + trampoline) passed to the C++ streambuf.
	/// The returned `CReader` borrows `self` and must not outlive it.
	pub(crate) fn as_c(&mut self) -> CReader {
		CReader { ctx: self as *mut RustReader as *mut c_void, read: read_trampoline }
	}
}

/// Wrapper around `dyn Write` passed to C++ as an opaque extern Rust type.
///
/// C++ calls `rust_writer_write()` to push bytes into the Rust writer,
/// receiving them from a `std::streambuf` subclass that OCC writes to.
pub struct RustWriter {
	inner: *mut dyn Write,
}

impl RustWriter {
	/// Create a new RustWriter wrapping the given writer.
	///
	/// # Safety
	/// Same as `RustReader::from_ref`.
	pub fn from_ref<'a>(writer: &'a mut (dyn Write + 'a)) -> Self {
		// SAFETY: Caller must ensure `writer` outlives this RustWriter.
		// See RustReader::from_ref for the same rationale.
		RustWriter { inner: unsafe { std::mem::transmute::<*mut (dyn Write + 'a), *mut (dyn Write + 'static)>(writer as *mut (dyn Write + 'a)) } }
	}

	/// Build the C ABI view (`ctx` + trampoline) passed to the C++ streambuf.
	/// The returned `CWriter` borrows `self` and must not outlive it.
	pub(crate) fn as_c(&mut self) -> CWriter {
		CWriter { ctx: self as *mut RustWriter as *mut c_void, write: write_trampoline }
	}
}

/// FFI callback: read up to `buf.len()` bytes from the RustReader.
/// Returns the number of bytes actually read (0 = EOF).
pub fn rust_reader_read(reader: &mut RustReader, buf: &mut [u8]) -> usize {
	unsafe { (*reader.inner).read(buf).unwrap_or(0) }
}

/// FFI callback: write bytes into the RustWriter.
/// Returns the number of bytes actually written.
pub fn rust_writer_write(writer: &mut RustWriter, buf: &[u8]) -> usize {
	unsafe { (*writer.inner).write(buf).unwrap_or(0) }
}
