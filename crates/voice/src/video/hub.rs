use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use protocol::Snowflake;

use super::outgoing::Outgoing;
use super::{VideoSink, VideoWant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VideoStats {
	pub frames_received: u64,
	pub frames_decoded: u64,
	pub frames_dropped: u64,
	pub keyframe_requests: u64,
	pub frames_encoded: u64,
	pub frames_sent: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteVideo {
	pub primary: u32,
	pub ssrcs: Vec<u32>,
}

#[derive(Default)]
pub(crate) struct VideoHub {
	pub sink: Mutex<Option<Arc<dyn VideoSink>>>,
	pub remote: Mutex<HashMap<Snowflake, RemoteVideo>>,
	pub wants: Mutex<Vec<VideoWant>>,
	pub decode_failures: Mutex<Vec<u32>>,
	pub received: AtomicU64,
	pub decoded: AtomicU64,
	pub dropped: AtomicU64,
	pub keyframe_requests: AtomicU64,
	pub outgoing: Outgoing,
}

impl VideoHub {
	pub fn owner(&self, ssrc: u32) -> Option<Snowflake> {
		self.remote
			.lock()
			.iter()
			.find(|(_, video)| video.ssrcs.contains(&ssrc))
			.map(|(user_id, _)| *user_id)
	}

	pub fn sink(&self) -> Option<Arc<dyn VideoSink>> {
		self.sink.lock().clone()
	}

	pub fn drop_frames(&self, count: u64) {
		self.dropped.fetch_add(count, Ordering::Relaxed);
	}

	pub fn report_decode_failure(&self, ssrc: u32) {
		self.decode_failures.lock().push(ssrc);
	}

	pub fn stats(&self) -> VideoStats {
		VideoStats {
			frames_received: self.received.load(Ordering::Relaxed),
			frames_decoded: self.decoded.load(Ordering::Relaxed),
			frames_dropped: self.dropped.load(Ordering::Relaxed),
			keyframe_requests: self.keyframe_requests.load(Ordering::Relaxed),
			frames_encoded: self.outgoing.encoded.load(Ordering::Relaxed),
			frames_sent: self.outgoing.sent.load(Ordering::Relaxed),
		}
	}
}
