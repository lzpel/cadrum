//! Explicit STEP units and optional product/assembly documents without OCAF dependencies.
use super::{ffi, report::kernel};
use crate::{Diagnostic, Error, OperationControl, OperationResult, Shape};
use std::io::{Read, Write};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LengthUnit {
	Inch = 1,
	Millimeter = 2,
	Foot = 4,
	Meter = 6,
	Kilometer = 7,
	Micrometer = 9,
	Centimeter = 10,
}
impl LengthUnit {
	pub fn millimeters(self) -> f64 {
		match self {
			Self::Inch => 25.4,
			Self::Millimeter => 1.0,
			Self::Foot => 304.8,
			Self::Meter => 1000.0,
			Self::Kilometer => 1_000_000.0,
			Self::Micrometer => 0.001,
			Self::Centimeter => 10.0,
		}
	}
}
#[derive(Clone, Copy, Debug)]
pub struct StepReadOptions {
	pub normalize_to: LengthUnit,
	pub allow_missing_units: bool,
	pub allow_partial_transfer: bool,
}
impl Default for StepReadOptions {
	fn default() -> Self {
		Self { normalize_to: LengthUnit::Millimeter, allow_missing_units: false, allow_partial_transfer: false }
	}
}
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum StepSchema {
	AP203 = 3,
	AP214 = 4,
	AP242 = 5,
}
#[derive(Clone, Copy, Debug)]
pub struct StepWriteOptions {
	pub model_unit: LengthUnit,
	pub output_unit: LengthUnit,
	pub schema: StepSchema,
	pub assembly: bool,
}
impl Default for StepWriteOptions {
	fn default() -> Self {
		Self { model_unit: LengthUnit::Millimeter, output_unit: LengthUnit::Millimeter, schema: StepSchema::AP214, assembly: true }
	}
}
#[derive(Clone, Debug)]
pub struct StepReadReport {
	pub detected_length_units: Vec<String>,
	pub normalized_millimeters_per_unit: f64,
	pub roots: usize,
	pub transferred_roots: usize,
}
#[derive(Debug)]
pub struct StepReadResult {
	pub result: OperationResult<Shape>,
	pub report: StepReadReport,
}
fn read_native<R: Read>(reader: &mut R, options: StepReadOptions, document: bool, control: &OperationControl) -> Result<ffi::StepReadData, Error> {
	control.check("STEP read")?;
	let mut bytes = Vec::new();
	reader.read_to_end(&mut bytes)?;
	let data = ffi::op_step_read(&bytes, options.normalize_to.millimeters(), options.allow_missing_units, options.allow_partial_transfer, document, control);
	control.check("STEP read")?;
	data.map_err(|e| kernel("STEP read", e))
}
fn report(data: &ffi::StepReadData) -> StepReadReport {
	StepReadReport { detected_length_units: data.units.clone(), normalized_millimeters_per_unit: data.normalized_unit, roots: data.roots, transferred_roots: data.transferred }
}
fn native_colors(shape: &mut Shape, ids: &[u64], rgb: &[f32]) {
	for (&id, c) in ids.iter().zip(rgb.chunks_exact(3)) {
		shape.colormap.insert(id, crate::Color { r: c[0], g: c[1], b: c[2] });
	}
}
fn config(options: StepWriteOptions) -> ffi::StepWriteConfig {
	ffi::StepWriteConfig { model_unit: options.model_unit.millimeters(), write_unit: options.output_unit as u32, schema: options.schema as u32, assembly: options.assembly }
}
fn diagnostics(messages: Vec<String>) -> Vec<Diagnostic> {
	messages.into_iter().map(|message| Diagnostic { message, subshapes: vec![] }).collect()
}
impl Shape {
	pub fn read_step_with_options<R: Read>(reader: &mut R, options: StepReadOptions, control: &OperationControl) -> Result<StepReadResult, Error> {
		let data = read_native(reader, options, false, control)?;
		let report = report(&data);
		let mut result = Self::operation(data.operation, &[]);
		native_colors(&mut result.value, &data.color_ids, &data.rgb);
		Ok(StepReadResult { result, report })
	}
	pub fn write_step_with_options<W: Write>(&self, writer: &mut W, options: StepWriteOptions, control: &OperationControl) -> Result<Vec<Diagnostic>, Error> {
		let ids: Vec<_> = self.colormap.keys().copied().collect();
		let rgb: Vec<_> = ids
			.iter()
			.flat_map(|id| {
				let c = self.colormap[id];
				[c.r, c.g, c.b]
			})
			.collect();
		control.check("STEP write")?;
		let data = ffi::op_step_write(&Self::vector([self]), &[], &[], &ids, &rgb, &config(options), control);
		control.check("STEP write")?;
		let data = data.map_err(|e| kernel("STEP write", e))?;
		writer.write_all(&data.bytes)?;
		Ok(diagnostics(data.warnings))
	}
}

