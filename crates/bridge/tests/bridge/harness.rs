use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bridge::{DmList, FriendsView, Inbox, MemberPanel, Rail, Sidebar, Timeline, changes_notifier};
use mockcord::{MockConfig, Server};
use protocol::{Snowflake, Token};
use session::{Command, GatewayTuning, Session, SessionConfig};
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
	pub dms: DmList,
	pub friends: FriendsView,
	pub members: MemberPanel,
	pub member_publishes: usize,
}

pub struct Running {
	pub server: Server,
	pub view: View,
	pub scratch: PathBuf,
	session: Option<Session>,
	inbox: Inbox<Changes>,
	gateway: GatewayTuning,
}

impl Running {
	pub async fn start(name: &str) -> Running {
		Self::start_with(name, MockConfig::default()).await
	}

	pub async fn start_with(name: &str, config: MockConfig) -> Running {
		Self::start_tuned(name, config, GatewayTuning::default()).await
	}

	pub async fn start_tuned(name: &str, config: MockConfig, gateway: GatewayTuning) -> Running {
		let server = Server::start(MockConfig {
			record: true,
			..config
		})
		.await
		.expect("mock server starts");
		let scratch = std::env::temp_dir().join(format!("kyoko-bridge-{name}-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&scratch);
		let inbox = Inbox::new();
		let session = open_session(&server, &scratch, &inbox, gateway);
		let mut running = Running {
			server,
			view: View::default(),
			scratch,
			session: Some(session),
			inbox,
			gateway,
		};
		running
			.until("the guild list to arrive", |view| {
				view.sidebar.channels().next().is_some()
			})
			.await;
		running
	}

	pub async fn restart(&mut self) {
		if let Some(session) = self.session.take() {
			tokio::task::spawn_blocking(move || session.shutdown())
				.await
				.expect("session shuts down");
		}
		self.inbox.close();
		self.inbox = Inbox::new();
		self.view = View::default();
		self.session = Some(open_session(
			&self.server,
			&self.scratch,
			&self.inbox,
			self.gateway,
		));
		self.until("the guild list after a restart", |view| {
			view.sidebar.channels().next().is_some()
		})
		.await;
	}

	pub fn session(&self) -> &Session {
		self.session.as_ref().expect("session is running")
	}

	pub async fn until(&mut self, what: &str, ready: impl Fn(&View) -> bool) {
		let deadline = Instant::now() + STEP_TIMEOUT;
		while !ready(&self.view) {
			let wake = self
				.view
				.members
				.due()
				.map_or(deadline, |due| deadline.min(Instant::from_std(due)));
			let batch = match timeout_at(wake, self.inbox.next()).await {
				Ok(Some(batch)) => batch,
				Ok(None) => panic!("the inbox closed while waiting for {what}"),
				Err(_) if wake < deadline => {
					self.flush_members();
					continue;
				}
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
			self.view.dms.apply(&state, &changes);
			self.view.friends.apply(&state, &changes);
			self.view
				.members
				.apply(&state, &changes, std::time::Instant::now());
			drop(state);
			self.flush_members();
		}
	}

	pub fn flush_members(&mut self) {
		let state = self.session().state().clone();
		let state = state.read();
		if self
			.view
			.members
			.flush(&state, std::time::Instant::now())
			.is_some()
		{
			self.view.member_publishes += 1;
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

fn open_session(server: &Server, scratch: &Path, inbox: &Inbox<Changes>, gateway: GatewayTuning) -> Session {
	let paths = Paths::at(scratch).expect("scratch paths");
	let mut settings = Settings::load(&paths);
	settings.storage.plaintext_token = true;
	settings.save(&paths).expect("settings save");
	let store = Store::open(paths).expect("store opens");
	Session::start(SessionConfig {
		identity: session::client_identity(&store),
		build_source: None,
		store: Arc::new(store),
		rest_base: Url::parse(server.rest_base_url()).expect("rest url"),
		gateway_url: server.gateway_url().to_owned(),
		token: Token::new(server.token()),
		runtime: None,
		notify: changes_notifier(inbox.clone()),
		hooks: None,
		gateway,
		rest: Default::default(),
		caches: Default::default(),
		alerts: Default::default(),
		retention: Default::default(),
		remember_account: false,
		theme_links: Default::default(),
	})
	.expect("session starts")
}
