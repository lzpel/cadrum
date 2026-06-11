use wasm_bindgen::prelude::*;


#[cfg(feature = "pure")]
pub fn volume() -> f64 {
	1.0
}

#[cfg(feature = "cpp")]
#[cxx::bridge]
mod ffi {
	unsafe extern "C++" {
		include!("cpp.h");
		fn add(a: f64, b: f64) -> f64;
	}
}

#[cfg(feature = "cpp")]
pub fn volume() -> f64 {
	ffi::add(2.0, 3.0)
}

#[cfg(feature = "cadrum")]
pub fn volume() -> f64 {
	use cadrum::{DVec3, Solid};
	let solid = Solid::cube(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0)).color("#4a90d9");
	solid.volume()
}

#[wasm_bindgen]
pub fn print_volume() -> String {
	format!("Solid volume: {}", volume())
}
