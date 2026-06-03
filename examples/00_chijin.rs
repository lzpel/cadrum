//! Build a chijin (hand drum from Amami Oshima) with colors, boolean ops, and SVG export.

use cadrum::{Color, DVec3, Edge, ProfileOrient, Solid};
use std::f64::consts::PI;

fn chijin() -> Result<Solid, cadrum::Error> {
	// ── Body (cylinder): r=15, h=8, centered at origin (z=-4..+4) ────────
	let cylinder = Solid::cylinder(15.0, DVec3::Z * 8.0)
		.translate(DVec3::Z * -4.0)
		.color("#999");

	// ── Sheet: closed polygon in the XZ plane (y=0), swept 360° around Z
	// により面と縁を一体で生成する。Face::from_polygon + Face::revolve の置換版:
	//   - Edge::polygon は最後の点 → 最初の点を自動補完して閉じる
	//   - spine は Z 軸まわりの円。半径によらずプロファイルを Z 周りに純粋回転
	//     させるだけなので任意の正の値で可
	//   - ProfileOrient::Up(Z) でプロファイルの上方向を Z 固定 → 回転(revolve)と等価
	let cross_section = Edge::polygon(&[
		DVec3::new(0.0, 0.0, 5.0),
		DVec3::new(15.0, 0.0, 5.0),
		DVec3::new(17.0, 0.0, 3.0),
		DVec3::new(15.0, 0.0, 4.0),
		DVec3::new(0.0, 0.0, 4.0),
	])?;
	let spine = Edge::circle(1.0, DVec3::Z)?;
	let sheet = Solid::sweep(&cross_section, &[spine], ProfileOrient::Up(DVec3::Z))?
		.color("#fff");
	let sheets = [sheet.clone().mirror(DVec3::ZERO, DVec3::Z), sheet];

	// ── Lacing blocks: 2x8x1, rotated 60° around Z, placed at y=15 ──────
	let block_proto = Solid::cube(DVec3::ZERO, DVec3::new(2.0, 8.0, 1.0))
		.translate(DVec3::new(-1.0, -4.0, -0.5))
		.rotate_z(60.0_f64.to_radians())
		.translate(DVec3::Y * 15.0);

	// ── Lacing holes: thin cylinders through each block ──────────────────
	let hole_proto = Solid::cylinder(0.7, (DVec3::X * 10.0 + DVec3::Z * 30.0).normalize() * 30.0)
		.translate(DVec3::new(-5.0, 16.0, -15.0));

	// Distribute N blocks and holes evenly around Z, each block in a rainbow color
	// N 個のブロックと穴を Z 軸周りに等間隔配置、各ブロックに虹色を割り当て
	const N: usize = 20;
	let angle = |i: usize| 2.0 * PI * (i as f64) / (N as f64);
	let color = |i: usize| Color::from_hsv(i as f32 / N as f32, 1.0, 1.0);
	let blocks: [Solid; N] = std::array::from_fn(|i| block_proto.clone().rotate_z(angle(i)).color(color(i)));
	let holes: [Solid; N] = std::array::from_fn(|i| hole_proto.clone().rotate_z(angle(i)));
	// ── Assemble with boolean operations: union, subtract, union ─────────
	let mut result: Solid = (&cylinder + &sheets[0] + &sheets[1]).build()?;
	for i in 0..N{
		result = (&result - &holes[i] + &blocks[i]).build()?;
	}
	Ok(result)
}

fn main() -> Result<(), cadrum::Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();
	let result = [chijin()?];

	Solid::write_step(&result, &mut std::fs::File::create(format!("{example_name}.step")).unwrap())?;

	let scene = Solid::mesh(&result, Default::default())?.scene(DVec3::ONE, DVec3::Y, true, false);
	scene.write_svg(&mut std::fs::File::create(format!("{example_name}.svg")).unwrap())?;
	scene.write_png([1280 , 640], &mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;
	let mesh = Solid::mesh(&result, Default::default())?;
	mesh.write_stl(&mut std::fs::File::create(format!("{example_name}.stl")).unwrap())?;
	mesh.write_gltf_binary(&mut std::fs::File::create(format!("{example_name}.glb")).unwrap())?;
	// This size 1280 x 640 is special because 00_chijin.png is used for github repository social media preview image.
	// > we recommend a size of at least 640 by 320 pixels (1280 by 640 pixels for best display).　See https://docs.github.com/ja/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/customizing-your-repositorys-social-media-preview

	println!("wrote {example_name}.step / {example_name}.svg / {example_name}.png");
	Ok(())
}
