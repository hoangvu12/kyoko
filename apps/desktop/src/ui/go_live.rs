use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use bridge::{Inbox, ToastKind};
use session::Command;
use voice::video::{
	CameraPreview, Picture, ScreenKind, ScreenTarget, TestPattern, VideoError, VideoQuality, VideoSource,
};
use zgx::image::ImageData;
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, record};
use crate::capture;

const PREVIEW_IMAGE: &str = "screen/preview";
const PREVIEW_WIDTH: u32 = 640;
const PREVIEW_HEIGHT: u32 = 360;
const THUMBNAIL_WIDTH: u32 = 320;
const THUMBNAIL_HEIGHT: u32 = 180;
const MOCK_THUMBNAIL_STEP: u32 = 24;
const RESOLUTIONS: [&str; 3] = ["720", "1080", "source"];
const FRAME_RATES: [u32; 3] = [15, 30, 60];
const DEFAULT_RESOLUTION: &str = "720";
const DEFAULT_FPS: u32 = 30;
const WINDOW_CLOSED: &str = "The window you picked was closed.";
const SCREEN_GONE: &str = "Couldn't capture that screen.";
const MOCK_SOURCES: [(ScreenKind, &str, u32, u32); 4] = [
	(ScreenKind::Screen, "Screen 1", 1920, 1080),
	(ScreenKind::Window, "Kyoko", 1600, 900),
	(ScreenKind::Window, "Mock Browser", 1280, 800),
	(ScreenKind::Window, "Mock Notes", 960, 720),
];
const CAPTURE_FRAMES: u64 = 3;
const BASE_PIXEL_RATE: f64 = 1280.0 * 720.0 * 30.0;
const BASE_BITRATE: f64 = 2_500_000.0;
const MIN_BITRATE: f64 = 1_000_000.0;
const MAX_BITRATE: f64 = 8_000_000.0;

enum ShareWake {
	Listed(u64, Vec<ScreenTarget>),
	Thumbnail(u64, String, Picture),
	Started(u64, Result<CameraPreview, VideoError>),
	Frame(u64, Picture, (u32, u32)),
	Ended(u64, VideoError),
}

#[derive(Clone, PartialEq)]
struct Previewing {
	key: String,
	name: String,
	resolution: &'static str,
	fps: u32,
	size: Option<(u32, u32)>,
	running: bool,
}

pub(super) struct GoLiveView {
	sources: Shared,
	preview: Shared,
	open: Cell<bool>,
	listed: Cell<bool>,
	targets: RefCell<Vec<ScreenTarget>>,
	thumbnails: RefCell<Vec<String>>,
	listing: Arc<AtomicU64>,
	previewing: RefCell<Option<Previewing>>,
	running: RefCell<Option<CameraPreview>>,
	preview_generation: Cell<u64>,
	frames: Cell<u64>,
	wakes: Inbox<ShareWake>,
}

impl GoLiveView {
	pub(super) fn new(host: &ScriptHost) -> Self {
		Self {
			sources: host.shared("voice.screens", sources_value(false, None)),
			preview: host.shared("voice.screen_preview", preview_value(None)),
			open: Cell::new(false),
			listed: Cell::new(false),
			targets: RefCell::new(Vec::new()),
			thumbnails: RefCell::new(Vec::new()),
			listing: Arc::new(AtomicU64::new(0)),
			previewing: RefCell::new(None),
			running: RefCell::new(None),
			preview_generation: Cell::new(0),
			frames: Cell::new(0),
			wakes: Inbox::new(),
		}
	}

	pub(super) fn close(&self) {
		self.listing.fetch_add(1, Ordering::AcqRel);
		self.running.borrow_mut().take();
		self.wakes.close();
	}

	fn publish_sources(&self) {
		let targets = self.targets.borrow();
		let listed = (self.open.get() && self.listed.get()).then_some(&targets[..]);
		self.sources.set(sources_value(self.open.get(), listed));
	}

	fn publish_preview(&self) {
		self.preview.set(preview_value(self.previewing.borrow().as_ref()));
	}
}

fn key_of(target: &ScreenTarget) -> String {
	let kind = match target.kind {
		ScreenKind::Screen => "screen",
		ScreenKind::Window => "window",
	};
	format!("{kind}-{}", target.id)
}

fn thumbnail_image(key: &str) -> String {
	format!("screen/{key}")
}

