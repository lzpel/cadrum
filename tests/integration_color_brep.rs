//! Integration tests for BRep + color trailer format.

#![cfg(feature = "color")]

use cadrum::{Color, Solid};
use glam::DVec3;
use std::fs;

const COLORED_BOX_STEP: &str = "steps/colored_box.step";

fn read_colored_box() -> Vec<Solid> {
	let data = fs::read(COLORED_BOX_STEP).expect("steps/colored_box.step should exist");
	cadrum::Solid::read_step(&mut data.as_slice()).expect("read_step should succeed")
}

fn colormap_len(shape: &[Solid]) -> usize {
	shape.iter().map(|s| s.colormap().len()).sum()
}

/// Colour each face actually renders in: its own, else its solid's.
///
/// The BRep trailer is keyed by face index and has nowhere to record "this is the
/// solid's colour", so a solid-level colour is flattened onto its faces on write.
/// Appearance is preserved exactly; the count of colormap entries is not, which is
/// why the boolean round-trip below compares effective colours rather than counts.
fn effective_colors(shape: &[Solid]) -> Vec<Option<Color>> {
	shape.iter().flat_map(|s| s.iter_face().map(move |f| s.colormap().get(&f.id()).copied().or(s.color_solid()))).collect()
}

fn roundtrip_bin(shape: &[Solid]) -> Vec<Solid> {
	let mut buf = Vec::new();
	cadrum::Solid::write_brep_binary(shape, &mut buf).expect("write_brep_binary should succeed");
	cadrum::Solid::read_brep_binary(&mut buf.as_slice()).expect("read_brep_binary should succeed")
}

fn roundtrip_text(shape: &[Solid]) -> Vec<Solid> {
	let mut buf = Vec::new();
	cadrum::Solid::write_brep_text(shape, &mut buf).expect("write_brep_text should succeed");
	cadrum::Solid::read_brep_text(&mut buf.as_slice()).expect("read_brep_text should succeed")
}

// ── binary tests ─────────────────────────────────────────────────────────────

/// Round-trip (binary) preserves the number of colors and the RGB values.
#[test]
fn bin_write_then_read_preserves_colors() {
	let original = read_colored_box();
	let reloaded = roundtrip_bin(&original);

	assert_eq!(colormap_len(&reloaded), colormap_len(&original), "color count should be preserved (binary)");

	let original_colors: Vec<Color> = original.iter().flat_map(|s| s.iter_face()).filter_map(|f| original.iter().find_map(|s| s.colormap().get(&f.id()).copied())).collect();
	let reloaded_colors: Vec<Color> = reloaded.iter().flat_map(|s| s.iter_face()).filter_map(|f| reloaded.iter().find_map(|s| s.colormap().get(&f.id()).copied())).collect();

	assert_eq!(original_colors, reloaded_colors, "RGB values should be identical (binary)");
}

/// A shape with an empty colormap round-trips without error (binary).
#[test]
fn bin_colorless_shape_roundtrip() {
	let shape = [Solid::cube(DVec3::ZERO, DVec3::ONE)];
	let reloaded = roundtrip_bin(&shape);
	assert_eq!(colormap_len(&reloaded), 0);
}

/// Round-trip (binary) after a boolean operation preserves surviving colors.
#[test]
fn bin_roundtrip_after_boolean() {
	let cube = read_colored_box();
	let half = [Solid::half_space(DVec3::ZERO, DVec3::NEG_Z)];
	let solids: Vec<Solid> = (&cube[0] * &half[0]).build_vec().expect("intersect should succeed");

	assert!(colormap_len(&solids) >= 1, "at least one color should survive intersect");

	let reloaded = roundtrip_bin(&solids);
	// Counts are not comparable: colored_box.step styles its solid *and* 11 of its
	// faces, and the trailer can only speak faces, so the solid's colour lands on
	// every face on write. What must hold is that no face changes how it looks.
	assert_eq!(effective_colors(&reloaded), effective_colors(&solids), "every face should keep its effective colour (binary)");
}

// ── text tests ───────────────────────────────────────────────────────────────

/// Round-trip (text) preserves the number of colors and the RGB values.
#[test]
fn text_write_then_read_preserves_colors() {
	let original = read_colored_box();
	let reloaded = roundtrip_text(&original);

	assert_eq!(colormap_len(&reloaded), colormap_len(&original), "color count should be preserved (text)");

	let original_colors: Vec<Color> = original.iter().flat_map(|s| s.iter_face()).filter_map(|f| original.iter().find_map(|s| s.colormap().get(&f.id()).copied())).collect();
	let reloaded_colors: Vec<Color> = reloaded.iter().flat_map(|s| s.iter_face()).filter_map(|f| reloaded.iter().find_map(|s| s.colormap().get(&f.id()).copied())).collect();

	assert_eq!(original_colors, reloaded_colors, "RGB values should be identical (text)");
}

/// A shape with an empty colormap round-trips without error (text).
#[test]
fn text_colorless_shape_roundtrip() {
	let shape = [Solid::cube(DVec3::ZERO, DVec3::ONE)];
	let reloaded = roundtrip_text(&shape);
	assert_eq!(colormap_len(&reloaded), 0);
}
