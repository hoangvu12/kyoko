use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use mockcord::{FixtureSummary, MockConfig, Server};
use plugins::{HostConfig, PluginHost};
use protocol::{Channel, ChannelType, Snowflake, Token};
use serde_json::json;
use session::{Command, Session, SessionConfig, SessionHooks};
use state::{SharedState, State};
use store::{Paths, Settings, Store};
use tokio::sync::Notify;
use tokio::time::{Instant, sleep_until, timeout_at};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::filter::{EnvFilter, Targets};
use tracing_subscriber::layer::{Context as LayerContext, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, fmt as subscriber_fmt};
use url::Url;

mod features;

const RUNTIME_GRACE: Duration = Duration::from_secs(2);
const MOCK_TOKEN: &str = "mock-token";
const STEP_TIMEOUT: Duration = Duration::from_secs(15);
const RETRY_MESSAGE: &str = "rest rate limited, retrying";
const UNREACHABLE_REST: &str = "http://127.0.0.1:9/api/v10";
const UNREACHABLE_GATEWAY: &str = "ws://127.0.0.1:9/gateway";
const DRAFT_TEXT: &str = "headless draft";
const REACTION: &str = "\u{1F44D}";
const HISTORY_PAGES: usize = 2;
const DIRTY_LINK: &str = "https://example.com/x?utm_source=a&b=1";
const CLEAN_LINK: &str = "https://example.com/x?b=1";
const CLEAR_URLS: &str = "clear-urls";
const CLEAR_URLS_NAME: &str = "ClearURLs";
const BROKEN_PLUGIN: &str = "headless-broken";
const SILENT_TYPING: &str = "silent-typing";
const TEXT_REPLACE: &str = "text-replace";
const TEXT_REPLACE_NAME: &str = "TextReplace";
const TYPO_SENT: &str = "headless typo: teh cat";
const TYPO_FIXED: &str = "headless typo: the cat";
const TYPO_EDITED: &str = "headless typo: teh dog";
const TYPO_EDIT_FIXED: &str = "headless typo: the dog";
const TYPING_QUIET: Duration = Duration::from_millis(500);
const ONE_PING_PER_DM: &str = "one-ping-per-dm";
const PING_TEXT: &str = "headless ping";
const ALERT_QUIET: Duration = Duration::from_millis(500);
const BROKEN_HOOK_TEXT: &str = "headless hook fails";
const BROKEN_MANIFEST: &str = "id = \"headless-broken\"
name = \"Headless Broken\"
version = \"1.0.0\"
api = 1
permissions = [\"hook:send\"]
";
const BROKEN_LOGIC: &str = "return { before_send = function(message) if message.content == \"headless hook fails\" then error(\"headless hook failure\") end return nil end }";

#[derive(clap::Args)]
pub struct Args {
	#[arg(long)]
	mock: bool,
	#[arg(long, default_value_t = 1)]
	seed: u64,
	#[arg(long)]
	chaos: bool,
	#[arg(long, default_value_t = 20)]
	duration: u64,
}

struct RetryCounter(Arc<AtomicU64>);

struct IsRetryMessage(bool);

impl Visit for IsRetryMessage {
	fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
		if field.name() == "message" {
			self.0 = format!("{value:?}") == RETRY_MESSAGE;
		}
	}
}

impl<S: Subscriber> Layer<S> for RetryCounter {
	fn on_event(&self, event: &Event<'_>, _context: LayerContext<'_, S>) {
		let mut visitor = IsRetryMessage(false);
		event.record(&mut visitor);
		if visitor.0 {
			self.0.fetch_add(1, Ordering::Relaxed);
		}
	}
}

struct Counts {
	guilds: usize,
	channels: usize,
	messages_cached: usize,
	unread_channels: usize,
	total_mentions: i64,
}

impl Counts {
	fn measure(state: &State) -> Self {
		let channels: Vec<&Channel> = state
			.guilds()
			.flat_map(|guild| guild.channels.values().chain(guild.threads.values()))
			.chain(state.private_channels())
			.collect();
		let mut counts = Self {
			guilds: state.guilds().count(),
			channels: channels.len(),
			messages_cached: 0,
			unread_channels: 0,
			total_mentions: 0,
		};
		for channel in channels {
			counts.messages_cached += state.messages(channel.id).map_or(0, |cache| cache.ordered.len());
			let read_state = state.read_state(channel.id);
			let read = read_state.and_then(|read_state| read_state.last_message_id);
			if matches!((read, channel.last_message_id), (Some(read), Some(last)) if read < last) {
				counts.unread_channels += 1;
			}
			counts.total_mentions += read_state
				.and_then(|read_state| read_state.mention_count)
				.unwrap_or(0);
		}
		counts
	}

