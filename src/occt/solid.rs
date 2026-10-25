use super::edge::{edges_to_ffi, Edge};
use super::face::Face;
use super::ffi;
use crate::common::boolean::Boolean;
use crate::common::error::Error;
use crate::traits::{Lineage, ProfileOrient, SolidStruct, Transform};
use glam::DVec3;
use std::sync::OnceLock;

/// Encode `ProfileOrient` into FFI arguments: (kind, ux, uy, uz, aux_spine_edges).
fn encode_orient(orient: ProfileOrient<'_>) -> (u32, f64, f64, f64, &[Edge]) {
	match orient {
		ProfileOrient::Fixed => (0, 0.0, 0.0, 0.0, &[]),
		ProfileOrient::Torsion => (1, 0.0, 0.0, 0.0, &[]),
		ProfileOrient::Up(v) => (2, v.x, v.y, v.z, &[]),
		ProfileOrient::Auxiliary(edges) => (3, 0.0, 0.0, 0.0, edges),
	}
}

/// The face ids of `inner`, in `iter_face` order.
pub(super) fn face_ids(inner: &ffi::TopoDS_Solid) -> Vec<u64> {
	let mut ids = Vec::new();
	let _ = ffi::solid_faces(inner, &mut ids);
	ids
}

/// Decode the `[kind, src, post]` triples every bridge builder writes (see `ffi.h`).
fn decode_history(flat: &[u64]) -> Vec<Lineage> {
	flat.chunks_exact(3)
		.map(|t| match t[0] {
			0 => Lineage::Kept { src: t[1], post: t[2] },
			1 => Lineage::Modified { src: t[1], post: t[2] },
			2 => Lineage::Generated { src: t[1], post: t[2] },
			_ => Lineage::Deleted { src: t[1] },
		})
		.collect()
}

/// A single solid topology shape wrapping a `TopoDS_Solid`.
///
/// `inner` is private to prevent external mutation that could break the solid invariant.
/// Use the provided methods to query and transform the solid.
///
/// `edges` / `faces` are lazy `OnceLock` caches populated on first `iter_edge` /
/// `iter_face` call. Since topology-changing ops consume `self` and construct
/// a new `Solid` via `Solid::new`, these caches are invalidated automatically
/// (new instance → fresh `OnceLock::new()`). See
/// `notes/20260420-OCCTトポロジ不変性と設計含意.md`.
pub struct Solid {
	inner: cxx::UniquePtr<ffi::TopoDS_Solid>,
	edges: OnceLock<Vec<Edge>>,
	faces: OnceLock<Vec<Face>>,
	vertices: OnceLock<Vec<DVec3>>,
	/// Keyed by a face's id, or by `Solid::id()` for the solid as a whole; a face
	/// colour wins over the solid's. Other solids' keys may be present (`from_ffi`).
	colormap: std::collections::HashMap<u64, crate::common::color::Color>,
	/// How this solid's faces and edges descend from the operands of the operation
	/// that built it (`iter_history`). Kept through translate/rotate/color/Clone.
	history: Vec<Lineage>,
}

