//! `Error` が `std::io::Error` を吸収することの検証。これがないと
//! `File::create(..)?` を `Result<_, cadrum::Error>` の関数内で書けない。

use cadrum::{DVec3, Error, Solid};
use std::error::Error as _;
use std::fs::File;

/// `?` だけで io と cadrum のエラーを混ぜられることを型で示す。
fn write_step_to(path: &std::path::Path) -> Result<u64, Error> {
	let solid = Solid::cube(DVec3::ZERO, DVec3::splat(2.0));
	Solid::write_step([&solid], &mut File::create(path)?)?;
	Ok(std::fs::metadata(path)?.len())
}

#[test]
fn test_io_error_composes_with_question_mark() {
	let path = std::env::temp_dir().join("cadrum_test_error_io.step");
	let size = write_step_to(&path).unwrap();
	assert!(size > 0, "STEP written through `?` should be non-empty");
	std::fs::remove_file(&path).unwrap();
}

#[test]
fn test_io_error_is_converted_and_kept() {
	// 存在しないディレクトリ配下なので File::create が NotFound を返す。
	let path = std::env::temp_dir().join("cadrum_no_such_dir_xyz").join("a.step");
	let error = write_step_to(&path).unwrap_err();

	let Error::Io(inner) = &error else {
		panic!("expected Error::Io, got {error:?}");
	};
	assert_eq!(inner.kind(), std::io::ErrorKind::NotFound);
	assert!(error.to_string().starts_with("I/O error: "), "unexpected Display: {error}");
	assert!(error.source().is_some(), "source() should expose the io::Error");
}
