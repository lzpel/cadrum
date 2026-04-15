//! Sweep showcase: M2 screw (helix spine) + U-shaped pipe (line+arc+line spine)
//! + twisted ribbon (`Auxiliary` aux-spine mode).
//!
//! `ProfileOrient` controls how the profile is oriented as it travels along the spine:
//!
//! - `Fixed`: profile is parallel-transported without rotating. Cross-sections
//!   stay parallel to the starting orientation. Suited for straight extrusions;
//!   on a curved spine the profile drifts off the tangent and the result breaks.
//! - `Torsion`: profile follows the spine's principal normal (raw Frenet–Serret
//!   frame). Suited for constant-curvature/torsion curves like helices and for
//!   3D free curves where the natural twist should carry into the profile.
//!   Fails near inflection points where the principal normal flips.
//! - `Up(axis)`: profile keeps `axis` as its binormal — at every point the
//!   profile is rotated around the tangent so one in-plane axis stays in the
//!   tangent–`axis` plane. Suited for roads/rails/pipes that must preserve a
//!   gravity direction. On a helix, `Up(helix_axis)` is equivalent to `Torsion`.
//!   Fails when the tangent becomes parallel to `axis`.
//! - `Auxiliary(aux_spine)`: profile's tracked axis points from the main spine
//!   toward a parallel auxiliary spine. Arbitrary twist control — e.g. a
//!   helical `aux_spine` on a straight `spine` produces a twisted ribbon.

use cadrum::{Compound, Edge, Error, ProfileOrient, Solid, Transform};
use glam::DVec3;

// ==================== Component 1: M2 ISO screw ====================

fn build_m2_screw() -> Result<Vec<Solid>, Error> {
	let r = 1.0;
	let h_pitch = 0.4;
	let h_thread = 6.0;
	let r_head = 1.75;
	let h_head = 1.3;
	// ISO M thread fundamental triangle height: H = √3/2 · P (sharp 60° triangle).
	let r_delta = 3f64.sqrt() / 2.0 * h_pitch;

	// Helix spine at the root radius. x_ref=+X anchors the start at (r-r_delta, 0, 0).
	let helix = Edge::helix(r - r_delta, h_pitch, h_thread, DVec3::Z, DVec3::X)?;

	// Closed triangular profile in local coords (x: radial, y: along helix tangent).
	let profile = Edge::polygon(&[DVec3::new(0.0, -h_pitch / 2.0, 0.0), DVec3::new(r_delta, 0.0, 0.0), DVec3::new(0.0, h_pitch / 2.0, 0.0)])?;

	// Align profile +Z with the helix start tangent, then translate to the start point.
	let profile = profile.align_z(helix.start_tangent(), helix.start_point()).translate(helix.start_point());

	// Sweep along the helix. Up(+Z) ≡ Torsion for a helix and yields a correct thread.
	let thread = Solid::sweep(&profile, &[helix], ProfileOrient::Up(DVec3::Z))?;

	// Reconstruct the ISO 68-1 basic profile (trapezoid) from the sharp triangle:
	//   union(shaft) fills the bottom H/4 → P/4-wide flat at the root
	//   intersect(crest) trims the top H/8 → P/8-wide flat at the crest
	let shaft = Solid::cylinder(r - r_delta * 6.0 / 8.0, DVec3::Z, h_thread);
	let crest = Solid::cylinder(r - r_delta / 8.0, DVec3::Z, h_thread);
	let thread_shaft = thread.union([&shaft])?.intersect([&crest])?;

	// Stack the flat head on top.
	let head = Solid::cylinder(r_head, DVec3::Z, h_head).translate(DVec3::Z * h_thread);
	thread_shaft.union([&head])
}

// ==================== Component 2: U-shaped pipe ====================

