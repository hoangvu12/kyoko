use std::collections::HashMap;
use std::time::Duration;

use dave::nal;
use parking_lot::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};

use super::VoiceControl;
use crate::config::CameraPlan;
use crate::fixtures::{CURRENT_USER_ID, Fixtures};

pub const CAMERA_H264: &[u8] = include_bytes!("../../fixtures/video/camera.h264");
pub const CAMERA_WIDTH: u32 = 640;
pub const CAMERA_HEIGHT: u32 = 360;
pub const CAMERA_FPS: u32 = 15;
pub const H264_PAYLOAD_TYPE: u8 = 101;

const CLOCK_HZ: u32 = 90_000;
const CAMERA_FIRST_SSRC: u32 = 7_001;
const CAMERA_SSRC_STRIDE: u32 = 4;
const ACCESS_UNIT_DELIMITER: u8 = 9;
const MAX_PAYLOAD_BYTES: usize = 1200;
const FU_A: u8 = 28;
const FU_INDICATOR_NRI_MASK: u8 = 0xe0;
const FU_START: u8 = 0x80;
const FU_END: u8 = 0x40;
const FU_HEADER_BYTES: usize = 2;

pub fn camera_frames() -> Vec<Vec<u8>> {
	clip_frames(CAMERA_H264)
}

pub fn clip_frames(clip: &[u8]) -> Vec<Vec<u8>> {
	let mut frames: Vec<Vec<&[u8]>> = Vec::new();
	for unit in nal::units(clip) {
		if nal::unit_type(unit[0]) == ACCESS_UNIT_DELIMITER {
			frames.push(Vec::new());
		} else if let Some(frame) = frames.last_mut() {
			frame.push(unit);
		} else {
			frames.push(vec![unit]);
		}
	}
	frames
		.into_iter()
		.filter(|units| !units.is_empty())
		.map(nal::annex_b)
		.collect()
}

pub fn payloads(frame: &[u8]) -> Vec<Vec<u8>> {
	let mut payloads = Vec::new();
	for unit in nal::units(frame) {
		if unit.len() <= MAX_PAYLOAD_BYTES {
			payloads.push(unit.to_vec());
			continue;
		}
		let indicator = (unit[0] & FU_INDICATOR_NRI_MASK) | FU_A;
		let kind = nal::unit_type(unit[0]);
		let chunks: Vec<&[u8]> = unit[nal::HEADER_BYTES..]
			.chunks(MAX_PAYLOAD_BYTES - FU_HEADER_BYTES)
			.collect();
		let last = chunks.len() - 1;
		for (index, chunk) in chunks.into_iter().enumerate() {
			let mut header = kind;
			if index == 0 {
				header |= FU_START;
			}
			if index == last {
				header |= FU_END;
			}
			let mut payload = vec![indicator, header];
			payload.extend_from_slice(chunk);
			payloads.push(payload);
		}
	}
	payloads
}

pub struct ScriptedCamera {
	control: VoiceControl,
	channel_id: u64,
	user_id: u64,
	ssrc: u32,
	frames: Vec<Vec<u8>>,
	fps: u32,
	next: usize,
	sequence: u16,
	timestamp: u32,
	lose_next: bool,
}

impl ScriptedCamera {
	pub fn new(control: VoiceControl, channel_id: u64, user_id: u64, ssrc: u32) -> Self {
		Self::with_clip(control, channel_id, user_id, ssrc, camera_frames(), CAMERA_FPS)
	}

	pub fn with_clip(
		control: VoiceControl,
		channel_id: u64,
		user_id: u64,
		ssrc: u32,
		frames: Vec<Vec<u8>>,
		fps: u32,
	) -> Self {
		Self {
			control,
			channel_id,
			user_id,
			ssrc,
			frames,
			fps: fps.max(1),
			next: 0,
			sequence: 0,
			timestamp: 0,
			lose_next: false,
		}
	}

	pub fn frames(&self) -> usize {
		self.frames.len()
	}

	pub fn lose_next_packet(&mut self) {
		self.lose_next = true;
	}

