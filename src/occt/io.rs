//! I/O helpers for `Solid`. Exposed via `impl SolidStruct for Solid` in
//! `super::solid` (e.g. `Solid::read_step`, `Solid::write_step`, `Solid::mesh`).

use super::ffi;
use super::ffi::{RustReader, RustWriter};
use super::solid::Solid;
use crate::common::error::Error;
use std::io::{Read, Write};

use crate::common::color::Color;

// ==================== Color trailer ====================
// Appended past the BinTools payload, which BinTools::Read stops at and ignores:
// `[b"CDCL"][u32 count][count x (u32 trailer_ids index, f32 r, f32 g, f32 b)]`, LE.

const COLOR_TRAILER_MAGIC: &[u8; 4] = b"CDCL";

/// `tail` is `&buf[consumed..]`, the bytes the BRep parser did not take. Anything that
/// is not our trailer yields an empty map — the geometry is valid either way.
fn read_color_trailer(tail: &[u8]) -> std::collections::HashMap<u32, Color> {
	let mut colormap = std::collections::HashMap::new();
	if tail.len() < 8 || &tail[..4] != COLOR_TRAILER_MAGIC {
		return colormap;
	}
	let count = u32::from_le_bytes(tail[4..8].try_into().unwrap()) as usize;
	// `count` comes from the file, and `usize` is 32-bit on wasm32.
	let Some(end) = count.checked_mul(16).and_then(|n| n.checked_add(8)) else {
		return colormap;
	};
	// `<`, not `!=`: the count self-delimits, so bytes appended after us are not an error.
	if tail.len() < end {
		return colormap;
	}
	for e in tail[8..end].chunks_exact(16) {
		let idx = u32::from_le_bytes(e[0..4].try_into().unwrap());
		let r = f32::from_le_bytes(e[4..8].try_into().unwrap());
		let g = f32::from_le_bytes(e[8..12].try_into().unwrap());
		let b = f32::from_le_bytes(e[12..16].try_into().unwrap());
		colormap.insert(idx, Color { r, g, b });
	}
	colormap
}

/// STEP cannot index like this — `try_sew_orphan_faces` shifts every index, so it
/// carries explicit ids instead.
fn trailer_ids(solids: &cxx::CxxVector<ffi::TopoDS_Solid>) -> Vec<u64> {
	// Every solid first, then every solid's faces in the same solid order.
	let faces = solids.iter().flat_map(|s| ffi::solid_faces(s).iter().map(ffi::face_tshape_id).collect::<Vec<_>>());
	solids.iter().map(ffi::solid_tshape_id).chain(faces).collect()
}

fn write_color_trailer<W: Write>(solids: &[&Solid], vec: &cxx::CxxVector<ffi::TopoDS_Solid>, writer: &mut W) -> Result<(), Error> {
	let id_to_index: std::collections::HashMap<u64, u32> = trailer_ids(vec).into_iter().enumerate().map(|(i, id)| (id, i as u32)).collect();
	let colormap: std::collections::HashMap<u64, Color> = solids.iter().flat_map(|s| s.colormap().iter().map(|(&k, &v)| (k, v))).collect();
	// `Solid::from_ffi` gives every solid a clone of the shared colormap, so a solid
	// may carry keys of solids not written here; those have no index and drop out.
	let mut entries: Vec<(u32, f32, f32, f32)> = colormap.iter().filter_map(|(id, rgb)| id_to_index.get(id).map(|&idx| (idx, rgb.r, rgb.g, rgb.b))).collect();
	if entries.is_empty() {
		return Ok(());
	}
	entries.sort_by_key(|e| e.0);

	let mut out = Vec::with_capacity(8 + entries.len() * 16);
	out.extend_from_slice(COLOR_TRAILER_MAGIC);
	out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
	for (idx, r, g, b) in &entries {
		out.extend_from_slice(&idx.to_le_bytes());
		out.extend_from_slice(&r.to_le_bytes());
		out.extend_from_slice(&g.to_le_bytes());
		out.extend_from_slice(&b.to_le_bytes());
	}
	writer.write_all(&out).map_err(Error::Io)
}

// ==================== Reader / writer / mesh helpers ====================
//
// Each function is invoked by the matching `SolidStruct` method in
// `super::solid::Solid`. Kept module-private (`pub(super)`) so the public
// surface lives entirely on `Solid`.

pub(super) fn read_step<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
	let mut rust_reader = RustReader::from_ref(reader);
	let mut ids: Vec<u64> = Default::default();
	let mut rgb: Vec<f32> = Default::default();
	let solids = ffi::read_step_color_stream(&mut rust_reader, &mut ids, &mut rgb).map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("step: {}", e.what()))))?;
	let colormap: std::collections::HashMap<u64, Color> = ids.into_iter().zip(rgb.chunks_exact(3)).map(|(id, c)| (id, Color { r: c[0], g: c[1], b: c[2] })).collect();
	Ok(Solid::from_ffi(&solids, &colormap, &[]))
}

