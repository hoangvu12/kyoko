use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bridge::{ChannelKind, Inbox, Sidebar, changes_notifier};
use mockcord::{MockConfig, Server};
use protocol::{Snowflake, Token};
use session::{Command, Session, SessionConfig};
use state::Changes;
use store::{Paths, Settings, Store};
use tokio::time::{Instant, timeout_at};
use url::Url;

const STEP_TIMEOUT: Duration = Duration::from_secs(15);

struct Running {
	server: Server,
	session: Option<Session>,
	inbox: Inbox<Changes>,
	sidebar: Sidebar,
	scratch: PathBuf,
}

impl Running {
	async fn start(name: &str) -> Running {
		let server = Server::start(MockConfig::default())
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
			remember_account: false,
		})
		.expect("session starts");
		let mut running = Running {
			server,
			session: Some(session),
			inbox,
			sidebar: Sidebar::default(),
			scratch,
		};
		running
			.until("the guild list to arrive", |sidebar| {
				!sidebar.channels().is_empty()
			})
			.await;
		running
	}

	fn session(&self) -> &Session {
		self.session.as_ref().expect("session is running")
	}

	async fn until(&mut self, what: &str, ready: impl Fn(&Sidebar) -> bool) {
		let deadline = Instant::now() + STEP_TIMEOUT;
		while !ready(&self.sidebar) {
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
			self.sidebar.apply(&state.read(), &changes);
		}
	}

	fn select(&mut self, guild_id: Snowflake) -> Option<Command> {
		let state = self.session().state().clone();
		self.sidebar.select(&state.read(), guild_id)
	}

	async fn stop(mut self) {
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

fn guild_names(sidebar: &Sidebar) -> BTreeSet<String> {
	sidebar.guilds().iter().map(|guild| guild.name.clone()).collect()
}

fn channel_names(sidebar: &Sidebar) -> Vec<String> {
	sidebar
		.channels()
		.iter()
		.map(|channel| channel.name.clone())
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lists_the_mock_guilds_and_the_first_guilds_channels() {
	let running = Running::start("listing").await;
	let fixtures = running.server.fixtures();
	let expected: BTreeSet<String> = fixtures
		.guilds
		.iter()
		.filter_map(|guild| guild.properties.name.clone())
		.collect();
	assert_eq!(guild_names(&running.sidebar), expected);
	let first = running.sidebar.guilds()[0].id;
	assert_eq!(running.sidebar.selected(), Some(first));
	let guild = fixtures.guild(first.0).expect("the selected guild is a fixture");
	let known: BTreeSet<String> = guild
		.channel_ids
		.iter()
		.chain(&guild.thread_ids)
		.filter_map(|id| fixtures.channels.get(id))
		.filter_map(|channel| channel.name.clone())
		.collect();
	for name in channel_names(&running.sidebar) {
		assert!(
			known.contains(&name),
			"{name} is not a channel of the selected guild"
		);
	}
	assert!(
		running
			.sidebar
			.channels()
			.iter()
			.any(|channel| channel.kind == ChannelKind::Text)
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_channel_created_on_the_server_appears_live() {
	let mut running = Running::start("channel").await;
	let guild_id = running.sidebar.selected().expect("a guild is selected");
	let channel_id = running
		.server
		.create_channel(guild_id, "spike-live")
		.expect("mockcord creates the channel");
	running
		.until("the new channel to be listed", |sidebar| {
			sidebar
				.channels()
				.iter()
				.any(|channel| channel.id == channel_id && channel.name == "spike-live")
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_guild_created_on_the_server_appears_and_can_be_selected() {
	let mut running = Running::start("guild").await;
	let guild_id = running
		.server
		.create_guild("Spike Guild")
		.expect("mockcord creates the guild");
	running
		.until("the new guild to be listed", |sidebar| {
			sidebar
				.guilds()
				.iter()
				.any(|guild| guild.id == guild_id && guild.name == "Spike Guild")
		})
		.await;
	let command = running.select(guild_id);
	assert!(matches!(command, Some(Command::SubscribeGuild { guild_id: id }) if id == guild_id));
	assert_eq!(running.sidebar.selected(), Some(guild_id));
	assert_eq!(channel_names(&running.sidebar), ["general"]);
	assert!(running.select(guild_id).is_none());
	running.session().send(command.expect("a subscribe command"));
	running.stop().await;
}
