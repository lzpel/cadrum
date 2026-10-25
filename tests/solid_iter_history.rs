//! `Solid::iter_history`: every builder reports how the faces and edges of its result
//! descend from those of its operands, as `Lineage` entries.

use cadrum::{DVec3, Edge, Lineage, ProfileOrient, Solid};
use std::collections::HashSet;
use std::f64::consts::TAU;

/// 一辺 `side` の閉じた正方形プロファイル（extrude 用）。
fn square(side: f64) -> Vec<Edge> {
	Edge::polygon(&[DVec3::new(0.0, 0.0, 0.0), DVec3::new(side, 0.0, 0.0), DVec3::new(side, side, 0.0), DVec3::new(0.0, side, 0.0)]).expect("square polygon")
}

fn face_ids(s: &Solid) -> HashSet<u64> {
	s.iter_face().map(|f| f.id()).collect()
}

fn edge_ids(s: &Solid) -> HashSet<u64> {
	s.iter_edge().map(|e| e.id()).collect()
}

/// Every `post` names a face or edge of `s`.
fn assert_posts_in(s: &Solid) {
	let (faces, edges) = (face_ids(s), edge_ids(s));
	for l in s.iter_history() {
		if let Some(post) = l.post() {
			assert!(faces.contains(&post) || edges.contains(&post), "{l:?}: post is not a face or edge of the result");
		}
	}
}

/// 入力に面も辺も持たない演算（プリミティブ / grid ソースの builder）は履歴が空。
#[test]
fn test_no_sub_shape_source_ops_have_empty_history() {
	assert_eq!(Solid::cube(DVec3::ZERO, DVec3::splat(1.0)).iter_history().count(), 0, "cube");
	assert_eq!(Solid::sphere(1.0).iter_history().count(), 0, "sphere");

	let bspline = Solid::bspline(16, 8, true, true, |i, j| {
		let phi = TAU * i as f64 / 16.0;
		let theta = TAU * j as f64 / 8.0;
		let r = 3.0 + theta.cos();
		DVec3::new(r * phi.cos(), r * phi.sin(), theta.sin())
	})
	.expect("bspline torus");
	assert_eq!(bspline.iter_history().count(), 0, "bspline");
}

/// extrude: 各 profile 辺は近い側のキャップの辺に Modified（ワイヤ内のコピー）、側面と
/// 遠い側のキャップの辺に Generated。キャップ自体はどの辺からも生えない。
#[test]
fn test_extrude_history_names_wall_and_far_edge_per_profile_edge() {
	let profile = square(4.0);
	let prism = Solid::extrude(&profile, DVec3::Z * 3.0).expect("extrude");
	assert_posts_in(&prism);
	let (faces, edges) = (face_ids(&prism), edge_ids(&prism));
	let mut walls = HashSet::new();
	for e in &profile {
		let mine: Vec<Lineage> = prism.iter_history().filter(|l| l.src() == e.id()).collect();
		let generated_faces: Vec<u64> = mine
			.iter()
			.filter_map(|l| match l {
				Lineage::Generated { post, .. } if faces.contains(post) => Some(*post),
				_ => None,
			})
			.collect();
		let generated_edges: Vec<u64> = mine
			.iter()
			.filter_map(|l| match l {
				Lineage::Generated { post, .. } if edges.contains(post) => Some(*post),
				_ => None,
			})
			.collect();
		let modified_edges: Vec<u64> = mine
			.iter()
			.filter_map(|l| match l {
				Lineage::Modified { post, .. } if edges.contains(post) => Some(*post),
				_ => None,
			})
			.collect();
		assert_eq!(generated_faces.len(), 1, "one wall per profile edge: {mine:?}");
		assert_eq!(generated_edges.len(), 1, "one far-cap edge per profile edge: {mine:?}");
		assert_eq!(modified_edges.len(), 1, "one near-cap edge per profile edge: {mine:?}");
		assert_ne!(generated_edges[0], modified_edges[0]);
		walls.insert(generated_faces[0]);
	}
	assert_eq!(walls.len(), 4, "the four walls are distinct");
	let caps: Vec<&cadrum::Face> = prism.iter_face().filter(|f| !walls.contains(&f.id())).collect();
	assert_eq!(caps.len(), 2);
	for cap in caps {
		assert!((cap.area() - 16.0).abs() < 1e-9, "a face no profile edge generated is a cap");
	}
}

