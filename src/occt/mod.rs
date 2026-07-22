mod compound;
pub mod edge;
pub mod face;
// cxx bridge to the OCCT wrapper; the file lives at src/ffi.rs next to
// src/ffi.h / src/ffi.cpp, mounted here to stay crate-internal.
#[path = "../ffi.rs"]
mod ffi;
pub mod io;
pub mod solid;
