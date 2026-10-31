//! Synchronous native progress with thread-safe cooperative cancellation.
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc, Mutex,
};
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
	pub fn cancel(&self) {
		self.0.store(true, Ordering::Release);
	}
	pub fn is_cancelled(&self) -> bool {
		self.0.load(Ordering::Acquire)
	}
}
#[derive(Clone, Copy, Debug)]
pub struct ProgressUpdate {
	pub fraction: f64,
}
type ProgressCallback = Box<dyn FnMut(ProgressUpdate) -> bool + Send>;
#[derive(Default)]
pub struct OperationControl {
	token: CancellationToken,
	callback: Mutex<Option<ProgressCallback>>,
	failed: AtomicBool,
}
impl OperationControl {
	/// Return false from the callback to cancel; callbacks may run on native worker threads.
	pub fn with_progress(callback: impl FnMut(ProgressUpdate) -> bool + Send + 'static) -> Self {
		Self { callback: Mutex::new(Some(Box::new(callback))), ..Default::default() }
	}
	pub fn cancellation_token(&self) -> CancellationToken {
		self.token.clone()
	}
	pub fn is_cancelled(&self) -> bool {
		self.token.is_cancelled()
	}
	pub(crate) fn check(&self, operation: &str) -> Result<(), crate::Error> {
		if self.failed.load(Ordering::Acquire) {
			Err(crate::Error::Operation { operation: operation.into(), diagnostics: vec!["progress callback panicked or reused its active control".into()], subshapes: vec![] })
		} else if self.is_cancelled() {
			Err(crate::Error::Cancelled { operation: operation.into() })
		} else {
			Ok(())
		}
	}
}
pub(crate) fn operation_cancelled(control: &OperationControl) -> bool {
	control.is_cancelled()
}
pub(crate) fn operation_progress(control: &OperationControl, fraction: f64) {
	let Ok(mut callback) = control.callback.try_lock() else {
		control.failed.store(true, Ordering::Release);
		control.token.cancel();
		return;
	};
	if let Some(callback) = callback.as_mut() {
		match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(ProgressUpdate { fraction }))) {
			Ok(true) => {}
			Ok(false) => control.token.cancel(),
			Err(_) => {
				control.failed.store(true, Ordering::Release);
				control.token.cancel();
			}
		}
	}
}
