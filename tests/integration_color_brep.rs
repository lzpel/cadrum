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

/// Colormap keys are TShape addresses, which a re-read invents afresh, so a round-trip
/// is compared by each face's effective colour (its own, else its solid's) in order.
fn effective_colors(shape: &[Solid]) -> Vec<Option<Color>> {
	shape.iter().flat_map(|s| s.iter_face().map(move |f| s.colormap().get(&f.id()).or(s.colormap().get(&s.id())).copied())).collect()
}

fn roundtrip(shape: &[Solid]) -> Vec<Solid> {
	let mut buf = Vec::new();
	cadrum::Solid::write_brep(shape, &mut buf).expect("write_brep should succeed");
	cadrum::Solid::read_brep(&mut buf.as_slice()).expect("read_brep should succeed")
}

/// Round-trip preserves how every face looks, and the RGB values.
#[test]
fn write_then_read_preserves_colors() {
	let original = read_colored_box();
	let reloaded = roundtrip(&original);

	assert_eq!(effective_colors(&reloaded), effective_colors(&original), "every face should keep its effective colour");

	let original_colors: Vec<Color> = original.iter().flat_map(|s| s.iter_face()).filter_map(|f| original.iter().find_map(|s| s.colormap().get(&f.id()).copied())).collect();
	let reloaded_colors: Vec<Color> = reloaded.iter().flat_map(|s| s.iter_face()).filter_map(|f| reloaded.iter().find_map(|s| s.colormap().get(&f.id()).copied())).collect();

	assert_eq!(original_colors, reloaded_colors, "RGB values should be identical");
}

/// A shape with an empty colormap round-trips without error.
#[test]
fn colorless_shape_roundtrip() {
	let shape = [Solid::cube(DVec3::ZERO, DVec3::ONE)];
	let reloaded = roundtrip(&shape);
	assert_eq!(colormap_len(&reloaded), 0);
}

/// Doubles as the regression test for `BinTools`' backward references: a boolean
/// result has shared sub-shapes, so reading it back exercises the reader's seeking.
#[test]
fn roundtrip_after_boolean() {
	let cube = read_colored_box();
	let half = [Solid::half_space(DVec3::ZERO, DVec3::NEG_Z)];
	let solids: Vec<Solid> = (&cube[0] * &half[0]).build_vec().expect("intersect should succeed");

	assert!(colormap_len(&solids) >= 1, "at least one color should survive intersect");

	let reloaded = roundtrip(&solids);
	assert_eq!(effective_colors(&reloaded), effective_colors(&solids), "every face should keep its effective colour after boolean + round-trip");
}

// ── trailer placement ────────────────────────────────────────────────────────

/// The design rests on `read_brep_stream` reporting the payload's end to the byte —
/// the reader looks for the magic there and nowhere else. This is what pins it.
#[test]
fn trailer_begins_where_the_payload_ends() {
	let red = Color::from_str("#ff0000").expect("valid hex");
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);

	let mut plain = Vec::new();
	cadrum::Solid::write_brep(&[cube.clone()], &mut plain).expect("write_brep should succeed");
	let mut tinted = Vec::new();
	cadrum::Solid::write_brep(&[cube.color(red)], &mut tinted).expect("write_brep should succeed");

	assert_eq!(&tinted[..plain.len()], &plain[..], "same geometry should give the same payload bytes");
	assert_eq!(&tinted[plain.len()..plain.len() + 4], b"CDCL", "the magic should sit at the payload's end");
	assert_eq!(tinted.len(), plain.len() + 8 + 16, "magic + count + one entry");
}

/// An empty reader is a read failure, not a panic.
#[test]
fn empty_input_fails() {
	assert!(cadrum::Solid::read_brep(&mut [].as_slice()).is_err(), "empty input should fail to parse");
}

// ── solid-level colour ───────────────────────────────────────────────────────

#[test]
fn solid_level_color_round_trips() {
	let red = Color::from_str("#ff0000").expect("valid hex");
	let src = [Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color(red)];
	assert_eq!(src[0].colormap().len(), 1, "color() paints the solid, not each of its faces");

	let reloaded = roundtrip(&src);
	assert_eq!(reloaded.len(), 1);
	assert_eq!(reloaded[0].colormap().get(&reloaded[0].id()), Some(&red), "the solid colour must round-trip");
	assert_eq!(reloaded[0].colormap().len(), 1, "one entry, not one per face");
	assert_eq!(reloaded[0].iter_face().filter(|f| reloaded[0].colormap().contains_key(&f.id())).count(), 0, "and must not have leaked onto the faces");
}

/// The cube sits second so the solid count shifts every face index — an off-by-one
/// at the solid/face boundary of the index space shows up here.
#[test]
fn solid_and_face_colors_share_the_trailer() {
	let blue = Color::from_str("#0000ff").expect("valid hex");
	let mut src = read_colored_box();
	src.push(Solid::cube(DVec3::splat(100.0), DVec3::splat(110.0)).color(blue));

	let reloaded = roundtrip(&src);
	assert_eq!(reloaded.len(), src.len(), "solid count");
	assert_eq!(effective_colors(&reloaded), effective_colors(&src), "every face keeps its effective colour");
	let solid_colors = |shape: &[Solid]| -> Vec<Option<Color>> { shape.iter().map(|s| s.colormap().get(&s.id()).copied()).collect() };
	assert_eq!(solid_colors(&reloaded), solid_colors(&src), "every solid keeps its own colour");
	let cube = reloaded.last().expect("cube");
	assert_eq!(cube.colormap().get(&cube.id()), Some(&blue), "the cube's solid colour survives alongside the face colours");
}
