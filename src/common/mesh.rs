#[cfg(feature = "color")]
use super::color::Color;
use glam::{DVec2, DVec3};
use std::collections::HashMap;

/// A triangle mesh produced by meshing a solid shape.
///
/// All vectors have the same length: one entry per vertex.
/// `indices` contains triangle indices (groups of 3).
#[derive(Debug, Clone)]
pub struct Mesh {
	/// Vertex positions in 3D space.
	pub vertices: Vec<DVec3>,
	/// UV coordinates, normalized to [0, 1] per face.
	pub uvs: Vec<DVec2>,
	/// Triangle indices (groups of 3, referencing into `vertices`).
	pub indices: Vec<usize>,
	/// Per-triangle face ID. Length equals `indices.len() / 3`.
	pub face_ids: Vec<u64>,
	/// Per-face color map (face_id → Color).
	#[cfg(feature = "color")]
	pub colormap: HashMap<u64, Color>,
	/// Topological edge polylines, NaN-separated (same convention as
	/// `Scene2D::edges_visible`): a single `DVec3::NAN` separates consecutive
	/// polylines, e.g. `[p0, p1, p2, NaN, p3, p4]` is the two polylines
	/// `p0-p1-p2` and `p3-p4`. Populated at mesh time and consumed by
	/// `write_gltf_binary` (the 3D scene / SVG / PNG pipeline derives its own
	/// silhouette edges and does not read this field).
	pub edges: Vec<DVec3>,
}

/// 2D rendering scene derived from a `Mesh` viewed from a given camera.
///
/// Backend-agnostic intermediate: the projection / shading / silhouette /
/// occlusion pipeline produces this, and SVG / PNG / other backends consume it.
///
/// Invariants:
/// - `triangles.len() == color.len()`
/// - `triangles` is pre-sorted back-to-front (painter's algorithm)
/// - In `edges_visible` / `edges_hidden`, polylines are concatenated with a
///   single `DVec2::NAN` between them. `[p0, p1, p2, NaN, p3, p4]` means the
///   two polylines `p0-p1-p2` and `p3-p4`. No trailing NaN; leading/consecutive
///   NaNs are treated as empty polylines and ignored.
#[derive(Debug, Clone)]
pub struct Scene2D {
	/// Projected triangles (back-to-front draw order).
	pub triangles: Vec<[DVec2; 3]>,
	/// Per-triangle RGB byte color with shading already baked in.
	pub color: Vec<[u8; 3]>,
	/// Visible edge polylines, NaN-separated.
	pub edges_visible: Vec<DVec2>,
	/// Occluded edge polylines, NaN-separated. Empty when hidden lines were
	/// disabled at scene construction.
	pub edges_hidden: Vec<DVec2>,
}

/// Camera + rendering options for `Mesh::scene`.
///
/// Use `SceneOption::default()` for the standard Z-up isometric CAD view, and
/// override individual fields with struct update syntax, e.g.
/// `SceneOption { view: DVec3::new(1.0, 1.0, 2.0), shading: true, ..Default::default() }`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneOption {
	/// Camera direction (higher `dot(view)` = closer).
	pub view: DVec3,
	/// World-space up axis on the output. Gram-Schmidt-orthogonalized against
	/// `view`. Panics if zero or parallel to `view`.
	pub up: DVec3,
	/// Classify occluded edges into `Scene2D::edges_hidden`. When false, hidden
	/// edges are dropped entirely.
	pub hidden_edges: bool,
	/// Lambertian shading with light == `view`. On for curved shapes, off for
	/// flat CAD-style output.
	pub shading: bool,
}

impl Default for SceneOption {
	fn default() -> Self {
		Self { view: DVec3::ONE, up: DVec3::Z, hidden_edges: true, shading: false }
	}
}

