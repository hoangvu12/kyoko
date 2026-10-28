use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tracing::warn;

use super::{VideoError, VideoFrame, VideoSource};

const FAILURE_BACKOFF: Duration = Duration::from_millis(100);
const MAX_FAILURES: u32 = 20;

pub struct CameraPreview {
	stop: Arc<AtomicBool>,
	thread: Option<JoinHandle<()>>,
}

impl CameraPreview {
	pub fn start(
		mut source: Box<dyn VideoSource>,
		show: impl Fn(Arc<VideoFrame>) + Send + 'static,
	) -> Result<Self, VideoError> {
		let stop = Arc::new(AtomicBool::new(false));
		let stopped = stop.clone();
		let thread = thread::Builder::new()
			.name("camera-preview".to_owned())
			.spawn(move || {
				let mut failures = 0;
				while !stopped.load(Ordering::Acquire) {
					match source.next_frame() {
						Ok(frame) => {
							failures = 0;
							show(Arc::new(frame));
						}
						Err(error) => {
							failures += 1;
							if failures >= MAX_FAILURES {
								warn!(%error, "the camera preview stopped");
								return;
							}
							thread::sleep(FAILURE_BACKOFF);
						}
					}
				}
			})
			.map_err(|error| VideoError::Device(error.to_string()))?;
		Ok(Self {
			stop,
			thread: Some(thread),
		})
	}

	pub fn running(&self) -> bool {
		self.thread.as_ref().is_some_and(|thread| !thread.is_finished())
	}
}

impl Drop for CameraPreview {
	fn drop(&mut self) {
		self.stop.store(true, Ordering::Release);
		self.thread.take();
	}
}
