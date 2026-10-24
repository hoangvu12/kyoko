use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::{Result, bail};
use mockcord::{MockConfig, Server};
use protocol::gateway::{CHROME_VERSION, FALLBACK_BUILD_NUMBER, IdentifyProperties};
use protocol::{ChannelType, DispatchEvent, Snowflake, Token};
use session::{Command, Session, SessionConfig, SessionHooks};
use state::State;
use store::{Paths, Store};
use tokio::sync::Notify;
use tokio::time::{Instant, timeout_at};
use url::Url;

const STEP_TIMEOUT: Duration = Duration::from_secs(15);
const HOOK_NAME: &str = "test.rewriter";

struct Rewriter;

impl SessionHooks for Rewriter {
	fn name(&self) -> &str {
		HOOK_NAME
	}

	fn before_send(&self, _channel_id: Snowflake, content: &str) -> Result<Option<String>> {
		Ok(Some(content.to_uppercase()))
	}
}

#[derive(Default)]
struct Broken {
	dispatches: AtomicUsize,
}

impl SessionHooks for Broken {
	fn before_send(&self, _channel_id: Snowflake, _content: &str) -> Result<Option<String>> {
		panic!("broken before_send");
	}

	fn before_edit(
		&self,
		_channel_id: Snowflake,
		_message_id: Snowflake,
		_content: &str,
	) -> Result<Option<String>> {
		bail!("broken before_edit")
	}

	fn allow_typing(&self, _channel_id: Snowflake) -> Result<bool> {
		panic!("broken allow_typing");
	}

	fn allow_notification(&self, _message: &protocol::Message) -> Result<bool> {
		bail!("broken allow_notification")
	}

	fn on_dispatch(&self, _event: &DispatchEvent) -> Result<()> {
		self.dispatches.fetch_add(1, Ordering::Relaxed);
		panic!("broken on_dispatch");
	}
}

struct Running {
	server: Server,
	session: Option<Session>,
	wake: Arc<Notify>,
	scratch: PathBuf,
}

impl Running {
	async fn start(name: &str, hooks: Arc<dyn SessionHooks>) -> Running {
		let server = Server::start(MockConfig::default())
			.await
			.expect("mock server starts");
		let scratch = std::env::temp_dir().join(format!("kyoko-session-{name}-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&scratch);
		let store = Store::open(Paths::at(&scratch).expect("scratch paths")).expect("store opens");
		store
			.update_settings(|settings| settings.storage.plaintext_token = true)
			.expect("settings save");
		let wake = Arc::new(Notify::new());
		let notifier = wake.clone();
		let session = Session::start(SessionConfig {
			store: Arc::new(store),
			rest_base: Url::parse(server.rest_base_url()).expect("rest url"),
			gateway_url: server.gateway_url().to_owned(),
			token: Token::new(server.token()),
			identify: IdentifyProperties::web_client("en-US", FALLBACK_BUILD_NUMBER, CHROME_VERSION),
			locale: "en-US".to_owned(),
			runtime: None,
			notify: Arc::new(move |_| notifier.notify_one()),
			hooks: Some(hooks),
		})
		.expect("session starts");
		Running {
			server,
			session: Some(session),
			wake,
			scratch,
		}
	}

	fn session(&self) -> &Session {
		self.session.as_ref().expect("session is running")
	}

	async fn until(&self, what: &str, ready: impl Fn(&State) -> bool) {
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

	async fn open_text_channel(&self) -> Snowflake {
		self.until("the gateway to become ready", |state| {
			state.current_user().is_some() && state.session().connected
		})
		.await;
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

	async fn send_and_wait(&self, channel_id: Snowflake, sent: &str, echoed: &str) -> Snowflake {
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

	async fn stop(mut self) {
		if let Some(session) = self.session.take() {
			tokio::task::spawn_blocking(move || session.shutdown())
				.await
				.expect("session shuts down");
		}
		self.server.shutdown().await;
		let _ = std::fs::remove_dir_all(&self.scratch);
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn before_send_rewrites_the_outgoing_message() {
	let running = Running::start("rewrite", Arc::new(Rewriter)).await;
	let channel_id = running.open_text_channel().await;
	let message_id = running
		.send_and_wait(channel_id, "hooked message", "HOOKED MESSAGE")
		.await;
	assert_eq!(
		running.session().state().read().altered_by(message_id),
		Some(HOOK_NAME)
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failing_hooks_never_stop_the_session() {
	let broken = Arc::new(Broken::default());
	let running = Running::start("broken", broken.clone()).await;
	let channel_id = running.open_text_channel().await;
	assert!(broken.dispatches.load(Ordering::Relaxed) > 0);
	running.session().send(Command::Typing { channel_id });
	let message_id = running
		.send_and_wait(channel_id, "plain message", "plain message")
		.await;
	assert_eq!(running.session().state().read().altered_by(message_id), None);
	let message = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| cache.by_id.get(&message_id).cloned())
		.expect("message is cached");
	assert!(running.session().should_notify(&message));
	running.session().send(Command::EditMessage {
		channel_id,
		message_id,
		content: "edited message".to_owned(),
	});
	running
		.until("the edit to arrive unmodified", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&message_id))
				.is_some_and(|message| message.content == "edited message")
		})
		.await;
	running
		.until("the session to stay connected", |state| state.session().connected)
		.await;
	running.stop().await;
}
