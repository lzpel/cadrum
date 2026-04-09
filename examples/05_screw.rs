use cadrum::{Edge, Error, ProfileOrient, Solid, SolidExt, Transform};
use glam::DVec3;

fn build_m2_screw(orient: ProfileOrient) -> Result<Vec<Solid>, Error> {
	// iso m2 screw
	let r = 1.0;
	let h_pitch = 0.4;
	let h_thread = 6.0;
	let r_head = 1.75;
	let h_head = 1.3;
	// iso m screw mountain height: 60° tooth → 正三角形の高さ √3/2
	let r_delta = 3f64.sqrt() / 2.0 * h_pitch;
	let r_root = r - (5.0 / 8.0) * r_delta;

	// 単一エッジのヘリックス (spine)。
	// x_ref=DVec3::X を渡しているので、ヘリックスは確定的に (r_root, 0, 0) から
	// 始まり、+Z 方向に上昇しつつ +X→+Y→-X→-Y… の順に巻いていく。
	let helix = Edge::helix(r_root, h_pitch, h_thread, DVec3::Z, DVec3::X);

	// 閉じた三角形プロファイル (Vec<Edge> = Wire)。
	// polygon は常に閉じる: 最後の点 → 最初の点が自動補完される。
	let profile = Edge::polygon([DVec3::new(0.0, -h_pitch / 2.0, 0.0), DVec3::new(r - r_root, 0.0, 0.0), DVec3::new(0.0, h_pitch / 2.0, 0.0)]);

	// プロファイルを XY 平面 (法線 Z) からヘリックス始点の接線方向に
	// 回転し、そのまま始点へ平行移動する。Vec<Edge> は Vec<T: Transform>
	// 経由で align_z / translate を持つ。
	let profile = profile.align_z(helix.start_tangent(), helix.start_point()).translate(helix.start_point());

	// ヘリックスに沿って sweep。ProfileOrient によりプロファイルの回転則が変わる。
	// helix では Torsion (raw Frenet) と Up(axis) が等価で、両方とも正しい
	// ねじ山を作る。Fixed は profile を回転させないので、ねじ山にならない
	// 「壊れた」ねじが出力される (これは Fixed の仕様どおりの挙動)。
	let thread = Solid::sweep(&profile, std::slice::from_ref(&helix), orient)?;

	// ねじ山と軸心円柱を合体。外側クリップ円柱は試行錯誤しやすいよう off。
	let shaft = Solid::cylinder(r_root, DVec3::Z, h_thread);
	let thread_shaft = thread.union(&[shaft])?;

	// 平頭を上に重ねる。
	let head = Solid::cylinder(r_head, DVec3::Z, h_head).translate(DVec3::Z * h_thread);
	thread_shaft.union(&[head])
}

fn main() {
	let example_name = std::path::Path::new(file!()).file_stem().unwrap().to_str().unwrap();

	// 3 つの ProfileOrient を順に並べる。各ねじの最大半径 = r_head = 1.75 なので、
	// 直径 3.5 + マージンで X 方向に 5.0 ずつずらせば干渉しない。
	// - red    Fixed              : profile が一切回転しない → 壊れたねじ (期待挙動)
	// - lime   Torsion            : raw Frenet → 正しいねじ山
	// - blue   Up(DVec3::Z)       : helix 軸 +Z を fixed binormal → Torsion と等価で正しいねじ山
	// blue (Up) と lime (Torsion) はジオメトリ的にビット単位で同じになるはず。
	let modes: [(&str, ProfileOrient, &str); 3] = [
		("Fixed", ProfileOrient::Fixed, "red"),
		("Torsion", ProfileOrient::Torsion, "lime"),
		("Up(+Z)", ProfileOrient::Up(DVec3::Z), "blue"),
	];
	let spacing = 5.0_f64;

	let mut all: Vec<Solid> = Vec::new();
	let mut volumes: Vec<(String, f64)> = Vec::new();
	for (i, (name, orient, color)) in modes.iter().enumerate() {
		match build_m2_screw(*orient) {
			Ok(screw) => {
				let v: f64 = screw.volume();
				volumes.push((name.to_string(), v));
				let dx = (i as f64) * spacing;
				let translated: Vec<Solid> = screw.translate(DVec3::new(dx, 0.0, 0.0)).color(*color);
				all.extend(translated);
				println!("  [{i}] {name:8} ({color:5}): ok  vol={v:.4}  (placed at x={dx:.1})");
			}
			Err(e) => {
				println!("  [{i}] {name:8} ({color:5}): failed ({e})");
			}
		}
	}

	// 数値検証: Torsion と Up(+Z) は helix では等価のはず → 体積も一致する。
	let torsion_vol = volumes.iter().find(|(n, _)| n == "Torsion").map(|(_, v)| *v);
	let up_vol = volumes.iter().find(|(n, _)| n == "Up(+Z)").map(|(_, v)| *v);
	if let (Some(tv), Some(uv)) = (torsion_vol, up_vol) {
		let diff = (tv - uv).abs();
		let rel = diff / tv.max(uv).max(1e-12);
		println!();
		println!("  equivalence check (helix で Torsion ≡ Up(axis) のはず):");
		println!("    Torsion volume = {tv:.6}");
		println!("    Up(+Z)  volume = {uv:.6}");
		println!("    abs diff       = {diff:.2e}  ({:.2e} relative)", rel);
		// 理論的には同一トリヘドロン → 同一ジオメトリのはずだが、OCCT 内部
		// (サーフェスフィッティングや曲線サンプリング) の違いで小さな数値
		// ドリフトが残る。CAD 実用上は < 1% で十分「等価」と見なせる。
		if rel < 1e-9 {
			println!("    ✓ bit-identical (within float precision)");
		} else if rel < 1e-2 {
			println!("    ✓ equivalent within 1% (helix の数学的等価性 + OCCT 数値ドリフト)");
		} else {
			println!("    ✗ DIVERGED — Torsion and Up should match for a helix");
		}
	}

	if all.is_empty() {
		eprintln!("all sweep orients failed — nothing to write");
		return;
	}

	let mut f = std::fs::File::create(format!("{example_name}.step")).expect("failed to create STEP file");
	cadrum::io::write_step(&all, &mut f).expect("failed to write STEP");
	let mut f_svg = std::fs::File::create(format!("{example_name}.svg")).expect("failed to create SVG file");
	cadrum::io::write_svg(&all, DVec3::new(1.0, 1.0, -1.0), 0.5, &mut f_svg).expect("failed to write SVG");
	println!("wrote {example_name}.step / {example_name}.svg ({} solids)", all.len());
}
