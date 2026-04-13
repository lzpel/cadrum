use cadrum::{Error, Solid};
use glam::{DQuat, DVec3};

const I_MAX: usize = 10;
const J_MAX: usize = 10;
fn s(i: usize, j: usize) -> DVec3 {
    let phi = (i as f64) / (J_MAX as f64) * 2.0 * std::f64::consts::PI;
    let theta = (j as f64) / (I_MAX as f64) * 2.0 * std::f64::consts::PI;
    let p=DVec3::new(1.0, 0.0, 0.0);
    let p_with_theta=DQuat::from_axis_angle(DVec3::Z, theta) * p;
    let p_with_phi=DQuat::from_axis_angle(DVec3::Y, phi) * (p_with_theta + DVec3::X*3.0);
    p_with_phi
}
fn bspline_solid(periodic: bool) -> Result<Solid, Error> {
    // periodic=trueのときトーラス、falseのときパイプ
    // 与えた制御点はサーフェス上(近似誤差の範囲で)を通る
    // periodic=trueの時、この関数の場合は完全な回転対称になる
    let grid: [[DVec3; J_MAX]; I_MAX] = std::array::from_fn(|i| std::array::from_fn(|j| s(i, j)));
    Solid::bspline(grid, periodic)
}
fn main() {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
    let mut objects: Vec<Solid> = Vec::new();
    for (periodic, offset) in [(false, 0.0), (true, 10.0)] {
        match bspline_solid(periodic) {
            Ok(g) => {
                let volume = g.volume();
                eprintln!("periodic={}: volume = {}", periodic, volume);
                if 50.0 <= volume && volume <= 60.0 {
                    eprintln!("  -> in range. great");
                } else {
                    eprintln!("  -> out of range");
                }
                objects.push(g.translate(DVec3::Y * offset));
            }
            Err(e) => eprintln!("periodic={}: error: {}", periodic, e),
        }
    }
    let mut f = std::fs::File::create(format!("{example_name}.step")).unwrap();
    cadrum::io::write_step(&objects, &mut f).unwrap();
    let mut f_svg = std::fs::File::create(format!("{example_name}.svg")).unwrap();
    cadrum::io::write_svg(&objects, DVec3::new(1.0, 1.0, 1.0), 0.5, false, &mut f_svg).unwrap();
    eprintln!("wrote {0}.step / {0}.svg ({1} solids)", example_name, objects.len());
}