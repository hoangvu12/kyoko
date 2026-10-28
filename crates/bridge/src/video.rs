use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use audio::{Deck, Feed, Output, Sink};
use media::{MediaService, Origin, Rank, Ticket, VideoFile, VideoLimits};
use parking_lot::Mutex;
use protocol::{Attachment, Embed, EmbedKind, EmbedMedia, Message, Snowflake};
use tokio::runtime::Handle;
use url::Url;
use video::{ByteSource, FileReader, Packet, Picture, VideoFrame};

pub const VIDEO_MAX_FPS: u32 = 30;
pub const DEFAULT_VIDEO_VIEW: (u32, u32) = (640, 360);
const LIVE_SESSIONS: usize = 6;
const QUEUED_FRAMES: usize = 4;
const MAX_QUEUED_FRAMES: usize = 30;
const SOUND_AHEAD: Duration = Duration::from_millis(500);
const SOUND_FLOOR: Duration = Duration::from_millis(150);
const READS_PER_TURN: usize = 24;
const IDLE_WAIT: Duration = Duration::from_millis(250);
const LONGEST_WAIT: Duration = Duration::from_millis(15);
const SHORTEST_WAIT: Duration = Duration::from_millis(1);
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "webm", "mov", "m4v", "mkv", "avi"];
const GIFV_MARK: &str = "gifv";
const PLAYER_HOSTS: [&str; 9] = [
	"youtube.com",
	"www.youtube.com",
	"m.youtube.com",
	"youtu.be",
	"player.vimeo.com",
	"player.twitch.tv",
	"clips.twitch.tv",
	"open.spotify.com",
	"w.soundcloud.com",
];

type Hook = Arc<dyn Fn() + Send + Sync>;
type Refresher = Arc<dyn Fn(Origin) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoClip {
	pub key: String,
	pub url: String,
	pub name: String,
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub gifv: bool,
	pub size: Option<(u32, u32)>,
	pub duration: Option<Duration>,
	pub attachment: bool,
}

impl VideoClip {
	pub fn attachment(message: &Message, attachment: &Attachment) -> Option<Self> {
		if !crate::is_video(attachment) {
			return None;
		}
		let url = attachment
			.url
			.as_deref()
			.or(attachment.proxy_url.as_deref())
			.filter(|url| !url.is_empty())?;
		Some(Self {
			key: format!("attachment-{}", attachment.id),
			url: url.to_owned(),
			name: attachment.filename.clone(),
			channel_id: message.channel_id,
			message_id: message.id,
			gifv: false,
			size: size_of(attachment.width, attachment.height),
			duration: attachment
				.duration_secs
				.filter(|seconds| seconds.is_finite() && *seconds > 0.)
				.map(Duration::from_secs_f64),
			attachment: true,
		})
	}

	pub fn embed(message: &Message, index: usize, embed: &Embed) -> Option<Self> {
		let video = embed.video.as_ref()?;
		let url = source(video)?;
		let gifv = embed.kind == Some(EmbedKind::Gifv);
		Some(Self {
			key: format!("embed-{}-{index}", message.id),
			url: url.to_owned(),
			name: file_name(url),
			channel_id: message.channel_id,
			message_id: message.id,
			gifv,
			size: size_of(video.width, video.height),
			duration: None,
			attachment: false,
		})
	}

	fn origin(&self) -> Origin {
		Origin {
			channel_id: self.channel_id.0,
			message_id: self.message_id.0,
		}
	}
}

fn size_of(width: Option<i64>, height: Option<i64>) -> Option<(u32, u32)> {
	let width = u32::try_from(width?).ok().filter(|width| *width > 0)?;
	let height = u32::try_from(height?).ok().filter(|height| *height > 0)?;
	Some((width, height))
}

fn streamable(url: &str) -> bool {
	Url::parse(url).is_ok_and(|parsed| {
		parsed.scheme() == "https"
			&& parsed
				.host_str()
				.is_some_and(|host| media::fetch::DEFAULT_ALLOWED_HOSTS.contains(&host))
	})
}

