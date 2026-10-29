use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use parking_lot::Mutex;
use tokio::sync::Notify;

use super::VideoQuality;
use super::send::CameraSender;

const MAX_QUEUED_FRAMES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoLayer {
	pub rid: String,
	pub quality: u8,
	pub ssrc: u32,
	pub rtx_ssrc: u32,
}

pub(crate) struct LayerState {
	pub layer: VideoLayer,
	wanted: AtomicBool,
	keyframe: AtomicBool,
}

impl LayerState {
	fn new(layer: VideoLayer) -> Self {
		Self {
			layer,
			wanted: AtomicBool::new(true),
			keyframe: AtomicBool::new(true),
		}
	}

	pub fn wanted(&self) -> bool {
		self.wanted.load(Ordering::Relaxed)
	}

	pub fn set_wanted(&self, wanted: bool) -> bool {
		let changed = self.wanted.swap(wanted, Ordering::Relaxed) != wanted;
		if changed && wanted {
			self.request_keyframe();
		}
		changed
	}

	pub fn request_keyframe(&self) {
		self.keyframe.store(true, Ordering::Relaxed);
	}

	pub fn take_keyframe(&self) -> bool {
		self.keyframe.swap(false, Ordering::Relaxed)
	}
}

pub(crate) struct EncodedUnit {
	pub ssrc: u32,
	pub rid: String,
	pub data: Vec<u8>,
	pub timestamp: u32,
}

#[derive(Default)]
pub(crate) struct Outgoing {
	layers: Mutex<Vec<Arc<LayerState>>>,
	sending: AtomicBool,
	paused: AtomicBool,
	quality: Mutex<VideoQuality>,
	queue: Mutex<VecDeque<EncodedUnit>>,
	pub ready: Notify,
	pub sender: Mutex<Option<CameraSender>>,
	pub sent: AtomicU64,
	pub encoded: AtomicU64,
	pub dropped: AtomicU64,
}

impl Outgoing {
	pub fn set_layers(&self, layers: Vec<VideoLayer>) {
		*self.layers.lock() = layers
			.into_iter()
			.map(|layer| Arc::new(LayerState::new(layer)))
			.collect();
		self.queue.lock().clear();
	}

	pub fn layers(&self) -> Vec<Arc<LayerState>> {
		self.layers.lock().clone()
	}

	pub fn layer(&self, ssrc: u32) -> Option<Arc<LayerState>> {
		self.layers
			.lock()
			.iter()
			.find(|state| state.layer.ssrc == ssrc)
			.cloned()
	}

	pub fn sending(&self) -> bool {
		self.sending.load(Ordering::Acquire)
	}

	pub fn set_sending(&self, sending: bool) {
		self.sending.store(sending, Ordering::Release);
		if sending {
			for layer in self.layers.lock().iter() {
				layer.request_keyframe();
			}
		} else {
			self.queue.lock().clear();
		}
	}

	pub fn paused(&self) -> bool {
		self.paused.load(Ordering::Acquire)
	}

	pub fn set_paused(&self, paused: bool) {
		if self.paused.swap(paused, Ordering::AcqRel) == paused {
			return;
		}
		if paused {
			self.queue.lock().clear();
		} else {
			for layer in self.layers.lock().iter() {
				layer.request_keyframe();
			}
		}
	}

	pub fn quality(&self) -> VideoQuality {
		*self.quality.lock()
	}

	pub fn set_quality(&self, quality: VideoQuality) {
		*self.quality.lock() = quality;
	}

	pub fn push(&self, unit: EncodedUnit) {
		let mut queue = self.queue.lock();
		if queue.len() >= MAX_QUEUED_FRAMES {
			queue.pop_front();
			self.dropped.fetch_add(1, Ordering::Relaxed);
		}
		queue.push_back(unit);
		drop(queue);
		self.encoded.fetch_add(1, Ordering::Relaxed);
		self.ready.notify_one();
	}

	pub fn pop(&self) -> Option<EncodedUnit> {
		if !self.sending() || self.paused() {
			self.queue.lock().clear();
			return None;
		}
		self.queue.lock().pop_front()
	}
}
