//! Fixed 4-view multiview PNG for LLM-driven design loops.
//!
//! `Solid::write_multiview_png` を 1 行で呼び出すと、ISO + 軸 cyclic 順 (+X / +Y / +Z)
//! の 4 視点を同一スケールで配置した 1024×1024 PNG が得られる。引数チューニングなしで
//! Solid → 画像が 1:1 対応するので、LLM への現状確認画像生成・自動設計ループに向く。

use cadrum::{DVec3, Solid};

fn main() -> Result<(), cadrum::Error> {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	let block = Solid::cube(40.0, 30.0, 20.0)
		.translate(-DVec3::new(20.0, 15.0, 10.0));
	let hole = Solid::cylinder(5.0, DVec3::Z, 30.0)
		.translate(-DVec3::Z * 15.0);
	// 軸方向検証用: +X+Y+Z コーナーだけを球で削る。
	// どのパネルでこのノッチがどの角に出るかで gnomon の指す方向が一意に確認できる。
	let corner_cut = Solid::sphere(10.0)
		.translate(DVec3::new(20.0, 15.0, 10.0));
	let part = (&block - &hole)?;
	let part = (&part - &corner_cut)?;

	part.write_multiview_png(&mut std::fs::File::create(format!("{example_name}.png")).unwrap())?;

	println!("wrote {example_name}.png");
	Ok(())
}