	fn check(&self, expected: &FixtureSummary) -> Result<()> {
		ensure!(
			self.guilds == expected.guilds,
			"guilds {} != fixture {}",
			self.guilds,
			expected.guilds
		);
		ensure!(
			self.channels == expected.channels,
			"channels {} != fixture {}",
			self.channels,
			expected.channels
		);
		ensure!(
			self.unread_channels == expected.unread_channels,
			"unread channels {} != fixture {}",
			self.unread_channels,
			expected.unread_channels
		);
		ensure!(
			self.total_mentions == expected.total_mentions,
			"mentions {} != fixture {}",
			self.total_mentions,
			expected.total_mentions
		);
		Ok(())
	}
}

fn plugin_host(root: &Path) -> Result<PluginHost> {
	let development = root.join("plugins-dev");
	let broken = development.join(BROKEN_PLUGIN);
	fs::create_dir_all(&broken).context("could not create the test plugin")?;
	fs::write(broken.join("logic.luau"), BROKEN_LOGIC)?;
	fs::write(broken.join("plugin.toml"), BROKEN_MANIFEST)?;
	let mut config = HostConfig::for_data_dir(root);
	config.user_dir = None;
	config.development_dirs = vec![development];
	let host = PluginHost::new(config);
	host.set_setting(
		TEXT_REPLACE,
		"rules",
		&json!([{ "find": "teh", "replace": "the", "only_if": "headless typo" }]),
	)
	.map_err(|error| anyhow::anyhow!("the TextReplace rules were refused: {error}"))?;
	for id in [
		CLEAR_URLS,
		SILENT_TYPING,
		TEXT_REPLACE,
		ONE_PING_PER_DM,
		BROKEN_PLUGIN,
	] {
		let declared = host
			.snapshot()
			.into_iter()
			.find(|plugin| plugin.id == id)
			.map(|plugin| plugin.permissions)
			.with_context(|| format!("the plugin {id} did not load"))?;
		host.approve(id, &declared)
			.map_err(|error| anyhow::anyhow!("the plugin {id} did not start: {error}"))?;
	}
	Ok(host)
}

struct Harness {
	session: Session,
	state: SharedState,
	wake: Arc<Notify>,
}

impl Harness {
	fn start(
		store: Arc<Store>,
		rest_base: &str,
		gateway_url: &str,
		hooks: Option<Arc<dyn SessionHooks>>,
	) -> Result<Self> {
		let wake = Arc::new(Notify::new());
		let notifier = wake.clone();
		let session = Session::start(SessionConfig {
			identity: session::client_identity(&store),
			build_source: None,
			store,
			rest_base: Url::parse(rest_base).context("rest url is invalid")?,
			gateway_url: gateway_url.to_owned(),
			token: Token::new(MOCK_TOKEN),
			runtime: None,
			notify: Arc::new(move |_| notifier.notify_one()),
			hooks,
			gateway: Default::default(),
			rest: Default::default(),
			caches: Default::default(),
			alerts: Default::default(),
			retention: Default::default(),
			remember_account: true,
			theme_links: Default::default(),
		})
		.context("session did not start")?;
		let state = session.state().clone();
		Ok(Self { session, state, wake })
	}

	async fn until(&self, what: &str, ready: impl Fn(&State) -> bool) -> Result<()> {
		self.wait(what, || ready(&self.state.read())).await
	}

	async fn wait(&self, what: &str, ready: impl Fn() -> bool) -> Result<()> {
		let deadline = Instant::now() + STEP_TIMEOUT;
		loop {
			if ready() {
				return Ok(());
			}
			if timeout_at(deadline, self.wake.notified()).await.is_err() {
				bail!("timed out waiting for {what}");
			}
		}
	}

	async fn round_trip(&self, channel_id: Snowflake, content: &str) -> Result<Snowflake> {
		self.round_trip_as(channel_id, content, content).await
	}

