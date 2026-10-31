use cadrum::{CurveControlData, CurveKind, DVec2, DVec3, Edge, Face, Frame, GeometrySurfaceKind, MeasurementKind, PointLocation, Shape, ShapeKind, Solid, Wire};
#[test]
fn curve_classification_lengths_and_arclength_samples() {
	let line = Edge::line(DVec3::ZERO, DVec3::new(3.0, 4.0, 0.0)).unwrap();
	assert_eq!(line.curve_info().unwrap().kind, CurveKind::Line);
	assert!((line.length(1e-10).unwrap() - 5.0).abs() < 1e-12);
	let circle = Edge::circle(2.0, DVec3::Z).unwrap();
	assert_eq!(circle.curve_info().unwrap().kind, CurveKind::Circle);
	assert_eq!(circle.curve_info().unwrap().parameters, [2.0]);
	assert!((circle.curvature(1.0).unwrap() - 0.5).abs() < 1e-12);
	assert!((circle.length(1e-9).unwrap() - 4.0 * std::f64::consts::PI).abs() < 1e-10);
	let ellipse = Edge::ellipse(Frame::default(), 3.0, 1.0, None).unwrap();
	assert_eq!(ellipse.curve_info().unwrap().kind, CurveKind::Ellipse);
	let samples = ellipse.sample_by_length(9, 1e-8).unwrap();
	let length = ellipse.length(1e-8).unwrap();
	for pair in samples.windows(2) {
		let segment = ellipse.trimmed([pair[0].0, pair[1].0]).unwrap();
		assert!((segment.length(1e-8).unwrap() - length / 8.0).abs() < 1e-6);
	}
	let spline = CurveControlData::Bezier { poles: vec![DVec3::ZERO, DVec3::Y, DVec3::X], weights: vec![] }.edge(None).unwrap();
	assert_eq!(spline.curve_info().unwrap().kind, CurveKind::Bezier);
	assert!(spline.length(1e-8).unwrap() > 1.0);
}
#[test]
fn trimmed_surface_membership_curvature_and_boundaries() {
	let outer = Wire::new([&Edge::circle(3.0, DVec3::Z).unwrap()]).unwrap();
	let hole = Wire::new([&Edge::circle(1.0, DVec3::Z).unwrap()]).unwrap();
	let face = Face::from_wires(&outer, &[&hole], 1e-7).unwrap();
	assert_eq!(face.surface_info().unwrap().kind, GeometrySurfaceKind::Plane);
	assert_eq!(face.classify_uv(DVec2::ZERO, 1e-7).unwrap(), PointLocation::Outside);
	assert_eq!(face.boundary_wires().unwrap().len(), 2);
	assert!(face.boundary_wires().unwrap()[0].outer);
	assert!((face.area_with_accuracy(1e-9).unwrap() - 8.0 * std::f64::consts::PI).abs() < 1e-7);
	let sphere = Solid::sphere(2.0);
	let spherical = sphere.iter_face().next().unwrap();
	let curvature = spherical.curvature_uv(1.0, 0.2).unwrap();
	assert!((curvature.gaussian - 0.25).abs() < 1e-10);
	assert!((curvature.maximum.abs() - 0.5).abs() < 1e-10);
	assert!(curvature.maximum_direction.dot(curvature.minimum_direction).abs() < 1e-10);
	assert!(spherical.surface_info().unwrap().u_period.is_some());
}
#[test]
fn closest_points_mass_properties_and_adjacency() {
	let a = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let b = Shape::from(&Solid::cube(DVec3::X * 3.0, DVec3::new(4.0, 1.0, 1.0)));
	let report = a.distance(&b, 1e-7).unwrap();
	assert!((report.distance - 2.0).abs() < 1e-10);
	assert!(report.pairs.len() >= 4);
	for pair in report.pairs {
		assert!((pair.first_point.distance(pair.second_point) - 2.0).abs() < 1e-7);
		assert!(!pair.first_support.is_empty().unwrap());
	}
	let touch = Shape::from(&Solid::cube(DVec3::X, DVec3::new(2.0, 1.0, 1.0)));
	assert!(a.distance(&touch, 1e-7).unwrap().distance < 1e-10);
	let overlap = Shape::from(&Solid::sphere(0.2).translate(DVec3::splat(0.5)));
	assert!(a.distance(&overlap, 1e-7).unwrap().distance < 1e-10);
	let mass = a.mass_properties(MeasurementKind::Volume, 1e-9).unwrap();
	assert!((mass.mass - 1.0).abs() < 1e-12);
	assert!((mass.center - DVec3::splat(0.5)).length() < 1e-12);
	assert_eq!(a.classify_point(DVec3::splat(0.5), 1e-7).unwrap(), PointLocation::Inside);
	assert_eq!(a.classify_point(DVec3::ZERO, 1e-7).unwrap(), PointLocation::OnBoundary);
	let edge = Shape::from(&a.edges().unwrap()[0]);
	assert_eq!(a.ancestors(&edge, ShapeKind::Face).unwrap().len(), 2);
}
