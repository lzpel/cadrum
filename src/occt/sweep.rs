//! Configurable native sweeps with explicit shell/solid output and guide policies.
use super::{
	ffi,
	report::{finite, kernel, positive},
};
use crate::{DVec3, Error, Frame, OperationControl, OperationResult, Shape, ShapeKind, Vertex, Wire};
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum SweepTransition {
	Transformed = 0,
	RightCorner = 1,
	RoundCorner = 2,
}
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum AuxiliaryContact {
	None = 0,
	Contact = 1,
	Boundary = 2,
}
#[derive(Clone, Copy, Debug)]
pub enum SweepOrientation<'a> {
	CorrectedFrenet,
	Frenet,
	Fixed(Frame),
	Binormal(DVec3),
	Discrete,
	Support(&'a Shape),
	Auxiliary { spine: &'a Wire, curvilinear: bool, contact: AuxiliaryContact },
}
#[derive(Clone, Copy, Debug)]
pub struct SweepSection<'a> {
	pub profile: &'a Shape,
	pub location: Option<&'a Vertex>,
	pub contact: bool,
	pub correction: bool,
}
#[derive(Clone, Debug)]
pub struct SweepOptions<'a> {
	pub transition: SweepTransition,
	pub orientation: SweepOrientation<'a>,
	pub tolerance: f64,
	pub boundary_tolerance: f64,
	pub angular_tolerance: f64,
	pub force_c1: bool,
	pub max_degree: u32,
	pub max_segments: u32,
	pub solid: bool,
	pub scaling: Vec<[f64; 2]>,
	pub simulation_sections: usize,
}
impl Default for SweepOptions<'_> {
	fn default() -> Self {
		Self {
			transition: SweepTransition::Transformed,
			orientation: SweepOrientation::CorrectedFrenet,
			tolerance: 1e-4,
			boundary_tolerance: 1e-4,
			angular_tolerance: 1e-2,
			force_c1: false,
			max_degree: 11,
			max_segments: 100,
			solid: true,
			scaling: Vec::new(),
			simulation_sections: 0,
		}
	}
}
#[derive(Debug)]
pub struct SweepResult {
	pub result: OperationResult<Shape>,
	pub sections: Vec<Wire>,
	pub status: u32,
	pub approximation_error: f64,
}
impl Wire {
	pub fn sweep(&self, sections: &[SweepSection<'_>], options: SweepOptions<'_>, control: &OperationControl) -> Result<SweepResult, Error> {
		positive(options.tolerance, "sweep tolerance")?;
		positive(options.boundary_tolerance, "sweep boundary tolerance")?;
		positive(options.angular_tolerance, "sweep angular tolerance")?;
		if sections.is_empty() || !(1..=25).contains(&options.max_degree) || options.max_segments == 0 || options.max_segments > i32::MAX as u32 || options.simulation_sections > 1_000_000 || options.simulation_sections == 1 {
			return Err(Error::Validation("sweep: invalid section count, approximation degree, segment count, or simulation count".into()));
		}
		for (i, s) in sections.iter().enumerate() {
			if !matches!(s.profile.kind(), ShapeKind::Wire | ShapeKind::Vertex) || (s.profile.kind() == ShapeKind::Vertex && i != 0 && i + 1 != sections.len()) {
				return Err(Error::Validation("sweep sections must be wires or endpoint vertices".into()));
			}
		}
		let mut inputs: Vec<&Shape> = std::iter::once(&**self).chain(sections.iter().map(|s| s.profile)).collect();
		let mut locations = Vec::new();
		for section in sections {
			if let Some(location) = section.location {
				locations.push(inputs.len() as i32);
				inputs.push(location);
			} else {
				locations.push(-1);
			}
		}
		let mut config = ffi::SweepConfig {
			mode: 0,
			frame: vec![],
			transition: options.transition as u32,
			tolerance: options.tolerance,
			boundary_tolerance: options.boundary_tolerance,
			angular_tolerance: options.angular_tolerance,
			force_c1: options.force_c1,
			max_degree: options.max_degree,
			max_segments: options.max_segments,
			solid: options.solid,
			contact: sections.iter().map(|s| s.contact).collect(),
			correction: sections.iter().map(|s| s.correction).collect(),
			locations,
			law: options.scaling.iter().flatten().copied().collect(),
			guide: -1,
			curvilinear: true,
			auxiliary_contact: 0,
			simulation: options.simulation_sections as u32,
		};
		match options.orientation {
			SweepOrientation::CorrectedFrenet => {}
			SweepOrientation::Frenet => config.mode = 1,
			SweepOrientation::Fixed(frame) => {
				config.mode = 2;
				config.frame = frame.values()?.to_vec();
			}
			SweepOrientation::Binormal(v) => {
				super::report::axis(v, "sweep binormal")?;
				config.mode = 3;
				config.frame = v.to_array().to_vec();
			}
			SweepOrientation::Discrete => config.mode = 4,
			SweepOrientation::Support(shape) => {
				config.mode = 5;
				config.guide = inputs.len() as i32;
				inputs.push(shape);
			}
			SweepOrientation::Auxiliary { spine, curvilinear, contact } => {
				config.mode = 6;
				config.guide = inputs.len() as i32;
				config.curvilinear = curvilinear;
				config.auxiliary_contact = contact as u32;
				inputs.push(spine);
			}
		}
		if !options.scaling.is_empty() {
			finite(config.law.iter().copied(), "sweep scaling law")?;
			if sections.len() != 1 || options.scaling.len() < 2 || config.mode == 6 || options.scaling[0][0] != 0.0 || options.scaling.last().unwrap()[0] != 1.0 || options.scaling.windows(2).any(|p| p[0][0] >= p[1][0]) || options.scaling.iter().any(|p| p[1] <= 0.0) {
				return Err(Error::Validation("sweep scaling needs one section, positive values, and increasing parameters from 0 to 1; auxiliary mode is incompatible".into()));
			}
			if self.is_closed()? && options.scaling[0][1] != options.scaling.last().unwrap()[1] {
				return Err(Error::Validation("closed-spine scaling law must have matching endpoint values".into()));
			}
		}
		control.check("sweep")?;
		let data = ffi::op_sweep(&Shape::vector(inputs.iter().copied()), sections.len(), &config, control);
		control.check("sweep")?;
		let data = data.map_err(|e| kernel("sweep", e))?;
		let simulated = (0..data.sections.len()).map(|i| Shape::new(ffi::native_take(&data.sections, i)).try_into()).collect::<Result<Vec<_>, Error>>()?;
		Ok(SweepResult { result: Shape::operation(data.operation, &inputs), sections: simulated, status: data.status, approximation_error: data.error })
	}
}