impl Solid {
	/// Create a `Solid` from a `TopoDS_Solid`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Solid>, colormap: std::collections::HashMap<u64, crate::common::color::Color>, history: Vec<Lineage>) -> Self {
		Solid { inner, edges: OnceLock::new(), faces: OnceLock::new(), vertices: OnceLock::new(), colormap, history }
	}

	// ==================== Internal accessors ====================

	/// Borrow the underlying `TopoDS_Solid` (crate-internal only).
	pub(crate) fn inner(&self) -> &ffi::TopoDS_Solid {
		&self.inner
	}

	/// Collect solids into the `std::vector` the multi-solid FFI calls take (shallow handle copies).
	pub(super) fn to_ffi<'a>(solids: impl IntoIterator<Item = &'a Solid>) -> cxx::UniquePtr<cxx::CxxVector<ffi::TopoDS_Solid>> {
		let mut vec = ffi::shape_vec_new();
		for s in solids {
			ffi::shape_vec_push(vec.pin_mut(), &s.inner);
		}
		vec
	}

	/// Wrap the solids of one FFI result. Each gets a clone of the whole `colormap`, the
	/// `history` entries whose post face or edge it owns, and every `Deleted` entry.
	pub(super) fn from_ffi(solids: &cxx::CxxVector<ffi::TopoDS_Solid>, colormap: &std::collections::HashMap<u64, crate::common::color::Color>, history: &[Lineage]) -> Vec<Solid> {
		solids
			.iter()
			.map(|s| {
				let mut local: std::collections::HashSet<u64> = face_ids(s).into_iter().collect();
				let mut edge_ids = Vec::new();
				let _ = ffi::solid_edges(s, &mut edge_ids);
				local.extend(edge_ids);
				let history = history.iter().filter(|l| l.post().is_none_or(|p| local.contains(&p))).copied().collect();
				Solid::new(ffi::clone_solid_handle(s), colormap.clone(), history)
			})
			.collect()
	}

	// ==================== Color accessors ====================

	/// Read-only access to the per-face colormap.
	pub fn colormap(&self) -> &std::collections::HashMap<u64, crate::common::color::Color> {
		&self.colormap
	}

	/// Mutable access to the per-face colormap.
	pub fn colormap_mut(&mut self) -> &mut std::collections::HashMap<u64, crate::common::color::Color> {
		&mut self.colormap
	}

	/// Carry face colours onto every result face that descends from a coloured face, and
	/// the solid's own colour onto the new solid.
	fn remap_colormap(&self, new_inner: &ffi::TopoDS_Solid, history: &[Lineage]) -> std::collections::HashMap<u64, crate::common::color::Color> {
		let mut colormap: std::collections::HashMap<u64, crate::common::color::Color> = history.iter().filter_map(|l| Some((l.post()?, *self.colormap.get(&l.src())?))).collect();
		// `history` covers faces and edges, not the solid, whose id these ops change.
		if let Some(&color) = self.colormap.get(&ffi::solid_tshape_id(&self.inner)) {
			colormap.insert(ffi::solid_tshape_id(new_inner), color);
		}
		colormap
	}

	/// Build a solid from a bridge call that fills a history; colours follow the history.
	fn rebuilt(&self, build: impl FnOnce(&mut Vec<u64>) -> Result<cxx::UniquePtr<ffi::TopoDS_Solid>, cxx::Exception>) -> Result<Self, cxx::Exception> {
		let mut flat: Vec<u64> = Vec::new();
		let inner = build(&mut flat)?;
		let history = decode_history(&flat);
		let colormap = self.remap_colormap(&inner, &history);
		Ok(Solid::new(inner, colormap, history))
	}
}

impl std::fmt::Debug for Solid {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Solid({}, center={:?}, faces={}, edges={})", self.id(), self.center(), self.iter_face().count(), self.iter_edge().count())
	}
}

impl SolidStruct for Solid {
	type Edge = Edge;
	type Face = Face;

	// ==================== Identity ====================

	fn id(&self) -> u64 {
		ffi::solid_tshape_id(&self.inner)
	}

	// ==================== Constructors ====================

	fn cube(corner0: DVec3, corner1: DVec3) -> Solid {
		let inner = ffi::make_box(corner0.x, corner0.y, corner0.z, corner1.x, corner1.y, corner1.z).unwrap_or_else(|e| panic!("Solid::cube: {}", e.what()));
		Solid::new(inner, std::collections::HashMap::new(), Default::default())
	}

	fn cylinder(r: f64, height: DVec3) -> Solid {
		let inner = ffi::make_cylinder(0.0, 0.0, 0.0, height.x, height.y, height.z, r, height.length()).unwrap_or_else(|e| panic!("Solid::cylinder: {}", e.what()));
		Solid::new(inner, std::collections::HashMap::new(), Default::default())
	}

	fn sphere(radius: f64) -> Solid {
		let inner = ffi::make_sphere(0.0, 0.0, 0.0, radius).unwrap_or_else(|e| panic!("Solid::sphere: {}", e.what()));
		Solid::new(inner, std::collections::HashMap::new(), Default::default())
	}

	fn cone(r1: f64, r2: f64, height: DVec3) -> Solid {
		let inner = ffi::make_cone(0.0, 0.0, 0.0, height.x, height.y, height.z, r1, r2, height.length()).unwrap_or_else(|e| panic!("Solid::cone: {}", e.what()));
		Solid::new(inner, std::collections::HashMap::new(), Default::default())
	}

