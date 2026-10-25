use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::DateTime;
use mockcord::{Control, Fixtures, MockConfig, Recording, Server};
use protocol::Opcode;
use protocol::{ChannelType, Snowflake, Token};
use session::{Command, GatewayTuning, RestTuning, Session, SessionConfig, SessionHooks};
use state::State;
use store::{Paths, Settings, Store};
use tokio::sync::Notify;
use tokio::time::{Instant, timeout_at};
use url::Url;

pub const STEP_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
pub const LOCALE: &str = "en-US";
const FIXTURE_ANCHOR_MS: i64 = 1_767_225_600_000;

type Prepare = Box<dyn FnOnce(&Control)>;

pub struct Options {
	name: &'static str,
	mock: MockConfig,
	hooks: Option<Arc<dyn SessionHooks>>,
	gateway: GatewayTuning,
	rest: RestTuning,
	prepare: Option<Prepare>,
	remember_account: bool,
}

impl Options {
	pub fn new(name: &'static str) -> Self {
		Self {
			name,
			mock: MockConfig {
				record: true,
				anchor: DateTime::from_timestamp_millis(FIXTURE_ANCHOR_MS),
				..MockConfig::default()
			},
			hooks: None,
			gateway: GatewayTuning::default(),
			rest: RestTuning::default(),
			prepare: None,
			remember_account: true,
		}
	}

	pub fn remember_account(mut self, remember: bool) -> Self {
		self.remember_account = remember;
		self
	}

	pub fn hooks(mut self, hooks: Arc<dyn SessionHooks>) -> Self {
		self.hooks = Some(hooks);
		self
	}

	pub fn heartbeat_ms(mut self, interval: u64) -> Self {
		self.mock.heartbeat_interval_ms = interval;
		self
	}

	pub fn gateway(mut self, tuning: GatewayTuning) -> Self {
		self.gateway = tuning;
		self
	}

	pub fn rest(mut self, tuning: RestTuning) -> Self {
		self.rest = tuning;
		self
	}

	pub fn buckets(mut self, limit: u32, window: Duration) -> Self {
		self.mock.bucket_limit = limit;
		self.mock.bucket_window = window;
		self
	}

	pub fn prepare(mut self, prepare: impl FnOnce(&Control) + 'static) -> Self {
		self.prepare = Some(Box::new(prepare));
		self
	}
}

pub struct Running {
	server: Server,
	mock: Control,
	session: Option<Session>,
	store: Arc<Store>,
	launch: Launch,
	wake: Arc<Notify>,
	scratch: PathBuf,
}

impl Running {
	pub async fn start(name: &'static str) -> Running {
		Self::start_with(Options::new(name)).await
	}

	pub async fn start_with(options: Options) -> Running {
		let server = Server::start(options.mock).await.expect("mock server starts");
		let mock = server.control();
		if let Some(prepare) = options.prepare {
			prepare(&mock);
		}
		let scratch =
			std::env::temp_dir().join(format!("kyoko-session-{}-{}", options.name, std::process::id()));
		let _ = std::fs::remove_dir_all(&scratch);
		let wake = Arc::new(Notify::new());
		let launch = Launch {
			hooks: options.hooks,
			gateway: options.gateway,
			rest: options.rest,
			remember_account: options.remember_account,
		};
		let (session, store) = open_session(&server, &scratch, &wake, &launch);
		Running {
			server,
			mock,
			session: Some(session),
			store,
			launch,
			wake,
			scratch,
		}
	}

	pub async fn restart(&mut self) {
		self.shutdown_session().await;
		let (session, store) = open_session(&self.server, &self.scratch, &self.wake, &self.launch);
		self.session = Some(session);
		self.store = store;
	}

	async fn shutdown_session(&mut self) {
		if let Some(session) = self.session.take() {
			tokio::task::spawn_blocking(move || session.shutdown())
				.await
				.expect("session shuts down");
		}
	}

	pub fn session(&self) -> &Session {
		self.session.as_ref().expect("session is running")
	}

	pub fn mock(&self) -> &Control {
		&self.mock
	}

	pub fn fixtures(&self) -> &Fixtures {
		self.server.fixtures()
	}

	pub fn token(&self) -> &str {
		self.server.token()
	}

