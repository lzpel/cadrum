use cadrum::Solid;

#[test]
fn test_shell_cube_reduces_volume() {
	let cube = Solid::cube(10.0, 10.0, 10.0);
	let original_volume = cube.volume();

	let open = cube.iter_face().next().unwrap();
	let shelled = cube.shell(-0.5, [open]).expect("shell should succeed");

	assert!(shelled.volume() > 0.0, "shelled solid must have positive volume");
	assert!(shelled.volume() < original_volume, "shelling inward must reduce volume");
}

#[test]
fn test_shell_cube_preserves_solid_structure() {
	let cube = Solid::cube(10.0, 10.0, 10.0);
	let open = cube.iter_face().next().unwrap();
	let shelled = cube.shell(-0.5, [open]).expect("shell should succeed");
	assert!(shelled.volume() > 0.0, "shelled cube should produce a valid solid");
}

#[test]
fn test_shell_outward_produces_wall() {
	let cube = Solid::cube(10.0, 10.0, 10.0);
	let open = cube.iter_face().next().unwrap();
	// Positive thickness: wall grows outward. The original solid becomes the
	// inner cavity of a 0.5-thick shell.
	let shell = cube.shell(0.5, [open]).expect("shell outward should succeed");
	assert!(shell.volume() > 0.0 && shell.volume() < 1000.0, "outer shell is wall material only, not the original cube");
}

#[test]
fn test_shell_empty_open_faces_inward_seals_cavity() {
	let cube = Solid::cube(10.0, 10.0, 10.0);
	// Negative thickness + empty open_faces: sealed solid with an internal void.
	// Expected wall-material volume = 10³ − 9³ = 271.
	let sealed = cube.shell(-0.5, std::iter::empty::<&cadrum::Face>()).expect("inward empty-open shell should succeed");
assert!((sealed.volume() - 271.0).abs() < 1e-3, "inward empty shell volume = 10³ − 9³, got {}", sealed.volume());
}

#[test]
fn test_shell_empty_open_faces_outward_seals_cavity() {
	let cube = Solid::cube(10.0, 10.0, 10.0);
	// Positive thickness + empty open_faces: outer shell expands outward with
	// GeomAbs_Arc join (spheres at corners, quarter-cylinders along edges),
	// original surface becomes the internal cavity wall.
	// Outer offset volume = 10³ + 6·(10²·0.5) [face slabs]
	//                     + 12·(π·0.5²·10/4) [quarter-cylinder edges]
	//                     + 8·((4/3)π·0.5³/8) [sphere-octant corners]
	//                   = 1000 + 300 + 7.5π/4·... = 1000 + 300 + 7.5π + π/6.
	// Wait: quarter-cyl vol per edge = π·r²·L/4 = π·0.25·10/4 = 0.625π; 12 edges = 7.5π.
	// Sphere-octant per corner = (4/3)π·0.5³/8 = π/48; 8 corners = π/6.
	// Shell material = 300 + 7.5π + π/6 ≈ 324.086.
	let sealed = cube.shell(0.5, std::iter::empty::<&cadrum::Face>()).expect("outward empty-open shell should succeed");
let expected = 300.0 + 7.5 * std::f64::consts::PI + std::f64::consts::PI / 6.0;
	assert!((sealed.volume() - expected).abs() < 1e-3, "outward empty shell volume ≈ {expected:.3}, got {}", sealed.volume());
}

