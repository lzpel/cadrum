//! Test: can the OCCT BRep reader tolerate extra trailing bytes?
//!
//! We write a valid BRep, append 1 KB of garbage, then read it back. The reader
//! must consume only what it needs (count-driven parsing) and ignore the rest —
//! which is also what lets the color trailer live past the payload's end.

use cadrum::{DVec3, Solid};

fn test_box() -> Vec<Solid> {
	vec![Solid::cube(DVec3::ZERO, DVec3::ONE)]
}

/// Write BRep binary, append 1 KB of 0xAB, read back.
#[test]
fn read_brep_with_trailing_garbage() {
	let shape = test_box();

	// Write valid BRep binary
	let mut buf = Vec::new();
	cadrum::Solid::write_brep(&shape, &mut buf).expect("write_brep should succeed");
	let brep_len = buf.len();
	assert!(brep_len > 0);

	// Append 1 KB of garbage
	buf.extend_from_slice(&[0xAB; 1024]);
	assert_eq!(buf.len(), brep_len + 1024);

	// Read back — should succeed despite trailing garbage
	let result = cadrum::Solid::read_brep(&mut buf.as_slice());
	match &result {
		Ok(solids) => {
			assert!(!solids.is_empty(), "should read at least one solid");
			let vol = solids[0].volume();
			assert!((vol - 1.0).abs() < 1e-6, "unit box volume should be ~1.0, got {}", vol);
			println!("BINARY: OK — read {} solid(s), volume={:.6}, brep_len={}, total={}", solids.len(), vol, brep_len, brep_len + 1024);
		}
		Err(e) => {
			println!("FAILED — {:?} (brep_len={}, total={})", e, brep_len, brep_len + 1024);
			panic!("read_brep failed with trailing garbage: {:?}", e);
		}
	}
}