/// revolve / sweep / loft: 各 profile 辺から 1 つの面が Generated される。
#[test]
fn test_revolve_sweep_loft_generate_a_face_per_profile_edge() {
	let profile = square(2.0).into_iter().map(|e| e.translate(DVec3::X * 5.0)).collect::<Vec<_>>();
	let revolved = Solid::revolve(&profile, DVec3::ZERO, DVec3::Y, TAU / 2.0).expect("revolve");
	let swept = Solid::sweep(&profile, &[Edge::line(DVec3::ZERO, DVec3::Z * 5.0).expect("line")], ProfileOrient::Fixed).expect("sweep");
	let lower = [Edge::circle(3.0, DVec3::Z).expect("circle")];
	let upper = [Edge::circle(1.5, DVec3::Z).expect("circle").translate(DVec3::Z * 8.0)];
	let lofted = Solid::loft([&lower[..], &upper[..]], false).expect("loft");

	for (name, solid, srcs) in [("revolve", &revolved, profile.iter().collect::<Vec<_>>()), ("sweep", &swept, profile.iter().collect()), ("loft", &lofted, vec![&lower[0], &upper[0]])] {
		assert_posts_in(solid);
		let faces = face_ids(solid);
		for e in srcs {
			let walls = solid.iter_history().filter(|l| l.src() == e.id() && matches!(l, Lineage::Generated { post, .. } if faces.contains(post))).count();
			assert_eq!(walls, 1, "{name}: profile edge {} generates one face", e.id());
		}
	}
}

/// shell: cube の top を開けて内側 offset。残り 5 面は Modified（開口側で短くなる
/// 面がある）か Kept、除去された top は開口の縁 (同じ平面の額縁) へ Modified される。
#[test]
fn test_shell_history_maps_retained_faces_and_rim() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let original = face_ids(&cube);
	let top = cube.iter_face().last().expect("cube has faces");
	let top_id = top.id();
	let shelled = cube.shell(-1.0, [top]).expect("shell");
	assert_posts_in(&shelled);

	let faces = face_ids(&shelled);
	let face_lineage: Vec<Lineage> = shelled.iter_history().filter(|l| original.contains(&l.src())).collect();
	assert!(!face_lineage.is_empty(), "shell must populate history");
	let srcs: HashSet<u64> = face_lineage.iter().map(|l| l.src()).collect();
	assert_eq!(srcs.len(), 6, "5 retained faces and the removed top should map; got {}", srcs.len());
	let rim: Vec<u64> = face_lineage.iter().filter(|l| l.src() == top_id).filter_map(|l| l.post()).filter(|p| faces.contains(p)).collect();
	assert_eq!(rim.len(), 1, "the top maps to exactly the rim");
	let rim = shelled.iter_face().find(|f| f.id() == rim[0]).expect("rim is a result face");
	assert!((rim.area() - (100.0 - 64.0)).abs() < 1e-9, "rim is the 10x10 top minus the 8x8 opening: {}", rim.area());
}

/// fillet: cube の edge 1 本を fillet → その edge は Deleted されブレンド面を Generated、
/// edge を共有する 2 面が Modified、残り 4 面は Kept。
#[test]
fn test_fillet_history_generates_blend_from_edge() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let original = face_ids(&cube);
	let edge = cube.iter_edge().next().expect("cube has edges");
	let edge_id = edge.id();
	let adjacent: HashSet<u64> = cube.iter_face().filter(|f| f.iter_edge().any(|e| e.id() == edge_id)).map(|f| f.id()).collect();
	assert_eq!(adjacent.len(), 2, "a cube edge borders exactly 2 faces");

	let filleted = cube.fillet_edges(0.5, [edge]).expect("fillet");
	assert_posts_in(&filleted);
	let faces = face_ids(&filleted);

	let blend: Vec<u64> = filleted
		.iter_history()
		.filter_map(|l| match l {
			Lineage::Generated { src, post } if src == edge_id && faces.contains(&post) => Some(post),
			_ => None,
		})
		.collect();
	assert_eq!(blend.len(), 1, "the edge generates one blend face");
	assert!(filleted.iter_history().any(|l| l == Lineage::Deleted { src: edge_id }), "the filleted edge is gone");
	let blend = filleted.iter_face().find(|f| f.id() == blend[0]).expect("blend is a result face");
	assert!(blend.surface().is_ok_and(|s| matches!(s.kind, cadrum::SurfaceKind::Cylinder { radius } if (radius - 0.5).abs() < 1e-9)), "the blend is a cylinder of the fillet radius");

	for id in &original {
		let mine: Vec<Lineage> = filleted.iter_history().filter(|l| l.src() == *id).collect();
		assert_eq!(mine.len(), 1, "one entry per original face: {mine:?}");
		if adjacent.contains(id) {
			assert!(matches!(mine[0], Lineage::Modified { .. }), "adjacent face {id} is Modified: {mine:?}");
		} else {
			assert!(matches!(mine[0], Lineage::Kept { .. } | Lineage::Modified { .. }), "far face {id} survives: {mine:?}");
		}
	}
}

