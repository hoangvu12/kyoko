use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bridge::{Inbox, Rail, Sidebar, Timeline, changes_notifier};
use mockcord::{MockConfig, Server};
use protocol::{Snowflake, Token};
use session::{Command, Session, SessionConfig};
use state::{Changes, State};
use store::{Paths, Settings, Store};
use tokio::time::{Instant, timeout_at};
use url::Url;

const STEP_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default)]
pub struct View {
	pub sidebar: Sidebar,
	pub timeline: Timeline,
	pub rail: Rail,
}

pub struct Running {
	pub server: Server,
	pub view: View,
	pub scratch: PathBuf,
	session: Option<Session>,
	inbox: Inbox<Changes>,
}

impl Running {
	pub async fn start(name: &str) -> Running {
		let server = Server::start(MockConfig {
			record: true,
			..MockConfig::default()
		})
		.await
		.expect("mock server starts");
		let scratch = std::env::temp_dir().join(format!("kyoko-bridge-{name}-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&scratch);
		let paths = Paths::at(&scratch).expect("scratch paths");
		let mut settings = Settings::load(&paths);
		settings.storage.plaintext_token = true;
		settings.save(&paths).expect("settings save");
		let store = Store::open(paths).expect("store opens");
		let inbox = Inbox::new();
		let session = Session::start(SessionConfig {
			identity: session::client_identity(&store),
			build_source: None,
			store: Arc::new(store),
			rest_base: Url::parse(server.rest_base_url()).expect("rest url"),
			gateway_url: server.gateway_url().to_owned(),
			token: Token::new(server.token()),
			runtime: None,
			notify: changes_notifier(inbox.clone()),
			hooks: None,
			gateway: Default::default(),
			rest: Default::default(),
			caches: Default::default(),
			alerts: Default::default(),
			retention: Default::default(),
			remember_account: false,
		})
		.expect("session starts");
		let mut running = Running {
			server,
			view: View::default(),
			scratch,
			session: Some(session),
			inbox,
		};
		running
			.until("the guild list to arrive", |view| {
				!view.sidebar.channels().is_empty()
			})
			.await;
		running
	}

	pub fn session(&self) -> &Session {
		self.session.as_ref().expect("session is running")
	}

	pub async fn until(&mut self, what: &str, ready: impl Fn(&View) -> bool) {
		let deadline = Instant::now() + STEP_TIMEOUT;
		while !ready(&self.view) {
			let batch = match timeout_at(deadline, self.inbox.next()).await {
				Ok(Some(batch)) => batch,
				Ok(None) => panic!("the inbox closed while waiting for {what}"),
				Err(_) => panic!("timed out waiting for {what}"),
			};
			let mut changes = Changes::default();
			for item in batch {
				changes.merge(item);
			}
			let state = self.session().state().clone();
			let state = state.read();
			self.view.sidebar.apply(&state, &changes);
			self.view.timeline.apply(&state, &changes);
			self.view.rail.apply(&state, &changes);
		}
	}

	pub async fn until_state(&mut self, what: &str, ready: impl Fn(&State) -> bool) {
		let state = self.session().state().clone();
		self.until(what, move |_| ready(&state.read())).await;
	}

	pub fn select_guild(&mut self, guild_id: Snowflake) -> Option<Command> {
		let state = self.session().state().clone();
		self.view.sidebar.select(&state.read(), guild_id)
	}

	pub fn open_channel(&mut self, channel_id: Snowflake) -> Option<Command> {
		let state = self.session().state().clone();
		let guild_id = self.view.sidebar.selected();
		self.view.timeline.select(&state.read(), guild_id, channel_id)
	}

	pub async fn stop(mut self) {
		self.inbox.close();
		if let Some(session) = self.session.take() {
			tokio::task::spawn_blocking(move || session.shutdown())
				.await
				.expect("session shuts down");
		}
		self.server.shutdown().await;
		let _ = std::fs::remove_dir_all(&self.scratch);
	}
}
