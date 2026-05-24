use glam::Vec2;

/// 境界上の直線・円弧を表すセグメント。
pub enum Segment {
	Line { point: Vec2, direction: Vec2 },
	Circle { center: Vec2, radius: f32 },
}

impl Segment {
	/// セグメントの両端点を返す。Line は十分長い線分として扱う。
	pub fn distance(&self, p: Vec2) -> f32 {
		match self {
			Segment::Line { point, direction } => {
				let d = p - *point;
				let t = d.dot(*direction) / direction.length_squared().max(f32::EPSILON);
				(d - *direction * t).length()
			}
			Segment::Circle { center, radius } => (p - *center).length() - *radius,
		}
	}
}

pub type EdgeLoop = Vec<Segment>;