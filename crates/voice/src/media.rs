use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use denoise::NoiseSuppressor;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, warn};

use crate::chain::{CaptureChain, PlaybackChain};
use crate::codec::{DEFAULT_BITRATE_BPS, FRAME_SAMPLES, SILENCE_FRAME, STEREO_FRAME_SAMPLES, VoiceEncoder};
use crate::gate::{VoiceGate, decayed_db, meter_level, rms, to_db};
use crate::jitter::Push;
use crate::mixer::Mixer;
use crate::processing::Processor;
use crate::remote::{Frame, RemoteStream};
use crate::rtp;
use crate::shared::{Control, InputMode, MixedFrame, Shared};
use crate::transform::{Codec, FrameTransform};
use crate::transport::{Inbound, Transport};
use crate::video::packetize;
use crate::video::receive::VideoReceive;
use crate::wire::{H264_PAYLOAD_TYPE, OPUS_PAYLOAD_TYPE};

const TICK: Duration = Duration::from_millis(10);
const FRAME_DURATION: Duration = Duration::from_millis(20);
const HOUSEKEEPING_INTERVAL: Duration = Duration::from_secs(1);
const STREAM_TTL: Duration = Duration::from_secs(10);
const SILENCE_FRAMES_BEFORE_PAUSE: u8 = 5;
const MAX_DTX_PACKET_BYTES: usize = 2;
const MAX_FRAMES_PER_TICK: usize = 6;
const MAX_PLAYOUT_LAG: Duration = Duration::from_millis(100);
const METER_DECAY: f32 = 0.85;
const RECEIVE_BUFFER_BYTES: usize = 2048;
const SPEAKING_FLOOR_DB: f32 = -50.0;

pub(crate) type SharedTransform = Arc<Mutex<Box<dyn FrameTransform>>>;

pub(crate) struct Media {
	transport: Arc<dyn Transport>,
	shared: Arc<Shared>,
	control: mpsc::UnboundedSender<Control>,
	transform: SharedTransform,
	audio_io: bool,
	capture: CaptureChain,
	playback: PlaybackChain,
	encoder: VoiceEncoder,
	suppressor: NoiseSuppressor,
	processor: Processor,
	gate: VoiceGate,
	mixer: Mixer,
	streams: HashMap<u32, RemoteStream>,
	video: VideoReceive,
	video_sequences: HashMap<u32, u16>,
	frame: Vec<f32>,
	sequence: u16,
	timestamp: u32,
	speaking: bool,
	silence_left: u8,
	next_playout: Instant,
	next_injection: Instant,
}

impl Media {
	pub fn new(
		transport: Arc<dyn Transport>,
		shared: Arc<Shared>,
		control: mpsc::UnboundedSender<Control>,
		transform: SharedTransform,
		audio_io: bool,
	) -> crate::error::Result<Self> {
		let now = Instant::now();
		shared
			.applied_bitrate
			.store(DEFAULT_BITRATE_BPS, Ordering::Relaxed);
		Ok(Self {
			video: VideoReceive::new(transport.clone(), shared.clone(), transform.clone()),
			transport,
			playback: PlaybackChain::new(shared.rings.clone()),
			shared,
			control,
			transform,
			audio_io,
			capture: CaptureChain::default(),
			encoder: VoiceEncoder::new()?,
			suppressor: NoiseSuppressor::new(),
			processor: Processor::default(),
			gate: VoiceGate::default(),
			mixer: Mixer::new(STEREO_FRAME_SAMPLES),
			streams: HashMap::new(),
			video_sequences: HashMap::new(),
			frame: vec![0.0; FRAME_SAMPLES],
			sequence: rand::random(),
			timestamp: rand::random(),
			speaking: false,
			silence_left: 0,
			next_playout: now,
			next_injection: now,
		})
	}

	pub async fn run(mut self) {
		let transport = self.transport.clone();
		let shared = self.shared.clone();
		let mut ticker = interval(TICK);
		ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
		let mut housekeeping = Instant::now();
		let mut buffer = [0u8; RECEIVE_BUFFER_BYTES];
		loop {
			tokio::select! {
				() = shared.video.outgoing.ready.notified() => self.send_video(),
				received = transport.receive(&mut buffer) => match received {
					Ok(Some(Inbound::Media { header, payload })) => self.on_packet(&header, payload, Instant::now()),
					Ok(Some(Inbound::KeyframeRequest { media_ssrc })) => self.on_keyframe_request(media_ssrc),
					Ok(None) => {}
					Err(error) => {
						warn!(%error, "voice udp receive failed");
						return;
					}
				},
				_ = ticker.tick() => {
					let now = Instant::now();
					self.follow_bitrate();
					self.capture_tick(now);
					self.playout_tick(now);
					self.shared.quiet_down(now);
					if now.duration_since(housekeeping) >= HOUSEKEEPING_INTERVAL {
						housekeeping = now;
						self.housekeeping(now);
					}
				}
			}
		}
	}

