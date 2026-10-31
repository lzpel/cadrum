//! Native modeling operations on reusable topology.
use super::{
	ffi,
	report::{axis, finite, kernel, positive, supported},
};
use crate::{DVec3, Error, Face, OperationResult, Shape, ShapeKind, Wire};
impl Shape {
	/// Normalize planar loop nesting, including holes, islands, and disconnected regions.
	pub fn profile(wires: &[&Wire], tolerance: f64) -> Result<OperationResult<Self>, Error> {
		positive(tolerance, "profile tolerance")?;
		let inputs: Vec<&Shape> = wires.iter().map(|w| &***w).collect();
		Ok(Self::operation(ffi::op_profile(&Self::vector(inputs.iter().copied()), tolerance, false).map_err(|e| kernel("profile", e))?, &inputs))
	}
	/// Extrude a face or compound profile, retaining every hole and disconnected region.
	pub fn extrude(&self, direction: DVec3) -> Result<OperationResult<Self>, Error> {
		axis(direction, "extrude direction")?;
		if !matches!(self.kind(), ShapeKind::Face | ShapeKind::Compound) {
			return Err(supported("extrude", self.kind()));
		}
		Ok(Self::operation(ffi::op_prism(&self.inner, direction.x, direction.y, direction.z).map_err(|e| kernel("extrude", e))?, &[self]))
	}
	/// Revolve a native profile through a signed angle in radians, at most one full turn.
	pub fn revolve(&self, origin: DVec3, direction: DVec3, angle: f64) -> Result<OperationResult<Self>, Error> {
		finite(origin.to_array().into_iter().chain([angle]), "revolve")?;
		axis(direction, "revolve axis")?;
		if angle == 0.0 || angle.abs() > std::f64::consts::TAU + 1e-12 {
			return Err(Error::Validation("revolve angle must be nonzero and at most one turn".into()));
		}
		if !matches!(self.kind(), ShapeKind::Face | ShapeKind::Compound) {
			return Err(supported("revolve", self.kind()));
		}
		let values = [origin.x, origin.y, origin.z, direction.x, direction.y, direction.z, angle];
		Ok(Self::operation(ffi::op_revolution(&self.inner, &values).map_err(|e| kernel("revolve", e))?, &[self]))
	}
}
impl Face {
	/// Construct a planar face from an outer wire and contained, disjoint hole wires.
	pub fn from_wires(outer: &Wire, holes: &[&Wire], tolerance: f64) -> Result<Self, Error> {
		positive(tolerance, "face tolerance")?;
		let inputs: Vec<&Shape> = std::iter::once(&**outer).chain(holes.iter().map(|w| &***w)).collect();
		Shape::operation(ffi::op_profile(&Shape::vector(inputs.iter().copied()), tolerance, true).map_err(|e| kernel("face from wires", e))?, &inputs).value.as_face()
	}
}

#[derive(Clone, Copy, Debug)]
pub enum ShapeTransform {
	Translation(DVec3),
	Rotation { origin: DVec3, axis: DVec3, angle: f64 },
	Scale { origin: DVec3, factor: f64 },
	Mirror { origin: DVec3, normal: DVec3 },
}
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum BooleanOp {
	Union = 1,
	Intersection = 0,
	Difference = 2,
}
impl Shape {
	pub fn copied(&self) -> Result<OperationResult<Self>, Error> {
		Ok(Self::operation(ffi::op_copy(&self.inner).map_err(|e| kernel("copy", e))?, &[self]))
	}
	pub fn transform_with_history(&self, transform: ShapeTransform) -> Result<OperationResult<Self>, Error> {
		let (kind, values) = match transform {
			ShapeTransform::Translation(v) => (0, v.to_array().to_vec()),
			ShapeTransform::Rotation { origin, axis: direction, angle } => {
				axis(direction, "rotation axis")?;
				(1, vec![origin.x, origin.y, origin.z, direction.x, direction.y, direction.z, angle])
			}
			ShapeTransform::Scale { origin, factor } => {
				positive(factor.abs(), "scale factor")?;
				(2, vec![origin.x, origin.y, origin.z, factor])
			}
			ShapeTransform::Mirror { origin, normal } => {
				axis(normal, "mirror normal")?;
				(3, vec![origin.x, origin.y, origin.z, normal.x, normal.y, normal.z])
			}
		};
		finite(values.iter().copied(), "transform")?;
		Ok(Self::operation(ffi::op_transform(&self.inner, kind, &values).map_err(|e| kernel("transform", e))?, &[self]))
	}
	pub fn boolean(&self, operation: BooleanOp, tools: &[&Shape]) -> Result<OperationResult<Self>, Error> {
		Self::boolean_batch(operation, &[self], tools, BooleanOptions::default(), &crate::OperationControl::default())
	}
	pub fn boolean_batch(operation: BooleanOp, arguments: &[&Shape], tools: &[&Shape], options: BooleanOptions, control: &crate::OperationControl) -> Result<OperationResult<Self>, Error> {
		finite([options.fuzzy_tolerance], "Boolean fuzzy tolerance")?;
		if options.fuzzy_tolerance < 0.0 || arguments.is_empty() || tools.is_empty() {
			return Err(Error::Validation("Boolean requires arguments, tools, and nonnegative tolerance".into()));
		}
		control.check("boolean")?;
		let inputs: Vec<_> = arguments.iter().chain(tools).copied().collect();
		let config = ffi::BooleanConfig { fuzzy: options.fuzzy_tolerance, glue: options.glue as u32, non_destructive: options.non_destructive, parallel: options.parallel };
		let result = ffi::op_boolean(&Self::vector(inputs.iter().copied()), arguments.len(), operation as u32, &config, control);
		control.check("boolean")?;
		Ok(Self::operation(result.map_err(|e| kernel("boolean", e))?, &inputs))
	}

