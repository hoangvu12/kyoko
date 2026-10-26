use std::collections::HashSet;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant, SystemTime};

use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use tokio::runtime::Handle;
use tokio::sync::Notify;

use crate::error::redact_url;
use crate::service::{MediaService, Source, Variant};
use crate::signed;
use crate::types::Decoded;

pub const DEFAULT_BUDGET_BYTES: usize = 128 * 1024 * 1024;
pub const ANIMATION_SHARE: usize = 8;
const RETRY_AFTER: Duration = Duration::from_secs(60);
const MAX_ENTRIES: usize = 4096;
const NAME_PREFIX: &str = "media/";

type Waker = Arc<dyn Fn() + Send + Sync>;
type Refresher = Arc<dyn Fn(Origin) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Playback {
	#[default]
	Always,
	OnHover,
	Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Animations {
	pub gifs: Playback,
	pub emoji: Playback,
	pub stickers: Playback,
}

impl Animations {
	pub const fn uniform(playback: Playback) -> Self {
		Self {
			gifs: playback,
			emoji: playback,
			stickers: playback,
		}
	}

	fn of(self, kind: Kind) -> Playback {
		match kind {
			Kind::Emoji => self.emoji,
			Kind::Sticker => self.stickers,
			Kind::Other => self.gifs,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
	Emoji,
	Sticker,
	Other,
}

impl Kind {
	fn of(url: &str) -> Self {
		let path = url.split(['?', '#']).next().unwrap_or_default();
		if path.contains("/emojis/") {
			Self::Emoji
		} else if path.contains("/stickers/") {
			Self::Sticker
		} else {
			Self::Other
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Origin {
	pub channel_id: u64,
	pub message_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRequest {
	pub url: String,
	pub variant: Variant,
	pub origin: Option<Origin>,
}

impl ImageRequest {
	pub fn new(url: impl Into<String>, variant: Variant) -> Self {
		Self {
			url: url.into(),
			variant,
			origin: None,
		}
	}

	pub fn from_message(mut self, origin: Origin) -> Self {
		self.origin = Some(origin);
		self
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageName(Arc<str>);

impl ImageName {
	fn of(key: u64) -> Self {
		Self(format!("{NAME_PREFIX}{key:016x}").into())
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Deref for ImageName {
	type Target = str;

	fn deref(&self) -> &str {
		&self.0
	}
}

impl fmt::Display for ImageName {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.0)
	}
}

fn key_of_name(name: &str) -> Option<u64> {
	u64::from_str_radix(name.strip_prefix(NAME_PREFIX)?, 16).ok()
}

fn key_of(url: &str, variant: Variant) -> u64 {
	let mut hasher = DefaultHasher::new();
	signed::unsigned(url).hash(&mut hasher);
	variant.hash(&mut hasher);
	hasher.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
	pub width: u32,
	pub height: u32,
	pub rgba: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
	Show { name: ImageName, pixels: Pixels },
	Clear { name: ImageName },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
	Loading,
	Ready { frames: usize },
	Refreshing,
	Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GalleryStats {
	pub entries: usize,
	pub bytes: usize,
	pub budget: usize,
}

struct Picture {
	frames: Vec<(Pixels, Duration)>,
	loops: Option<u32>,
	bytes: usize,
}

impl Picture {
	fn from_decoded(decoded: Decoded, animation_cap: usize) -> Self {
		let downgrade = decoded.byte_len() > animation_cap;
		let (frames, loops) = match decoded {
			Decoded::Still(image) => (vec![(image, 0)], None),
			Decoded::Animated(animation) if downgrade => {
				let first = animation.frames.into_iter().next();
				(first.map(|frame| (frame.image, 0)).into_iter().collect(), None)
			}
			Decoded::Animated(animation) => (
				animation
					.frames
					.into_iter()
					.map(|frame| (frame.image, frame.delay_ms))
					.collect(),
				animation.loops,
			),
		};
		let frames: Vec<(Pixels, Duration)> = frames
			.into_iter()
			.map(|(image, delay)| {
				(
					Pixels {
						width: image.width,
						height: image.height,
						rgba: Arc::from(image.rgba),
					},
					Duration::from_millis(u64::from(delay)),
				)
			})
			.collect();
		let bytes = frames.iter().map(|(pixels, _)| pixels.rgba.len()).sum();
		Self { frames, loops, bytes }
	}

	fn animated(&self) -> bool {
		self.frames.len() > 1
	}

	fn delay(&self, frame: usize) -> Duration {
		self.frames[frame].1
	}

	fn cycle(&self) -> Duration {
		self.frames.iter().map(|(_, delay)| *delay).sum()
	}
}

enum Phase {
	Loading { url: String },
	Ready(Picture),
	Refreshing { stale: String },
	Failed { at: Instant },
}

struct Entry {
	name: ImageName,
	origin: Option<Origin>,
	kind: Kind,
	phase: Phase,
	refreshed: bool,
	frame: usize,
	shown: Option<usize>,
	due: Option<Instant>,
	plays: u32,
}

impl Entry {
	fn new(key: u64, url: &str, origin: Option<Origin>) -> Self {
		Self {
			name: ImageName::of(key),
			origin,
			kind: Kind::of(url),
			phase: Phase::Loading { url: url.to_owned() },
			refreshed: false,
			frame: 0,
			shown: None,
			due: None,
			plays: 0,
		}
	}

	fn loading(&self, url: &str) -> bool {
		matches!(&self.phase, Phase::Loading { url: loading } if loading == url)
	}

	fn wants_load(&self, url: &str) -> bool {
		match &self.phase {
			Phase::Loading { .. } | Phase::Ready(_) => false,
			Phase::Refreshing { stale } => stale != url,
			Phase::Failed { at } => at.elapsed() >= RETRY_AFTER,
		}
	}

	fn finished(&self, picture: &Picture) -> bool {
		picture.loops.is_some_and(|loops| self.plays >= loops.max(1))
	}
}

struct Inner {
	entries: LruCache<u64, Entry>,
	bytes: usize,
	budget: usize,
	visible: HashSet<u64>,
	hovered: Option<u64>,
	playback: Animations,
	reduced_motion: bool,
	animating: HashSet<u64>,
	dirty: Vec<u64>,
	clears: Vec<ImageName>,
}

impl Inner {
	fn new(budget: usize) -> Self {
		Self {
			entries: LruCache::unbounded(),
			bytes: 0,
			budget,
			visible: HashSet::new(),
			hovered: None,
			playback: Animations::default(),
			reduced_motion: false,
			animating: HashSet::new(),
			dirty: Vec::new(),
			clears: Vec::new(),
		}
	}

	fn moving(&self, key: u64) -> bool {
		let kind = self.entries.peek(&key).map_or(Kind::Other, |entry| entry.kind);
		!self.reduced_motion
			&& match self.playback.of(kind) {
				Playback::Always => true,
				Playback::OnHover => self.hovered == Some(key),
				Playback::Never => false,
			}
	}

	fn evict(&mut self) {
		if self.bytes > self.budget {
			let mut excess = self.bytes - self.budget;
			let victims: Vec<u64> = self
				.entries
				.iter()
				.rev()
				.filter(|(key, entry)| matches!(entry.phase, Phase::Ready(_)) && !self.visible.contains(*key))
				.map_while(|(key, entry)| {
					if excess == 0 {
						return None;
					}
					if let Phase::Ready(picture) = &entry.phase {
						excess = excess.saturating_sub(picture.bytes);
					}
					Some(*key)
				})
				.collect();
			for key in victims {
				self.forget(key);
			}
		}
		if self.entries.len() > MAX_ENTRIES {
			let extra = self.entries.len() - MAX_ENTRIES;
			let idle: Vec<u64> = self
				.entries
				.iter()
				.rev()
				.filter(|(_, entry)| matches!(entry.phase, Phase::Failed { .. } | Phase::Refreshing { .. }))
				.map(|(key, _)| *key)
				.take(extra)
				.collect();
			for key in idle {
				self.forget(key);
			}
		}
	}

	fn forget(&mut self, key: u64) {
		let Some(entry) = self.entries.pop(&key) else {
			return;
		};
		if let Phase::Ready(picture) = &entry.phase {
			self.bytes -= picture.bytes;
		}
		self.animating.remove(&key);
		if entry.shown.is_some() {
			self.clears.push(entry.name);
		}
	}

	fn advance(&mut self, now: Instant) {
		let candidates: Vec<u64> = self.visible.union(&self.animating).copied().collect();
		for key in candidates {
			let visible = self.visible.contains(&key);
			let moving = self.moving(key);
			let Some(entry) = self.entries.peek_mut(&key) else {
				self.animating.remove(&key);
				continue;
			};
			let Phase::Ready(picture) = &entry.phase else {
				continue;
			};
			if !picture.animated() {
				continue;
			}
			if !visible || !moving || entry.finished(picture) {
				self.animating.remove(&key);
				entry.due = None;
				if !moving && entry.frame != 0 {
					entry.frame = 0;
					entry.plays = 0;
					self.dirty.push(key);
				}
				continue;
			}
			self.animating.insert(key);
			let mut due = entry.due.unwrap_or_else(|| now + picture.delay(entry.frame));
			if now.saturating_duration_since(due) > picture.cycle() {
				due = now;
			}
			let before = entry.frame;
			while due <= now {
				let next = (entry.frame + 1) % picture.frames.len();
				if next == 0 {
					entry.plays += 1;
					if entry.finished(picture) {
						break;
					}
				}
				entry.frame = next;
				due += picture.delay(next).max(Duration::from_millis(1));
			}
			entry.due = (!entry.finished(picture)).then_some(due);
			if entry.frame != before {
				self.dirty.push(key);
			}
		}
	}

	fn next_due(&self) -> Option<Instant> {
		self.animating
			.iter()
			.filter_map(|key| self.entries.peek(key)?.due)
			.min()
	}

	fn take_updates(&mut self) -> Vec<Update> {
		let mut updates: Vec<Update> = self.clears.drain(..).map(|name| Update::Clear { name }).collect();
		for key in std::mem::take(&mut self.dirty) {
			let Some(entry) = self.entries.peek_mut(&key) else {
				continue;
			};
			let Phase::Ready(picture) = &entry.phase else {
				continue;
			};
			if entry.shown == Some(entry.frame) {
				continue;
			}
			entry.shown = Some(entry.frame);
			updates.push(Update::Show {
				name: entry.name.clone(),
				pixels: picture.frames[entry.frame].0.clone(),
			});
		}
		updates
	}
}

struct Shared {
	inner: Mutex<Inner>,
	loader: MediaService,
	runtime: Handle,
	waker: RwLock<Option<Waker>>,
	refresher: RwLock<Option<Refresher>>,
	ticker: Arc<Notify>,
}

impl Drop for Shared {
	fn drop(&mut self) {
		self.ticker.notify_one();
	}
}

#[derive(Clone)]
pub struct Gallery {
	shared: Arc<Shared>,
}

impl Gallery {
	pub fn new(loader: MediaService, runtime: Handle) -> Self {
		Self::with_budget(loader, runtime, DEFAULT_BUDGET_BYTES)
	}

	pub fn with_budget(loader: MediaService, runtime: Handle, budget: usize) -> Self {
		let ticker = Arc::new(Notify::new());
		let shared = Arc::new(Shared {
			inner: Mutex::new(Inner::new(budget)),
			loader,
			runtime: runtime.clone(),
			waker: RwLock::new(None),
			refresher: RwLock::new(None),
			ticker: ticker.clone(),
		});
		runtime.spawn(tick(Arc::downgrade(&shared), ticker));
		Self { shared }
	}

	pub fn loader(&self) -> &MediaService {
		&self.shared.loader
	}

	pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
		*self.shared.waker.write() = Some(Arc::new(waker));
	}

	pub fn set_refresher(&self, refresher: impl Fn(Origin) + Send + Sync + 'static) {
		*self.shared.refresher.write() = Some(Arc::new(refresher));
	}

	pub fn request(&self, request: ImageRequest) -> ImageName {
		let key = key_of(&request.url, request.variant);
		let load = {
			let mut inner = self.shared.inner.lock();
			match inner.entries.get_mut(&key) {
				Some(entry) if entry.wants_load(&request.url) => {
					entry.phase = Phase::Loading {
						url: request.url.clone(),
					};
					entry.origin = request.origin.or(entry.origin);
					true
				}
				Some(_) => false,
				None => {
					inner
						.entries
						.put(key, Entry::new(key, &request.url, request.origin));
					inner.evict();
					true
				}
			}
		};
		if load {
			let shared = self.shared.clone();
			self.shared
				.runtime
				.spawn(async move { load_entry(shared, key, request).await });
		}
		ImageName::of(key)
	}

	pub fn status(&self, name: &str) -> Option<Status> {
		let key = key_of_name(name)?;
		let inner = self.shared.inner.lock();
		Some(match &inner.entries.peek(&key)?.phase {
			Phase::Loading { .. } => Status::Loading,
			Phase::Ready(picture) => Status::Ready {
				frames: picture.frames.len(),
			},
			Phase::Refreshing { .. } => Status::Refreshing,
			Phase::Failed { .. } => Status::Failed,
		})
	}

	pub fn pixels(&self, name: &str) -> Option<Pixels> {
		let key = key_of_name(name)?;
		let inner = self.shared.inner.lock();
		let entry = inner.entries.peek(&key)?;
		let Phase::Ready(picture) = &entry.phase else {
			return None;
		};
		let frame = entry
			.shown
			.unwrap_or(entry.frame)
			.min(picture.frames.len().checked_sub(1)?);
		Some(picture.frames[frame].0.clone())
	}
	pub fn set_visible<I, S>(&self, names: I)
	where
		I: IntoIterator<Item = S>,
		S: AsRef<str>,
	{
		{
			let mut inner = self.shared.inner.lock();
			let visible: HashSet<u64> = names
				.into_iter()
				.filter_map(|name| key_of_name(name.as_ref()))
				.collect();
			for key in &visible {
				inner.entries.promote(key);
			}
			inner.visible = visible;
			inner.evict();
		}
		self.changed();
	}

	pub fn set_hovered(&self, name: Option<&str>) {
		self.shared.inner.lock().hovered = name.and_then(key_of_name);
		self.changed();
	}

	pub fn set_playback(&self, playback: Playback) {
		self.set_animations(Animations::uniform(playback));
	}

	pub fn set_animations(&self, animations: Animations) {
		self.shared.inner.lock().playback = animations;
		self.changed();
	}

	pub fn set_reduced_motion(&self, reduced: bool) {
		self.shared.inner.lock().reduced_motion = reduced;
		self.changed();
	}

	pub fn set_budget(&self, budget: usize) {
		{
			let mut inner = self.shared.inner.lock();
			inner.budget = budget;
			inner.evict();
		}
		self.changed();
	}

	pub fn stats(&self) -> GalleryStats {
		let inner = self.shared.inner.lock();
		GalleryStats {
			entries: inner.entries.len(),
			bytes: inner.bytes,
			budget: inner.budget,
		}
	}

	pub fn drain(&self, now: Instant) -> Vec<Update> {
		let (updates, animating) = {
			let mut inner = self.shared.inner.lock();
			inner.advance(now);
			(inner.take_updates(), !inner.animating.is_empty())
		};
		if animating {
			self.shared.ticker.notify_one();
		}
		updates
	}

	pub fn next_due(&self) -> Option<Instant> {
		self.shared.inner.lock().next_due()
	}

	fn changed(&self) {
		self.shared.ticker.notify_one();
		self.shared.wake();
	}
}

impl Shared {
	fn wake(&self) {
		let waker = self.waker.read().clone();
		if let Some(waker) = waker {
			waker();
		}
	}

	fn refresh(&self, origin: Origin) {
		let refresher = self.refresher.read().clone();
		if let Some(refresher) = refresher {
			refresher(origin);
		}
	}

	fn finish(&self, key: u64, url: &str, decoded: Decoded) {
		{
			let mut inner = self.inner.lock();
			let cap = inner.budget / ANIMATION_SHARE;
			let Some(entry) = inner.entries.peek_mut(&key) else {
				return;
			};
			if !entry.loading(url) {
				return;
			}
			let picture = Picture::from_decoded(decoded, cap);
			let bytes = picture.bytes;
			entry.phase = Phase::Ready(picture);
			entry.refreshed = false;
			entry.frame = 0;
			entry.shown = None;
			entry.due = None;
			entry.plays = 0;
			inner.bytes += bytes;
			inner.dirty.push(key);
			inner.evict();
		}
		self.ticker.notify_one();
		self.wake();
	}

	fn refresh_or_fail(&self, key: u64, url: &str) {
		let origin = {
			let mut inner = self.inner.lock();
			let Some(entry) = inner.entries.peek_mut(&key) else {
				return;
			};
			if !entry.loading(url) {
				return;
			}
			match entry.origin {
				Some(origin) if !entry.refreshed => {
					entry.refreshed = true;
					entry.phase = Phase::Refreshing {
						stale: url.to_owned(),
					};
					Some(origin)
				}
				_ => {
					entry.phase = Phase::Failed { at: Instant::now() };
					None
				}
			}
		};
		if let Some(origin) = origin {
			self.refresh(origin);
		}
		self.wake();
	}

	fn fail(&self, key: u64, url: &str) {
		{
			let mut inner = self.inner.lock();
			if let Some(entry) = inner.entries.peek_mut(&key)
				&& entry.loading(url)
			{
				entry.phase = Phase::Failed { at: Instant::now() };
			}
		}
		self.wake();
	}
}

async fn load_entry(shared: Arc<Shared>, key: u64, request: ImageRequest) {
	let url = request.url.as_str();
	let source = if signed::is_expired(url, SystemTime::now()) {
		Source::DiskOnly
	} else {
		Source::Anywhere
	};
	match shared.loader.load_variant(url, request.variant, source).await {
		Ok(Some(decoded)) => shared.finish(key, url, decoded),
		Ok(None) => shared.refresh_or_fail(key, url),
		Err(error) if error.is_gone() && signed::is_signed(url) => shared.refresh_or_fail(key, url),
		Err(error) => {
			tracing::debug!(url = redact_url(url), %error, "image did not load");
			shared.fail(key, url);
		}
	}
}

async fn tick(shared: Weak<Shared>, ticker: Arc<Notify>) {
	loop {
		let notified = ticker.notified();
		tokio::pin!(notified);
		notified.as_mut().enable();
		let due = match shared.upgrade() {
			Some(shared) => shared.inner.lock().next_due(),
			None => return,
		};
		match due {
			Some(due) => {
				tokio::select! {
					() = tokio::time::sleep_until(due.into()) => {
						match shared.upgrade() {
							Some(shared) => shared.wake(),
							None => return,
						}
						notified.as_mut().await;
					}
					() = &mut notified => {}
				}
			}
			None => notified.await,
		}
	}
}
