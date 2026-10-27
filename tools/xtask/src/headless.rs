use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use login::{LoginStep, MfaMethod, PasswordLogin, QrConfig, QrEvent, QrLogin};
use mockcord::{FixtureSummary, MockConfig, Server};
use plugins::{HostConfig, PluginHost};
use protocol::{Channel, ChannelType, Snowflake, Token};
use serde_json::json;
use session::{Command, Formatting, Session, SessionConfig, SessionHooks};
use state::{EmojiCandidate, SharedState, State};
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
const SHOW_ME_YOUR_NAME: &str = "show-me-your-name";
const DONT_ROUND: &str = "dont-round-my-timestamps";
const EXPRESSION_CLONER: &str = "expression-cloner";
const MESSAGE_LOGGER: &str = "message-logger";
const SHOW_HIDDEN_CHANNELS: &str = "show-hidden-channels";
const SHOW_HIDDEN_THINGS: &str = "show-hidden-things";
const READ_ALL_NOTIFICATIONS: &str = "read-all-notifications";
const READ_ALL_KEY: &str = "read_all";
const CLONE_KEY: &str = "clone_emoji";
const CLONE_NAME: &str = "headless_clone";
const REDRAWS: usize = 100;
const FAVORITE_EMOJI_FIRST: &str = "favorite-emoji-first";
const FAVORITE_QUERY: &str = "wave";
const FAVORITE_EMOJI: &str = "blob_wave";
const WAVE: &str = "👋";
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
		SHOW_ME_YOUR_NAME,
		DONT_ROUND,
		FAVORITE_EMOJI_FIRST,
		EXPRESSION_CLONER,
		MESSAGE_LOGGER,
		SHOW_HIDDEN_CHANNELS,
		SHOW_HIDDEN_THINGS,
		READ_ALL_NOTIFICATIONS,
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
		token: Token,
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
			token,
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

	async fn formats_names_and_times(
		&self,
		plugins: &PluginHost,
		server: &Server,
		guild_id: Snowflake,
		channel_id: Snowflake,
	) -> Result<u64> {
		let wake = self.wake.clone();
		let display = bridge::Display::new(Arc::new(plugins.clone()), move |_| wake.notify_one());
		let timeline = RefCell::new(bridge::Timeline::default());
		timeline.borrow_mut().set_display(display.clone());
		timeline
			.borrow_mut()
			.select(&self.state.read(), Some(guild_id), channel_id);
		self.wait("ShowMeYourName to settle on the timeline", || {
			let state = self.state.read();
			let settled = members_settled(&state, server, guild_id, channel_id);
			let mut timeline = timeline.borrow_mut();
			timeline.redraw(&state, Formatting::Names);
			let mut plain = timeline.messages().filter(|entry| entry.is_plain()).peekable();
			settled
				&& display.pending() == 0
				&& plain.peek().is_some()
				&& plain.all(|entry| shows_username(&state, guild_id, entry))
		})
		.await?;
		let calls = display.calls();
		for _ in 0..REDRAWS {
			timeline
				.borrow_mut()
				.redraw(&self.state.read(), Formatting::Names);
		}
		ensure!(
			display.calls() == calls && display.pending() == 0,
			"redrawing the timeline asked ShowMeYourName again"
		);
		let now = chrono::Utc::now();
		let unix = (now - chrono::TimeDelta::hours(2 * 24 + 23)).timestamp();
		self.wait("DontRoundMyTimestamps to round a relative time down", || {
			display.relative(unix, now) == "2 days ago"
		})
		.await?;
		Ok(display.calls())
	}

	async fn logs_kept_messages(
		&self,
		plugins: &PluginHost,
		server: &Server,
		guild_id: Snowflake,
		channel_id: Snowflake,
		edited_id: Snowflake,
	) -> Result<u64> {
		let wake = self.wake.clone();
		let display = bridge::Display::new(Arc::new(plugins.clone()), move |_| wake.notify_one());
		let timeline = RefCell::new(bridge::Timeline::default());
		timeline.borrow_mut().set_display(display.clone());
		timeline
			.borrow_mut()
			.select(&self.state.read(), Some(guild_id), channel_id);
		let doomed = {
			let state = self.state.read();
			let mine = state.current_user_id();
			state
				.messages(channel_id)
				.and_then(|cache| {
					cache
						.ordered
						.iter()
						.rev()
						.find(|message| Some(message.author.id) != mine && !message.content.is_empty())
						.map(|message| message.id)
				})
				.context("no message by someone else to delete")?
		};
		ensure!(
			server.control().delete_message(channel_id.0, doomed.0),
			"the mock refused to delete a message"
		);
		self.wait("MessageLogger to show the deleted and the edited message", || {
			let state = self.state.read();
			let mut timeline = timeline.borrow_mut();
			timeline.redraw(&state, Formatting::Messages);
			let deleted = timeline.messages().any(|entry| {
				entry.id == doomed
					&& entry.deleted
					&& entry
						.look
						.as_ref()
						.is_some_and(|look| look.deleted == Some(session::DeletedLook::Text))
			});
			let edited = timeline.messages().any(|entry| {
				entry.id == edited_id
					&& !entry.history.is_empty()
					&& entry
						.look
						.as_ref()
						.is_some_and(|look| look.history == Some(session::HistoryLook::Inline))
			});
			deleted && edited && display.pending() == 0
		})
		.await?;
		let calls = display.look_calls();
		for _ in 0..REDRAWS {
			timeline
				.borrow_mut()
				.redraw(&self.state.read(), Formatting::Messages);
		}
		ensure!(
			display.look_calls() == calls && display.pending() == 0,
			"redrawing the timeline asked MessageLogger again"
		);
		Ok(calls)
	}

	async fn favourites_first(&self, server: &Server) -> Result<()> {
		let guild = server.fixtures().guilds.first().context("no mock server")?;
		let favourite = guild
			.emojis
			.iter()
			.find(|emoji| emoji.name.as_deref() == Some(FAVORITE_EMOJI))
			.and_then(|emoji| emoji.id)
			.context("the mock server has no favourite emoji")?;
		let candidates = vec![
			EmojiCandidate {
				key: FAVORITE_QUERY.to_owned(),
				name: FAVORITE_QUERY.to_owned(),
				id: None,
				animated: false,
				guild_id: None,
				unicode: Some(WAVE.to_owned()),
			},
			EmojiCandidate {
				key: favourite.to_string(),
				name: FAVORITE_EMOJI.to_owned(),
				id: Some(favourite),
				animated: false,
				guild_id: Some(Snowflake(guild.id)),
				unicode: None,
			},
		];
		let ask = |candidates: Vec<EmojiCandidate>| {
			self.session.send(Command::RankEmoji {
				guild_id: None,
				query: FAVORITE_QUERY.to_owned(),
				candidates,
			});
		};
		ask(candidates.clone());
		self.until("the favourite emoji", |state| state.favorites_loaded())
			.await?;
		ask(candidates);
		let key = favourite.to_string();
		self.until("FavoriteEmojiFirst to rank the favourite first", |state| {
			state
				.emoji_ranking(None, FAVORITE_QUERY)
				.and_then(<[String]>::first)
				== Some(&key)
		})
		.await
	}

	async fn clones(&self, plugins: &PluginHost, server: &Server, root: &Path) -> Result<bool> {
		let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
		let sink = asked.clone();
		plugins.on_request(move |request| {
			if let Ok(mut asked) = sink.lock() {
				asked.push(request);
			}
		});
		let (source, target) = {
			let state = self.state.read();
			let source = state
				.guilds()
				.find_map(|guild| {
					guild.emojis.iter().find_map(|emoji| {
						Some(bridge::emoji_expression(
							emoji.name.as_deref()?,
							emoji.id?,
							emoji.animated.unwrap_or(false),
						))
					})
				})
				.context("no custom emoji to clone")?;
			let target = bridge::clone_targets(&state, &source)
				.into_iter()
				.find(|target| target.blocked.is_none())
				.context("no server takes the clone")?;
			(source, target)
		};
		let items = plugins.menu_items(plugins::Slot::MessageMenu);
		ensure!(
			items.iter().any(|item| item.key == CLONE_KEY),
			"ExpressionCloner offers no Clone Emoji item"
		);
		plugins
			.menu_gesture(
				EXPRESSION_CLONER,
				CLONE_KEY,
				plugins::Slot::MessageMenu,
				Some(source.clone()),
				&[],
			)
			.map_err(|error| anyhow::anyhow!("the Clone Emoji click failed: {error}"))?;
		let requested = asked.lock().map(|asked| asked.len()).unwrap_or_default();
		ensure!(
			requested == 1,
			"ExpressionCloner asked {requested} times for one click"
		);
		let fingerprint = protocol::fingerprint::Fingerprint::web_client("en-US", 1);
		let fetcher = bridge::media_fetcher(&fingerprint, Some(server.address().port()))?;
		let loader = media::MediaService::new(fetcher, root.join("clone-cache"), media::Limits::default());
		let url = bridge::clone_source(&server.cdn_url(), &source).context("the emoji has no source")?;
		let bytes = loader.bytes(&url).await?.to_vec();
		let command = bridge::clone_command(&self.state.read(), &source, target.guild_id, CLONE_NAME, bytes);
		self.session.send(command);
		self.until("the cloned emoji", |state| {
			state.guild(target.guild_id).is_some_and(|guild| {
				guild
					.emojis
					.iter()
					.any(|emoji| emoji.name.as_deref() == Some(CLONE_NAME))
			})
		})
		.await?;
		Ok(true)
	}

	async fn leaves_hidden_channels_alone(&self, plugins: &PluginHost, server: &Server) -> Result<bool> {
		ensure!(
			plugins.shows(plugins::Slot::HiddenChannels) && plugins.shows(plugins::Slot::ModeratorNotes),
			"ShowHiddenChannels and ShowHiddenThings did not turn their views on"
		);
		let (guild_id, hidden) = {
			let state = self.state.read();
			state
				.guilds()
				.find_map(|guild| {
					guild
						.channels
						.values()
						.find(|channel| channel.kind == ChannelType::GuildText && state.is_hidden(channel.id))
						.map(|channel| (guild.id, channel.id))
				})
				.context("no hidden channel in the fixtures")?
		};
		let info = {
			let state = self.state.read();
			let mut sidebar = bridge::Sidebar::showing_hidden(true);
			sidebar.select(&state, guild_id);
			sidebar.open_hidden(&state, hidden)
		};
		let info = info.context("the hidden channel's details did not open")?;
		ensure!(
			!info.denied.is_empty(),
			"the hidden channel lists nobody kept out"
		);
		self.session.send(Command::SelectChannel {
			guild_id: Some(guild_id),
			channel_id: hidden,
		});
		self.session.send(Command::Typing { channel_id: hidden });
		self.session.send(Command::SubscribeGuild { guild_id });
		tokio::time::sleep(TYPING_QUIET).await;
		let id = hidden.to_string();
		let named = server.control().inspect(|recording| {
			recording
				.requests
				.iter()
				.filter(|request| {
					request.path.contains(&id) || String::from_utf8_lossy(&request.body).contains(&id)
				})
				.count() + recording
				.frames
				.iter()
				.filter(|frame| frame.raw.contains(&id))
				.count()
		});
		ensure!(named == 0, "{named} requests or frames named a hidden channel");
		Ok(true)
	}

	async fn reads_everything(&self, plugins: &PluginHost, server: &Server) -> Result<usize> {
		let before = server
			.control()
			.recording()
			.requests_to("POST", "/read-states/ack-bulk")
			.count();
		plugins
			.gesture(
				READ_ALL_NOTIFICATIONS,
				READ_ALL_KEY,
				&[
					("source".to_owned(), "slot".to_owned()),
					("slot".to_owned(), "rail.buttons".to_owned()),
				],
			)
			.map_err(|error| anyhow::anyhow!("the Read All click failed: {error}"))?;
		self.until("every server and DM to be read", |state| {
			state
				.guild_summaries()
				.iter()
				.all(|guild| !guild.unread && guild.mentions == 0)
				&& state.dm_rows().iter().all(|dm| !dm.unread && dm.mentions == 0)
		})
		.await?;
		let control = server.control();
		self.wait("the bulk acks", || {
			control
				.recording()
				.requests_to("POST", "/read-states/ack-bulk")
				.count() > before
		})
		.await?;
		Ok(control
			.recording()
			.requests_to("POST", "/read-states/ack-bulk")
			.count() - before)
	}

	async fn stop(self) -> Result<()> {
		tokio::task::spawn_blocking(move || self.session.shutdown())
			.await
			.context("session shutdown panicked")
	}
}