	fn follow_bitrate(&mut self) {
		let wanted = self.shared.bitrate.load(Ordering::Relaxed);
		if wanted == self.shared.applied_bitrate.load(Ordering::Relaxed) {
			return;
		}
		match self.encoder.set_bitrate(wanted) {
			Ok(()) => self.shared.applied_bitrate.store(wanted, Ordering::Relaxed),
			Err(error) => {
				warn!(%error, "opus bitrate was not applied");
				self.shared.bitrate.store(
					self.shared.applied_bitrate.load(Ordering::Relaxed),
					Ordering::Relaxed,
				);
			}
		}
	}

	fn capture_tick(&mut self, now: Instant) {
		if self.shared.restart_capture.swap(false, Ordering::Relaxed) {
			self.capture = CaptureChain::default();
		}
		if self.audio_io {
			if let Err(error) = self.capture.pull(&self.shared.rings) {
				warn!(%error, "capture resampling failed");
			}
		} else {
			self.inject(now);
		}
		let mut frame = std::mem::take(&mut self.frame);
		while self.capture.take_frame(&mut frame) {
			self.send_frame(&mut frame);
		}
		self.frame = frame;
	}

	fn inject(&mut self, now: Instant) {
		if now < self.next_injection {
			return;
		}
		if let Some(frame) = self.shared.manual_input.lock().pop_front() {
			self.capture.inject(&frame);
			self.next_injection =
				(self.next_injection + FRAME_DURATION).max(now.checked_sub(MAX_PLAYOUT_LAG).unwrap_or(now));
		}
	}

	fn send_frame(&mut self, frame: &mut [f32]) {
		let processing = self.shared.processing();
		let delay_ms = self.echo_delay_ms();
		let counters = &self.shared.counters;
		if self.processor.cancel_echo(processing, delay_ms, frame) {
			counters.echo_cancelled.fetch_add(1, Ordering::Relaxed);
		}
		let suppressor = self.suppressor.process(self.shared.noise_suppression(), frame);
		counters.denoised_by(suppressor);
		if self.processor.control_gain(processing, frame) {
			counters.gain_controlled.fetch_add(1, Ordering::Relaxed);
		}
		let gain = self.shared.input_volume.load();
		for sample in frame.iter_mut() {
			*sample *= gain;
		}
		let input_level = meter_level(rms(frame));
		let previous = self.shared.input_level.load();
		self.shared
			.input_level
			.store(input_level.max(previous * METER_DECAY));
		let level_db = to_db(rms(frame));
		self.shared
			.input_db
			.store(decayed_db(level_db, self.shared.input_db.load()));
		if self.should_transmit(level_db) {
			self.silence_left = SILENCE_FRAMES_BEFORE_PAUSE;
			if !self.speaking {
				self.speaking = true;
				let _ = self.control.send(Control::Speaking(true));
			}
			match self.encoder.encode(frame) {
				Ok(packet) if packet.len() > MAX_DTX_PACKET_BYTES => {
					let packet = packet.to_vec();
					self.send_audio(&packet);
				}
				Ok(_) => {}
				Err(error) => warn!(%error, "opus encode failed"),
			}
		} else if self.speaking {
			if self.silence_left > 0 {
				self.silence_left -= 1;
				self.send_payload(&SILENCE_FRAME);
			}
			if self.silence_left == 0 {
				self.speaking = false;
				let _ = self.control.send(Control::Speaking(false));
			}
		}
		self.timestamp = self.timestamp.wrapping_add(FRAME_SAMPLES as u32);
	}

	fn echo_delay_ms(&self) -> i32 {
		if !self.audio_io {
			return 0;
		}
		i32::try_from(self.shared.rings.echo_delay().as_millis()).unwrap_or(i32::MAX)
	}

	fn should_transmit(&mut self, level_db: f32) -> bool {
		if self.shared.muted.load(Ordering::Relaxed) || self.shared.deafened.load(Ordering::Relaxed) {
			self.gate.reset();
			return false;
		}
		match self.shared.input_mode() {
			InputMode::VoiceActivity => self.gate.update(level_db, self.shared.vad_threshold_db.load()),
			InputMode::PushToTalk => self.shared.transmitting.load(Ordering::Relaxed),
		}
	}

	fn send_audio(&mut self, opus: &[u8]) {
		let Some(payload) = self.transform.lock().encrypt(Codec::Opus, opus) else {
			return;
		};
		self.send_payload(&payload);
	}

	fn send_payload(&mut self, payload: &[u8]) {
		let header = rtp::write_header(
			OPUS_PAYLOAD_TYPE,
			self.sequence,
			self.timestamp,
			self.transport.ssrc(),
		);
		self.sequence = self.sequence.wrapping_add(1);
		if self.transport.send(&header, payload) {
			self.shared.counters.sent.fetch_add(1, Ordering::Relaxed);
		}
	}

