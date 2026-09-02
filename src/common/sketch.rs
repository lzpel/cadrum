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

use crate::common::error::Error;
use glam::{DVec2, DVec4};
use std::ops::{Add, Mul, Neg, Sub};

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sketch(pub Vec<DVec4>);

impl Sketch {
	/// 内側 = 半径 `r` の円板内部 (`a=+1` の正準符号)。外側は `-Sketch::circle(..)`。
	pub fn circle(center: DVec2, r: f64) -> Sketch {
		Sketch(vec![DVec4::new(1.0, -2.0 * center.x, -2.0 * center.y, center.length_squared() - r * r)])
	}

	/// 内側 = 有向線分 `a→b` の左側 (半径∞の退化円板)。`line(b, a)` は補元。
	///
	/// 一般化円板は正の定数倍で不変なので、`circle` の `a=+1` と対にして `|b|=1` に正準化する。
	/// 揃えないと同じ直線が線分長ぶんスケールの違うリテラルになり (L 字の 2 腕が共有する辺など)、
	/// `boundary` の重複除去も `side` の同一曲線判定もビット等価なので素通りしてしまう。
	pub fn line(a: DVec2, b: DVec2) -> Sketch {
		let d = (b - a).normalize();
		Sketch(vec![DVec4::new(0.0, d.y, -d.x, d.x * a.y - d.y * a.x)])
	}

	/// 点 `p` が領域に含まれるか。DNF を評価 (節内 AND・節間 OR)。空 `Sketch` = ⊥ = false。
	pub fn contains(&self, p: DVec2) -> bool {
		clauses(&self.0).any(|clause| clause.iter().all(|d| eval(*d, p) <= 0.0))
	}

