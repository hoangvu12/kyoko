use std::sync::Arc;
use std::time::Duration;

use net::Trust;
use parking_lot::Mutex;
use voice::{AudioIo, VoiceConnection};

const JOIN_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct VoiceSetup {
	pub audio: AudioIo,
	pub trust: Trust,
	pub join_timeout: Duration,
}

impl Default for VoiceSetup {
	fn default() -> Self {
		Self {
			audio: AudioIo::Devices {
				input: None,
				output: None,
			},
			trust: Trust::default(),
			join_timeout: JOIN_TIMEOUT,
		}
	}
}

#[derive(Default)]
pub(crate) struct VoiceShared {
	pub(crate) setup: Mutex<VoiceSetup>,
	pub(crate) connection: Mutex<Option<Arc<VoiceConnection>>>,
}
