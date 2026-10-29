use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use protocol::Snowflake;
use tokio::sync::oneshot;
use tracing::{debug, warn};

use super::outgoing::EncodedUnit;
use super::rtp::CLOCK_HZ;
use super::{EncoderConfig, H264Encoder, VideoCodecs};
use super::{VideoError, VideoQuality, VideoSource, VideoStream, fit, scale_nv12};
use crate::event::VoiceEvent;
use crate::shared::Shared;

const KEYFRAME_SECONDS: u32 = 2;
const FAILURE_BACKOFF: Duration = Duration::from_millis(250);
const MAX_CONSECUTIVE_FAILURES: u32 = 20;
const FRAME_SKIP_RATIO: f64 = 0.8;

pub(crate) struct CameraSender {
	stop: Arc<AtomicBool>,
	thread: Option<JoinHandle<()>>,
}

pub(crate) struct Camera {
	pub source: Box<dyn VideoSource>,
	pub quality: VideoQuality,
	pub codecs: VideoCodecs,
	pub user_id: Snowflake,
}

impl CameraSender {
	pub fn spawn(
		camera: Camera,
		shared: Arc<Shared>,
	) -> Result<(Self, oneshot::Receiver<Result<(), VideoError>>), VideoError> {
		let stop = Arc::new(AtomicBool::new(false));
		let stopped = stop.clone();
		let (ready, started) = oneshot::channel();
		let thread = thread::Builder::new()
			.name("video-send".to_owned())
			.spawn(move || {
				let mut pipeline = Pipeline::new(camera, shared, stopped);
				match pipeline.prepare() {
					Ok(()) => {
						let _ = ready.send(Ok(()));
						pipeline.run();
					}
					Err(error) => {
						let _ = ready.send(Err(error));
					}
				}
			})
			.map_err(|error| VideoError::Device(error.to_string()))?;
		Ok((
			Self {
				stop,
				thread: Some(thread),
			},
			started,
		))
	}
}

impl Drop for CameraSender {
	fn drop(&mut self) {
		self.stop.store(true, Ordering::Release);
		self.thread.take();
	}
}

struct Pipeline {
	camera: Camera,
	shared: Arc<Shared>,
	stop: Arc<AtomicBool>,
	encoders: Vec<(u8, H264Encoder)>,
	scratch: Vec<u8>,
	started: Instant,
	base_timestamp: u32,
	last_encoded: Option<Instant>,
}

impl Pipeline {
	fn new(camera: Camera, shared: Arc<Shared>, stop: Arc<AtomicBool>) -> Self {
		Self {
			camera,
			shared,
			stop,
			encoders: Vec::new(),
			scratch: Vec::new(),
			started: Instant::now(),
			base_timestamp: rand::random(),
			last_encoded: None,
		}
	}

	fn prepare(&mut self) -> Result<(), VideoError> {
		let layers = self.shared.video.outgoing.layers();
		if layers.is_empty() {
			return Err(VideoError::NoLayers);
		}
		let size = self.camera.source.size();
		for state in layers {
			self.encoder_for(state.layer.quality, size)?;
		}
		Ok(())
	}

	fn run(mut self) {
		let mut failures = 0;
		while !self.stop.load(Ordering::Acquire) {
			match self.step() {
				Ok(()) => failures = 0,
				Err(VideoError::Ended) => {
					debug!("the video source ended");
					self.shared.hub.emit(VoiceEvent::VideoSourceEnded);
					break;
				}
				Err(error) => {
					failures += 1;
					warn!(%error, "camera frame was not sent");
					if failures >= MAX_CONSECUTIVE_FAILURES {
						self.shared.hub.emit(VoiceEvent::CameraStopped);
						break;
					}
					thread::sleep(FAILURE_BACKOFF);
				}
			}
		}
		debug!("camera pipeline stopped");
	}

	fn encoder_for(&mut self, layer: u8, source: (u32, u32)) -> Result<&mut H264Encoder, VideoError> {
		let target = self.camera.quality.layer(layer);
		let (width, height) = fit(source.0, source.1, target.width, target.height);
		let config = EncoderConfig {
			width,
			height,
			fps: target.fps.max(1),
			bitrate: target.bitrate,
			keyframe_seconds: KEYFRAME_SECONDS,
		};
		let position = self.encoders.iter().position(|(quality, _)| *quality == layer);
		let current = position.filter(|index| self.encoders[*index].1.config() == config);
		let index = match (current, position) {
			(Some(index), _) => index,
			(None, Some(index)) => {
				self.encoders[index].1 = self.camera.codecs.encoder(config)?;
				index
			}
			(None, None) => {
				self.encoders.push((layer, self.camera.codecs.encoder(config)?));
				self.encoders.len() - 1
			}
		};
		Ok(&mut self.encoders[index].1)
	}

	fn step(&mut self) -> Result<(), VideoError> {
		let frame = Arc::new(self.camera.source.next_frame()?);
		if let Some(sink) = self.shared.video.sink() {
			sink.frame(
				VideoStream {
					user_id: self.camera.user_id,
					ssrc: 0,
				},
				frame.clone(),
			);
		}
		let outgoing = &self.shared.video.outgoing;
		if !outgoing.sending() || outgoing.paused() {
			return Ok(());
		}
		let now = Instant::now();
		let gap = Duration::from_secs(1) / self.camera.quality.fps.max(1);
		if self
			.last_encoded
			.is_some_and(|last| now.duration_since(last) < gap.mul_f64(FRAME_SKIP_RATIO))
		{
			return Ok(());
		}
		self.last_encoded = Some(now);
		let elapsed = self.started.elapsed().as_secs_f64() * f64::from(CLOCK_HZ);
		let timestamp = self.base_timestamp.wrapping_add(elapsed as u64 as u32);
		let size = (frame.width, frame.height);
		let mut scratch = std::mem::take(&mut self.scratch);
		let result = self.encode_layers(&frame, size, timestamp, &mut scratch);
		self.scratch = scratch;
		result
	}

	fn encode_layers(
		&mut self,
		frame: &super::VideoFrame,
		size: (u32, u32),
		timestamp: u32,
		scratch: &mut Vec<u8>,
	) -> Result<(), VideoError> {
		let shared = self.shared.clone();
		let outgoing = &shared.video.outgoing;
		for state in outgoing.layers() {
			if !state.wanted() {
				continue;
			}
			let keyframe = state.take_keyframe();
			let encoder = self.encoder_for(state.layer.quality, size)?;
			if keyframe {
				encoder.request_keyframe();
			}
			let config = encoder.config();
			scale_nv12(frame, config.width, config.height, scratch)?;
			for unit in encoder.encode(scratch, frame.timestamp)? {
				outgoing.push(EncodedUnit {
					ssrc: state.layer.ssrc,
					rid: state.layer.rid.clone(),
					data: unit.data,
					timestamp,
				});
			}
		}
		Ok(())
	}
}
