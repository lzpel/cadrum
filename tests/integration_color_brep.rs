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

/// Colormap keys are TShape addresses, which a re-read invents afresh, so a round-trip
/// is compared by each face's effective colour (its own, else its solid's) in order.
fn effective_colors(shape: &[Solid]) -> Vec<Option<Color>> {
	shape.iter().flat_map(|s| s.iter_face().map(move |f| s.colormap().get(&f.id()).or(s.colormap().get(&s.id())).copied())).collect()
}

fn solid_colors(shape: &[Solid]) -> Vec<Option<Color>> {
	shape.iter().map(|s| s.colormap().get(&s.id()).copied()).collect()
}

fn roundtrip(shape: &[Solid]) -> Vec<Solid> {
	let mut buf = Vec::new();
	cadrum::Solid::write_brep(shape, &mut buf).expect("write_brep should succeed");
	cadrum::Solid::read_brep(&mut buf.as_slice()).expect("read_brep should succeed")
}

/// Doubles as the regression test for `BinTools`' backward references: a boolean
/// result has shared sub-shapes, so reading it back exercises the reader's seeking.
#[test]
fn roundtrip_after_boolean() {
	let cube = read_colored_box();
	let half = [Solid::half_space(DVec3::ZERO, DVec3::NEG_Z)];
	let solids: Vec<Solid> = (&cube[0] * &half[0]).build_vec().expect("intersect should succeed");
	assert!(solids.iter().any(|s| !s.colormap().is_empty()), "at least one color should survive intersect");

	let reloaded = roundtrip(&solids);
	assert_eq!(effective_colors(&reloaded), effective_colors(&solids), "every face should keep its effective colour after boolean + round-trip");
}

/// Solid colours and face colours share one index space, solids first. The cube sits
/// second so the solid count shifts every face index — an off-by-one shows up here.
#[test]
fn solid_and_face_colors_share_the_trailer() {
	let blue = Color::from_str("#0000ff").expect("valid hex");
	let mut src = read_colored_box();
	src.push(Solid::cube(DVec3::splat(100.0), DVec3::splat(110.0)).color(blue));

	let reloaded = roundtrip(&src);
	assert_eq!(reloaded.len(), src.len(), "solid count");
	assert_eq!(effective_colors(&reloaded), effective_colors(&src), "every face keeps its effective colour");
	assert_eq!(solid_colors(&reloaded), solid_colors(&src), "every solid keeps its own colour");
	let cube = reloaded.last().expect("cube");
	assert_eq!(cube.colormap().get(&cube.id()), Some(&blue), "the cube's solid colour survives alongside the face colours");
}

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

	assert_eq!(&tinted[..plain.len()], &plain[..], "an uncoloured shape gets no trailer, and a coloured one leaves the payload untouched");
	assert_eq!(&tinted[plain.len()..plain.len() + 4], b"CDCL", "the magic should sit at the payload's end");
	assert_eq!(tinted.len(), plain.len() + 8 + 16, "magic + count + one entry");

	// The count self-delimits, so a section appended after ours must not hide the colour.
	tinted.extend_from_slice(&[0xAB; 32]);
	let back = cadrum::Solid::read_brep(&mut tinted.as_slice()).expect("read_brep should succeed");
	assert_eq!(solid_colors(&back), [Some(red)], "the solid colour survives bytes appended past the trailer");
}

/// An empty reader is a read failure, not a panic.
#[test]
fn empty_input_fails() {
	assert!(cadrum::Solid::read_brep(&mut [].as_slice()).is_err(), "empty input should fail to parse");
}
