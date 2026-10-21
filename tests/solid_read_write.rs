//! The BRep reader must ignore bytes past its own payload — that is what lets the
//! color trailer live there.

use cadrum::{Color, DVec3, Solid};

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
fn step_color_lossless() {
	// Color space policy: STEP and Color share one color space, so a round trip changes no bit.
	let colors = [Color::from("#336699"), Color::from("#f00"), Color { r: 0.0123457, g: 0.7654321, b: 0.03 }];
	let solids: Vec<Solid> = colors.iter().enumerate().map(|(i, &c)| Solid::cube(DVec3::splat(2.0 * i as f64), DVec3::splat(2.0 * i as f64 + 1.0)).color(c)).collect();
	let mut buf = Vec::new();
	Solid::write_step(&solids, &mut buf).unwrap();
	// STEP wraps long lines, so an entity's values may span two lines.
	let text = String::from_utf8_lossy(&buf).replace(['\r', '\n'], "");
	let written: Vec<Color> = text
		.split("COLOUR_RGB('',")
		.skip(1)
		.map(|rest| {
			let v: Vec<f32> = rest[..rest.find(')').unwrap()].split(',').map(|x| x.trim().parse().unwrap()).collect();
			Color { r: v[0], g: v[1], b: v[2] }
		})
		.collect();
	assert_eq!(written, [colors[0], colors[2]], "COLOUR_RGB holds Color unconverted; #f00 is pre-defined");
	let read = Solid::read_step(&mut buf.as_slice()).unwrap();
	let got: Vec<Color> = read.iter().map(|s| s.colormap()[&s.id()]).collect();
	assert_eq!(got, colors);
}

// A triangulation stored in the BRep would win over the reader's coarser request.
#[test]
fn write_brep_leaves_triangulation_out() {
	let fine = cadrum::Tessellation { deflection_linear: 0.001, deflection_angular: 0.05, relative_linear: true };
	let coarse = cadrum::Tessellation { deflection_linear: 0.05, deflection_angular: 1.0, relative_linear: true };
	let shape = vec![Solid::cylinder(20.0, DVec3::new(0.0, 0.0, 30.0))];
	let fine_count = Solid::mesh(&shape, fine).expect("mesh").vertices.len();

	let mut buf = Vec::new();
	Solid::write_brep(&shape, &mut buf).expect("write_brep");
	let back = Solid::read_brep(&mut buf.as_slice()).expect("read_brep");
	let coarse_count = Solid::mesh(&back, coarse).expect("mesh").vertices.len();
	assert!(coarse_count < fine_count, "coarse {coarse_count} vs fine {fine_count}: the BRep carried the fine triangulation");
}
