//! `Sketch` (一般化円板の DNF) の向き・演算子・membership を `contains` で黒箱検証する。
//! DNF の内部表現は実装詳細なので、点の内外という不変量で確認する。

use cadrum::{DVec2, Sketch};

fn p(x: f64, y: f64) -> DVec2 {
	DVec2::new(x, y)
}

#[test]
fn circle_inside_outside() {
	let c = Sketch::circle(p(0.0, 0.0), 1.0);
	assert!(c.contains(p(0.0, 0.0)));
	assert!(c.contains(p(0.9, 0.0)));
	assert!(!c.contains(p(2.0, 0.0)));
}

#[test]
fn line_left_is_inside() {
	// a→b = +x 方向 ⇒ 左 = 上半面 (y>0) が内側。
	let l = Sketch::line(p(0.0, 0.0), p(1.0, 0.0));
	assert!(l.contains(p(0.0, 1.0)));
	assert!(!l.contains(p(0.0, -1.0)));
}

#[test]
fn line_reverse_is_complement() {
	let l = Sketch::line(p(0.0, 0.0), p(1.0, 0.0));
	let rev = Sketch::line(p(1.0, 0.0), p(0.0, 0.0));
	let neg = -Sketch::line(p(0.0, 0.0), p(1.0, 0.0));
	for &q in &[p(0.0, 1.0), p(0.0, -1.0), p(3.0, 2.0), p(-1.0, -4.0)] {
		assert_eq!(rev.contains(q), neg.contains(q));
		assert_eq!(rev.contains(q), !l.contains(q));
	}
}

#[test]
fn complement_negates_membership() {
	let c = Sketch::circle(p(0.0, 0.0), 1.0);
	let n = -c.clone();
	for &q in &[p(0.0, 0.0), p(0.5, 0.0), p(2.0, 0.0), p(0.0, 3.0)] {
		assert_eq!(n.contains(q), !c.contains(q));
	}
}

#[test]
fn union_covers_both() {
	let a = Sketch::circle(p(0.0, 0.0), 1.0);
	let b = Sketch::circle(p(5.0, 0.0), 1.0);
	let u = a + b;
	assert!(u.contains(p(0.0, 0.0)));
	assert!(u.contains(p(5.0, 0.0)));
	assert!(!u.contains(p(2.5, 0.0)));
}

#[test]
fn intersect_is_overlap() {
	let a = Sketch::circle(p(0.0, 0.0), 2.0);
	let b = Sketch::circle(p(1.0, 0.0), 2.0);
	let i = a * b;
	assert!(i.contains(p(0.5, 0.0))); // 両方の内側
	assert!(!i.contains(p(-1.9, 0.0))); // a のみ
	assert!(!i.contains(p(2.9, 0.0))); // b のみ
}

#[test]
fn subtract_cuts_hole() {
	let plate = Sketch::circle(p(0.0, 0.0), 3.0);
	let hole = Sketch::circle(p(0.0, 0.0), 1.0);
	let d = plate - hole;
	assert!(d.contains(p(2.0, 0.0))); // 板内・穴外
	assert!(!d.contains(p(0.0, 0.0))); // 穴内
	assert!(!d.contains(p(4.0, 0.0))); // 板外
}

#[test]
fn region_complement_de_morgan() {
	// ¬(A ∪ B) = ¬A ∩ ¬B を代表点で確認。
	let a = Sketch::circle(p(0.0, 0.0), 1.0);
	let b = Sketch::circle(p(5.0, 0.0), 1.0);
	let u = a + b;
	let n = -u.clone();
	for &q in &[p(0.0, 0.0), p(5.0, 0.0), p(2.5, 0.0), p(0.0, 9.0)] {
		assert_eq!(n.contains(q), !u.contains(q));
	}
}

#[test]
fn chaining_type_and_value() {
	let a = Sketch::circle(p(0.0, 0.0), 2.0);
	let b = Sketch::circle(p(1.0, 0.0), 2.0);
	let hole = Sketch::circle(p(0.5, 0.0), 0.3);
	let s: Sketch = a * b - hole; // Sketch→Sketch の連鎖
	assert!(s.contains(p(0.5, 1.0))); // 重なりの内・穴の外
	assert!(!s.contains(p(0.5, 0.0))); // 穴の中心
}
