//! Indexed triangle validation and explicit faceted BRep reconstruction.
use super::{
	ffi,
	report::{finite, kernel, positive},
};
use crate::{DVec3, Error, Shape};
use std::{
	collections::{HashMap, HashSet, VecDeque},
	io::Read,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshOutcome {
	MeshOnly,
	OpenShell,
	ClosedSolid,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct MeshRepair {
	pub weld: bool,
	pub remove_degenerate: bool,
	pub remove_duplicates: bool,
	pub orient_components: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct MeshImportOptions {
	pub outcome: MeshOutcome,
	pub repair: MeshRepair,
	pub weld_tolerance: f64,
	pub sewing_tolerance: f64,
	pub minimum_area: f64,
	pub max_vertices: usize,
	pub max_triangles: usize,
	pub max_input_bytes: usize,
}
impl Default for MeshImportOptions {
	fn default() -> Self {
		Self {
			outcome: MeshOutcome::MeshOnly,
			repair: MeshRepair::default(),
			weld_tolerance: 1e-7,
			sewing_tolerance: 1e-7,
			minimum_area: 1e-14,
			max_vertices: 3_000_000,
			max_triangles: 1_000_000,
			max_input_bytes: 256 * 1024 * 1024,
		}
	}
}
#[derive(Clone, Debug)]
pub struct TriangleMesh {
	pub positions: Vec<DVec3>,
	pub triangles: Vec<[usize; 3]>,
}
#[derive(Clone, Debug, Default)]
pub struct MeshImportReport {
	pub input_vertices: usize,
	pub input_triangles: usize,
	pub welded_vertices: usize,
	pub removed_degenerate: usize,
	pub removed_duplicates: usize,
	pub flipped_triangles: usize,
	pub components: usize,
	pub boundary_edges: Vec<[usize; 2]>,
	pub non_manifold_edges: Vec<[usize; 2]>,
	pub inconsistent_edges: Vec<[usize; 2]>,
	pub degenerate_triangles: Vec<usize>,
	pub duplicate_triangles: Vec<usize>,
	pub component_volumes: Vec<f64>,
}
#[derive(Debug)]
pub struct MeshImportResult {
	pub mesh: TriangleMesh,
	pub shape: Option<Shape>,
	pub report: MeshImportReport,
}
fn invalid(message: &str, subshapes: Vec<usize>) -> Error {
	Error::Operation { operation: "mesh import".into(), diagnostics: vec![message.into()], subshapes }
}
impl TriangleMesh {
	pub fn import(&self, options: MeshImportOptions) -> Result<MeshImportResult, Error> {
		positive(options.sewing_tolerance, "mesh sewing tolerance")?;
		positive(options.weld_tolerance, "mesh weld tolerance")?;
		finite([options.minimum_area], "mesh minimum area")?;
		if options.minimum_area < 0.0 {
			return Err(Error::Validation("mesh minimum area is negative".into()));
		}
		if self.positions.len() > options.max_vertices || self.triangles.len() > options.max_triangles || self.positions.len() > u32::MAX as usize {
			return Err(Error::Validation("mesh resource limit exceeded".into()));
		}
		finite(self.positions.iter().flat_map(|p| p.to_array()), "mesh positions")?;
		for (i, t) in self.triangles.iter().enumerate() {
			if t.iter().any(|&v| v >= self.positions.len()) {
				return Err(invalid("triangle index out of bounds", vec![i]));
			}
		}
		let mut mesh = self.clone();
		let mut report = MeshImportReport { input_vertices: self.positions.len(), input_triangles: self.triangles.len(), ..Default::default() };
		if options.repair.weld {
			let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
			let mut vertices = Vec::<DVec3>::new();
			let mut map = Vec::new();
			for &p in &mesh.positions {
				let key = (p / options.weld_tolerance).floor();
				if key.abs().max_element() > (i64::MAX - 2) as f64 {
					return Err(Error::Validation("welding coordinate grid overflows; change tolerance or origin".into()));
				}
				let [x, y, z] = key.to_array().map(|v| v as i64);
				let mut found = None;
				'neighbors: for dx in -1..=1 {
					for dy in -1..=1 {
						for dz in -1..=1 {
							if let Some(bucket) = grid.get(&(x + dx, y + dy, z + dz)) {
								for &index in bucket {
									if vertices[index].distance(p) <= options.weld_tolerance {
										found = Some(index);
										break 'neighbors;
									}
								}
							}
						}
					}
				}
				let index = found.unwrap_or_else(|| {
					let index = vertices.len();
					vertices.push(p);
					grid.entry((x, y, z)).or_default().push(index);
					index
				});
				map.push(index);
			}
			report.welded_vertices = mesh.positions.len() - vertices.len();
			mesh.positions = vertices;
			for t in &mut mesh.triangles {
				for v in t {
					*v = map[*v];
				}
			}
		}
		let mut seen = HashSet::new();
		let mut kept = Vec::new();
		for (i, &t) in mesh.triangles.iter().enumerate() {
			let [a, b, c] = t.map(|i| mesh.positions[i]);
			let area = (b - a).cross(c - a).length() * 0.5;
			if !area.is_finite() || area <= options.minimum_area {
				report.degenerate_triangles.push(i);
				if options.repair.remove_degenerate {
					report.removed_degenerate += 1;
					continue;
				}
			}
			let mut key = t;
			key.sort_unstable();
			if !seen.insert(key) {
				report.duplicate_triangles.push(i);
				if options.repair.remove_duplicates {
					report.removed_duplicates += 1;
					continue;
				}
			}
			kept.push(t);
		}
		mesh.triangles = kept;
		let mut edges: HashMap<[usize; 2], Vec<(usize, bool)>> = HashMap::new();
		for (i, t) in mesh.triangles.iter().enumerate() {
			for [a, b] in [[t[0], t[1]], [t[1], t[2]], [t[2], t[0]]] {
				edges.entry([a.min(b), a.max(b)]).or_default().push((i, a < b));
			}
		}
		let mut graph = vec![Vec::new(); mesh.triangles.len()];
		for (&edge, faces) in &edges {
			match faces.len() {
				1 => report.boundary_edges.push(edge),
				2 => {
					let [(a, da), (b, db)] = [faces[0], faces[1]];
					if da == db {
						report.inconsistent_edges.push(edge);
					}
					graph[a].push((b, da == db));
					graph[b].push((a, da == db));
				}
				_ => report.non_manifold_edges.push(edge),
			}
		}
		let mut flips = vec![None; mesh.triangles.len()];
		let mut components = Vec::new();
		for root in 0..mesh.triangles.len() {
			if flips[root].is_some() {
				continue;
			}
			let mut queue = VecDeque::from([root]);
			flips[root] = Some(false);
			let mut component = Vec::new();
			while let Some(i) = queue.pop_front() {
				component.push(i);
				let flip = flips[i].unwrap();
				for &(j, different) in &graph[i] {
					let expected = flip ^ different;
					match flips[j] {
						None => {
							flips[j] = Some(expected);
							queue.push_back(j);
						}
						Some(actual) if actual != expected => return Err(invalid("mesh component is not orientable", vec![i, j])),
						_ => {}
					}
				}
			}
			components.push(component);
		}
		report.components = components.len();
		if options.repair.orient_components {
			for (i, flip) in flips.iter().enumerate() {
				if *flip == Some(true) {
					mesh.triangles[i].swap(1, 2);
					report.flipped_triangles += 1;
				}
			}
		}
		for component in &components {
			let origin = mesh.positions[mesh.triangles[component[0]][0]];
			let mut volume = 0.0;
			for &i in component {
				let [a, b, c] = mesh.triangles[i].map(|v| mesh.positions[v] - origin);
				volume += a.dot(b.cross(c)) / 6.0;
			}
			let closed = component.iter().all(|&i| {
				let t = mesh.triangles[i];
				[[t[0], t[1]], [t[1], t[2]], [t[2], t[0]]].iter().all(|&[a, b]| edges[&[a.min(b), a.max(b)]].len() == 2)
			});
			if closed && volume < 0.0 && options.repair.orient_components {
				for &i in component {
					mesh.triangles[i].swap(1, 2);
					report.flipped_triangles += 1;
				}
				volume = -volume;
			}
			report.component_volumes.push(volume);
			if options.outcome == MeshOutcome::ClosedSolid && closed && volume <= 0.0 {
				return Err(invalid("closed component has non-positive signed volume; request orientation repair", component.clone()));
			}
		}
		if options.outcome != MeshOutcome::MeshOnly {
			if (!report.degenerate_triangles.is_empty() && !options.repair.remove_degenerate) || (!report.duplicate_triangles.is_empty() && !options.repair.remove_duplicates) || !report.non_manifold_edges.is_empty() || (!report.inconsistent_edges.is_empty() && !options.repair.orient_components) {
				return Err(invalid("mesh has degenerate/duplicate triangles, non-manifold edges, or inconsistent winding", report.degenerate_triangles.iter().chain(&report.duplicate_triangles).copied().collect()));
			}
			if options.outcome == MeshOutcome::ClosedSolid && !report.boundary_edges.is_empty() {
				return Err(invalid("closed-solid reconstruction has boundary edges", vec![]));
			}
		}
		let shape = if options.outcome == MeshOutcome::MeshOnly {
			None
		} else {
			let positions: Vec<_> = mesh.positions.iter().flat_map(|p| p.to_array()).collect();
			let indices: Vec<u32> = mesh.triangles.iter().flatten().map(|&v| v as u32).collect();
			Some(Shape::new(ffi::op_faceted_mesh(&positions, &indices, options.sewing_tolerance, options.outcome == MeshOutcome::ClosedSolid).map_err(|e| kernel("faceted mesh reconstruction", e))?))
		};
		Ok(MeshImportResult { mesh, shape, report })
	}
	/// STL has no units; scale explicitly converts file coordinates to model length units.
	pub fn read_stl<R: Read>(reader: &mut R, scale: f64, options: MeshImportOptions) -> Result<MeshImportResult, Error> {
		positive(scale, "STL scale")?;
		let limit = options.max_input_bytes.checked_add(1).ok_or_else(|| Error::Validation("STL byte limit overflow".into()))?;
		let mut bytes = Vec::new();
		reader.take(limit as u64).read_to_end(&mut bytes)?;
		if bytes.len() > options.max_input_bytes {
			return Err(Error::Validation("STL byte limit exceeded".into()));
		}
		let mut positions = Vec::new();
		let mut triangles = Vec::new();
		let mut exact = HashMap::<[u64; 3], usize>::new();
		let mut add = |points: [DVec3; 3]| -> Result<(), Error> {
			if triangles.len() >= options.max_triangles {
				return Err(Error::Validation("STL triangle limit exceeded".into()));
			}
			let mut triangle = [0; 3];
			for (i, p) in points.into_iter().enumerate() {
				let p = p * scale;
				finite(p.to_array(), "STL vertex")?;
				let key = p.to_array().map(|v| if v == 0.0 { 0 } else { v.to_bits() });
				let index = *exact.entry(key).or_insert_with(|| {
					let index = positions.len();
					positions.push(p);
					index
				});
				triangle[i] = index;
			}
			if positions.len() > options.max_vertices {
				return Err(Error::Validation("STL vertex limit exceeded".into()));
			}
			triangles.push(triangle);
			Ok(())
		};
		let binary_count = (bytes.len() >= 84).then(|| u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize);
		let binary = binary_count.and_then(|n| n.checked_mul(50)?.checked_add(84)).is_some_and(|n| n == bytes.len());
		if binary {
			for facet in bytes[84..].chunks_exact(50) {
				let mut values = [0.0; 12];
				for (i, v) in values.iter_mut().enumerate() {
					*v = f32::from_le_bytes(facet[4 * i..4 * i + 4].try_into().unwrap()) as f64;
				}
				finite(values, "STL facet")?;
				add([DVec3::from_slice(&values[3..6]), DVec3::from_slice(&values[6..9]), DVec3::from_slice(&values[9..12])])?;
			}
		} else {
			let text = std::str::from_utf8(&bytes).map_err(|_| invalid("truncated or invalid binary STL", vec![]))?;
			let mut lines = text.lines();
			let first = lines.next().ok_or_else(|| invalid("empty STL", vec![]))?;
			if !first.trim_start().starts_with("solid") {
				return Err(invalid("invalid STL header or truncated binary payload", vec![]));
			}
			let rest = lines.collect::<Vec<_>>().join("\n");
			let mut tokens = rest.split_ascii_whitespace();
			let mut ended = false;
			fn expect(tokens: &mut std::str::SplitAsciiWhitespace<'_>, word: &str) -> Result<(), Error> {
				if tokens.next() == Some(word) {
					Ok(())
				} else {
					Err(invalid("malformed ASCII STL", vec![]))
				}
			}
			fn number(tokens: &mut std::str::SplitAsciiWhitespace<'_>) -> Result<f64, Error> {
				tokens.next().and_then(|s| s.parse::<f64>().ok()).filter(|v| v.is_finite()).ok_or_else(|| invalid("invalid ASCII STL number", vec![]))
			}
			while let Some(token) = tokens.next() {
				if token == "endsolid" {
					ended = true;
					break;
				}
				if token != "facet" {
					return Err(invalid("malformed ASCII STL facet", vec![]));
				}
				expect(&mut tokens, "normal")?;
				for _ in 0..3 {
					number(&mut tokens)?;
				}
				expect(&mut tokens, "outer")?;
				expect(&mut tokens, "loop")?;
				let mut p = [DVec3::ZERO; 3];
				for point in &mut p {
					expect(&mut tokens, "vertex")?;
					*point = DVec3::new(number(&mut tokens)?, number(&mut tokens)?, number(&mut tokens)?);
				}
				expect(&mut tokens, "endloop")?;
				expect(&mut tokens, "endfacet")?;
				add(p)?;
			}
			if !ended {
				return Err(invalid("truncated ASCII STL", vec![]));
			}
		}
		Self { positions, triangles }.import(options)
	}
}
