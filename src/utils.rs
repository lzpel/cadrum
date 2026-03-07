use crate::error::Error;
use crate::shape::Shape;
use glam::DVec3;

/// 切断面フェイスの Compound を delta 方向に押し出してフィラーを作ります。
fn extrude_faces(cut_faces: &Shape, delta: DVec3) -> Result<Shape, Error> {
	let mut filler: Option<Shape> = None;
	for face in cut_faces.faces() {
		let extruded = Shape::from(face.extrude(delta)?);
		filler = Some(match filler {
			None => extruded,
			Some(f) => Shape::from(f.union(&extruded)?),
		});
	}
	Ok(filler.unwrap_or_else(Shape::empty))
}

/// `origin` を含み `plane_normal` が法線の平面で `shape` を切断し、
/// その断面を `origin` を通る `axis_dir` 軸周りに `angle` だけ回転させた回転体を返す。
///
/// # 制約
/// `axis_dir ⊥ plane_normal` であること（軸が切断平面内に含まれる）。
/// これが満たされない場合、回転体は有意な体積を持たない。
///
/// # カラー
/// 回転体は新規ジオメトリのため colormap は空。必要に応じて `.paint()` で着色すること。
pub fn revolve_section(
	shape: &Shape,
	origin: DVec3,
	axis_direction: DVec3,
	plane_normal: DVec3,
	angle: f64,
) -> Result<Shape, Error> {
	let half = Shape::half_space(origin, -plane_normal.normalize());
	let cut_faces = shape.intersect(&half)?.new_faces;

	let mut result: Option<Shape> = None;
	for face in cut_faces.faces() {
		let revolved = Shape::from(face.revolve(origin, axis_direction, angle)?);
		result = Some(match result {
			None => revolved,
			Some(r) => Shape::from(r.union(&revolved)?),
		});
	}
	Ok(result.unwrap_or_else(Shape::empty))
}

/// 指定された座標とベクトルで形状を分割し、片方を平行移動させた後、隙間を押し出し形状で埋めることで引き伸ばしを行います。
/// intersect の BooleanShape::new_faces から切断面を直接取得するため、
/// 法線・重心による heuristic フィルタを使いません。
pub fn stretch_vector(shape: &Shape, origin: DVec3, delta: DVec3) -> Result<Shape, Error> {
	// Negate so the solid fills the -delta side; intersect then yields part_neg.
	let half = Shape::half_space(origin, -delta.normalize());

	let intersect_result = shape.intersect(&half)?;
	let part_neg = intersect_result.shape;
	let cut_faces = intersect_result.new_faces;
	let part_pos = Shape::from(shape.subtract(&half)?).translated(delta);

	let filler = extrude_faces(&cut_faces, delta)?;
	let combined = Shape::from(part_neg.union(&filler)?);
	combined.union(&part_pos).map(Shape::from)
}
