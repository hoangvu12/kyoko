use std::future::{Future, pending};

use tokio::sync::watch;

use crate::error::{RestError, Result};

pub(crate) struct Stop {
	stopped: watch::Sender<bool>,
}

impl Stop {
	pub fn new() -> Self {
		Self {
			stopped: watch::Sender::new(false),
		}
	}

	pub fn trigger(&self) {
		if !self.stopped.send_replace(true) {
			tracing::warn!("the token was rejected, every request is stopped");
		}
	}

	pub fn is_stopped(&self) -> bool {
		*self.stopped.borrow()
	}

	pub fn ensure_running(&self) -> Result<()> {
		if self.is_stopped() {
			return Err(RestError::Unauthorized);
		}
		Ok(())
	}

	pub fn waiter(&self) -> impl Future<Output = ()> + Send + 'static {
		let mut stopped = self.stopped.subscribe();
		async move {
			if stopped.wait_for(|stopped| *stopped).await.is_err() {
				pending::<()>().await;
			}
		}
	}

	pub async fn unless_stopped<F: Future>(&self, future: F) -> Result<F::Output> {
		self.ensure_running()?;
		tokio::select! {
			biased;
			() = self.waiter() => Err(RestError::Unauthorized),
			output = future => Ok(output),
		}
	}
}
