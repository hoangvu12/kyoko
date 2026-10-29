use std::cell::{Cell, RefCell};
use std::rc::Rc;

use protocol::Embed;
use url::Url;
use url::form_urlencoded::byte_serialize;

use crate::message::{EMBED_MEDIA_BOX, fit_box};

const YOUTUBE_ID_LEN: usize = 11;
const SPOTIFY_ID_LEN: usize = 22;
const VIMEO_ID_LEN: usize = 12;
const MAX_SEGMENT_LEN: usize = 128;
const MAX_START_SECONDS: u64 = 24 * 3600;
const DEFAULT_VIDEO_SIZE: (i64, i64) = (1280, 720);
const WIDGET_WIDTH: f32 = 400.;
const SPOTIFY_TRACK_HEIGHT: f32 = 80.;
const SPOTIFY_EPISODE_HEIGHT: f32 = 152.;
const SPOTIFY_LIST_HEIGHT: f32 = 352.;
const SOUNDCLOUD_TRACK_HEIGHT: f32 = 166.;
const SOUNDCLOUD_SET_HEIGHT: f32 = 400.;

pub const FRAME_ORIGINS: [&str; 6] = [
	"https://www.youtube.com",
	"https://www.youtube-nocookie.com",
	"https://consent.youtube.com",
	"https://player.vimeo.com",
	"https://open.spotify.com",
	"https://w.soundcloud.com",
];
pub const FRAME_SCHEME: &str = "kyoko";
pub const FRAME_URL: &str = "kyoko://localhost/frame";
pub const FRAME_ORIGIN: &str = "https://kyoko.localhost/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
	YouTube,
	Vimeo,
	Spotify,
	SoundCloud,
}