impl Mesh {
	/// Write this mesh as binary STL to a writer.
	/// このメッシュをバイナリ STL 形式で書き出す。
	pub fn write_stl<W: std::io::Write>(&self, writer: &mut W) -> Result<(), super::error::Error> {
		let tri_count = self.indices.len() / 3;
		// 80-byte header
		writer.write_all(&[0u8; 80]).map_err(|_| super::error::Error::StlWriteFailed)?;
		// Triangle count (u32 LE)
		writer.write_all(&(tri_count as u32).to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?;
		for ti in 0..tri_count {
			let i0 = self.indices[ti * 3];
			let i1 = self.indices[ti * 3 + 1];
			let i2 = self.indices[ti * 3 + 2];
			let v0 = self.vertices[i0];
			let v1 = self.vertices[i1];
			let v2 = self.vertices[i2];
			// Face normal from cross product / 外積から面法線を計算
			let n = (v1 - v0).cross(v2 - v0).normalize_or_zero();
			// Normal (3 x f32 LE)
			for c in [n.x, n.y, n.z] {
				writer.write_all(&(c as f32).to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?;
			}
			// Vertices (3 x 3 x f32 LE)
			for v in [v0, v1, v2] {
				for c in [v.x, v.y, v.z] {
					writer.write_all(&(c as f32).to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?;
				}
			}
			// Attribute byte count — RGB555 color (SolidView/MeshLab convention)
			#[cfg(feature = "color")]
			let attr = self.colormap.get(&self.face_ids[ti]).map_or(0, Color::as_u16);
			#[cfg(not(feature = "color"))]
			let attr = 0u16;
			writer.write_all(&attr.to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?;
		}
		Ok(())
	}

	/// Write this mesh as binary glTF (`.glb`) to a writer.
	/// このメッシュをバイナリ glTF (GLB) 形式で書き出す。
	///
	/// Emits a single mesh whose primitives are:
	/// - triangles (`mode` TRIANGLES) — with the `color` feature, one primitive
	///   per distinct face color, each backed by a `KHR_materials_unlit`
	///   material (same-color faces share one material); otherwise a single
	///   uncolored primitive.
	/// - edges (`mode` LINES, `extras: {"cadrum":"edges"}`) built from
	///   `self.edges`, so viewers render the wireframe and `cadrum` readers can
	///   identify it.
	///
	/// Geometry lives in the GLB BIN chunk; JSON is hand-written (no serde
	/// dependency). `RWGltf_CafWriter` is intentionally not used.
	pub fn write_gltf_binary<W: std::io::Write>(&self, writer: &mut W) -> Result<(), super::error::Error> {
		use super::error::Error;
		let mut bin: Vec<u8> = Vec::new();
		let mut buffer_views: Vec<String> = Vec::new();
		let mut accessors: Vec<String> = Vec::new();
		let mut primitives: Vec<String> = Vec::new();
		#[allow(unused_mut)]
		let mut materials: Vec<String> = Vec::new();

		// ---- Triangle primitives (shared POSITION accessor) ----
		let tri_groups = self.gltf_triangle_groups(&mut materials);
		if !tri_groups.is_empty() && !self.vertices.is_empty() {
			let pos_acc = push_accessor_position(&mut buffer_views, &mut accessors, &mut bin, self.vertices.iter().map(|v| [v.x as f32, v.y as f32, v.z as f32]));

			for (indices, material) in tri_groups {
				if indices.is_empty() {
					continue;
				}
				let acc = push_accessor_index(&mut buffer_views, &mut accessors, &mut bin, &indices, self.vertices.len());
				let mat = material.map_or(String::new(), |m| format!(r#","material":{}"#, m));
				primitives.push(format!(r#"{{"attributes":{{"POSITION":{}}},"indices":{},"mode":4{}}}"#, pos_acc, acc, mat));
			}
		}

		// ---- Edge LINES primitive ----
		let (epos, eidx) = self.gltf_edge_buffers();
		if !eidx.is_empty() {
			let pos_acc = push_accessor_position(&mut buffer_views, &mut accessors, &mut bin, epos.iter().copied());
			let idx_acc = push_accessor_index(&mut buffer_views, &mut accessors, &mut bin, &eidx, epos.len());
			primitives.push(format!(r#"{{"attributes":{{"POSITION":{}}},"indices":{},"mode":1,"extras":{{"cadrum":"edges"}}}}"#, pos_acc, idx_acc));
		}

		// ---- Assemble JSON ----
		let mut members: Vec<String> = vec![r#""asset":{"version":"2.0","generator":"cadrum"}"#.to_string()];
		if !bin.is_empty() {
			members.push(format!(r#""buffers":[{{"byteLength":{}}}]"#, bin.len()));
		}
		if !buffer_views.is_empty() {
			members.push(format!(r#""bufferViews":[{}]"#, buffer_views.join(",")));
		}
		if !accessors.is_empty() {
			members.push(format!(r#""accessors":[{}]"#, accessors.join(",")));
		}
		if !materials.is_empty() {
			members.push(format!(r#""materials":[{}]"#, materials.join(",")));
			members.push(r#""extensionsUsed":["KHR_materials_unlit"]"#.to_string());
		}
		if !primitives.is_empty() {
			members.push(format!(r#""meshes":[{{"primitives":[{}]}}]"#, primitives.join(",")));
			members.push(r#""nodes":[{"mesh":0}]"#.to_string());
			members.push(r#""scenes":[{"nodes":[0]}]"#.to_string());
			members.push(r#""scene":0"#.to_string());
		}
		let json = format!("{{{}}}", members.join(","));

		// ---- GLB container (12-byte header + JSON chunk + optional BIN chunk) ----
		let mut json_bytes = json.into_bytes();
		while json_bytes.len() % 4 != 0 {
			json_bytes.push(b' ');
		}
		while bin.len() % 4 != 0 {
			bin.push(0);
		}

		let mut total = 12 + 8 + json_bytes.len();
		if !bin.is_empty() {
			total += 8 + bin.len();
		}

		let mut w = |b: &[u8]| writer.write_all(b).map_err(|_| Error::GltfWriteFailed);
		w(&0x46546C67u32.to_le_bytes())?; // magic "glTF"
		w(&2u32.to_le_bytes())?; // version 2
		w(&(total as u32).to_le_bytes())?;
		w(&(json_bytes.len() as u32).to_le_bytes())?;
		w(&0x4E4F534Au32.to_le_bytes())?; // chunk type "JSON"
		w(&json_bytes)?;
		if !bin.is_empty() {
			w(&(bin.len() as u32).to_le_bytes())?;
			w(&0x004E4942u32.to_le_bytes())?; // chunk type "BIN\0"
			w(&bin)?;
		}
		Ok(())
	}

	/// Group triangle indices by face color, appending one `KHR_materials_unlit`
	/// material per distinct color to `materials`. Returns `(index_buffer,
	/// Some(material_index))` per group. Faces without a color use a default
	/// gray material.
	#[cfg(feature = "color")]
	fn gltf_triangle_groups(&self, materials: &mut Vec<String>) -> Vec<(Vec<u32>, Option<usize>)> {
		const DEFAULT: [f32; 3] = [0.8667, 0.8667, 0.8667]; // 0xdd, matches scene fallback
		let tri_count = self.indices.len() / 3;
		let mut groups: Vec<(Vec<u32>, [f32; 3])> = Vec::new();
		let mut key_to_group: HashMap<[u32; 3], usize> = HashMap::new();
		for ti in 0..tri_count {
			let col = self.colormap.get(&self.face_ids[ti]).map_or(DEFAULT, |c| [c.r, c.g, c.b]);
			let key = [col[0].to_bits(), col[1].to_bits(), col[2].to_bits()];
			let g = *key_to_group.entry(key).or_insert_with(|| {
				groups.push((Vec::new(), col));
				groups.len() - 1
			});
			groups[g].0.extend_from_slice(&[self.indices[ti * 3] as u32, self.indices[ti * 3 + 1] as u32, self.indices[ti * 3 + 2] as u32]);
		}
		groups
			.into_iter()
			.map(|(indices, col)| {
				let mat = materials.len();
				materials.push(format!(r#"{{"pbrMetallicRoughness":{{"baseColorFactor":[{},{},{},1.0],"metallicFactor":0.0,"roughnessFactor":1.0}},"alphaMode":"OPAQUE","doubleSided":true,"extensions":{{"KHR_materials_unlit":{{}}}}}}"#, col[0], col[1], col[2]));
				(indices, Some(mat))
			})
			.collect()
	}

	/// Without the `color` feature: a single uncolored triangle group.
	#[cfg(not(feature = "color"))]
	fn gltf_triangle_groups(&self, _materials: &mut Vec<String>) -> Vec<(Vec<u32>, Option<usize>)> {
		let indices: Vec<u32> = self.indices.iter().map(|&i| i as u32).collect();
		if indices.is_empty() {
			Vec::new()
		} else {
			vec![(indices, None)]
		}
	}

	/// Expand the NaN-separated `edges` into a flat LINES position list plus a
	/// segment index buffer (consecutive `(i, i+1)` pairs within each polyline;
	/// NaN separators break the chain).
	fn gltf_edge_buffers(&self) -> (Vec<[f32; 3]>, Vec<u32>) {
		let mut pos: Vec<[f32; 3]> = Vec::new();
		let mut idx: Vec<u32> = Vec::new();
		let mut prev: Option<u32> = None;
		for p in &self.edges {
			if p.is_nan() {
				prev = None;
				continue;
			}
			let i = pos.len() as u32;
			pos.push([p.x as f32, p.y as f32, p.z as f32]);
			if let Some(pr) = prev {
				idx.push(pr);
				idx.push(i);
			}
			prev = Some(i);
		}
		(pos, idx)
	}

	/// Build a 2D scene from this mesh for the given camera.
	///
	/// See `SceneOption` for the camera (`view` / `up`) and rendering
	/// (`hidden_edges` / `shading`) parameters. `SceneOption::default()` is the
	/// standard Z-up isometric CAD view.
	///
	/// Render via `Scene2D::write_svg`.
	pub fn scene(&self, option: SceneOption) -> Scene2D {
		let SceneOption { view, up, hidden_edges, shading } = option;
		let (u, v, dir) = projection_basis(view, up);

		let (triangles, color) = project_and_sort_triangles(self, dir, u, v, shading);

		// `self.edges` holds topological edges for glTF export only; the 2D
		// scene derives its own view-dependent silhouette set.
		let silhouette_edges = detect_silhouette_edges(self, dir);
		let all_edges: Vec<&Vec<DVec3>> = silhouette_edges.iter().collect();

		// Even when hidden lines are not rendered, we still need to drop
		// occluded segments from the visible set — so always classify, then
		// throw away the hidden output when disabled.
		let occlusion_tris = build_occlusion_data(self, dir, u, v);
		let (edges_visible, hidden) = classify_edges(&all_edges, &occlusion_tris, dir, u, v);
		let edges_hidden = if hidden_edges { hidden } else { Vec::new() };

		Scene2D { triangles, color, edges_visible, edges_hidden }
	}
}

// ==================== glTF writer helpers ====================

/// Append `data` to the BIN buffer (4-byte aligned) and register a bufferView
/// over it. Returns the new bufferView index.
fn push_buffer_view(views: &mut Vec<String>, bin: &mut Vec<u8>, data: &[u8], target: u32) -> usize {
	while bin.len() % 4 != 0 {
		bin.push(0);
	}
	let offset = bin.len();
	bin.extend_from_slice(data);
	let idx = views.len();
	views.push(format!(r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":{}}}"#, offset, data.len(), target));
	idx
}

/// Append a POSITION buffer to BIN and register the VEC3 float accessor over it,
/// computing the glTF-required min/max in the same pass. Returns the accessor index.
fn push_accessor_position(views: &mut Vec<String>, accs: &mut Vec<String>, bin: &mut Vec<u8>, points: impl ExactSizeIterator<Item = [f32; 3]>) -> usize {
	let n = points.len();
	let mut bytes = Vec::with_capacity(n * 12);
	let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
	for p in points {
		for k in 0..3 {
			min[k] = min[k].min(p[k]);
			max[k] = max[k].max(p[k]);
		}
		for c in p {
			bytes.extend_from_slice(&c.to_le_bytes());
		}
	}
	let bv = push_buffer_view(views, bin, &bytes, 34962);
	let idx = accs.len();
	accs.push(format!(r#"{{"bufferView":{},"componentType":5126,"count":{},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}}"#, bv, n, min[0], min[1], min[2], max[0], max[1], max[2]));
	idx
}

/// Append an index buffer to BIN and register a SCALAR accessor over it. Uses
/// `UNSIGNED_SHORT` (componentType 5123, 2 bytes/index) when the referenced vertex
/// count fits in u16 (`<= 65535`), else `UNSIGNED_INT` (5125, 4 bytes). Halving the
/// index width shrinks the BIN chunk for the common small-mesh case (every cadrum
/// example qualifies). `push_buffer_view` 4-byte-aligns the start, which also
/// satisfies the 2-byte alignment `UNSIGNED_SHORT` requires.
fn push_accessor_index(views: &mut Vec<String>, accs: &mut Vec<String>, bin: &mut Vec<u8>, indices: &[u32], vertex_count: usize) -> usize {
	let (bytes, component_type) = if vertex_count <= u16::MAX as usize {
		let mut b = Vec::with_capacity(indices.len() * 2);
		for &i in indices {
			b.extend_from_slice(&(i as u16).to_le_bytes());
		}
		(b, 5123)
	} else {
		let mut b = Vec::with_capacity(indices.len() * 4);
		for &i in indices {
			b.extend_from_slice(&i.to_le_bytes());
		}
		(b, 5125)
	};
	let bv = push_buffer_view(views, bin, &bytes, 34963);
	let idx = accs.len();
	accs.push(format!(r#"{{"bufferView":{},"componentType":{},"count":{},"type":"SCALAR"}}"#, bv, component_type, indices.len()));
	idx
}

// ==================== Scene pipeline internals ====================

/// Per-triangle face normal from the cross product of its two edges.
/// Not normalized — callers that need a unit vector should normalize.
/// Sign convention matches the STL writer at `Mesh::write_stl`: outward-
/// pointing for face-orientation-consistent winding (which OCCT meshing
/// produces).
fn tri_normal(mesh: &Mesh, ti: usize) -> DVec3 {
	let i0 = mesh.indices[ti * 3];
	let i1 = mesh.indices[ti * 3 + 1];
	let i2 = mesh.indices[ti * 3 + 2];
	(mesh.vertices[i1] - mesh.vertices[i0]).cross(mesh.vertices[i2] - mesh.vertices[i0])
}

/// Projected front-facing triangle for occlusion testing.
struct OcclusionTri {
	pts: [DVec2; 3],
	depths: [f64; 3],
}

/// Build an orthonormal camera frame `(u, v, dir)` from user-supplied
/// `view` and `up`:
///
/// - `dir` = normalized `view` (points from the scene toward the camera)
/// - `v`   = `up` Gram-Schmidt-orthogonalized against `dir` and normalized
///           (the "up" axis on the output SVG)
/// - `u`   = `v × dir` (the "right" axis on the output SVG; right-handed)
///
/// Panics with a descriptive `expect` message when any input is degenerate
/// (`view` zero, `up` zero, or `up` parallel to `view`) — consistent with
/// `Transform::align_x` / `align_y` / `align_z` which also treat degenerate
/// geometric inputs as programmer errors rather than recoverable runtime
/// conditions.
fn projection_basis(view: DVec3, up: DVec3) -> (DVec3, DVec3, DVec3) {
	let dir = view.try_normalize().expect("write_svg: view is zero");
	let v = (up - dir * up.dot(dir)).try_normalize().expect("write_svg: up is zero or parallel to view");
	let u = v.cross(dir);
	(u, v, dir)
}

/// Project all front-facing triangles to 2D, compute per-triangle shaded
/// color, and return both vectors sorted back-to-front by centroid depth.
fn project_and_sort_triangles(mesh: &Mesh, dir: DVec3, u: DVec3, v: DVec3, shading: bool) -> (Vec<[DVec2; 3]>, Vec<[u8; 3]>) {
	let tri_count = mesh.indices.len() / 3;
	// Build with depth so we can sort, then strip it.
	let mut buf: Vec<([DVec2; 3], [u8; 3], f64)> = Vec::with_capacity(tri_count);

	for ti in 0..tri_count {
		let i0 = mesh.indices[ti * 3];
		let i1 = mesh.indices[ti * 3 + 1];
		let i2 = mesh.indices[ti * 3 + 2];

		let v0 = mesh.vertices[i0];
		let v1 = mesh.vertices[i1];
		let v2 = mesh.vertices[i2];

		let face_normal = tri_normal(mesh, ti);
		if face_normal.dot(dir) < 0.0 {
			continue;
		}

		let p0 = DVec2::new(v0.dot(u), v0.dot(v));
		let p1 = DVec2::new(v1.dot(u), v1.dot(v));
		let p2 = DVec2::new(v2.dot(u), v2.dot(v));

		let depth = (v0.dot(dir) + v1.dot(dir) + v2.dot(dir)) / 3.0;

		// Lambertian shading with head-on light (light direction == view direction).
		// Front-facing triangles get `normal · dir ∈ (0, 1]`; normalize to handle
		// the face normal's non-unit length. Shade maps [0, 1] → [0.5, 1.0]
		// so glancing faces darken to half-intensity (not black) — enough to
		// read the 3D shape without swallowing the silhouette into the stroke.
		// When `shading` is false, every triangle gets shade=1.0 → flat fill.
		let shade = if shading {
			let dot = face_normal.normalize_or_zero().dot(dir).clamp(0.0, 1.0);
			0.5 + 0.5 * dot
		} else {
			1.0
		};

		let gray = 0xdd as f64 / 255.0;
		#[cfg(feature = "color")]
		let (base_r, base_g, base_b) = {
			let face_id = mesh.face_ids[ti];
			if let Some(c) = mesh.colormap.get(&face_id) {
				(c.r as f64, c.g as f64, c.b as f64)
			} else {
				(gray, gray, gray)
			}
		};
		#[cfg(not(feature = "color"))]
		let (base_r, base_g, base_b) = (gray, gray, gray);

		let color = [(base_r * shade * 255.0) as u8, (base_g * shade * 255.0) as u8, (base_b * shade * 255.0) as u8];

		buf.push(([p0, p1, p2], color, depth));
	}

	buf.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

	let mut triangles = Vec::with_capacity(buf.len());
	let mut colors = Vec::with_capacity(buf.len());
	for (pts, color, _) in buf {
		triangles.push(pts);
		colors.push(color);
	}
	(triangles, colors)
}

/// Build projected front-facing triangles for occlusion testing.
fn build_occlusion_data(mesh: &Mesh, dir: DVec3, u: DVec3, v: DVec3) -> Vec<OcclusionTri> {
	let tri_count = mesh.indices.len() / 3;
	let mut tris = Vec::with_capacity(tri_count / 2);

	for ti in 0..tri_count {
		let i0 = mesh.indices[ti * 3];
		let i1 = mesh.indices[ti * 3 + 1];
		let i2 = mesh.indices[ti * 3 + 2];

		let v0 = mesh.vertices[i0];
		let v1 = mesh.vertices[i1];
		let v2 = mesh.vertices[i2];

		if tri_normal(mesh, ti).dot(dir) <= 0.0 {
			continue;
		}

		tris.push(OcclusionTri { pts: [DVec2::new(v0.dot(u), v0.dot(v)), DVec2::new(v1.dot(u), v1.dot(v)), DVec2::new(v2.dot(u), v2.dot(v))], depths: [v0.dot(dir), v1.dot(dir), v2.dot(dir)] });
	}
	tris
}

/// Detect silhouette edges from mesh triangle adjacency.
///
/// A silhouette edge is one where:
/// - One adjacent triangle faces the camera, the other faces away (contour edge)
/// - Only one adjacent triangle exists (boundary edge) and it faces the camera
fn detect_silhouette_edges(mesh: &Mesh, dir: DVec3) -> Vec<Vec<DVec3>> {
	let tri_count = mesh.indices.len() / 3;

	let mut edge_tris: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
	for ti in 0..tri_count {
		let i0 = mesh.indices[ti * 3];
		let i1 = mesh.indices[ti * 3 + 1];
		let i2 = mesh.indices[ti * 3 + 2];
		for &(a, b) in &[(i0, i1), (i1, i2), (i2, i0)] {
			let key = if a < b { (a, b) } else { (b, a) };
			edge_tris.entry(key).or_default().push(ti);
		}
	}

	let mut silhouettes = Vec::new();
	for (&(a, b), tris) in &edge_tris {
		let is_silhouette = if tris.len() == 1 {
			tri_facing(mesh, tris[0], dir)
		} else if tris.len() == 2 {
			tri_facing(mesh, tris[0], dir) != tri_facing(mesh, tris[1], dir)
		} else {
			false
		};
		if is_silhouette {
			silhouettes.push(vec![mesh.vertices[a], mesh.vertices[b]]);
		}
	}
	silhouettes
}

fn tri_facing(mesh: &Mesh, ti: usize, dir: DVec3) -> bool {
	tri_normal(mesh, ti).dot(dir) > 0.0
}

/// Classify edge segments as visible or hidden by occlusion against the
/// front-facing triangle set. Output: NaN-separated 2D polyline lists for
/// each class.
fn classify_edges(edges: &[&Vec<DVec3>], occlusion_tris: &[OcclusionTri], dir: DVec3, u: DVec3, v: DVec3) -> (Vec<DVec2>, Vec<DVec2>) {
	let mut visible: Vec<DVec2> = Vec::new();
	let mut hidden: Vec<DVec2> = Vec::new();

	for edge in edges {
		if edge.len() < 2 {
			continue;
		}

		let mut vis_line: Vec<DVec2> = Vec::new();
		let mut hid_line: Vec<DVec2> = Vec::new();

		for window in edge.windows(2) {
			let a3d = window[0];
			let b3d = window[1];
			let mid = (a3d + b3d) * 0.5;
			let mid_2d = DVec2::new(mid.dot(u), mid.dot(v));
			let mid_depth = mid.dot(dir);

			let a_2d = DVec2::new(a3d.dot(u), a3d.dot(v));
			let b_2d = DVec2::new(b3d.dot(u), b3d.dot(v));

			let is_hidden = is_point_occluded(mid_2d, mid_depth, occlusion_tris);

			if is_hidden {
				flush_polyline(&mut visible, &mut vis_line);
				if hid_line.is_empty() {
					hid_line.push(a_2d);
				}
				hid_line.push(b_2d);
			} else {
				flush_polyline(&mut hidden, &mut hid_line);
				if vis_line.is_empty() {
					vis_line.push(a_2d);
				}
				vis_line.push(b_2d);
			}
		}

		flush_polyline(&mut visible, &mut vis_line);
		flush_polyline(&mut hidden, &mut hid_line);
	}

	(visible, hidden)
}

/// Append a polyline (≥2 points) to a NaN-separated output buffer, then
/// clear the staging buffer. No-op if the polyline is shorter than 2 points.
fn flush_polyline(out: &mut Vec<DVec2>, staging: &mut Vec<DVec2>) {
	if staging.len() < 2 {
		staging.clear();
		return;
	}
	if !out.is_empty() {
		out.push(DVec2::NAN);
	}
	out.append(staging);
}

fn is_point_occluded(p: DVec2, point_depth: f64, tris: &[OcclusionTri]) -> bool {
	// Tolerance for self-occlusion: edge lies on the surface, so its depth
	// is approximately equal to the adjacent face's depth.
	let eps = 1e-4;

	for tri in tris {
		if let Some((w0, w1, w2)) = barycentric_2d(p, tri.pts) {
			let tri_depth = w0 * tri.depths[0] + w1 * tri.depths[1] + w2 * tri.depths[2];
			if tri_depth > point_depth + eps {
				return true;
			}
		}
	}
	false
}

/// Compute barycentric coordinates of point `p` in triangle `t` (2D).
/// Returns `Some((w0, w1, w2))` if the point is inside the triangle.
fn barycentric_2d(p: DVec2, t: [DVec2; 3]) -> Option<(f64, f64, f64)> {
	let denom = (t[1].y - t[2].y) * (t[0].x - t[2].x) + (t[2].x - t[1].x) * (t[0].y - t[2].y);
	if denom.abs() < 1e-12 {
		return None;
	}

	let w0 = ((t[1].y - t[2].y) * (p.x - t[2].x) + (t[2].x - t[1].x) * (p.y - t[2].y)) / denom;
	let w1 = ((t[2].y - t[0].y) * (p.x - t[2].x) + (t[0].x - t[2].x) * (p.y - t[2].y)) / denom;
	let w2 = 1.0 - w0 - w1;

	if w0 >= -1e-8 && w1 >= -1e-8 && w2 >= -1e-8 {
		Some((w0, w1, w2))
	} else {
		None
	}
}

// ==================== Scene2D layout (shared by backends) ====================

/// Viewport + stroke parameters derived from `Scene2D::viewbox`. Shared by
/// SVG and PNG backends so both honor the same margin / stroke / dash ratios.
/// All units are scene units; per-backend code converts to its target space
/// (SVG keeps scene units; PNG multiplies by pixels-per-scene-unit).
struct Layout {
	/// Output rect in scene-style coordinates with Y already flipped (origin
	/// at top-left, matching SVG `viewBox` and pixel image conventions).
	vx: f64,
	vy: f64,
	vw: f64,
	vh: f64,
	stroke_width: f64,
	hidden_stroke_width: f64,
	dash_len: f64,
	dash_gap: f64,
	/// AA policy for triangle *fills* (lines are always anti-aliased).
	/// `false` disables fill AA to kill the conflation seams between
	/// adjacent same-surface triangles (the meshy white cracks, #201).
	/// PNG maps this to `paint.anti_alias`; SVG to `shape-rendering`.
	anti_alias: bool,
}

impl Scene2D {
	/// Bounding box `[min, max]` of all projected triangle vertices.
	/// Falls back to `[0,1]×[0,1]` when the scene is empty. Edges always
	/// lie on the projected surface (= union of front-facing triangles),
	/// so they don't extend the bbox and are not scanned here.
	pub fn viewbox(&self) -> [DVec2; 2] {
		let init = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
		let (min, max) = self.triangles.iter().flatten().copied().fold(init, |(mn, mx), p| (mn.min(p), mx.max(p)));
		if min.x > max.x {
			[DVec2::ZERO, DVec2::ONE]
		} else {
			[min, max]
		}
	}

	fn layout(&self) -> Layout {
		let [min, max] = self.viewbox();
		let margin_frac = 0.05;
		let w = max.x - min.x;
		let h = max.y - min.y;
		let span = w.max(h);
		let margin = span * margin_frac;
		let stroke_width = span * 0.003;
		Layout {
			vx: min.x - margin,
			// SVG / image Y axis points down, so flip Y for the output rect.
			vy: -(max.y + margin),
			vw: w + margin * 2.0,
			vh: h + margin * 2.0,
			stroke_width,
			// Hidden lines: thinner stroke and longer dashes to reduce
			// visual clutter on dense models (e.g. helical sweeps).
			hidden_stroke_width: stroke_width * 0.6,
			dash_len: stroke_width * 5.0,
			dash_gap: stroke_width * 4.0,
			// Fixed noaa: disable fill AA to remove the conflation seams (#201).
			anti_alias: false,
		}
	}
}

// ==================== Scene2D → SVG backend ====================

impl Scene2D {
	/// Write this scene as an SVG to a writer.
	pub fn write_svg<W: std::io::Write>(&self, writer: &mut W) -> Result<(), super::error::Error> {
		let Layout { vx, vy, vw, vh, stroke_width: sw, hidden_stroke_width: hidden_sw, dash_len, dash_gap, anti_alias } = self.layout();

		let mut svg = String::with_capacity(4096 + self.triangles.len() * 120);
		svg.push_str(&format!(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vx:.4} {vy:.4} {vw:.4} {vh:.4}\" \
			 stroke-width=\"{sw:.4}\">\n"
		));

		// Fills: wrap in a group with `crispEdges` when AA is off so adjacent
		// same-surface triangles don't leave hairline gaps (#201). Lines stay
		// outside the group and keep default (anti-aliased) rendering.
		if !anti_alias {
			svg.push_str("<g shape-rendering=\"crispEdges\">\n");
		}
		for (tri, color) in self.triangles.iter().zip(self.color.iter()) {
			let [p0, p1, p2] = *tri;
			let [r, g, b] = *color;
			svg.push_str(&format!(
				"<polygon points=\"{:.4},{:.4} {:.4},{:.4} {:.4},{:.4}\" \
				 fill=\"#{r:02x}{g:02x}{b:02x}\" stroke=\"none\"/>\n",
				p0.x, -p0.y, p1.x, -p1.y, p2.x, -p2.y,
			));
		}
		if !anti_alias {
			svg.push_str("</g>\n");
		}

		polylines_to_svg(&mut svg, &self.edges_visible, "black", "", None);
		polylines_to_svg(&mut svg, &self.edges_hidden, "#bbb", &format!("{dash_len:.4},{dash_gap:.4}"), Some(hidden_sw));

		svg.push_str("</svg>\n");
		writer.write_all(svg.as_bytes()).map_err(|_| super::error::Error::SvgExportFailed)
	}
}

/// Walk a NaN-separated polyline buffer and emit one `<polyline>` per
/// segment of consecutive non-NaN points.
fn polylines_to_svg(svg: &mut String, polylines: &[DVec2], stroke: &str, dash: &str, width: Option<f64>) {
	let mut start = 0;
	for i in 0..=polylines.len() {
		let is_sep = i == polylines.len() || polylines[i].is_nan();
		if is_sep {
			let line = &polylines[start..i];
			if line.len() >= 2 {
				emit_polyline(svg, line, stroke, dash, width);
			}
			start = i + 1;
		}
	}
}

fn emit_polyline(svg: &mut String, line: &[DVec2], stroke: &str, dash: &str, width: Option<f64>) {
	svg.push_str("<polyline points=\"");
	for (i, p) in line.iter().enumerate() {
		if i > 0 {
			svg.push(' ');
		}
		svg.push_str(&format!("{:.4},{:.4}", p.x, -p.y));
	}
	svg.push_str("\" fill=\"none\" stroke=\"");
	svg.push_str(stroke);
	svg.push('"');
	if let Some(w) = width {
		svg.push_str(&format!(" stroke-width=\"{w:.4}\""));
	}
	if !dash.is_empty() {
		svg.push_str(" stroke-dasharray=\"");
		svg.push_str(dash);
		svg.push('"');
	}
	svg.push_str("/>\n");
}

// ==================== Scene2D → PNG backend ====================

impl Scene2D {
	/// Rasterize this scene as a PNG and write to a writer.
	///
	/// `dimensions` is `[width, height]` in pixels. The scene aspect ratio
	/// (from `viewbox`) is preserved by scaling to fit and centering — the
	/// remainder (when the requested aspect doesn't match the scene's) is
	/// transparent. Anti-aliased via `tiny-skia`. Background is transparent;
	/// composite over your desired color downstream if needed.
	#[cfg(feature = "png")]
	pub fn write_png<W: std::io::Write>(&self, dimensions: [usize; 2], writer: &mut W) -> Result<(), super::error::Error> {
		use tiny_skia::{Pixmap, Transform};

		let [width, height] = dimensions;
		let layout = self.layout();

		// Preserve aspect: pick the smaller per-axis scale so the whole
		// viewbox fits, then center the content within the pixmap.
		let pps = ((width as f64) / layout.vw).min((height as f64) / layout.vh);
		let off_x = (width as f64 - layout.vw * pps) / 2.0;
		let off_y = (height as f64 - layout.vh * pps) / 2.0;

		// Scene→pixel transform. SVG y was already flipped in `layout.vy`,
		// so the same `(vx, vy)` origin maps to pixel `(off_x, off_y)` once
		// we scale scene-y by `-pps`.
		let s = pps as f32;
		let tx = -(layout.vx as f32) * s + off_x as f32;
		let ty = -(layout.vy as f32) * s + off_y as f32;
		let transform = Transform::from_row(s, 0.0, 0.0, -s, tx, ty);

		let mut pixmap = Pixmap::new(width as u32, height as u32).ok_or(super::error::Error::PngExportFailed)?;
		self.render_to_pixmap(&mut pixmap, transform, layout.stroke_width as f32, layout.hidden_stroke_width as f32, layout.dash_len as f32, layout.dash_gap as f32, layout.anti_alias);

		let png_bytes = pixmap.encode_png().map_err(|_| super::error::Error::PngExportFailed)?;
		writer.write_all(&png_bytes).map_err(|_| super::error::Error::PngExportFailed)
	}

	/// Render this scene's triangles + edges into an existing pixmap with the
	/// given transform and stroke widths (in scene units — tiny-skia scales
	/// them to pixels via the transform). Used by both `write_png` and
	/// `Mesh::write_multiview_png` (which composites 4 of these into a grid).
	#[cfg(feature = "png")]
	pub(crate) fn render_to_pixmap(&self, pixmap: &mut tiny_skia::Pixmap, transform: tiny_skia::Transform, stroke_width: f32, hidden_stroke_width: f32, dash_len: f32, dash_gap: f32, anti_alias: bool) {
		use tiny_skia::{FillRule, Paint, PathBuilder, Stroke, StrokeDash};

		// Triangles (back-to-front, already sorted by Scene2D construction).
		for (tri, color) in self.triangles.iter().zip(self.color.iter()) {
			let mut pb = PathBuilder::new();
			pb.move_to(tri[0].x as f32, tri[0].y as f32);
			pb.line_to(tri[1].x as f32, tri[1].y as f32);
			pb.line_to(tri[2].x as f32, tri[2].y as f32);
			pb.close();
			if let Some(path) = pb.finish() {
				let mut paint = Paint::default();
				paint.set_color_rgba8(color[0], color[1], color[2], 255);
				// Fill AA per layout policy; off kills conflation seams (#201).
				paint.anti_alias = anti_alias;
				pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
			}
		}

		// Visible edges — solid black.
		let mut visible_paint = Paint::default();
		visible_paint.set_color_rgba8(0, 0, 0, 255);
		visible_paint.anti_alias = true;
		let visible_stroke = Stroke { width: stroke_width, ..Stroke::default() };
		Self::stroke_polylines(pixmap, &self.edges_visible, &visible_paint, &visible_stroke, transform);

		// Hidden edges — gray dashed.
		let mut hidden_paint = Paint::default();
		hidden_paint.set_color_rgba8(0xbb, 0xbb, 0xbb, 255);
		hidden_paint.anti_alias = true;
		let hidden_stroke = Stroke { width: hidden_stroke_width, dash: StrokeDash::new(vec![dash_len, dash_gap], 0.0), ..Stroke::default() };
		Self::stroke_polylines(pixmap, &self.edges_hidden, &hidden_paint, &hidden_stroke, transform);
	}

	#[cfg(feature = "png")]
	fn stroke_polylines(pixmap: &mut tiny_skia::Pixmap, polylines: &[DVec2], paint: &tiny_skia::Paint, stroke: &tiny_skia::Stroke, transform: tiny_skia::Transform) {
		let mut start = 0;
		for i in 0..=polylines.len() {
			let is_sep = i == polylines.len() || polylines[i].is_nan();
			if is_sep {
				let line = &polylines[start..i];
				if line.len() >= 2 {
					let mut pb = tiny_skia::PathBuilder::new();
					pb.move_to(line[0].x as f32, line[0].y as f32);
					for p in &line[1..] {
						pb.line_to(p.x as f32, p.y as f32);
					}
					if let Some(path) = pb.finish() {
						pixmap.stroke_path(&path, paint, stroke, transform, None);
					}
				}
				start = i + 1;
			}
		}
	}
}

// ==================== Mesh::write_multiview_png — fixed 4-view PNG ====================
//
// LLM 視覚フィードバック向けの「固定プロトコル」プレビュー。引数を取らず、Solid 1
// つを 4 視点 (ISO/TOP/FRONT/RIGHT) で 1024×1024 PNG にレンダリングする。すべての
// 視点は **同一スケール** で描かれ、viewbox は世界原点中心の `[-h, h]²` で固定
// (h は世界 AABB 角点を全 4 視点で投影した最大絶対座標)。原点との相対位置と相対
// スケールが画像から読み取れる。下部に単位なしの scale bar、各パネルに gnomon。

impl Mesh {
	/// Write a fixed-format 4-view preview PNG (1024×1024) to `writer`.
	///
	/// レイアウトは固定:
	/// - 2×2 グリッド: 左上 ISO, 右上 TOP, 左下 FRONT, 右下 RIGHT (Z-up を仮定)
	/// - 全ビュー共通スケール (原点中心 `[-h, h]²`)
	/// - 各パネルに world axis gnomon (右下)
	/// - 画像下部に単位なし scale bar ({1,2,5}×10^n の round value)
	///
	/// 引数チューニングが必要な用途では `Mesh::scene → Scene2D::write_png` を使う。
	/// この関数は LLM への現状確認画像生成という単一目的のための「固定プロトコル」。
	#[cfg(feature = "png")]
	pub fn write_multiview_png<W: std::io::Write>(&self, writer: &mut W) -> Result<(), super::error::Error> {
		use tiny_skia::{Pixmap, Transform};

		const IMG_SIZE: u32 = 1024;
		const H_SCALE: f64 = 1.05;
		const GNOMON_SIZE: f32 = 48.0;
		const GNOMON_INSET: f32 = 24.0;
		const TICK_SIZE: f32 = 12.0;
		const LABEL_SIZE: f32 = 16.0;

		// 4 view configs (Z-up convention): (view_dir, up).
		// `view` points FROM scene TOWARD camera per Mesh::scene's convention.
		//
		// パネル配置 (row-major: TL, TR, BL, BR) と視点ベクトルの対応:
		//
		//   ┌──────────┬──────────┐
		//   │ TL (1,1,1│ TR (0,0,1│
		//   │  = ISO ) │  = +Z 視点)│
		//   ├──────────┼──────────┤
		//   │ BL (1,0,0│ BR (0,1,0│
		//   │  = +X 視点)│  = +Y 視点)│
		//   └──────────┴──────────┘
		//
		// **反時計回りの読み順** (TL → BL → BR → TR) で視点が
		// `(1,1,1) → (1,0,0) → (0,1,0) → (0,0,1)` の cyclic 順になる:
		// ISO の後は X→Y→Z の世界軸を順に正面から見ることに対応し、
		// 工学規格 (第一/第三角法) ではなく **座標軸の cyclic 順** という
		// より基本的な不変量に揃えた配置。視点識別は gnomon で行うので
		// テキストラベルは持たない。
		//
		// **up の選択**: 各 ortho 視点の +X+Y+Z コーナーがグリッド中央 (ISO 側)
		// を向くよう up を選ぶ。BL/BR は up=+Z で自然に成立、TR (+Z 視点) のみ
		// up=-Y にして画面上 +Y を下向きにする必要がある。これにより 4 パネルの
		// part 配置がグリッド中央を中心とした鏡像構造になる。
		let views: [(DVec3, DVec3); 4] = [
			(DVec3::new(1.0, 1.0, 1.0), DVec3::Z), // TL: ISO
			(DVec3::Z, -DVec3::Y),                 // TR: +Z 視点 (up=-Y で内向き)
			(DVec3::X, DVec3::Z),                  // BL: +X 視点
			(DVec3::Y, DVec3::Z),                  // BR: +Y 視点
		];

		let bases: [(DVec3, DVec3, DVec3); 4] = std::array::from_fn(|i| projection_basis(views[i].0, views[i].1));

		// 4 視点の Scene2D を先に構築し、各々の `viewbox()` (= 実際に描画される
		// 前面三角形頂点の bbox) の絶対値最大から共通 h を導く。
		// 含意: 各パネルは原点中心の `[-h, h]²` を表示し、コンテンツは全パネルで必ず収まる。
		// 世界 AABB 角投影より tight (球面など曲面で part が panel いっぱいに描かれる)。
		let scenes: [Scene2D; 4] = std::array::from_fn(|i| self.scene(SceneOption { view: views[i].0, up: views[i].1, ..Default::default() }));
		let h = scenes.iter().map(|v| v.viewbox()).flat_map(|[a, b]| [a.x, a.y, b.x, b.y]).map(|x| x.abs() * H_SCALE).reduce(f64::max).unwrap_or(1.0);

		// 各パネルは正方形 512×512、4 パネル交点はちょうど画像中心 (512, 512)。
		// padding なし: part は panel 端まで使い切る。scale bar は y=512 の水平
		// パネル境界線に 2 つ埋め込む (フッター帯を持たない)。
		let panel_w = (IMG_SIZE as f32) / 2.0;
		let panel_h = (IMG_SIZE as f32) / 2.0;
		let pps = (panel_w.min(panel_h) as f64) / (2.0 * h);

		// 背景は透過。下流で任意色に composite できる。
		let mut pixmap = Pixmap::new(IMG_SIZE, IMG_SIZE).ok_or(super::error::Error::PngExportFailed)?;

		// Per-panel stroke widths in scene units (tiny-skia transform scales them to px).
		let stroke_px = 1.5_f32;
		let scene_stroke = stroke_px / (pps as f32);
		let scene_hidden_stroke = scene_stroke * 0.6;
		let scene_dash_len = scene_stroke * 4.0;
		let scene_dash_gap = scene_stroke * 3.0;

		for (i, scene) in scenes.iter().enumerate() {
			let (col, row) = (i % 2, i / 2);
			let px0 = (col as f32) * panel_w;
			let py0 = (row as f32) * panel_h;
			let cx = px0 + panel_w / 2.0;
			let cy = py0 + panel_h / 2.0;

			// Scene→pixel transform: scene-y up → pixel-y down via the `-s` row.
			let s = pps as f32;
			let transform = Transform::from_row(s, 0.0, 0.0, -s, cx, cy);

			scene.render_to_pixmap(&mut pixmap, transform, scene_stroke, scene_hidden_stroke, scene_dash_len, scene_dash_gap, scene.layout().anti_alias);

			// Gnomon (bottom-right corner of panel) — also serves as view identifier.
			let (u_basis, v_basis, _) = bases[i];
			let g_origin = (px0 + panel_w - GNOMON_SIZE - GNOMON_INSET, py0 + panel_h - GNOMON_SIZE - GNOMON_INSET);
			draw_gnomon(&mut pixmap, g_origin, GNOMON_SIZE, LABEL_SIZE, u_basis, v_basis);
		}

		// 4 パネルを区切る十字線 (light gray)。中央の縦横 2 本だけ、外周は画像端と一致するので描かない。
		preview_path(&mut pixmap, [[0.0, panel_h, IMG_SIZE as f32, panel_h], [panel_w, 0.0, panel_w, IMG_SIZE as f32]], 0xcccccc, 1.0);

		// Scale bars embedded on the y=panel_h horizontal panel boundary.
		// 左半分にメインスケール、右半分にサブスケールを配置。大小 2 つの reference を
		// 与えることで LLM が任意長さを推定しやすくなる。
		//
		// 係数の理屈: bar_px = step × pps、pps = usable / (2h) なので step = 2h で
		// bar_px = usable (= padding 込みの最大幅)。よって 1.6h で ~80% 幅のメイン bar、
		// その半分 0.8h でサブ bar (nice_step は round-down なので bar は target 以下)。
		let step1 = nice_step(h * 1.6);
		let step2 = nice_step(h * 0.7);
		let boundary_y = panel_h; // = 512
		for (step, center_x) in [(step1, panel_w / 2.0), (step2, panel_w * 1.5)] {
			let bar_px = (step * pps) as f32;
			let x0 = center_x - bar_px / 2.0;
			let x1 = center_x + bar_px / 2.0;
			// scale bar: 横棒 + 両端 tick の 3 セグメント
			preview_path(&mut pixmap, [[x0, boundary_y, x1, boundary_y], [x0, boundary_y - TICK_SIZE / 2.0, x0, boundary_y + TICK_SIZE / 2.0], [x1, boundary_y - TICK_SIZE / 2.0, x1, boundary_y + TICK_SIZE / 2.0]], 0x1f3a8a, 2.0);
			let label = format!("{}", step);
			let glyph_w = LABEL_SIZE * 0.6;
			let text_w = (label.chars().count() as f32) * glyph_w * 1.2 - glyph_w * 0.2;
			draw_text(&mut pixmap, &label, center_x - text_w / 2.0, boundary_y - LABEL_SIZE - 4.0, LABEL_SIZE, 0x1f3a8a);
		}

		let png_bytes = pixmap.encode_png().map_err(|_| super::error::Error::PngExportFailed)?;
		writer.write_all(&png_bytes).map_err(|_| super::error::Error::PngExportFailed)
	}
}

// ==================== Preview helpers (scale / overlay drawing) ====================

/// Glyph/label size in pixels. Shared between scale-bar labels and gnomon axis labels.

/// Largest `{1, 2, 5} × 10^n` value ≤ `target` (round-down). Used for scale-bar
/// length so the bar is guaranteed not to exceed the requested target size.
fn nice_step(target: f64) -> f64 {
	if !target.is_finite() || target <= 0.0 {
		return 1.0;
	}
	let exp = target.log10().floor() as i32;
	let pow = 10f64.powi(exp);
	let m = target / pow;
	let nice = if m < 2.0 {
		1.0
	} else if m < 5.0 {
		2.0
	} else {
		5.0
	};
	nice * pow
}

// ---- Glyph paths (single polyline per glyph, in unit square; y=0 bottom, y=1 top) ----
//
// 各文字は **1 本のポリライン** で表現。出現しうる文字だけ収録。フォント crate を引かず
// PathBuilder の move_to/line_to だけで描く。
//
// - scale bar: `nice_step` が `{1,2,5} × 10^n` のみ返すため、format 結果に出る数字は `0, 1, 2, 5` と小数点 `.` の 5 種類。
// - gnomon: 世界軸ラベル `X, Y, Z` の 3 種類。
//
// 'X' と 'Y' は内部分岐があり厳密な Eulerian 一筆書きではないが、ポリライン上で中央
// を 2 度通る (重ね描き) ことで単一列に詰めている — AA 描画では重ね描きと 1 度描きが
// 視覚的に同一なので問題ない。'1' は base を持たず stem + flag のみで認識可能とした。
fn glyph_polyline(c: char) -> &'static [[f32; 2]] {
	match c {
		'0' => &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]],
		'1' => &[[0.2, 0.8], [0.5, 1.0], [0.5, 0.0]],
		'2' => &[[0.0, 1.0], [1.0, 1.0], [1.0, 0.5], [0.0, 0.5], [0.0, 0.0], [1.0, 0.0]],
		'5' => &[[1.0, 1.0], [0.0, 1.0], [0.0, 0.5], [1.0, 0.5], [1.0, 0.0], [0.0, 0.0]],
		'.' => &[[0.4, 0.0], [0.6, 0.0], [0.6, 0.15], [0.4, 0.15], [0.4, 0.0]],
		'X' => &[[0.0, 0.0], [1.0, 1.0], [0.5, 0.5], [0.0, 1.0], [1.0, 0.0]],
		'Y' => &[[0.0, 1.0], [0.5, 0.5], [0.5, 0.0], [0.5, 0.5], [1.0, 1.0]],
		'Z' => &[[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
		_ => &[],
	}
}

/// Stroke a list of line segments as a single anti-aliased path.
/// Preview UI 用の唯一の描画プリミティブ — gnomon / 十字線 / scale bar / glyph 文字
/// すべてこの 1 関数を経由する。`color` は `0xRRGGBB` (full alpha)。
#[cfg(feature = "png")]
fn preview_path(pixmap: &mut tiny_skia::Pixmap, segments: impl IntoIterator<Item = [f32; 4]>, color: u32, stroke_width: f32) {
	use tiny_skia::{Paint, PathBuilder, Stroke, Transform};
	let mut pb = PathBuilder::new();
	for [x0, y0, x1, y1] in segments {
		pb.move_to(x0, y0);
		pb.line_to(x1, y1);
	}
	let Some(path) = pb.finish() else { return };
	let mut paint = Paint::default();
	paint.set_color_rgba8(((color >> 16) & 0xff) as u8, ((color >> 8) & 0xff) as u8, (color & 0xff) as u8, 255);
	paint.anti_alias = true;
	let stroke = Stroke { width: stroke_width, ..Stroke::default() };
	pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

/// Draw `text` with top-left at `(x, y)` in pixel space. Glyph y is flipped
/// internally so y=1 in unit-space maps to the top of the line and y=0 to the
/// bottom (pixel-y points down).
#[cfg(feature = "png")]
fn draw_text(pixmap: &mut tiny_skia::Pixmap, text: &str, x: f32, y: f32, size: f32, color: u32) {
	let glyph_w = size * 0.6;
	let advance = glyph_w * 1.2;
	let mut segments: Vec<[f32; 4]> = Vec::new();
	let mut cursor = x;
	for ch in text.chars() {
		for w in glyph_polyline(ch).windows(2) {
			segments.push([cursor + w[0][0] * glyph_w, y + (1.0 - w[0][1]) * size, cursor + w[1][0] * glyph_w, y + (1.0 - w[1][1]) * size]);
		}
		cursor += advance;
	}
	preview_path(pixmap, segments, color, (size * 0.1).max(1.0));
}

/// Draw a 3-axis gnomon at `origin` (pixel coords, top-left of gnomon bounding
/// box). Each axis projects to 2D via `u`/`v` and is drawn as a short arrow
/// labeled X/Y/Z. Axes with near-zero projected length are skipped (they're
/// pointing into/out of the screen).
#[cfg(feature = "png")]
fn draw_gnomon(pixmap: &mut tiny_skia::Pixmap, origin: (f32, f32), size: f32, text_size: f32, u: DVec3, v: DVec3) {
	let cx = origin.0 + size / 2.0;
	let cy = origin.1 + size / 2.0;
	let axes: [(DVec3, &str, u32); 3] = [(DVec3::X, "X", 0xc0392b), (DVec3::Y, "Y", 0x27ae60), (DVec3::Z, "Z", 0x2980b9)];
	for (axis, label, color) in axes {
		let p = DVec2::new(axis.dot(u), axis.dot(v));
		let len = p.length();
		if len < 0.15 {
			// 軸が画面に対しほぼ垂直 → 点になるだけなので描画スキップ
			continue;
		}
		let ex = cx + (p.x as f32) * (size / 2.0);
		// pixel-y is down, scene-y is up → flip
		let ey = cy - (p.y as f32) * (size / 2.0);
		preview_path(pixmap, [[cx, cy, ex, ey]], color, 1.5);

		// Label past the arrow tip in the same direction.
		// label_off >= text_size/2 (= glyph 半高) でないと vertical な軸でラベルが線に被る。
		// text_size と同値にして 6 px 程度のクリアランスを確保する。
		let label_off = text_size;
		let dir_x = (ex - cx) / (len.max(1e-9) as f32 * (size / 2.0));
		let dir_y = (ey - cy) / (len.max(1e-9) as f32 * (size / 2.0));
		let lx = ex + dir_x * label_off - text_size * 0.3;
		let ly = ey + dir_y * label_off - text_size * 0.5;
		draw_text(pixmap, label, lx, ly, text_size, color);
	}
}