	fn torus(r1: f64, r2: f64, axis: DVec3) -> Solid {
		let inner = ffi::make_torus(0.0, 0.0, 0.0, axis.x, axis.y, axis.z, r1, r2).unwrap_or_else(|e| panic!("Solid::torus: {}", e.what()));
		Solid::new(inner, std::collections::HashMap::new(), Default::default())
	}

	fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Solid {
		let inner = ffi::make_half_space(plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z).unwrap_or_else(|e| panic!("Solid::half_space: {}", e.what()));
		Solid::new(inner, std::collections::HashMap::new(), Default::default())
	}

	// ==================== Topology iteration ====================
	//
	// `iter_edge` / `iter_face` lazily populate `OnceLock<Vec<T>>` caches on
	// first call. Topology-changing ops construct a fresh `Solid` via `Solid::new`
	// so these caches are invalidated automatically.

	fn iter_edge(&self) -> impl Iterator<Item = &Edge> + '_ {
		self.edges
			.get_or_init(|| {
				let mut ids = Vec::new();
				ffi::solid_edges(&self.inner, &mut ids).map(|v| v.iter().zip(ids).map(|(e, id)| Edge::with_id(ffi::clone_edge_handle(e), id)).collect()).unwrap_or_default()
			})
			.iter()
	}

	fn iter_face(&self) -> impl Iterator<Item = &Face> + '_ {
		self.faces
			.get_or_init(|| {
				let mut ids = Vec::new();
				ffi::solid_faces(&self.inner, &mut ids).map(|v| v.iter().zip(ids).map(|(f, id)| Face::new(ffi::clone_face_handle(f), ffi::local_face(f, &self.inner), id)).collect()).unwrap_or_default()
			})
			.iter()
	}

	fn iter_vertex(&self) -> impl Iterator<Item = &DVec3> + '_ {
		self.vertices.get_or_init(|| ffi::solid_vertices(&self.inner).unwrap_or_default().chunks_exact(3).map(|c| DVec3::new(c[0], c[1], c[2])).collect()).iter()
	}

	fn iter_history(&self) -> impl Iterator<Item = Lineage> + '_ {
		self.history.iter().copied()
	}

	// ==================== Extrude / Revolve ====================

	fn extrude<'a>(profile: impl IntoIterator<Item = &'a Edge>, dir: DVec3) -> Result<Self, Error> {
		let mut flat = Vec::new();
		let shape = ffi::make_extrude(&edges_to_ffi(profile), dir.x, dir.y, dir.z, &mut flat).map_err(|e| Error::Extrude(e.what().into()))?;
		Ok(Solid::new(shape, std::collections::HashMap::new(), decode_history(&flat)))
	}

	fn revolve<'a>(profile: impl IntoIterator<Item = &'a Edge>, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Result<Self, Error> {
		let mut flat = Vec::new();
		let shape = ffi::make_revolve(&edges_to_ffi(profile), axis_origin.x, axis_origin.y, axis_origin.z, axis_direction.x, axis_direction.y, axis_direction.z, angle, &mut flat).map_err(|e| Error::Revolve(format!("angle={angle} about {axis_direction:?} through {axis_origin:?}: {}", e.what())))?;
		Ok(Solid::new(shape, std::collections::HashMap::new(), decode_history(&flat)))
	}

	// ==================== Shell ====================

	fn shell<'a>(&self, thickness: f64, open_faces: impl IntoIterator<Item = &'a Face>) -> Result<Self, Error> {
		let mut face_vec = ffi::face_vec_new();
		for f in open_faces {
			ffi::face_vec_push(face_vec.pin_mut(), &f.inner);
		}
		self.rebuilt(|h| ffi::builder_thick_solid(&self.inner, &face_vec, thickness, h)).map_err(|e| Error::Shell(format!("thickness={thickness} incompatible with the geometry, or self-intersecting offset ({} open face(s)): {}", face_vec.len(), e.what())))
	}

	// ==================== Fillet / Chamfer ====================

	fn fillet_edges<'a>(&self, radius: f64, edges: impl IntoIterator<Item = &'a Edge>) -> Result<Self, Error> {
		let edge_vec = edges_to_ffi(edges);
		self.rebuilt(|h| ffi::builder_fillet(&self.inner, &edge_vec, radius, h)).map_err(|e| Error::Fillet(format!("radius={radius} does not fit the local geometry on {} edge(s): {}", edge_vec.len(), e.what())))
	}

	fn chamfer_edges<'a>(&self, distance: f64, edges: impl IntoIterator<Item = &'a Edge>) -> Result<Self, Error> {
		let edge_vec = edges_to_ffi(edges);
		self.rebuilt(|h| ffi::builder_chamfer(&self.inner, &edge_vec, distance, h)).map_err(|e| Error::Chamfer(format!("distance={distance} does not fit the local geometry on {} edge(s): {}", edge_vec.len(), e.what())))
	}

	// ==================== Sweep ====================

	fn sweep<'a, 'b, 'c>(profile: impl IntoIterator<Item = &'a Edge>, spine: impl IntoIterator<Item = &'b Edge>, orient: ProfileOrient<'c>) -> Result<Self, Error> {
		let (profile, spine) = (edges_to_ffi(profile), edges_to_ffi(spine));
		let (kind, ux, uy, uz, aux) = encode_orient(orient);
		let mut flat = Vec::new();
		let shape = ffi::make_pipe_shell(&profile, &spine, kind, ux, uy, uz, &edges_to_ffi(aux), &mut flat).map_err(|e| Error::Sweep(format!("profile ({} edge(s)) could not be swept along the spine ({} edge(s)): {}", profile.len(), spine.len(), e.what())))?;
		Ok(Solid::new(shape, std::collections::HashMap::new(), decode_history(&flat)))
	}

	// ==================== Loft / ThruSections ====================

	fn loft<'a, I: IntoIterator<Item = &'a Edge>, S: IntoIterator<Item = I>>(sections: S, ruled: bool) -> Result<Self, Error>
	where
		Edge: 'a,
	{
		let mut all_edges: Vec<&Edge> = Vec::new();
		let mut section_count = 0usize;

		for sec in sections {
			let mut count = 0u32;
			for edge in sec {
				all_edges.push(edge);
				count += 1;
			}
			if count == 0 {
				return Err(Error::Loft(format!("loft: section {} is empty (each section must contain ≥1 edge)", section_count)));
			}
			section_count += 1;
		}

		if section_count < 2 {
			return Err(Error::Loft(format!("loft: need ≥2 sections, got {} (a single section has no thickness to skin across)", section_count)));
		}

		let mut flat = Vec::new();
		let shape = ffi::make_loft(&edges_to_ffi(all_edges), section_count as u32, ruled, &mut flat).map_err(|e| Error::Loft(format!("loft: OCCT BRepOffsetAPI_ThruSections failed (sections={section_count}, ruled={ruled}): {}", e.what())))?;
		Ok(Solid::new(shape, std::collections::HashMap::new(), decode_history(&flat)))
	}

	// ==================== Sew ====================

	fn sew<'a>(faces: impl IntoIterator<Item = &'a Face>, tolerance: f64) -> Result<Self, Error>
	where
		Face: 'a,
	{
		let mut face_vec = ffi::face_vec_new();
		let mut ids = Vec::new();
		for f in faces {
			ffi::face_vec_push(face_vec.pin_mut(), &f.inner);
			ids.push(f.id());
		}
		if ids.is_empty() {
			return Err(Error::Sew("sew: no faces given (need a face set forming one closed shell)".into()));
		}
		let mut flat = Vec::new();
		let shape = ffi::make_sewn_solid(&face_vec, &ids, tolerance, &mut flat).map_err(|e| Error::Sew(format!("sew: {} faces do not form exactly one closed shell within tolerance {tolerance}: {}", ids.len(), e.what())))?;
		Ok(Solid::new(shape, std::collections::HashMap::new(), decode_history(&flat)))
	}

	// ==================== Offset surface ====================

	fn offset<'a>(&self, offset: f64, faces: impl IntoIterator<Item = &'a Face>) -> Result<Self, Error> {
		let mut face_vec = ffi::face_vec_new();
		for f in faces {
			ffi::face_vec_push(face_vec.pin_mut(), &f.inner);
		}
		self.rebuilt(|h| ffi::make_offset(&self.inner, &face_vec, offset, Edge::precision_distance(), h)).map_err(|e| Error::Offset(format!("offset: OCCT BRepOffset_MakeOffset failed (offset={offset}): {}", e.what())))
	}

	// ==================== Bspline ====================

	fn bspline(u: usize, v: usize, u_periodic: bool, v_periodic: bool, point: impl Fn(usize, usize) -> DVec3) -> Result<Self, Error> {
		if u.min(v) < 3 {
			return Err(Error::Bspline(format!("grid is too small (u={u}, v={v}, u_periodic={u_periodic}, v_periodic={v_periodic})")));
		}

		let coords: Vec<f64> = (0..u * v).flat_map(|k| point(k / v, k % v).to_array()).collect();

		let shape = ffi::make_bspline_solid(&coords, u as u32, v as u32, u_periodic, v_periodic).map_err(|e| Error::Bspline(format!("OCCT construction failed (u={u}, v={v}, u_periodic={u_periodic}, v_periodic={v_periodic}): {}", e.what())))?;
		Ok(Solid::new(shape, std::collections::HashMap::new(), Default::default()))
	}

	// ==================== Clean ====================

	fn clean(&self) -> Result<Self, Error> {
		self.rebuilt(|h| ffi::builder_clean(&self.inner, h)).map_err(|e| Error::Clean(e.what().into()))
	}

	// ==================== Boolean primitive ====================

	fn boolean<'a>(solids: impl IntoIterator<Item = &'a Self>, clauses: impl IntoIterator<Item = i64>) -> Boolean<Self>
	where
		Self: 'a,
	{
		let solids: Vec<Solid> = solids.into_iter().cloned().collect();
		Boolean::from_parts(solids, clauses.into_iter().collect())
	}
	fn boolean_build(b: &Boolean<Self>) -> Result<Vec<Self>, Error> {
		// CellsBuilder ベースの一括評価。DIMACS-flat DNF (`clauses`) を C++ 側に渡す。
		let (solids, clauses) = (b.solids(), b.clauses());
		if solids.is_empty() || clauses.is_empty() {
			return Err(Error::NotOne(0));
		}
		debug_assert!(clauses.last() == Some(&0), "clauses must be 0-terminated");

		let solid_vec = Solid::to_ffi(solids);
		let mut flat: Vec<u64> = Vec::new();
		let result = ffi::builder_cells(&solid_vec, clauses, &mut flat).map_err(|e| Error::Boolean(e.what().into()))?;
		let history = decode_history(&flat);

		// A result face takes the colour of the operand face it descends from.
		let colormap = {
			let mut m = std::collections::HashMap::new();
			for l in &history {
				let Some(post) = l.post() else { continue };
				if let Some(&c) = solids.iter().find_map(|s| s.colormap.get(&l.src())) {
					m.entry(post).or_insert(c);
				}
			}
			m
		};

		// No history carries a solid colour — the result volume descends from no single
		// operand. Take the left operand's, as Fusion 360 does.
		let solid_color = solids[0].colormap.get(&solids[0].id()).copied();

		let mut out = Solid::from_ffi(&result, &colormap, &history);
		// Only now do the result solids exist, so only now can their ids be keyed.
		if let Some(c) = solid_color {
			for s in &mut out {
				let id = s.id();
				s.colormap_mut().insert(id, c);
			}
		}
		Ok(out)
	}

	// --- I/O (delegates to super::io helpers) ---

	fn read_step<R: std::io::Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		super::io::read_step(reader)
	}

	fn read_brep<R: std::io::Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		super::io::read_brep(reader)
	}

	fn write_step<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error>
	where
		Self: 'a,
	{
		super::io::write_step(solids, writer)
	}

	fn write_brep<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error>
	where
		Self: 'a,
	{
		super::io::write_brep(solids, writer)
	}

	fn mesh<'a>(solids: impl IntoIterator<Item = &'a Self>, options: crate::traits::Tessellation) -> Result<crate::common::mesh::Mesh, Error>
	where
		Self: 'a,
	{
		super::io::mesh(solids, options)
	}

	// ==================== Queries ====================

	fn volume(&self) -> f64 {
		ffi::shape_volume(&self.inner).unwrap_or(f64::NAN)
	}

	fn center(&self) -> DVec3 {
		let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
		if ffi::shape_center_of_mass(&self.inner, &mut x, &mut y, &mut z).is_err() {
			return DVec3::NAN;
		}
		DVec3::new(x, y, z)
	}

	fn inertia(&self) -> glam::DMat3 {
		let (mut m00, mut m01, mut m02) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut m10, mut m11, mut m12) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut m20, mut m21, mut m22) = (0.0_f64, 0.0_f64, 0.0_f64);
		if ffi::shape_inertia_tensor(&self.inner, &mut m00, &mut m01, &mut m02, &mut m10, &mut m11, &mut m12, &mut m20, &mut m21, &mut m22).is_err() {
			return glam::DMat3::NAN;
		}
		// OCCT fills row-major; DMat3::from_cols_array is column-major so
		// transpose when handing the components over.
		glam::DMat3::from_cols_array(&[m00, m10, m20, m01, m11, m21, m02, m12, m22])
	}

	fn contains(&self, point: DVec3) -> bool {
		ffi::shape_contains_point(&self.inner, point.x, point.y, point.z).unwrap_or(false)
	}

	fn bounding_box(&self) -> [DVec3; 2] {
		let (mut xmin, mut ymin, mut zmin) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut xmax, mut ymax, mut zmax) = (0.0_f64, 0.0_f64, 0.0_f64);
		if ffi::shape_bounding_box(&self.inner, &mut xmin, &mut ymin, &mut zmin, &mut xmax, &mut ymax, &mut zmax).is_err() {
			return [DVec3::NAN; 2];
		}
		[DVec3::new(xmin, ymin, zmin), DVec3::new(xmax, ymax, zmax)]
	}

	// ==================== Color ====================

	fn color(self, color: impl Into<crate::common::color::Color>) -> Self {
		let c = color.into();
		// Existing face colours are dropped: painting the whole solid is a statement
		// about the whole solid.
		let colormap = std::collections::HashMap::from([(ffi::solid_tshape_id(&self.inner), c)]);
		Self::new(self.inner, colormap, self.history)
	}

	fn color_clear(self) -> Self {
		Self::new(self.inner, std::collections::HashMap::new(), self.history)
	}
}

