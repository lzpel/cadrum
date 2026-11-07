#![cfg(feature = "step-document")]
use cadrum::{DMat4, DVec3, LengthUnit, OperationControl, Shape, Solid, StepDefinition, StepDocument, StepInstance, StepReadOptions, StepWriteOptions};
fn instance(definition: usize, name: &str, translation: DVec3) -> StepInstance {
	StepInstance { id: format!("id-{name}"), name: name.into(), reference_designator: Some(format!("ref-{name}")), definition, transform: Some(DMat4::from_translation(translation)) }
}
fn document() -> StepDocument {
	StepDocument {
		definitions: vec![
			StepDefinition { id: "P".into(), name: "Part".into(), geometry: Some(Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE).color("#ff0000"))), components: vec![] },
			StepDefinition { id: "S".into(), name: "Subassembly".into(), geometry: None, components: vec![instance(0, "left", DVec3::ZERO), instance(0, "right", DVec3::X * 2.0)] },
			StepDefinition { id: "T".into(), name: "Top".into(), geometry: None, components: vec![instance(1, "first", DVec3::Z * 5.0), instance(1, "second", DVec3::new(10.0, 0.0, 5.0))] },
			StepDefinition { id: "B".into(), name: "Ball".into(), geometry: Some(Shape::from(&Solid::sphere(1.0).color("#0000ff"))), components: vec![] },
		],
		roots: vec![StepInstance { id: String::new(), name: String::new(), reference_designator: None, ..instance(2, "root", DVec3::ZERO) }, StepInstance { id: String::new(), name: String::new(), reference_designator: None, ..instance(3, "extra", DVec3::X * 30.0) }],
	}
}
#[test]
fn nested_shared_assemblies_names_labels_placements_and_colors_round_trip() {
	let mut doc = document();
	doc.definitions[2].components[1].transform = Some(DMat4::from_translation(DVec3::new(10.0, 0.0, 5.0)) * DMat4::from_rotation_z(std::f64::consts::FRAC_PI_2));
	let mut bytes = Vec::new();
	let warnings = doc.write(&mut bytes, StepWriteOptions { output_unit: LengthUnit::Meter, ..Default::default() }, &OperationControl::default()).unwrap();
	assert!(warnings.is_empty(), "{warnings:?}");
	let read = StepDocument::read(&mut bytes.as_slice(), StepReadOptions::default(), &OperationControl::default()).unwrap();
	assert!(read.result.warnings.is_empty(), "{:?}", read.result.warnings);
	assert_eq!(read.report.roots, 2);
	assert_eq!(read.document.roots.len(), 2);
	assert_eq!(read.result.value.solids().unwrap().len(), 5);
	assert_eq!(read.document.definitions.len(), 4);
	let top = read.document.definitions.iter().find(|d| d.name == "Top").unwrap();
	assert_eq!(top.id, "T");
	assert_eq!(top.components.len(), 2);
	assert_eq!(top.components[0].definition, top.components[1].definition);
	let second = top.components.iter().find(|i| i.name == "second").unwrap();
	assert!((second.transform.unwrap().w_axis.truncate() - DVec3::new(10.0, 0.0, 5.0)).length() < 1e-6);
	assert!((second.transform.unwrap().x_axis.truncate() - DVec3::Y).length() < 1e-6);
	let sub = &read.document.definitions[second.definition];
	assert_eq!(sub.name, "Subassembly");
	assert_eq!(sub.components.len(), 2);
	assert_eq!(sub.components[0].definition, sub.components[1].definition);
	let right = sub.components.iter().find(|i| i.name == "right").unwrap();
	assert_eq!(right.reference_designator.as_deref(), Some("ref-right"));
	assert!((right.transform.unwrap().w_axis.truncate() - DVec3::X * 2.0).length() < 1e-6);
	let mesh = read.result.value.mesh(Default::default()).unwrap();
	assert!(mesh.colormap.values().any(|c| c.r == 1.0 && c.b == 0.0));
	assert!(mesh.colormap.values().any(|c| c.r == 0.0 && c.b == 1.0));
	let mut second_export = Vec::new();
	read.document.write(&mut second_export, StepWriteOptions::default(), &OperationControl::default()).unwrap();
	let again = StepDocument::read(&mut second_export.as_slice(), StepReadOptions::default(), &OperationControl::default()).unwrap();
	assert_eq!(again.result.value.solids().unwrap().len(), 5);
}
#[test]
fn invalid_assembly_graphs_and_nonrigid_placements_are_rejected() {
	let mut cyclic = document();
	cyclic.definitions[1].components.push(instance(2, "cycle", DVec3::ZERO));
	assert!(cyclic.write(&mut Vec::new(), StepWriteOptions::default(), &OperationControl::default()).is_err());
	let mut invalid = document();
	invalid.roots[0].transform = Some(DMat4::from_scale(DVec3::splat(2.0)));
	assert!(invalid.write(&mut Vec::new(), StepWriteOptions::default(), &OperationControl::default()).is_err());
}
