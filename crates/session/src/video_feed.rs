use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use protocol::Snowflake;
use voice::video::{Picture, VideoFrame, VideoSink, VideoStream, VideoWant};

pub const FOCUSED_QUALITY: u8 = 100;
pub const THUMBNAIL_QUALITY: u8 = 50;
pub const FOCUSED_MAX_FPS: u32 = 30;
pub const THUMBNAIL_MAX_FPS: u32 = 15;
const FRAME_GAP_SLACK: f64 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoView {
	pub user_id: Snowflake,
	pub focused: bool,
	pub width: u32,
	pub height: u32,
}

impl VideoView {
	pub fn want(&self) -> VideoWant {
		VideoWant {
			user_id: self.user_id,
			quality: if self.focused {
				FOCUSED_QUALITY
			} else {
				THUMBNAIL_QUALITY
			},
			pixels: u64::from(self.width) * u64::from(self.height),
		}
	}

	fn frame_gap(&self) -> Duration {
		let fps = if self.focused {
			FOCUSED_MAX_FPS
		} else {
			THUMBNAIL_MAX_FPS
		};
		(Duration::from_secs(1) / fps).mul_f64(FRAME_GAP_SLACK)
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoUpdate {
	Show { user_id: Snowflake, picture: Picture },
	Clear { user_id: Snowflake },
}

#[derive(Default)]
struct Tile {
	decoded: u64,
	source: Option<(u32, u32)>,
	view: Option<VideoView>,
	shown_at: Option<Instant>,
}

type Waker = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Inner {
	tiles: HashMap<Snowflake, Tile>,
	pending: Vec<VideoUpdate>,
	waker: Option<Waker>,
}

impl Inner {
	fn post(&mut self, update: VideoUpdate) -> Option<Waker> {
		let user_id = update.user_id();
		self.pending.retain(|pending| pending.user_id() != user_id);
		self.pending.push(update);
		self.waker.clone()
	}
}

#[derive(Clone, Default)]
pub struct VideoFeed(Arc<Mutex<Inner>>);

impl VideoFeed {
	pub fn same(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.0, &other.0)
	}

	pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
		self.0.lock().waker = Some(Arc::new(waker));
	}

	pub fn drain(&self) -> Vec<VideoUpdate> {
		std::mem::take(&mut self.0.lock().pending)
	}

	pub fn decoded(&self, user_id: Snowflake) -> u64 {
		self.0.lock().tiles.get(&user_id).map_or(0, |tile| tile.decoded)
	}

	pub fn source_size(&self, user_id: Snowflake) -> Option<(u32, u32)> {
		self.0.lock().tiles.get(&user_id)?.source
	}

	pub(crate) fn set_views(&self, views: &[VideoView]) {
		let mut inner = self.0.lock();
		for (user_id, tile) in &mut inner.tiles {
			tile.view = views.iter().find(|view| view.user_id == *user_id).copied();
		}
		for view in views {
			inner.tiles.entry(view.user_id).or_default().view = Some(*view);
		}
	}

	pub(crate) fn sink(&self) -> Arc<dyn VideoSink> {
		Arc::new(FeedSink(self.clone()))
	}

	pub(crate) fn clear(&self) {
		let users: Vec<Snowflake> = self.0.lock().tiles.keys().copied().collect();
		for user_id in users {
			self.end(user_id);
		}
	}

	fn end(&self, user_id: Snowflake) {
		let waker = {
			let mut inner = self.0.lock();
			let Some(tile) = inner.tiles.get_mut(&user_id) else {
				return;
			};
			tile.decoded = 0;
			tile.source = None;
			tile.shown_at = None;
			inner.post(VideoUpdate::Clear { user_id })
		};
		if let Some(waker) = waker {
			waker();
		}
	}

	fn accept(&self, user_id: Snowflake, frame: &VideoFrame) {
		let now = Instant::now();
		let view = {
			let mut inner = self.0.lock();
			let tile = inner.tiles.entry(user_id).or_default();
			tile.decoded += 1;
			tile.source = Some((frame.width, frame.height));
			let Some(view) = tile.view else {
				return;
			};
			if tile
				.shown_at
				.is_some_and(|shown| now.duration_since(shown) < view.frame_gap())
			{
				return;
			}
			tile.shown_at = Some(now);
			view
		};
		let Ok(picture) = Picture::from_nv12(frame, view.width, view.height) else {
			return;
		};
		let waker = {
			let mut inner = self.0.lock();
			inner.post(VideoUpdate::Show { user_id, picture })
		};
		if let Some(waker) = waker {
			waker();
		}
	}
}

impl VideoUpdate {
	pub fn user_id(&self) -> Snowflake {
		match self {
			Self::Show { user_id, .. } | Self::Clear { user_id } => *user_id,
		}
	}
}

struct FeedSink(VideoFeed);

impl VideoSink for FeedSink {
	fn frame(&self, stream: VideoStream, frame: Arc<VideoFrame>) {
		self.0.accept(stream.user_id, &frame);
	}

	fn ended(&self, user_id: Snowflake) {
		self.0.end(user_id);
	}
}