fn file_name(url: &str) -> String {
	let name = Url::parse(url)
		.ok()
		.and_then(|parsed| parsed.path_segments()?.next_back().map(str::to_owned))
		.unwrap_or_default();
	let known = name
		.rsplit_once('.')
		.is_some_and(|(_, extension)| VIDEO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()));
	if known { name } else { format!("{GIFV_MARK}.mp4") }
}

fn source(video: &EmbedMedia) -> Option<&str> {
	let player = video
		.url
		.as_deref()
		.and_then(|url| Url::parse(url).ok())
		.is_some_and(|parsed| parsed.host_str().is_some_and(|host| PLAYER_HOSTS.contains(&host)));
	if player {
		return None;
	}
	[video.proxy_url.as_deref(), video.url.as_deref()]
		.into_iter()
		.flatten()
		.find(|url| streamable(url))
}

pub fn is_playable_embed(embed: &Embed) -> bool {
	embed.video.as_ref().and_then(source).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoPhase {
	Idle,
	Loading,
	Playing,
	Paused,
	Ended,
	Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFailure {
	Unavailable,
	Unplayable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoState {
	pub phase: VideoPhase,
	pub position: Duration,
	pub duration: Option<Duration>,
	pub volume: f32,
	pub muted: bool,
	pub failure: Option<VideoFailure>,
	pub shown: bool,
}

impl VideoState {
	pub fn progress(&self) -> f32 {
		match self.duration {
			Some(duration) if !duration.is_zero() => {
				(self.position.as_secs_f64() / duration.as_secs_f64()).clamp(0., 1.) as f32
			}
			_ => 0.,
		}
	}

	pub fn is_playing(&self) -> bool {
		matches!(self.phase, VideoPhase::Playing | VideoPhase::Loading)
	}

	pub fn started(&self) -> bool {
		!matches!(self.phase, VideoPhase::Idle | VideoPhase::Failed)
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoPicture {
	pub key: String,
	pub picture: Picture,
}

#[derive(Debug, Clone)]
struct Memory {
	channel_id: Snowflake,
	phase: VideoPhase,
	position: Duration,
	duration: Option<Duration>,
	failure: Option<VideoFailure>,
	shown: bool,
	view: (u32, u32),
}

impl Memory {
	fn of(clip: &VideoClip) -> Self {
		Self {
			channel_id: clip.channel_id,
			phase: VideoPhase::Idle,
			position: Duration::ZERO,
			duration: clip.duration,
			failure: None,
			shown: false,
			view: DEFAULT_VIDEO_VIEW,
		}
	}
}

enum Control {
	Play,
	Pause,
	Seek(Duration),
	View(u32, u32),
}

struct Session {
	controls: Sender<Control>,
	feed: Arc<Mutex<Option<Feed>>>,
	gifv: bool,
}

#[derive(Default)]
struct Board {
	memories: HashMap<String, Memory>,
	sessions: HashMap<String, Session>,
	used: VecDeque<String>,
	sounding: Option<String>,
}

impl Board {
	fn touch(&mut self, key: &str) {
		self.used.retain(|known| known != key);
		self.used.push_back(key.to_owned());
	}

	fn trim(&mut self) {
		while self.sessions.len() > LIVE_SESSIONS {
			let idle = self.used.iter().position(|key| {
				self.memories
					.get(key)
					.is_none_or(|memory| !matches!(memory.phase, VideoPhase::Playing | VideoPhase::Loading))
					&& self.sessions.contains_key(key)
			});
			let Some(index) = idle else {
				return;
			};
			if let Some(key) = self.used.remove(index) {
				self.sessions.remove(&key);
			}
		}
	}
}

#[derive(Clone)]
pub struct VideoPlayer {
	inner: Arc<Inner>,
}

struct Inner {
	service: MediaService,
	runtime: Handle,
	output: Mutex<Output>,
	limits: Mutex<VideoLimits>,
	board: Mutex<Board>,
	sound: Mutex<(f32, bool)>,
	pictures: Mutex<HashMap<String, Picture>>,
	waker: Mutex<Option<Hook>>,
	refresher: Mutex<Option<Refresher>>,
}

impl VideoPlayer {
	pub fn new(service: MediaService, runtime: Handle, sink: Sink) -> Self {
		Self {
			inner: Arc::new(Inner {
				service,
				runtime,
				output: Mutex::new(Output::new(sink, Deck::new())),
				limits: Mutex::new(VideoLimits::default()),
				board: Mutex::new(Board::default()),
				sound: Mutex::new((1., false)),
				pictures: Mutex::new(HashMap::new()),
				waker: Mutex::new(None),
				refresher: Mutex::new(None),
			}),
		}
	}

	pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
		*self.inner.waker.lock() = Some(Arc::new(waker));
	}

	pub fn set_refresher(&self, refresher: impl Fn(Origin) + Send + Sync + 'static) {
		*self.inner.refresher.lock() = Some(Arc::new(refresher));
	}

	pub fn set_limits(&self, limits: VideoLimits) {
		*self.inner.limits.lock() = limits;
	}

	pub fn set_sink(&self, sink: Sink) {
		let mut output = self.inner.output.lock();
		if output.sink() != sink {
			output.idle();
			let deck = output.deck().clone();
			*output = Output::new(sink, deck);
		}
	}

	pub fn pump(&self, played: Duration) {
		self.inner.output.lock().pump(played);
	}

	pub fn state(&self, clip: &VideoClip) -> VideoState {
		let (volume, muted) = *self.inner.sound.lock();
		let board = self.inner.board.lock();
		let memory = board
			.memories
			.get(&clip.key)
			.cloned()
			.unwrap_or_else(|| Memory::of(clip));
		VideoState {
			phase: memory.phase,
			position: memory.position,
			duration: memory.duration,
			volume,
			muted: muted || clip.gifv,
			failure: memory.failure,
			shown: memory.shown,
		}
	}

	pub fn playing(&self) -> Vec<String> {
		self.inner
			.board
			.lock()
			.memories
			.iter()
			.filter(|(_, memory)| matches!(memory.phase, VideoPhase::Playing | VideoPhase::Loading))
			.map(|(key, _)| key.clone())
			.collect()
	}

	pub fn toggle(&self, clip: &VideoClip) {
		let phase = self.state(clip).phase;
		match phase {
			VideoPhase::Playing | VideoPhase::Loading => self.pause(clip),
			_ => self.play(clip),
		}
	}

	pub fn play(&self, clip: &VideoClip) {
		let mut board = self.inner.board.lock();
		if !clip.gifv {
			let others: Vec<String> = board
				.sessions
				.iter()
				.filter(|(key, session)| !session.gifv && **key != clip.key)
				.map(|(key, _)| key.clone())
				.collect();
			for other in others {
				pause_in(&mut board, &other);
			}
			board.sounding = Some(clip.key.clone());
		}
		let memory = board
			.memories
			.entry(clip.key.clone())
			.or_insert_with(|| Memory::of(clip));
		let failed = memory.phase == VideoPhase::Failed;
		memory.failure = None;
		memory.phase = if memory.shown && !failed {
			VideoPhase::Playing
		} else {
			VideoPhase::Loading
		};
		let alive = !failed
			&& board
				.sessions
				.get(&clip.key)
				.is_some_and(|session| session.controls.send(Control::Play).is_ok());
		if !alive {
			board.sessions.remove(&clip.key);
			let session = self.start(&board, clip);
			board.sessions.insert(clip.key.clone(), session);
		}
		board.touch(&clip.key);
		board.trim();
		let feed = board
			.sounding
			.as_ref()
			.and_then(|key| board.sessions.get(key))
			.and_then(|session| session.feed.lock().clone());
		drop(board);
		if !clip.gifv {
			self.attach_sound(feed);
		}
		self.inner.wake();
	}

	pub fn pause(&self, clip: &VideoClip) {
		let mut board = self.inner.board.lock();
		pause_in(&mut board, &clip.key);
		let sounding = board.sounding.as_deref() == Some(clip.key.as_str());
		drop(board);
		if sounding {
			self.inner.output.lock().idle();
		}
		self.inner.wake();
	}

	pub fn seek(&self, clip: &VideoClip, fraction: f32) {
		let mut board = self.inner.board.lock();
		let memory = board
			.memories
			.entry(clip.key.clone())
			.or_insert_with(|| Memory::of(clip));
		let Some(duration) = memory.duration else {
			return;
		};
		let fraction = if fraction.is_finite() {
			fraction.clamp(0., 1.)
		} else {
			0.
		};
		let at = duration.mul_f32(fraction);
		memory.position = at;
		if memory.phase == VideoPhase::Ended {
			memory.phase = VideoPhase::Paused;
		}
		let sent = board
			.sessions
			.get(&clip.key)
			.is_some_and(|session| session.controls.send(Control::Seek(at)).is_ok());
		if !sent
			&& board
				.memories
				.get(&clip.key)
				.is_some_and(|memory| memory.phase == VideoPhase::Idle)
		{
			if let Some(memory) = board.memories.get_mut(&clip.key) {
				memory.phase = VideoPhase::Paused;
			}
			let session = self.start(&board, clip);
			let _ = session.controls.send(Control::Pause);
			board.sessions.insert(clip.key.clone(), session);
			board.touch(&clip.key);
			board.trim();
		}
		drop(board);
		self.inner.wake();
	}

	pub fn set_view(&self, clip: &VideoClip, width: u32, height: u32) {
		let (width, height) = (width.max(2), height.max(2));
		let mut board = self.inner.board.lock();
		let memory = board
			.memories
			.entry(clip.key.clone())
			.or_insert_with(|| Memory::of(clip));
		if memory.view == (width, height) {
			return;
		}
		memory.view = (width, height);
		if let Some(session) = board.sessions.get(&clip.key) {
			let _ = session.controls.send(Control::View(width, height));
		}
	}

	pub fn set_volume(&self, volume: f32) {
		let volume = if volume.is_finite() {
			volume.clamp(0., 1.)
		} else {
			0.
		};
		let mut sound = self.inner.sound.lock();
		*sound = (volume, volume == 0. && sound.1);
		drop(sound);
		self.apply_gain();
		self.inner.wake();
	}

	pub fn toggle_mute(&self) -> bool {
		let mut sound = self.inner.sound.lock();
		sound.1 = !sound.1;
		let muted = sound.1;
		drop(sound);
		self.apply_gain();
		self.inner.wake();
		muted
	}

	pub fn sound(&self) -> (f32, bool) {
		*self.inner.sound.lock()
	}

	pub fn set_visible<'a>(&self, keys: impl IntoIterator<Item = &'a str>) {
		let visible: HashSet<&str> = keys.into_iter().collect();
		let mut board = self.inner.board.lock();
		let hidden: Vec<String> = board
			.memories
			.iter()
			.filter(|(key, memory)| {
				matches!(memory.phase, VideoPhase::Playing | VideoPhase::Loading)
					&& !visible.contains(key.as_str())
			})
			.map(|(key, _)| key.clone())
			.collect();
		for key in &hidden {
			pause_in(&mut board, key);
		}
		drop(board);
		if !hidden.is_empty() {
			self.inner.wake();
		}
	}

	pub fn drain(&self) -> Vec<VideoPicture> {
		self.inner
			.pictures
			.lock()
			.drain()
			.map(|(key, picture)| VideoPicture { key, picture })
			.collect()
	}

	pub fn left(&self, channel_id: Snowflake) {
		let mut board = self.inner.board.lock();
		let gone: Vec<String> = board
			.memories
			.iter()
			.filter(|(_, memory)| memory.channel_id == channel_id)
			.map(|(key, _)| key.clone())
			.collect();
		for key in &gone {
			board.sessions.remove(key);
			board.memories.remove(key);
			board.used.retain(|known| known != key);
		}
		if board.sounding.as_ref().is_some_and(|key| gone.contains(key)) {
			board.sounding = None;
		}
		drop(board);
		self.attach_sound(None);
		self.inner.wake();
	}

	pub fn stop(&self) {
		*self.inner.board.lock() = Board::default();
		self.inner.pictures.lock().clear();
		self.attach_sound(None);
		self.inner.wake();
	}

	fn attach_sound(&self, feed: Option<Feed>) {
		let output = self.inner.output.lock();
		if let Some(feed) = &feed {
			feed.set_gain(self.inner.gain());
		}
		let running = feed.is_some();
		output.deck().set_feed(feed);
		if running {
			output.run();
		} else {
			output.idle();
		}
	}

	fn apply_gain(&self) {
		let board = self.inner.board.lock();
		let gain = self.inner.gain();
		for session in board.sessions.values() {
			if let Some(feed) = session.feed.lock().as_ref() {
				feed.set_gain(gain);
			}
		}
	}

	fn start(&self, board: &Board, clip: &VideoClip) -> Session {
		let (controls, received) = mpsc::channel();
		let feed = Arc::new(Mutex::new(None));
		let memory = board.memories.get(&clip.key);
		let job = Job {
			clip: clip.clone(),
			view: memory.map_or(DEFAULT_VIDEO_VIEW, |memory| memory.view),
			from: memory.map_or(Duration::ZERO, |memory| memory.position),
			play: memory.is_none_or(|memory| memory.phase != VideoPhase::Paused),
			controls: received,
			feed: feed.clone(),
			inner: Arc::downgrade(&self.inner),
			ticket: Ticket::new(if clip.gifv { Rank::Visible } else { Rank::Urgent }),
		};
		let spawned = std::thread::Builder::new()
			.name("video-play".to_owned())
			.spawn(move || job.run());
		if let Err(error) = spawned {
			tracing::warn!(%error, "the video thread did not start");
		}
		Session {
			controls,
			feed,
			gifv: clip.gifv,
		}
	}
}

fn pause_in(board: &mut Board, key: &str) {
	if let Some(memory) = board.memories.get_mut(key)
		&& matches!(memory.phase, VideoPhase::Playing | VideoPhase::Loading)
	{
		memory.phase = VideoPhase::Paused;
	}
	if let Some(session) = board.sessions.get(key) {
		let _ = session.controls.send(Control::Pause);
	}
}

impl Inner {
	fn wake(&self) {
		let waker = self.waker.lock().clone();
		if let Some(waker) = waker {
			waker();
		}
	}

	fn gain(&self) -> f32 {
		let (volume, muted) = *self.sound.lock();
		if muted { 0. } else { volume }
	}

	fn update(&self, key: &str, change: impl FnOnce(&mut Memory)) {
		if let Some(memory) = self.board.lock().memories.get_mut(key) {
			change(memory);
		}
		self.wake();
	}

	fn show(&self, key: &str, picture: Picture) {
		self.pictures.lock().insert(key.to_owned(), picture);
		self.update(key, |memory| memory.shown = true);
	}

	fn sounding(&self, key: &str) -> bool {
		self.board.lock().sounding.as_deref() == Some(key)
	}

	fn refresh(&self, clip: &VideoClip) {
		let refresher = self.refresher.lock().clone();
		if let Some(refresher) = refresher {
			refresher(clip.origin());
		}
	}
}

struct Streamed(VideoFile);

impl ByteSource for Streamed {
	fn length(&self) -> io::Result<Option<u64>> {
		self.0.length()
	}

	fn read_at(&self, position: u64, buffer: &mut [u8]) -> io::Result<usize> {
		self.0.read_at(position, buffer)
	}
}

struct Wall {
	base: Duration,
	since: Option<Instant>,
}

impl Wall {
	fn now(&self) -> Duration {
		self.base + self.since.map_or(Duration::ZERO, |since| since.elapsed())
	}

	fn run(&mut self) {
		if self.since.is_none() {
			self.since = Some(Instant::now());
		}
	}

	fn hold(&mut self) {
		self.base = self.now();
		self.since = None;
	}

	fn set(&mut self, at: Duration) {
		self.base = at;
		if self.since.is_some() {
			self.since = Some(Instant::now());
		}
	}
}

struct Job {
	clip: VideoClip,
	view: (u32, u32),
	from: Duration,
	play: bool,
	controls: Receiver<Control>,
	feed: Arc<Mutex<Option<Feed>>>,
	inner: Weak<Inner>,
	ticket: Arc<Ticket>,
}

enum Opened {
	Ready(Box<FileReader>, VideoFile),
	Failed(VideoFailure),
}

impl Job {
	fn run(self) {
		let Some(inner) = self.inner.upgrade() else {
			return;
		};
		let opened = self.open(&inner);
		drop(inner);
		match opened {
			Opened::Ready(reader, file) => Playback::new(self, *reader, file).run(),
			Opened::Failed(failure) => {
				if let Some(inner) = self.inner.upgrade() {
					inner.update(&self.clip.key, |memory| {
						memory.phase = VideoPhase::Failed;
						memory.failure = Some(failure);
					});
				}
			}
		}
	}

	fn open(&self, inner: &Inner) -> Opened {
		let limits = *inner.limits.lock();
		let file = match VideoFile::open(
			inner.service.fetcher().clone(),
			&inner.runtime,
			&self.clip.url,
			self.ticket.clone(),
			limits,
		) {
			Ok(file) => file,
			Err(error) => {
				tracing::debug!(%error, "the video could not be opened");
				return Opened::Failed(VideoFailure::Unavailable);
			}
		};
		match FileReader::open(Arc::new(Streamed(file.clone())), &self.clip.name) {
			Ok(reader) => Opened::Ready(Box::new(reader), file),
			Err(error) => match file.failure() {
				Some(failure) => {
					tracing::debug!(%failure, "the video download failed");
					if self.clip.attachment && failure.is_gone() {
						inner.refresh(&self.clip);
					}
					Opened::Failed(VideoFailure::Unavailable)
				}
				None => {
					tracing::debug!(%error, "the video does not decode");
					Opened::Failed(VideoFailure::Unplayable)
				}
			},
		}
	}
}

struct Playback {
	job: Job,
	reader: FileReader,
	_file: VideoFile,
	sound: Option<(Feed, u16)>,
	frames: VecDeque<VideoFrame>,
	last: Option<VideoFrame>,
	shown_at: Option<Duration>,
	wall: Wall,
	playing: bool,
	ended: bool,
	skip_until: Duration,
	reveal: bool,
	duration: Option<Duration>,
}

impl Playback {
	fn new(job: Job, reader: FileReader, file: VideoFile) -> Self {
		let tracks = reader.tracks();
		let sound = tracks
			.sound
			.filter(|_| !job.clip.gifv)
			.map(|track| (Feed::new(track.rate), track.channels));
		*job.feed.lock() = sound.as_ref().map(|(feed, _)| feed.clone());
		let from = job.from;
		let playing = job.play;
		Self {
			job,
			reader,
			_file: file,
			sound,
			frames: VecDeque::new(),
			last: None,
			shown_at: None,
			wall: Wall {
				base: from,
				since: None,
			},
			playing,
			ended: false,
			skip_until: from,
			reveal: true,
			duration: tracks.duration,
		}
	}

	fn inner(&self) -> Option<Arc<Inner>> {
		self.job.inner.upgrade()
	}

	fn run(mut self) {
		let Some(inner) = self.inner() else {
			return;
		};
		let duration = self.duration;
		inner.update(&self.job.clip.key, |memory| {
			if duration.is_some() {
				memory.duration = duration;
			}
		});
		if let Some(feed) = self.feed() {
			feed.set_gain(inner.gain());
			if inner.sounding(&self.job.clip.key) {
				let output = inner.output.lock();
				output.deck().set_feed(Some(feed));
				output.run();
			}
		}
		drop(inner);
		if !self.job.from.is_zero() && self.reader.seek(self.job.from).is_err() {
			self.skip_until = Duration::ZERO;
		}
		loop {
			let wait = if self.playing || self.reveal {
				self.turn()
			} else {
				IDLE_WAIT
			};
			match self.job.controls.recv_timeout(wait) {
				Ok(control) => self.control(control),
				Err(RecvTimeoutError::Timeout) => {}
				Err(RecvTimeoutError::Disconnected) => return,
			}
			while let Ok(control) = self.job.controls.try_recv() {
				self.control(control);
			}
			if self.inner().is_none() {
				return;
			}
		}
	}

	fn feed(&self) -> Option<Feed> {
		self.sound.as_ref().map(|(feed, _)| feed.clone())
	}

	fn control(&mut self, control: Control) {
		match control {
			Control::Play => {
				if self.ended {
					self.seek(Duration::ZERO);
				}
				self.playing = true;
				if let Some(feed) = self.feed()
					&& let Some(inner) = self.inner()
					&& inner.sounding(&self.job.clip.key)
				{
					let output = inner.output.lock();
					output.deck().set_feed(Some(feed));
					output.run();
				}
			}
			Control::Pause => {
				self.playing = false;
				self.wall.hold();
				if let Some(feed) = self.feed() {
					feed.set_playing(false);
				}
			}
			Control::Seek(at) => self.seek(at),
			Control::View(width, height) => {
				self.job.view = (width, height);
				if let Some(frame) = self.last.take() {
					self.present(&frame);
					self.last = Some(frame);
				}
			}
		}
	}

	fn seek(&mut self, at: Duration) {
		let at = self.duration.map_or(at, |duration| at.min(duration));
		if self.reader.seek(at).is_err() {
			return;
		}
		self.frames.clear();
		self.ended = false;
		self.skip_until = at;
		self.reveal = true;
		self.shown_at = None;
		self.wall.set(at);
		if let Some(feed) = self.feed() {
			feed.restart(at);
		}
		let key = self.job.clip.key.clone();
		if let Some(inner) = self.inner() {
			inner.update(&key, |memory| {
				memory.position = at;
				if memory.phase == VideoPhase::Ended {
					memory.phase = VideoPhase::Paused;
				}
			});
		}
	}

	fn needs_more(&self) -> bool {
		if self.ended {
			return false;
		}
		let buffered = self.feed().map(|feed| feed.buffered());
		let sound_hungry = buffered.is_some_and(|buffered| buffered < SOUND_AHEAD);
		let sound_starving = buffered.is_some_and(|buffered| buffered < SOUND_FLOOR);
		(self.frames.len() < QUEUED_FRAMES || sound_hungry)
			&& (self.frames.len() < MAX_QUEUED_FRAMES || sound_starving)
	}

	fn read(&mut self) -> bool {
		match self.reader.packet() {
			Ok(Packet::Video(frame)) => {
				if frame.timestamp + frame_gap() < self.skip_until {
					return true;
				}
				if self.frames.len() >= MAX_QUEUED_FRAMES {
					self.frames.pop_front();
				}
				self.frames.push_back(frame);
				true
			}
			Ok(Packet::Audio { timestamp, samples }) => {
				if let Some((feed, channels)) = &self.sound
					&& timestamp + frame_gap() >= self.skip_until
				{
					feed.push(timestamp, &samples, *channels);
				}
				true
			}
			Ok(Packet::End) => {
				self.ended = true;
				if let Some(feed) = self.feed() {
					feed.finish();
				}
				false
			}
			Err(error) => {
				tracing::debug!(%error, "a video packet did not decode");
				self.ended = true;
				if let Some(feed) = self.feed() {
					feed.finish();
				}
				false
			}
		}
	}

	fn clock(&mut self) -> Duration {
		match self.feed() {
			Some(feed) if !feed.ended() => {
				let at = feed.clock();
				self.wall.set(at);
				at
			}
			_ => self.wall.now(),
		}
	}

	fn turn(&mut self) -> Duration {
		let mut reads = 0;
		while reads < READS_PER_TURN
			&& (self.needs_more() || (self.reveal && self.frames.is_empty() && !self.ended))
		{
			if !self.read() {
				break;
			}
			reads += 1;
		}
		if self.reveal
			&& let Some(frame) = self.frames.pop_front()
		{
			self.reveal = false;
			self.present(&frame);
			self.report(frame.timestamp);
			self.last = Some(frame);
			if self.playing {
				self.begin();
			}
			return SHORTEST_WAIT;
		}
		if !self.playing {
			return IDLE_WAIT;
		}
		self.begin();
		let now = self.clock();
		let mut due = None;
		while self.frames.front().is_some_and(|frame| frame.timestamp <= now) {
			due = self.frames.pop_front();
		}
		if let Some(frame) = due {
			if self
				.shown_at
				.is_none_or(|shown| frame.timestamp >= shown + frame_gap().mul_f32(0.75))
			{
				self.present(&frame);
			}
			self.report(frame.timestamp);
			self.last = Some(frame);
		}
		let silent = self.feed().is_none_or(|feed| feed.ended());
		if silent && self.frames.is_empty() && !self.ended {
			self.wall.hold();
		} else {
			self.wall.run();
		}
		if self.ended && self.frames.is_empty() && silent {
			self.finish();
			return SHORTEST_WAIT;
		}
		self.frames
			.front()
			.map_or(LONGEST_WAIT, |frame| frame.timestamp.saturating_sub(now))
			.clamp(SHORTEST_WAIT, LONGEST_WAIT)
	}

	fn begin(&mut self) {
		if let Some(feed) = self.feed() {
			feed.set_playing(true);
		}
		let key = self.job.clip.key.clone();
		if let Some(inner) = self.inner() {
			let mut board = inner.board.lock();
			if let Some(memory) = board.memories.get_mut(&key)
				&& memory.phase == VideoPhase::Loading
			{
				memory.phase = VideoPhase::Playing;
				drop(board);
				inner.wake();
			}
		}
	}

	fn finish(&mut self) {
		if self.job.clip.gifv {
			self.seek(Duration::ZERO);
			self.reveal = false;
			return;
		}
		self.playing = false;
		self.wall.hold();
		if let Some(feed) = self.feed() {
			feed.set_playing(false);
		}
		let end = self.duration.or(self.last.as_ref().map(|frame| frame.timestamp));
		let key = self.job.clip.key.clone();
		if let Some(inner) = self.inner() {
			inner.update(&key, |memory| {
				memory.phase = VideoPhase::Ended;
				if let Some(end) = end {
					memory.position = end;
				}
			});
		}
		self.ended = true;
	}

	fn present(&mut self, frame: &VideoFrame) {
		let (width, height) = self.job.view;
		match Picture::from_nv12(frame, width, height) {
			Ok(picture) => {
				self.shown_at = Some(frame.timestamp);
				if let Some(inner) = self.inner() {
					inner.show(&self.job.clip.key, picture);
				}
			}
			Err(error) => tracing::debug!(%error, "a video frame was not usable"),
		}
	}

	fn report(&self, at: Duration) {
		let key = self.job.clip.key.clone();
		if let Some(inner) = self.inner() {
			inner.update(&key, |memory| memory.position = at);
		}
	}
}

fn frame_gap() -> Duration {
	Duration::from_secs(1) / VIDEO_MAX_FPS
}