fn sources_value(open: bool, targets: Option<&[ScreenTarget]>) -> Value {
	let items = targets.unwrap_or_default().iter().map(|target| {
		let kind = match target.kind {
			ScreenKind::Screen => "screen",
			ScreenKind::Window => "window",
		};
		record([
			("id", key_of(target).as_str().into()),
			("kind", kind.into()),
			("name", target.name.as_str().into()),
			(
				"size",
				format!("{}x{}", target.width, target.height).as_str().into(),
			),
		])
	});
	record([
		("open", open.into()),
		("loading", (open && targets.is_none()).into()),
		("items", Value::List(items.collect())),
	])
}

fn preview_value(previewing: Option<&Previewing>) -> Value {
	let (width, height) = previewing.and_then(|shown| shown.size).unwrap_or((0, 0));
	record([
		("id", previewing.map_or("", |shown| shown.key.as_str()).into()),
		("name", previewing.map_or("", |shown| shown.name.as_str()).into()),
		(
			"resolution",
			previewing
				.map_or(DEFAULT_RESOLUTION, |shown| shown.resolution)
				.into(),
		),
		(
			"fps",
			f64::from(previewing.map_or(DEFAULT_FPS, |shown| shown.fps)).into(),
		),
		("width", f64::from(width).into()),
		("height", f64::from(height).into()),
		("running", previewing.is_some_and(|shown| shown.running).into()),
	])
}

fn mock_targets() -> Vec<ScreenTarget> {
	MOCK_SOURCES
		.iter()
		.enumerate()
		.map(|(index, (kind, name, width, height))| ScreenTarget {
			kind: *kind,
			id: index as u64 + 1,
			name: (*name).to_owned(),
			width: *width,
			height: *height,
		})
		.collect()
}

fn largest(resolution: &str, target: &ScreenTarget) -> (u32, u32) {
	match resolution {
		"1080" => (1920, 1080),
		"source" => (target.width, target.height),
		_ => (1280, 720),
	}
}

fn stream_quality(resolution: &str, target: &ScreenTarget, fps: u32) -> VideoQuality {
	let (width, height) = largest(resolution, target);
	let (sent_width, sent_height) = voice::video::fit(target.width, target.height, width, height);
	let pixel_rate = f64::from(sent_width) * f64::from(sent_height) * f64::from(fps);
	let bitrate = (BASE_BITRATE * pixel_rate / BASE_PIXEL_RATE).clamp(MIN_BITRATE, MAX_BITRATE);
	VideoQuality {
		width,
		height,
		fps,
		bitrate: bitrate.round() as u32,
	}
}

fn mock_frame_size(resolution: &str, target: &ScreenTarget) -> (u32, u32) {
	let (width, height) = largest(resolution, target);
	voice::video::fit(target.width, target.height, width, height)
}

