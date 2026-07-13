//! I/O helpers for `Solid`. Exposed via `impl SolidStruct for Solid` in
//! `super::solid` (e.g. `Solid::read_step`, `Solid::write_step`, `Solid::mesh`).

use super::compound::CompoundShape;
use super::ffi;
use super::solid::Solid;
use super::stream::{RustReader, RustWriter};
use crate::common::error::Error;
use std::io::{Read, Write};

#[cfg(feature = "color")]
use crate::common::color::Color;

// ==================== Color trailer ====================

#[cfg(feature = "color")]
const COLOR_TRAILER_MAGIC: &[u8; 4] = b"CDCL";

#[cfg(feature = "color")]
fn strip_color_trailer(buf: &[u8]) -> (std::collections::HashMap<u64, Color>, usize) {
	if buf.len() < 8 || &buf[buf.len() - 4..] != COLOR_TRAILER_MAGIC {
		return (std::collections::HashMap::new(), buf.len());
	}
	let entry_count = u32::from_le_bytes(buf[buf.len() - 8..buf.len() - 4].try_into().unwrap()) as usize;
	let trailer_size = 8 + entry_count * 16;
	if buf.len() < trailer_size {
		return (std::collections::HashMap::new(), buf.len());
	}
	let brep_len = buf.len() - trailer_size;
	let entries_start = brep_len;
	let mut colormap = std::collections::HashMap::new();
	for i in 0..entry_count {
		let off = entries_start + i * 16;
		let idx = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
		let r = f32::from_le_bytes(buf[off + 4..off + 8].try_into().unwrap());
		let g = f32::from_le_bytes(buf[off + 8..off + 12].try_into().unwrap());
		let b = f32::from_le_bytes(buf[off + 12..off + 16].try_into().unwrap());
		colormap.insert(idx as u64, Color { r, g, b });
	}
	(colormap, brep_len)
}

#[cfg(feature = "color")]
fn resolve_color_trailer(inner: &ffi::TopoDS_Shape, index_colormap: &std::collections::HashMap<u64, Color>) -> std::collections::HashMap<u64, Color> {
	let faces = ffi::shape_faces(inner);
	let index_to_id: Vec<u64> = faces.iter().map(ffi::face_tshape_id).collect();
	index_colormap.iter().filter_map(|(&idx, &color)| index_to_id.get(idx as usize).map(|&id| (id, color))).collect()
}

/// Effective colour of every face: its own if it has one, else the colour of the
/// solid it belongs to.
///
/// A solid-level colour is keyed by the solid's id and so has no face key of its
/// own. Consumers that speak only face colours — the BRep trailer, which is keyed
/// by face index and has nowhere to say "this is the solid", and `Mesh`, which is
/// what every renderer reads — get it expanded here. Appearance survives exactly;
/// the solid/face distinction lives on in STEP alone, which is the one format that
/// can express it.
#[cfg(feature = "color")]
fn resolve_face_colors<'a>(solids: impl IntoIterator<Item = &'a Solid>) -> std::collections::HashMap<u64, Color> {
	let mut map = std::collections::HashMap::new();
	for s in solids {
		if let Some(c) = s.color_solid() {
			for f in ffi::shape_faces(s.inner()).iter() {
				map.insert(ffi::face_tshape_id(f), c);
			}
		}
		// Face colours are the more specific style and win over the solid's.
		map.extend(s.colormap().iter().map(|(&k, &v)| (k, v)));
	}
	map
}

