use std::time::{Duration, Instant};

use mockcord::{MockConfig, Server, VOICE_TOKEN, VoiceControl, VoiceRecording};
use net::Trust;
use protocol::{Snowflake, Token};
use tokio::sync::mpsc;
use voice::{AudioIo, VoiceConnection, VoiceEvent, VoiceOptions, VoiceParams};

pub const GUILD_ID: Snowflake = Snowflake(41_771_983_423_143_937);
pub const CHANNEL_ID: Snowflake = Snowflake(127_121_515_262_115_840);
pub const USER_ID: Snowflake = Snowflake(104_694_319_306_248_192);
pub const SESSION_ID: &str = "30f32c5d54ae86130fc4a215c7474263";
pub const LOCALE: &str = "en-US";

const PATIENCE: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(20);

pub struct Call {
	pub server: Server,
	pub voice: VoiceConnection,
	pub events: mpsc::Receiver<VoiceEvent>,
}

impl Call {
	pub async fn start(setup: impl FnOnce(&VoiceControl)) -> Call {
		let server = Server::start(MockConfig::default())
			.await
			.expect("mockcord starts");
		setup(&server.voice());
		let voice = VoiceConnection::connect(params(&server), options(&server))
			.await
			.expect("the voice connection comes up");
		let events = voice.events();
		Call {
			server,
			voice,
			events,
		}
	}

	pub fn control(&self) -> VoiceControl {
		self.server.voice()
	}

	pub fn recording(&self) -> VoiceRecording {
		self.control().recording()
	}

	pub async fn until(&self, what: &str, done: impl Fn(&VoiceRecording) -> bool) -> VoiceRecording {
		let deadline = Instant::now() + PATIENCE;
		loop {
			let recording = self.recording();
			if done(&recording) {
				return recording;
			}
			assert!(Instant::now() < deadline, "timed out waiting for {what}");
			tokio::time::sleep(POLL).await;
		}
	}

	pub async fn event(&mut self, what: &str, wanted: impl Fn(&VoiceEvent) -> bool) -> VoiceEvent {
		let found = tokio::time::timeout(PATIENCE, async {
			loop {
				let event = self.events.recv().await.expect("the event stream stays open");
				if wanted(&event) {
					return event;
				}
			}
		})
		.await;
		found.unwrap_or_else(|_| panic!("timed out waiting for {what}"))
	}

	pub async fn stop(self) {
		self.voice.disconnect();
		drop(self.voice);
		self.server.shutdown().await;
	}
}

pub fn params(server: &Server) -> VoiceParams {
	VoiceParams {
		guild_id: Some(GUILD_ID),
		channel_id: CHANNEL_ID,
		user_id: USER_ID,
		session_id: SESSION_ID.to_owned(),
		token: Token::new(VOICE_TOKEN),
		endpoint: server.voice_url().to_owned(),
	}
}

pub fn options(server: &Server) -> VoiceOptions {
	VoiceOptions {
		audio: AudioIo::Manual,
		trust: Trust::only(
			server
				.voice()
				.root_pem()
				.expect("mockcord's voice server runs on tls"),
		),
		user_agent: protocol::fingerprint::user_agent(),
		locale: LOCALE.to_owned(),
	}
}

pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
	headers
		.iter()
		.find(|(key, _)| key.eq_ignore_ascii_case(name))
		.map(|(_, value)| value.as_str())
}
