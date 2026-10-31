//! Validation reports and explicit native operation outcomes.
use super::{
	ffi,
	shape::{Shape, ShapeKind},
};
use crate::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
	pub message: String,
	pub subshapes: Vec<usize>,
}

#[derive(Debug)]
pub struct OperationResult<T> {
	pub history: History,
	pub value: T,
	pub warnings: Vec<Diagnostic>,
	pub diagnostics: Vec<Diagnostic>,
}

/// OCCT BRepCheck status code and the indexed subshape/context that produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationIssue {
	pub subshape: usize,
	pub context: Option<usize>,
	pub kind: ShapeKind,
	pub status: u32,
	pub orientation_problem: bool,
}
#[derive(Clone, Debug)]
pub struct ValidationReport {
	pub valid: bool,
	pub kind: ShapeKind,
	pub closed: bool,
	pub empty: bool,
	pub expected: Option<ShapeKind>,
	pub category_matches: bool,
	pub issues: Vec<ValidationIssue>,
}
impl Shape {
	/// Validate geometry independently from the caller's expected output category.
	pub fn validate(&self, expected: Option<ShapeKind>) -> Result<ValidationReport, Error> {
		let data = ffi::op_validate(&self.inner).map_err(|e| kernel("validate", e))?;
		let kind = self.kind();
		Ok(ValidationReport {
			valid: data.valid,
			kind,
			closed: data.closed,
			empty: self.is_empty()?,
			expected,
			category_matches: expected.is_none_or(|v| v == kind),
			issues: data
				.issues
				.into_iter()
				.map(|v| ValidationIssue {
					subshape: v.subshape as usize,
					context: (v.context > 0).then_some(v.context as usize),
					kind: ShapeKind::from_native(v.kind),
					status: v.status,
					orientation_problem: matches!(v.status, 27 | 31 | 32),
				})
				.collect(),
		})
	}
}

pub(crate) fn kernel(operation: &str, error: cxx::Exception) -> Error {
	if error.what() == "cancelled" {
		return Error::Cancelled { operation: operation.into() };
	}
	if let Some(kind) = error.what().strip_prefix("unsupported: ") {
		return Error::Unsupported { operation: operation.into(), kind: kind.into() };
	}
	Error::Operation { operation: operation.into(), diagnostics: vec![error.what().into()], subshapes: Vec::new() }
}
pub(crate) fn finite(values: impl IntoIterator<Item = f64>, operation: &str) -> Result<(), Error> {
	if values.into_iter().all(f64::is_finite) {
		Ok(())
	} else {
		Err(Error::Validation(format!("{operation}: non-finite input")))
	}
}
pub(crate) fn positive(value: f64, operation: &str) -> Result<(), Error> {
	finite([value], operation)?;
	if value > 0.0 {
		Ok(())
	} else {
		Err(Error::Validation(format!("{operation}: expected a positive value")))
	}
}
pub(crate) fn axis(value: crate::DVec3, operation: &str) -> Result<(), Error> {
	positive(value.length(), operation)
}
pub(crate) fn supported(operation: &str, kind: ShapeKind) -> Error {
	Error::Unsupported { operation: operation.into(), kind: format!("{kind:?}") }
}