	async fn round_trip_as(&self, channel_id: Snowflake, sent: &str, echoed: &str) -> Result<Snowflake> {
		self.session.send(Command::SendMessage {
			channel_id,
			content: sent.to_owned(),
			reply_to: None,
			ping_reply: true,
			files: Vec::new(),
		});
		self.until("the sent message to replace its pending row", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.pending.is_empty() && cache.ordered.iter().any(|message| message.content == echoed)
			})
		})
		.await?;
		self.state
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
			.context("sent message vanished from the cache")
	}

	async fn receive(
		&self,
		server: &Server,
		channel_id: Snowflake,
		author: u64,
		content: &str,
	) -> Result<Snowflake> {
		let id = server
			.control()
			.post_message(channel_id.0, author, content)
			.map(Snowflake)
			.context("the mock refused to post a direct message")?;
		self.until("the direct message to arrive", |state| {
			state
				.channel(channel_id)
				.is_some_and(|channel| channel.last_message_id >= Some(id))
		})
		.await?;
		Ok(id)
	}

	async fn pings_once(&self, server: &Server) -> Result<()> {
		let (dm_id, author) =
			quiet_direct_message(&self.state.read(), server).context("no direct message that can notify")?;
		self.session.take_alerts();
		let mut last = None;
		for step in 0..3 {
			last = Some(
				self.receive(server, dm_id, author, &format!("{PING_TEXT} {step}"))
					.await?,
			);
		}
		tokio::time::sleep(ALERT_QUIET).await;
		let pinged = self.session.take_alerts().len();
		ensure!(
			pinged == 1,
			"OnePingPerDM let {pinged} of three direct messages notify"
		);
		let message_id = last.context("no direct message was posted")?;
		self.session.send(Command::AckChannel {
			channel_id: dm_id,
			message_id,
		});
		self.until("the direct message to be read", |state| {
			state
				.read_state(dm_id)
				.is_some_and(|read| read.last_message_id >= Some(message_id))
		})
		.await?;
		self.receive(server, dm_id, author, &format!("{PING_TEXT} after reading"))
			.await?;
		self.wait("a notification once the direct message was read", || {
			!self.session.take_alerts().is_empty()
		})
		.await
	}

	async fn stop(self) -> Result<()> {
		tokio::task::spawn_blocking(move || self.session.shutdown())
			.await
			.context("session shutdown panicked")
	}
}

fn first_text_channel(state: &State) -> Option<(Snowflake, Snowflake)> {
	state.guilds().find_map(|guild| {
		guild
			.channels
			.values()
			.find(|channel| channel.kind == ChannelType::GuildText)
			.map(|channel| (guild.id, channel.id))
	})
}

fn quiet_direct_message(state: &State, server: &Server) -> Option<(Snowflake, u64)> {
	let overridden: Vec<Snowflake> = state
		.guild_settings(None)
		.map(|settings| {
			settings
				.channel_overrides
				.iter()
				.map(|over| over.channel_id)
				.collect()
		})
		.unwrap_or_default();
	let blocked: Vec<Snowflake> = state.blocked().iter().map(|row| row.user_id).collect();
	state
		.private_channels()
		.filter(|channel| channel.kind == ChannelType::Dm && !overridden.contains(&channel.id))
		.find_map(|channel| {
			let author = channel.recipient_ids.first()?;
			(!blocked.contains(author) && server.fixtures().users.contains_key(&author.0))
				.then_some((channel.id, author.0))
		})
}

fn scratch_dir() -> Result<std::path::PathBuf> {
	let root = std::env::temp_dir().join(format!("kyoko-headless-{}", std::process::id()));
	match fs::remove_dir_all(&root) {
		Ok(()) => {}
		Err(error) if error.kind() == io::ErrorKind::NotFound => {}
		Err(error) => return Err(error).context("could not clear the scratch directory"),
	}
	Ok(root)
}

fn init_tracing() -> Arc<AtomicU64> {
	let retries = Arc::new(AtomicU64::new(0));
	let console = subscriber_fmt::layer()
		.with_writer(io::stderr)
		.with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")));
	let counter = RetryCounter(retries.clone()).with_filter(Targets::new().with_target("rest", Level::DEBUG));
	tracing_subscriber::registry().with(console).with(counter).init();
	retries
}

