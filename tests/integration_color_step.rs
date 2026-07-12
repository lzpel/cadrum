//! Integration tests for colored STEP I/O.
//!
//! Reads `steps/colored_box.step` (an AP214 STEP file with per-face colors),
//! applies boolean / clean / translate operations, and writes results to `out/`.

#![cfg(feature = "color")]

use cadrum::Solid;
use glam::DVec3;
use std::fs;

const COLORED_BOX_STEP: &str = "steps/colored_box.step";

/// Read `colored_box.step` and return the shape.  Panics if reading fails.
fn read_colored_box() -> Vec<Solid> {
	let data = fs::read(COLORED_BOX_STEP).expect("steps/colored_box.step should exist");
	cadrum::Solid::read_step(&mut data.as_slice()).expect("read_step should succeed")
}

fn colormap_len(shape: &[Solid]) -> usize {
	shape.iter().map(|s| s.colormap().len()).sum()
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn write_colored(shape: &[Solid], path: &str) {
	fs::create_dir_all("out").unwrap();
	let mut buf = Vec::new();
	cadrum::Solid::write_step(shape, &mut buf).expect("write_step should succeed");
	fs::write(path, &buf).expect("should write output file");
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Reading colored_box.step should yield at least 6 colored faces.
#[test]
fn read_colored_step_populates_colormap() {
	let shape = read_colored_box();
	assert!(colormap_len(&shape) >= 6, "expected at least 6 colored faces, got {}", colormap_len(&shape));
	// Every entry in the colormap should correspond to an actual face.
	let face_ids: std::collections::HashSet<u64> = shape.iter().flat_map(|s| s.iter_face()).map(|f| f.id()).collect();
	for solid in &shape {
		for id in solid.colormap().keys() {
			assert!(face_ids.contains(id), "colormap key {:?} does not match any face in the shape", id);
		}
	}
}

/// Write the colored shape to STEP and read it back — colormap should be
/// non-empty after the round-trip (XDE preserves face colors).
#[test]
fn write_then_read_preserves_colors() {
	let original = read_colored_box();
	let path = "out/colored_box_roundtrip.step";
	write_colored(&original, path);

	let data = fs::read(path).unwrap();
	let reloaded = cadrum::Solid::read_step(&mut data.as_slice()).expect("re-read should succeed");

	assert!(colormap_len(&reloaded) >= 6, "re-read shape should have at least 6 colored faces, got {}", colormap_len(&reloaded));
}

/// Cut the colored box with a half-space (z > 0) and write the result.
/// The 5 surviving original faces should keep their colors; the new cut face
/// has no color (it comes from the tool which has an empty colormap).
#[test]
fn intersect_colored_step_preserves_colors() {
	let cube = read_colored_box();
	let original_colors = colormap_len(&cube);

	// Half-space keeping z > 0 side.
	let half = [Solid::half_space(DVec3::ZERO, DVec3::Z)];
	let solids: Vec<Solid> = (&cube[0] * &half[0]).build_vec().expect("intersect should succeed");

	// At least one face should have kept its color.
	assert!(colormap_len(&solids) >= 1, "at least one face should keep its color after intersect, got 0");
	assert!(colormap_len(&solids) < original_colors + 1, "intersect should not invent new colors");

	write_colored(&solids, "out/colored_box_intersect.step");
}

/// Translate the colored box and verify colors survive the move.
#[test]
fn translate_colored_step_preserves_colors() {
	let shape = read_colored_box();
	let original_len = colormap_len(&shape);

	let moved: Vec<Solid> = shape.into_iter().map(|s| s.translate(DVec3::new(100.0, 0.0, 0.0))).collect();

	assert_eq!(colormap_len(&moved), original_len, "translate should preserve all {} face colors", original_len);
	write_colored(&moved, "out/colored_box_translated.step");
}

/// clean() on the read shape should not lose colors.
#[test]
fn clean_colored_step_preserves_colors() {
	let shape = read_colored_box();
	let original_len = colormap_len(&shape);

	let cleaned: Vec<Solid> = shape.iter().map(|s| s.clean().expect("clean should succeed")).collect();

	assert_eq!(colormap_len(&cleaned), original_len, "clean should preserve all {} face colors", original_len);
	write_colored(&cleaned, "out/colored_box_cleaned.step");
}

/// #129: multi-color STEP from SolveSpace lands as Compound{Shell×3} with
/// no Solid because adjacent faces don't share EDGE_CURVE entities. The
/// Sewing post-process should recover 1 Solid AND preserve per-face colors.
///
/// Writes the recovered shape to STEP / STL (RGB555 attribute bytes, MeshLab
/// readable) / SVG (DVec3::ONE viewpoint) for visual verification.
/// Blue, light green, red faces should be preserved.
#[test]
fn multicolor_solvespace_step_recovers_solid_with_colors() {
	let data = fs::read("steps/multicolor_solvespace.step").expect("fixture should exist");
	let solids = cadrum::Solid::read_step(&mut data.as_slice()).expect("read_step should succeed");

	assert_eq!(solids.len(), 1, "expected 1 recovered solid, got {}", solids.len());
	assert!(solids[0].volume() > 0.0, "recovered solid should have non-zero volume");
	assert!(colormap_len(&solids) > 0, "expected color info to survive sewing, got 0 colored faces");

	write_colored(&solids, "out/multicolor_solvespace_recovered.step");

	let mut stl = std::fs::File::create("out/multicolor_solvespace_recovered.stl").expect("stl file");
	cadrum::Solid::mesh(&solids, cadrum::Tessellation { deflection_linear: 0.1, relative_linear: false, ..Default::default() }).and_then(|m| m.write_stl(&mut stl)).expect("stl write should succeed");

	let mut svg = std::fs::File::create("out/multicolor_solvespace_recovered.svg").expect("svg file");
	cadrum::Solid::mesh(&solids, cadrum::Tessellation { deflection_linear: 0.1, relative_linear: false, ..Default::default() }).and_then(|m| m.scene(cadrum::SceneOption { shading: true, ..Default::default() }).write_svg(&mut svg)).expect("svg write should succeed");
}

// ── solid-level colour (STYLED_ITEM → MANIFOLD_SOLID_BREP) ────────────────────

/// `LAMBDA360-BOX-*.step` is a real commercial-CAD export whose single
/// `STYLED_ITEM` targets `#14 = MANIFOLD_SOLID_BREP`, not an `ADVANCED_FACE`.
/// cadrum used to skip every non-FACE label and silently drop the colour, so
/// the box rendered in the default grey. The bug hid for so long because the
/// file's own colour is `#a0a0a0` ("鋼 - サテン") — grey, like the fallback.
const LAMBDA360_STEP: &str = "steps/LAMBDA360-BOX-d6cb2eb2d6e0d802095ea1eda691cf9a3e9bf3394301a0d148f53e55f0f97951.step";

#[test]
fn solid_level_styled_item_is_read() {
	let data = fs::read(LAMBDA360_STEP).expect("fixture should exist");
	let solids = cadrum::Solid::read_step(&mut data.as_slice()).expect("read_step should succeed");
	assert_eq!(solids.len(), 1, "expected 1 solid");

	let c = solids[0].color_solid().expect("solid-level colour must survive the read");
	// The file says COLOUR_RGB('鋼 - サテン', 0.627450980392157, ×3). OCCT reads STEP
	// colours as sRGB and stores Quantity_Color in linear RGB, so what comes back is
	// the linear form. That conversion is not new here — face colours have always
	// gone through it.
	let srgb = 0.627_450_98_f32;
	let linear = ((srgb + 0.055) / 1.055).powf(2.4);
	for v in [c.r, c.g, c.b] {
		assert!((v - linear).abs() < 1e-5, "expected {linear} (linear of sRGB {srgb}), got {v}");
	}
	assert!(solids[0].colormap().is_empty(), "a solid-level style must not be expanded onto faces");
}

/// The solid colour must reach the renderers, which speak only face colours —
/// `io::mesh` resolves it onto every face.
#[test]
fn solid_level_color_reaches_the_mesh() {
	let data = fs::read(LAMBDA360_STEP).expect("fixture should exist");
	let solids = cadrum::Solid::read_step(&mut data.as_slice()).expect("read_step should succeed");
	let mesh = cadrum::Solid::mesh(&solids, Default::default()).expect("mesh should succeed");

	assert!(!mesh.colormap.is_empty(), "solid colour must be resolved onto faces for rendering");
	for fid in &mesh.face_ids {
		assert!(mesh.colormap.contains_key(fid), "every meshed face should carry the solid's colour");
	}
}

/// Round-trip: a solid colour goes out as ONE styled item on the solid, not N on
/// its faces, and comes back as a solid colour.
#[test]
fn solid_level_color_round_trips() {
	let red = cadrum::Color::from_str("#ff0000").unwrap();
	let src = Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color(red);
	assert_eq!(src.color_solid(), Some(red));
	assert!(src.colormap().is_empty(), "color() paints the solid, not its faces");

	let mut buf = Vec::new();
	cadrum::Solid::write_step(&[src], &mut buf).expect("write_step should succeed");
	let step = String::from_utf8_lossy(&buf);
	assert_eq!(step.matches("STYLED_ITEM").count(), 1, "one styled item, not one per face");
	assert!(step.contains("MANIFOLD_SOLID_BREP"), "the styled item must target the solid");

	let back = cadrum::Solid::read_step(&mut buf.as_slice()).expect("read_step should succeed");
	assert_eq!(back.len(), 1);
	assert_eq!(back[0].color_solid(), Some(red), "solid colour must round-trip");
	assert!(back[0].colormap().is_empty(), "and must not have leaked onto the faces");
}

/// Boolean results take the operands' colour only when the ones that have a
/// colour agree — a union of two different colours has no single answer.
#[test]
fn boolean_carries_solid_color_only_when_operands_agree() {
	let red = cadrum::Color::from_str("#ff0000").unwrap();
	let blue = cadrum::Color::from_str("#0000ff").unwrap();
	let at = |x: f64| Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).translate(DVec3::X * x);

	let same: Vec<Solid> = (&at(0.0).color(red) + &at(5.0).color(red)).build_vec().unwrap();
	assert_eq!(same[0].color_solid(), Some(red), "agreeing operands carry their colour");

	let mixed: Vec<Solid> = (&at(0.0).color(red) + &at(5.0).color(blue)).build_vec().unwrap();
	assert_eq!(mixed[0].color_solid(), None, "a mixture has no single colour");

	// A cutting tool usually has no colour of its own; it must not erase the part's.
	let cut: Vec<Solid> = (&at(0.0).color(red) - &at(5.0)).build_vec().unwrap();
	assert_eq!(cut[0].color_solid(), Some(red), "an uncoloured operand is ignored");
}