	fn send_video(&mut self) {
		let outgoing = &self.shared.video.outgoing;
		while let Some(unit) = outgoing.pop() {
			let Some(sealed) = self.transform.lock().encrypt(Codec::H264, &unit.data) else {
				outgoing.dropped.fetch_add(1, Ordering::Relaxed);
				continue;
			};
			let extensions = packetize::extensions(&unit.rid);
			let sequence = self.video_sequences.entry(unit.ssrc).or_insert_with(rand::random);
			for payload in packetize::payload_frame(&sealed) {
				let header = packetize::header(
					H264_PAYLOAD_TYPE,
					payload.marker,
					*sequence,
					unit.timestamp,
					unit.ssrc,
					extensions.len(),
				);
				*sequence = sequence.wrapping_add(1);
				let mut body = Vec::with_capacity(extensions.len() + payload.data.len());
				body.extend_from_slice(&extensions);
				body.extend_from_slice(&payload.data);
				self.transport.send(&header, &body);
			}
			outgoing.sent.fetch_add(1, Ordering::Relaxed);
		}
	}

	fn on_keyframe_request(&self, media_ssrc: u32) {
		if let Some(layer) = self.shared.video.outgoing.layer(media_ssrc) {
			layer.request_keyframe();
		}
	}

	fn on_packet(&mut self, header: &rtp::Header, payload: Vec<u8>, now: Instant) {
		if header.payload_type == H264_PAYLOAD_TYPE && header.ssrc != self.transport.ssrc() {
			self.video.on_packet(header, &payload, now);
			return;
		}
		if header.payload_type != OPUS_PAYLOAD_TYPE || header.ssrc == self.transport.ssrc() {
			return;
		}
		let payload = if payload == SILENCE_FRAME {
			payload
		} else {
			let Some(decrypted) = self.transform.lock().decrypt(Codec::Opus, header.ssrc, &payload) else {
				return;
			};
			decrypted
		};
		let stream = match self.streams.entry(header.ssrc) {
			Entry::Occupied(entry) => entry.into_mut(),
			Entry::Vacant(entry) => match RemoteStream::new(now) {
				Ok(stream) => entry.insert(stream),
				Err(error) => {
					warn!(%error, "failed to create an opus decoder");
					return;
				}
			},
		};
		self.shared.counters.received.fetch_add(1, Ordering::Relaxed);
		if stream.push(header.sequence, header.timestamp, payload, now) == Push::Late {
			self.shared.counters.late.fetch_add(1, Ordering::Relaxed);
		}
	}

	fn playout_tick(&mut self, now: Instant) {
		if self.audio_io && self.playback.has_device() {
			for _ in 0..MAX_FRAMES_PER_TICK {
				if !self.playback.wants_frame() {
					break;
				}
				self.mix_frame(now);
				if let Err(error) = self.playback.push(self.mixer.output()) {
					warn!(%error, "playback resampling failed");
					break;
				}
			}
			return;
		}
		if self.next_playout + MAX_PLAYOUT_LAG < now {
			self.next_playout = now;
		}
		while self.next_playout <= now {
			let sources = self.mix_frame(now);
			self.next_playout += FRAME_DURATION;
			if !self.audio_io {
				self.tap_output(sources);
			}
		}
	}

	fn tap_output(&self, sources: Vec<u32>) {
		let pcm = self
			.mixer
			.output()
			.iter()
			.map(|sample| (sample * f32::from(i16::MAX)) as i16)
			.collect();
		self.shared.push_output(MixedFrame { sources, pcm });
	}

	fn mix_frame(&mut self, now: Instant) -> Vec<u32> {
		self.mixer.clear();
		let deafened = self.shared.deafened.load(Ordering::Relaxed);
		let users = self.shared.ssrc_users.lock();
		let volumes = self.shared.user_volumes.lock();
		let mut sources = Vec::new();
		let mut talking = Vec::new();
		for (ssrc, stream) in &mut self.streams {
			let frame = stream.next_frame();
			if frame == Frame::Silence {
				continue;
			}
			sources.push(*ssrc);
			if frame == Frame::Concealed {
				self.shared.counters.lost.fetch_add(1, Ordering::Relaxed);
			}
			let user = users.get(ssrc).copied();
			if frame == Frame::Decoded
				&& to_db(rms(stream.pcm())) > SPEAKING_FLOOR_DB
				&& let Some(user) = user
			{
				talking.push(user);
			}
			if !deafened {
				let gain = user.and_then(|user| volumes.get(&user)).copied().unwrap_or(1.0);
				self.mixer.add(stream.pcm(), gain);
			}
		}
		drop((users, volumes));
		for user in talking {
			self.shared.heard(user, now);
		}
		let master = if deafened {
			0.0
		} else {
			self.shared.output_volume.load()
		};
		let level = meter_level(rms(self.mixer.finish(master)));
		self.processor
			.render(self.shared.processing(), self.mixer.output());
		let previous = self.shared.output_level.load();
		self.shared.output_level.store(level.max(previous * METER_DECAY));
		sources.sort_unstable();
		sources
	}

	fn housekeeping(&mut self, now: Instant) {
		self.video.housekeeping(now);
		self.streams.retain(|ssrc, stream| {
			let keep = stream.idle_since(now) < STREAM_TTL;
			if !keep {
				debug!(ssrc, "dropping idle voice stream");
			}
			keep
		});
		let jitter = self
			.streams
			.values()
			.map(RemoteStream::jitter_ms)
			.fold(0.0_f32, f32::max);
		self.shared.counters.jitter_ms.store(jitter);
	}
}
