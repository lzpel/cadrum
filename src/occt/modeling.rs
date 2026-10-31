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
		if tools.is_empty() {
			return Err(Error::Validation("boolean: no tools".into()));
		}
		let inputs: Vec<_> = std::iter::once(self).chain(tools.iter().copied()).collect();
		Ok(Self::operation(ffi::op_boolean(&Self::vector(inputs.iter().copied()), operation as u32).map_err(|e| kernel("boolean", e))?, &inputs))
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
