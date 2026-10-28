use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use bridge::Inbox;
use protocol::Snowflake;
use session::{Command, VideoFeed, VideoUpdate, VideoView};
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

enum Wake {
	Frames,
	Available(bool),
}

pub(super) struct VideoSurface {
	wakes: Inbox<Wake>,
	available: Shared,
	feed: RefCell<Option<VideoFeed>>,
	shown: RefCell<HashSet<Snowflake>>,
	reported: RefCell<Vec<Reported>>,
	sent: RefCell<Option<Vec<VideoView>>>,
	frames: Cell<u64>,
}

fn image_name(user_id: Snowflake) -> String {
	format!("video/{user_id}")
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
			feed: RefCell::new(None),
			shown: RefCell::new(HashSet::new()),
			reported: RefCell::new(Vec::new()),
			sent: RefCell::new(None),
			frames: Cell::new(0),
		}
	}

	pub(super) fn close(&self) {
		self.wakes.close();
	}

	fn frames_shown(&self) -> u64 {
		self.frames.get()
	}
}

impl Link {
	pub(super) fn install_video(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("voice.video_views", move |argument| {
			if let Some(link) = actions.upgrade() {
				*link.video.reported.borrow_mut() = reported_views(&argument);
				link.send_video_views();
			}
		});
	}

	pub(super) fn sync_video(&self) {
		let feed = self.flow.borrow().session().map(session::Session::video_feed);
		let attached = self.video.feed.borrow().clone();
		let same = match (&feed, &attached) {
			(Some(feed), Some(attached)) => feed.same(attached),
			(None, None) => true,
			_ => false,
		};
		if !same {
			self.clear_video();
			if let Some(feed) = &feed {
				let wakes = self.video.wakes.clone();
				feed.set_waker(move || wakes.push(Wake::Frames));
			}
			*self.video.feed.borrow_mut() = feed;
			self.video.sent.borrow_mut().take();
		}
		self.send_video_views();
	}

	fn send_video_views(&self) {
		let scale = self.scale_factor();
		let views: Vec<VideoView> = if self.call_open() {
			self.video
				.reported
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
		if self.video.sent.borrow().as_ref() == Some(&views) {
			return;
		}
		*self.video.sent.borrow_mut() = Some(views.clone());
		self.send(Command::SetVideoViews(views));
	}

	fn clear_video(&self) {
		let shown: Vec<Snowflake> = self.video.shown.borrow_mut().drain().collect();
		for user_id in shown {
			self.host.image(&image_name(user_id), None);
		}
	}

	pub(super) async fn follow_video(self: Rc<Self>) {
		while let Some(wakes) = self.video.wakes.next().await {
			for wake in &wakes {
				if let Wake::Available(available) = wake {
					if !available {
						tracing::warn!("video unavailable: no Media Foundation h264 decoder");
					}
					self.video.available.set((*available).into());
				}
			}
			if !wakes.iter().any(|wake| matches!(wake, Wake::Frames)) {
				continue;
			}
			let feed = self.video.feed.borrow().clone();
			let Some(feed) = feed else {
				continue;
			};
			for update in feed.drain() {
				self.show_video(update);
			}
		}
	}

	fn show_video(&self, update: VideoUpdate) {
		match update {
			VideoUpdate::Show { user_id, picture } => {
				match ImageData::new(picture.width, picture.height, picture.rgba) {
					Ok(image) => {
						self.video.shown.borrow_mut().insert(user_id);
						self.video.frames.set(self.video.frames.get() + 1);
						self.host.image(&image_name(user_id), Some(Arc::new(image)));
					}
					Err(error) => tracing::warn!(error, "a video frame was not usable"),
				}
			}
			VideoUpdate::Clear { user_id } => {
				self.video.shown.borrow_mut().remove(&user_id);
				self.host.image(&image_name(user_id), None);
			}
		}
	}
}

impl Link {
	pub(super) async fn drive_voice_video(&self) {
		self.drive_voice_call().await;
		self.until_capture(|link| link.video.frames_shown() >= CAPTURE_FRAMES)
			.await;
		tracing::info!(
			frames = self.video.frames_shown(),
			"camera frames are showing for the capture"
		);
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
