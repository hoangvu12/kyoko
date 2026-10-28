use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use bridge::{Inbox, ToastKind};
use session::{AudioDevices, Command, DeviceInfo};
use state::{MIC_TEST_FLOOR_DB, State};
use voice::video::{CameraInfo, CameraPreview, Picture, TestPattern, VideoError, VideoQuality, VideoSource};
use zgx::image::ImageData;
use zgx::input::{EventPhase, InputBinding, InputEvent, Modifiers, PointerButton};
use zgx::widgets::{Ui, WeakUi};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, record};
use crate::capture;

const VOICE_PAGE: &str = "voice";
const MOCK_MICROPHONE: (&str, &str) = ("mock-microphone", "Mock Microphone");
const MOCK_SPEAKERS: (&str, &str) = ("mock-speakers", "Mock Speakers");
const SIDE_BUTTON_KEY: &str = "keybinds.side_button";
const MOCK_CAMERA: (&str, &str) = ("mock-camera", "Mock Camera");
const PREVIEW_IMAGE: &str = "video/preview";
const PREVIEW_WIDTH: u32 = 640;
const PREVIEW_HEIGHT: u32 = 360;
const PREVIEW_FAILED: &str = "Couldn't start your camera.";
const PREVIEW_CAPTURE_FRAMES: u64 = 3;

enum PreviewWake {
	Started(u64, Result<CameraPreview, VideoError>),
	Frame(u64, Picture),
}

pub(super) struct VoiceSettingsView {
	devices: Shared,
	mic_test: Shared,
	side_button: Shared,
	side_listener: RefCell<Option<InputBinding>>,
	found: Inbox<AudioDevices>,
	listing: Cell<bool>,
	shown: RefCell<Option<Option<f32>>>,
	cameras: Shared,
	camera_list: Inbox<Vec<CameraInfo>>,
	listing_cameras: Cell<bool>,
	previewing: Shared,
	running: Cell<bool>,
	preview: RefCell<Option<CameraPreview>>,
	preview_wakes: Inbox<PreviewWake>,
	preview_generation: Cell<u64>,
	preview_frames: Cell<u64>,
}

impl VoiceSettingsView {
	pub(super) fn new(host: &ScriptHost) -> Self {
		Self {
			devices: host.shared("voice.devices", devices_value(&AudioDevices::default())),
			mic_test: host.shared("voice.mic_test", mic_test_value(None)),
			side_button: host.shared(SIDE_BUTTON_KEY, Value::Nil),
			side_listener: RefCell::new(None),
			found: Inbox::new(),
			listing: Cell::new(false),
			shown: RefCell::new(None),
			cameras: host.shared("voice.cameras", camera_list(&[])),
			camera_list: Inbox::new(),
			listing_cameras: Cell::new(false),
			previewing: host.shared("voice.camera_preview", Value::Bool(false)),
			running: Cell::new(false),
			preview: RefCell::new(None),
			preview_wakes: Inbox::new(),
			preview_generation: Cell::new(0),
			preview_frames: Cell::new(0),
		}
	}

	pub(super) fn close(&self) {
		self.preview.borrow_mut().take();
		self.camera_list.close();
		self.preview_wakes.close();
	}

	fn publish_mic_test(&self, level_db: Option<f32>) {
		if self.shown.borrow().as_ref() == Some(&level_db) {
			return;
		}
		self.mic_test.set(mic_test_value(level_db));
		*self.shown.borrow_mut() = Some(level_db);
	}
}

pub(super) fn listen_for_side_buttons(ui: &Ui, pressed: Shared) -> InputBinding {
	let scope = ui.input.focus_scope().unwrap_or_else(|| ui.scene.borrow().root());
	let serial = Cell::new(0.);
	ui.input.listen(scope, move |event| {
		if event.phase != EventPhase::Capture {
			return;
		}
		let InputEvent::PointerDown {
			button: PointerButton::Other(number),
			..
		} = event.event
		else {
			return;
		};
		let Some(binding) = side_binding(number, event.pointer_modifiers()) else {
			return;
		};
		serial.set(serial.get() + 1.);
		pressed.set(record([
			("binding", binding.as_str().into()),
			("serial", serial.get().into()),
		]));
	})
}

