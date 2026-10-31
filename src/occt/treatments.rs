//! Explicit 3D chamfers, variable-radius fillets, and planar corner treatments.
use super::{
	ffi,
	report::{finite, kernel, positive},
};
use crate::{Edge, Error, Face, OperationControl, OperationResult, Shape, Vertex};
#[derive(Clone, Copy, Debug)]
pub enum ChamferKind {
	TwoDistances { first: f64, second: f64 },
	DistanceAngle { distance: f64, angle: f64 },
}
#[derive(Clone, Copy, Debug)]
pub struct ChamferSpec<'a> {
	pub edge: &'a Edge,
	pub reference_face: &'a Face,
	pub kind: ChamferKind,
}
#[derive(Clone, Debug)]
pub struct VariableFillet<'a> {
	pub edge: &'a Edge,
	pub radii: Vec<[f64; 2]>,
}
#[derive(Clone, Copy, Debug)]
pub struct PlanarChamfer<'a> {
	pub first_edge: &'a Edge,
	pub second_edge: &'a Edge,
	pub first_distance: f64,
	pub second_distance: f64,
}
impl Shape {
	pub fn chamfer(&self, specifications: &[ChamferSpec<'_>], control: &OperationControl) -> Result<OperationResult<Self>, Error> {
		if specifications.is_empty() {
			return Err(Error::Validation("chamfer: no selected edges".into()));
		}
		let mut shapes = Vec::new();
		let mut values = Vec::new();
		for spec in specifications {
			shapes.push(Shape::from(spec.edge));
			shapes.push(Shape::from(spec.reference_face));
			match spec.kind {
				ChamferKind::TwoDistances { first, second } => {
					positive(first, "first chamfer distance")?;
					positive(second, "second chamfer distance")?;
					values.extend([0.0, first, second]);
				}
				ChamferKind::DistanceAngle { distance, angle } => {
					positive(distance, "chamfer distance")?;
					positive(angle, "chamfer angle")?;
					if angle >= std::f64::consts::FRAC_PI_2 {
						return Err(Error::Validation("chamfer angle must be below pi/2".into()));
					}
					values.extend([1.0, distance, angle]);
				}
			}
		}
		control.check("chamfer")?;
		let result = ffi::op_chamfer(&self.inner, &Self::vector(&shapes), &values, control);
		control.check("chamfer")?;
		Ok(Self::operation(result.map_err(|e| kernel("chamfer", e))?, &[self]))
	}
	pub fn variable_fillet(&self, specifications: &[VariableFillet<'_>], control: &OperationControl) -> Result<OperationResult<Self>, Error> {
		if specifications.is_empty() {
			return Err(Error::Validation("fillet: no selected edges".into()));
		}
		let mut shapes = Vec::new();
		let mut values = Vec::new();
		let mut counts = Vec::new();
		for spec in specifications {
			finite(spec.radii.iter().flatten().copied(), "fillet law")?;
			if spec.radii.len() < 2 || spec.radii.len() > i32::MAX as usize || spec.radii[0][0] != 0.0 || spec.radii.last().unwrap()[0] != 1.0 || spec.radii.windows(2).any(|p| p[0][0] >= p[1][0]) || spec.radii.iter().any(|p| p[1] <= 0.0) {
				return Err(Error::Validation("fillet radius law requires positive radii and increasing parameters from zero to one".into()));
			}
			shapes.push(Shape::from(spec.edge));
			counts.push(spec.radii.len() as u32);
			values.extend(spec.radii.iter().flatten().copied());
		}
		control.check("variable fillet")?;
		let result = ffi::op_variable_fillet(&self.inner, &Self::vector(&shapes), &values, &counts, control);
		control.check("variable fillet")?;
		Ok(Self::operation(result.map_err(|e| kernel("variable fillet", e))?, &[self]))
	}
	pub fn reversed(&self) -> Result<OperationResult<Self>, Error> {
		Ok(Self::operation(ffi::op_reversed(&self.inner).map_err(|e| kernel("reverse orientation", e))?, &[self]))
	}
}
impl Face {
	pub fn fillet_corners(&self, radius: f64, vertices: &[&Vertex]) -> Result<OperationResult<Shape>, Error> {
		positive(radius, "planar fillet radius")?;
		if vertices.is_empty() {
			return Err(Error::Validation("planar fillet: no selected corners".into()));
		}
		let inputs: Vec<&Shape> = vertices.iter().map(|v| &***v).collect();
		let source = Shape::from(self);
		Ok(Shape::operation(ffi::op_planar_blend(&self.inner, &Shape::vector(inputs), &[radius], false).map_err(|e| kernel("planar fillet", e))?, &[&source]))
	}
	pub fn chamfer_corners(&self, specifications: &[PlanarChamfer<'_>]) -> Result<OperationResult<Shape>, Error> {
		if specifications.is_empty() {
			return Err(Error::Validation("planar chamfer: no selected corners".into()));
		}
		let mut inputs = Vec::new();
		let mut values = Vec::new();
		for spec in specifications {
			positive(spec.first_distance, "first planar chamfer distance")?;
			positive(spec.second_distance, "second planar chamfer distance")?;
			inputs.push(Shape::from(spec.first_edge));
			inputs.push(Shape::from(spec.second_edge));
			values.extend([spec.first_distance, spec.second_distance]);
		}
		let source = Shape::from(self);
		Ok(Shape::operation(ffi::op_planar_blend(&self.inner, &Shape::vector(&inputs), &values, true).map_err(|e| kernel("planar chamfer", e))?, &[&source]))
	}
}
