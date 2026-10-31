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
