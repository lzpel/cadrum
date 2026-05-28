//! Boolean expression tree over Solids.
//!
//! `Boolean<S>` は `Solid` の `+`/`-`/`*` で構築される遅延式ツリー。内部表現は
//! DIMACS-flat DNF (`Vec<i64>` + 0 終端)。エンコーディングと合成規則の詳細は
//! `notes/20260527-boolean演算の刷新.md` を参照。
//!
//! 終端評価は [`Boolean::build`] で単一 Solid、[`Boolean::build_vec`] で全ピース。
//! FFI (`SolidStruct::boolean_build`) を 1 回呼び BOPAlgo_CellsBuilder が全交差を1 パスで計算する。

use crate::common::error::Error;
use crate::traits::SolidStruct;
use std::ops::{Add, Mul, Sub};

pub struct Boolean<S: SolidStruct> {
	pub(crate) solids: Vec<S>,
	pub(crate) clauses: Vec<i64>, // 0-terminated DNF
}

impl<S: SolidStruct> Boolean<S> {
	/// `S::boolean(...)` 専用コンストラクタ。
	pub(crate) fn from_parts(solids: Vec<S>, clauses: Vec<i64>) -> Self {
		Boolean { solids, clauses }
	}

	/// 内部表現アクセサ (FFI 用)。
	pub fn solids(&self) -> &[S] { &self.solids }
	pub fn clauses(&self) -> &[i64] { &self.clauses }

	/// FFI を呼んで結果が単一 Solid なら返す。複数または 0 個なら `OneFailed(n)`。
	pub fn build(self) -> Result<S, Error> {
		let mut v = self.build_vec()?;
		match v.len() {
			1 => Ok(v.pop().unwrap()),
			n => Err(Error::OneFailed(n)),
		}
	}

	/// FFI を呼んで全ピースを返す。空式は `OneFailed(0)`。
	pub fn build_vec(self) -> Result<Vec<S>, Error> {
		if self.solids.is_empty() || self.clauses.is_empty() {
			return Err(Error::OneFailed(0));
		}
		S::boolean_build(&self)
	}

	// DNF 上で閉じる合成 (規則は notes/20260527 参照)。
	pub(crate) fn dnf_union(mut a: Self, b: Self) -> Self {
		let shift = a.solids.len() as i64;
		a.solids.extend(b.solids);
		for lit in b.clauses {
			if lit == 0 {
				a.clauses.push(0);
			} else if lit > 0 {
				a.clauses.push(lit + shift);
			} else {
				a.clauses.push(lit - shift);
			}
		}
		a
	}

	pub(crate) fn dnf_intersect(a: Self, b: Self) -> Self {
		let a_clauses: Vec<Vec<i64>> = a.clauses
			.split(|&l| l == 0)
			.filter(|c| !c.is_empty())
			.map(|c| c.to_vec())
			.collect();
		let shift = a.solids.len() as i64;
		let b_clauses: Vec<Vec<i64>> = b.clauses
			.split(|&l| l == 0)
			.filter(|c| !c.is_empty())
			.map(|c| c.iter().map(|&l| if l > 0 { l + shift } else { l - shift }).collect())
			.collect();
		let mut solids = a.solids;
		solids.extend(b.solids);
		let mut clauses = Vec::with_capacity(a_clauses.len() * b_clauses.len() * 4);
		for ca in &a_clauses {
			for cb in &b_clauses {
				clauses.extend_from_slice(ca);
				clauses.extend_from_slice(cb);
				clauses.push(0);
			}
		}
		Boolean { solids, clauses }
	}

	pub(crate) fn dnf_subtract(a: Self, b: Self) -> Self {
		// a ∩ ¬b。¬b = 各 b_clause から lit を 1 つずつ選び否定した AND の全パターン。
		let b_clauses: Vec<Vec<i64>> = b.clauses
			.split(|&l| l == 0)
			.filter(|c| !c.is_empty())
			.map(|c| c.to_vec())
			.collect();
		if b_clauses.is_empty() {
			return a; // b = ⊥ ⇒ ¬b = ⊤ ⇒ a - b = a
		}
		let mut accum: Vec<Vec<i64>> = vec![Vec::new()];
		for clause in &b_clauses {
			let mut next = Vec::with_capacity(accum.len() * clause.len());
			for partial in &accum {
				for &lit in clause {
					let mut combined = partial.clone();
					combined.push(-lit);
					next.push(combined);
				}
			}
			accum = next;
		}
		let mut neg_b_clauses = Vec::new();
		for cl in accum {
			neg_b_clauses.extend(cl);
			neg_b_clauses.push(0);
		}
		let neg_b = Boolean { solids: b.solids, clauses: neg_b_clauses };
		Self::dnf_intersect(a, neg_b)
	}
}

