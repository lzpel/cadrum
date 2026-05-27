//! Boolean expression tree over Solids.
//!
//! `Boolean<S>` は `Solid` の `+`/`-`/`*` 演算子で構築される遅延式ツリー。
//! 内部表現は **DIMACS-flat DNF** (`Vec<i64>` + 0 終端):
//!
//! - `+i` (i ≥ 1): `solids[i-1]` を AddToResult の `toTake` 側に
//! - `-i`         : `toAvoid` 側に
//! - `0`          : clause 終端 (末尾の `0` も必須)
//!
//! 例 (`solids = [A, B, C, D]`):
//!
//! | 式         | clauses                          |
//! |---         |---                               |
//! | `A + B`    | `[1, 0, 2, 0]`                   |
//! | `A * B`    | `[1, 2, 0]`                      |
//! | `A - B`    | `[1, -2, 0]`                     |
//! | `A + B - C`| `[1, -3, 0, 2, -3, 0]`           |
//! | `A - B*C`  | `[1, -2, 0, 1, -3, 0]`           |
//!
//! 終端評価は [`Boolean::build`] (= `TryInto<S>`) で単一 Solid、
//! [`Boolean::build_vec`] (= `TryInto<Vec<S>>`) で全ピース。
//! FFI は `SolidStruct::boolean_build` を 1 回だけ呼び、BOPAlgo_CellsBuilder
//! が全交差を 1 パスで計算する。
//!
//! 集約コンストラクタ:
//! - [`Boolean::union_all`] — iter 全要素を union
//! - [`Boolean::intersect_all`] — iter 全要素を intersect

use crate::common::error::Error;
use crate::traits::SolidStruct;

pub struct Boolean<S: SolidStruct> {
	pub(crate) solids: Vec<S>,
	pub(crate) clauses: Vec<i64>, // 0-terminated DNF
}

impl<S: SolidStruct> Boolean<S> {
	/// バックエンドから solids + clauses を受け取って Boolean を組む。
	/// `S::boolean(...)` 実装専用のコンストラクタ。
	pub(crate) fn from_parts(solids: Vec<S>, clauses: Vec<i64>) -> Self {
		Boolean { solids, clauses }
	}

	/// 内部表現を読むためのアクセサ (FFI から呼ぶ用途)。
	pub fn solids(&self) -> &[S] { &self.solids }
	/// 内部表現を読むためのアクセサ (FFI から呼ぶ用途)。
	pub fn clauses(&self) -> &[i64] { &self.clauses }

	/// `iter` の要素を全て union した `Boolean<S>` を返す。
	/// 空の場合は空の `Boolean` (`build()` で `OneFailed(0)`)。
	pub fn union_all<'a, I>(iter: I) -> Self
	where
		I: IntoIterator<Item = &'a S>,
		S: 'a,
	{
		let refs: Vec<&S> = iter.into_iter().collect();
		let clauses: Vec<i64> = (1..=refs.len() as i64).flat_map(|i| [i, 0]).collect();
		S::boolean(refs, clauses)
	}

	/// `iter` の要素を全て intersect した `Boolean<S>` を返す。
	/// 空の場合は空の `Boolean` (`build()` で `OneFailed(0)`)。
	pub fn intersect_all<'a, I>(iter: I) -> Self
	where
		I: IntoIterator<Item = &'a S>,
		S: 'a,
	{
		let refs: Vec<&S> = iter.into_iter().collect();
		let mut clauses: Vec<i64> = (1..=refs.len() as i64).collect();
		if !clauses.is_empty() { clauses.push(0); }
		S::boolean(refs, clauses)
	}

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

	// ==================== DNF 合成 ====================
	//
	// `+` (union)    : a の clauses + b の clauses (b の lit は index shift)
	// `*` (intersect): a × b の直積 (各組合せで lit を merge)
	// `-` (subtract) : a ∩ ¬b。¬b の DNF 化:
	//                  ¬(c1 ∨ c2 ∨ ...) = ¬c1 ∧ ¬c2 ∧ ...
	//                  各 ¬ci は c_i の各 lit を否定した OR なので、
	//                  全 ci から lit を 1 つずつ選び否定した AND の全パターンが
	//                  ¬b の DNF clause になる。

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
		// ¬b の DNF を構築する。b の各 clause c_i から lit を 1 つずつ選び否定した
		// AND の全パターンが ¬b の DNF。
		let b_clauses: Vec<Vec<i64>> = b.clauses
			.split(|&l| l == 0)
			.filter(|c| !c.is_empty())
			.map(|c| c.to_vec())
			.collect();
		if b_clauses.is_empty() {
			// b が空 = ⊥ なので ¬b = ⊤、a - b = a (恒等)
			return a;
		}
		// 全パターン: 各 b_clause から lit を 1 つずつ選ぶ
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
		// neg_b = 上記 accum を clauses 形式に展開した Boolean
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
		// `S::boolean` 経由でバックエンドの shallow copy (TShape identity 保存) を通す。
		// `self.solids.clone()` だと `S::clone()` (= OCCT では deep_copy) が走り
		// face id が変わってしまうため使えない。
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

// Solid は SolidStruct そのものに 1:1 で対応する具象型なので、
// 「Boolean<Self> から Self へ」の TryFrom は具象型側 (src/occt/solid.rs の
// Solid impl) で実装する。汎用 `impl<S: SolidStruct> TryFrom<Boolean<S>> for S`
// は orphan rule 違反 (`S` が外部型かもしれないという扱いで coherence チェックに
// 引っかかる) ため、`Boolean::build` メソッドだけ提供してユーザーは
// `(expr).build()?` を使う。
