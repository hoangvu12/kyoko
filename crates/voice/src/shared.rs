use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use parking_lot::Mutex;
use protocol::Snowflake;

use crate::atomic::AtomicF32;
use crate::audio::{AudioEngine, Rings};
use crate::codec::DEFAULT_BITRATE_BPS;
use crate::event::{EventHub, VoiceEvent};
use crate::gate::LEVEL_FLOOR_DB;
use crate::talk::Talkers;
use crate::video::hub::VideoHub;

const NO_PING: u32 = u32::MAX;
const DEFAULT_VAD_THRESHOLD_DB: f32 = -50.0;
const MAX_MANUAL_OUTPUT_FRAMES: usize = 3_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
	#[default]
	VoiceActivity,
	PushToTalk,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VoiceStats {
	pub ping_ms: Option<u32>,
	pub packets_sent: u64,
	pub packets_received: u64,
	pub packets_late: u64,
	pub packets_lost: u64,
	pub jitter_ms: f32,
	pub frames_denoised: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioDevicesInUse {
	pub input: Option<String>,
	pub output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedFrame {
	pub sources: Vec<u32>,
	pub pcm: Vec<i16>,
}

pub(crate) enum Control {
	Speaking(bool),
	VideoWants,
}

pub(crate) struct Counters {
	pub sent: AtomicU64,
	pub received: AtomicU64,
	pub late: AtomicU64,
	pub lost: AtomicU64,
	pub denoised: AtomicU64,
	pub jitter_ms: AtomicF32,
	ping_ms: AtomicU32,
}

impl Counters {
	fn new() -> Self {
		Self {
			sent: AtomicU64::new(0),
			received: AtomicU64::new(0),
			late: AtomicU64::new(0),
			lost: AtomicU64::new(0),
			denoised: AtomicU64::new(0),
			jitter_ms: AtomicF32::new(0.0),
			ping_ms: AtomicU32::new(NO_PING),
		}
	}

	pub fn set_ping(&self, ping_ms: u32) {
		self.ping_ms.store(ping_ms, Ordering::Relaxed);
	}

	pub fn snapshot(&self) -> VoiceStats {
		VoiceStats {
			ping_ms: Some(self.ping_ms.load(Ordering::Relaxed)).filter(|ping| *ping != NO_PING),
			packets_sent: self.sent.load(Ordering::Relaxed),
			packets_received: self.received.load(Ordering::Relaxed),
			packets_late: self.late.load(Ordering::Relaxed),
			packets_lost: self.lost.load(Ordering::Relaxed),
			jitter_ms: self.jitter_ms.load(),
			frames_denoised: self.denoised.load(Ordering::Relaxed),
		}
	}
}

pub(crate) struct Shared {
	pub muted: AtomicBool,
	pub deafened: AtomicBool,
	pub transmitting: AtomicBool,
	pub noise_suppression: AtomicBool,
	input_mode: AtomicU8,
	pub vad_threshold_db: AtomicF32,
	pub input_volume: AtomicF32,
	pub output_volume: AtomicF32,
	pub input_level: AtomicF32,
	pub input_db: AtomicF32,
	pub output_level: AtomicF32,
	pub devices: Mutex<AudioDevicesInUse>,
	pub audio_restarts: AtomicU64,
	pub restart_capture: AtomicBool,
	pub user_volumes: Mutex<HashMap<Snowflake, f32>>,
	pub ssrc_users: Mutex<HashMap<u32, Snowflake>>,
	pub bitrate: AtomicU32,
	pub applied_bitrate: AtomicU32,
	talkers: Mutex<Talkers>,
	pub rings: Arc<Rings>,
	pub engine: Mutex<Option<AudioEngine>>,
	pub hub: EventHub,
	pub video: VideoHub,
	pub counters: Counters,
	pub manual_input: Mutex<VecDeque<Vec<f32>>>,
	manual_output: Mutex<Vec<MixedFrame>>,
}

impl Shared {
	pub fn new(rings: Arc<Rings>) -> Self {
		Self {
			muted: AtomicBool::new(false),
			deafened: AtomicBool::new(false),
			transmitting: AtomicBool::new(false),
			noise_suppression: AtomicBool::new(false),
			input_mode: AtomicU8::new(InputMode::VoiceActivity as u8),
			vad_threshold_db: AtomicF32::new(DEFAULT_VAD_THRESHOLD_DB),
			input_volume: AtomicF32::new(1.0),
			output_volume: AtomicF32::new(1.0),
			input_level: AtomicF32::new(0.0),
			input_db: AtomicF32::new(LEVEL_FLOOR_DB),
			output_level: AtomicF32::new(0.0),
			devices: Mutex::new(AudioDevicesInUse::default()),
			audio_restarts: AtomicU64::new(0),
			restart_capture: AtomicBool::new(false),
			user_volumes: Mutex::new(HashMap::new()),
			ssrc_users: Mutex::new(HashMap::new()),
			bitrate: AtomicU32::new(DEFAULT_BITRATE_BPS),
			applied_bitrate: AtomicU32::new(DEFAULT_BITRATE_BPS),
			talkers: Mutex::new(Talkers::default()),
			rings,
			engine: Mutex::new(None),
			hub: EventHub::default(),
			video: VideoHub::default(),
			counters: Counters::new(),
			manual_input: Mutex::new(VecDeque::new()),
			manual_output: Mutex::new(Vec::new()),
		}
	}

	pub fn input_mode(&self) -> InputMode {
		if self.input_mode.load(Ordering::Relaxed) == InputMode::PushToTalk as u8 {
			InputMode::PushToTalk
		} else {
			InputMode::VoiceActivity
		}
	}

	pub fn set_input_mode(&self, mode: InputMode) {
		self.input_mode.store(mode as u8, Ordering::Relaxed);
	}

	pub fn heard(&self, user_id: Snowflake, now: Instant) {
		let started = self.talkers.lock().heard(user_id, now);
		if started {
			self.speaking(user_id, true);
		}
	}

	pub fn announced(&self, user_id: Snowflake, now: Instant) {
		self.talkers.lock().heard(user_id, now);
		self.speaking(user_id, true);
	}

	pub fn hushed(&self, user_id: Snowflake) {
		let stopped = self.talkers.lock().hushed(user_id);
		if stopped {
			self.speaking(user_id, false);
		}
	}

	pub fn quiet_down(&self, now: Instant) {
		let quiet = self.talkers.lock().fallen_quiet(now);
		for user_id in quiet {
			self.speaking(user_id, false);
		}
	}

	pub fn hush_everyone(&self) {
		let quiet = self.talkers.lock().clear();
		for user_id in quiet {
			self.speaking(user_id, false);
		}
	}

	pub fn speaking(&self, user_id: Snowflake, speaking: bool) {
		self.hub.emit(VoiceEvent::Speaking { user_id, speaking });
	}

	pub fn push_output(&self, frame: MixedFrame) {
		let mut output = self.manual_output.lock();
		if output.len() >= MAX_MANUAL_OUTPUT_FRAMES {
			output.remove(0);
		}
		output.push(frame);
	}

	pub fn take_output(&self) -> Vec<MixedFrame> {
		std::mem::take(&mut *self.manual_output.lock())
	}
}
