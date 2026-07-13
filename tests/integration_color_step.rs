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
	// Every colormap key is either a face of the shape or a solid's own id — the
	// two levels STEP itself has. colored_box.step exercises both: 11 styled_items
	// target advanced_faces and a 12th targets the manifold_solid_brep.
	let ids: std::collections::HashSet<u64> = shape.iter().flat_map(|s| s.iter_face().map(|f| f.id()).chain(std::iter::once(s.id()))).collect();
	for solid in &shape {
		for id in solid.colormap().keys() {
			assert!(ids.contains(id), "colormap key {:?} is neither a face nor a solid of the shape", id);
		}
	}
	assert!(shape.iter().any(|s| s.color_solid().is_some()), "colored_box.step styles its manifold_solid_brep too; that colour must not be dropped");
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

/// A real commercial-CAD export (Autodesk ATF / ST-DEVELOPER) whose single
/// `STYLED_ITEM` targets `#14 = MANIFOLD_SOLID_BREP`, not an `ADVANCED_FACE`.
/// cadrum used to skip every non-FACE label and drop the colour, so this box
/// rendered in the default grey. The bug hid because the file's own colour is
/// `#a0a0a0` ("鋼 - サテン") — grey, like the fallback — and because the fixture
/// had never been wired into a test.
const LAMBDA360_STEP: &str = "steps/LAMBDA360-BOX-d6cb2eb2d6e0d802095ea1eda691cf9a3e9bf3394301a0d148f53e55f0f97951.step";

fn read_lambda360() -> Vec<Solid> {
	let data = fs::read(LAMBDA360_STEP).expect("fixture should exist");
	cadrum::Solid::read_step(&mut data.as_slice()).expect("read_step should succeed")
}

#[test]
fn solid_level_styled_item_is_read() {
	let solids = read_lambda360();
	assert_eq!(solids.len(), 1, "expected 1 solid");

	let c = solids[0].color_solid().expect("the solid-level colour must survive the read");
	// The file says COLOUR_RGB('鋼 - サテン', 0.627450980392157, ×3). OCCT reads STEP
	// colours as sRGB and stores Quantity_Color linear, so what comes back is the
	// linear form. Not new here — face colours have always gone through it.
	let srgb = 0.627_450_98_f32;
	let linear = ((srgb + 0.055) / 1.055).powf(2.4);
	for v in [c.r, c.g, c.b] {
		assert!((v - linear).abs() < 1e-5, "expected {linear} (linear of sRGB {srgb}), got {v}");
	}
	// The colour belongs to the solid, and only to the solid: expanding it onto the
	// faces at read time would write back N styled items instead of the one.
	let faces_colored = solids[0].iter_face().filter(|f| solids[0].colormap().contains_key(&f.id())).count();
	assert_eq!(faces_colored, 0, "a solid-level style must not be expanded onto faces");
}

/// The renderers speak only face colours, so `io::mesh` resolves the solid's onto
/// every face. This is what makes the box stop rendering grey.
#[test]
fn solid_level_color_reaches_the_mesh() {
	let solids = read_lambda360();
	let mesh = cadrum::Solid::mesh(&solids, Default::default()).expect("mesh should succeed");

	assert!(!mesh.colormap.is_empty(), "the solid colour must be resolved onto faces for rendering");
	for fid in &mesh.face_ids {
		assert!(mesh.colormap.contains_key(fid), "every meshed face should carry the solid's colour");
	}
}

/// A solid colour goes out as ONE styled item on the solid, not N on its faces,
/// and comes back as a solid colour.
#[test]
fn solid_level_color_round_trips() {
	let red = cadrum::Color::from_str("#ff0000").expect("valid hex");
	let src = Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color(red);
	assert_eq!(src.color_solid(), Some(red));
	assert_eq!(src.colormap().len(), 1, "color() paints the solid, not each of its faces");

	let mut buf = Vec::new();
	cadrum::Solid::write_step(&[src], &mut buf).expect("write_step should succeed");
	let step = String::from_utf8_lossy(&buf);
	assert_eq!(step.matches("STYLED_ITEM").count(), 1, "one styled item, not one per face");
	assert!(step.contains("MANIFOLD_SOLID_BREP"), "the styled item must target the solid");

	let back = cadrum::Solid::read_step(&mut buf.as_slice()).expect("read_step should succeed");
	assert_eq!(back.len(), 1);
	assert_eq!(back[0].color_solid(), Some(red), "the solid colour must round-trip");
	assert_eq!(back[0].iter_face().filter(|f| back[0].colormap().contains_key(&f.id())).count(), 0, "and must not have leaked onto the faces");
}

/// A boolean result takes the operands' colour only when the ones that have a
/// colour agree: the volume of `a ∪ b` is a mixture, not a descendant of one
/// operand, so unlike a face colour there is no history to carry it.
#[test]
fn boolean_carries_solid_color_only_when_operands_agree() {
	let red = cadrum::Color::from_str("#ff0000").expect("valid hex");
	let blue = cadrum::Color::from_str("#0000ff").expect("valid hex");
	let at = |x: f64| Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).translate(DVec3::X * x);

	let same: Vec<Solid> = (&at(0.0).color(red) + &at(5.0).color(red)).build_vec().expect("union should succeed");
	assert_eq!(same[0].color_solid(), Some(red), "agreeing operands carry their colour");

	let mixed: Vec<Solid> = (&at(0.0).color(red) + &at(5.0).color(blue)).build_vec().expect("union should succeed");
	assert_eq!(mixed[0].color_solid(), None, "a mixture of two colours has no single answer");

	// A cutting tool usually has no colour of its own; it must not erase the part's.
	let cut: Vec<Solid> = (&at(0.0).color(red) - &at(5.0)).build_vec().expect("cut should succeed");
	assert_eq!(cut[0].color_solid(), Some(red), "an uncoloured operand is ignored");
}

/// The solid colour is keyed by the solid's TShape id, which every topology-
/// rebuilding op changes. Nothing in the type system forces those ops to carry it
/// across — this test does.
#[test]
fn single_source_ops_carry_the_solid_color() {
	let red = cadrum::Color::from_str("#ff0000").expect("valid hex");
	let cube = || Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color(red);

	// fillet/chamfer must be handed an edge of the very solid they operate on, so
	// each keeps the source alive for the length of its own call.
	let filleted = {
		let c = cube();
		let e = c.iter_edge().next().expect("cube has edges");
		c.fillet_edges(0.5, [e]).expect("fillet should succeed")
	};
	let chamfered = {
		let c = cube();
		let e = c.iter_edge().next().expect("cube has edges");
		c.chamfer_edges(0.5, [e]).expect("chamfer should succeed")
	};

	let cases: Vec<(&str, Solid)> = vec![("translate", cube().translate(DVec3::X * 5.0)), ("rotate", cube().rotate(DVec3::ZERO, DVec3::Z, 0.5)), ("scale", cube().scale(DVec3::ZERO, 2.0)), ("mirror", cube().mirror(DVec3::ZERO, DVec3::Z)), ("clone", cube().clone()), ("clean", cube().clean().expect("clean should succeed")), ("shell", cube().shell(-1.0, std::iter::empty::<&cadrum::Face>()).expect("shell should succeed")), ("fillet", filleted), ("chamfer", chamfered)];

	for (name, solid) in cases {
		assert_eq!(solid.color_solid(), Some(red), "{name} must carry the solid colour across");
	}
}