#[cfg(feature = "color")]
fn write_color_trailer<W: Write>(compound: &CompoundShape, colormap: &std::collections::HashMap<u64, Color>, writer: &mut W) -> Result<(), Error> {
	if colormap.is_empty() {
		return Ok(());
	}
	let faces = ffi::shape_faces(compound.inner());
	let id_to_index: std::collections::HashMap<u64, u32> = faces.iter().enumerate().map(|(i, f)| (ffi::face_tshape_id(f), i as u32)).collect();
	let mut entries: Vec<(u32, f32, f32, f32)> = colormap.iter().filter_map(|(id, rgb)| id_to_index.get(id).map(|&idx| (idx, rgb.r, rgb.g, rgb.b))).collect();
	entries.sort_by_key(|e| e.0);

	for (idx, r, g, b) in &entries {
		writer.write_all(&idx.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
		writer.write_all(&r.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
		writer.write_all(&g.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
		writer.write_all(&b.to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
	}
	writer.write_all(&(entries.len() as u32).to_le_bytes()).map_err(|_| Error::BrepWriteFailed)?;
	writer.write_all(COLOR_TRAILER_MAGIC).map_err(|_| Error::BrepWriteFailed)?;
	Ok(())
}

// ==================== Reader / writer / mesh helpers ====================
//
// Each function is invoked by the matching `SolidStruct` method in
// `super::solid::Solid`. Kept module-private (`pub(super)`) so the public
// surface lives entirely on `Solid`.

pub(super) fn read_step<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
	#[cfg(feature = "color")]
	{
		let mut rust_reader = RustReader::from_ref(reader);
		let mut ids: Vec<u64> = Default::default();
		let mut rgb: Vec<f32> = Default::default();
		let inner = ffi::read_step_color_stream(&mut rust_reader, &mut ids, &mut rgb);
		if inner.is_null() {
			return Err(Error::StepReadFailed);
		}
		let colormap: std::collections::HashMap<u64, Color> = ids.into_iter().zip(rgb.chunks_exact(3)).map(|(id, c)| (id, Color { r: c[0], g: c[1], b: c[2] })).collect();
		Ok(CompoundShape::from_raw(inner, colormap, Default::default()).decompose())
	}
	#[cfg(not(feature = "color"))]
	{
		let mut rust_reader = RustReader::from_ref(reader);
		let inner = ffi::read_step_stream(&mut rust_reader);
		if inner.is_null() {
			return Err(Error::StepReadFailed);
		}
		Ok(CompoundShape::from_raw(inner, Default::default()).decompose())
	}
}

/// Read solids from a BRep binary stream. Color trailer is parsed if present.
pub(super) fn read_brep_binary<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
	read_brep_with(reader, ffi::read_brep_bin_stream)
}

/// Read solids from a BRep text stream. Color trailer is parsed if present.
pub(super) fn read_brep_text<R: Read>(reader: &mut R) -> Result<Vec<Solid>, Error> {
	read_brep_with(reader, ffi::read_brep_text_stream)
}

/// Shared body for BRep binary/text reads. The two variants differ only in
/// which FFI stream parser they call; everything else (color trailer handling,
/// null check, decompose) is identical.
fn read_brep_with<R: Read>(reader: &mut R, ffi_read: fn(&mut RustReader) -> cxx::UniquePtr<ffi::TopoDS_Shape>) -> Result<Vec<Solid>, Error> {
	#[cfg(feature = "color")]
	{
		let mut buf = Vec::new();
		reader.read_to_end(&mut buf).map_err(|_| Error::BrepReadFailed)?;
		let (index_colormap, brep_len) = strip_color_trailer(&buf);
		let mut cursor = std::io::Cursor::new(&buf[..brep_len]);
		let mut rust_reader = RustReader::from_ref(&mut cursor);
		let inner = ffi_read(&mut rust_reader);
		if inner.is_null() {
			return Err(Error::BrepReadFailed);
		}
		let colormap = resolve_color_trailer(&inner, &index_colormap);
		Ok(CompoundShape::from_raw(inner, colormap, Default::default()).decompose())
	}
	#[cfg(not(feature = "color"))]
	{
		let mut rust_reader = RustReader::from_ref(reader);
		let inner = ffi_read(&mut rust_reader);
		if inner.is_null() {
			return Err(Error::BrepReadFailed);
		}
		Ok(CompoundShape::from_raw(inner, Default::default()).decompose())
	}
}

/// Write solids to a STEP stream.
///
/// With the `color` feature enabled, face colors are automatically embedded
/// in the STEP file (XDE / AP214 styled items).
pub(super) fn write_step<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> {
	let compound = CompoundShape::new(solids);
	#[cfg(feature = "color")]
	{
		let colormap = compound.colormap();
		let mut ids: Vec<u64> = Vec::with_capacity(colormap.len());
		let mut rgb: Vec<f32> = Vec::with_capacity(colormap.len() * 3);
		for (&id, c) in colormap {
			ids.push(id);
			rgb.extend_from_slice(&[c.r, c.g, c.b]);
		}
		let mut rust_writer = RustWriter::from_ref(writer);
		if ffi::write_step_color_stream(compound.inner(), &ids, &rgb, &mut rust_writer) {
			Ok(())
		} else {
			Err(Error::StepWriteFailed)
		}
	}
	#[cfg(not(feature = "color"))]
	{
		let mut rust_writer = RustWriter::from_ref(writer);
		if ffi::write_step_stream(compound.inner(), &mut rust_writer) {
			Ok(())
		} else {
			Err(Error::StepWriteFailed)
		}
	}
}

/// Write solids to a BRep binary stream. Color trailer is appended if `color`
/// feature is enabled.
pub(super) fn write_brep_binary<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> {
	write_brep_with(solids, writer, ffi::write_brep_bin_stream)
}

/// Write solids to a BRep text stream. Color trailer is appended if `color`
/// feature is enabled.
pub(super) fn write_brep_text<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error> {
	write_brep_with(solids, writer, ffi::write_brep_text_stream)
}

/// Shared body for BRep binary/text writes — see `read_brep_with` for context.
fn write_brep_with<'a, W: Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W, ffi_write: fn(&ffi::TopoDS_Shape, &mut RustWriter) -> bool) -> Result<(), Error> {
	#[cfg(feature = "color")]
	let solids: Vec<&Solid> = solids.into_iter().collect();
	#[cfg(feature = "color")]
	let colormap = resolve_face_colors(solids.iter().copied());

	let compound = CompoundShape::new(solids);
	let mut rust_writer = RustWriter::from_ref(writer);
	if !ffi_write(compound.inner(), &mut rust_writer) {
		return Err(Error::BrepWriteFailed);
	}
	#[cfg(feature = "color")]
	write_color_trailer(&compound, &colormap, writer)?;
	Ok(())
}

pub(super) fn mesh<'a>(solids: impl IntoIterator<Item = &'a Solid>, options: crate::traits::Tessellation) -> Result<crate::common::mesh::Mesh, Error> {
	use crate::common::mesh::Mesh;
	use glam::DVec3;

	#[cfg(feature = "color")]
	let solids: Vec<&Solid> = solids.into_iter().collect();
	// `Mesh` speaks only face colours, so a solid's is resolved onto its faces
	// here. That keeps every renderer (glTF / STL / Scene2D) unchanged.
	#[cfg(feature = "color")]
	let face_colors = resolve_face_colors(solids.iter().copied());

	let compound = CompoundShape::new(solids);
	let data = ffi::mesh_shape(compound.inner(), options.deflection_linear, options.deflection_angular, options.relative_linear);
	if !data.success {
		return Err(Error::TriangulationFailed);
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
	for e in ffi::shape_edges(compound.inner()).iter() {
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

	#[cfg(feature = "color")]
	let colormap = {
		let mut map = std::collections::HashMap::new();
		for &fid in &face_ids {
			if let Some(&color) = face_colors.get(&fid) {
				map.insert(fid, color);
			}
		}
		map
	};

	Ok(Mesh {
		vertices,
		normals,
		indices,
		face_ids,
		#[cfg(feature = "color")]
		colormap,
		edges,
	})
}