/// chamfer: fillet と同形（辺は Deleted されて面を Generated、隣接 2 面が Modified）。
#[test]
fn test_chamfer_history_generates_bevel_from_edge() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let edge = cube.iter_edge().next().expect("cube has edges");
	let edge_id = edge.id();
	let adjacent: HashSet<u64> = cube.iter_face().filter(|f| f.iter_edge().any(|e| e.id() == edge_id)).map(|f| f.id()).collect();

	let chamfered = cube.chamfer_edges(0.5, [edge]).expect("chamfer");
	assert_posts_in(&chamfered);
	let faces = face_ids(&chamfered);
	assert_eq!(chamfered.iter_history().filter(|l| matches!(l, Lineage::Generated { src, post } if *src == edge_id && faces.contains(post))).count(), 1);
	assert!(chamfered.iter_history().any(|l| l == Lineage::Deleted { src: edge_id }));
	let modified: HashSet<u64> = chamfered
		.iter_history()
		.filter_map(|l| match l {
			Lineage::Modified { src, .. } => Some(src),
			_ => None,
		})
		.collect();
	assert!(adjacent.is_subset(&modified), "adjacent faces {adjacent:?} are Modified: {modified:?}");
}

/// color: fillet 後も Modified/Kept 面が src 面の色を history 経由で引き継ぐ。面色を
/// `colormap_mut` で直接置くのは、`Solid::color` がソリッド単位の色を塗るため。
#[test]
fn test_fillet_carries_face_color_via_history() {
	let red = "#ff0000".parse::<cadrum::Color>().expect("valid hex");
	let mut cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let ids: Vec<u64> = cube.iter_face().map(|f| f.id()).collect();
	cube.colormap_mut().extend(ids.into_iter().map(|id| (id, red)));

	let edge = cube.iter_edge().next().expect("cube has edges");
	let filleted = cube.fillet_edges(0.5, [edge]).expect("fillet");

	let faces = face_ids(&filleted);
	let from_faces: Vec<u64> = filleted.iter_history().filter(|l| cube.colormap().contains_key(&l.src())).filter_map(|l| l.post()).filter(|p| faces.contains(p)).collect();
	assert!(!from_faces.is_empty(), "fillet must populate history");
	for post in from_faces {
		assert!(filleted.colormap().contains_key(&post), "face {post} should inherit color via history");
	}
}

/// offset は全面でも一部の面でも、結果の全 face が history 経由で元の色を継ぐ。
#[test]
fn test_offset_carries_face_color_via_history() {
	let red = "#ff0000".parse::<cadrum::Color>().expect("valid hex");
	let mut cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let ids: Vec<u64> = cube.iter_face().map(|f| f.id()).collect();
	cube.colormap_mut().extend(ids.into_iter().map(|id| (id, red)));

	let top = cube.iter_face().last().expect("cube has faces");
	for (name, offset) in [("all faces", cube.offset(1.0, cube.iter_face()).expect("offset all")), ("top face", cube.offset(2.0, [top]).expect("offset top"))] {
		assert_posts_in(&offset);
		let post: Vec<u64> = offset.iter_face().map(|f| f.id()).collect();
		assert!(post.iter().all(|id| offset.colormap().get(id) == Some(&red)), "{name}: every face should inherit red");
	}
}

/// clean: 同一平面の 2 面を併合すると、両方の元面がその 1 面へ Modified される。
#[test]
fn test_clean_merges_coplanar_faces_as_modified() {
	let left = Solid::cube(DVec3::ZERO, DVec3::new(5.0, 10.0, 10.0));
	let right = Solid::cube(DVec3::new(5.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0));
	let fused = (&left + &right).build().expect("fuse");
	let cleaned = fused.clean().expect("clean");
	assert_posts_in(&cleaned);
	assert_eq!(cleaned.iter_face().count(), 6);
	let faces = face_ids(&cleaned);
	// UnifySameDomain rebuilds every face, so all are Modified; the four faces the fuse
	// split in two each descend from both halves.
	let mut srcs_of: std::collections::HashMap<u64, HashSet<u64>> = std::collections::HashMap::new();
	for l in cleaned.iter_history() {
		if let Lineage::Modified { src, post } = l {
			if faces.contains(&post) {
				srcs_of.entry(post).or_default().insert(src);
			}
		}
	}
	assert_eq!(srcs_of.len(), 6, "every result face descends from a fused face: {srcs_of:?}");
	assert_eq!(srcs_of.values().filter(|s| s.len() == 2).count(), 4, "four faces merge two halves: {srcs_of:?}");
}

