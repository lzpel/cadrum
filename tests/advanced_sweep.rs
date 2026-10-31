use cadrum::{AuxiliaryContact, DVec3, Edge, Frame, OperationControl, Shape, ShapeKind, SweepOptions, SweepOrientation, SweepSection, SweepTransition, Wire};
fn section(z: f64, r: f64) -> Shape {
	Wire::new([&Edge::ellipse(Frame { origin: DVec3::Z * z, ..Default::default() }, r, r, None).unwrap()]).unwrap().into()
}
fn line() -> Wire {
	Wire::new([&Edge::line(DVec3::ZERO, DVec3::Z * 5.0).unwrap()]).unwrap()
}
#[test]
fn scaling_law_simulation_and_multiple_sections() {
	let spine = line();
	let profile = section(0.0, 1.0);
	let sections = [SweepSection { profile: &profile, location: None, contact: false, correction: true }];
	let sweep = spine.sweep(&sections, SweepOptions { scaling: vec![[0.0, 1.0], [1.0, 2.0]], simulation_sections: 4, ..Default::default() }, &OperationControl::default()).unwrap();
	assert_eq!(sweep.sections.len(), 4);
	assert_eq!(sweep.status, 0);
	assert!((sweep.result.value.as_solid().unwrap().volume() - 35.0 * std::f64::consts::PI / 3.0).abs() < 1e-5);
	assert!(!sweep.result.history.complete);
	assert!(!sweep.result.warnings.is_empty());
	let end = section(5.0, 2.0);
	let vertices = spine.vertices().unwrap();
	let sections = [SweepSection { profile: &profile, location: Some(&vertices[0]), contact: false, correction: true }, SweepSection { profile: &end, location: Some(&vertices[1]), contact: false, correction: true }];
	assert!(spine.sweep(&sections, SweepOptions::default(), &OperationControl::default()).unwrap().result.value.is_valid().unwrap());
}
#[test]
fn open_profile_and_transition_policies() {
	let open: Shape = Wire::new([&Edge::line(DVec3::ZERO, DVec3::X).unwrap()]).unwrap().into();
	let spine = line();
	let sections = [SweepSection { profile: &open, location: None, contact: false, correction: false }];
	let sweep = spine.sweep(&sections, SweepOptions { solid: false, ..Default::default() }, &OperationControl::default()).unwrap();
	assert!(matches!(sweep.result.value.kind(), ShapeKind::Face | ShapeKind::Shell));
	assert!(sweep.result.value.solids().unwrap().is_empty());
	assert!(spine.sweep(&sections, SweepOptions::default(), &OperationControl::default()).is_err());
	let path = Wire::new(&[Edge::line(DVec3::ZERO, DVec3::Z * 5.0).unwrap(), Edge::line(DVec3::Z * 5.0, DVec3::new(5.0, 0.0, 5.0)).unwrap()]).unwrap();
	let profile = section(0.0, 0.2);
	for transition in [SweepTransition::Transformed, SweepTransition::RightCorner, SweepTransition::RoundCorner] {
		assert!(path.sweep(&[SweepSection { profile: &profile, location: None, contact: false, correction: true }], SweepOptions { transition, ..Default::default() }, &OperationControl::default()).unwrap().result.value.is_valid().unwrap());
	}
}
#[test]
fn discrete_fixed_binormal_and_auxiliary_frames() {
	let spine = line();
	let profile = section(0.0, 0.2);
	let sections = [SweepSection { profile: &profile, location: None, contact: false, correction: true }];
	let auxiliary = Wire::new([&Edge::line(DVec3::X, DVec3::new(1.0, 0.0, 5.0)).unwrap()]).unwrap();
	for orientation in [SweepOrientation::Discrete, SweepOrientation::Fixed(Frame::default()), SweepOrientation::Binormal(DVec3::X), SweepOrientation::Auxiliary { spine: &auxiliary, curvilinear: true, contact: AuxiliaryContact::None }] {
		assert!(spine.sweep(&sections, SweepOptions { orientation, ..Default::default() }, &OperationControl::default()).unwrap().result.value.is_valid().unwrap());
	}
	assert!(spine.sweep(&sections, SweepOptions { scaling: vec![[0.0, 1.0], [1.0, 0.0]], ..Default::default() }, &OperationControl::default()).is_err());
}

#[test]
fn helix_and_support_surface_modes() {
	let helix = Edge::helix(3.0, 2.0, 4.0, DVec3::Z, DVec3::X).unwrap();
	let frame = Frame { origin: helix.start_point(), axis_z: helix.start_tangent(), axis_x: DVec3::X };
	let profile: Shape = Wire::new([&Edge::ellipse(frame, 0.1, 0.1, None).unwrap()]).unwrap().into();
	let spine = Wire::new([&helix]).unwrap();
	assert!(spine.sweep(&[SweepSection { profile: &profile, location: None, contact: false, correction: true }], SweepOptions::default(), &OperationControl::default()).unwrap().result.value.is_valid().unwrap());
	let wire = Wire::new(&Edge::polygon(&[DVec3::ZERO, DVec3::X * 5.0, DVec3::new(5.0, 5.0, 0.0), DVec3::Y * 5.0]).unwrap()).unwrap();
	let support = cadrum::Face::from_wires(&wire, &[], 1e-7).unwrap();
	let edge = support.iter_edge().next().unwrap();
	let frame = Frame { origin: edge.start_point(), axis_z: edge.start_tangent(), axis_x: DVec3::Z };
	let profile: Shape = Wire::new([&Edge::ellipse(frame, 0.1, 0.1, None).unwrap()]).unwrap().into();
	let spine = Wire::new([edge]).unwrap();
	let support = Shape::from(&support);
	assert!(spine.sweep(&[SweepSection { profile: &profile, location: None, contact: false, correction: true }], SweepOptions { orientation: SweepOrientation::Support(&support), ..Default::default() }, &OperationControl::default()).unwrap().result.value.is_valid().unwrap());
}
