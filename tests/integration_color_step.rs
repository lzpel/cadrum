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
	// colored_box.step has both levels: 11 styled_items on advanced_faces, a 12th on
	// the manifold_solid_brep.
	let ids: std::collections::HashSet<u64> = shape.iter().flat_map(|s| s.iter_face().map(|f| f.id()).chain(std::iter::once(s.id()))).collect();
	for solid in &shape {
		for id in solid.colormap().keys() {
			assert!(ids.contains(id), "colormap key {:?} is neither a face nor a solid of the shape", id);
		}
	}
	assert!(shape.iter().any(|s| s.colormap().contains_key(&s.id())), "colored_box.step styles its manifold_solid_brep too; that colour must not be dropped");
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

/// Ops that rebuild topology keep every colour. Intersect is the exception: it can
/// lose faces to the cut, but it must never invent a colour — the tool has none.
#[test]
fn ops_on_a_colored_step_preserve_colors() {
	let shape = read_colored_box();
	let original_len = colormap_len(&shape);

	let translated: Vec<Solid> = shape.iter().map(|s| s.clone().translate(DVec3::X * 100.0)).collect();
	let cleaned: Vec<Solid> = shape.iter().map(|s| s.clean().expect("clean should succeed")).collect();
	for (name, solids) in [("translated", translated), ("cleaned", cleaned)] {
		assert_eq!(colormap_len(&solids), original_len, "{name} should preserve all {original_len} colors");
		write_colored(&solids, &format!("out/colored_box_{name}.step"));
	}

	let half = [Solid::half_space(DVec3::ZERO, DVec3::Z)];
	let intersected: Vec<Solid> = (&shape[0] * &half[0]).build_vec().expect("intersect should succeed");
	assert!((1..=original_len).contains(&colormap_len(&intersected)), "intersect should keep some colors and invent none");
	write_colored(&intersected, "out/colored_box_intersect.step");
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

/// A commercial-CAD export (Autodesk ATF) whose single `STYLED_ITEM` targets
/// `#14 = MANIFOLD_SOLID_BREP`, not an `ADVANCED_FACE`.
const LAMBDA360_STEP: &str = "steps/LAMBDA360-BOX-d6cb2eb2d6e0d802095ea1eda691cf9a3e9bf3394301a0d148f53e55f0f97951.step";

/// The colour stays on the solid through the read — expanding it onto the faces would
/// write back N styled items — and `Mesh`, which has only a face level, expands it.
#[test]
fn solid_level_styled_item_is_read_and_reaches_the_mesh() {
	let data = fs::read(LAMBDA360_STEP).expect("fixture should exist");
	let solids = cadrum::Solid::read_step(&mut data.as_slice()).expect("read_step should succeed");
	assert_eq!(solids.len(), 1, "expected 1 solid");

	// The file says COLOUR_RGB('鋼 - サテン', 0.627450980392157, ×3), which OCCT reads as
	// sRGB and stores linear.
	let linear = ((0.627_450_98_f32 + 0.055) / 1.055).powf(2.4);
	let c = solids[0].colormap().get(&solids[0].id()).copied().expect("the solid-level colour must survive the read");
	assert!([c.r, c.g, c.b].iter().all(|v| (v - linear).abs() < 1e-5), "expected the linear form of the file's sRGB, got {c:?}");
	assert_eq!(solids[0].iter_face().filter(|f| solids[0].colormap().contains_key(&f.id())).count(), 0, "a solid-level style must not be expanded onto faces");

	let mesh = cadrum::Solid::mesh(&solids, Default::default()).expect("mesh should succeed");
	assert!(!mesh.face_ids.is_empty() && mesh.face_ids.iter().all(|f| mesh.colormap.contains_key(f)), "every meshed face should carry the solid's colour");
}

#[test]
fn solid_level_color_round_trips() {
	let red = cadrum::Color::from_str("#ff0000").expect("valid hex");
	let src = Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color(red);
	assert_eq!(src.colormap().get(&src.id()).copied(), Some(red));
	assert_eq!(src.colormap().len(), 1, "color() paints the solid, not each of its faces");

	let mut buf = Vec::new();
	cadrum::Solid::write_step(&[src], &mut buf).expect("write_step should succeed");
	let step = String::from_utf8_lossy(&buf);
	assert_eq!(step.matches("STYLED_ITEM").count(), 1, "one styled item, not one per face");
	assert!(step.contains("MANIFOLD_SOLID_BREP"), "the styled item must target the solid");

	let back = cadrum::Solid::read_step(&mut buf.as_slice()).expect("read_step should succeed");
	assert_eq!(back.len(), 1);
	assert_eq!(back[0].colormap().get(&back[0].id()).copied(), Some(red), "the solid colour must round-trip");
	assert_eq!(back[0].iter_face().filter(|f| back[0].colormap().contains_key(&f.id())).count(), 0, "and must not have leaked onto the faces");
}

/// Unlike a face colour, no history carries a solid's — the result volume descends
/// from no single operand. The left operand decides it, as in Fusion 360.
#[test]
fn boolean_carries_the_left_operand_solid_color() {
	let red = cadrum::Color::from_str("#ff0000").expect("valid hex");
	let blue = cadrum::Color::from_str("#0000ff").expect("valid hex");
	let at = |x: f64| Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).translate(DVec3::X * x);

	let mixed: Vec<Solid> = (&at(0.0).color(red) + &at(5.0).color(blue)).build_vec().expect("union should succeed");
	assert_eq!(mixed[0].colormap().get(&mixed[0].id()).copied(), Some(red), "the left operand's colour wins");

	// A cutting tool usually has no colour of its own; it must not erase the part's.
	let cut: Vec<Solid> = (&at(0.0).color(red) - &at(5.0)).build_vec().expect("cut should succeed");
	assert_eq!(cut[0].colormap().get(&cut[0].id()).copied(), Some(red), "an uncoloured right operand is ignored");

	let promoted: Vec<Solid> = (&at(0.0) + &at(5.0).color(red)).build_vec().expect("union should succeed");
	assert_eq!(promoted[0].colormap().get(&promoted[0].id()).copied(), None, "and an uncoloured left operand is not painted by the right");
}

/// Every topology-rebuilding op changes the solid's TShape id, and nothing in the type
/// system forces it to carry the colour across. One case per code path: `translate`
/// passes the colormap through, `clone` goes via `remap_colormap_by_order` (as do
/// scale/mirror), `fillet` via `remap_colormap` (as do shell/chamfer/clean).
#[test]
fn single_source_ops_carry_the_solid_color() {
	let red = cadrum::Color::from_str("#ff0000").expect("valid hex");
	let cube = || Solid::cube(DVec3::ZERO, DVec3::splat(10.0)).color(red);

	// fillet must be handed an edge of the very solid it consumes.
	let filleted = {
		let c = cube();
		let e = c.iter_edge().next().expect("cube has edges");
		c.fillet_edges(0.5, [e]).expect("fillet should succeed")
	};
	let cases: Vec<(&str, Solid)> = vec![("translate", cube().translate(DVec3::X * 5.0)), ("clone", cube().clone()), ("fillet", filleted)];

	for (name, solid) in cases {
		assert_eq!(solid.colormap().get(&solid.id()).copied(), Some(red), "{name} must carry the solid colour across");
	}
}