	pub fn send_frame(&mut self) -> usize {
		if self.frames.is_empty() {
			return 0;
		}
		let frame = &self.frames[self.next % self.frames.len()];
		self.next += 1;
		let Some(sealed) = self.control.seal_video(self.channel_id, self.user_id, frame) else {
			return 0;
		};
		let payloads = payloads(&sealed);
		let last = payloads.len().saturating_sub(1);
		let mut sent = 0;
		for (index, payload) in payloads.into_iter().enumerate() {
			let sequence = self.sequence;
			self.sequence = self.sequence.wrapping_add(1);
			if std::mem::take(&mut self.lose_next) {
				continue;
			}
			let header = rtp_header(index == last, sequence, self.timestamp, self.ssrc);
			if self.control.send_rtp(self.channel_id, &header, &payload) > 0 {
				sent += 1;
			}
		}
		self.timestamp = self.timestamp.wrapping_add(CLOCK_HZ / self.fps);
		sent
	}

	pub fn play(mut self) -> JoinHandle<()> {
		let mut shutdown = self.control.shutdown();
		tokio::spawn(async move {
			let mut ticker = interval(Duration::from_secs(1) / self.fps);
			ticker.set_missed_tick_behavior(MissedTickBehavior::Burst);
			loop {
				tokio::select! {
					_ = ticker.tick() => {
						self.send_frame();
					}
					_ = shutdown.changed() => return,
				}
			}
		})
	}
}

fn rtp_header(marker: bool, sequence: u16, timestamp: u32, ssrc: u32) -> Vec<u8> {
	let mut header = vec![
		super::RTP_VERSION,
		H264_PAYLOAD_TYPE | if marker { 0x80 } else { 0 },
	];
	header.extend_from_slice(&sequence.to_be_bytes());
	header.extend_from_slice(&timestamp.to_be_bytes());
	header.extend_from_slice(&ssrc.to_be_bytes());
	header
}

pub struct Cameras {
	tiles: usize,
	fps: u32,
	frames: Vec<Vec<u8>>,
	candidates: HashMap<u64, Vec<u64>>,
	playing: Mutex<HashMap<u64, Vec<Playing>>>,
}

struct Playing {
	user_id: u64,
	ssrc: u32,
	task: JoinHandle<()>,
}

impl Cameras {
	pub fn new(plan: &CameraPlan, fixtures: &Fixtures) -> Self {
		let frames = plan
			.clip
			.as_ref()
			.and_then(|path| match std::fs::read(path) {
				Ok(clip) => Some(clip_frames(&clip)),
				Err(error) => {
					tracing::warn!(%error, "the camera clip was not read; playing the fixture");
					None
				}
			})
			.filter(|frames| !frames.is_empty())
			.unwrap_or_else(camera_frames);
		let mut candidates = HashMap::new();
		for guild in &fixtures.guilds {
			let members: Vec<u64> = guild
				.members
				.iter()
				.filter_map(|member| member.user.as_ref().map(|user| user.id.0))
				.filter(|user_id| *user_id != CURRENT_USER_ID)
				.collect();
			for state in &guild.voice_states {
				let Some(channel_id) = state.channel_id else {
					continue;
				};
				let seated: &mut Vec<u64> = candidates.entry(channel_id.0).or_default();
				if state.user_id.0 != CURRENT_USER_ID && !seated.contains(&state.user_id.0) {
					seated.push(state.user_id.0);
				}
			}
			for seated in candidates.values_mut() {
				for member in &members {
					if !seated.contains(member) {
						seated.push(*member);
					}
				}
			}
		}
		Self {
			tiles: plan.tiles,
			fps: plan.fps,
			frames,
			candidates,
			playing: Mutex::new(HashMap::new()),
		}
	}

	pub(super) fn start(&self, control: &VoiceControl, channel_id: u64) {
		let mut playing = self.playing.lock();
		let cameras = playing.entry(channel_id).or_insert_with(|| {
			let users = self.candidates.get(&channel_id).cloned().unwrap_or_default();
			users
				.into_iter()
				.take(self.tiles)
				.zip(0u32..)
				.map(|(user_id, index)| {
					let ssrc = CAMERA_FIRST_SSRC + index * CAMERA_SSRC_STRIDE;
					let camera = ScriptedCamera::with_clip(
						control.clone(),
						channel_id,
						user_id,
						ssrc,
						self.frames.clone(),
						self.fps,
					);
					Playing {
						user_id,
						ssrc,
						task: camera.play(),
					}
				})
				.collect()
		});
		for camera in cameras.iter() {
			control.connect_as(camera.user_id);
			control.video_as(camera.user_id, camera.ssrc - 1, camera.ssrc);
		}
	}
}

impl Drop for Cameras {
	fn drop(&mut self) {
		for cameras in self.playing.get_mut().values() {
			for camera in cameras {
				camera.task.abort();
			}
		}
	}
}
