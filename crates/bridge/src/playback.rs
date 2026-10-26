use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use audio::{Clip, Deck, Limits, Output, Sink};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use media::{MediaError, MediaService, Origin};
use parking_lot::Mutex;
use protocol::{Attachment, Message, MessageFlags, Snowflake};
use tokio::runtime::Handle;

pub const WAVEFORM_BARS: usize = 40;
pub const FLAT_BAR: f32 = 0.1;
pub const PROBE_LIMIT_BYTES: i64 = 16 * 1024 * 1024;
const MAX_DURATION_SECS: f64 = 24. * 60. * 60.;
const AUDIO_EXTENSIONS: [&str; 9] = ["mp3", "ogg", "oga", "opus", "wav", "flac", "m4a", "aac", "mka"];

type Hook = Arc<dyn Fn() + Send + Sync>;
type Refresher = Arc<dyn Fn(Origin) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioSpeed {
	#[default]
	Normal,
	Faster,
	Fastest,
}

impl AudioSpeed {
	pub fn factor(self) -> f32 {
		match self {
			Self::Normal => 1.,
			Self::Faster => 1.5,
			Self::Fastest => 2.,
		}
	}

	pub fn next(self) -> Self {
		match self {
			Self::Normal => Self::Faster,
			Self::Faster => Self::Fastest,
			Self::Fastest => Self::Normal,
		}
	}

