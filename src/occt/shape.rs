//! Native topology ownership, including intentional open and empty results.
use super::{ffi, solid::Solid};
use crate::{Color, DVec3, Edge, Error, Face, Mesh, Tessellation};
use std::{
	collections::HashMap,
	io::{Read, Write},
};

/// The native topology category. An empty compound is a valid empty result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ShapeKind {
	Compound = 0,
	CompSolid = 1,
	Solid = 2,
	Shell = 3,
	Face = 4,
	Wire = 5,
	Edge = 6,
	Vertex = 7,
}

/// An owning native shape. Views share topology; clone copies geometry; RAII releases handles.
/// This type is neither Send nor Sync, because views can share native geometry.
pub struct Shape {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	colormap: HashMap<u64, Color>,
}

fn failure(e: cxx::Exception) -> Error {
	Error::Shape(e.what().to_owned())
}
fn positive(value: f64, name: &str) -> Result<(), Error> {
	if value.is_finite() && value > 0.0 {
		Ok(())
	} else {
		Err(Error::Validation(format!("{name} must be finite and positive")))
	}
}

impl Shape {
	fn new(inner: cxx::UniquePtr<ffi::TopoDS_Shape>) -> Self {
		Self { inner, colormap: HashMap::new() }
	}
	fn vector<'a>(shapes: impl IntoIterator<Item = &'a Shape>) -> cxx::UniquePtr<cxx::CxxVector<ffi::TopoDS_Shape>> {
		let mut out = ffi::native_vec_new();
		for s in shapes {
			ffi::native_vec_push(out.pin_mut(), &s.inner);
		}
		out
	}
	/// Return the exact native category, including compounds containing mixed topology.
	pub fn kind(&self) -> ShapeKind {
		match ffi::native_kind(&self.inner) {
			0 => ShapeKind::Compound,
			1 => ShapeKind::CompSolid,
			2 => ShapeKind::Solid,
			3 => ShapeKind::Shell,
			4 => ShapeKind::Face,
			5 => ShapeKind::Wire,
			6 => ShapeKind::Edge,
			7 => ShapeKind::Vertex,
			_ => unreachable!("Shape never owns null topology"),
		}
	}
	pub fn is_valid(&self) -> Result<bool, Error> {
		ffi::native_valid(&self.inner).map_err(failure)
	}
	/// Check topological closure. Closure does not imply validity or solid topology.
	pub fn is_closed(&self) -> Result<bool, Error> {
		ffi::native_closed(&self.inner).map_err(failure)
	}
	/// Check for an empty compound, including nested empty containers.
	pub fn is_empty(&self) -> Result<bool, Error> {
		Ok(ffi::native_empty(&self.inner))
	}
	/// Return unique owning views, including self when its category matches.
	pub fn subshapes(&self, kind: ShapeKind) -> Result<Vec<Shape>, Error> {
		let shapes = ffi::native_subshapes(&self.inner, kind as u32).map_err(failure)?;
		let mut out = Vec::new();
		for i in 0..shapes.len() {
			let mut shape = Self::new(ffi::native_take(&shapes, i));
			shape.colormap = self.colormap.clone();
			out.push(shape);
		}
		Ok(out)
	}
	pub fn vertices(&self) -> Result<Vec<Vertex>, Error> {
		Ok(self.subshapes(ShapeKind::Vertex)?.into_iter().map(Vertex).collect())
	}
	pub fn edges(&self) -> Result<Vec<Edge>, Error> {
		self.subshapes(ShapeKind::Edge)?.iter().map(Shape::as_edge).collect()
	}
	pub fn wires(&self) -> Result<Vec<Wire>, Error> {
		Ok(self.subshapes(ShapeKind::Wire)?.into_iter().map(Wire).collect())
	}
	pub fn faces(&self) -> Result<Vec<Face>, Error> {
		self.subshapes(ShapeKind::Face)?.iter().map(Shape::as_face).collect()
	}
	pub fn shells(&self) -> Result<Vec<Shell>, Error> {
		Ok(self.subshapes(ShapeKind::Shell)?.into_iter().map(Shell).collect())
	}
	pub fn solids(&self) -> Result<Vec<Solid>, Error> {
		self.subshapes(ShapeKind::Solid)?.iter().map(Shape::as_solid).collect()
	}
	pub fn compounds(&self) -> Result<Vec<Compound>, Error> {
		Ok(self.subshapes(ShapeKind::Compound)?.into_iter().map(Compound).collect())
	}
	pub fn as_edge(&self) -> Result<Edge, Error> {
		Ok(Edge { inner: ffi::native_as_edge(&self.inner).map_err(failure)? })
	}
	pub fn as_face(&self) -> Result<Face, Error> {
		Ok(Face::new(ffi::native_as_face(&self.inner).map_err(failure)?))
	}
	/// Require solid topology; this never promotes shells or discards other components.
	pub fn as_solid(&self) -> Result<Solid, Error> {
		Ok(Solid::new(ffi::native_as_solid(&self.inner).map_err(failure)?, self.colormap.clone(), vec![]))
	}
	/// Build a compound retaining every input, including open and disconnected shapes.
	pub fn compound<'a>(shapes: impl IntoIterator<Item = &'a Shape>) -> Result<Compound, Error> {
		let shapes: Vec<_> = shapes.into_iter().collect();
		let mut out = Self::new(ffi::native_build(&Self::vector(shapes.iter().copied()), ShapeKind::Compound as u32).map_err(failure)?);
		for s in shapes {
			out.colormap.extend(s.colormap.iter().map(|(&k, &v)| (k, v)));
		}
		Ok(Compound(out))
	}
	/// Sew faces without solid conversion. Empty input returns an empty compound.
	/// Disconnected shells and isolated faces remain together in the native result.
	pub fn sew<'a>(faces: impl IntoIterator<Item = &'a Face>, tolerance: f64) -> Result<Self, Error> {
		Self::sew_styled(faces, tolerance, &HashMap::new())
	}
	fn sew_styled<'a>(faces: impl IntoIterator<Item = &'a Face>, tolerance: f64, colors: &HashMap<u64, Color>) -> Result<Self, Error> {
		positive(tolerance, "sewing tolerance")?;
		let mut v = ffi::face_vec_new();
		for f in faces {
			ffi::face_vec_push(v.pin_mut(), &f.inner);
		}
		let mut history = Vec::new();
		let mut out = Self::new(ffi::native_sew(&v, tolerance, &mut history).map_err(failure)?);
		for pair in history.chunks_exact(2) {
			if let Some(&color) = colors.get(&pair[1]) {
				out.colormap.insert(pair[0], color);
			}
		}
		Ok(out)
	}
	/// Retain selected member faces, sewing them into their natural output category.
	pub fn extract_faces<'a>(&self, faces: impl IntoIterator<Item = &'a Face>, tolerance: f64) -> Result<Self, Error> {
		let own = self.faces()?;
		let selected: Vec<_> = faces.into_iter().collect();
		if selected.iter().any(|f| !own.iter().any(|v| ffi::native_same_face(&v.inner, &f.inner))) {
			return Err(Error::Validation("extract_faces: foreign face".into()));
		}
		Self::sew_styled(selected, tolerance, &self.colormap)
	}
	/// Remove selected member faces and retain every remaining component.
	pub fn remove_faces<'a>(&self, faces: impl IntoIterator<Item = &'a Face>, tolerance: f64) -> Result<Self, Error> {
		let own = self.faces()?;
		let selected: Vec<_> = faces.into_iter().collect();
		positive(tolerance, "sewing tolerance")?;
		if selected.is_empty() {
			return Ok(self.clone());
		}
		if selected.iter().any(|f| !own.iter().any(|v| ffi::native_same_face(&v.inner, &f.inner))) {
			return Err(Error::Validation("remove_faces: foreign face".into()));
		}
		let sewn = Self::sew_styled(own.iter().filter(|f| !selected.iter().any(|v| ffi::native_same_face(&v.inner, &f.inner))), tolerance, &self.colormap)?;
		let loose = ffi::native_loose(&self.inner).map_err(failure)?;
		if loose.is_empty() {
			return Ok(sewn);
		}
		let mut parts = Vec::new();
		if !sewn.is_empty()? {
			parts.push(sewn);
		}
		for i in 0..loose.len() {
			parts.push(Self::new(ffi::native_take(&loose, i)));
		}
		Ok(Self::compound(&parts)?.into())
	}
	/// Promote exactly one valid closed shell and orient its volume outward.
	pub fn to_solid(&self) -> Result<Solid, Error> {
		Ok(Solid::new(ffi::native_solidify(&self.inner).map_err(failure)?, self.colormap.clone(), vec![]))
	}
	fn transformed(&self, kind: u32, values: &[f64]) -> Result<Self, Error> {
		if !values.iter().all(|v| v.is_finite()) {
			return Err(Error::Validation("transform: non-finite parameter".into()));
		}
		let mut out = Self::new(ffi::native_transform(&self.inner, kind, values).map_err(failure)?);
		for (old, new) in self.faces()?.iter().zip(out.faces()?.iter()) {
			if let Some(&color) = self.colormap.get(&old.id()) {
				out.colormap.insert(new.id(), color);
			}
		}
		Ok(out)
	}
	pub fn translate(&self, v: DVec3) -> Result<Self, Error> {
		self.transformed(0, &v.to_array())
	}
	/// Rotate about an explicit axis, with angle in radians.
	pub fn rotate(&self, origin: DVec3, axis: DVec3, angle: f64) -> Result<Self, Error> {
		positive(axis.length(), "axis length")?;
		self.transformed(1, &[origin.x, origin.y, origin.z, axis.x, axis.y, axis.z, angle])
	}
	pub fn scale(&self, center: DVec3, factor: f64) -> Result<Self, Error> {
		positive(factor.abs(), "absolute scale")?;
		self.transformed(2, &[center.x, center.y, center.z, factor])
	}
	pub fn mirror(&self, origin: DVec3, normal: DVec3) -> Result<Self, Error> {
		positive(normal.length(), "normal length")?;
		self.transformed(3, &[origin.x, origin.y, origin.z, normal.x, normal.y, normal.z])
	}
	/// Return None for empty topology. Coordinates use the model's length units.
	pub fn bounding_box(&self) -> Result<Option<[DVec3; 2]>, Error> {
		let v = ffi::native_bounds(&self.inner).map_err(failure)?;
		Ok(if v.is_empty() { None } else { Some([DVec3::from_slice(&v[..3]), DVec3::from_slice(&v[3..])]) })
	}
	/// Mesh faces and discretize edges; vertices and empty compounds have no triangles.
	pub fn mesh(&self, options: Tessellation) -> Result<Mesh, Error> {
		positive(options.deflection_linear, "linear deflection")?;
		positive(options.deflection_angular, "angular deflection")?;
		let data = ffi::native_mesh(&self.inner, options.deflection_linear, options.deflection_angular, options.relative_linear).map_err(failure)?;
		if !data.success {
			return Err(Error::Tesselation);
		}
		let mut edges = Vec::new();
		for edge in self.edges()? {
			let points = edge.approximation_segments(options);
			if points.is_empty() {
				continue;
			}
			if !edges.is_empty() {
				edges.push(DVec3::NAN);
			}
			edges.extend(points);
		}
		Ok(Mesh {
			vertices: data.vertices.chunks_exact(3).map(DVec3::from_slice).collect(),
			normals: data.normals.chunks_exact(3).map(DVec3::from_slice).collect(),
			indices: data.indices.into_iter().map(|i| i as usize).collect(),
			face_ids: data.face_tshape_ids,
			colormap: self.colormap.clone(),
			edges,
		})
	}
	/// Write exact geometry as binary BRep, without mesh, colors, or operation history.
	/// Binary format compatibility follows the linked OCCT version.
	pub fn write_brep<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
		writer.write_all(&ffi::native_write_brep(&self.inner).map_err(failure)?)?;
		Ok(())
	}
	/// Restore the root category and all components, including empty compounds.
	pub fn read_brep<R: Read>(reader: &mut R) -> Result<Self, Error> {
		let mut bytes = Vec::new();
		reader.read_to_end(&mut bytes)?;
		Ok(Self::new(ffi::native_read_brep(&bytes).map_err(failure)?))
	}
	/// Thicken a face or open shell with intersection joins and boundary walls.
	/// Signed thickness follows normals; coincidence tolerance is OCCT's Precision::Confusion().
	pub fn thicken(&self, thickness: f64) -> Result<Self, Error> {
		positive(thickness.abs(), "absolute thickness")?;
		let mut out = Self::new(ffi::native_thicken(&self.inner, thickness).map_err(failure)?);
		out.colormap = self.colormap.clone();
		Ok(out)
	}
}
impl Clone for Shape {
	fn clone(&self) -> Self {
		let mut out = Self::new(ffi::native_copy(&self.inner).expect("Shape::clone"));
		for (old, new) in self.faces().expect("Shape::clone faces").iter().zip(out.faces().expect("Shape::clone faces").iter()) {
			if let Some(&c) = self.colormap.get(&old.id()) {
				out.colormap.insert(new.id(), c);
			}
		}
		out
	}
}
impl std::fmt::Debug for Shape {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Shape").field("kind", &self.kind()).finish()
	}
}
impl From<&Solid> for Shape {
	fn from(s: &Solid) -> Self {
		let mut out = Self::new(ffi::native_from_solid(s.inner()));
		out.colormap = s.colormap().clone();
		// Expand whole-solid styling so subshape views retain it.
		if let Some(&c) = s.colormap().get(&s.id()) {
			for face in s.iter_face() {
				out.colormap.entry(face.id()).or_insert(c);
			}
		}
		out
	}
}
impl From<&Face> for Shape {
	fn from(s: &Face) -> Self {
		Self::new(ffi::native_from_face(&s.inner))
	}
}
impl From<&Edge> for Shape {
	fn from(s: &Edge) -> Self {
		Self::new(ffi::native_from_edge(&s.inner))
	}
}

