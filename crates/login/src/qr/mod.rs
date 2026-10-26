mod crypto;
mod flow;
mod messages;

use rest::ClientIdentity;
use tokio::sync::mpsc::{self, Receiver};
use tokio::task::JoinHandle;
use url::Url;

const EVENT_BUFFER: usize = 8;
pub const REMOTE_AUTH_URL: &str = "wss://remote-auth-gateway.discord.gg/?v=2";
const ORIGIN: &str = "https://discord.com";
const REST_BASE: &str = "https://discord.com/api/v9";

#[derive(Debug, Clone)]
pub struct QrConfig {
	pub gateway_url: Url,
	pub origin: String,
	pub rest_base: Url,
	pub identity: ClientIdentity,
}

impl Default for QrConfig {
	fn default() -> Self {
		Self {
			gateway_url: Url::parse(REMOTE_AUTH_URL).expect("default gateway url is valid"),
			origin: ORIGIN.to_owned(),
			rest_base: Url::parse(REST_BASE).expect("default rest base is valid"),
			identity: ClientIdentity::default(),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scanner {
	pub id: protocol::Snowflake,
	pub discriminator: Option<String>,
	pub avatar: Option<String>,
	pub username: String,
}

#[derive(Debug)]
pub enum QrEvent {
	Code { url: String },
	Scanned(Scanner),
	Token(protocol::Token),
	Expired,
	Cancelled,
	Failed(crate::LoginError),
}

pub struct QrLogin;

impl QrLogin {
	pub fn start(config: QrConfig) -> QrSession {
		let (sender, receiver) = mpsc::channel(EVENT_BUFFER);
		QrSession {
			events: Some(receiver),
			task: tokio::spawn(flow::drive(config, sender)),
		}
	}
}

pub struct QrSession {
	events: Option<Receiver<QrEvent>>,
	task: JoinHandle<()>,
}

impl QrSession {
	pub fn events(&mut self) -> Option<Receiver<QrEvent>> {
		self.events.take()
	}

	pub fn abort(&self) {
		self.task.abort();
	}
}

impl Drop for QrSession {
	fn drop(&mut self) {
		self.task.abort();
	}
}
