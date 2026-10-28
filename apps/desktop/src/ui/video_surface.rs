use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use bridge::Inbox;
use protocol::Snowflake;
use session::{Command, Session, VideoFeed, VideoUpdate, VideoView};
use zgx::image::ImageData;
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, snowflake};

const CAPTURE_FRAMES: u64 = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Reported {
	user_id: Snowflake,
	width: f64,
	height: f64,
	focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
	Camera,
	Stream,
}

impl Source {
	fn image_name(self, user_id: Snowflake) -> String {
		match self {
			Self::Camera => format!("video/{user_id}"),
			Self::Stream => format!("stream/{user_id}"),
		}
	}

	fn feed(self, session: &Session) -> VideoFeed {
		match self {
			Self::Camera => session.video_feed(),
			Self::Stream => session.stream_feed(),
		}
	}

	fn command(self, views: Vec<VideoView>) -> Command {
		match self {
			Self::Camera => Command::SetVideoViews(views),
			Self::Stream => Command::SetStreamViews(views),
		}
	}
}

enum Wake {
	Frames(Source),
	Available(bool),
}

struct Lane {
	source: Source,
	feed: RefCell<Option<VideoFeed>>,
	shown: RefCell<HashSet<Snowflake>>,
	reported: RefCell<Vec<Reported>>,
	sent: RefCell<Option<Vec<VideoView>>>,
	frames: Cell<u64>,
}

impl Lane {
	fn new(source: Source) -> Self {
		Self {
			source,
			feed: RefCell::new(None),
			shown: RefCell::new(HashSet::new()),
			reported: RefCell::new(Vec::new()),
			sent: RefCell::new(None),
			frames: Cell::new(0),
		}
	}
}

pub(super) struct VideoSurface {
	wakes: Inbox<Wake>,
	available: Shared,
	cameras: Lane,
	streams: Lane,
}

impl VideoSurface {
	pub(super) fn new(host: &ScriptHost) -> Self {
		let wakes = Inbox::new();
		let probe = wakes.clone();
		let spawned = std::thread::Builder::new()
			.name("video-probe".to_owned())
			.spawn(move || probe.push(Wake::Available(voice::video::available())));
		if let Err(error) = spawned {
			tracing::warn!(%error, "the video decoder probe did not start");
		}
		Self {
			wakes,
			available: host.shared("voice.video_available", Value::Bool(true)),
			cameras: Lane::new(Source::Camera),
			streams: Lane::new(Source::Stream),
		}
	}

	pub(super) fn close(&self) {
		self.wakes.close();
	}

	fn lane(&self, source: Source) -> &Lane {
		match source {
			Source::Camera => &self.cameras,
			Source::Stream => &self.streams,
		}
	}

	fn lanes(&self) -> [&Lane; 2] {
		[&self.cameras, &self.streams]
	}
}

impl Link {
	pub(super) fn install_video(host: &ScriptHost, link: &Rc<Link>) {
		for (action, source) in [
			("voice.video_views", Source::Camera),
			("voice.stream_views", Source::Stream),
		] {
			let actions = Rc::downgrade(link);
			host.action(action, move |argument| {
				if let Some(link) = actions.upgrade() {
					*link.video.lane(source).reported.borrow_mut() = reported_views(&argument);
					link.send_video_views(link.video.lane(source));
				}
			});
		}
	}

	pub(super) fn sync_video(&self) {
		for lane in self.video.lanes() {
			let feed = self
				.flow
				.borrow()
				.session()
				.map(|session| lane.source.feed(session));
			let attached = lane.feed.borrow().clone();
			let same = match (&feed, &attached) {
				(Some(feed), Some(attached)) => feed.same(attached),
				(None, None) => true,
				_ => false,
			};
			if !same {
				self.clear_video(lane);
				if let Some(feed) = &feed {
					let wakes = self.video.wakes.clone();
					let source = lane.source;
					feed.set_waker(move || wakes.push(Wake::Frames(source)));
				}
				*lane.feed.borrow_mut() = feed;
				lane.sent.borrow_mut().take();
			}
			self.send_video_views(lane);
		}
	}