pub(super) fn read_brep<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
	// Buffered whole: `BinTools::Read` seeks backwards to resolve shared sub-shape
	// references, so it cannot run off a sequential stream.
	let mut buf = Vec::new();
	reader.read_to_end(&mut buf)?;

	// Payload length — where a trailer would begin. Unwritten, and unread, on error.
	let mut consumed = 0usize;
	let solids = ffi::read_brep_stream(&buf, &mut consumed).map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("brep: {}", e.what()))))?;

	let ids = trailer_ids(&solids);
	let colormap = read_color_trailer(buf.get(consumed..).unwrap_or_default()).into_iter().filter_map(|(idx, color)| ids.get(idx as usize).map(|&id| (id, color))).collect();
	Ok(Solid::from_ffi(&solids, &colormap, &[]))
}

/// Write solids to a STEP stream, embedding face and solid colors as AP214 styled items.
pub(super) fn write_step<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> {
	let solids: Vec<&Solid> = solids.into_iter().collect();
	let vec = Solid::to_ffi(solids.iter().copied());
	// A key repeated across solids is sent twice; C++ keeps the later one.
	let mut ids: Vec<u64> = Vec::new();
	let mut rgb: Vec<f32> = Vec::new();
	for (&id, c) in solids.iter().flat_map(|s| s.colormap()) {
		ids.push(id);
		rgb.extend_from_slice(&[c.r, c.g, c.b]);
	}
	let mut rust_writer = RustWriter::from_ref(writer);
	ffi::write_step_color_stream(&vec, &ids, &rgb, &mut rust_writer).map_err(|e| Error::Io(std::io::Error::other(format!("step: {}", e.what()))))
}

pub(super) fn write_brep<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> {
	let solids: Vec<&Solid> = solids.into_iter().collect();
	let vec = Solid::to_ffi(solids.iter().copied());
	{
		// Scoped: the streambuf flushes on drop, so the payload lands before the trailer.
		let mut rust_writer = RustWriter::from_ref(writer);
		ffi::write_brep_stream(&vec, &mut rust_writer).map_err(|e| Error::Io(std::io::Error::other(format!("brep: {}", e.what()))))?;
	}
	write_color_trailer(&solids, &vec, writer)?;
	Ok(())
}

pub(super) fn mesh<'a>(solids: impl IntoIterator<Item = &'a Solid>, options: crate::traits::Tessellation) -> Result<crate::common::mesh::Mesh, Error> {
	use crate::common::mesh::Mesh;
	use glam::DVec3;

	let solids: Vec<&Solid> = solids.into_iter().collect();
	// `Mesh` has only a face level, so a solid-level colour is expanded onto its faces
	// here. STEP and the BRep trailer keep the distinction; the renderers cannot.
	let face_colors = {
		let mut map = std::collections::HashMap::new();
		for s in solids.iter().copied() {
			if let Some(&c) = s.colormap().get(&s.id()) {
				for f in ffi::solid_faces(s.inner()).iter() {
					map.insert(ffi::face_tshape_id(f), c);
				}
			}
			// Face colours are the more specific style and win over the solid's.
			map.extend(s.colormap().iter().map(|(&k, &v)| (k, v)));
		}
		map
	};

	let data = ffi::mesh_shape(&Solid::to_ffi(solids.iter().copied()), options.deflection_linear, options.deflection_angular, options.relative_linear);
	if !data.success {
		return Err(Error::Tesselation);
	}
	let vertex_count = data.vertices.len() / 3;
	let vertices: Vec<DVec3> = (0..vertex_count).map(|i| DVec3::new(data.vertices[i * 3], data.vertices[i * 3 + 1], data.vertices[i * 3 + 2])).collect();
	let normals: Vec<DVec3> = (0..vertex_count).map(|i| DVec3::new(data.normals[i * 3], data.normals[i * 3 + 1], data.normals[i * 3 + 2])).collect();
	let indices: Vec<usize> = data.indices.iter().map(|&i| i as usize).collect();
	let face_ids = data.face_tshape_ids;

	// Topological edge polylines, NaN-separated. Reuses the existing edge
	// discretizer (GCPnts_TangentialDeflection). `relative_linear` applies to
	// surface triangulation only; edges use `deflection_linear` as an absolute
	// chord here.
	let mut edges: Vec<DVec3> = Vec::new();
	let solid_edges: Vec<_> = solids.iter().map(|s| ffi::solid_edges(s.inner())).collect();
	for e in solid_edges.iter().flat_map(|v| v.iter()) {
		let segs = ffi::edge_approximation_segments(e, options.deflection_linear, options.deflection_angular, options.relative_linear);
		if segs.len() < 6 {
			continue; // fewer than 2 points — nothing to draw
		}
		if !edges.is_empty() {
			edges.push(DVec3::NAN);
		}
		for c in segs.chunks_exact(3) {
			edges.push(DVec3::new(c[0], c[1], c[2]));
		}
	}

	let colormap = {
		let mut map = std::collections::HashMap::new();
		for &fid in &face_ids {
			if let Some(&color) = face_colors.get(&fid) {
				map.insert(fid, color);
			}
		}
		map
	};

	Ok(Mesh { vertices, normals, indices, face_ids, colormap, edges })
}
