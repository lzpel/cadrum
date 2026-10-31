//! Exact native classification, measurements, closest points, and differential geometry.
use super::{
	ffi,
	report::{finite, kernel, positive},
};
use crate::{DMat3, DVec2, DVec3, Edge, Error, Face, Frame, Shape, ShapeKind, Wire};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveKind {
	Line,
	Circle,
	Ellipse,
	Hyperbola,
	Parabola,
	Bezier,
	BSpline,
	Offset,
	Other,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometrySurfaceKind {
	Plane,
	Cylinder,
	Cone,
	Sphere,
	Torus,
	Bezier,
	BSpline,
	Revolution,
	Extrusion,
	Offset,
	Other,
}
#[derive(Clone, Debug)]
pub struct CurveInfo {
	pub kind: CurveKind,
	pub range: [f64; 2],
	pub period: Option<f64>,
	pub frame: Option<Frame>,
	pub parameters: Vec<f64>,
}
#[derive(Clone, Debug)]
pub struct SurfaceInfo {
	pub kind: GeometrySurfaceKind,
	pub range: [f64; 4],
	pub u_period: Option<f64>,
	pub v_period: Option<f64>,
	pub frame: Option<Frame>,
	pub parameters: Vec<f64>,
}
fn frame(v: &[f64]) -> Option<Frame> {
	(v.len() == 9).then(|| Frame { origin: DVec3::from_slice(&v[..3]), axis_z: DVec3::from_slice(&v[3..6]), axis_x: DVec3::from_slice(&v[6..9]) })
}
impl Edge {
	pub fn curve_info(&self) -> Result<CurveInfo, Error> {
		let d = ffi::op_curve_info(&self.inner).map_err(|e| kernel("curve info", e))?;
		Ok(CurveInfo {
			kind: match d.kind {
				0 => CurveKind::Line,
				1 => CurveKind::Circle,
				2 => CurveKind::Ellipse,
				3 => CurveKind::Hyperbola,
				4 => CurveKind::Parabola,
				5 => CurveKind::Bezier,
				6 => CurveKind::BSpline,
				7 => CurveKind::Offset,
				_ => CurveKind::Other,
			},
			range: [d.range[0], d.range[1]],
			period: d.u_periodic.then_some(d.u_period),
			frame: frame(&d.frame),
			parameters: d.parameters,
		})
	}
	pub fn length(&self, tolerance: f64) -> Result<f64, Error> {
		positive(tolerance, "length tolerance")?;
		ffi::op_curve_length(&self.inner, tolerance).map_err(|e| kernel("edge length", e))
	}
	pub fn curvature(&self, parameter: f64) -> Result<f64, Error> {
		let e = self.evaluate(parameter)?;
		positive(e.first.length(), "curve speed")?;
		Ok(e.first.cross(e.second).length() / e.first.length().powi(3))
	}
	/// Sample equal arc-length intervals, including the trimmed endpoints.
	pub fn sample_by_length(&self, count: usize, tolerance: f64) -> Result<Vec<(f64, crate::CurveEvaluation)>, Error> {
		positive(tolerance, "arc-length tolerance")?;
		if !(2..=1_000_000).contains(&count) {
			return Err(Error::Validation("arc-length sample count must be 2..=1000000".into()));
		}
		ffi::op_curve_abscissae(&self.inner, count as u32, tolerance).map_err(|e| kernel("arc-length sampling", e))?.into_iter().map(|t| Ok((t, self.evaluate(t)?))).collect()
	}
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointLocation {
	Inside,
	Outside,
	OnBoundary,
	Unknown,
}
fn location(value: u32) -> PointLocation {
	match value {
		0 => PointLocation::Inside,
		1 => PointLocation::Outside,
		2 => PointLocation::OnBoundary,
		_ => PointLocation::Unknown,
	}
}
#[derive(Clone, Debug)]
pub struct SurfaceCurvature {
	pub minimum: f64,
	pub maximum: f64,
	pub mean: f64,
	pub gaussian: f64,
	pub minimum_direction: DVec3,
	pub maximum_direction: DVec3,
}
#[derive(Debug)]
pub struct BoundaryWire {
	pub wire: Wire,
	pub outer: bool,
}
impl Face {
	pub fn surface_info(&self) -> Result<SurfaceInfo, Error> {
		let d = ffi::op_surface_info(&self.inner).map_err(|e| kernel("surface info", e))?;
		Ok(SurfaceInfo {
			kind: match d.kind {
				0 => GeometrySurfaceKind::Plane,
				1 => GeometrySurfaceKind::Cylinder,
				2 => GeometrySurfaceKind::Cone,
				3 => GeometrySurfaceKind::Sphere,
				4 => GeometrySurfaceKind::Torus,
				5 => GeometrySurfaceKind::Bezier,
				6 => GeometrySurfaceKind::BSpline,
				7 => GeometrySurfaceKind::Revolution,
				8 => GeometrySurfaceKind::Extrusion,
				9 => GeometrySurfaceKind::Offset,
				_ => GeometrySurfaceKind::Other,
			},
			range: [d.range[0], d.range[1], d.range[2], d.range[3]],
			u_period: d.u_periodic.then_some(d.u_period),
			v_period: d.v_periodic.then_some(d.v_period),
			frame: frame(&d.frame),
			parameters: d.parameters,
		})
	}
	pub fn classify_uv(&self, uv: DVec2, tolerance: f64) -> Result<PointLocation, Error> {
		finite(uv.to_array(), "UV point")?;
		positive(tolerance, "UV classification tolerance")?;
		Ok(location(ffi::op_classify_uv(&self.inner, uv.x, uv.y, tolerance).map_err(|e| kernel("UV classification", e))?))
	}
	pub fn curvature_uv(&self, u: f64, v: f64) -> Result<SurfaceCurvature, Error> {
		let p = self.evaluate_uv(u, v)?;
		let e = p.du.length_squared();
		let f = p.du.dot(p.dv);
		let g = p.dv.length_squared();
		let determinant = e * g - f * f;
		positive(determinant, "surface metric determinant")?;
		let l = p.normal.dot(p.duu);
		let m = p.normal.dot(p.duv);
		let n = p.normal.dot(p.dvv);
		let mean = (l * g - 2.0 * m * f + n * e) / (2.0 * determinant);
		let gaussian = (l * n - m * m) / determinant;
		let delta = (mean * mean - gaussian).max(0.0).sqrt();
		let minimum = mean - delta;
		let maximum = mean + delta;
		let direction = |k: f64| {
			let mut a = -(m - k * f);
			let mut b = l - k * e;
			if a.abs() + b.abs() < 1e-12 {
				a = -(n - k * g);
				b = m - k * f;
			}
			if a.abs() + b.abs() < 1e-12 {
				p.du.normalize()
			} else {
				(p.du * a + p.dv * b).normalize()
			}
		};
		let maximum_direction = direction(maximum);
		let minimum_direction = if delta < 1e-12 { p.normal.cross(maximum_direction).normalize() } else { direction(minimum) };
		Ok(SurfaceCurvature { minimum, maximum, mean, gaussian, minimum_direction, maximum_direction })
	}
	pub fn boundary_wires(&self) -> Result<Vec<BoundaryWire>, Error> {
		let data = ffi::op_boundary_wires(&self.inner).map_err(|e| kernel("boundary wires", e))?;
		(0..data.len()).map(|i| Ok(BoundaryWire { wire: Shape::new(ffi::native_take(&data, i)).try_into()?, outer: i == 0 })).collect()
	}
	pub fn area_with_accuracy(&self, relative_accuracy: f64) -> Result<f64, Error> {
		Ok(Shape::from(self).mass_properties(MeasurementKind::Surface, relative_accuracy)?.mass)
	}
}
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum MeasurementKind {
	Length = 0,
	Surface = 1,
	Volume = 2,
}
#[derive(Clone, Debug)]
pub struct MassProperties {
	pub mass: f64,
	pub center: DVec3,
	pub inertia: DMat3,
	pub estimated_relative_error: f64,
}
#[derive(Debug)]
pub struct ClosestPair {
	pub first_point: DVec3,
	pub second_point: DVec3,
	pub first_support: Shape,
	pub second_support: Shape,
}
#[derive(Debug)]
pub struct DistanceReport {
	pub distance: f64,
	pub pairs: Vec<ClosestPair>,
	pub inner_solution: bool,
}
impl Shape {
	pub fn mass_properties(&self, kind: MeasurementKind, relative_accuracy: f64) -> Result<MassProperties, Error> {
		positive(relative_accuracy, "measurement accuracy")?;
		if relative_accuracy > 0.001 {
			return Err(Error::Validation("measurement accuracy must be <= 0.001 for adaptive integration".into()));
		}
		let v = ffi::op_properties(&self.inner, kind as u32, relative_accuracy).map_err(|e| kernel("mass properties", e))?;
		Ok(MassProperties {
			mass: v[0],
			center: DVec3::from_slice(&v[1..4]),
			inertia: DMat3::from_cols(DVec3::new(v[4], v[7], v[10]), DVec3::new(v[5], v[8], v[11]), DVec3::new(v[6], v[9], v[12])),
			estimated_relative_error: v[13],
		})
	}
	pub fn classify_point(&self, point: DVec3, tolerance: f64) -> Result<PointLocation, Error> {
		finite(point.to_array(), "point classification")?;
		positive(tolerance, "classification tolerance")?;
		Ok(location(ffi::op_classify_point(&self.inner, point.x, point.y, point.z, tolerance).map_err(|e| kernel("point classification", e))?))
	}
	pub fn distance(&self, other: &Shape, tolerance: f64) -> Result<DistanceReport, Error> {
		positive(tolerance, "distance tolerance")?;
		let data = ffi::op_distance(&self.inner, &other.inner, tolerance).map_err(|e| kernel("distance", e))?;
		let pairs = (0..data.first.len())
			.map(|i| ClosestPair {
				first_point: DVec3::from_slice(&data.points[6 * i..6 * i + 3]),
				second_point: DVec3::from_slice(&data.points[6 * i + 3..6 * i + 6]),
				first_support: Self::new(ffi::native_take(&data.first, i)),
				second_support: Self::new(ffi::native_take(&data.second, i)),
			})
			.collect();
		Ok(DistanceReport { distance: data.distance, pairs, inner_solution: data.inner_solution })
	}
	pub fn ancestors(&self, subshape: &Shape, kind: ShapeKind) -> Result<Vec<Shape>, Error> {
		let data = ffi::op_ancestors(&self.inner, &subshape.inner, kind as u32).map_err(|e| kernel("topology adjacency", e))?;
		Ok((0..data.len()).map(|i| Self::new(ffi::native_take(&data, i))).collect())
	}
}