impl Provider {
	pub fn name(self) -> &'static str {
		match self {
			Provider::YouTube => "YouTube",
			Provider::Vimeo => "Vimeo",
			Provider::Spotify => "Spotify",
			Provider::SoundCloud => "SoundCloud",
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerLayout {
	Video,
	Widget,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderPlayer {
	pub provider: Provider,
	pub layout: PlayerLayout,
	pub src: String,
	pub size: (f32, f32),
	pub link: Option<String>,
}

impl ProviderPlayer {
	pub fn of(embed: &Embed) -> Option<Self> {
		let video = embed.video.as_ref();
		let video_size = video.and_then(|video| Some((video.width?, video.height?)));
		[embed.url.as_deref(), video.and_then(|video| video.url.as_deref())]
			.into_iter()
			.flatten()
			.filter_map(|raw| Url::parse(raw).ok())
			.filter(|url| url.scheme() == "https" && url.username().is_empty() && url.password().is_none())
			.find_map(|url| resolve(&url, video_size))
			.map(|player| Self {
				link: embed.url.clone().filter(|link| link.starts_with("https://")),
				..player
			})
	}

	pub fn document(&self) -> String {
		frame_document(&self.src)
	}
}

fn resolve(url: &Url, video_size: Option<(i64, i64)>) -> Option<ProviderPlayer> {
	let host = url.host_str()?.to_ascii_lowercase();
	let host = host.strip_prefix("www.").unwrap_or(&host);
	let (width, height) = video_size
		.filter(|(width, height)| *width > 0 && *height > 0)
		.unwrap_or(DEFAULT_VIDEO_SIZE);
	let size = fit_box(Some(width), Some(height), EMBED_MEDIA_BOX);
	match host {
		"youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be" | "youtube-nocookie.com" => {
			youtube(url, host).map(|(id, start)| {
				let start = start.map(|start| format!("&start={start}")).unwrap_or_default();
				video(
					Provider::YouTube,
					format!("https://www.youtube.com/embed/{id}?autoplay=1&auto_play=1{start}"),
					size,
				)
			})
		}
		"vimeo.com" | "player.vimeo.com" => vimeo(url).map(|id| {
			video(
				Provider::Vimeo,
				format!("https://player.vimeo.com/video/{id}?autoplay=1"),
				size,
			)
		}),
		"open.spotify.com" => spotify(url),
		"soundcloud.com" | "m.soundcloud.com" | "w.soundcloud.com" => soundcloud(url),
		_ => None,
	}
}

fn video(provider: Provider, src: String, size: (f32, f32)) -> ProviderPlayer {
	ProviderPlayer {
		provider,
		layout: PlayerLayout::Video,
		src,
		size,
		link: None,
	}
}

fn widget(provider: Provider, src: String, height: f32) -> ProviderPlayer {
	ProviderPlayer {
		provider,
		layout: PlayerLayout::Widget,
		src,
		size: (WIDGET_WIDTH, height),
		link: None,
	}
}

fn segments(url: &Url) -> Vec<&str> {
	url.path_segments()
		.map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
		.unwrap_or_default()
}

fn query(url: &Url, key: &str) -> Option<String> {
	url.query_pairs()
		.find(|(name, _)| name == key)
		.map(|(_, value)| value.into_owned())
}

fn youtube_id(candidate: &str) -> Option<String> {
	(candidate.len() == YOUTUBE_ID_LEN
		&& candidate
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'))
	.then(|| candidate.to_owned())
}

fn youtube(url: &Url, host: &str) -> Option<(String, Option<u64>)> {
	let path = segments(url);
	let id = if host == "youtu.be" {
		path.first().copied().and_then(youtube_id)
	} else {
		match path.as_slice() {
			["watch"] => query(url, "v").as_deref().and_then(youtube_id),
			["embed" | "shorts" | "live" | "v", id, ..] => youtube_id(id),
			_ => None,
		}
	}?;
	let start = query(url, "t")
		.or_else(|| query(url, "start"))
		.and_then(|value| seconds(&value))
		.filter(|start| *start > 0 && *start <= MAX_START_SECONDS);
	Some((id, start))
}

fn seconds(value: &str) -> Option<u64> {
	if let Ok(plain) = value.trim_end_matches('s').parse::<u64>() {
		return Some(plain);
	}
	let mut total = 0u64;
	let mut number = 0u64;
	let mut seen = false;
	for character in value.chars() {
		let unit = match character {
			'0'..='9' => {
				number = number
					.checked_mul(10)?
					.checked_add(u64::from(character as u8 - b'0'))?;
				seen = true;
				continue;
			}
			'h' if seen => 3600,
			'm' if seen => 60,
			's' if seen => 1,
			_ => return None,
		};
		total = total.checked_add(number.checked_mul(unit)?)?;
		number = 0;
		seen = false;
	}
	(!seen).then_some(total)
}

fn vimeo(url: &Url) -> Option<String> {
	let path = segments(url);
	let id = match path.as_slice() {
		["video", id, ..] | [id] | ["channels", _, id] => *id,
		_ => return None,
	};
	(!id.is_empty() && id.len() <= VIMEO_ID_LEN && id.bytes().all(|byte| byte.is_ascii_digit()))
		.then(|| id.to_owned())
}

fn spotify(url: &Url) -> Option<ProviderPlayer> {
	let path = segments(url);
	let path = match path.as_slice() {
		[locale, rest @ ..] if locale.starts_with("intl-") => rest,
		["embed", rest @ ..] => rest,
		rest => rest,
	};
	let [kind, id] = path else {
		return None;
	};
	let height = match *kind {
		"track" => SPOTIFY_TRACK_HEIGHT,
		"episode" => SPOTIFY_EPISODE_HEIGHT,
		"album" | "playlist" | "artist" | "show" => SPOTIFY_LIST_HEIGHT,
		_ => return None,
	};
	let valid = id.len() == SPOTIFY_ID_LEN && id.bytes().all(|byte| byte.is_ascii_alphanumeric());
	valid.then(|| {
		widget(
			Provider::Spotify,
			format!("https://open.spotify.com/embed/{kind}/{id}?autoplay=1"),
			height,
		)
	})
}

fn soundcloud_segment(segment: &str) -> bool {
	!segment.is_empty()
		&& segment.len() <= MAX_SEGMENT_LEN
		&& segment
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn soundcloud(url: &Url) -> Option<ProviderPlayer> {
	if url.host_str() == Some("w.soundcloud.com") {
		let inner = query(url, "url").and_then(|inner| Url::parse(&inner).ok())?;
		let host = inner.host_str()?.to_ascii_lowercase();
		let host = host.strip_prefix("www.").unwrap_or(&host);
		return (inner.scheme() == "https" && matches!(host, "soundcloud.com" | "m.soundcloud.com"))
			.then(|| soundcloud(&inner))
			.flatten();
	}
	let path = segments(url);
	let (canonical, height) = match path.as_slice() {
		[user, "sets", set] if soundcloud_segment(user) && soundcloud_segment(set) => (
			format!("https://soundcloud.com/{user}/sets/{set}"),
			SOUNDCLOUD_SET_HEIGHT,
		),
		[user, track] if soundcloud_segment(user) && soundcloud_segment(track) && *track != "sets" => (
			format!("https://soundcloud.com/{user}/{track}"),
			SOUNDCLOUD_TRACK_HEIGHT,
		),
		_ => return None,
	};
	let encoded: String = byte_serialize(canonical.as_bytes()).collect();
	Some(widget(
		Provider::SoundCloud,
		format!("https://w.soundcloud.com/player/?url={encoded}&auto_play=true&visual=false"),
		height,
	))
}

pub fn frame_policy() -> String {
	format!(
		"default-src 'none'; style-src 'unsafe-inline'; frame-src {}",
		FRAME_ORIGINS.join(" ")
	)
}

pub fn frame_document(src: &str) -> String {
	let policy = frame_policy();
	let src = escape_attribute(src);
	format!(
		"<!doctype html><html><head><meta charset=\"utf-8\">\
		<meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\">\
		<meta name=\"referrer\" content=\"strict-origin\">\
		<style>html,body{{margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#000}}\
		iframe{{border:0;width:100%;height:100%;display:block}}</style>\
		</head><body><iframe src=\"{src}\" allow=\"autoplay; encrypted-media; picture-in-picture; fullscreen\" \
		allowfullscreen sandbox=\"allow-scripts allow-same-origin allow-popups allow-popups-to-escape-sandbox allow-presentation\" \
		referrerpolicy=\"strict-origin\"></iframe></body></html>"
	)
}

pub fn frame_allows(url: &str) -> bool {
	url.starts_with(FRAME_ORIGIN)
}

fn escape_attribute(value: &str) -> String {
	let mut escaped = String::with_capacity(value.len());
	for character in value.chars() {
		match character {
			'&' => escaped.push_str("&amp;"),
			'"' => escaped.push_str("&quot;"),
			'<' => escaped.push_str("&lt;"),
			'>' => escaped.push_str("&gt;"),
			'\'' => escaped.push_str("&#39;"),
			other => escaped.push(other),
		}
	}
	escaped
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameRect {
	pub x: i32,
	pub y: i32,
	pub width: i32,
	pub height: i32,
}

impl FrameRect {
	pub fn is_empty(self) -> bool {
		self.width <= 0 || self.height <= 0
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sight {
	Gone,
	Hidden,
	Shown { rect: FrameRect, clip: FrameRect },
}

pub trait FrameHost {
	fn available(&self) -> bool;
	fn open(&mut self, id: u64, document: String) -> Result<(), String>;
	fn place(&mut self, id: u64, rect: FrameRect, clip: FrameRect);
	fn hide(&mut self, id: u64);
	fn close(&mut self, id: u64);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Played {
	Opened,
	Browser,
}

struct LiveFrame {
	key: String,
	id: u64,
	shown: Option<(FrameRect, FrameRect)>,
}

pub struct ProviderFrames {
	host: RefCell<Box<dyn FrameHost>>,
	live: RefCell<Option<LiveFrame>>,
	next: Cell<u64>,
}

impl ProviderFrames {
	pub fn new(host: Box<dyn FrameHost>) -> Self {
		Self {
			host: RefCell::new(host),
			live: RefCell::new(None),
			next: Cell::new(0),
		}
	}

	pub fn set_host(&self, host: Box<dyn FrameHost>) {
		self.close();
		*self.host.borrow_mut() = host;
	}

	pub fn available(&self) -> bool {
		self.host.borrow().available()
	}

	pub fn live(&self) -> Option<String> {
		self.live.borrow().as_ref().map(|live| live.key.clone())
	}

	pub fn play(&self, key: &str, player: &ProviderPlayer) -> Played {
		if self.live().as_deref() == Some(key) {
			return Played::Opened;
		}
		self.close();
		if !self.available() {
			return Played::Browser;
		}
		let id = self.next.get() + 1;
		self.next.set(id);
		match self.host.borrow_mut().open(id, player.document()) {
			Ok(()) => {
				*self.live.borrow_mut() = Some(LiveFrame {
					key: key.to_owned(),
					id,
					shown: None,
				});
				Played::Opened
			}
			Err(error) => {
				tracing::warn!(%error, provider = player.provider.name(), "the embed player could not be made");
				Played::Browser
			}
		}
	}

	pub fn follow(&self, sight: Sight) {
		let mut live = self.live.borrow_mut();
		let Some(frame) = live.as_mut() else {
			return;
		};
		let mut host = self.host.borrow_mut();
		match sight {
			Sight::Gone => {
				host.close(frame.id);
				*live = None;
			}
			Sight::Shown { rect, clip } if !clip.is_empty() => {
				if frame.shown != Some((rect, clip)) {
					frame.shown = Some((rect, clip));
					host.place(frame.id, rect, clip);
				}
			}
			Sight::Hidden | Sight::Shown { .. } => {
				if frame.shown.take().is_some() {
					host.hide(frame.id);
				}
			}
		}
	}

	pub fn close(&self) -> bool {
		let Some(frame) = self.live.borrow_mut().take() else {
			return false;
		};
		self.host.borrow_mut().close(frame.id);
		true
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameCall {
	Open {
		id: u64,
		document: String,
	},
	Place {
		id: u64,
		rect: FrameRect,
		clip: FrameRect,
	},
	Hide {
		id: u64,
	},
	Close {
		id: u64,
	},
}

#[derive(Clone)]
pub struct StandInFrames {
	available: bool,
	calls: Rc<RefCell<Vec<FrameCall>>>,
}

impl StandInFrames {
	pub fn new(available: bool) -> Self {
		Self {
			available,
			calls: Rc::default(),
		}
	}

	pub fn calls(&self) -> Vec<FrameCall> {
		self.calls.borrow().clone()
	}

	pub fn open_frames(&self) -> usize {
		self.calls.borrow().iter().fold(0usize, |open, call| match call {
			FrameCall::Open { .. } => open + 1,
			FrameCall::Close { .. } => open.saturating_sub(1),
			_ => open,
		})
	}

	pub fn placement(&self) -> Option<(FrameRect, FrameRect)> {
		self.calls
			.borrow()
			.iter()
			.rev()
			.find_map(|call| match call {
				FrameCall::Place { rect, clip, .. } => Some(Some((*rect, *clip))),
				FrameCall::Hide { .. } | FrameCall::Close { .. } => Some(None),
				FrameCall::Open { .. } => None,
			})
			.flatten()
	}
}

impl FrameHost for StandInFrames {
	fn available(&self) -> bool {
		self.available
	}

	fn open(&mut self, id: u64, document: String) -> Result<(), String> {
		self.calls.borrow_mut().push(FrameCall::Open { id, document });
		Ok(())
	}

	fn place(&mut self, id: u64, rect: FrameRect, clip: FrameRect) {
		self.calls.borrow_mut().push(FrameCall::Place { id, rect, clip });
	}

	fn hide(&mut self, id: u64) {
		self.calls.borrow_mut().push(FrameCall::Hide { id });
	}

	fn close(&mut self, id: u64) {
		self.calls.borrow_mut().push(FrameCall::Close { id });
	}
}
