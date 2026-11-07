//! Authored native curves and surfaces; pole construction never interpolates samples.
use super::{
	ffi,
	report::{axis, finite, kernel, positive},
};
use crate::{DVec3, Edge, Error, Face};

#[derive(Clone, Copy, Debug)]
pub struct Frame {
	pub origin: DVec3,
	pub axis_z: DVec3,
	pub axis_x: DVec3,
}
impl Default for Frame {
	fn default() -> Self {
		Self { origin: DVec3::ZERO, axis_z: DVec3::Z, axis_x: DVec3::X }
	}
}
impl Frame {
	pub(crate) fn values(&self) -> Result<[f64; 9], Error> {
		finite(self.origin.to_array(), "frame origin")?;
		axis(self.axis_z, "frame Z")?;
		axis(self.axis_x, "frame X")?;
		if self.axis_z.normalize().dot(self.axis_x.normalize()).abs() > 1e-10 {
			return Err(Error::Validation("frame: X and Z must be perpendicular".into()));
		}
		Ok([self.origin.x, self.origin.y, self.origin.z, self.axis_z.x, self.axis_z.y, self.axis_z.z, self.axis_x.x, self.axis_x.y, self.axis_x.z])
	}
}
#[derive(Clone, Debug, PartialEq)]
pub struct SplineAxis {
	pub degree: u32,
	pub knots: Vec<f64>,
	pub multiplicities: Vec<i32>,
	pub periodic: bool,
}
impl SplineAxis {
	pub(crate) fn validate(&self, count: usize) -> Result<(), Error> {
		finite(self.knots.iter().copied(), "spline knots")?;
		if !(1..=25).contains(&self.degree) || self.knots.len() < 2 || self.knots.len() != self.multiplicities.len() || self.knots.windows(2).any(|p| p[0] >= p[1]) {
			return Err(Error::Validation("spline: invalid degree or knots".into()));
		}
		for (i, &m) in self.multiplicities.iter().enumerate() {
			let limit = self.degree as i32 + i32::from(!self.periodic && (i == 0 || i + 1 == self.knots.len()));
			if m <= 0 || m > limit {
				return Err(Error::Validation("spline: invalid knot multiplicity".into()));
			}
		}
		let sum: i64 = self.multiplicities.iter().map(|&m| i64::from(m)).sum();
		let expected = if self.periodic {
			if self.multiplicities.first() != self.multiplicities.last() {
				return Err(Error::Validation("periodic spline: endpoint multiplicities differ".into()));
			}
			sum - i64::from(self.multiplicities[0])
		} else {
			sum - i64::from(self.degree) - 1
		};
		if count < 2 || expected != count as i64 {
			return Err(Error::Validation("spline: pole count does not match knot multiplicities".into()));
		}
		Ok(())
	}
}
#[derive(Clone, Debug, PartialEq)]
pub enum CurveControlData {
	Bezier { poles: Vec<DVec3>, weights: Vec<f64> },
	BSpline { poles: Vec<DVec3>, weights: Vec<f64>, axis: SplineAxis },
}
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceControlData {
	Bezier { u_count: usize, v_count: usize, poles: Vec<DVec3>, weights: Vec<f64> },
	BSpline { u_count: usize, v_count: usize, poles: Vec<DVec3>, weights: Vec<f64>, u: SplineAxis, v: SplineAxis },
}
fn poles_data(poles: &[DVec3], weights: &[f64], u: usize, v: usize) -> Result<ffi::SplineData, Error> {
	if u.checked_mul(v) != Some(poles.len()) || u > i32::MAX as usize || v > i32::MAX as usize || poles.len() < 2 {
		return Err(Error::Validation("spline: invalid control net dimensions".into()));
	}
	finite(poles.iter().flat_map(|p| p.to_array()), "spline poles")?;
	if !weights.is_empty() && weights.len() != poles.len() {
		return Err(Error::Validation("spline: weights and poles have different lengths".into()));
	}
	for &w in weights {
		positive(w, "spline weight")?;
		if w <= 1e-300 {
			return Err(Error::Validation("spline: weight is below kernel resolution".into()));
		}
	}
	Ok(ffi::SplineData {
		poles: poles.iter().flat_map(|p| p.to_array()).collect(),
		weights: if weights.is_empty() { vec![1.0; poles.len()] } else { weights.to_vec() },
		u_count: u as u32,
		v_count: v as u32,
		..Default::default()
	})
}
fn set_axis(data: &mut ffi::SplineData, u: &SplineAxis, v: Option<&SplineAxis>) {
	data.u_degree = u.degree;
	data.u_knots = u.knots.clone();
	data.u_mults = u.multiplicities.clone();
	data.u_periodic = u.periodic;
	if let Some(v) = v {
		data.v_degree = v.degree;
		data.v_knots = v.knots.clone();
		data.v_mults = v.multiplicities.clone();
		data.v_periodic = v.periodic;
	}
}
pub(crate) fn range(values: &[f64]) -> Result<(), Error> {
	finite(values.iter().copied(), "parameter range")?;
	if values.chunks_exact(2).any(|p| p[0] >= p[1]) {
		return Err(Error::Validation("parameter range must be increasing".into()));
	}
	Ok(())
}
impl CurveControlData {
	pub fn edge(&self, trim: Option<[f64; 2]>) -> Result<Edge, Error> {
		let (poles, weights) = match self {
			Self::Bezier { poles, weights } | Self::BSpline { poles, weights, .. } => (poles, weights),
		};
		let mut data = poles_data(poles, weights, poles.len(), 1)?;
		match self {
			Self::Bezier { .. } => {
				if poles.len() > 26 {
					return Err(Error::Validation("Bezier degree exceeds 25".into()));
				}
				data.bezier = true;
			}
			Self::BSpline { axis, .. } => {
				axis.validate(poles.len())?;
				set_axis(&mut data, axis, None);
			}
		}
		let trim = trim.map(Vec::from).unwrap_or_default();
		range(&trim)?;
		Ok(Edge { inner: ffi::op_authored_curve(&data, &trim).map_err(|e| kernel("authored curve", e))? })
	}
}
impl SurfaceControlData {
	pub fn face(&self, trim: Option<[f64; 4]>) -> Result<Face, Error> {
		let (u_count, v_count, poles, weights) = match self {
			Self::Bezier { u_count, v_count, poles, weights } | Self::BSpline { u_count, v_count, poles, weights, .. } => (*u_count, *v_count, poles, weights),
		};
		if u_count < 2 || v_count < 2 {
			return Err(Error::Validation("surface: each control net direction needs at least two poles".into()));
		}
		let mut data = poles_data(poles, weights, u_count, v_count)?;
		match self {
			Self::Bezier { .. } => {
				if u_count > 26 || v_count > 26 {
					return Err(Error::Validation("Bezier surface degree exceeds 25".into()));
				}
				data.bezier = true;
			}
			Self::BSpline { u, v, .. } => {
				u.validate(u_count)?;
				v.validate(v_count)?;
				set_axis(&mut data, u, Some(v));
			}
		}
		let trim = trim.map(Vec::from).unwrap_or_default();
		range(&trim)?;
		Ok(Face::new(ffi::op_authored_surface(&data, &trim).map_err(|e| kernel("authored surface", e))?))
	}
}
fn axis_data(data: &ffi::SplineData, u: bool) -> SplineAxis {
	if u {
		SplineAxis { degree: data.u_degree, knots: data.u_knots.clone(), multiplicities: data.u_mults.clone(), periodic: data.u_periodic }
	} else {
		SplineAxis { degree: data.v_degree, knots: data.v_knots.clone(), multiplicities: data.v_mults.clone(), periodic: data.v_periodic }
	}
}
#[derive(Clone, Debug)]
pub struct CurveEvaluation {
	pub point: DVec3,
	pub first: DVec3,
	pub second: DVec3,
}
#[derive(Clone, Debug)]
pub struct SurfaceEvaluation {
	pub point: DVec3,
	pub du: DVec3,
	pub dv: DVec3,
	pub duu: DVec3,
	pub duv: DVec3,
	pub dvv: DVec3,
	pub normal: DVec3,
}
impl Edge {
	pub fn ellipse(frame: Frame, major: f64, minor: f64, trim: Option<[f64; 2]>) -> Result<Self, Error> {
		positive(major, "ellipse major radius")?;
		positive(minor, "ellipse minor radius")?;
		if minor > major {
			return Err(Error::Validation("ellipse: minor radius exceeds major".into()));
		}
		let trim = trim.map(Vec::from).unwrap_or_default();
		range(&trim)?;
		Ok(Self { inner: ffi::op_ellipse(&frame.values()?, major, minor, &trim).map_err(|e| kernel("ellipse", e))? })
	}
	pub fn control_data(&self) -> Result<CurveControlData, Error> {
		let data = ffi::op_curve_control(&self.inner).map_err(|e| kernel("curve control data", e))?;
		let poles = data.poles.chunks_exact(3).map(DVec3::from_slice).collect();
		if data.bezier {
			Ok(CurveControlData::Bezier { poles, weights: data.weights })
		} else {
			let axis = axis_data(&data, true);
			Ok(CurveControlData::BSpline { poles, weights: data.weights, axis })
		}
	}
	pub fn parameter_range(&self) -> Result<[f64; 2], Error> {
		let v = ffi::op_curve_range(&self.inner).map_err(|e| kernel("curve range", e))?;
		Ok([v[0], v[1]])
	}
	pub fn evaluate(&self, parameter: f64) -> Result<CurveEvaluation, Error> {
		finite([parameter], "curve parameter")?;
		let [a, b] = self.parameter_range()?;
		if parameter < a || parameter > b {
			return Err(Error::Validation("curve parameter outside trimmed range".into()));
		}
		let v = ffi::op_curve_evaluate(&self.inner, parameter).map_err(|e| kernel("curve evaluation", e))?;
		Ok(CurveEvaluation { point: DVec3::from_slice(&v[0..3]), first: DVec3::from_slice(&v[3..6]), second: DVec3::from_slice(&v[6..9]) })
	}
	pub fn trimmed(&self, trim: [f64; 2]) -> Result<Self, Error> {
		range(&trim)?;
		let [a, b] = self.parameter_range()?;
		if trim[0] < a || trim[1] > b {
			return Err(Error::Validation("curve trim outside original domain".into()));
		}
		Ok(Self { inner: ffi::op_trim_edge(&self.inner, &trim).map_err(|e| kernel("curve trim", e))? })
	}
}
impl Face {
	pub fn control_data(&self) -> Result<SurfaceControlData, Error> {
		let data = ffi::op_surface_control(&self.inner).map_err(|e| kernel("surface control data", e))?;
		let poles = data.poles.chunks_exact(3).map(DVec3::from_slice).collect();
		if data.bezier {
			Ok(SurfaceControlData::Bezier { u_count: data.u_count as usize, v_count: data.v_count as usize, poles, weights: data.weights })
		} else {
			let u = axis_data(&data, true);
			let v = axis_data(&data, false);
			Ok(SurfaceControlData::BSpline { u_count: data.u_count as usize, v_count: data.v_count as usize, poles, weights: data.weights, u, v })
		}
	}
	pub fn parameter_range(&self) -> Result<[f64; 4], Error> {
		let v = ffi::op_surface_range(&self.inner).map_err(|e| kernel("surface range", e))?;
		Ok([v[0], v[1], v[2], v[3]])
	}
	pub fn evaluate_uv(&self, u: f64, v: f64) -> Result<SurfaceEvaluation, Error> {
		finite([u, v], "surface parameter")?;
		let [u0, u1, v0, v1] = self.parameter_range()?;
		if u < u0 || u > u1 || v < v0 || v > v1 {
			return Err(Error::Validation("surface parameter outside range".into()));
		}
		let r = ffi::op_surface_evaluate(&self.inner, u, v).map_err(|e| kernel("surface evaluation", e))?;
		Ok(SurfaceEvaluation {
			point: DVec3::from_slice(&r[0..3]),
			du: DVec3::from_slice(&r[3..6]),
			dv: DVec3::from_slice(&r[6..9]),
			duu: DVec3::from_slice(&r[9..12]),
			duv: DVec3::from_slice(&r[12..15]),
			dvv: DVec3::from_slice(&r[15..18]),
			normal: DVec3::from_slice(&r[18..21]),
		})
	}
	/// Create a rectangular patch of the underlying surface, without reusing existing trim wires.
	pub fn surface_patch(&self, trim: [f64; 4]) -> Result<Self, Error> {
		range(&trim)?;
		Ok(Self::new(ffi::op_trim_surface(&self.inner, &trim).map_err(|e| kernel("surface patch", e))?))
	}
}
