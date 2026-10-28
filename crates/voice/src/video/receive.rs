use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

use tracing::{debug, warn};

use super::rtp::{CLOCK_HZ, H264Depacketizer};
use super::{H264Decoder, VideoStream};
use crate::media::SharedTransform;
use crate::rtp::Header;
use crate::shared::Shared;
use crate::transform::Codec;
use crate::transport::Transport;

const QUEUE_DEPTH: usize = 8;
const KEYFRAME_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const STREAM_TTL: Duration = Duration::from_secs(10);
const RTCP_VERSION_2: u8 = 0x80;
const PLI_FORMAT: u8 = 1;
const PSFB_PAYLOAD_TYPE: u8 = 206;
const PLI_LENGTH_WORDS: u16 = 2;
const RTCP_HEADER_LEN: usize = 8;

struct Incoming {
	depacketizer: H264Depacketizer,
	needs_keyframe: bool,
	last_request: Option<Instant>,
	last_packet: Instant,
}

pub(crate) struct VideoReceive {
	transport: Arc<dyn Transport>,
	shared: Arc<Shared>,
	transform: SharedTransform,
	streams: HashMap<u32, Incoming>,
	worker: Option<DecodeWorker>,
}

impl VideoReceive {
	pub fn new(transport: Arc<dyn Transport>, shared: Arc<Shared>, transform: SharedTransform) -> Self {
		Self {
			transport,
			shared,
			transform,
			streams: HashMap::new(),
			worker: None,
		}
	}

	pub fn on_packet(&mut self, header: &Header, payload: &[u8], now: Instant) {
		let ssrc = header.ssrc;
		let Some(user_id) = self.shared.video.owner(ssrc) else {
			return;
		};
		let incoming = self.streams.entry(ssrc).or_insert_with(|| Incoming {
			depacketizer: H264Depacketizer::default(),
			needs_keyframe: false,
			last_request: None,
			last_packet: now,
		});
		incoming.last_packet = now;
		let output = incoming
			.depacketizer
			.push(header.sequence, header.timestamp, header.marker, payload);
		self.shared.video.drop_frames(u64::from(output.dropped));
		let starved = output.lost || (output.dropped > 0 && incoming.depacketizer.waiting_for_keyframe());
		let opened = output.frame.map(|frame| {
			let clear = self.transform.lock().decrypt(Codec::H264, ssrc, &frame.data);
			(frame, clear)
		});
		let mut wants_keyframe = starved;
		match opened {
			Some((frame, Some(clear))) => {
				self.shared.video.received.fetch_add(1, Ordering::Relaxed);
				if frame.keyframe
					&& let Some(incoming) = self.streams.get_mut(&ssrc)
				{
					incoming.needs_keyframe = false;
				}
				self.submit(VideoStream { user_id, ssrc }, clear, frame.timestamp);
			}
			Some((_, None)) => {
				self.shared.video.drop_frames(1);
				if let Some(incoming) = self.streams.get_mut(&ssrc) {
					incoming.depacketizer.require_keyframe();
				}
				wants_keyframe = true;
			}
			None => {}
		}
		if wants_keyframe {
			if let Some(incoming) = self.streams.get_mut(&ssrc) {
				incoming.needs_keyframe = true;
			}
			self.request_keyframe(ssrc, now);
		}
	}

	fn submit(&mut self, stream: VideoStream, data: Vec<u8>, timestamp: u32) {
		if self.worker.is_none() {
			self.worker = DecodeWorker::spawn(self.shared.clone());
		}
		let accepted = self.worker.as_ref().is_some_and(|worker| {
			worker.submit(DecodeJob {
				stream,
				data,
				timestamp: Duration::from_secs_f64(f64::from(timestamp) / f64::from(CLOCK_HZ)),
			})
		});
		if !accepted {
			self.shared.video.drop_frames(1);
		}
	}

	fn request_keyframe(&mut self, ssrc: u32, now: Instant) {
		let Some(incoming) = self.streams.get_mut(&ssrc) else {
			return;
		};
		if incoming
			.last_request
			.is_some_and(|last| now.duration_since(last) < KEYFRAME_REQUEST_INTERVAL)
		{
			return;
		}
		incoming.last_request = Some(now);
		let mut header = [0u8; RTCP_HEADER_LEN];
		header[0] = RTCP_VERSION_2 | PLI_FORMAT;
		header[1] = PSFB_PAYLOAD_TYPE;
		header[2..4].copy_from_slice(&PLI_LENGTH_WORDS.to_be_bytes());
		header[4..8].copy_from_slice(&self.transport.ssrc().to_be_bytes());
		if self.transport.send(&header, &ssrc.to_be_bytes()) {
			self.shared
				.video
				.keyframe_requests
				.fetch_add(1, Ordering::Relaxed);
		}
	}

	pub fn housekeeping(&mut self, now: Instant) {
		let failed = std::mem::take(&mut *self.shared.video.decode_failures.lock());
		for ssrc in failed {
			if let Some(incoming) = self.streams.get_mut(&ssrc) {
				incoming.depacketizer.require_keyframe();
				incoming.needs_keyframe = true;
			}
		}
		let waiting: Vec<u32> = self
			.streams
			.iter()
			.filter(|(_, incoming)| incoming.needs_keyframe)
			.map(|(ssrc, _)| *ssrc)
			.collect();
		for ssrc in waiting {
			self.request_keyframe(ssrc, now);
		}
		let video = &self.shared.video;
		self.streams.retain(|ssrc, incoming| {
			video.owner(*ssrc).is_some() && now.duration_since(incoming.last_packet) < STREAM_TTL
		});
	}
}

struct DecodeJob {
	stream: VideoStream,
	data: Vec<u8>,
	timestamp: Duration,
}

struct DecodeWorker {
	jobs: SyncSender<DecodeJob>,
}

impl DecodeWorker {
	fn spawn(shared: Arc<Shared>) -> Option<Self> {
		let (jobs, receiver) = sync_channel(QUEUE_DEPTH);
		thread::Builder::new()
			.name("video-decode".to_owned())
			.spawn(move || decode_loop(&shared, &receiver))
			.ok()?;
		Some(Self { jobs })
	}

	fn submit(&self, job: DecodeJob) -> bool {
		!matches!(
			self.jobs.try_send(job),
			Err(TrySendError::Full(_) | TrySendError::Disconnected(_))
		)
	}
}

fn decode_loop(shared: &Shared, receiver: &Receiver<DecodeJob>) {
	let mut decoders: HashMap<u32, H264Decoder> = HashMap::new();
	let mut unavailable = false;
	while let Ok(job) = receiver.recv() {
		let ssrc = job.stream.ssrc;
		let decoder = match decoders.entry(ssrc) {
			Entry::Occupied(entry) => entry.into_mut(),
			Entry::Vacant(entry) => match H264Decoder::new() {
				Ok(decoder) => entry.insert(decoder),
				Err(error) => {
					if !std::mem::replace(&mut unavailable, true) {
						warn!(%error, "video unavailable: no h264 decoder");
					}
					shared.video.drop_frames(1);
					continue;
				}
			},
		};
		match decoder.decode(&job.data, job.timestamp) {
			Ok(Some(frame)) => {
				shared.video.decoded.fetch_add(1, Ordering::Relaxed);
				if let Some(sink) = shared.video.sink() {
					sink.frame(job.stream, Arc::new(frame));
				}
			}
			Ok(None) => {}
			Err(error) => {
				debug!(%error, ssrc, "h264 decode failed");
				decoders.remove(&ssrc);
				shared.video.drop_frames(1);
				shared.video.report_decode_failure(ssrc);
			}
		}
	}
}
