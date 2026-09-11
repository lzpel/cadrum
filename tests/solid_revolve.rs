//! `Solid::revolve` を解析解で検証する。
//!
//! XZ 平面の断面を Z 軸まわりに一周させると、パップス・ギュルダンの定理から
//! 体積は `2π · x̄ · A`（x̄ は断面重心の軸からの距離、A は断面積）になる。

use cadrum::{DVec3, Edge, Solid};
use std::f64::consts::{PI, TAU};

const EPS: f64 = 1e-3;

/// XZ 平面上の矩形 `x ∈ [x0, x1]`, `z ∈ [z0, z1]`。
fn rect(x0: f64, x1: f64, z0: f64, z1: f64) -> Vec<Edge> {
	Edge::polygon(&[DVec3::new(x0, 0.0, z0), DVec3::new(x1, 0.0, z0), DVec3::new(x1, 0.0, z1), DVec3::new(x0, 0.0, z1)]).expect("rect polygon")
}

#[test]
fn test_full_revolution_matches_the_pipe_volume() {
	let solid = Solid::revolve([&rect(2.0, 5.0, 0.0, 4.0)], DVec3::ZERO, DVec3::Z, TAU).expect("revolve");
	let want = PI * (5.0f64.powi(2) - 2.0f64.powi(2)) * 4.0;
	assert!((solid.volume() - want).abs() < EPS, "volume = {}, want {want}", solid.volume());
}

#[test]
fn test_partial_revolution_scales_with_the_angle() {
	let full = Solid::revolve([&rect(2.0, 5.0, 0.0, 4.0)], DVec3::ZERO, DVec3::Z, TAU).expect("full turn");
	let quarter = Solid::revolve([&rect(2.0, 5.0, 0.0, 4.0)], DVec3::ZERO, DVec3::Z, TAU / 4.0).expect("quarter turn");
	assert!((quarter.volume() - full.volume() / 4.0).abs() < EPS, "quarter = {}, full = {}", quarter.volume(), full.volume());
}

/// 断面に開けた穴は、パップスの定理どおり自身の重心半径で体積を減らす。
#[test]
fn test_a_hole_in_the_profile_is_revolved_too() {
	let solid = Solid::revolve([&rect(1.0, 5.0, 0.0, 6.0), &rect(2.5, 3.5, 2.5, 3.5)], DVec3::ZERO, DVec3::Z, TAU).expect("revolve with hole");
	let outer = TAU * 3.0 * 24.0;
	let hole = TAU * 3.0 * 1.0;
	assert!((solid.volume() - (outer - hole)).abs() < EPS, "volume = {}, want {}", solid.volume(), outer - hole);
}

#[test]
fn test_revolve_rejects_a_zero_axis_or_angle() {
	assert!(Solid::revolve([&rect(2.0, 5.0, 0.0, 4.0)], DVec3::ZERO, DVec3::ZERO, TAU).is_err(), "zero axis");
	assert!(Solid::revolve([&rect(2.0, 5.0, 0.0, 4.0)], DVec3::ZERO, DVec3::Z, 0.0).is_err(), "zero angle");
}
