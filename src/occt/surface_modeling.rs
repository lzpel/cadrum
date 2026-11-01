//! Native surface construction and modification.
use super::{
	ffi,
	report::{finite, kernel, positive},
};
use crate::{DVec3, Edge, Error, Face, OperationControl, OperationResult, Shape, ShapeKind, Wire};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FillContinuity {
	Position,
	Tangent,
	Curvature,
}
#[derive(Clone, Copy, Debug)]
pub struct FillBoundary<'a> {
	pub edge: &'a Edge,
	pub support: Option<&'a Face>,
	pub continuity: FillContinuity,
}
#[derive(Clone, Copy, Debug)]
pub struct FillingOptions<'a> {
	/// Optional starting surface; OCCT requires orthogonal local U/V directions.
	pub initial_surface: Option<&'a Face>,
	pub tolerance: f64,
	pub angular_tolerance: f64,
	pub curvature_tolerance: f64,
	pub max_degree: u32,
	pub max_segments: u32,
}
impl Default for FillingOptions<'_> {
	fn default() -> Self {
		Self { initial_surface: None, tolerance: 1e-4, angular_tolerance: 0.01, curvature_tolerance: 0.1, max_degree: 8, max_segments: 9 }
	}
}
#[derive(Debug)]
pub struct FillingResult {
	pub result: OperationResult<Shape>,
	/// Native constraint errors: distance, normal angle in radians, and curvature difference.
	pub maximum_errors: [f64; 3],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SurfaceBoundary {
	UMin,
	UMax,
	VMin,
	VMax,
}

impl Face {
	/// Fill one ordered closed boundary, optionally constrained by points and support faces.
	/// G1/G2 constraints require pcurves on their explicit support faces.
	pub fn fill(boundary: &[FillBoundary<'_>], points: &[DVec3], options: FillingOptions<'_>, control: &OperationControl) -> Result<FillingResult, Error> {
		positive(options.tolerance, "filling tolerance")?;
		positive(options.angular_tolerance, "filling angular tolerance")?;
		positive(options.curvature_tolerance, "filling curvature tolerance")?;
		finite(points.iter().flat_map(|p| p.to_array()), "filling points")?;
		if boundary.is_empty() || boundary.len() > 100_000 || points.len() > 100_000 || !(2..=25).contains(&options.max_degree) || !(1..=10_000).contains(&options.max_segments) {
			return Err(Error::Validation("filling requires a boundary, degree 2..=25, segments 1..=10000, and at most 100000 edges/points".into()));
		}
		let mut inputs: Vec<Shape> = boundary.iter().map(|b| Shape::from(b.edge)).collect();
		let mut supports = Vec::new();
		for b in boundary {
			if b.continuity != FillContinuity::Position && b.support.is_none() {
				return Err(Error::Validation("tangent/curvature filling requires an explicit support face".into()));
			}
			supports.push(if let Some(face) = b.support {
				inputs.push(Shape::from(face));
				(inputs.len() - 1) as i32
			} else {
				-1
			});
		}
		let initial = if let Some(face) = options.initial_surface {
			inputs.push(Shape::from(face));
			(inputs.len() - 1) as i32
		} else {
			-1
		};
		let config = ffi::FillingConfig {
			initial,
			tolerance: options.tolerance,
			angular_tolerance: options.angular_tolerance,
			curvature_tolerance: options.curvature_tolerance,
			max_degree: options.max_degree,
			max_segments: options.max_segments,
		};
		control.check("surface filling")?;
		let data = ffi::op_fill(&Shape::vector(&inputs), &supports, &boundary.iter().map(|b| b.continuity as u32).collect::<Vec<_>>(), &points.iter().flat_map(|p| p.to_array()).collect::<Vec<_>>(), &config, control);
		control.check("surface filling")?;
		let data = data.map_err(|e| kernel("surface filling", e))?;
		Ok(FillingResult { result: Shape::operation(data.operation, &inputs.iter().collect::<Vec<_>>()), maximum_errors: [data.distance_error, data.angle_error, data.curvature_error] })
	}

	/// Replace trims on the underlying surface; original holes and trims are not inherited.
	/// Boundary curves must already lie on that surface within the supplied tolerance.
	pub fn retrim_surface(&self, outer: &Wire, holes: &[&Wire], tolerance: f64) -> Result<OperationResult<Shape>, Error> {
		positive(tolerance, "surface trim tolerance")?;
		let face = Shape::from(self);
		let inputs: Vec<&Shape> = std::iter::once(&face).chain(std::iter::once(&**outer)).chain(holes.iter().map(|w| &***w)).collect();
		Ok(Shape::operation(ffi::op_retrim(&self.inner, &Shape::vector(inputs.iter().copied().skip(1)), tolerance).map_err(|e| kernel("surface retrim", e))?, &inputs))
	}

	/// Extend a natural rectangular Bezier/BSpline patch, rebuilding its boundary.
	/// Continuity is parametric C1, C2, or C3; trimmed and periodic patches are rejected.
	pub fn extend_surface(&self, boundary: SurfaceBoundary, length: f64, continuity: u32, tolerance: f64) -> Result<OperationResult<Shape>, Error> {
		positive(length, "surface extension length")?;
		positive(tolerance, "surface extension tolerance")?;
		if !(1..=3).contains(&continuity) {
			return Err(Error::Validation("surface extension continuity must be 1, 2, or 3".into()));
		}
		let face = Shape::from(self);
		Ok(Shape::operation(ffi::op_extend_surface(&self.inner, boundary as u32, length, continuity, tolerance).map_err(|e| kernel("surface extension", e))?, &[&face]))
	}
}
impl Shape {
	/// Split a member face with curves already lying on its trimmed domain.
	/// This creates topology without adding material; projection is a separate operation.
	pub fn imprint(&self, face: &Face, curves: &[&Shape], tolerance: f64, control: &OperationControl) -> Result<OperationResult<Shape>, Error> {
		positive(tolerance, "imprint tolerance")?;
		if curves.is_empty() || curves.iter().any(|s| !matches!(s.kind(), ShapeKind::Edge | ShapeKind::Wire)) {
			return Err(Error::Validation("imprint requires edges or wires".into()));
		}
		let inputs: Vec<&Shape> = std::iter::once(self).chain(curves.iter().copied()).collect();
		control.check("surface imprint")?;
		let data = ffi::op_imprint(&self.inner, &face.inner, &Shape::vector(curves.iter().copied()), tolerance, control);
		control.check("surface imprint")?;
		Ok(Shape::operation(data.map_err(|e| kernel("surface imprint", e))?, &inputs))
	}
}