fn shows_username(state: &State, guild_id: Snowflake, entry: &bridge::MessageEntry) -> bool {
	let mut person = bridge::person(state, Some(guild_id), &entry.message.author);
	if person.nick.is_none() {
		person.nick = entry
			.message
			.member
			.as_ref()
			.and_then(|member| member.nick.clone());
	}
	let name = person.name().to_owned();
	let extra = (name.to_lowercase() != person.username.to_lowercase()).then_some(person.username.as_str());
	entry.author == name && entry.author_extra.as_deref() == extra
}

fn members_settled(state: &State, server: &Server, guild_id: Snowflake, channel_id: Snowflake) -> bool {
	let Some(guild) = server
		.fixtures()
		.guilds
		.iter()
		.find(|guild| guild.id == guild_id.0)
	else {
		return true;
	};
	state
		.backfill_needs(channel_id, None)
		.members
		.iter()
		.all(|user_id| guild.member(user_id.0).is_none())
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

async fn password_sign_in(server: &Server, store: &Store) -> Result<()> {
	let rest_base = Url::parse(server.rest_base_url()).context("rest url is invalid")?;
	let login = PasswordLogin::new(rest_base, &session::client_identity(store));
	let step = tokio::time::timeout(
		STEP_TIMEOUT,
		login.login(mockcord::MFA_LOGIN, mockcord::MOCK_PASSWORD),
	)
	.await
	.context("timed out waiting for the email sign-in")?
	.context("the email sign-in failed")?;
	let LoginStep::Mfa(challenge) = step else {
		bail!("the email sign-in skipped two-factor authentication");
	};
	ensure!(
		challenge.methods().totp,
		"the email sign-in offered no authenticator code"
	);
	let token = tokio::time::timeout(
		STEP_TIMEOUT,
		login.verify(&challenge, MfaMethod::Totp, mockcord::TOTP_CODE),
	)
	.await
	.context("timed out waiting for the two-factor code")?
	.context("the two-factor code was refused")?;
	ensure!(
		token.as_str() == server.token(),
		"the email sign-in gave the wrong token"
	);
	tracing::info!("signed in with the email form and an authenticator code");
	Ok(())
}

async fn qr_sign_in(server: &Server, store: &Store) -> Result<Token> {
	let mut gateway_url = Url::parse(&server.remote_auth_url()).context("remote auth url is invalid")?;
	gateway_url.query_pairs_mut().append_pair("phone", "login");
	let mut login = QrLogin::start(QrConfig {
		gateway_url,
		origin: protocol::fingerprint::WEB_ORIGIN.to_owned(),
		rest_base: Url::parse(server.rest_base_url()).context("rest url is invalid")?,
		identity: session::client_identity(store),
	});
	let mut events = login.events().context("the QR login has no events")?;
	let mut code_shown = false;
	let mut scanned = false;
	loop {
		let event = tokio::time::timeout(STEP_TIMEOUT, events.recv())
			.await
			.context("timed out waiting for the QR login")?
			.context("the QR login stopped without a token")?;
		match event {
			QrEvent::Code { url } => code_shown = url.starts_with("https://discord.com/ra/"),
			QrEvent::Scanned(scanner) => {
				scanned = scanner.username == server.fixtures().current_user.username;
			}
			QrEvent::Token(token) => {
				ensure!(code_shown, "the QR login gave a token before showing a code");
				ensure!(
					scanned,
					"the QR login gave a token before showing the scanning user"
				);
				ensure!(
					token.as_str() == server.token(),
					"the QR login gave the wrong token"
				);
				tracing::info!("signed in with the QR code");
				return Ok(token);
			}
			QrEvent::Expired => bail!("the QR code expired"),
			QrEvent::Cancelled => bail!("the QR login was cancelled"),
			QrEvent::Failed(error) => bail!("the QR login failed: {error}"),
		}
	}
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
		record: true,
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
	password_sign_in(server, &store).await?;
	let token = qr_sign_in(server, &store).await?;
	let harness = Harness::start(
		store.clone(),
		server.rest_base_url(),
		server.gateway_url(),
		token,
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
	let display_calls = harness
		.formats_names_and_times(&plugins, server, guild_id, channel_id)
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
	ensure!(
		harness.state.read().shows_deleted_messages(),
		"MessageLogger did not turn on deleted messages"
	);
	let kept_calls = harness
		.logs_kept_messages(&plugins, server, guild_id, channel_id, typo_id)
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
		burst: false,
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
	harness.favourites_first(server).await?;

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
	let clone_ok = harness.clones(&plugins, server, root).await?;
	let hidden_quiet = harness.leaves_hidden_channels_alone(&plugins, server).await?;
	let read_all_acks = harness.reads_everything(&plugins, server).await?;

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
			"clone_ok": clone_ok,
			"hidden_quiet": hidden_quiet,
			"read_all_acks": read_all_acks,
			"display_calls": display_calls,
			"kept_calls": kept_calls,
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
	let offline = Harness::start(
		store.clone(),
		UNREACHABLE_REST,
		UNREACHABLE_GATEWAY,
		Token::new(MOCK_TOKEN),
		None,
	)?;
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
