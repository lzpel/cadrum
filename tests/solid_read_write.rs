//! The BRep reader must ignore bytes past its own payload — that is what lets the
//! color trailer live there.

use cadrum::{DVec3, Solid};

fn test_box() -> Vec<Solid> {
	vec![Solid::cube(DVec3::ZERO, DVec3::ONE)]
}

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

#[test]
fn step_color_is_srgb() {
	// #307: a hex color must land in COLOUR_RGB as given, not gamma-shifted.
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE).color("#336699");
	let mut buf = Vec::new();
	Solid::write_step([&cube], &mut buf).unwrap();
	let text = String::from_utf8_lossy(&buf);
	let line = text.lines().find(|l| l.contains("COLOUR_RGB")).expect("COLOUR_RGB entity");
	let args = &line[line.find("'',").unwrap() + 3..line.rfind(')').unwrap()];
	let rgb: Vec<f64> = args.split(',').map(|v| v.trim().parse().unwrap()).collect();
	for (got, want) in rgb.iter().zip([0.2, 0.4, 0.6]) {
		assert!((got - want).abs() < 1e-6, "{line}");
	}

	let read = Solid::read_step(&mut buf.as_slice()).unwrap();
	let color = *read[0].colormap().values().next().expect("color survives the round trip");
	for (got, want) in [color.r, color.g, color.b].into_iter().zip([0.2, 0.4, 0.6]) {
		assert!((got - want).abs() < 1e-6, "{color:?}");
	}
}