macro_rules! topology {
	($name:ident, $kind:ident) => {
		#[doc = concat!("An owning ", stringify!($kind), " view with Shape inspection and operations.")]
		#[derive(Clone, Debug)]
		pub struct $name(Shape);
		impl std::ops::Deref for $name {
			type Target = Shape;
			fn deref(&self) -> &Shape {
				&self.0
			}
		}
		impl From<$name> for Shape {
			fn from(value: $name) -> Shape {
				value.0
			}
		}
		impl TryFrom<Shape> for $name {
			type Error = Error;
			fn try_from(value: Shape) -> Result<Self, Error> {
				if value.kind() == ShapeKind::$kind {
					Ok(Self(value))
				} else {
					Err(Error::Validation(concat!("expected ", stringify!($kind)).into()))
				}
			}
		}
	};
}
topology!(Vertex, Vertex);
topology!(Wire, Wire);
topology!(Shell, Shell);
topology!(Compound, Compound);
impl Vertex {
	pub fn new(point: DVec3) -> Result<Self, Error> {
		if !point.is_finite() {
			return Err(Error::Validation("vertex: non-finite position".into()));
		}
		Ok(Self(Shape::new(ffi::native_vertex(point.x, point.y, point.z).map_err(failure)?)))
	}
	pub fn point(&self) -> Result<DVec3, Error> {
		Ok(DVec3::from_slice(&ffi::native_point(&self.inner).map_err(failure)?))
	}
}
impl Wire {
	/// Connect edges in input order, preserving intentional open ends.
	pub fn new<'a>(edges: impl IntoIterator<Item = &'a Edge>) -> Result<Self, Error> {
		let shapes: Vec<_> = edges.into_iter().map(Shape::from).collect();
		Ok(Self(Shape::new(ffi::native_build(&Shape::vector(&shapes), ShapeKind::Wire as u32).map_err(failure)?)))
	}
}
impl Shell {
	/// Build a shell from faces already sharing native edges; use Shape::sew for independent faces.
	pub fn new<'a>(faces: impl IntoIterator<Item = &'a Face>) -> Result<Self, Error> {
		let shapes: Vec<_> = faces.into_iter().map(Shape::from).collect();
		Ok(Self(Shape::new(ffi::native_build(&Shape::vector(&shapes), ShapeKind::Shell as u32).map_err(failure)?)))
	}
}
