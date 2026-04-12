// Probe for GeomFill_GordonBuilder pass conditions.
// Sweeps (N, M) from 3x3 through 6x6 on a torus subset, relying on
// cpp/wrapper.cpp::make_gordon_direct's internal combinatorial search
// (which logs one line to stderr per (sP, sG, pP, pG) attempt).
//
// Run:
//   cargo run --example 09_gordon_probe 2> probe.log
//   grep "★ FIRST PASS" probe.log
//
// Each matrix cell reports the first PASS it finds (if any), then emits
// the resulting surface as a single-solid-attempt STEP for visual check.

use cadrum::{BSplineEnd, Edge, Error, Solid, Transform};
use glam::{DQuat, DVec3};

fn s(i: usize, j: usize, i_max: usize, j_max: usize) -> DVec3 {
    let theta = (i as f64) / (i_max as f64) * 2.0 * std::f64::consts::PI;
    let phi = (j as f64) / (j_max as f64) * 2.0 * std::f64::consts::PI;
    let p = DVec3::new(1.0, 0.0, 0.0);
    let p_with_theta = DQuat::from_axis_angle(DVec3::Z, theta) * p;
    DQuat::from_axis_angle(DVec3::Y, phi) * (p_with_theta + DVec3::X * 3.0)
}

fn attempt(n: usize, m: usize) -> Result<Solid, Error> {
    let profiles: Vec<[Edge; 1]> = (0..n)
        .map(|i| {
            let pts: Vec<DVec3> = (0..m).map(|k| s(k, i, m, n)).collect();
            [Edge::bspline(pts, BSplineEnd::Periodic).unwrap()]
        })
        .collect();
    let guides: Vec<[Edge; 1]> = (0..m)
        .map(|j| {
            let pts: Vec<DVec3> = (0..n).map(|k| s(j, k, m, n)).collect();
            [Edge::bspline(pts, BSplineEnd::NotAKnot).unwrap()]
        })
        .collect();
    Solid::gordon(&profiles, &guides)
}

fn main() {
    let mut objects: Vec<Solid> = Vec::new();
    let mut x = 0.0;
    for (n, m) in [(3, 3), (3, 4), (4, 3), (4, 4), (5, 5), (6, 6)] {
        eprintln!("\n=== probing N={} M={} ===", n, m);
        match attempt(n, m) {
            Ok(solid) => {
                let vol = solid.volume();
                eprintln!("N={} M={}: OK volume={:.3}", n, m, vol);
                objects.push(solid.translate(DVec3::X * x));
                x += 10.0;
            }
            Err(e) => {
                eprintln!("N={} M={}: Err {}", n, m, e);
            }
        }
    }

    if objects.is_empty() {
        eprintln!("\n!! no combination yielded a solid");
    } else {
        let mut f = std::fs::File::create("09_gordon_probe.step").unwrap();
        cadrum::io::write_step(&objects, &mut f).unwrap();
        let mut f_svg = std::fs::File::create("09_gordon_probe.svg").unwrap();
        cadrum::io::write_svg(
            &objects,
            DVec3::new(1.0, 1.0, 1.0),
            0.5,
            false,
            &mut f_svg,
        )
        .unwrap();
        eprintln!("\nwrote 09_gordon_probe.step / .svg ({} solids)", objects.len());
    }
}
