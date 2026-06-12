use wasm_bindgen::prelude::*;


#[cfg(feature = "pure")]
pub fn volume() -> f64 {
	1.0
}

#[cfg(feature = "cc")]
unsafe extern "C" {
	fn add(a: f64, b: f64) -> f64;
}
#[cfg(feature = "cc")]
pub fn volume() -> f64 {
	unsafe { add(2.0, 3.0) }
}

#[cfg(feature = "cxx")]
#[cxx::bridge]
mod ffi {
	unsafe extern "C++" {
		include!("ffi.h");
		fn add(a: f64, b: f64) -> f64;
	}
}

#[cfg(feature = "cxx")]
pub fn volume() -> f64 {
	ffi::add(2.0, 3.0)
}

#[cfg(feature = "cadrum")]
pub fn volume() -> f64 {
	use cadrum::{DVec3, Solid};
	let solid = Solid::cube(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0)).color("#4a90d9");
	// ファイルを経由せずメモリへ STEP を書き、メモリから読み戻す（OSD_File スタブ層を通らない）。
	let mut bytes: Vec<u8> = Vec::new();
	Solid::write_step([&solid], &mut bytes).expect("write_step to memory failed");
	let mut cursor = std::io::Cursor::new(&bytes);
	let solids = Solid::read_step(&mut cursor).expect("read_step from memory failed");
	solids.first().expect("no solid after round-trip").volume()
}

#[wasm_bindgen]
pub fn print_volume() -> String {
	format!("Solid volume: {}", volume())
}
