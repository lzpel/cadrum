use std::io::{Read, Write};

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
}

/// FFI callback: read up to `len` bytes from the `RustReader` behind `reader`.
/// Returns the number of bytes actually read (0 = EOF). Called by the C++
/// `RustReadStreambuf` with an opaque pointer produced in `super::ffi`.
#[no_mangle]
extern "C" fn cadrum_reader_read(reader: *mut std::ffi::c_void, buf: *mut u8, len: usize) -> usize {
	let reader = unsafe { &mut *reader.cast::<RustReader>() };
	let buf = unsafe { std::slice::from_raw_parts_mut(buf, len) };
	unsafe { (*reader.inner).read(buf).unwrap_or(0) }
}

/// FFI callback: write `len` bytes into the `RustWriter` behind `writer`.
/// Returns the number of bytes actually written.
#[no_mangle]
extern "C" fn cadrum_writer_write(writer: *mut std::ffi::c_void, buf: *const u8, len: usize) -> usize {
	let writer = unsafe { &mut *writer.cast::<RustWriter>() };
	let buf = unsafe { std::slice::from_raw_parts(buf, len) };
	unsafe { (*writer.inner).write(buf).unwrap_or(0) }
}
