use cadrum::{CurveControlData, DVec3, Edge, Frame, Shape, SplineAxis, SurfaceControlData};
fn clamped(degree: u32) -> SplineAxis {
	SplineAxis { degree, knots: vec![0.0, 1.0], multiplicities: vec![degree as i32 + 1; 2], periodic: false }
}
#[test]
fn rational_conic_and_cubic_use_control_points() {
	let conic = CurveControlData::Bezier { poles: vec![DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y], weights: vec![1.0, 0.5f64.sqrt(), 1.0] };
	let edge = conic.edge(None).unwrap();
	let midpoint = edge.evaluate(0.5).unwrap();
	assert!((midpoint.point - DVec3::new(0.5f64.sqrt(), 0.5f64.sqrt(), 0.0)).length() < 1e-12);
	assert!(midpoint.first.dot(midpoint.point).abs() < 1e-12);
	assert_eq!(edge.control_data().unwrap(), conic);
	let cubic = CurveControlData::Bezier { poles: vec![DVec3::ZERO, DVec3::new(0.0, 2.0, 0.0), DVec3::new(1.0, 2.0, 0.0), DVec3::X], weights: vec![] };
	let edge = cubic.edge(Some([0.2, 0.8])).unwrap();
	assert!((edge.evaluate(0.5).unwrap().point - DVec3::new(0.5, 1.5, 0.0)).length() < 1e-12);
	assert_eq!(edge.trimmed([0.3, 0.7]).unwrap().parameter_range().unwrap(), [0.3, 0.7]);
	assert!(edge.trimmed([0.0, 1.0]).is_err());
}
#[test]
fn authored_bspline_and_periodic_data_round_trip() {
	let curve = CurveControlData::BSpline { poles: vec![DVec3::ZERO, DVec3::new(0.5, 2.0, 0.0), DVec3::X], weights: vec![1.0; 3], axis: clamped(2) };
	let edge = curve.edge(None).unwrap();
	assert_eq!(edge.control_data().unwrap(), curve);
	assert!((edge.evaluate(0.5).unwrap().point.y - 1.0).abs() < 1e-12);
	let periodic = CurveControlData::BSpline { poles: vec![DVec3::ZERO, DVec3::X, DVec3::Y], weights: vec![1.0; 3], axis: SplineAxis { degree: 2, knots: vec![0.0, 1.0, 2.0, 3.0], multiplicities: vec![1; 4], periodic: true } };
	let edge = periodic.edge(None).unwrap();
	let [a, b] = edge.parameter_range().unwrap();
	assert!((edge.evaluate(a).unwrap().point - edge.evaluate(b).unwrap().point).length() < 1e-12);
	assert_eq!(edge.control_data().unwrap(), periodic);
	let mut bytes = Vec::new();
	Shape::from(&edge).write_brep(&mut bytes).unwrap();
	let restored = Shape::read_brep(&mut bytes.as_slice()).unwrap().as_edge().unwrap();
	assert_eq!(restored.control_data().unwrap(), periodic);
}
#[test]
fn weighted_surface_trim_and_independent_degrees() {
	let data = SurfaceControlData::Bezier { u_count: 2, v_count: 2, poles: vec![DVec3::ZERO, DVec3::Y, DVec3::X, DVec3::ONE], weights: vec![1.0, 1.0, 1.0, 2.0] };
	let face = data.face(Some([0.2, 0.8, 0.1, 0.9])).unwrap();
	assert!((face.evaluate_uv(0.5, 0.5).unwrap().point - DVec3::new(0.6, 0.6, 0.4)).length() < 1e-12);
	assert_eq!(face.control_data().unwrap(), data);
	assert_eq!(face.surface_patch([0.3, 0.7, 0.3, 0.7]).unwrap().parameter_range().unwrap(), [0.3, 0.7, 0.3, 0.7]);
	let bs = SurfaceControlData::BSpline {
		u_count: 2,
		v_count: 3,
		poles: (0..2).flat_map(|u| (0..3).map(move |v| DVec3::new(u as f64, v as f64, (u * v) as f64))).collect(),
		weights: vec![1.0; 6],
		u: clamped(1),
		v: clamped(2),
	};
	assert_eq!(bs.face(None).unwrap().control_data().unwrap(), bs);
}
#[test]
fn ellipses_and_invalid_control_data() {
	let edge = Edge::ellipse(Frame { origin: DVec3::splat(3.0), axis_z: DVec3::Y, axis_x: DVec3::X }, 2.0, 1.0, Some([0.0, std::f64::consts::PI])).unwrap();
	assert!((edge.evaluate(0.0).unwrap().point - DVec3::new(5.0, 3.0, 3.0)).length() < 1e-12);
	assert!(CurveControlData::BSpline { poles: vec![DVec3::ZERO, DVec3::X], weights: vec![], axis: clamped(3) }.edge(None).is_err());
	assert!(CurveControlData::Bezier { poles: vec![DVec3::ZERO, DVec3::X], weights: vec![0.0, 1.0] }.edge(None).is_err());
	assert!(Edge::ellipse(Frame::default(), 1.0, 2.0, None).is_err());
}
