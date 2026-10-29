use std::sync::Arc;
use std::time::Duration;

use state::LocalEvent;
use store::VoiceSettings;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};
use voice::{AudioIo, MicCheck, MicCheckOptions, VoiceError};

use super::voice_link::{suppression, volume};
use super::{Core, Internal};

const LEVEL_INTERVAL: Duration = Duration::from_millis(50);
const MIC_TEST_FAILED: &str = "Couldn't start the mic test. Check your input and output devices.";

#[derive(Default)]
pub(crate) struct MicTestRun {
	generation: u64,
	running: bool,
	check: Option<Arc<MicCheck>>,
	devices: Option<AudioIo>,
	poll: Option<JoinHandle<()>>,
}

impl Core {
	pub(super) fn start_mic_test(&mut self) {
		if self.mic_test.running {
			return;
		}
		self.mic_test.generation += 1;
		self.mic_test.running = true;
		self.apply_local(LocalEvent::MicTestStarted);
		let generation = self.mic_test.generation;
		let internal = self.internal.clone();
		if let Some(connection) = self.voice_connection() {
			self.mic_test.poll = Some(self.runtime.spawn(async move {
				let mut ticker = interval(LEVEL_INTERVAL);
				ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
				loop {
					ticker.tick().await;
					let level_db = connection.input_level_db();
					if internal
						.send(Internal::MicTestLevel { generation, level_db })
						.is_err()
					{
						break;
					}
				}
			}));
			return;
		}
		let settings = self.store.settings.read().voice.clone();
		let audio = self.audio_io();
		self.mic_test.devices = Some(audio.clone());
		let options = MicCheckOptions {
			audio,
			input_volume: volume(settings.input_volume),
			noise_suppression: suppression(&settings),
		};
		self.runtime.spawn_blocking(move || {
			let levels = internal.clone();
			let result = MicCheck::start(options, move |level_db| {
				let _ = levels.send(Internal::MicTestLevel { generation, level_db });
			});
			let _ = internal.send(Internal::MicCheckStarted {
				generation,
				result: result.map(Arc::new),
			});
		});
	}

	pub(super) fn stop_mic_test(&mut self) {
		if !self.mic_test.running {
			return;
		}
		self.mic_test.running = false;
		self.mic_test.generation += 1;
		self.mic_test.devices = None;
		if let Some(poll) = self.mic_test.poll.take() {
			poll.abort();
		}
		*self.voice_shared.mic_check.lock() = None;
		if let Some(check) = self.mic_test.check.take() {
			self.runtime.spawn_blocking(move || check.stop());
		}
		self.apply_local(LocalEvent::MicTestFinished);
	}

	pub(super) fn mic_test_follow(&mut self, settings: &VoiceSettings) {
		if let Some(check) = &self.mic_test.check {
			check.set_input_volume(volume(settings.input_volume));
			check.set_noise_suppression(suppression(settings));
		}
		let moved = self
			.mic_test
			.devices
			.as_ref()
			.is_some_and(|devices| *devices != self.audio_io());
		if moved {
			self.stop_mic_test();
			self.start_mic_test();
		}
	}

	pub(super) fn on_mic_check_started(
		&mut self,
		generation: u64,
		result: Result<Arc<MicCheck>, VoiceError>,
	) {
		if generation != self.mic_test.generation || !self.mic_test.running {
			if let Ok(check) = result {
				self.runtime.spawn_blocking(move || check.stop());
			}
			return;
		}
		match result {
			Ok(check) => {
				*self.voice_shared.mic_check.lock() = Some(check.clone());
				self.mic_test.check = Some(check);
			}
			Err(error) => {
				tracing::warn!(%error, "mic test failed to start");
				self.stop_mic_test();
				self.reject(MIC_TEST_FAILED);
			}
		}
	}

	pub(super) fn on_mic_test_level(&mut self, generation: u64, level_db: f32) {
		if generation == self.mic_test.generation && self.mic_test.running {
			self.apply_local(LocalEvent::MicTestLevel { level_db });
		}
	}
}