#[cfg(feature = "step-document")]
pub mod document {
	use super::*;
	use crate::DMat4;
	#[derive(Debug)]
	pub struct StepDefinition {
		pub id: String,
		pub name: String,
		pub geometry: Option<Shape>,
		pub components: Vec<StepInstance>,
	}
	#[derive(Clone, Debug)]
	pub struct StepInstance {
		pub id: String,
		pub name: String,
		pub reference_designator: Option<String>,
		pub definition: usize,
		pub transform: Option<DMat4>,
	}
	#[derive(Debug, Default)]
	pub struct StepDocument {
		pub definitions: Vec<StepDefinition>,
		pub roots: Vec<StepInstance>,
	}
	#[derive(Debug)]
	pub struct StepDocumentRead {
		pub document: StepDocument,
		pub result: OperationResult<Shape>,
		pub report: StepReadReport,
	}
	fn instance(row: ffi::StepInstanceData) -> StepInstance {
		StepInstance {
			id: row.id,
			name: row.name,
			reference_designator: row.has_reference.then_some(row.reference),
			definition: row.definition as usize,
			transform: (row.transform.len() == 16).then(|| DMat4::from_cols_slice(&row.transform)),
		}
	}
	fn placement(matrix: Option<DMat4>) -> Result<Vec<f64>, Error> {
		let m = matrix.ok_or_else(|| Error::Unsupported { operation: "STEP document export".into(), kind: "unresolved instance placement".into() })?;
		super::super::report::finite(m.to_cols_array(), "assembly placement")?;
		let r = crate::DMat3::from_mat4(m);
		if m.x_axis.w != 0.0 || m.y_axis.w != 0.0 || m.z_axis.w != 0.0 || m.w_axis.w != 1.0 || (r.determinant() - 1.0).abs() > 1e-10 || !(r.transpose() * r).abs_diff_eq(crate::DMat3::IDENTITY, 1e-10) {
			return Err(Error::Validation("assembly placements must be proper rigid transforms".into()));
		}
		Ok(m.to_cols_array().to_vec())
	}
	impl StepDocument {
		pub fn read<R: Read>(reader: &mut R, options: StepReadOptions, control: &OperationControl) -> Result<StepDocumentRead, Error> {
			let data = read_native(reader, options, true, control)?;
			let report = report(&data);
			let mut definitions = Vec::new();
			for row in data.definitions {
				let geometry = if row.shape >= 0 {
					let mut shape = Shape::new(ffi::native_take(&data.geometry, row.shape as usize));
					native_colors(&mut shape, &data.color_ids, &data.rgb);
					Some(shape)
				} else {
					None
				};
				definitions.push(StepDefinition { id: row.id, name: row.name, geometry, components: Vec::new() });
			}
			let mut roots = Vec::new();
			for row in data.instances {
				let parent = row.parent;
				if parent < 0 {
					roots.push(instance(row));
				} else if let Some(definition) = definitions.get_mut(parent as usize) {
					definition.components.push(instance(row));
				}
			}
			for definition in &mut definitions {
				if !definition.components.is_empty() {
					definition.geometry = None;
				}
			}
			let mut result = Shape::operation(data.operation, &[]);
			native_colors(&mut result.value, &data.color_ids, &data.rgb);
			Ok(StepDocumentRead { document: Self { definitions, roots }, result, report })
		}
		pub fn write<W: Write>(&self, writer: &mut W, options: StepWriteOptions, control: &OperationControl) -> Result<Vec<Diagnostic>, Error> {
			if !options.assembly && self.definitions.iter().any(|d| !d.components.is_empty()) {
				return Err(Error::Validation("STEP document assembly export requires assembly mode".into()));
			}
			fn visit(index: usize, document: &StepDocument, state: &mut [u8]) -> Result<(), Error> {
				if index >= state.len() {
					return Err(Error::Validation("assembly definition index out of bounds".into()));
				}
				if state[index] == 1 {
					return Err(Error::Validation("cyclic assembly definitions".into()));
				}
				if state[index] == 2 {
					return Ok(());
				}
				state[index] = 1;
				let d = &document.definitions[index];
				if d.geometry.is_none() && d.components.is_empty() {
					return Err(Error::Validation("assembly definition has neither geometry nor components".into()));
				}
				if d.geometry.is_some() && !d.components.is_empty() {
					return Err(Error::Validation("assembly definition cannot mix leaf geometry with component definitions".into()));
				}
				for instance in &d.components {
					visit(instance.definition, document, state)?;
				}
				state[index] = 2;
				Ok(())
			}
			for definition in &self.definitions {
				if definition.id.contains('\0') || definition.name.contains('\0') {
					return Err(Error::Validation("STEP labels cannot contain NUL".into()));
				}
			}
			let mut root_definitions = std::collections::HashSet::new();
			for root in &self.roots {
				if !root_definitions.insert(root.definition) {
					return Err(Error::Unsupported { operation: "STEP document export".into(), kind: "repeated root definitions; use repeated components within an assembly".into() });
				}
			}
			let mut state = vec![0; self.definitions.len()];
			for root in &self.roots {
				visit(root.definition, self, &mut state)?;
			}
			if self.roots.is_empty() {
				return Err(Error::Validation("STEP document has no roots".into()));
			}
			let mut geometries = Vec::new();
			let mut defs = Vec::new();
			let mut ids = Vec::new();
			let mut rgb = Vec::new();
			for d in &self.definitions {
				let shape = if let Some(geometry) = &d.geometry {
					let index = geometries.len() as i32;
					geometries.push(geometry);
					for (&id, c) in &geometry.colormap {
						ids.push(id);
						rgb.extend([c.r, c.g, c.b]);
					}
					index
				} else {
					-1
				};
				defs.push(ffi::StepDefinitionData { id: d.id.clone(), name: d.name.clone(), shape });
			}
			let mut omitted = Vec::new();
			for root in &self.roots {
				if !root.id.is_empty() || (!root.name.is_empty() && root.name != self.definitions[root.definition].name) || root.reference_designator.is_some() {
					omitted.push(Diagnostic { message: "Root occurrence labels have no STEP assembly link and are omitted; product labels are retained".into(), subshapes: vec![] });
				}
			}
			for (i, &visited) in state.iter().enumerate() {
				if visited == 0 {
					omitted.push(Diagnostic { message: format!("Unreferenced definition {i} is omitted"), subshapes: vec![] });
				}
			}
			let mut instances = Vec::new();
			let mut add = |instance: &StepInstance, parent: i32| -> Result<(), Error> {
				if instance.definition >= defs.len() {
					return Err(Error::Validation("assembly definition index out of bounds".into()));
				}
				if instance.id.contains('\0') || instance.name.contains('\0') || instance.reference_designator.as_ref().is_some_and(|v| v.contains('\0')) {
					return Err(Error::Validation("STEP occurrence labels cannot contain NUL".into()));
				}
				instances.push(ffi::StepInstanceData {
					id: instance.id.clone(),
					name: instance.name.clone(),
					reference: instance.reference_designator.clone().unwrap_or_default(),
					has_reference: instance.reference_designator.is_some(),
					parent,
					definition: instance.definition as u32,
					transform: placement(instance.transform)?,
				});
				Ok(())
			};
			for root in &self.roots {
				add(root, -1)?;
			}
			for (i, d) in self.definitions.iter().enumerate() {
				for instance in &d.components {
					add(instance, i as i32)?;
				}
			}
			control.check("STEP document write")?;
			let data = ffi::op_step_write(&Shape::vector(geometries), &defs, &instances, &ids, &rgb, &config(options), control);
			control.check("STEP document write")?;
			let data = data.map_err(|e| kernel("STEP document write", e))?;
			writer.write_all(&data.bytes)?;
			omitted.extend(diagnostics(data.warnings));
			Ok(omitted)
		}
	}
}
