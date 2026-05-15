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
	/// Topological edge polylines for SVG rendering.
	pub edges: Vec<Vec<DVec3>>,
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
	/// Bounding box `[min, max]` covering triangles and edges.
	pub viewbox: [DVec2; 2],
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
			for c in [n.x, n.y, n.z] { writer.write_all(&(c as f32).to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?; }
			// Vertices (3 x 3 x f32 LE)
			for v in [v0, v1, v2] {
				for c in [v.x, v.y, v.z] { writer.write_all(&(c as f32).to_le_bytes()).map_err(|_| super::error::Error::StlWriteFailed)?; }
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

	/// Render this mesh as an SVG.
	///
	/// - `view`: camera direction (higher `dot(view)` = closer).
	/// - `up`: world-space up axis on the output. Gram-Schmidt-orthogonalized
	///   against `view`. Panics if zero or parallel to `view`.
	/// - `hidden_lines`: render occluded edges as dashed lines. Off for dense
	///   models where they dominate.
	/// - `shading`: Lambertian shading with light == `view`. On for curved
	///   shapes, off for flat CAD-style output.
	pub fn write_svg<W: std::io::Write>(&self, view: DVec3, up: DVec3, hidden_lines: bool, shading: bool, writer: &mut W) -> Result<(), super::error::Error> {
		writer.write_all(self.to_svg(view, up, hidden_lines, shading).as_bytes()).map_err(|_| super::error::Error::SvgExportFailed)
	}

	pub fn to_svg(&self, view: DVec3, up: DVec3, hidden_lines: bool, shading: bool) -> String {
		self.scene(view, up, hidden_lines, shading).to_svg()
	}

	/// Build a 2D scene from this mesh for the given camera.
	/// Parameter semantics match `write_svg`.
	pub fn scene(&self, view: DVec3, up: DVec3, hidden_lines: bool, shading: bool) -> Scene2D {
		let (u, v, dir) = projection_basis(view, up);

		let (triangles, color) = project_and_sort_triangles(self, dir, u, v, shading);

		let silhouette_edges = detect_silhouette_edges(self, dir);
		let all_edges: Vec<&Vec<DVec3>> = self.edges.iter().chain(silhouette_edges.iter()).collect();

		// Even when hidden lines are not rendered, we still need to drop
		// occluded segments from the visible set — so always classify, then
		// throw away the hidden output when disabled.
		let occlusion_tris = build_occlusion_data(self, dir, u, v);
		let (edges_visible, hidden) = classify_edges(&all_edges, &occlusion_tris, dir, u, v);
		let edges_hidden = if hidden_lines { hidden } else { Vec::new() };

		let viewbox = compute_viewbox(&triangles, &edges_visible, &edges_hidden);

		Scene2D { triangles, color, edges_visible, edges_hidden, viewbox }
	}
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
	let v = (up - dir * up.dot(dir))
		.try_normalize()
		.expect("write_svg: up is zero or parallel to view");
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

		let color = [
			(base_r * shade * 255.0) as u8,
			(base_g * shade * 255.0) as u8,
			(base_b * shade * 255.0) as u8,
		];

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

		tris.push(OcclusionTri {
			pts: [DVec2::new(v0.dot(u), v0.dot(v)), DVec2::new(v1.dot(u), v1.dot(v)), DVec2::new(v2.dot(u), v2.dot(v))],
			depths: [v0.dot(dir), v1.dot(dir), v2.dot(dir)],
		});
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

/// Bounding box of all triangle vertices and edge points (skipping NaN
/// separators). Falls back to `[0,1]×[0,1]` when the scene is empty.
fn compute_viewbox(triangles: &[[DVec2; 3]], visible: &[DVec2], hidden: &[DVec2]) -> [DVec2; 2] {
	let mut min = DVec2::new(f64::INFINITY, f64::INFINITY);
	let mut max = DVec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);

	for tri in triangles {
		for p in tri {
			min = min.min(*p);
			max = max.max(*p);
		}
	}
	for p in visible.iter().chain(hidden.iter()) {
		if p.is_nan() {
			continue;
		}
		min = min.min(*p);
		max = max.max(*p);
	}

	if min.x > max.x {
		return [DVec2::ZERO, DVec2::ONE];
	}
	[min, max]
}

// ==================== Scene2D → SVG backend ====================

impl Scene2D {
	/// Render this scene as an SVG string.
	pub fn to_svg(&self) -> String {
		let [min, max] = self.viewbox;
		let margin_frac = 0.05;
		let w = max.x - min.x;
		let h = max.y - min.y;
		let span = w.max(h);
		let margin = span * margin_frac;
		let vx = min.x - margin;
		// SVG Y axis points down, so flip Y for output.
		let vy = -(max.y + margin);
		let vw = w + margin * 2.0;
		let vh = h + margin * 2.0;
		let sw = span * 0.003;
		// Hidden lines: thinner stroke and longer dashes to reduce visual clutter
		// on dense models (e.g. helical sweeps).
		let hidden_sw = sw * 0.6;
		let dash_len = sw * 5.0;
		let dash_gap = sw * 4.0;

		let mut svg = String::with_capacity(4096 + self.triangles.len() * 120);
		svg.push_str(&format!(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vx:.4} {vy:.4} {vw:.4} {vh:.4}\" \
			 stroke-width=\"{sw:.4}\">\n"
		));

		for (tri, color) in self.triangles.iter().zip(self.color.iter()) {
			let [p0, p1, p2] = *tri;
			let [r, g, b] = *color;
			svg.push_str(&format!(
				"<polygon points=\"{:.4},{:.4} {:.4},{:.4} {:.4},{:.4}\" \
				 fill=\"#{r:02x}{g:02x}{b:02x}\" stroke=\"none\"/>\n",
				p0.x, -p0.y, p1.x, -p1.y, p2.x, -p2.y,
			));
		}

		polylines_to_svg(&mut svg, &self.edges_visible, "black", "", None);
		polylines_to_svg(&mut svg, &self.edges_hidden, "#bbb", &format!("{dash_len:.4},{dash_gap:.4}"), Some(hidden_sw));

		svg.push_str("</svg>\n");
		svg
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
