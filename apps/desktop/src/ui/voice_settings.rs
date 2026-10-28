use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::Inbox;
use session::{AudioDevices, Command, DeviceInfo};
use state::{MIC_TEST_FLOOR_DB, State};
use zgx::input::{EventPhase, InputBinding, InputEvent, Modifiers, PointerButton};
use zgx::widgets::{Ui, WeakUi};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, record};

const VOICE_PAGE: &str = "voice";
const MOCK_MICROPHONE: (&str, &str) = ("mock-microphone", "Mock Microphone");
const MOCK_SPEAKERS: (&str, &str) = ("mock-speakers", "Mock Speakers");
const SIDE_BUTTON_KEY: &str = "keybinds.side_button";

pub(super) struct VoiceSettingsView {
	devices: Shared,
	mic_test: Shared,
	side_button: Shared,
	side_listener: RefCell<Option<InputBinding>>,
	found: Inbox<AudioDevices>,
	listing: Cell<bool>,
	shown: RefCell<Option<Option<f32>>>,
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
		}
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
	}

	fn listen_side_buttons(&self, listening: bool) {
		let ui = listening
			.then(|| self.ui.borrow().as_ref().and_then(WeakUi::upgrade))
			.flatten();
		let listener = ui.map(|ui| listen_for_side_buttons(&ui, self.voice_settings.side_button.clone()));
		*self.voice_settings.side_listener.borrow_mut() = listener;
	}

	pub(super) fn voice_settings_apply(&self, state: &State) {
		self.voice_settings
			.publish_mic_test(state.mic_test().map(|test| test.level_db));
	}

	pub(super) fn voice_page_shown(&self, page: &str) {
		if page == VOICE_PAGE {
			self.list_audio_devices();
		} else {
			self.stop_mic_test();
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