	pub fn fillet_edges(&self, radius: f64, edges: &[&crate::Edge]) -> Result<OperationResult<Self>, Error> {
		self.blend(radius, edges, false)
	}
	pub fn chamfer_edges(&self, distance: f64, edges: &[&crate::Edge]) -> Result<OperationResult<Self>, Error> {
		self.blend(distance, edges, true)
	}
	fn blend(&self, radius: f64, edges: &[&crate::Edge], chamfer: bool) -> Result<OperationResult<Self>, Error> {
		positive(radius, "blend size")?;
		let selected: Vec<Shape> = edges.iter().map(|e| Shape::from(*e)).collect();
		Ok(Self::operation(ffi::op_blend(&self.inner, &Self::vector(&selected), radius, chamfer).map_err(|e| kernel(if chamfer { "chamfer" } else { "fillet" }, e))?, &[self]))
	}
	pub fn unify_faces(&self, linear_tolerance: f64, angular_tolerance: f64) -> Result<OperationResult<Self>, Error> {
		positive(linear_tolerance, "unify linear tolerance")?;
		positive(angular_tolerance, "unify angular tolerance")?;
		Ok(Self::operation(ffi::op_unify(&self.inner, linear_tolerance, angular_tolerance).map_err(|e| kernel("unify faces", e))?, &[self]))
	}
	pub fn sew_with_history(shapes: &[&Shape], tolerance: f64) -> Result<OperationResult<Self>, Error> {
		positive(tolerance, "sew tolerance")?;
		Ok(Self::operation(ffi::op_sew_history(&Self::vector(shapes.iter().copied()), tolerance).map_err(|e| kernel("sew", e))?, shapes))
	}
	/// Resolve a one-based report/history index against the original operand or result.
	pub fn topology_at(&self, index: usize) -> Result<Self, Error> {
		let index = u32::try_from(index).map_err(|_| Error::Validation("topology index overflow".into()))?;
		Ok(Self::new(ffi::op_topology_at(&self.inner, index).map_err(|e| kernel("topology index", e))?))
	}
}

