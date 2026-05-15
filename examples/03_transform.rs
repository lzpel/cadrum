//! Transform operations: translate, rotate, scale, and mirror applied to a cone.

use cadrum::{DVec3, Solid};
use std::f64::consts::PI;

fn main() -> Result<(), cadrum::Error> {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

    let base = Solid::cone(8.0, 0.0, DVec3::Z, 20.0)
        .color("#888888");

    let solids = [
        // original — reference, no transform
        base.clone(),
        // translate — shift +20 along Z
        base.clone()
            .color("#4a90d9")
            .translate(DVec3::X * 40.0 + DVec3::Z * 20.0),
        // rotate — 90° around X axis so the cone tips toward Y
        base.clone()
            .color("#e67e22")
            .rotate_x(PI / 2.0)
            .translate(DVec3::X * 80.0),
        // scaled — 1.5x from its local origin
        base.clone()
            .color("#2ecc71")
            .scale(DVec3::ZERO, 1.5)
            .translate(DVec3::X * 120.0),
        // mirror — flip across Z=0 plane so the tip points down
        base.clone()
            .color("#e74c3c")
            .mirror(DVec3::ZERO, DVec3::Z)
            .translate(DVec3::X * 160.0),
    ];

    Solid::write_step(&solids, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

    let scene = Solid::mesh(&solids, 0.5)?.scene(DVec3::ONE, DVec3::Z, true, false);
    scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
    scene.write_png([640, 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;

    println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
    Ok(())
}
