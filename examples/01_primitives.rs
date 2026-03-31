use cadrum::{Color, Shape, Solid};
use glam::DVec3;

fn main() {
    let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

    // Box: 10x20x30, placed at origin
    let box_ = Solid::box_from_corners(DVec3::ZERO, DVec3::new(10.0, 20.0, 30.0))
        .color_paint(Some(Color::from_hex("#4a90d9").unwrap()));

    // Cylinder: radius=8, height=30, placed 30 units away along X so it does not overlap
    let cylinder = Solid::cylinder(DVec3::new(30.0, 0.0, 0.0), 8.0, DVec3::Z, 30.0)
        .color_paint(Some(Color::from_hex("#e67e22").unwrap()));

    let shapes = vec![box_, cylinder];

    let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create file");
    cadrum::write_step_with_colors(&shapes, &mut f).expect("failed to write STEP");

    let svg = shapes.to_svg(DVec3::new(1.0, 1.0, 1.0), 0.5).expect("failed to export SVG");
    std::fs::write(format!("{example_name}.svg"), svg.as_bytes()).expect("failed to write SVG");
}