/// boolean: 手つかずの面は Kept として元の面に結び付き、切られた面は Modified、断面は
/// tool の面から Modified、新しい辺は Generated される。
#[test]
fn test_boolean_keeps_untouched_faces_by_history() {
	let big = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let hole = Solid::cube(DVec3::new(3.0, 3.0, -1.0), DVec3::new(7.0, 7.0, 11.0));
	let (big_ids, hole_ids) = (face_ids(&big), face_ids(&hole));
	let cut = (&big - &hole).build().expect("cut");
	assert_posts_in(&cut);
	let faces = face_ids(&cut);

	let kept: HashSet<u64> = cut
		.iter_history()
		.filter_map(|l| match l {
			Lineage::Kept { src, post } if big_ids.contains(&src) && faces.contains(&post) => Some(post),
			_ => None,
		})
		.collect();
	assert_eq!(kept.len(), 4, "the four side faces of the block are untouched: {kept:?}");
	let modified: HashSet<u64> = cut
		.iter_history()
		.filter_map(|l| match l {
			Lineage::Modified { src, post } if big_ids.contains(&src) && faces.contains(&post) => Some(post),
			_ => None,
		})
		.collect();
	assert_eq!(modified.len(), 2, "top and bottom gain a hole: {modified:?}");
	let walls: HashSet<u64> = cut
		.iter_history()
		.filter_map(|l| match l {
			Lineage::Modified { src, post } | Lineage::Kept { src, post } if hole_ids.contains(&src) && faces.contains(&post) => Some(post),
			_ => None,
		})
		.collect();
	assert_eq!(walls.len(), 4, "the hole's four side faces become the bore walls: {walls:?}");
	assert_eq!(kept.len() + modified.len() + walls.len(), faces.len(), "every result face is accounted for");
	let section_edges = cut.iter_history().filter(|l| matches!(l, Lineage::Generated { src, .. } if big_ids.contains(src) || hole_ids.contains(src))).count();
	assert!(section_edges > 0, "the intersection curves are Generated from faces");
}

/// boolean が複数ピースに割れたとき、各ピースの history は自分の face/edge だけを
/// `post` に持つ（兄弟ピースの由来を主張しない）。Deleted は全ピースに付く。
#[test]
fn test_split_boolean_pieces_own_disjoint_history() {
	let block = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let slab = Solid::cube(DVec3::new(-1.0, -1.0, 4.0), DVec3::new(11.0, 11.0, 6.0));
	let pieces = (&block - &slab).build_vec().expect("cut splits the block");
	assert_eq!(pieces.len(), 2, "the slab must cut the block in two");

	// A post may descend from several srcs (a section edge from two faces), but never
	// from a sibling piece.
	let mut seen: HashSet<u64> = HashSet::new();
	for piece in &pieces {
		assert_posts_in(piece);
		let posts: HashSet<u64> = piece.iter_history().filter_map(|l| l.post()).collect();
		assert!(!posts.is_empty(), "a cut piece must keep its own history");
		assert!(seen.is_disjoint(&posts), "a post is claimed by more than one piece");
		seen.extend(posts);
	}
}

/// scale / mirror はトポロジを作り直すので、全面が Modified で結ばれ色が追従する。
#[test]
fn test_scale_and_mirror_report_modified_faces() {
	let red = "#ff0000".parse::<cadrum::Color>().expect("valid hex");
	let mut cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let ids: Vec<u64> = cube.iter_face().map(|f| f.id()).collect();
	cube.colormap_mut().extend(ids.iter().map(|&id| (id, red)));
	for (name, s) in [("scale", cube.clone().scale(DVec3::ZERO, 2.0)), ("mirror", cube.clone().mirror(DVec3::ZERO, DVec3::X))] {
		assert_posts_in(&s);
		let faces = face_ids(&s);
		let modified: HashSet<u64> = s
			.iter_history()
			.filter_map(|l| match l {
				Lineage::Modified { src, post } if ids.contains(&src) && faces.contains(&post) => Some(post),
				_ => None,
			})
			.collect();
		assert_eq!(modified, faces, "{name}: every face is Modified from an original face");
		assert!(faces.iter().all(|id| s.colormap().get(id) == Some(&red)), "{name}: colours follow");
	}
}