fn build_u_pipe() -> Result<Vec<Solid>, Error> {
	let pipe_radius = 0.4;
	let leg_length = 6.0;
	let gap = 3.0;
	let bend_radius = gap / 2.0;

	// U-shaped path in the XZ plane: A↑B ⌒ C↓D
	let a = DVec3::ZERO;
	let b = DVec3::new(0.0, 0.0, leg_length);
	let arc_mid = DVec3::new(bend_radius, 0.0, leg_length + bend_radius);
	let c = DVec3::new(gap, 0.0, leg_length);
	let d = DVec3::new(gap, 0.0, 0.0);

	// Spine wire: line → semicircle → line.
	let up_leg = Edge::line(a, b)?;
	let bend = Edge::arc_3pts(b, arc_mid, c)?;
	let down_leg = Edge::line(c, d)?;

	// Circular profile in XY (normal +Z) — already aligned with the spine start tangent.
	let profile = Edge::circle(pipe_radius, DVec3::Z)?;

	// Up(+Y) fixes the binormal to the path-plane normal, avoiding Frenet
	// degeneracy on the straight segments.
	let pipe = Solid::sweep(&[profile], &[up_leg, bend, down_leg], ProfileOrient::Up(DVec3::Y))?;
	Ok(vec![pipe])
}

// ==================== Component 3: Auxiliary-spine twisted ribbon ====================

// 直線 spine を `Auxiliary(&[helix])` で掃引すると、各点で profile の tracked 軸が
// 対応するヘリックス点を向くように回転される。pitch=h のヘリックスは [0, h] の
// あいだにちょうど 360° 一周するので、平たい長方形 profile は 1 回捻れた
// リボンになる — `Fixed` や `Torsion` だと直線 spine では profile は全く
// 回転しないので、ねじれが見えれば Auxiliary が効いている証拠。
fn build_twisted_ribbon() -> Result<Solid, Error> {
	let h = 8.0;
	let aux_r = 3.0;

	let spine = Edge::line(DVec3::ZERO, DVec3::Z * h)?;
	let aux = Edge::helix(aux_r, h, h, DVec3::Z, DVec3::X)?;

	// 平たい長方形 (10:1 アスペクト) — 円や正方形ではねじれが見えない。
	let profile = Edge::polygon(&[DVec3::new(-2.0, -0.2, 0.0), DVec3::new(2.0, -0.2, 0.0), DVec3::new(2.0, 0.2, 0.0), DVec3::new(-2.0, 0.2, 0.0)])?;

	Solid::sweep(&profile, &[spine], ProfileOrient::Auxiliary(&[aux]))
}

// ==================== main: side-by-side layout ====================

fn main() {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	// Screw at origin, U-pipe at +x_offset. Ribbon at 2.5·x_offset so the
	// ribbon→U-pipe gap roughly matches the U-pipe→screw gap (screw visual
	// center ≈ 0, U-pipe visual center ≈ x_offset + gap/2 ≈ 7.5, ribbon
	// visual center = ribbon_x ≈ 15).
	let x_offset = 6.0;
	let ribbon_x = x_offset * 2.5;

	let mut all: Vec<Solid> = Vec::new();

	match build_m2_screw() {
		Ok(screw) => {
			all.extend(screw.color("red"));
			println!("✓ screw built (red, centered at origin)");
		}
		Err(e) => eprintln!("✗ screw failed: {e}"),
	}

	match build_u_pipe() {
		Ok(pipe) => {
			let placed: Vec<Solid> = pipe.translate(DVec3::X * x_offset).color("blue");
			all.extend(placed);
			println!("✓ U-pipe built (blue, offset x={x_offset})");
		}
		Err(e) => eprintln!("✗ U-pipe failed: {e}"),
	}

	match build_twisted_ribbon() {
		Ok(ribbon) => {
			all.push(ribbon.translate(DVec3::X * ribbon_x).color("green"));
			println!("✓ twisted ribbon built (green, offset x={ribbon_x})");
		}
		Err(e) => eprintln!("✗ twisted ribbon failed: {e}"),
	}

	if all.is_empty() {
		eprintln!("nothing to write");
		return;
	}

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::write_step(&all, &mut f).expect("failed to write STEP");
	let mut f_svg = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	// Helical threads have dense hidden lines that clutter the SVG; disable them.
	cadrum::mesh(&all, 0.5).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, -1.0), false, false, &mut f_svg)).expect("failed to write SVG");
	println!("wrote {example_name}.step / {example_name}.svg ({} solids)", all.len());
}