	pub async fn until(&self, what: &str, ready: impl Fn(&State) -> bool) {
		let deadline = Instant::now() + STEP_TIMEOUT;
		loop {
			if ready(&self.session().state().read()) {
				return;
			}
			if timeout_at(deadline, self.wake.notified()).await.is_err() {
				panic!("timed out waiting for {what}");
			}
		}
	}

	pub fn store(&self) -> &Store {
		&self.store
	}

	pub async fn eventually(&self, what: &str, done: impl Fn() -> bool) {
		let deadline = Instant::now() + STEP_TIMEOUT;
		while !done() {
			if Instant::now() >= deadline {
				panic!("timed out waiting for {what}");
			}
			tokio::time::sleep(POLL_INTERVAL).await;
		}
	}

	pub async fn until_wire<T>(&self, what: &str, found: impl FnMut(&Recording) -> Option<T>) -> T {
		match self.mock.wait_for(STEP_TIMEOUT, found).await {
			Some(value) => value,
			None => panic!("timed out waiting on the wire for {what}"),
		}
	}

	pub async fn connected(&self) {
		self.until("the gateway to become ready", |state| {
			state.current_user().is_some() && state.session().connected
		})
		.await;
	}

	pub async fn identified_connection(&self) -> u64 {
		self.until_wire("the first IDENTIFY", |recording| {
			recording
				.frames_with(Opcode::Identify)
				.next()
				.map(|frame| frame.connection)
		})
		.await
	}

	pub async fn open_text_channel(&self) -> Snowflake {
		self.connected().await;
		let (guild_id, channel_id) = self
			.session()
			.state()
			.read()
			.guilds()
			.find_map(|guild| {
				guild
					.channels
					.values()
					.find(|channel| channel.kind == ChannelType::GuildText)
					.map(|channel| (guild.id, channel.id))
			})
			.expect("a text channel in the fixtures");
		self.session().send(Command::SelectChannel {
			guild_id: Some(guild_id),
			channel_id,
		});
		self.until("the channel history to load", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
		channel_id
	}

	pub async fn send_and_wait(&self, channel_id: Snowflake, sent: &str, echoed: &str) -> Snowflake {
		self.session().send(Command::SendMessage {
			channel_id,
			content: sent.to_owned(),
			reply_to: None,
			files: Vec::new(),
		});
		self.until("the sent message to arrive", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.pending.is_empty() && cache.ordered.iter().any(|message| message.content == echoed)
			})
		})
		.await;
		self.session()
			.state()
			.read()
			.messages(channel_id)
			.and_then(|cache| {
				cache
					.ordered
					.iter()
					.rev()
					.find(|message| message.content == echoed)
					.map(|message| message.id)
			})
			.expect("the sent message is cached")
	}

	pub async fn stop(mut self) {
		self.shutdown_session().await;
		self.server.shutdown().await;
		let _ = std::fs::remove_dir_all(&self.scratch);
	}
}

struct Launch {
	hooks: Option<Arc<dyn SessionHooks>>,
	gateway: GatewayTuning,
	rest: RestTuning,
	remember_account: bool,
}

fn open_session(
	server: &Server,
	scratch: &Path,
	wake: &Arc<Notify>,
	launch: &Launch,
) -> (Session, Arc<Store>) {
	let paths = Paths::at(scratch).expect("scratch paths");
	let mut settings = Settings::load(&paths);
	settings.storage.plaintext_token = true;
	settings.save(&paths).expect("settings save");
	let store = Arc::new(Store::open(paths).expect("store opens"));
	let notifier = wake.clone();
	let session = Session::start(SessionConfig {
		identity: session::client_identity(&store),
		build_source: Some(Url::parse(&server.app_url()).expect("app url")),
		store: store.clone(),
		rest_base: Url::parse(server.rest_base_url()).expect("rest url"),
		gateway_url: server.gateway_url().to_owned(),
		token: Token::new(server.token()),
		runtime: None,
		notify: Arc::new(move |_| notifier.notify_one()),
		hooks: launch.hooks.clone(),
		gateway: launch.gateway,
		rest: launch.rest,
		remember_account: launch.remember_account,
	})
	.expect("session starts");
	(session, store)
}
