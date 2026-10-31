//! Configurable lofts through wires, point ends, and corresponding native profiles.
use super::{
	ffi,
	report::{kernel, positive},
};
use crate::{Error, OperationControl, OperationResult, Shape, ShapeKind};
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum LoftParametrization {
	ChordLength = 0,
	Centripetal = 1,
	Uniform = 2,
}
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum LoftContinuity {
	C0 = 0,
	C1 = 2,
	C2 = 4,
}
#[derive(Clone, Copy, Debug)]
pub struct LoftOptions {
	pub solid: bool,
	pub ruled: bool,
	pub compatibility: bool,
	pub tolerance: f64,
	pub smoothing: bool,
	pub parametrization: LoftParametrization,
	pub continuity: LoftContinuity,
	pub max_degree: u32,
	pub criterion_weights: [f64; 3],
}
impl Default for LoftOptions {
	fn default() -> Self {
		Self {
			solid: true,
			ruled: false,
			compatibility: true,
			tolerance: 1e-6,
			smoothing: false,
			parametrization: LoftParametrization::ChordLength,
			continuity: LoftContinuity::C2,
			max_degree: 8,
			criterion_weights: [1.0; 3],
		}
	}
}
impl Shape {
	pub fn loft(sections: &[&Shape], options: LoftOptions, control: &OperationControl) -> Result<OperationResult<Self>, Error> {
		positive(options.tolerance, "loft tolerance")?;
		for w in options.criterion_weights {
			positive(w, "loft criterion weight")?;
		}
		if sections.len() < 2 || !(1..=25).contains(&options.max_degree) {
			return Err(Error::Validation("loft requires at least two sections and degree 1..=25".into()));
		}
		for (i, section) in sections.iter().enumerate() {
			if !matches!(section.kind(), ShapeKind::Wire | ShapeKind::Vertex | ShapeKind::Face | ShapeKind::Compound) || (section.kind() == ShapeKind::Vertex && i != 0 && i + 1 != sections.len()) {
				return Err(Error::Validation("loft: sections must be wires, profiles, or endpoint vertices".into()));
			}
		}
		let config = ffi::LoftConfig {
			solid: options.solid,
			ruled: options.ruled,
			compatibility: options.compatibility,
			tolerance: options.tolerance,
			smoothing: options.smoothing,
			parametrization: options.parametrization as u32,
			continuity: options.continuity as u32,
			max_degree: options.max_degree,
			weights: options.criterion_weights.to_vec(),
		};
		control.check("loft")?;
		let data = ffi::op_loft(&Self::vector(sections.iter().copied()), &config, control);
		control.check("loft")?;
		Ok(Self::operation(data.map_err(|e| kernel("loft", e))?, sections))
	}
}