#[derive(Clone, Copy, Debug)]
pub struct RepairOptions {
	pub precision: f64,
	pub max_tolerance: f64,
	pub max_sampled_deviation: f64,
	pub fix_wires: bool,
	pub fix_faces: bool,
	pub fix_shells: bool,
	pub fix_solids: bool,
	pub allow_closing: bool,
}
impl Default for RepairOptions {
	fn default() -> Self {
		Self { precision: 1e-7, max_tolerance: 1e-5, max_sampled_deviation: 1e-5, fix_wires: true, fix_faces: true, fix_shells: true, fix_solids: false, allow_closing: false }
	}
}
#[derive(Debug)]
pub struct RepairReport {
	pub shape: Shape,
	pub before: ValidationReport,
	pub after: ValidationReport,
	pub changed: bool,
	pub maximum_tolerance_before: f64,
	pub maximum_tolerance_after: f64,
	pub boundary_edges_before: usize,
	pub boundary_edges_after: usize,
	pub changed_subshapes: Vec<usize>,
	pub maximum_sampled_deviation: f64,
	pub sample_count: usize,
}
impl Shape {
	/// Repair an independent copy under explicit permissions and tolerance limits.
	pub fn repair(&self, options: RepairOptions) -> Result<RepairReport, Error> {
		positive(options.precision, "repair precision")?;
		positive(options.max_sampled_deviation, "repair maximum sampled deviation")?;
		positive(options.max_tolerance, "repair maximum tolerance")?;
		if options.max_tolerance < options.precision {
			return Err(Error::Validation("repair: maximum tolerance below precision".into()));
		}
		let before = self.validate(None)?;
		let data = ffi::op_repair(&self.inner, options.precision, options.max_tolerance, options.max_sampled_deviation, &[options.fix_wires, options.fix_faces, options.fix_shells, options.fix_solids, options.allow_closing]).map_err(|e| kernel("repair", e))?;
		let shape = Shape::new(data.shape);
		let after = shape.validate(None)?;
		Ok(RepairReport {
			shape,
			before,
			after,
			changed: data.changed,
			maximum_tolerance_before: data.tolerance_before,
			maximum_tolerance_after: data.tolerance_after,
			boundary_edges_before: data.boundary_before as usize,
			boundary_edges_after: data.boundary_after as usize,
			changed_subshapes: data.changed_subshapes.into_iter().map(|i| i as usize).collect(),
			maximum_sampled_deviation: data.maximum_deviation,
			sample_count: data.sample_count,
		})
	}
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RelationKind {
	Modified,
	Generated,
	Removed,
	Unchanged,
	Unknown,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TopologyRef {
	pub operand: usize,
	pub index: usize,
	pub kind: ShapeKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyRelation {
	pub source: TopologyRef,
	pub target: Option<TopologyRef>,
	pub relation: RelationKind,
}
#[derive(Clone, Debug, Default)]
pub struct History {
	pub relations: Vec<TopologyRelation>,
	pub complete: bool,
}
impl Shape {
	pub(crate) fn operation(data: ffi::OperationData, inputs: &[&Shape]) -> OperationResult<Shape> {
		let mut value = Shape::new(data.shape);
		let mut relations = Vec::new();
		for row in data.history {
			if let Some(source) = inputs.get(row.operand as usize) {
				if row.target > 0 && row.target_kind == ShapeKind::Face as u32 {
					if let Some(&color) = source.colormap.get(&row.source_id) {
						value.colormap.insert(row.target_id, color);
					}
				}
			}
			relations.push(TopologyRelation {
				source: TopologyRef { operand: row.operand as usize, index: row.source as usize, kind: ShapeKind::from_native(row.source_kind) },
				target: (row.target > 0).then(|| TopologyRef { operand: 0, index: row.target as usize, kind: ShapeKind::from_native(row.target_kind) }),
				relation: match row.relation {
					0 => RelationKind::Modified,
					1 => RelationKind::Generated,
					2 => RelationKind::Removed,
					3 => RelationKind::Unchanged,
					_ => RelationKind::Unknown,
				},
			});
		}
		OperationResult {
			value,
			history: History { relations, complete: data.complete },
			warnings: data.warnings.into_iter().map(|message| Diagnostic { message, subshapes: vec![] }).collect(),
			diagnostics: data.diagnostics.into_iter().map(|message| Diagnostic { message, subshapes: vec![] }).collect(),
		}
	}
}

impl History {
	/// Compose descendants through the specified operand of the next operation.
	/// Other next-operation operands retain identity after the first history's operands.
	pub fn compose(&self, next: &History, intermediate_operand: usize) -> History {
		let mut out = History { relations: Vec::new(), complete: self.complete && next.complete };
		let offset = self.relations.iter().map(|r| r.source.operand).max().map_or(0, |n| n + 1);
		for first in &self.relations {
			let Some(target) = first.target else {
				out.relations.push(first.clone());
				continue;
			};
			let mut found = false;
			for second in next.relations.iter().filter(|r| r.source.operand == intermediate_operand && r.source.index == target.index && r.source.kind == target.kind) {
				found = true;
				let relation = match (first.relation, second.relation) {
					(_, RelationKind::Removed) => RelationKind::Removed,
					(_, RelationKind::Unknown) | (RelationKind::Unknown, _) => RelationKind::Unknown,
					(RelationKind::Generated, _) | (_, RelationKind::Generated) => RelationKind::Generated,
					(RelationKind::Unchanged, RelationKind::Unchanged) => RelationKind::Unchanged,
					_ => RelationKind::Modified,
				};
				out.relations.push(TopologyRelation { source: first.source, target: second.target, relation });
			}
			if !found {
				out.complete = false;
				out.relations.push(TopologyRelation { source: first.source, target: None, relation: RelationKind::Unknown });
			}
		}
		for relation in &next.relations {
			if relation.source.operand != intermediate_operand {
				let mut relation = relation.clone();
				relation.source.operand = offset + relation.source.operand - usize::from(relation.source.operand > intermediate_operand);
				out.relations.push(relation);
			}
		}
		let mut seen = std::collections::HashSet::new();
		out.relations.retain(|r| seen.insert((r.source, r.target, r.relation)));
		out
	}
}