// ==================== impl Transform for Solid ====================

impl Transform for Solid {
	fn translate(self, translation: DVec3) -> Self {
		let inner = ffi::transform_translate(&self.inner, translation.x, translation.y, translation.z).unwrap_or_else(|e| panic!("Solid::translate: {}", e.what()));
		// Moved() shares the TShape and changes only the solid's own Location, so every
		// id, the colormap and the history stay valid; only the caches are rebuilt.
		Solid::new(inner, self.colormap, self.history)
	}

	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self {
		let inner = ffi::transform_rotate(&self.inner, axis_origin.x, axis_origin.y, axis_origin.z, axis_direction.x, axis_direction.y, axis_direction.z, angle).unwrap_or_else(|e| panic!("Solid::rotate: {}", e.what()));
		Solid::new(inner, self.colormap, self.history)
	}

	// scale/mirror cannot use Moved(): since OCCT Fix 0027457 (v7.6), TopLoc_Location
	// rejects gp_Trsf with scale != 1 or negative determinant, so BRepBuilderAPI_Transform
	// rebuilds the topology and the result carries a Modified history instead.
	// See: https://dev.opencascade.org/content/how-scale-or-mirror-shape

	fn scale(self, center: DVec3, factor: f64) -> Self {
		self.rebuilt(|h| ffi::transform_scale(&self.inner, center.x, center.y, center.z, factor, h)).unwrap_or_else(|e| panic!("Solid::scale: {}", e.what()))
	}

	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self {
		self.rebuilt(|h| ffi::transform_mirror(&self.inner, plane_origin.x, plane_origin.y, plane_origin.z, plane_normal.x, plane_normal.y, plane_normal.z, h)).unwrap_or_else(|e| panic!("Solid::mirror: {}", e.what()))
	}
}

// A shallow handle copy, as OCCT's own `TopoDS_Shape` copy is: the topology is shared
// and immutable, so every id, the colormap and the history stay valid.
impl Clone for Solid {
	fn clone(&self) -> Self {
		Solid::new(ffi::clone_solid_handle(&self.inner), self.colormap.clone(), self.history.clone())
	}
}
