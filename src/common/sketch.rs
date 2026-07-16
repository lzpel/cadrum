//! 一般化円板の DNF による 2D スケッチ (issue #250)。
//!
//! `Sketch` は各 `DVec4 = (a, bx, by, c)` を一般化円板 `a|x|² + b·x + c ≤ 0` と解釈し、
//! それらのブール式を `Vec<DVec4>` の平坦な DNF で保持する。円・直線・半平面が単一表現に
//! 収まり (直線は `a=0` の退化円板)、補元は符号反転のみ。
//!
//! **エンコード**: `DVec4::ZERO` を節区切りとする 0 区切り DNF。`[節1.., ZERO, 節2.., ...]`。
//! 1 節 = 円板の AND (積集合)、節の OR = 和集合。単一円板は len=1 の `Sketch`。恒真
//! `DVec4::ZERO` は区切り専用値として扱い、意味を持つリテラルには使わない。
//!
//! **演算子** (3D の [`crate::Boolean`] と同義): `+`=union, `*`=intersect, `-`=subtract
//! (`a ∩ ¬b`)、単項 `-`(`Neg`)=補元。合成規則は `crate::common::boolean` の `dnf_*` を移植。
//!
//! **制限**: 空 `Sketch` = ⊥ (空集合)。その補 `¬⊥ = ⊤` (全平面) はこの平坦エンコードでは
//! 表現できない (恒真リテラルが `ZERO` 区切りと衝突するため) ので `Neg` は ⊥ を返す。
//! `boolean.rs` の「単位元 ⊤ は表現不可」と同じ既知の制限。

use glam::{DVec2, DVec4};
use std::ops::{Add, Mul, Neg, Sub};

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sketch(Vec<DVec4>);

impl Sketch {
	/// 内側 = 半径 `r` の円板内部 (`a=+1` の正準符号)。外側は `-Sketch::circle(..)`。
	pub fn circle(center: DVec2, r: f64) -> Sketch {
		Sketch(vec![DVec4::new(1.0, -2.0 * center.x, -2.0 * center.y, center.length_squared() - r * r)])
	}

	/// 内側 = 有向線分 `a→b` の左側 (半径∞の退化円板)。`line(b, a)` は補元。
	pub fn line(a: DVec2, b: DVec2) -> Sketch {
		let d = b - a;
		Sketch(vec![DVec4::new(0.0, d.y, -d.x, d.x * a.y - d.y * a.x)])
	}

	/// 点 `p` が領域に含まれるか。DNF を評価 (節内 AND・節間 OR)。空 `Sketch` = ⊥ = false。
	pub fn contains(&self, p: DVec2) -> bool {
		clauses(&self.0).any(|clause| clause.iter().all(|d| eval(*d, p) <= 0.0))
	}
}

/// 一般化円板 `a|x|² + b·x + c` を点 `p` で評価。
fn eval(d: DVec4, p: DVec2) -> f64 {
	d.x * p.length_squared() + d.y * p.x + d.z * p.y + d.w
}

/// `ZERO` 区切りで節 (AND グループ) に分割。空節 (連続区切り・端) は除く。
fn clauses(v: &[DVec4]) -> impl Iterator<Item = &[DVec4]> {
	v.split(|d| *d == DVec4::ZERO).filter(|c| !c.is_empty())
}

/// 節リストを `ZERO` 区切りで平坦化。
fn flatten<'a>(clauses: impl IntoIterator<Item = &'a [DVec4]>) -> Vec<DVec4> {
	let mut out: Vec<DVec4> = Vec::new();
	for clause in clauses {
		if !out.is_empty() {
			out.push(DVec4::ZERO);
		}
		out.extend_from_slice(clause);
	}
	out
}

// ==================== DNF 合成 (boolean.rs の dnf_* を移植) ====================

/// `a ∪ b`。空は union の単位元 (⊥)。
fn union(a: Sketch, b: Sketch) -> Sketch {
	if a.0.is_empty() {
		return b;
	}
	if b.0.is_empty() {
		return a;
	}
	let mut v = a.0;
	v.push(DVec4::ZERO);
	v.extend(b.0);
	Sketch(v)
}

/// `a ∩ b`。節の直積。どちらか空 (⊥) なら空 (annihilator)。
fn intersect(a: Sketch, b: Sketch) -> Sketch {
	let ca: Vec<&[DVec4]> = clauses(&a.0).collect();
	let cb: Vec<&[DVec4]> = clauses(&b.0).collect();
	if ca.is_empty() || cb.is_empty() {
		return Sketch(Vec::new());
	}
	let mut out: Vec<DVec4> = Vec::new();
	for x in &ca {
		for y in &cb {
			if !out.is_empty() {
				out.push(DVec4::ZERO);
			}
			out.extend_from_slice(x);
			out.extend_from_slice(y);
		}
	}
	Sketch(out)
}

/// `¬s`。ド・モルガンで各節から `-lit` を 1 つずつ選ぶ全組み合わせ (DNF へ再分配)。
/// `¬⊥ = ⊤` は表現不可 (モジュール doc 参照) なので空入力は空 (⊥) を返す。
fn complement(s: Sketch) -> Sketch {
	let cs: Vec<&[DVec4]> = clauses(&s.0).collect();
	if cs.is_empty() {
		return Sketch(Vec::new());
	}
	let mut accum: Vec<Vec<DVec4>> = vec![Vec::new()];
	for clause in &cs {
		let mut next = Vec::with_capacity(accum.len() * clause.len());
		for partial in &accum {
			for lit in clause.iter() {
				let mut combined = partial.clone();
				combined.push(-*lit);
				next.push(combined);
			}
		}
		accum = next;
	}
	Sketch(flatten(accum.iter().map(|c| c.as_slice())))
}

/// `a - b = a ∩ ¬b`。`b = ⊥` なら `¬b = ⊤` で `a` を返す。
fn subtract(a: Sketch, b: Sketch) -> Sketch {
	if b.0.is_empty() {
		return a;
	}
	intersect(a, complement(b))
}

// ==================== 演算子 ====================

impl Add for Sketch {
	type Output = Sketch;
	fn add(self, rhs: Sketch) -> Sketch {
		union(self, rhs)
	}
}

impl Mul for Sketch {
	type Output = Sketch;
	fn mul(self, rhs: Sketch) -> Sketch {
		intersect(self, rhs)
	}
}

impl Sub for Sketch {
	type Output = Sketch;
	fn sub(self, rhs: Sketch) -> Sketch {
		subtract(self, rhs)
	}
}

impl Neg for Sketch {
	type Output = Sketch;
	fn neg(self) -> Sketch {
		complement(self)
	}
}
