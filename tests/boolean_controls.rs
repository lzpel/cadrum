use cadrum::{BooleanOp, BooleanOptions, DVec3, Error, GluePolicy, OperationControl, Shape, Solid};
use std::sync::{Arc, Mutex};
#[test]
fn fuzzy_policy_and_batch_preserve_disconnected_output() {
	let a = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::ONE));
	let b = Shape::from(&Solid::cube(DVec3::new(1.00001, 0.0, 0.0), DVec3::new(2.00001, 1.0, 1.0)));
	assert_eq!(a.boolean(BooleanOp::Union, &[&b]).unwrap().value.solids().unwrap().len(), 2);
	let fuzzy = Shape::boolean_batch(BooleanOp::Union, &[&a], &[&b], BooleanOptions { fuzzy_tolerance: 1e-4, ..Default::default() }, &OperationControl::default()).unwrap();
	assert_eq!(fuzzy.value.solids().unwrap().len(), 1);
	assert!(fuzzy.diagnostics[0].message.contains("fuzzy=0.0001"));
	let c = Shape::from(&Solid::cube(DVec3::splat(10.0), DVec3::splat(11.0)));
	let d = Shape::from(&Solid::cube(DVec3::splat(20.0), DVec3::splat(21.0)));
	let batch = Shape::boolean_batch(BooleanOp::Union, &[&a, &c], &[&b, &d], BooleanOptions { parallel: true, glue: GluePolicy::None, ..Default::default() }, &OperationControl::default()).unwrap();
	assert_eq!(batch.value.solids().unwrap().len(), 4);
	assert!(batch.history.relations.iter().any(|r| r.source.operand == 3));
}
#[test]
fn progress_and_cooperative_cancellation_stay_inside_ffi() {
	let a = Shape::from(&Solid::cube(DVec3::ZERO, DVec3::splat(10.0)));
	let b = Shape::from(&Solid::sphere(8.0));
	let fractions = Arc::new(Mutex::new(Vec::new()));
	let recorded = fractions.clone();
	let control = OperationControl::with_progress(move |p| {
		recorded.lock().unwrap().push(p.fraction);
		true
	});
	let result = Shape::boolean_batch(BooleanOp::Difference, &[&a], &[&b], BooleanOptions::default(), &control).unwrap();
	assert!(!result.value.is_empty().unwrap());
	let values = fractions.lock().unwrap();
	assert!(!values.is_empty());
	assert!(values.windows(2).all(|p| p[0] <= p[1]));
	drop(values);
	let observed = Arc::new(Mutex::new(false));
	let flag = observed.clone();
	let cancel = OperationControl::with_progress(move |p| {
		if p.fraction > 0.0 && p.fraction < 1.0 {
			*flag.lock().unwrap() = true;
			false
		} else {
			true
		}
	});
	assert!(matches!(Shape::boolean_batch(BooleanOp::Difference, &[&a], &[&b], BooleanOptions::default(), &cancel), Err(Error::Cancelled { .. })));
	assert!(*observed.lock().unwrap());
	let pre = OperationControl::default();
	pre.cancellation_token().cancel();
	assert!(matches!(Shape::boolean_batch(BooleanOp::Union, &[&a], &[&b], BooleanOptions::default(), &pre), Err(Error::Cancelled { .. })));
}
#[test]
fn callback_panics_are_reported_without_crossing_native_frames() {
	let a = Shape::from(&Solid::sphere(1.0));
	let b = Shape::from(&Solid::sphere(2.0));
	let control = OperationControl::with_progress(|_| panic!("callback failure"));
	assert!(matches!(Shape::boolean_batch(BooleanOp::Union, &[&a], &[&b], BooleanOptions::default(), &control), Err(Error::Operation { .. })));
}

#[test]
fn active_control_rejects_concurrent_reuse_before_native_work() {
	let (started_tx, started_rx) = std::sync::mpsc::channel();
	let (release_tx, release_rx) = std::sync::mpsc::channel();
	let mut first = true;
	let control = Arc::new(OperationControl::with_progress(move |_| {
		if first {
			first = false;
			started_tx.send(()).unwrap();
			release_rx.recv().unwrap();
		}
		true
	}));
	let worker_control = control.clone();
	let worker = std::thread::spawn(move || {
		let a = Shape::from(&Solid::sphere(1.0));
		let b = Shape::from(&Solid::sphere(2.0));
		Shape::boolean_batch(BooleanOp::Union, &[&a], &[&b], BooleanOptions::default(), &worker_control)
	});
	started_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
	let a = Shape::from(&Solid::sphere(1.0));
	let b = Shape::from(&Solid::sphere(2.0));
	let reuse = Shape::boolean_batch(BooleanOp::Union, &[&a], &[&b], BooleanOptions::default(), &control);
	release_tx.send(()).unwrap();
	let original = worker.join().unwrap();
	assert!(matches!(reuse, Err(Error::Operation { .. })));
	assert!(original.is_ok(), "rejecting reuse must leave the active operation running: {original:?}");
	assert!(Shape::boolean_batch(BooleanOp::Union, &[&a], &[&b], BooleanOptions::default(), &control).is_ok());
}