	pub fn label(self) -> &'static str {
		match self {
			Self::Normal => "1x",
			Self::Faster => "1.5x",
			Self::Fastest => "2x",
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPhase {
	Idle,
	Loading,
	Playing,
	Paused,
	Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFailure {
	Unavailable,
	Unplayable,
	NoOutput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack {
	pub id: Snowflake,
	pub url: String,
	pub filename: String,
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub duration: Option<Duration>,
	pub size: Option<i64>,
}

impl AudioTrack {
	pub fn of(message: &Message, attachment: &Attachment) -> Option<Self> {
		let url = attachment
			.url
			.as_deref()
			.or(attachment.proxy_url.as_deref())
			.filter(|url| !url.is_empty())?;
		Some(Self {
			id: attachment.id,
			url: url.to_owned(),
			filename: attachment.filename.clone(),
			channel_id: message.channel_id,
			message_id: message.id,
			duration: attachment.duration_secs.and_then(seconds),
			size: attachment.size,
		})
	}

	fn extension(&self) -> Option<String> {
		self.filename
			.rsplit_once('.')
			.map(|(_, extension)| extension.to_ascii_lowercase())
	}

	fn origin(&self) -> Origin {
		Origin {
			channel_id: self.channel_id.0,
			message_id: self.message_id.0,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioState {
	pub phase: AudioPhase,
	pub position: Duration,
	pub duration: Option<Duration>,
	pub speed: AudioSpeed,
	pub muted: bool,
	pub failure: Option<AudioFailure>,
}

impl AudioState {
	pub fn progress(&self) -> f32 {
		match self.duration {
			Some(duration) if !duration.is_zero() => {
				(self.position.as_secs_f64() / duration.as_secs_f64()).clamp(0., 1.) as f32
			}
			_ => 0.,
		}
	}

	pub fn is_playing(&self) -> bool {
		self.phase == AudioPhase::Playing
	}

	pub fn started(&self) -> bool {
		matches!(
			self.phase,
			AudioPhase::Playing | AudioPhase::Paused | AudioPhase::Loading
		)
	}
}

#[derive(Debug, Clone)]
struct Memory {
	channel_id: Snowflake,
	phase: AudioPhase,
	position: Duration,
	duration: Option<Duration>,
	speed: AudioSpeed,
	muted: bool,
	failure: Option<AudioFailure>,
	probing: bool,
}

impl Memory {
	fn of(track: &AudioTrack) -> Self {
		Self {
			channel_id: track.channel_id,
			phase: AudioPhase::Idle,
			position: Duration::ZERO,
			duration: track.duration,
			speed: AudioSpeed::Normal,
			muted: false,
			failure: None,
			probing: false,
		}
	}

	fn state(&self) -> AudioState {
		AudioState {
			phase: self.phase,
			position: self.position,
			duration: self.duration,
			speed: self.speed,
			muted: self.muted,
			failure: self.failure,
		}
	}
}

#[derive(Default)]
struct Board {
	active: Option<Snowflake>,
	tracks: HashMap<Snowflake, Memory>,
	generation: u64,
}

#[derive(Clone)]
pub struct AudioPlayer {
	inner: Arc<Inner>,
}

struct Inner {
	service: MediaService,
	runtime: Handle,
	output: Mutex<Output>,
	limits: Limits,
	board: Mutex<Board>,
	waker: Mutex<Option<Hook>>,
	refresher: Mutex<Option<Refresher>>,
}

impl AudioPlayer {
	pub fn new(service: MediaService, runtime: Handle, sink: Sink) -> Self {
		let deck = Deck::new();
		let inner = Arc::new(Inner {
			service,
			runtime,
			output: Mutex::new(Output::new(sink, deck.clone())),
			limits: Limits::default(),
			board: Mutex::new(Board::default()),
			waker: Mutex::new(None),
			refresher: Mutex::new(None),
		});
		let weak = Arc::downgrade(&inner);
		deck.set_notify(move || {
			if let Some(inner) = weak.upgrade() {
				inner.wake();
			}
		});
		Self { inner }
	}

	pub fn sink(&self) -> Sink {
		self.inner.output().sink()
	}

	pub fn set_sink(&self, sink: Sink) {
		self.stop();
		let mut output = self.inner.output.lock();
		if output.sink() != sink {
			*output = Output::new(sink, output.deck().clone());
		}
	}

	pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
		*self.inner.waker.lock() = Some(Arc::new(waker));
	}

	pub fn set_refresher(&self, refresher: impl Fn(Origin) + Send + Sync + 'static) {
		*self.inner.refresher.lock() = Some(Arc::new(refresher));
	}

	pub fn state(&self, track: &AudioTrack) -> AudioState {
		let mut board = self.inner.board.lock();
		let active = board.active == Some(track.id);
		let Some(memory) = board.tracks.get_mut(&track.id) else {
			return Memory::of(track).state();
		};
		if active {
			self.inner.follow_deck(track.id, memory);
		}
		memory.state()
	}

	pub fn playing(&self) -> Option<Snowflake> {
		let board = self.inner.board.lock();
		board.active.filter(|id| {
			board
				.tracks
				.get(id)
				.is_some_and(|memory| memory.phase == AudioPhase::Playing)
		})
	}

	pub fn toggle(&self, track: &AudioTrack) {
		let mut board = self.inner.board.lock();
		let active = board.active == Some(track.id);
		let loaded = self.inner.output().deck().loaded() == Some(track.id.0);
		let phase = board
			.tracks
			.entry(track.id)
			.or_insert_with(|| Memory::of(track))
			.phase;
		match (active, phase) {
			(true, AudioPhase::Playing) => {
				self.inner.output().deck().pause();
				self.inner.output().idle();
				if let Some(memory) = board.tracks.get_mut(&track.id) {
					self.inner.follow_deck(track.id, memory);
					memory.phase = AudioPhase::Paused;
				}
			}
			(true, AudioPhase::Paused | AudioPhase::Idle) if loaded => {
				if let Some(memory) = board.tracks.get_mut(&track.id) {
					self.inner.output().deck().play();
					self.inner.output().run();
					memory.phase = AudioPhase::Playing;
					memory.failure = None;
				}
			}
			(true, AudioPhase::Loading) => {}
			_ => self.start(&mut board, track),
		}
		drop(board);
		self.inner.wake();
	}

	pub fn seek(&self, track: &AudioTrack, fraction: f32) {
		let mut board = self.inner.board.lock();
		let active = board.active == Some(track.id);
		let loaded = self.inner.output().deck().loaded() == Some(track.id.0);
		let memory = board.tracks.entry(track.id).or_insert_with(|| Memory::of(track));
		let Some(duration) = memory.duration else {
			return;
		};
		let fraction = if fraction.is_finite() {
			fraction.clamp(0., 1.)
		} else {
			0.
		};
		memory.position = duration.mul_f32(fraction);
		if active && loaded {
			self.inner.output().deck().seek(memory.position);
		}
		if memory.phase == AudioPhase::Idle {
			memory.phase = AudioPhase::Paused;
		}
		drop(board);
		self.inner.wake();
	}

	pub fn cycle_speed(&self, track: &AudioTrack) -> AudioSpeed {
		let speed = self.change(track, |memory, deck| {
			memory.speed = memory.speed.next();
			if let Some(deck) = deck {
				deck.set_speed(memory.speed.factor());
			}
			memory.speed
		});
		self.inner.wake();
		speed
	}

	pub fn toggle_mute(&self, track: &AudioTrack) -> bool {
		let muted = self.change(track, |memory, deck| {
			memory.muted = !memory.muted;
			if let Some(deck) = deck {
				deck.set_muted(memory.muted);
			}
			memory.muted
		});
		self.inner.wake();
		muted
	}

	pub fn probe(&self, track: &AudioTrack) {
		if track.duration.is_some() || track.size.is_some_and(|size| size > PROBE_LIMIT_BYTES) {
			return;
		}
		{
			let mut board = self.inner.board.lock();
			let memory = board.tracks.entry(track.id).or_insert_with(|| Memory::of(track));
			if memory.probing || memory.duration.is_some() {
				return;
			}
			memory.probing = true;
		}
		let inner = self.inner.clone();
		let track = track.clone();
		self.inner.runtime.spawn(async move {
			let Ok(bytes) = inner.service.audio(&track.url).await else {
				return;
			};
			let hint = track.extension();
			let probed = tokio::task::spawn_blocking(move || audio::duration(bytes, hint.as_deref()))
				.await
				.ok()
				.flatten();
			if let Some(duration) = probed {
				let mut board = inner.board.lock();
				if let Some(memory) = board.tracks.get_mut(&track.id)
					&& memory.duration.is_none()
				{
					memory.duration = Some(duration);
				}
				drop(board);
				inner.wake();
			}
		});
	}

	pub fn left(&self, channel_id: Snowflake) {
		let playing_there = {
			let board = self.inner.board.lock();
			board
				.active
				.and_then(|id| board.tracks.get(&id))
				.is_some_and(|memory| memory.channel_id == channel_id)
		};
		if playing_there {
			self.stop();
		}
	}

	pub fn stop(&self) {
		let mut board = self.inner.board.lock();
		self.inner.output().deck().unload();
		self.inner.output().idle();
		board.active = None;
		board.tracks.clear();
		board.generation += 1;
		drop(board);
		self.inner.wake();
	}

	pub fn pump(&self, played: Duration) {
		self.inner.output().pump(played);
	}

	fn change<T>(&self, track: &AudioTrack, edit: impl FnOnce(&mut Memory, Option<&Deck>) -> T) -> T {
		let mut board = self.inner.board.lock();
		let active = board.active == Some(track.id);
		let output = self.inner.output();
		let deck = output.deck();
		let loaded = active && deck.loaded() == Some(track.id.0);
		let memory = board.tracks.entry(track.id).or_insert_with(|| Memory::of(track));
		edit(memory, loaded.then_some(deck))
	}

	fn start(&self, board: &mut Board, track: &AudioTrack) {
		if let Some(previous) = board.active.take()
			&& let Some(memory) = board.tracks.get_mut(&previous)
		{
			self.inner.follow_deck(previous, memory);
			memory.phase = if memory.position.is_zero() {
				AudioPhase::Idle
			} else {
				AudioPhase::Paused
			};
		}
		self.inner.output().deck().unload();
		self.inner.output().idle();
		board.generation += 1;
		board.active = Some(track.id);
		let generation = board.generation;
		if let Some(memory) = board.tracks.get_mut(&track.id) {
			memory.phase = AudioPhase::Loading;
			memory.failure = None;
		}
		let inner = self.inner.clone();
		let track = track.clone();
		self.inner
			.runtime
			.spawn(async move { inner.load(track, generation).await });
	}
}

impl Inner {
	fn output(&self) -> Output {
		self.output.lock().clone()
	}

	fn wake(&self) {
		let waker = self.waker.lock().clone();
		if let Some(waker) = waker {
			waker();
		}
	}

	fn follow_deck(&self, id: Snowflake, memory: &mut Memory) {
		let output = self.output();
		let deck = output.deck();
		if let Some(status) = deck.status().filter(|status| status.id == id.0) {
			memory.duration = Some(status.duration);
			if status.ended {
				memory.position = Duration::ZERO;
				if memory.phase == AudioPhase::Playing {
					memory.phase = AudioPhase::Idle;
					deck.seek(Duration::ZERO);
					output.idle();
				}
			} else {
				memory.position = status.position;
			}
		}
		if memory.phase == AudioPhase::Playing && output.failure().is_some() {
			memory.phase = AudioPhase::Failed;
			memory.failure = Some(AudioFailure::NoOutput);
			deck.pause();
		}
	}

	async fn load(self: Arc<Self>, track: AudioTrack, generation: u64) {
		let clip = match self.service.audio(&track.url).await {
			Ok(bytes) => {
				let limits = self.limits.clone();
				let hint = track.extension();
				tokio::task::spawn_blocking(move || audio::decode(bytes, hint.as_deref(), &limits))
					.await
					.map_err(|_| AudioFailure::Unplayable)
					.and_then(|decoded| decoded.map_err(|_| AudioFailure::Unplayable))
			}
			Err(error) => {
				self.refresh_if_gone(&track, &error);
				Err(AudioFailure::Unavailable)
			}
		};
		self.install(&track, generation, clip);
		self.wake();
	}

	fn refresh_if_gone(&self, track: &AudioTrack, error: &MediaError) {
		if !error.is_gone() {
			return;
		}
		let refresher = self.refresher.lock().clone();
		if let Some(refresher) = refresher {
			refresher(track.origin());
		}
	}

	fn install(&self, track: &AudioTrack, generation: u64, clip: Result<Clip, AudioFailure>) {
		let mut board = self.board.lock();
		if board.generation != generation || board.active != Some(track.id) {
			return;
		}
		let Some(memory) = board.tracks.get_mut(&track.id) else {
			return;
		};
		match clip {
			Ok(clip) => {
				let duration = clip.duration();
				if memory.position >= duration {
					memory.position = Duration::ZERO;
				}
				memory.duration = Some(duration);
				let output = self.output();
				let deck = output.deck();
				deck.load(track.id.0, Arc::new(clip), memory.position, memory.speed.factor());
				deck.set_muted(memory.muted);
				deck.play();
				output.run();
				memory.phase = AudioPhase::Playing;
			}
			Err(failure) => {
				memory.phase = AudioPhase::Failed;
				memory.failure = Some(failure);
				board.active = None;
			}
		}
	}
}

pub fn is_voice_message(message: &Message, attachment: &Attachment) -> bool {
	message.flags.contains(MessageFlags::IS_VOICE_MESSAGE)
		|| (attachment.waveform.is_some() && attachment.duration_secs.is_some())
}

pub fn is_audio(attachment: &Attachment) -> bool {
	match attachment.content_type.as_deref() {
		Some(kind) if kind.starts_with("audio/") => true,
		Some(_) => false,
		None => attachment
			.filename
			.rsplit_once('.')
			.is_some_and(|(_, extension)| {
				AUDIO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
			}),
	}
}

pub fn waveform(attachment: &Attachment, bars: usize) -> Vec<f32> {
	let samples = attachment
		.waveform
		.as_deref()
		.map(str::trim)
		.and_then(|encoded| {
			STANDARD
				.decode(encoded)
				.or_else(|_| STANDARD_NO_PAD.decode(encoded.trim_end_matches('=')))
				.ok()
		})
		.unwrap_or_default();
	if samples.is_empty() || bars == 0 {
		return vec![FLAT_BAR; bars];
	}
	(0..bars)
		.map(|bar| {
			let start = bar * samples.len() / bars;
			let end = ((bar + 1) * samples.len() / bars)
				.max(start + 1)
				.min(samples.len());
			let bucket = &samples[start.min(samples.len() - 1)..end];
			let peak = bucket.iter().copied().max().unwrap_or(0);
			(f32::from(peak) / 255.).max(FLAT_BAR)
		})
		.collect()
}

pub fn audio_clock(duration: Duration) -> String {
	let total = duration.as_secs();
	let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
	if hours > 0 {
		format!("{hours}:{minutes:02}:{seconds:02}")
	} else {
		format!("{minutes}:{seconds:02}")
	}
}

fn seconds(value: f64) -> Option<Duration> {
	(value.is_finite() && (0. ..=MAX_DURATION_SECS).contains(&value)).then(|| Duration::from_secs_f64(value))
}