impl<S: SolidStruct> Clone for Boolean<S> {
	fn clone(&self) -> Self {
		// `S::boolean` 経由の shallow copy で TShape identity を保つ。`self.solids.clone()`
		// は deep_copy が走り face id が変わるため不可。
		S::boolean(self.solids.iter(), self.clauses.iter().copied())
	}
}

// ==================== TryFrom (終端評価) ====================

impl<S: SolidStruct> TryFrom<Boolean<S>> for Vec<S> {
	type Error = Error;
	fn try_from(b: Boolean<S>) -> Result<Self, Error> {
		b.build_vec()
	}
}

// 汎用 `impl<S: SolidStruct> TryFrom<Boolean<S>> for S` は orphan rule 違反のため、
// 「Boolean<Self> → Self」の TryFrom は具象側 (src/occt/solid.rs) に置く。

// ==================== From / 演算子 ====================
//
// `From` が `Solid`/`&Solid` → `Boolean<S>` の入口。演算子は `Boolean<S>` 左辺に集約
// (dnf 呼び出しはこのファイルのみ)。裸の Solid/&Solid 左辺の糖衣は orphan rule で
// generic 化できず traits::impl_solid_boolean_ops! が src/occt/solid.rs で生成する。

impl<S: SolidStruct> From<S> for Boolean<S> {
	// owned でも借用渡し。metadata move 最適化はしない (generic 契約の軽微なコスト)。
	fn from(s: S) -> Self {
		S::boolean(std::iter::once(&s), [1i64, 0])
	}
}

impl<'a, S: SolidStruct> From<&'a S> for Boolean<S> {
	fn from(s: &'a S) -> Self {
		S::boolean(std::iter::once(s), [1i64, 0])
	}
}

// `Boolean<S>` 左辺の `+`/`-`/`*`。RHS を `.into()` で `Boolean<S>` 化 (reflexive /
// 上記 From) して dnf 合成。値 RHS と参照 RHS でライフタイムが違うので 2 アーム。
macro_rules! boolean_lhs_ops {
	(& $rhs:ty) => {
		impl<'a, S: SolidStruct> Add<&'a $rhs> for Boolean<S> {
			type Output = Boolean<S>;
			fn add(self, rhs: &'a $rhs) -> Boolean<S> { Boolean::dnf_union(self, rhs.into()) }
		}
		impl<'a, S: SolidStruct> Sub<&'a $rhs> for Boolean<S> {
			type Output = Boolean<S>;
			fn sub(self, rhs: &'a $rhs) -> Boolean<S> { Boolean::dnf_subtract(self, rhs.into()) }
		}
		impl<'a, S: SolidStruct> Mul<&'a $rhs> for Boolean<S> {
			type Output = Boolean<S>;
			fn mul(self, rhs: &'a $rhs) -> Boolean<S> { Boolean::dnf_intersect(self, rhs.into()) }
		}
	};
	($rhs:ty) => {
		impl<S: SolidStruct> Add<$rhs> for Boolean<S> {
			type Output = Boolean<S>;
			fn add(self, rhs: $rhs) -> Boolean<S> { Boolean::dnf_union(self, rhs.into()) }
		}
		impl<S: SolidStruct> Sub<$rhs> for Boolean<S> {
			type Output = Boolean<S>;
			fn sub(self, rhs: $rhs) -> Boolean<S> { Boolean::dnf_subtract(self, rhs.into()) }
		}
		impl<S: SolidStruct> Mul<$rhs> for Boolean<S> {
			type Output = Boolean<S>;
			fn mul(self, rhs: $rhs) -> Boolean<S> { Boolean::dnf_intersect(self, rhs.into()) }
		}
	};
}
boolean_lhs_ops!(Boolean<S>);
boolean_lhs_ops!(S);
boolean_lhs_ops!(&S);

// ==================== Default (= ⊥ / union の単位元) ====================
//
// 空式 = ⊥ (空集合 / 何も選択していない)。`build()` は `OneFailed(0)`。
// union を fold で畳むときの init に使う: `iter.fold(Boolean::default(), |a, s| a + s)`。
// `dnf_union(default(), b) == b` なので union の単位元として正しい。
// intersect では零元 (annihilator) になるため init に使ってはいけない — intersect の集約は
// init を持たない `reduce(|a, b| a * b)` を使う (単位元 ⊤ は buildable な solid に表現不可)。
impl<S: SolidStruct> Default for Boolean<S> {
	fn default() -> Self {
		Boolean { solids: Vec::new(), clauses: Vec::new() }
	}
}
