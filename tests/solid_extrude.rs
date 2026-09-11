//! `Solid::extrude` の穴あきプロファイルを解析解で検証する。
//!
//! 外周 `a` × 内周 `b` の板を高さ `h` 押し出すと体積は `(a² − b²) · h`。

use cadrum::{DVec3, Edge, Solid};

const EPS: f64 = 1e-6;

/// 一辺 `side` の閉じた正方形を `offset` だけ平行移動したプロファイル。
fn square(side: f64, offset: f64) -> Vec<Edge> {
	let o = offset;
	Edge::polygon(&[DVec3::new(o, o, 0.0), DVec3::new(o + side, o, 0.0), DVec3::new(o + side, o + side, 0.0), DVec3::new(o, o + side, 0.0)]).expect("square polygon")
}

#[test]
fn test_extrude_without_holes_matches_analytical() {
	let solid = Solid::extrude(&square(10.0, 0.0), &[], DVec3::Z * 3.0).expect("extrude");
	assert!((solid.volume() - 300.0).abs() < EPS, "volume = {}", solid.volume());
	assert_eq!(solid.iter_face().count(), 6, "a square prism has six faces");
}

#[test]
fn test_extrude_with_one_hole_subtracts_it() {
	let solid = Solid::extrude(&square(10.0, 0.0), [&square(4.0, 3.0)], DVec3::Z * 3.0).expect("extrude with hole");
	assert!((solid.volume() - (100.0 - 16.0) * 3.0).abs() < EPS, "volume = {}", solid.volume());
	assert_eq!(solid.iter_face().count(), 10, "four outer walls, four hole walls, two caps");
}

#[test]
fn test_extrude_with_two_holes_subtracts_both() {
	let solid = Solid::extrude(&square(20.0, 0.0), [&square(2.0, 2.0), &square(3.0, 12.0)], DVec3::Z * 2.0).expect("extrude with holes");
	assert!((solid.volume() - (400.0 - 4.0 - 9.0) * 2.0).abs() < EPS, "volume = {}", solid.volume());
}

#[test]
fn test_extrude_rejects_a_zero_direction() {
	assert!(Solid::extrude(&square(10.0, 0.0), &[], DVec3::ZERO).is_err());
}
