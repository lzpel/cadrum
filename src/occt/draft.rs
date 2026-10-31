//! Draft, tapered extrusion, and native directional/point projection.
use super::{
	ffi,
	report::{axis, finite, kernel, positive, supported},
};
use crate::{DVec3, Edge, Error, Face, GeometrySurfaceKind, OperationControl, OperationResult, Plane, Shape, ShapeKind};
#[derive(Clone, Copy, Debug)]
pub struct DraftOptions {
	pub neutral_plane: Plane,
	pub pull_direction: DVec3,
	pub angle: f64,
	pub flag: bool,
}
#[derive(Clone, Debug)]
pub struct DraftFaceStatus {
	pub selection: usize,
	pub status: u32,
}
#[derive(Debug)]
pub struct DraftResult {
	pub result: OperationResult<Shape>,
	pub face_status: Vec<DraftFaceStatus>,
}
#[derive(Clone, Copy, Debug)]
pub enum Projection {
	Directional(DVec3),
	FromPoint(DVec3),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultipleResults {
	KeepAll,
	Reject,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoHitPolicy {
	Empty,
	Error,
}
#[derive(Clone, Copy, Debug)]
pub struct ProjectionOptions {
	pub tolerance: f64,
	pub untrimmed: bool,
	pub multiple: MultipleResults,
	pub no_hit: NoHitPolicy,
}
impl Default for ProjectionOptions {
	fn default() -> Self {
		Self { tolerance: 1e-7, untrimmed: false, multiple: MultipleResults::KeepAll, no_hit: NoHitPolicy::Empty }
	}
}
impl Shape {
	pub fn draft(&self, faces: &[&Face], options: DraftOptions, control: &OperationControl) -> Result<DraftResult, Error> {
		finite(options.neutral_plane.origin.to_array().into_iter().chain([options.angle]), "draft")?;
		axis(options.neutral_plane.normal, "draft neutral normal")?;
		axis(options.pull_direction, "draft pull direction")?;
		if faces.is_empty() || options.angle == 0.0 || options.angle.abs() >= std::f64::consts::FRAC_PI_2 {
			return Err(Error::Validation("draft requires selected faces and a nonzero angle with magnitude below pi/2".into()));
		}
		let selected: Vec<_> = faces.iter().map(|f| Shape::from(*f)).collect();
		let p = options.neutral_plane.origin;
		let n = options.neutral_plane.normal;
		let d = options.pull_direction;
		let values = [p.x, p.y, p.z, n.x, n.y, n.z, d.x, d.y, d.z, options.angle];
		control.check("draft")?;
		let data = ffi::op_draft(&self.inner, &Self::vector(&selected), &values, options.flag, control);
		control.check("draft")?;
		let data = data.map_err(|e| kernel("draft", e))?;
		Ok(DraftResult { result: Self::operation(data.operation, &[self]), face_status: data.statuses.into_iter().enumerate().map(|(selection, status)| DraftFaceStatus { selection, status }).collect() })
	}
	pub fn project_onto(&self, target: &Shape, projection: Projection, options: ProjectionOptions) -> Result<OperationResult<Self>, Error> {
		positive(options.tolerance, "projection tolerance")?;
		if !matches!(self.kind(), ShapeKind::Edge | ShapeKind::Wire) {
			return Err(supported("projection source", self.kind()));
		}
		if !matches!(target.kind(), ShapeKind::Face | ShapeKind::Shell | ShapeKind::Solid | ShapeKind::Compound) {
			return Err(supported("projection target", target.kind()));
		}
		if options.untrimmed && target.kind() != ShapeKind::Face {
			return Err(Error::Validation("untrimmed projection requires one face".into()));
		}
		let (point, vector) = match projection {
			Projection::Directional(v) => {
				axis(v, "projection direction")?;
				(false, v)
			}
			Projection::FromPoint(p) => {
				finite(p.to_array(), "projection point")?;
				(true, p)
			}
		};
		Ok(Self::operation(ffi::op_project(&self.inner, &target.inner, &vector.to_array(), point, options.tolerance, options.untrimmed, options.multiple == MultipleResults::Reject, options.no_hit == NoHitPolicy::Error).map_err(|e| kernel("projection", e))?, &[self, target]))
	}
}
impl Face {
	pub fn tapered_extrude(&self, direction: DVec3, angle: f64, control: &OperationControl) -> Result<DraftResult, Error> {
		if self.surface_info()?.kind != GeometrySurfaceKind::Plane {
			return Err(Error::Unsupported { operation: "tapered extrusion".into(), kind: "non-planar face".into() });
		}
		let frame = self.surface_info()?.frame.ok_or_else(|| Error::Validation("tapered extrusion: missing planar frame".into()))?;
		let source = Shape::from(self);
		let prism = source.extrude(direction)?;
		let mut faces = Vec::new();
		for relation in &prism.history.relations {
			if relation.source.kind == ShapeKind::Edge && relation.relation == crate::RelationKind::Generated {
				if let Some(target) = relation.target {
					if target.kind == ShapeKind::Face {
						faces.push(prism.value.topology_at(target.index)?.as_face()?);
					}
				}
			}
		}
		let refs: Vec<_> = faces.iter().collect();
		let mut drafted = prism.value.draft(&refs, DraftOptions { neutral_plane: Plane { origin: frame.origin, normal: frame.axis_z }, pull_direction: direction, angle, flag: true }, control)?;
		drafted.result.history = prism.history.compose(&drafted.result.history, 0);
		Ok(drafted)
	}
}
impl Edge {
	pub fn project_onto(&self, target: &Shape, projection: Projection, options: ProjectionOptions) -> Result<OperationResult<Shape>, Error> {
		Shape::from(self).project_onto(target, projection, options)
	}
}
