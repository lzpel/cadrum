//! ちぢん例: 奄美大島の楽器「ちぢん」を chijin ライブラリで再現する
//!
//! ```
//! cargo run --example chijin --features bundled,color
//! ```
//!
//! 出力: out/chijin.step (AP214 STEP, 色付き)

use chijin::{Boolean, Face, Rgb, Shape, Solid};
use glam::DVec3;
use std::f64::consts::PI;
use std::path::Path;


fn chijin() -> Solid {
	// ── 色定義 ────────────────────────────────────────────────────────────────
	// 鼓面・胴体: 濃い茶色
	let dark_brown = Rgb {
		r: 0.6,
		g: 0.6,
		b: 0.6,
	};
	// 縁・締め木: 明るい茶色
	let light_brown = Rgb {
		r: 1.0,
		g: 1.0,
		b: 1.0,
	};

	// ── 胴体 (cylinder): r=15cm, h=10cm, 原点中心 ────────────────────────────
	// 底面中心を z=-5 にして、形状が z=-5..+5 の範囲に収まるようにする
	let cylinder: Solid =
		Solid::cylinder(DVec3::new(0.0, 0.0, -4.0), 15.0, DVec3::Z, 8.0).color_paint(dark_brown);

	// ── 縁板 (sheet): x=0 の多角形プロファイルを Z 軸回転体にした薄いリング ──
	// プロファイル点(y, z): (0,5),(15,5),(16,3),(15,4),(0,4)
	// → extrude で厚みを持たせ、回転 (revolution) の代わりに
	//   多数の薄いくさびを union して近似する。
	//
	// 簡易実装: 外径16cm・内径15cmのリングを z=3..5 に配置する薄いシェル
	// outer cylinder - inner cylinder で中空シリンダーを作る
	let sheet_face = Face::from_polygon(&[
		DVec3::new(0.0, 0.0, 5.0),
		DVec3::new(0.0, 15.0, 5.0),
		DVec3::new(0.0, 17.0, 3.0),
		DVec3::new(0.0, 15.0, 4.0),
		DVec3::new(0.0, 0.0, 4.0),
		DVec3::new(0.0, 0.0, 5.0),
	])
	.unwrap();
	let sheet = sheet_face
		.revolve(DVec3::ZERO, DVec3::Z, 2.0 * PI)
		.unwrap()
		.color_paint(light_brown);
	let sheets = [sheet.mirrored(DVec3::ZERO, DVec3::Z), sheet];

	// ── 締め木 (block): 2cm x 5cm x 1cm ─────────────────────────────────────
	// x軸方向に伸びた板。z軸方向に 60° の仰角で配置し、x=0, y=0, z=15cm へ移動
	// corners: (-1, -2.5, -0.5) .. (1, 2.5, 0.5)
	let block_proto =
		Solid::box_from_corners(DVec3::new(-1.0, -4.0, -0.5), DVec3::new(1.0, 4.0, 0.5));
	// z軸まわりに 60°回転（板を斜めにする）してから (0, 15, 0) に移動
	let block_proto = block_proto
		.rotate(DVec3::ZERO, DVec3::Z, 60.0_f64.to_radians())
		.translate(DVec3::new(0.0, 15.0, 0.0));
	let hole_proto = Solid::cylinder(
		DVec3::new(-5.0, 16.0, -15.0),
		0.7,
		DVec3::new(10.0, 0.0, 30.0),
		30.0,
	);

	// n=20 個の締め木を 360°/n ずつ Z 軸回転して虹色に配置
	let n = 20usize;
	let mut blocks: Vec<Solid> = Vec::with_capacity(n);
	let mut holes: Vec<Solid> = Vec::with_capacity(n);
	for i in 0..n {
		let angle = 2.0 * PI * (i as f64) / (n as f64);
		let color = Rgb::from_hsv(i as f32 / n as f32, 1.0, 1.0);
		blocks.push(
			block_proto
				.clone()
				.rotate(DVec3::ZERO, DVec3::Z, angle)
				.color_paint(color),
		);
		holes.push(hole_proto.clone().rotate(DVec3::ZERO, DVec3::Z, angle));
	}
	let blocks = blocks
		.into_iter()
		.map(|v| vec![v])
		.reduce(|a, b| Boolean::union(&a, &b).unwrap().solids)
		.unwrap();
	let holes = holes
		.into_iter()
		.map(|v| vec![v])
		.reduce(|a, b| Boolean::union(&a, &b).unwrap().solids)
		.unwrap();

	// ── すべてを union ───────────────────────────────────────────────────────
	// cylinder + sheet
	let combined: Vec<Solid> = Boolean::union(&[cylinder], &sheets)
		.expect("cylinder + sheet union に失敗")
		.into();

	// combined + blocks - holes
	let result: Vec<Solid> = Boolean::subtract(&combined, &holes).unwrap().into();
	let result: Vec<Solid> = Boolean::union(&result, &blocks).unwrap().into();
	assert!(result.len() == 1);
	result.into_iter().next().unwrap()
}
fn main() {
	let result = vec![chijin()];
	std::fs::create_dir_all("out").unwrap();

	// ── STEP ファイルとして書き出し ──────────────────────────────────────────
	let step_path = "out/chijin.step";
	let mut f = std::fs::File::create(step_path).expect("STEP ファイル作成に失敗");
	chijin::write_step_with_colors(&result, &mut f).expect("STEP 書き込みに失敗");
	println!("wrote {step_path}");

	// ── SVG として書き出し (斜め上からの視点) ───────────────────────────────
	let svg_path = "out/chijin.svg";
	let svg = result
		.to_svg(DVec3::new(1.0, 1.0, 1.0), 0.5)
		.expect("SVG 書き出しに失敗");
	let mut f = std::fs::File::create(svg_path).expect("SVG ファイル作成に失敗");
	std::io::Write::write_all(&mut f, svg.as_bytes()).expect("SVG 書き込みに失敗");
	println!("wrote {svg_path}");
}
