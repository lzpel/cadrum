//! Boolean operations: union, subtract, and intersect between a box and a cylinder.

use cadrum::{DVec3, Solid};

fn main() -> Result<(), cadrum::Error> {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

    let make_box = Solid::cube(20.0, 20.0, 20.0)
        .translate(DVec3::X * -10.+ DVec3:: Y*-10.)
        .color("#4a90d9");
    let make_cyl = Solid::cylinder(8.0, DVec3::Z, 30.0)
        .translate(DVec3::Z*-5.)
        .color("#e67e22");

    // union: merge both shapes into one — offset X=0
    let union = (&make_box + &make_cyl)?;

    // subtract: box minus cylinder — offset X=40
    let subtract = (&make_box - &make_cyl)?;

    // intersect: only the overlapping volume — offset X=80
    let intersect = (&make_box * &make_cyl)?;

    let cylinder = Solid::cylinder(8.0, DVec3::Z, 30.0)
        .translate(DVec3::X*4.)
    let [cylinder0, cylinder1, cylinder2] = [cylinder.clone(), cylinder.clone().rotate_z(std::f64::consts::TAU/3.), cylinder.clone().rotate_z(-std::f64::consts::TAU/3.)];

    // 3円（円柱）ベン図のsum
    let sum = [&cylinder0, &cylinder1, &cylinder2].into_iter().sum::<Result<Solid, _>>()?.color("#d875ff");
    
    // 3円（円柱）ベン図のproduct
    let product = [&cylinder0, &cylinder1, &cylinder2].into_iter().product::<Result<Solid, _>>()?.color("#00ff22");

    let shapes = [
        union.translate(DVec3::X * 0.0), 
        subtract.translate(DVec3::X * 40.0), 
        intersect.translate(DVec3::X * 80.0), 
        sum.translate(DVec3::X * 20.0 + DVec3::Y * 40.0), 
        product.translate(DVec3::X * 60.0 + DVec3::Y * 40.0)
    ];

    let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create file");
    Solid::write_step(&shapes, &mut f).expect("failed to write STEP");

    let mut svg = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
    Solid::mesh(&shapes, 0.5).and_then(|m| m.write_svg(DVec3::new(1.0, 1.0, 2.0), DVec3::Z, true, false, &mut svg)).expect("failed to write SVG");

    Ok(())
}