	/// 領域が有界か。和集合なので全節が有界なときだけ有界。`boundary` に渡せるかの事前判定で、
	/// 半平面など非有界な中間結果は式の途中では自由に使える (OCCT に降ろす直前だけ有界を要求)。
	///
	/// `x = x₀ + R·u` を代入すると `eval = a·R² + R·(2a(x₀·u) + b·u) + 定数` なので、R→∞ の符号は
	/// `a` だけで決まる: `a>0` (真の円板) は全方向の無限遠を排除、`a<0` (円の外側) は全方向を許すので
	/// 有界化に寄与せず、`a=0` (半平面) は `b·u ≤ 0` の向きだけを残す。よって節が有界 ⇔ 生き残る u が
	/// 無い。後退錐 `{u : b_i·u ≤ 0}` の端 ray は 2D では必ずどれかの `b` に直交するので、
	/// `±perp(b_j)` を候補に総当たりすれば厳密判定できる (三角関数も許容誤差も不要)。
	///
	/// 既知の穴: 後退錐が非自明な**空**の節 (`y≥0 ∧ y≤-1`) は空ではなく非有界と報告する。
	pub fn bounded(&self) -> bool {
		clauses(&self.0).all(|clause| {
			if clause.iter().any(|d| d.x > 0.0) {
				return true; // 真の円板が 1 つあれば、節はその内部に収まる
			}
			let bs: Vec<DVec2> = clause.iter().filter(|d| d.x == 0.0).map(|d| DVec2::new(d.y, d.z)).collect();
			if bs.is_empty() {
				return false; // 円の外側だけ = 全方向が無限遠へ抜ける
			}
			let escapes = |u: DVec2| u != DVec2::ZERO && bs.iter().all(|b| b.dot(u) <= 0.0);
			!bs.iter().flat_map(|b| [DVec2::new(-b.y, b.x), DVec2::new(b.y, -b.x)]).any(escapes)
		})
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

// 演算子 (`+` `*` `-` 単項 `-`) の実体。公開面は演算子だけなので pub にはしない。
impl Sketch {
	/// `self ∪ b`。空は union の単位元 (⊥)。
	fn union(self, b: Sketch) -> Sketch {
		if self.0.is_empty() {
			return b;
		}
		if b.0.is_empty() {
			return self;
		}
		let mut v = self.0;
		v.push(DVec4::ZERO);
		v.extend(b.0);
		Sketch(v)
	}

	/// `self ∩ b`。節の直積。どちらか空 (⊥) なら空 (annihilator)。
	fn intersect(self, b: Sketch) -> Sketch {
		let ca: Vec<&[DVec4]> = clauses(&self.0).collect();
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

	/// `¬self`。ド・モルガンで各節から `-lit` を 1 つずつ選ぶ全組み合わせ (DNF へ再分配)。
	/// `¬⊥ = ⊤` は表現不可 (モジュール doc 参照) なので空入力は空 (⊥) を返す。
	fn complement(self) -> Sketch {
		let cs: Vec<&[DVec4]> = clauses(&self.0).collect();
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

	/// `self - b = self ∩ ¬b`。`b = ⊥` なら `¬b = ⊤` で `self` を返す。
	fn subtract(self, b: Sketch) -> Sketch {
		if b.0.is_empty() {
			return self;
		}
		self.intersect(b.complement())
	}
}

// ==================== 演算子 ====================

impl Add for Sketch {
	type Output = Sketch;
	fn add(self, rhs: Sketch) -> Sketch {
		self.union(rhs)
	}
}

impl Mul for Sketch {
	type Output = Sketch;
	fn mul(self, rhs: Sketch) -> Sketch {
		self.intersect(rhs)
	}
}

impl Sub for Sketch {
	type Output = Sketch;
	fn sub(self, rhs: Sketch) -> Sketch {
		self.subtract(rhs)
	}
}

impl Neg for Sketch {
	type Output = Sketch;
	fn neg(self) -> Sketch {
		self.complement()
	}
}

// ==================== 境界抽出 (arrangement) ====================

/// 生成側でループにトレースした境界セグメント列。`None` がループ区切り番兵。
/// `end` は持たず「次セグメントの `start`」(ループ末尾は先頭の `start`)。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Segment {
	Line { start: DVec2 },                 // end = 次の start
	Arc { start: DVec2, mid: DVec2 },      // end = 次の start; mid で曲率・向き
	Circle { center: DVec2, radius: f64 }, // 自己閉ループ; radius<0 = 穴(CW), >0 = 外周(CCW)
	None,                                  // EdgeLoop 区切り
}

/// 有向境界ピース (頂点 index 参照)。全円は別扱い。
struct Piece {
	start: usize,
	end: usize,
	mid: Option<DVec2>, // Some=弧の中点、None=線分
}

/// 2 つの一般化円板の交点 (0/1/2 点)。
fn intersect_disks(k: DVec4, j: DVec4) -> Vec<DVec2> {
	let (ka, ja) = (k.x, j.x);
	if ka == 0.0 && ja == 0.0 {
		line_line(k, j)
	} else if ka == 0.0 {
		line_circle(k, j)
	} else if ja == 0.0 {
		line_circle(j, k)
	} else {
		// circle∩circle: 根軸 (radical line, |x|² 項を消去した直線 ja·k − ka·j) と円 k の交点。
		// 交点は両円上にあり l=0 も満たす。逆に l=0 ∧ k=0 なら −ka·j=0 (ka≠0) で j=0 となり両円上。
		let l = DVec4::new(0.0, ja * k.y - ka * j.y, ja * k.z - ka * j.z, ja * k.w - ka * j.w);
		line_circle(l, k)
	}
}

/// 2 直線 (`a=0`) の交点。
fn line_line(l1: DVec4, l2: DVec4) -> Vec<DVec2> {
	let (b1, b2) = (DVec2::new(l1.y, l1.z), DVec2::new(l2.y, l2.z));
	let det = b1.x * b2.y - b1.y * b2.x;
	if det == 0.0 {
		return Vec::new(); // 平行 (交点なし)
	}
	let x = (-l1.w * b2.y + b1.y * l2.w) / det;
	let y = (-b1.x * l2.w + l1.w * b2.x) / det;
	vec![DVec2::new(x, y)]
}

/// 直線 `l` (`a=0`) と円 `c` (`a≠0`) の交点。
fn line_circle(l: DVec4, c: DVec4) -> Vec<DVec2> {
	let lb = DVec2::new(l.y, l.z);
	let bl = lb.length();
	if bl == 0.0 {
		return Vec::new(); // 退化した直線 (法線ゼロ) — 同心円の根軸など
	}
	let p0 = lb * (-l.w / (bl * bl)); // 原点最近点
	let dir = DVec2::new(-l.z, l.y) / bl; // 直線方向 (単位)
	let cb = DVec2::new(c.y, c.z);
	// c.x t² + qb t + qc = 0
	let qb = 2.0 * c.x * p0.dot(dir) + cb.dot(dir);
	let qc = c.x * p0.length_squared() + cb.dot(p0) + c.w;
	let disc = qb * qb - 4.0 * c.x * qc;
	if disc < 0.0 {
		Vec::new()
	} else if disc == 0.0 {
		vec![p0 + dir * (-qb / (2.0 * c.x))] // 接する (1 点)
	} else {
		let s = disc.sqrt();
		vec![p0 + dir * ((-qb - s) / (2.0 * c.x)), p0 + dir * ((-qb + s) / (2.0 * c.x))]
	}
}

/// `pm` での membership を、リテラル `k` を満たす (inside) / 満たさない に固定して評価。
/// `k` と同一曲線の補元リテラル `-k` は逆側で満たす。他リテラルは非ゼロなので `pm` で厳密評価。
fn side(s: &Sketch, k: DVec4, pm: DVec2, inside: bool) -> bool {
	clauses(&s.0).any(|clause| {
		clause.iter().all(|l| {
			if *l == k {
				inside
			} else if *l == -k {
				!inside
			} else {
				eval(*l, pm) <= 0.0
			}
		})
	})
}

/// 点 `pm` (円板 `k` 上) が領域境界か: 曲線 `k` を挟んで membership が変わるか (厳密・ずらし不要)。
fn on_boundary(s: &Sketch, k: DVec4, pm: DVec2) -> bool {
	side(s, k, pm, true) != side(s, k, pm, false)
}

/// `Sketch` (DNF) の境界を、生成側でループにトレースしたセグメント列に降ろす。
/// 円・直線のみ対応。非有界領域・退化は `Err(InvalidEdge)`。第一版は各頂点 degree 2 前提。
pub(crate) fn boundary(s: &Sketch) -> Result<Vec<Segment>, Error> {
	// 有界性は境界を追う前に式から決まる (境界が有界でも領域が非有界な形もここで捕まる)。
	if !s.bounded() {
		return Err(Error::InvalidEdge("unbounded sketch region".into()));
	}
	// arrangement は「相異なる曲線」の上に組む。同じ曲線が複数のリテラルとして現れても
	// (共線辺・(a+b)*c の展開) 1 本として扱わないと、同一点が別 index の頂点に化けて連結が切れる。
	// `d` と `-d` は同じ曲線の裏表なので 1 本に畳む (凸ピースの union が接する辺がこれ)。
	let mut disks: Vec<DVec4> = Vec::new();
	for d in s.0.iter().copied().filter(|d| *d != DVec4::ZERO) {
		if !disks.iter().any(|&e| e == d || e == -d) {
			disks.push(d);
		}
	}
	// 全ペア交点を一度だけ計算し、頂点 index を両円板で共有 (連結を index 一致で判定)。
	let mut verts: Vec<DVec2> = Vec::new();
	let mut on_disk: Vec<Vec<usize>> = vec![Vec::new(); disks.len()];
	for i in 0..disks.len() {
		for jj in (i + 1)..disks.len() {
			for pt in intersect_disks(disks[i], disks[jj]) {
				let idx = verts.len();
				verts.push(pt);
				on_disk[i].push(idx);
				on_disk[jj].push(idx);
			}
		}
	}

	let mut pieces: Vec<Piece> = Vec::new();
	let mut full_circles: Vec<(DVec2, f64)> = Vec::new();

	for (ki, &k) in disks.iter().enumerate() {
		if k.x == 0.0 {
			// 直線 (半平面)
			let lb = DVec2::new(k.y, k.z);
			let bl = lb.length();
			if bl == 0.0 {
				continue; // 退化した直線 (法線ゼロ)
			}
			let p0 = lb * (-k.w / (bl * bl));
			let dir = DVec2::new(-k.z, k.y) / bl;
			let mut ts: Vec<(f64, usize)> = on_disk[ki].iter().map(|&vi| ((verts[vi] - p0).dot(dir), vi)).collect();
			ts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
			if ts.len() < 2 {
				continue; // 有界領域の境界は頂点間の線分。頂点 2 個未満の直線は寄与しない
			}
			let n = lb / bl;
			for w in 0..ts.len() - 1 {
				let (t0, i0) = ts[w];
				let (t1, i1) = ts[w + 1];
				let pm = p0 + dir * (0.5 * (t0 + t1));
				if on_boundary(s, k, pm) {
					// 材料が k の内側 (−n 側) か外側かは区間ごとに違う — 同じ曲線が k と ¬k の
					// 両方のリテラルとして現れる union があるので、決め打ちできない。
					let mat = if side(s, k, pm, true) { -n } else { n };
					// 材料が進行方向左に来る向き
					let left = DVec2::new(-dir.y, dir.x);
					if left.dot(mat) > 0.0 {
						pieces.push(Piece { start: i0, end: i1, mid: None });
					} else {
						pieces.push(Piece { start: i1, end: i0, mid: None });
					}
				}
			}
		} else {
			// 円
			let center = DVec2::new(-k.y / (2.0 * k.x), -k.z / (2.0 * k.x));
			let r2 = (k.y * k.y + k.z * k.z - 4.0 * k.x * k.w) / (4.0 * k.x * k.x);
			if r2 <= 0.0 {
				return Err(Error::InvalidEdge("degenerate circle in sketch".into()));
			}
			let r = r2.sqrt();
			let mut vs: Vec<(f64, usize)> = on_disk[ki].iter().map(|&vi| ((verts[vi].y - center.y).atan2(verts[vi].x - center.x), vi)).collect();
			if vs.is_empty() {
				let pm = center + DVec2::new(r, 0.0);
				if on_boundary(s, k, pm) {
					// 材料が円板の内側なら外周 (CCW, radius>0)、外側なら穴 (CW, radius<0)。
					let in_disk = side(s, k, pm, true) == (k.x > 0.0);
					full_circles.push((center, if in_disk { r } else { -r }));
				}
				continue;
			}
			vs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
			let m = vs.len();
			for w in 0..m {
				let (a0, i0) = vs[w];
				let (a1, i1) = vs[(w + 1) % m];
				// a0→a1 を CCW (角度増加) に辿る弧の中点角
				let am = if a1 > a0 { 0.5 * (a0 + a1) } else { 0.5 * (a0 + a1 + std::f64::consts::TAU) };
				let pm = center + DVec2::new(r * am.cos(), r * am.sin());
				if on_boundary(s, k, pm) {
					// 材料が円板の内側なら CCW (角度増加方向)、外側なら穴で CW。直線と同じく
					// 区間ごとに決める (k と ¬k が同居しうるので a の符号だけでは決まらない)。
					let in_disk = side(s, k, pm, true) == (k.x > 0.0);
					if in_disk {
						pieces.push(Piece { start: i0, end: i1, mid: Some(pm) });
					} else {
						pieces.push(Piece { start: i1, end: i0, mid: Some(pm) });
					}
				}
			}
		}
	}

	// 有向ピースを共有頂点 index でループにトレース (degree 2 前提)。
	let mut used = vec![false; pieces.len()];
	let mut loop_segs: Vec<Vec<Segment>> = Vec::new();
	for seed in 0..pieces.len() {
		if used[seed] {
			continue;
		}
		let mut seg_loop: Vec<Segment> = Vec::new();
		let mut cur = seed;
		loop {
			used[cur] = true;
			let pc = &pieces[cur];
			seg_loop.push(match pc.mid {
				Some(mid) => Segment::Arc { start: verts[pc.start], mid },
				None => Segment::Line { start: verts[pc.start] },
			});
			let end_v = pc.end;
			match (0..pieces.len()).find(|&p| !used[p] && pieces[p].start == end_v) {
				Some(nx) => cur = nx,
				None => break,
			}
		}
		loop_segs.push(seg_loop);
	}
	for (center, radius) in full_circles {
		loop_segs.push(vec![Segment::Circle { center, radius }]);
	}

	// ループを `None` 区切りで連結。
	let mut out: Vec<Segment> = Vec::new();
	for (i, ls) in loop_segs.into_iter().enumerate() {
		if i > 0 {
			out.push(Segment::None);
		}
		out.extend(ls);
	}
	Ok(out)
}

#[cfg(test)]
mod tests {
	//! 向き・演算子・membership を `contains` の内外という不変量で、
	//! 境界抽出を `boundary()` のループ列 (種別数・`None` 区切り・向き) で検証する。

	use super::*;

	fn p(x: f64, y: f64) -> DVec2 {
		DVec2::new(x, y)
	}

	/// [-2,2] x [-1,1] の矩形 (4 半平面の積)。
	fn rect() -> Sketch {
		Sketch::line(p(-2.0, -1.0), p(2.0, -1.0)) * Sketch::line(p(2.0, -1.0), p(2.0, 1.0)) * Sketch::line(p(2.0, 1.0), p(-2.0, 1.0)) * Sketch::line(p(-2.0, 1.0), p(-2.0, -1.0))
	}

	/// CCW 頂点列 -> 凸多角形 (半平面の積)。
	fn convex(pts: &[DVec2]) -> Sketch {
		let mut s = Sketch::line(pts[pts.len() - 1], pts[0]);
		for w in pts.windows(2) {
			s = s * Sketch::line(w[0], w[1]);
		}
		s
	}

	fn count(segs: &[Segment], f: impl Fn(&Segment) -> bool) -> usize {
		segs.iter().filter(|s| f(s)).count()
	}

	#[test]
	fn boundary_single_circle() {
		let segs = boundary(&Sketch::circle(p(0.0, 0.0), 1.0)).unwrap();
		assert_eq!(segs.len(), 1);
		match segs[0] {
			Segment::Circle { radius, .. } => assert!(radius > 0.0), // 外周 = CCW = radius>0
			_ => panic!("expected a full circle"),
		}
	}

	#[test]
	fn boundary_rect_four_lines() {
		let segs = boundary(&rect()).unwrap();
		assert_eq!(count(&segs, |s| matches!(s, Segment::Line { .. })), 4);
		assert_eq!(count(&segs, |s| matches!(s, Segment::None)), 0); // 単一ループ
	}

	#[test]
	fn boundary_rect_minus_circle_hole() {
		let segs = boundary(&(rect() - Sketch::circle(p(0.0, 0.0), 0.5))).unwrap();
		assert_eq!(count(&segs, |s| matches!(s, Segment::Line { .. })), 4);
		assert_eq!(count(&segs, |s| matches!(s, Segment::None)), 1); // 2 ループ (外周 + 穴)
		let radii: Vec<f64> = segs.iter().filter_map(|s| if let Segment::Circle { radius, .. } = s { Some(*radius) } else { None }).collect();
		assert_eq!(radii.len(), 1);
		assert!(radii[0] < 0.0); // 穴 = CW = radius<0
	}

	#[test]
	fn boundary_two_circle_lens() {
		let segs = boundary(&(Sketch::circle(p(0.0, 0.0), 1.0) * Sketch::circle(p(1.0, 0.0), 1.0))).unwrap();
		assert_eq!(count(&segs, |s| matches!(s, Segment::Arc { .. })), 2);
		assert_eq!(count(&segs, |s| matches!(s, Segment::None)), 0); // 単一ループ
	}

	#[test]
	fn boundary_halfplane_is_unbounded() {
		assert!(boundary(&Sketch::line(p(0.0, 0.0), p(1.0, 0.0))).is_err());
	}

	/// 三角形 (0,1)(0,-1)(1,0) を CCW の 3 半平面で。
	fn tri() -> Sketch {
		Sketch::line(p(0.0, 1.0), p(0.0, -1.0)) * Sketch::line(p(0.0, -1.0), p(1.0, 0.0)) * Sketch::line(p(1.0, 0.0), p(0.0, 1.0))
	}

	#[test]
	fn boundary_complement_of_bounded_is_unbounded() {
		// 境界は三角形の 3 辺で有界だが領域は外側 = 非有界。境界レイを追う判定では捕れない。
		assert!(boundary(&-tri()).is_err());
		assert!(boundary(&-Sketch::circle(p(0.0, 0.0), 1.0)).is_err());
	}

	#[test]
	fn boundary_strip_is_unbounded() {
		// 平行 2 半平面。法線が真逆で後退錐は直線 = 非有界。
		let strip = Sketch::line(p(0.0, 0.0), p(1.0, 0.0)) * Sketch::line(p(1.0, 1.0), p(0.0, 1.0));
		assert!(boundary(&strip).is_err());
	}

	#[test]
	fn reversed_winding_is_empty_not_exterior() {
		// 辺を全部逆向きにすると ¬A∩¬B∩¬C。外側 (¬A∪¬B∪¬C) ではなく空集合。
		let rev = Sketch::line(p(0.0, -1.0), p(0.0, 1.0)) * Sketch::line(p(1.0, 0.0), p(0.0, -1.0)) * Sketch::line(p(0.0, 1.0), p(1.0, 0.0));
		assert!(!rev.contains(p(0.2, 0.0))); // 三角形の中でもない
		assert!(!rev.contains(p(50.0, 50.0))); // 遠方でもない = 空
		assert_eq!(boundary(&rev).unwrap().len(), 0);
	}

	#[test]
	fn boundary_collinear_edge_is_one_curve() {
		// 辺上の共線頂点 (0,0) は 2 辺を同一リテラルに畳む。曲線 1 本として扱わないと
		// 同じ点が別 index の頂点に化けてループが割れる。
		let pts = [p(0.0, 1.0), p(0.0, 0.0), p(0.0, -1.0), p(1.0, 0.0)];
		let mut s = Sketch::line(pts[3], pts[0]);
		for w in pts.windows(2) {
			s = s * Sketch::line(w[0], w[1]);
		}
		let segs = boundary(&s).unwrap();
		assert_eq!(count(&segs, |x| matches!(x, Segment::Line { .. })), 3);
		assert_eq!(count(&segs, |x| matches!(x, Segment::None)), 0); // 単一ループ
	}

	#[test]
	fn boundary_shared_literal_across_clauses() {
		// (a+b)*c は節 [a,c] [b,c] に展開され c が 2 度現れる。退化は無い。
		let a = Sketch::circle(p(-0.5, 0.0), 1.0);
		let b = Sketch::circle(p(0.5, 0.0), 1.0);
		let c = Sketch::circle(p(0.0, 0.0), 1.2);
		let segs = boundary(&((a + b) * c)).unwrap();
		// 右 ∂c / 上 ∂b / 上 ∂a / 左 ∂c / 下 ∂a / 下 ∂b
		assert_eq!(count(&segs, |x| matches!(x, Segment::Arc { .. })), 6);
		assert_eq!(count(&segs, |x| matches!(x, Segment::None)), 0); // 単一ループ
	}

	#[test]
	fn line_is_normalized() {
		// 同じ直線を長さ違いの線分から作っても同一リテラルになること。
		let short = Sketch::line(p(0.0, 0.0), p(1.0, 0.0));
		let long = Sketch::line(p(0.0, 0.0), p(4.0, 0.0));
		assert_eq!(short.0, long.0);
		let b = DVec2::new(short.0[0].y, short.0[0].z);
		assert!((b.length() - 1.0).abs() < 1e-15);
	}

	#[test]
	fn boundary_union_sharing_an_edge() {
		// [0,1]² ∪ [1,2]x[0,1] = [0,2]x[0,1]。共有辺 x=1 は片方が `x≤1`、もう片方が `x≥1` で
		// 符号が逆になる。同じ曲線なので arrangement には 1 本しか入ってはいけない (2 本入ると
		// 同じ点に頂点が二重にでき、長さ 0 の辺が生える)。
		//
		// 辺が 6 本なのは正しい: x=1 は境界ではないが曲線としては残るので、上辺と下辺を
		// そこで分割する。共線 2 本の連結であって、形は 2x1 の矩形。
		let r1 = convex(&[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)]);
		let r2 = convex(&[p(1.0, 0.0), p(2.0, 0.0), p(2.0, 1.0), p(1.0, 1.0)]);
		let segs = boundary(&(r1 + r2)).unwrap();
		assert_eq!(count(&segs, |x| matches!(x, Segment::Line { .. })), 6);
		assert_eq!(count(&segs, |x| matches!(x, Segment::None)), 0); // 単一ループ
															   // 長さ 0 の辺が無いこと (頂点の二重化の直接の症状)
		let pts: Vec<DVec2> = segs.iter().filter_map(|x| if let Segment::Line { start } = x { Some(*start) } else { None }).collect();
		assert!((0..pts.len()).all(|i| pts[i] != pts[(i + 1) % pts.len()]));
	}

	#[test]
	fn boundary_orientation_follows_material_per_segment() {
		// 上の union の外周は CCW (材料が進行方向左)。`k` と `¬k` が同居するので、材料側を
		// リテラルの符号から決め打ちすると向きが反転する。符号付き面積で確認。
		let r1 = convex(&[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)]);
		let r2 = convex(&[p(1.0, 0.0), p(2.0, 0.0), p(2.0, 1.0), p(1.0, 1.0)]);
		let segs = boundary(&(r1 + r2)).unwrap();
		let pts: Vec<DVec2> = segs.iter().filter_map(|x| if let Segment::Line { start } = x { Some(*start) } else { None }).collect();
		let area: f64 = (0..pts.len())
			.map(|i| {
				let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
				a.x * b.y - b.x * a.y
			})
			.sum::<f64>()
			/ 2.0;
		assert!((area - 2.0).abs() < 1e-9, "CCW の外周なら符号付き面積 +2、得た値 {area}");
	}

	#[test]
	fn circle_coefficients() {
		// 中心 (0,0) 半径 1 ⇒ |x|² - 1 ≤ 0。
		assert_eq!(Sketch::circle(p(0.0, 0.0), 1.0).0, vec![DVec4::new(1.0, 0.0, 0.0, -1.0)]);
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
}
