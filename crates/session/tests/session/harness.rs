use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::DateTime;
use mockcord::{Control, Fixtures, MockConfig, Recording, Server};
use protocol::Opcode;
use protocol::fingerprint::Clock;
use protocol::{ChannelType, Snowflake, Token};
use session::{
	AlertPacing, CacheLimits, Command, GatewayTuning, RestTuning, Retention, Session, SessionConfig,
	SessionHooks,
};
use state::State;
use store::{MemoryKeyring, Paths, Settings, Store};
use tokio::sync::Notify;
use tokio::time::{Instant, timeout_at};
use url::Url;

pub const STEP_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
pub const LOCALE: &str = "en-US";
const FIXTURE_ANCHOR_MS: i64 = 1_767_225_600_000;

type Prepare = Box<dyn FnOnce(&Control)>;
type Seed = Box<dyn FnOnce(&Paths)>;

pub struct Options {
	name: &'static str,
	mock: MockConfig,
	hooks: Option<Arc<dyn SessionHooks>>,
	gateway: GatewayTuning,
	rest: RestTuning,
	caches: CacheLimits,
	alerts: AlertPacing,
	prepare: Option<Prepare>,
	seed: Option<Seed>,
	remember_account: bool,
	plaintext_secrets: bool,
	retention: Retention,
	keyring: Arc<MemoryKeyring>,
	clock: Clock,
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
			caches: CacheLimits::default(),
			alerts: AlertPacing::default(),
			prepare: None,
			seed: None,
			remember_account: true,
			plaintext_secrets: false,
			retention: Retention::default(),
			keyring: Arc::new(MemoryKeyring::new()),
			clock: Clock::system(),
		}
	}

	pub fn clock(mut self, clock: Clock) -> Self {
		self.clock = clock;
		self
	}

	pub fn plaintext_secrets(mut self, plaintext: bool) -> Self {
		self.plaintext_secrets = plaintext;
		self
	}

	pub fn retention(mut self, retention: Retention) -> Self {
		self.retention = retention;
		self
	}

	pub fn keyring(mut self, keyring: Arc<MemoryKeyring>) -> Self {
		self.keyring = keyring;
		self
	}

	pub fn seed(mut self, seed: impl FnOnce(&Paths) + 'static) -> Self {
		self.seed = Some(Box::new(seed));
		self
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

	pub fn guilds(mut self, count: usize) -> Self {
		self.mock.guilds = count;
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

	pub fn members(mut self, per_guild: usize) -> Self {
		self.mock.members_per_guild = per_guild;
		self
	}

	pub fn caches(mut self, limits: CacheLimits) -> Self {
		self.caches = limits;
		self
	}

	pub fn alerts(mut self, pacing: AlertPacing) -> Self {
		self.alerts = pacing;
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
		if let Some(seed) = options.seed {
			seed(&Paths::at(&scratch).expect("scratch paths"));
		}
		let wake = Arc::new(Notify::new());
		let launch = Launch {
			hooks: options.hooks,
			gateway: options.gateway,
			rest: options.rest,
			caches: options.caches,
			alerts: options.alerts,
			remember_account: options.remember_account,
			plaintext_secrets: options.plaintext_secrets,
			retention: options.retention,
			keyring: options.keyring,
			clock: options.clock,
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
		self.restart_after(|_| {}).await;
	}

	pub async fn restart_after(&mut self, between: impl FnOnce(&Control)) {
		self.shutdown_session().await;
		between(&self.mock);
		let (session, store) = open_session(&self.server, &self.scratch, &self.wake, &self.launch);
		self.session = Some(session);
		self.store = store;
	}

	pub async fn restart_with_retention(&mut self, retention: Retention) {
		self.launch.retention = retention;
		self.restart().await;
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

	pub fn keyring(&self) -> &MemoryKeyring {
		&self.launch.keyring
	}

	pub fn scratch(&self) -> &Path {
		&self.scratch
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
	caches: CacheLimits,
	alerts: AlertPacing,
	remember_account: bool,
	plaintext_secrets: bool,
	retention: Retention,
	keyring: Arc<MemoryKeyring>,
	clock: Clock,
}

fn open_session(
	server: &Server,
	scratch: &Path,
	wake: &Arc<Notify>,
	launch: &Launch,
) -> (Session, Arc<Store>) {
	let paths = Paths::at(scratch).expect("scratch paths");
	let mut settings = Settings::load(&paths);
	if settings.storage.plaintext_token != launch.plaintext_secrets {
		settings.storage.plaintext_token = launch.plaintext_secrets;
		settings.save(&paths).expect("settings save");
	}
	let store = Arc::new(Store::open_with(paths, launch.keyring.clone()).expect("store opens"));
	let notifier = wake.clone();
	let session = Session::start(SessionConfig {
		identity: session::client_identity_with_clock(&store, launch.clock.clone()),
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
		caches: launch.caches,
		alerts: launch.alerts,
		retention: launch.retention,
		remember_account: launch.remember_account,
	})
	.expect("session starts");
	(session, store)
}