pub fn run(args: Args) -> Result<()> {
	ensure!(args.mock, "only the mock server is supported, pass --mock");
	let retries = init_tracing();
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(2)
		.enable_all()
		.build()
		.context("could not start the async runtime")?;
	let root = scratch_dir()?;
	let outcome = runtime.block_on(mock_run(args, &root, retries));
	runtime.shutdown_timeout(RUNTIME_GRACE);
	let _ = fs::remove_dir_all(&root);
	outcome
}

async fn mock_run(args: Args, root: &Path, retries: Arc<AtomicU64>) -> Result<()> {
	let started = Instant::now();
	let server = Server::start(MockConfig {
		seed: args.seed,
		chaos: args.chaos,
		..MockConfig::default()
	})
	.await
	.context("mock server did not start")?;
	let outcome = scenario(&args, &server, root, started, &retries).await;
	server.shutdown().await;
	outcome
}

async fn scenario(
	args: &Args,
	server: &Server,
	root: &Path,
	started: Instant,
	retries: &AtomicU64,
) -> Result<()> {
	let expected = server.fixtures().summary();
	let paths = Paths::at(root)?;
	let mut settings = Settings::load(&paths);
	settings.storage.plaintext_token = true;
	settings.save(&paths)?;
	let store = Arc::new(Store::open(paths)?);
	let plugins = plugin_host(root)?;
	let harness = Harness::start(
		store.clone(),
		server.rest_base_url(),
		server.gateway_url(),
		Some(Arc::new(plugins.clone())),
	)?;
	plugins.attach(harness.state.clone(), harness.session.commander());

	harness
		.until("the gateway to become ready", |state| {
			state.current_user().is_some() && state.session().connected
		})
		.await?;
	let baseline = Counts::measure(&harness.state.read());
	baseline
		.check(&expected)
		.context("state after READY does not match the fixtures")?;

	let (guild_id, channel_id) =
		first_text_channel(&harness.state.read()).context("no text channel in the fixtures")?;
	harness.session.send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	harness
		.until("the channel history to load", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await?;
	for _ in 0..HISTORY_PAGES {
		let cached = harness
			.state
			.read()
			.messages(channel_id)
			.map_or(0, |cache| cache.ordered.len());
		harness.session.send(Command::LoadOlder { channel_id });
		harness
			.until("older history to load", |state| {
				state
					.messages(channel_id)
					.is_some_and(|cache| cache.ordered.len() > cached)
			})
			.await?;
	}

	let first_message = harness
		.state
		.read()
		.messages(channel_id)
		.and_then(|cache| cache.ordered.first().map(|message| message.id))
		.context("no loaded message to render")?;
	harness
		.until("a loaded message to render", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.rendered.get(&first_message))
				.is_some_and(|rendered| !rendered.ast.is_empty())
		})
		.await?;
	let cleaned_id = harness.round_trip_as(channel_id, DIRTY_LINK, CLEAN_LINK).await?;
	ensure!(
		harness.state.read().altered_by(cleaned_id) == Some(CLEAR_URLS_NAME),
		"the cleaned message is not marked as altered by ClearURLs"
	);
	let typo_id = harness.round_trip_as(channel_id, TYPO_SENT, TYPO_FIXED).await?;
	ensure!(
		harness.state.read().altered_by(typo_id) == Some(TEXT_REPLACE_NAME),
		"the replaced message is not marked as altered by TextReplace"
	);
	harness.session.send(Command::EditMessage {
		channel_id,
		message_id: typo_id,
		content: TYPO_EDITED.to_owned(),
	});
	harness
		.until("the edited typo to be replaced", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&typo_id))
				.is_some_and(|message| message.content == TYPO_EDIT_FIXED)
		})
		.await?;
	harness.round_trip(channel_id, BROKEN_HOOK_TEXT).await?;
	harness.session.send(Command::Typing { channel_id });
	tokio::time::sleep(TYPING_QUIET).await;
	ensure!(
		server
			.control()
			.recording()
			.requests_to("POST", "/channels/*/typing")
			.count() == 0,
		"a typing request reached the wire while SilentTyping was on"
	);

	let sent_id = harness
		.round_trip(
			channel_id,
			&format!("headless probe {}", started.elapsed().as_nanos()),
		)
		.await?;
	harness.session.send(Command::AddReaction {
		channel_id,
		message_id: sent_id,
		emoji: REACTION.to_owned(),
	});
	harness
		.until("the reaction to appear", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&sent_id))
				.is_some_and(|message| {
					message
						.reactions
						.iter()
						.any(|reaction| reaction.me && reaction.emoji.name.as_deref() == Some(REACTION))
				})
		})
		.await?;
	harness.session.send(Command::SetDraft {
		channel_id,
		content: DRAFT_TEXT.to_owned(),
	});
	harness
		.until("the draft to be stored", |state| {
			state.draft(channel_id) == Some(DRAFT_TEXT)
		})
		.await?;
	harness.session.send(Command::AckChannel {
		channel_id,
		message_id: sent_id,
	});
	harness
		.until("the channel to be read", |state| {
			state.read_state(channel_id).is_some_and(|read_state| {
				read_state.last_message_id >= Some(sent_id) && read_state.mention_count == Some(0)
			})
		})
		.await?;
	harness.pings_once(server).await?;

	if args.chaos {
		let session_id = harness.state.read().session().session_id.clone();
		let drops = harness.session.reconnects();
		harness.session.send(Command::Reconnect);
		harness
			.until("the forced disconnect", |_| harness.session.reconnects() > drops)
			.await?;
		harness
			.until("the session to resume", |state| {
				state.session().connected && state.session().session_id == session_id
			})
			.await?;
		harness
			.round_trip(
				channel_id,
				&format!("headless probe after resume {}", started.elapsed().as_nanos()),
			)
			.await?;
	}

	let text = harness
		.text_features(server.fixtures(), guild_id, channel_id, sent_id)
		.await?;

	sleep_until(started + Duration::from_secs(args.duration)).await;
	harness
		.until("the gateway to be connected", |state| state.session().connected)
		.await?;

	let counts = Counts::measure(&harness.state.read());
	let reconnects = harness.session.reconnects();
	println!(
		"{}",
		json!({
			"guilds": counts.guilds,
			"channels": counts.channels,
			"messages_cached": counts.messages_cached,
			"unread_channels": baseline.unread_channels,
			"total_mentions": baseline.total_mentions,
			"reconnects": reconnects,
			"rest_calls_429_retried": retries.load(Ordering::Relaxed),
			"threads_created": text.threads_created,
			"moderation_ok": text.moderation_ok,
			"expressions_ok": text.expressions_ok,
			"community_ok": text.community_ok,
			"plugins": plugins
				.snapshot()
				.iter()
				.map(|plugin| (plugin.id.clone(), json!(plugin.status.name())))
				.collect::<serde_json::Map<_, _>>(),
		})
	);
	ensure!(
		counts.guilds == expected.guilds
			&& counts.channels == expected.channels + text.threads_created + text.groups_created,
		"final guild and channel counts do not match the fixtures"
	);
	ensure!(
		!args.chaos || reconnects >= 1,
		"chaos run finished without a reconnect"
	);

	harness.stop().await?;
	verify_cache(&store, channel_id)?;
	cached_start(&store, guild_id, channel_id, counts.guilds).await
}

fn verify_cache(store: &Store, channel_id: Snowflake) -> Result<()> {
	ensure!(
		store.db.get_draft(channel_id.get())?.as_deref() == Some(DRAFT_TEXT),
		"draft was not persisted"
	);
	ensure!(
		!store.db.recent_messages(channel_id.get(), 1)?.is_empty(),
		"messages were not persisted"
	);
	ensure!(
		store.db.get_read_state(channel_id.get())?.is_some(),
		"read state was not persisted"
	);
	Ok(())
}

async fn cached_start(
	store: &Arc<Store>,
	guild_id: Snowflake,
	channel_id: Snowflake,
	guilds: usize,
) -> Result<()> {
	let offline = Harness::start(store.clone(), UNREACHABLE_REST, UNREACHABLE_GATEWAY, None)?;
	offline
		.until("cached guilds to load", |state| state.guilds().count() == guilds)
		.await?;
	offline.session.send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	offline
		.until("cached messages and draft to load", |state| {
			state.draft(channel_id) == Some(DRAFT_TEXT)
				&& state
					.messages(channel_id)
					.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await?;
	offline.stop().await
}