	fn send_video_views(&self, lane: &Lane) {
		let scale = self.scale_factor();
		let views: Vec<VideoView> = if self.call_open() {
			lane.reported
				.borrow()
				.iter()
				.map(|view| VideoView {
					user_id: view.user_id,
					focused: view.focused,
					width: device_pixels(view.width, scale),
					height: device_pixels(view.height, scale),
				})
				.collect()
		} else {
			Vec::new()
		};
		if lane.sent.borrow().as_ref() == Some(&views) {
			return;
		}
		*lane.sent.borrow_mut() = Some(views.clone());
		self.send(lane.source.command(views));
	}

	fn clear_video(&self, lane: &Lane) {
		let shown: Vec<Snowflake> = lane.shown.borrow_mut().drain().collect();
		for user_id in shown {
			self.host.image(&lane.source.image_name(user_id), None);
		}
	}

	pub(super) async fn follow_video(self: Rc<Self>) {
		while let Some(wakes) = self.video.wakes.next().await {
			let mut woken = Vec::new();
			for wake in &wakes {
				match wake {
					Wake::Available(available) => {
						if !available {
							tracing::warn!("video unavailable: no Media Foundation h264 decoder");
						}
						self.video.available.set((*available).into());
					}
					Wake::Frames(source) if !woken.contains(source) => woken.push(*source),
					Wake::Frames(_) => {}
				}
			}
			for source in woken {
				let lane = self.video.lane(source);
				let feed = lane.feed.borrow().clone();
				let Some(feed) = feed else {
					continue;
				};
				for update in feed.drain() {
					self.show_video(lane, update);
				}
			}
		}
	}

	fn show_video(&self, lane: &Lane, update: VideoUpdate) {
		match update {
			VideoUpdate::Show { user_id, picture } => {
				match ImageData::new(picture.width, picture.height, picture.rgba) {
					Ok(image) => {
						lane.shown.borrow_mut().insert(user_id);
						lane.frames.set(lane.frames.get() + 1);
						self.host
							.image(&lane.source.image_name(user_id), Some(Arc::new(image)));
					}
					Err(error) => tracing::warn!(error, "a video frame was not usable"),
				}
			}
			VideoUpdate::Clear { user_id } => {
				lane.shown.borrow_mut().remove(&user_id);
				self.host.image(&lane.source.image_name(user_id), None);
			}
		}
	}
}

impl Link {
	pub(super) async fn drive_voice_video(&self) {
		self.drive_voice_call().await;
		self.until_capture(|link| link.video.cameras.frames.get() >= CAPTURE_FRAMES)
			.await;
		tracing::info!(
			frames = self.video.cameras.frames.get(),
			"camera frames are showing for the capture"
		);
	}

	pub(super) fn stream_frames_shown(&self) -> u64 {
		self.video.streams.frames.get()
	}

	pub(super) fn stream_view_width(&self) -> f64 {
		self.video
			.streams
			.reported
			.borrow()
			.iter()
			.map(|view| view.width)
			.fold(0., f64::max)
	}
}

fn device_pixels(points: f64, scale: f64) -> u32 {
	(points * scale).ceil().clamp(0., f64::from(u32::MAX)) as u32
}

fn reported_views(argument: &Value) -> Vec<Reported> {
	let Value::List(items) = argument else {
		return Vec::new();
	};
	items
		.iter()
		.filter_map(|item| {
			let user_id = field(item, "id").and_then(snowflake)?;
			let number = |key: &str| match field(item, key) {
				Some(Value::Num(value)) if value.is_finite() && *value > 0. => Some(*value),
				_ => None,
			};
			let focused = matches!(field(item, "focused"), Some(Value::Bool(true)));
			Some(Reported {
				user_id,
				width: number("w")?,
				height: number("h")?,
				focused,
			})
		})
		.collect()
}