#[derive(Clone, Copy, Debug)]
pub struct Plane {
	pub origin: DVec3,
	pub normal: DVec3,
}
impl Plane {
	pub fn face(&self) -> Result<Face, Error> {
		finite(self.origin.to_array(), "plane origin")?;
		axis(self.normal, "plane normal")?;
		Shape::new(ffi::op_plane(&[self.origin.x, self.origin.y, self.origin.z, self.normal.x, self.normal.y, self.normal.z]).map_err(|e| kernel("plane", e))?).as_face()
	}
}
#[derive(Clone, Copy, Debug)]
pub struct SectionOptions {
	pub tolerance: f64,
	pub approximation: bool,
	pub pcurves_on_first: bool,
	pub pcurves_on_second: bool,
}
impl Default for SectionOptions {
	fn default() -> Self {
		Self { tolerance: 1e-7, approximation: false, pcurves_on_first: true, pcurves_on_second: true }
	}
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JoinType {
	Arc,
	Intersection,
}
#[derive(Clone, Copy, Debug)]
pub struct OffsetOptions {
	pub tolerance: f64,
	pub join: JoinType,
	pub intersect_all: bool,
}
impl Default for OffsetOptions {
	fn default() -> Self {
		Self { tolerance: 1e-7, join: JoinType::Intersection, intersect_all: false }
	}
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartitionSide {
	Positive,
	Negative,
	OnPlane,
}
#[derive(Debug)]
pub struct Partition {
	pub shape: Shape,
	pub side: PartitionSide,
	pub ancestors: Vec<crate::TopologyRef>,
}
#[derive(Debug)]
pub struct PlaneSplit {
	pub result: OperationResult<Shape>,
	pub partitions: Vec<Partition>,
}
impl Shape {
	pub fn section(&self, other: &Shape, options: SectionOptions) -> Result<OperationResult<Self>, Error> {
		positive(options.tolerance, "section tolerance")?;
		Ok(Self::operation(ffi::op_section(&self.inner, &other.inner, options.tolerance, &[options.approximation, options.pcurves_on_first, options.pcurves_on_second]).map_err(|e| kernel("section", e))?, &[self, other]))
	}
	pub fn split(&self, tools: &[&Shape], tolerance: f64) -> Result<OperationResult<Self>, Error> {
		positive(tolerance, "split tolerance")?;
		if tools.is_empty() {
			return Err(Error::Validation("split requires at least one tool".into()));
		}
		let inputs: Vec<_> = std::iter::once(self).chain(tools.iter().copied()).collect();
		Ok(Self::operation(ffi::op_split(&Self::vector(inputs.iter().copied()), tolerance).map_err(|e| kernel("split", e))?, &inputs))
	}
	pub fn children(&self) -> Result<Vec<Self>, Error> {
		let data = ffi::op_children(&self.inner).map_err(|e| kernel("children", e))?;
		Ok((0..data.len())
			.map(|i| {
				let mut s = Self::new(ffi::native_take(&data, i));
				s.colormap = self.colormap.clone();
				s
			})
			.collect())
	}
	pub fn split_by_plane(&self, plane: Plane, tolerance: f64) -> Result<PlaneSplit, Error> {
		let tool = Shape::from(&plane.face()?);
		let result = self.split(&[&tool], tolerance)?;
		let pieces = if result.value.kind() == ShapeKind::Compound { result.value.children()? } else { vec![result.value.copied()?.value] };
		let mut partitions = Vec::new();
		for shape in pieces {
			let center = ffi::op_centroid(&shape.inner).map_err(|e| kernel("partition centroid", e))?;
			let distance = (DVec3::from_slice(&center) - plane.origin).dot(plane.normal.normalize());
			let side = if distance > tolerance {
				PartitionSide::Positive
			} else if distance < -tolerance {
				PartitionSide::Negative
			} else {
				PartitionSide::OnPlane
			};
			let members = ffi::op_member_indices(&result.value.inner, &shape.inner).map_err(|e| kernel("partition provenance", e))?;
			let mut ancestors = Vec::new();
			for relation in &result.history.relations {
				if relation.target.is_some_and(|t| members.contains(&(t.index as u32))) && !ancestors.contains(&relation.source) {
					ancestors.push(relation.source);
				}
			}
			partitions.push(Partition { shape, side, ancestors });
		}
		Ok(PlaneSplit { result, partitions })
	}
	pub fn offset_surface(&self, distance: f64, options: OffsetOptions) -> Result<OperationResult<Self>, Error> {
		self.surface_offset(distance, options, false)
	}
	pub fn thicken_with_options(&self, thickness: f64, options: OffsetOptions) -> Result<OperationResult<Self>, Error> {
		self.surface_offset(thickness, options, true)
	}
	fn surface_offset(&self, distance: f64, options: OffsetOptions, thicken: bool) -> Result<OperationResult<Self>, Error> {
		positive(distance.abs(), "offset distance")?;
		positive(options.tolerance, "offset tolerance")?;
		if !matches!(self.kind(), ShapeKind::Face | ShapeKind::Shell) {
			return Err(supported(if thicken { "thicken" } else { "surface offset" }, self.kind()));
		}
		if thicken && self.kind() == ShapeKind::Shell && self.is_closed()? {
			return Err(Error::Validation("thicken requires an open shell".into()));
		}
		Ok(Self::operation(ffi::op_surface_offset(&self.inner, distance, options.tolerance, options.join == JoinType::Intersection, options.intersect_all, thicken).map_err(|e| kernel(if thicken { "thicken" } else { "surface offset" }, e))?, &[self]))
	}
}

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum GluePolicy {
	None = 0,
	Shift = 1,
	Full = 2,
}
#[derive(Clone, Copy, Debug)]
pub struct BooleanOptions {
	pub fuzzy_tolerance: f64,
	pub glue: GluePolicy,
	pub non_destructive: bool,
	pub parallel: bool,
}
impl Default for BooleanOptions {
	fn default() -> Self {
		Self { fuzzy_tolerance: 0.0, glue: GluePolicy::None, non_destructive: true, parallel: false }
	}
}