fn side_binding(number: u16, held: Modifiers) -> Option<String> {
	let button = match number {
		4 => "Mouse4",
		5 => "Mouse5",
		_ => return None,
	};
	let modifiers = [
		(held.control, "ctrl"),
		(held.alt, "alt"),
		(held.shift, "shift"),
		(held.meta, "super"),
	];
	let parts: Vec<&str> = modifiers
		.into_iter()
		.filter_map(|(held, name)| held.then_some(name))
		.chain([button])
		.collect();
	Some(parts.join("+"))
}

fn device_list(devices: &[DeviceInfo]) -> Value {
	Value::List(
		devices
			.iter()
			.map(|device| {
				record([
					("id", device.id.as_str().into()),
					("name", device.name.as_str().into()),
				])
			})
			.collect(),
	)
}

fn devices_value(devices: &AudioDevices) -> Value {
	record([
		("inputs", device_list(&devices.inputs)),
		("outputs", device_list(&devices.outputs)),
	])
}

fn mic_test_value(level_db: Option<f32>) -> Value {
	let level = level_db.map_or(0., |level| {
		f64::from((level - MIC_TEST_FLOOR_DB) / -MIC_TEST_FLOOR_DB).clamp(0., 1.)
	});
	record([("running", level_db.is_some().into()), ("level", level.into())])
}

fn camera_list(cameras: &[CameraInfo]) -> Value {
	Value::List(
		cameras
			.iter()
			.map(|camera| {
				record([
					("id", camera.id.as_str().into()),
					("name", camera.name.as_str().into()),
				])
			})
			.collect(),
	)
}

fn mock_camera() -> CameraInfo {
	CameraInfo {
		id: MOCK_CAMERA.0.to_owned(),
		name: MOCK_CAMERA.1.to_owned(),
	}
}

fn mock_devices() -> AudioDevices {
	let device = |(id, name): (&str, &str)| DeviceInfo {
		id: id.to_owned(),
		name: name.to_owned(),
		is_default: true,
	};
	AudioDevices {
		inputs: vec![device(MOCK_MICROPHONE)],
		outputs: vec![device(MOCK_SPEAKERS)],
	}
}