fn mock_thumbnail(target: &ScreenTarget) -> Result<Picture, VideoError> {
	let (width, height) = voice::video::fit(target.width, target.height, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
	let mut pattern = TestPattern::new(VideoQuality {
		width,
		height,
		fps: u32::MAX,
		..VideoQuality::default()
	});
	let mut frame = pattern.next_frame()?;
	for _ in 0..target.id as u32 * MOCK_THUMBNAIL_STEP {
		frame = pattern.next_frame()?;
	}
	Picture::from_nv12(&frame, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT)
}

fn list_sources(mock: bool, generation: u64, listing: &AtomicU64, wakes: &Inbox<ShareWake>) {
	let targets = if mock {
		mock_targets()
	} else {
		voice::video::screens().unwrap_or_default()
	};
	wakes.push(ShareWake::Listed(generation, targets.clone()));
	for target in &targets {
		if listing.load(Ordering::Acquire) != generation {
			return;
		}
		let picture = if mock {
			mock_thumbnail(target)
		} else {
			voice::video::screen_thumbnail(target, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT)
				.and_then(|frame| Picture::from_nv12(&frame, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT))
		};
		match picture {
			Ok(picture) => wakes.push(ShareWake::Thumbnail(generation, key_of(target), picture)),
			Err(error) => tracing::debug!(%error, "no thumbnail for a share source"),
		}
	}
}

fn choice(argument: &Value) -> Option<(String, &'static str, u32)> {
	let key = match field(argument, "id")? {
		Value::Str(key) if !key.is_empty() => key.to_string(),
		_ => return None,
	};
	let resolution = match field(argument, "resolution") {
		Some(Value::Str(wanted)) => RESOLUTIONS
			.into_iter()
			.find(|resolution| *resolution == wanted.as_ref())
			.unwrap_or(DEFAULT_RESOLUTION),
		_ => DEFAULT_RESOLUTION,
	};
	let fps = match field(argument, "fps") {
		Some(Value::Num(wanted)) => FRAME_RATES
			.into_iter()
			.find(|fps| f64::from(*fps) == *wanted)
			.unwrap_or(DEFAULT_FPS),
		_ => DEFAULT_FPS,
	};
	Some((key, resolution, fps))
}

fn image_of(picture: Picture) -> Option<Arc<ImageData>> {
	match ImageData::new(picture.width, picture.height, picture.rgba) {
		Ok(image) => Some(Arc::new(image)),
		Err(error) => {
			tracing::warn!(error, "a screen frame was not usable");
			None
		}
	}
}

impl Link {
	pub(super) fn install_go_live(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("voice.go_live", move |argument| {
			if let Some(link) = actions.upgrade() {
				if matches!(argument, Value::Bool(true)) {
					link.open_go_live();
				} else {
					link.close_go_live();
				}
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.go_live_start", move |argument| {
			if let (Some(link), Some((key, resolution, fps))) = (actions.upgrade(), choice(&argument)) {
				link.start_go_live(&key, resolution, fps);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.stop_go_live", move |_| {
			if let Some(link) = actions.upgrade() {
				link.send(Command::StopGoLive);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.screen_preview", move |argument| {
			if let Some(link) = actions.upgrade() {
				match choice(&argument) {
					Some((key, resolution, fps)) => link.preview_screen(&key, resolution, fps),
					None => link.stop_screen_preview(),
				}
			}
		});
	}

	fn mock_sources(&self) -> bool {
		self.images.mock_origin.is_some()
	}

	pub(super) fn open_go_live(&self) {
		let view = &self.go_live;
		view.open.set(true);
		self.list_share_sources();
	}

	fn list_share_sources(&self) {
		let view = &self.go_live;
		let generation = view.listing.fetch_add(1, Ordering::AcqRel) + 1;
		view.listed.set(false);
		view.targets.borrow_mut().clear();
		view.publish_sources();
		let mock = self.mock_sources();
		let listing = view.listing.clone();
		let wakes = view.wakes.clone();
		self.plugins
			.runtime()
			.spawn_blocking(move || list_sources(mock, generation, &listing, &wakes));
	}

	pub(super) fn close_go_live(&self) {
		let view = &self.go_live;
		if !view.open.replace(false) {
			return;
		}
		view.listing.fetch_add(1, Ordering::AcqRel);
		self.stop_screen_preview();
		for key in view.thumbnails.borrow_mut().drain(..) {
			self.host.image(&thumbnail_image(&key), None);
		}
		view.targets.borrow_mut().clear();
		view.publish_sources();
	}

	pub(super) fn preview_screen(&self, key: &str, resolution: &'static str, fps: u32) {
		let view = &self.go_live;
		if !view.open.get() {
			return;
		}
		let Some(target) = view
			.targets
			.borrow()
			.iter()
			.find(|target| key_of(target) == key)
			.cloned()
		else {
			return;
		};
		let generation = view.preview_generation.get() + 1;
		view.preview_generation.set(generation);
		view.running.borrow_mut().take();
		let same = view
			.previewing
			.borrow()
			.as_ref()
			.is_some_and(|shown| shown.key == key);
		if !same {
			self.host.image(PREVIEW_IMAGE, None);
		}
		*view.previewing.borrow_mut() = Some(Previewing {
			key: key.to_owned(),
			name: target.name.clone(),
			resolution,
			fps,
			size: None,
			running: false,
		});
		view.publish_preview();
		let (width, height) = largest(resolution, &target);
		let quality = VideoQuality {
			width,
			height,
			fps,
			..VideoQuality::default()
		};
		let mock = self.mock_sources();
		let wakes = view.wakes.clone();
		self.plugins.runtime().spawn_blocking(move || {
			let source: Result<Box<dyn VideoSource>, VideoError> = if mock {
				let (width, height) = mock_frame_size(resolution, &target);
				Ok(Box::new(TestPattern::new(VideoQuality {
					width,
					height,
					..quality
				})))
			} else {
				voice::video::open_screen(&target, quality)
			};
			let frames = wakes.clone();
			let ends = wakes.clone();
			let started = source.and_then(|source| {
				CameraPreview::start_until(
					source,
					move |frame| {
						if let Ok(picture) = Picture::from_nv12(&frame, PREVIEW_WIDTH, PREVIEW_HEIGHT) {
							frames.push(ShareWake::Frame(generation, picture, (frame.width, frame.height)));
						}
					},
					move |error| ends.push(ShareWake::Ended(generation, error)),
				)
			});
			wakes.push(ShareWake::Started(generation, started));
		});
	}

	pub(super) fn start_go_live(&self, key: &str, resolution: &'static str, fps: u32) {
		let target = self
			.go_live
			.targets
			.borrow()
			.iter()
			.find(|target| key_of(target) == key)
			.cloned();
		let Some(target) = target else {
			return;
		};
		let quality = stream_quality(resolution, &target, fps);
		self.close_go_live();
		self.send(Command::GoLive {
			source: target,
			quality,
		});
	}

	pub(super) fn stop_screen_preview(&self) {
		let view = &self.go_live;
		view.preview_generation.set(view.preview_generation.get() + 1);
		view.running.borrow_mut().take();
		if view.previewing.borrow_mut().take().is_some() {
			self.host.image(PREVIEW_IMAGE, None);
			view.publish_preview();
		}
	}

	fn screen_preview_ended(&self, error: &VideoError) {
		tracing::info!(%error, "the screen preview ended");
		self.stop_screen_preview();
		let text = if matches!(error, VideoError::Ended) {
			WINDOW_CLOSED
		} else {
			SCREEN_GONE
		};
		self.toasts.push(ToastKind::Error, text);
		if self.go_live.open.get() {
			self.list_share_sources();
		}
	}

	pub(super) async fn follow_go_live(self: Rc<Self>) {
		while let Some(wakes) = self.go_live.wakes.next().await {
			let listing = self.go_live.listing.load(Ordering::Acquire);
			let previewing = self.go_live.preview_generation.get();
			let mut latest = None;
			for wake in wakes {
				match wake {
					ShareWake::Listed(generation, targets) if generation == listing => {
						*self.go_live.targets.borrow_mut() = targets;
						self.go_live.listed.set(true);
						self.go_live.publish_sources();
					}
					ShareWake::Thumbnail(generation, key, picture) if generation == listing => {
						if let Some(image) = image_of(picture) {
							self.host.image(&thumbnail_image(&key), Some(image));
							self.go_live.thumbnails.borrow_mut().push(key);
						}
					}
					ShareWake::Started(generation, started) if generation == previewing => match started {
						Ok(preview) => {
							*self.go_live.running.borrow_mut() = Some(preview);
						}
						Err(error) => self.screen_preview_ended(&error),
					},
					ShareWake::Frame(generation, picture, size) if generation == previewing => {
						latest = Some((picture, size));
					}
					ShareWake::Ended(generation, error) if generation == previewing => {
						latest = None;
						self.screen_preview_ended(&error);
					}
					ShareWake::Listed(..)
					| ShareWake::Thumbnail(..)
					| ShareWake::Started(..)
					| ShareWake::Frame(..)
					| ShareWake::Ended(..) => {}
				}
			}
			if let Some((picture, size)) = latest
				&& let Some(image) = image_of(picture)
			{
				self.host.image(PREVIEW_IMAGE, Some(image));
				self.go_live.frames.set(self.go_live.frames.get() + 1);
				let changed = {
					let mut previewing = self.go_live.previewing.borrow_mut();
					match previewing.as_mut() {
						Some(shown) if shown.size != Some(size) || !shown.running => {
							shown.size = Some(size);
							shown.running = true;
							true
						}
						_ => false,
					}
				};
				if changed {
					self.go_live.publish_preview();
				}
			}
		}
	}

	pub(super) async fn drive_go_live(&self) {
		self.drive_voice_call().await;
		self.open_go_live();
		self.until_capture(|link| link.go_live.thumbnails.borrow().len() >= MOCK_SOURCES.len())
			.await;
		zgx::timer::sleep(capture::SETTLE).await;
		tracing::info!("the go live picker shows for the capture");
	}

	pub(super) async fn drive_go_live_preview(&self) {
		self.drive_go_live().await;
		let first = self.go_live.targets.borrow().first().map(key_of);
		let Some(key) = first else {
			tracing::warn!("no share source to preview");
			return;
		};
		self.preview_screen(&key, DEFAULT_RESOLUTION, DEFAULT_FPS);
		self.until_capture(|link| link.go_live.frames.get() >= CAPTURE_FRAMES)
			.await;
		zgx::timer::sleep(capture::SETTLE).await;
		tracing::info!("the screen preview shows for the capture");
	}

	pub(super) async fn drive_going_live(&self) {
		self.drive_go_live_preview().await;
		let first = self.go_live.targets.borrow().first().map(key_of);
		let Some(key) = first else {
			tracing::warn!("no share source to go live with");
			return;
		};
		self.start_go_live(&key, DEFAULT_RESOLUTION, DEFAULT_FPS);
		self.until_capture(Link::shows_live).await;
		zgx::timer::sleep(capture::SETTLE).await;
		tracing::info!("went live for the capture");
	}
}
