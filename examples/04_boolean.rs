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
        .translate(DVec3::X*4.);
    let [cylinder0, cylinder1, cylinder2] = [cylinder.clone(), cylinder.clone().rotate_z(std::f64::consts::TAU/3.), cylinder.clone().rotate_z(-std::f64::consts::TAU/3.)];

    // sum = union of all cylinders
    let sum = [&cylinder0, &cylinder1, &cylinder2].into_iter().sum::<Result<Solid, _>>()?.color("#d875ff");
    
    // product = intersection of all cylinders
    let product = [&cylinder0, &cylinder1, &cylinder2].into_iter().product::<Result<Solid, _>>()?.color("#00ff22");

    let shapes = [
        union.translate(DVec3::X * 0.0), 
        subtract.translate(DVec3::X * 40.0), 
        intersect.translate(DVec3::X * 80.0), 
        sum.translate(DVec3::X * 20.0 + DVec3::Y * 40.0), 
        product.translate(DVec3::X * 60.0 + DVec3::Y * 40.0)
    ];

    Solid::write_step(&shapes, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

    let scene = Solid::mesh(&shapes, 0.5)?.scene(DVec3::new(1.0, 1.0, 2.0), DVec3::Z, true, false);
    scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
    scene.write_png([1024, 1024], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;

    println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
    Ok(())
}
