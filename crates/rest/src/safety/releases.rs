use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;

const RELEASES_KEPT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
	pub method: String,
	pub path: String,
	pub at: Instant,
}

#[derive(Clone, Default)]
pub struct Releases(Arc<Mutex<VecDeque<Release>>>);

impl Releases {
	pub fn snapshot(&self) -> Vec<Release> {
		self.0.lock().iter().cloned().collect()
	}

	pub(crate) fn record(&self, release: Release) {
		let mut kept = self.0.lock();
		if kept.len() == RELEASES_KEPT {
			kept.pop_front();
		}
		kept.push_back(release);
	}
}