impl Link {
	pub(super) fn install_voice_settings(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("voice.mic_test", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.send(if matches!(argument, Value::Bool(true)) {
					Command::StartMicTest
				} else {
					Command::StopMicTest
				});
			}
		});
		let listens = Rc::downgrade(link);
		host.action("keybinds.side_buttons", move |argument| {
			if let Some(link) = listens.upgrade() {
				link.listen_side_buttons(matches!(argument, Value::Bool(true)));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.camera_preview", move |argument| {
			if let Some(link) = actions.upgrade() {
				if matches!(argument, Value::Bool(true)) {
					link.start_camera_preview();
				} else {
					link.stop_camera_preview();
				}
			}
		});
	}

	fn listen_side_buttons(&self, listening: bool) {
		let ui = listening
			.then(|| self.ui.borrow().as_ref().and_then(WeakUi::upgrade))
			.flatten();
		let listener = ui.map(|ui| listen_for_side_buttons(&ui, self.voice_settings.side_button.clone()));
		*self.voice_settings.side_listener.borrow_mut() = listener;
	}

	fn start_camera_preview(&self) {
		let view = &self.voice_settings;
		if view.running.replace(true) {
			return;
		}
		let generation = view.preview_generation.get() + 1;
		view.preview_generation.set(generation);
		view.previewing.set(Value::Bool(true));
		let quality = VideoQuality {
			width: PREVIEW_WIDTH,
			height: PREVIEW_HEIGHT,
			..VideoQuality::default()
		};
		let device = self.settings.settings().voice.camera_device;
		let mock = self.images.mock_origin.is_some();
		let wakes = view.preview_wakes.clone();
		self.plugins.runtime().spawn_blocking(move || {
			let source: Result<Box<dyn VideoSource>, VideoError> = if mock {
				Ok(Box::new(TestPattern::new(quality)))
			} else {
				voice::video::open_camera(device.as_deref(), quality)
			};
			let frames = wakes.clone();
			let started = source.and_then(|source| {
				CameraPreview::start(source, move |frame| {
					if let Ok(picture) = Picture::from_nv12(&frame, PREVIEW_WIDTH, PREVIEW_HEIGHT) {
						frames.push(PreviewWake::Frame(generation, picture));
					}
				})
			});
			wakes.push(PreviewWake::Started(generation, started));
		});
	}

	pub(super) fn stop_camera_preview(&self) {
		let view = &self.voice_settings;
		view.preview_generation.set(view.preview_generation.get() + 1);
		view.preview.borrow_mut().take();
		if view.running.replace(false) {
			view.previewing.set(Value::Bool(false));
			self.host.image(PREVIEW_IMAGE, None);
		}
	}

	pub(super) async fn follow_camera_preview(self: Rc<Self>) {
		while let Some(wakes) = self.voice_settings.preview_wakes.next().await {
			let current = self.voice_settings.preview_generation.get();
			let mut latest = None;
			for wake in wakes {
				match wake {
					PreviewWake::Started(generation, started) if generation == current => match started {
						Ok(preview) => *self.voice_settings.preview.borrow_mut() = Some(preview),
						Err(error) => {
							tracing::warn!(%error, "the camera preview did not start");
							self.stop_camera_preview();
							self.toasts.push(ToastKind::Error, PREVIEW_FAILED);
						}
					},
					PreviewWake::Frame(generation, picture) if generation == current => {
						latest = Some(picture)
					}
					PreviewWake::Started(..) | PreviewWake::Frame(..) => {}
				}
			}
			if let Some(picture) = latest {
				match ImageData::new(picture.width, picture.height, picture.rgba) {
					Ok(image) => {
						self.voice_settings
							.preview_frames
							.set(self.voice_settings.preview_frames.get() + 1);
						self.host.image(PREVIEW_IMAGE, Some(Arc::new(image)));
					}
					Err(error) => tracing::warn!(error, "a preview frame was not usable"),
				}
			}
		}
	}

	pub(super) async fn drive_settings_camera(&self) {
		self.open_settings(VOICE_PAGE);
		self.start_camera_preview();
		self.until_capture(|link| link.voice_settings.preview_frames.get() >= PREVIEW_CAPTURE_FRAMES)
			.await;
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) else {
			tracing::warn!("the window is gone");
			return;
		};
		let (x, y) = capture::SETTINGS_CONTENT;
		ui.dispatch(InputEvent::PointerMove { x, y });
		ui.dispatch(InputEvent::Scroll {
			x,
			y,
			delta_x: 0.,
			delta_y: capture::SETTINGS_VIDEO_SCROLL,
		});
		zgx::timer::sleep(capture::SETTLE).await;
		tracing::info!("the camera preview is showing for the capture");
	}

	pub(super) async fn follow_cameras(self: Rc<Self>) {
		while let Some(batch) = self.voice_settings.camera_list.next().await {
			self.voice_settings.listing_cameras.set(false);
			if let Some(cameras) = batch.last() {
				self.voice_settings.cameras.set(camera_list(cameras));
			}
		}
	}

	fn list_cameras(&self) {
		if self.images.mock_origin.is_some() {
			self.voice_settings.cameras.set(camera_list(&[mock_camera()]));
			return;
		}
		if self.voice_settings.listing_cameras.replace(true) {
			return;
		}
		let found = self.voice_settings.camera_list.clone();
		self.plugins
			.runtime()
			.spawn_blocking(move || found.push(voice::video::cameras().unwrap_or_default()));
	}

	pub(super) fn voice_settings_apply(&self, state: &State) {
		self.voice_settings
			.publish_mic_test(state.mic_test().map(|test| test.level_db));
	}

	pub(super) fn voice_page_shown(&self, page: &str) {
		if page == VOICE_PAGE {
			self.list_audio_devices();
			self.list_cameras();
		} else {
			self.stop_mic_test();
			self.stop_camera_preview();
		}
	}

	pub(super) fn stop_mic_test(&self) {
		let testing = self
			.state()
			.is_some_and(|shared| shared.read().mic_test().is_some());
		if testing {
			self.send(Command::StopMicTest);
		}
	}

	fn list_audio_devices(&self) {
		if self.images.mock_origin.is_some() {
			self.voice_settings.devices.set(devices_value(&mock_devices()));
			return;
		}
		if self.voice_settings.listing.replace(true) {
			return;
		}
		let found = self.voice_settings.found.clone();
		self.plugins
			.runtime()
			.spawn_blocking(move || found.push(session::audio_devices()));
	}

	pub(super) async fn follow_audio_devices(self: Rc<Self>) {
		while let Some(batch) = self.voice_settings.found.next().await {
			self.voice_settings.listing.set(false);
			if let Some(devices) = batch.last() {
				self.voice_settings.devices.set(devices_value(devices));
			}
		}
	}
}
