use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant as StdInstant};

use chrono::Utc;
use gateway::{GatewayConfig, GatewayError, GatewayEvent, GatewayHandle, MemberLookup, MemberRequest};
use parking_lot::Mutex;
use protocol::fingerprint::channel_referer;
use protocol::{
	Channel, ChannelType, CreateMessage, DISCORD_EPOCH_MS, DispatchEvent, EditMessage, GatewayRateLimited,
	Message, MessageAck, MessageReference, MessagesQuery, Nonce, Opcode, Snowflake, StatusKind, Token, User,
};
use rest::api::SearchQuery;
use rest::{RestClient, RestTuning, UploadFile};
use state::{
	Changes, ConnectionStatus, DeleteStatus, DisconnectReason, Event, LoadDirection, Loaded, LocalEvent,
	RenderedMessage, SendFailure, SharedState, TYPING_TIMEOUT,
};
use store::Store;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, sleep_until};
use url::Url;

use crate::catalog::ThemeCatalogs;
use crate::command::Command;
use crate::hooks::{Hooks, guarded, guarded_blocking};
use crate::identity;
use crate::notifier::Notifier;
use crate::persist::{self, Job, Persistor};
use crate::search::SearchResults;

mod accounts;
mod actions;
mod commands;
mod deletes;
mod expressions;
mod failures;
mod locks;
mod member_requests;
mod reads;
mod subscriptions;
mod themes;

const PAGE_SIZE: u8 = 50;
const TYPING_THROTTLE: Duration = Duration::from_secs(9);
const TYPING_EXPIRY_SLACK: Duration = Duration::from_millis(100);
const DAY_MILLIS: i64 = 86_400_000;
const NONCE_SEQUENCE_BITS: u32 = 22;
const RENDER_REVISION: u64 = 1;

#[derive(Default)]
pub(crate) struct Stats {
	pub(crate) reconnects: AtomicU64,
}

pub(crate) type SharedSearch = Arc<Mutex<Option<Arc<SearchResults>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Page {
	Older,
	Newer,
}

pub(crate) enum Internal {
	Messages {
		channel_id: Snowflake,
		direction: LoadDirection,
		messages: Vec<Message>,
		reached_start: bool,
		cache: bool,
	},
	LoadFinished(Snowflake, Page),
	SendFailed(Nonce, SendFailure),
	Search(SearchResults),
	DmOpened(Box<Channel>),
	ThreadOpened(Box<Channel>),
	Loaded(Loaded),
	Rejected(String),
	Draft {
		channel_id: Snowflake,
		content: String,
	},
	BeforeSend {
		nonce: Nonce,
		channel_id: Snowflake,
		content: String,
		reply_to: Option<Snowflake>,
		files: Vec<UploadFile>,
		rewritten: Option<String>,
	},
	ThemeCatalogChanged,
	AccountSwitch(Snowflake),
	DeleteProgressed {
		batch: u64,
		message_id: Snowflake,
		status: DeleteStatus,
	},
}

pub(crate) struct CoreParts {
	pub(crate) state: SharedState,
	pub(crate) rest: RestClient,
	pub(crate) gateway: GatewayHandle,
	pub(crate) store: Arc<Store>,
	pub(crate) notifier: Notifier,
	pub(crate) runtime: Handle,
	pub(crate) search: SharedSearch,
	pub(crate) stats: Arc<Stats>,
	pub(crate) internal: mpsc::UnboundedSender<Internal>,
	pub(crate) hooks: Hooks,
	pub(crate) themes: Arc<ThemeCatalogs>,
	pub(crate) locale: String,
	pub(crate) token: Arc<Token>,
	pub(crate) remember_account: bool,
	pub(crate) tuning: RestTuning,
	pub(crate) startup: Startup,
}

pub(crate) struct Startup {
	pub(crate) gateway: oneshot::Sender<GatewayConfig>,
	pub(crate) gateway_config: GatewayConfig,
	pub(crate) build_source: Option<Url>,
}

pub(crate) struct Core {
	state: SharedState,
	rest: RestClient,
	gateway: GatewayHandle,
	store: Arc<Store>,
	persist: Persistor,
	notifier: Notifier,
	runtime: Handle,
	search: SharedSearch,
	stats: Arc<Stats>,
	internal: mpsc::UnboundedSender<Internal>,
	typing_sent: HashMap<Snowflake, StdInstant>,
	loading: HashSet<(Snowflake, Page)>,
	typing_expiry: Option<Instant>,
	nonce_sequence: u64,
	status: StatusKind,
	afk: bool,
	custom_status: Option<CustomStatus>,
	hooks: Hooks,
	themes: Arc<ThemeCatalogs>,
	locale: String,
	token: Arc<Token>,
	remember_account: bool,
	signed_in: Arc<Mutex<bool>>,
	account_saved: bool,
	select_created_guild: bool,
	startup: Option<Startup>,
	acks: reads::Acks,
	viewing: bool,
	manual_unread: HashSet<Snowflake>,
	deletes: mpsc::UnboundedSender<deletes::DeleteJob>,
	delete_batches: u64,
	subscriptions: subscriptions::Subscriptions,
	member_requests: member_requests::MemberRequests,
}

fn page_query(
	before: Option<Snowflake>,
	after: Option<Snowflake>,
	around: Option<Snowflake>,
) -> MessagesQuery {
	MessagesQuery {
		around,
		before,
		after,
		limit: Some(PAGE_SIZE),
	}
}

fn disconnect_reason(error: &GatewayError) -> DisconnectReason {
	if error.is_authentication_failure() {
		return DisconnectReason::AuthenticationFailed;
	}
	DisconnectReason::Rejected {
		code: error.close_code(),
		message: error.to_string(),
	}
}

fn ack_day() -> i64 {
	(Utc::now().timestamp_millis() - DISCORD_EPOCH_MS) / DAY_MILLIS
}

struct CustomStatus {
	text: String,
	emoji: Option<String>,
}

impl Core {
	pub(crate) fn new(parts: CoreParts) -> Self {
		let persist = Persistor::spawn(parts.store.clone(), &parts.runtime);
		let deletes = deletes::spawn_deleter(parts.rest.clone(), parts.internal.clone(), &parts.runtime);
		Self {
			state: parts.state,
			rest: parts.rest,
			gateway: parts.gateway,
			store: parts.store,
			persist,
			notifier: parts.notifier,
			runtime: parts.runtime,
			search: parts.search,
			stats: parts.stats,
			internal: parts.internal,
			typing_sent: HashMap::new(),
			loading: HashSet::new(),
			typing_expiry: None,
			nonce_sequence: 0,
			status: StatusKind::Online,
			afk: false,
			custom_status: None,
			hooks: parts.hooks,
			themes: parts.themes,
			locale: parts.locale,
			token: parts.token,
			remember_account: parts.remember_account,
			signed_in: Arc::new(Mutex::new(true)),
			account_saved: !parts.remember_account,
			select_created_guild: false,
			startup: Some(parts.startup),
			acks: reads::Acks::new(&parts.tuning),
			viewing: true,
			manual_unread: HashSet::new(),
			deletes,
			delete_batches: 0,
			subscriptions: subscriptions::Subscriptions::default(),
			member_requests: member_requests::MemberRequests::default(),
		}
	}

	pub(crate) async fn run(
		mut self,
		mut gateway_events: mpsc::Receiver<GatewayEvent>,
		mut commands: mpsc::Receiver<Command>,
		mut internal: mpsc::UnboundedReceiver<Internal>,
	) {
		self.start_gateway().await;
		let cookie_saver = self.runtime.spawn(identity::keep_cookies_saved(
			self.rest.identity().clone(),
			self.store.clone(),
		));
		self.restore().await;
		self.restore_recents().await;
		let mut gateway_open = true;
		let rest_stopped = self.rest.stopped();
		tokio::pin!(rest_stopped);
		let mut signed_out = false;
		loop {
			let flush_at = self.notifier.deadline();
			let expire_at = self.typing_expiry;
			let ack_at = self.acks.deadline();
			let lazy_at = self.lazy_deadline();
			tokio::select! {
				event = gateway_events.recv(), if gateway_open => match event {
					Some(event) => self.on_gateway(event),
					None => gateway_open = false,
				},
				command = commands.recv() => match command {
					Some(command) => {
						if !self.on_command(command) {
							break;
						}
					}
					None => break,
				},
				Some(message) = internal.recv() => self.on_internal(message),
				() = &mut rest_stopped, if !signed_out => {
					signed_out = true;
					self.on_token_rejected();
				}
				() = sleep_until(flush_at.unwrap_or_else(Instant::now)), if flush_at.is_some() => {
					self.notifier.flush();
				}
				() = sleep_until(expire_at.unwrap_or_else(Instant::now)), if expire_at.is_some() => {
					self.expire_typing();
				}
				() = sleep_until(ack_at.unwrap_or_else(Instant::now)), if ack_at.is_some() => {
					self.flush_acks();
				}
				() = sleep_until(lazy_at.unwrap_or_else(Instant::now)), if lazy_at.is_some() => {
					self.lazy_tick();
				}
			}
		}
		self.notifier.flush();
		self.gateway.shutdown();
		cookie_saver.abort();
		let identity = self.rest.identity().clone();
		let store = self.store.clone();
		let _ = tokio::task::spawn_blocking(move || identity::save_cookies(&identity, &store)).await;
		self.persist.close().await;
	}

	async fn start_gateway(&mut self) {
		let Some(mut startup) = self.startup.take() else {
			return;
		};
		let identity = self.rest.identity().clone();
		if let Some(source) = &startup.build_source {
			identity::discover_build_number(&identity, &self.store, source).await;
		}
		startup.gateway_config.identify = identity.fingerprint().properties().clone();
		let _ = startup.gateway.send(startup.gateway_config);
	}

	async fn restore(&mut self) {
		let store = self.store.clone();
		let cached = tokio::task::spawn_blocking(move || {
			(persist::load_ready(&store), persist::load_read_states(&store))
		})
		.await;
		let Ok((Some(mut ready), read_states)) = cached else {
			return;
		};
		ready.session_id.clear();
		ready.resume_gateway_url = None;
		self.apply(Event::Ready(&ready));
		for read_state in read_states {
			let Some(message_id) = read_state.last_message_id else {
				continue;
			};
			self.apply_dispatch(&DispatchEvent::MessageAck(MessageAck {
				channel_id: Snowflake(read_state.channel_id),
				message_id: Snowflake(message_id),
				version: None,
				mention_count: Some(i64::from(read_state.mention_count)),
			}));
		}
		self.apply_local(LocalEvent::Connection(ConnectionStatus::Connecting));
	}

	fn apply(&mut self, event: Event<'_>) {
		let (changes, fresh) = {
			let mut state = self.state.write();
			let changes = state.apply(&event);
			(changes, state.take_unrendered())
		};
		if !fresh.is_empty() {
			self.render_messages(fresh);
		}
		if !changes.typing.is_empty() {
			self.typing_expiry = Some(Instant::now() + TYPING_TIMEOUT + TYPING_EXPIRY_SLACK);
		}
		self.notifier.push(changes);
	}

	fn apply_local(&mut self, event: LocalEvent) {
		self.apply(Event::Local(event));
	}

	fn apply_dispatch(&mut self, event: &DispatchEvent) {
		self.apply(Event::Dispatch(event));
	}

	fn expire_typing(&mut self) {
		self.typing_expiry = None;
		let changes = self.state.write().expire(StdInstant::now());
		self.notifier.push(changes);
	}

	fn on_token_rejected(&mut self) {
		tracing::warn!("the token was rejected, signing out");
		self.gateway.shutdown();
		self.apply_local(LocalEvent::Connection(ConnectionStatus::Disconnected(
			DisconnectReason::AuthenticationFailed,
		)));
	}

	fn on_gateway(&mut self, event: GatewayEvent) {
		if self.rest.is_stopped() && !matches!(event, GatewayEvent::Fatal(_)) {
			return;
		}
		match event {
			GatewayEvent::Connecting { resuming, .. } => {
				let status = if resuming {
					ConnectionStatus::Resuming
				} else {
					self.lazy_offline();
					ConnectionStatus::Connecting
				};
				self.apply_local(LocalEvent::Connection(status));
			}
			GatewayEvent::Hello { .. } => {}
			GatewayEvent::TokenRefreshed(token) => self.adopt_token(token),
			GatewayEvent::Ready(ready) => {
				if let Some(id) = ready.installation_id() {
					identity::save_installation_id(self.rest.identity(), &self.store, &self.runtime, id);
				}
				self.apply(Event::Ready(&ready));
				self.remember_account();
				self.lazy_connected();
				self.persist.submit(Job::Ready(ready));
			}
			GatewayEvent::ReadySupplemental(supplemental) => {
				self.apply(Event::ReadySupplemental(&supplemental));
			}
			GatewayEvent::Resumed => {
				self.apply_dispatch(&DispatchEvent::Resumed);
			}
			GatewayEvent::Dispatch { event, .. } => self.on_dispatch(event),
			GatewayEvent::Disconnected {
				code,
				reason,
				will_resume,
				attempt,
				retry_in,
			} => {
				tracing::info!(
					?code,
					reason,
					will_resume,
					attempt,
					?retry_in,
					"gateway disconnected"
				);
				self.stats.reconnects.fetch_add(1, Ordering::Relaxed);
				self.apply_local(LocalEvent::Connection(ConnectionStatus::Reconnecting {
					attempt,
					retry_in,
				}));
			}
			GatewayEvent::Fatal(error) => {
				tracing::error!(%error, "gateway stopped");
				if error.is_authentication_failure() {
					self.rest.stop_all();
				}
				self.apply_local(LocalEvent::Connection(ConnectionStatus::Disconnected(
					disconnect_reason(&error),
				)));
			}
			GatewayEvent::Latency { heartbeat_rtt } => {
				self.apply_local(LocalEvent::LatencyMeasured(heartbeat_rtt));
			}
			GatewayEvent::DecodeFailure { event } => {
				tracing::debug!(event, "gateway event could not be decoded");
			}
		}
	}

	fn on_dispatch(&mut self, event: DispatchEvent) {
		self.apply_dispatch(&event);
		guarded(&self.hooks, "on_dispatch", |hooks| hooks.on_dispatch(&event));
		match event {
			DispatchEvent::MessageCreate(message) => {
				let channel_id = message.channel_id;
				self.persist.submit(Job::Messages {
					channel_id,
					messages: vec![Arc::from(message)],
				});
				self.persist_read_state(channel_id);
				self.read_if_viewing(channel_id);
			}
			DispatchEvent::MessageUpdate(update) => self.persist_cached_message(update.channel_id, update.id),
			DispatchEvent::MessageReactionAdd(reaction) => {
				self.persist_cached_message(reaction.channel_id, reaction.message_id);
			}
			DispatchEvent::MessageReactionRemove(reaction) => {
				self.persist_cached_message(reaction.channel_id, reaction.message_id);
			}
			DispatchEvent::MessageReactionRemoveAll(reaction) => {
				self.persist_cached_message(reaction.channel_id, reaction.message_id);
			}
			DispatchEvent::MessageReactionRemoveEmoji(reaction) => {
				self.persist_cached_message(reaction.channel_id, reaction.message_id);
			}
			DispatchEvent::MessageDelete(delete) => {
				self.persist.submit(Job::DeleteMessages(vec![delete.id]));
			}
			DispatchEvent::MessageDeleteBulk(delete) => {
				self.persist.submit(Job::DeleteMessages(delete.ids));
			}
			DispatchEvent::MessageAck(ack) => self.persist_read_state(ack.channel_id),
			DispatchEvent::ChannelPinsUpdate(update) => self.reload_pins(update.channel_id),
			DispatchEvent::GuildCreate(guild) => self.select_new_guild(guild.id),
			DispatchEvent::GuildMembersChunk(chunk) => self.member_requests.answered(&chunk, Instant::now()),
			DispatchEvent::RateLimited(limited) => self.on_rate_limited(&limited),
			_ => {}
		}
	}

	fn persist_cached_message(&self, channel_id: Snowflake, message_id: Snowflake) {
		let message = self
			.state
			.read()
			.messages(channel_id)
			.and_then(|cache| cache.by_id.get(&message_id).cloned());
		if let Some(message) = message {
			self.persist.submit(Job::Messages {
				channel_id,
				messages: vec![message],
			});
		}
	}

	fn persist_read_state(&self, channel_id: Snowflake) {
		let stored = self
			.state
			.read()
			.read_state(channel_id)
			.map(persist::stored_read_state);
		if let Some(stored) = stored {
			self.persist.submit(Job::ReadState(stored));
		}
	}

	fn on_internal(&mut self, message: Internal) {
		match message {
			Internal::Messages {
				channel_id,
				direction,
				messages,
				reached_start,
				cache,
			} => self.on_messages(channel_id, direction, messages, reached_start, cache),
			Internal::LoadFinished(channel_id, page) => {
				self.loading.remove(&(channel_id, page));
			}
			Internal::SendFailed(nonce, reason) => {
				self.apply_local(LocalEvent::PendingFailed { nonce, reason })
			}
			Internal::Search(results) => {
				*self.search.lock() = Some(Arc::new(results));
				self.notifier.push(Changes {
					search: true,
					..Changes::default()
				});
			}
			Internal::DmOpened(channel) => {
				let channel_id = channel.id;
				self.apply_dispatch(&DispatchEvent::ChannelCreate(channel));
				self.select_channel(None, channel_id);
			}
			Internal::ThreadOpened(thread) => self.on_thread_opened(thread),
			Internal::Loaded(loaded) => self.on_loaded(loaded),
			Internal::Rejected(reason) => self.reject(reason),
			Internal::Draft { channel_id, content } => {
				if self.state.read().draft(channel_id).is_none() {
					self.apply_local(LocalEvent::DraftChanged { channel_id, content });
				}
			}
			Internal::BeforeSend {
				nonce,
				channel_id,
				content,
				reply_to,
				files,
				rewritten,
			} => self.on_before_send(nonce, channel_id, content, reply_to, files, rewritten),
			Internal::ThemeCatalogChanged => self.notifier.push(Changes {
				theme_catalog: true,
				..Changes::default()
			}),
			Internal::AccountSwitch(user_id) => self.apply_local(LocalEvent::AccountSwitchRequested(user_id)),
			Internal::DeleteProgressed {
				batch,
				message_id,
				status,
			} => self.apply_local(LocalEvent::DeleteProgressed {
				batch,
				message_id,
				status,
			}),
		}
	}

	fn on_messages(
		&mut self,
		channel_id: Snowflake,
		direction: LoadDirection,
		messages: Vec<Message>,
		reached_start: bool,
		cache: bool,
	) {
		let ids: Vec<Snowflake> = if cache {
			messages.iter().map(|message| message.id).collect()
		} else {
			Vec::new()
		};
		self.apply(Event::MessagesLoaded {
			channel_id,
			messages,
			direction,
			reached_start,
		});
		if !cache {
			return;
		}
		let stored: Vec<Arc<Message>> = self
			.state
			.read()
			.messages(channel_id)
			.map(|cached| {
				ids.iter()
					.filter_map(|id| cached.by_id.get(id).cloned())
					.collect()
			})
			.unwrap_or_default();
		if !stored.is_empty() {
			self.persist.submit(Job::Messages {
				channel_id,
				messages: stored,
			});
		}
	}

	fn on_command(&mut self, command: Command) -> bool {
		if self.held_by_lock(&command) {
			return true;
		}
		match command {
			Command::ConfirmRisky { target, reason } => self.confirm_risky(target, reason),
			Command::SelectChannel { guild_id, channel_id } => self.select_channel(guild_id, channel_id),
			Command::LoadOlder { channel_id } => self.load_older(channel_id),
			Command::LoadNewer { channel_id } => self.load_newer(channel_id),
			Command::SetViewport { channel_id, viewport } => {
				self.apply_local(LocalEvent::ViewportChanged { channel_id, viewport });
				self.read_if_viewing(channel_id);
			}
			Command::LoadAround {
				channel_id,
				message_id,
			} => self.load_page(
				channel_id,
				None,
				LoadDirection::Around,
				page_query(None, None, Some(message_id)),
			),
			Command::SendMessage {
				channel_id,
				content,
				reply_to,
				files,
			} => self.send_message(channel_id, content, reply_to, files),
			Command::EditMessage {
				channel_id,
				message_id,
				content,
			} => self.edit_message(channel_id, message_id, content),
			Command::DeleteMessage {
				channel_id,
				message_id,
			} => self.delete_messages(channel_id, vec![message_id]),
			Command::DeleteMessages { channel_id, ids } => self.delete_messages(channel_id, ids),
			Command::AddReaction {
				channel_id,
				message_id,
				emoji,
			} => {
				self.remember_reaction(&emoji);
				self.spawn_rest("add reaction", move |rest| async move {
					rest.add_reaction(channel_id, message_id, &emoji).await
				});
			}
			Command::RemoveReaction {
				channel_id,
				message_id,
				emoji,
			} => self.spawn_rest("remove reaction", move |rest| async move {
				rest.remove_own_reaction(channel_id, message_id, &emoji).await
			}),
			Command::Typing { channel_id } => self.typing(channel_id),
			Command::AckChannel {
				channel_id,
				message_id,
			} => self.read_up_to(channel_id, message_id),
			Command::SetViewing(viewing) => self.set_viewing(viewing),
			Command::SetDraft { channel_id, content } => {
				self.persist.submit(Job::Draft {
					channel_id,
					content: content.clone(),
				});
				self.apply_local(LocalEvent::DraftChanged { channel_id, content });
			}
			Command::SetPresence {
				status,
				custom_status,
			} => self.set_presence_command(status, custom_status),
			Command::SetAfk(afk) => self.set_afk(afk),
			Command::SubscribeGuild { guild_id } => {
				let channel_id = self.subscription_channel(guild_id);
				self.subscriptions.touch(guild_id, channel_id, Instant::now());
			}
			Command::SetMemberViewport {
				guild_id,
				channel_id,
				first,
				last,
			} => self.subscriptions.member_viewport(
				guild_id,
				channel_id,
				subscriptions::MemberViewport { first, last },
				Instant::now(),
			),
			Command::SearchMembers { guild_id, query } => {
				self.member_requests.search(guild_id, query, Instant::now());
			}
			Command::RequestMembers { guild_id, user_ids } => self.request_members(guild_id, user_ids),
			Command::OpenDm { user_id } => self.open_dm(user_id),
			Command::Search {
				guild_id,
				channel_id,
				query,
			} => self.search(guild_id, channel_id, query),
			Command::ToggleCategory { category_id, .. } => {
				let collapsed = !self.state.read().is_collapsed(category_id);
				self.apply_local(LocalEvent::CategoryCollapsed {
					channel_id: category_id,
					collapsed,
				});
			}
			Command::SwitchAccount(user_id) => self.switch_account(user_id),
			command @ (Command::ThemeCatalogRefresh { .. } | Command::ThemeInstall { .. }) => {
				self.on_theme_command(command)
			}
			Command::Reconnect => self.gateway.reconnect(),
			Command::Logout => {
				let mut signed_in = self.signed_in.lock();
				*signed_in = false;
				self.store.tokens.clear();
				self.forget_account();
				drop(signed_in);
				persist::clear_cache(&self.store);
				self.rest.clear_token();
				return false;
			}
			Command::Shutdown => return false,
			command => self.on_extra(command),
		}
		true
	}

	fn spawn_rest<F, Fut>(&self, what: &'static str, call: F)
	where
		F: FnOnce(RestClient) -> Fut,
		Fut: Future<Output = rest::Result<()>> + Send + 'static,
	{
		let request = call(self.rest.clone());
		self.runtime.spawn(async move {
			if let Err(error) = request.await {
				tracing::warn!(%error, what, "rest call failed");
			}
		});
	}

	fn select_channel(&mut self, guild_id: Option<Snowflake>, channel_id: Snowflake) {
		self.rest.set_referer(&channel_referer(guild_id, channel_id));
		self.apply_local(LocalEvent::ChannelSelected { guild_id, channel_id });
		match guild_id {
			Some(guild_id) => self
				.subscriptions
				.open(guild_id, Some(channel_id), Instant::now()),
			None => self.subscriptions.leave_guilds(),
		}
		let (has_messages, has_draft) = {
			let state = self.state.read();
			(
				state
					.messages(channel_id)
					.is_some_and(|cache| !cache.ordered.is_empty()),
				state.draft(channel_id).is_some(),
			)
		};
		if !has_messages {
			self.load_initial(channel_id);
		}
		if !has_draft {
			self.load_draft(channel_id);
		}
		self.on_channel_opened(channel_id);
	}

	fn load_initial(&self, channel_id: Snowflake) {
		let rest = self.rest.clone();
		let store = self.store.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let cached = tokio::task::spawn_blocking(move || persist::load_messages(&store, channel_id))
				.await
				.unwrap_or_default();
			let had_cache = !cached.is_empty();
			if had_cache {
				let _ = internal.send(Internal::Messages {
					channel_id,
					direction: LoadDirection::History,
					messages: cached,
					reached_start: false,
					cache: false,
				});
			}
			match rest.messages(channel_id, page_query(None, None, None)).await {
				Ok(messages) => {
					let messages = messages.into_vec();
					let _ = internal.send(Internal::Messages {
						channel_id,
						direction: if had_cache {
							LoadDirection::Around
						} else {
							LoadDirection::History
						},
						reached_start: messages.len() < usize::from(PAGE_SIZE),
						messages,
						cache: true,
					});
				}
				Err(error) => tracing::warn!(%error, "could not load messages"),
			}
		});
	}

	fn load_draft(&self, channel_id: Snowflake) {
		let store = self.store.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let draft = tokio::task::spawn_blocking(move || persist::load_draft(&store, channel_id))
				.await
				.ok()
				.flatten();
			if let Some(content) = draft {
				let _ = internal.send(Internal::Draft { channel_id, content });
			}
		});
	}

	fn load_older(&mut self, channel_id: Snowflake) {
		let before = {
			let state = self.state.read();
			let Some(cache) = state.messages(channel_id) else {
				return;
			};
			if cache.reached_start {
				return;
			}
			cache.ordered.first().map(|message| message.id)
		};
		self.load_page(
			channel_id,
			Some(Page::Older),
			LoadDirection::History,
			page_query(before, None, None),
		);
	}

	fn load_newer(&mut self, channel_id: Snowflake) {
		let after = {
			let state = self.state.read();
			let Some(cache) = state.messages(channel_id) else {
				return;
			};
			if !cache.is_detached() {
				return;
			}
			cache.newest_id()
		};
		self.load_page(
			channel_id,
			Some(Page::Newer),
			LoadDirection::Newer,
			page_query(None, after, None),
		);
	}

	fn load_page(
		&mut self,
		channel_id: Snowflake,
		page: Option<Page>,
		direction: LoadDirection,
		query: MessagesQuery,
	) {
		if let Some(page) = page
			&& !self.loading.insert((channel_id, page))
		{
			return;
		}
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let loaded = rest.messages(channel_id, query).await;
			if let Some(page) = page {
				let _ = internal.send(Internal::LoadFinished(channel_id, page));
			}
			match loaded {
				Ok(messages) => {
					let messages = messages.into_vec();
					let _ = internal.send(Internal::Messages {
						channel_id,
						direction,
						reached_start: messages.len() < usize::from(PAGE_SIZE),
						messages,
						cache: true,
					});
				}
				Err(error) => tracing::warn!(%error, "could not load messages"),
			}
		});
	}

	fn next_nonce(&mut self) -> Nonce {
		self.nonce_sequence = self.nonce_sequence.wrapping_add(1);
		let elapsed = (Utc::now().timestamp_millis() - DISCORD_EPOCH_MS).max(0) as u64;
		let sequence = self.nonce_sequence & ((1 << NONCE_SEQUENCE_BITS) - 1);
		Nonce(((elapsed << NONCE_SEQUENCE_BITS) | sequence).to_string())
	}

	fn current_user(&self) -> Option<User> {
		self.state.read().current_user().cloned()
	}

	fn send_message(
		&mut self,
		channel_id: Snowflake,
		content: String,
		reply_to: Option<Snowflake>,
		files: Vec<UploadFile>,
	) {
		let Some(author) = self.current_user() else {
			tracing::warn!("message not sent, no signed in user");
			return;
		};
		self.remember_message_emoji(&content);
		let nonce = self.next_nonce();
		self.apply_local(LocalEvent::PendingMessage {
			channel_id,
			nonce: nonce.clone(),
			content: content.clone(),
			author: Box::new(author),
			by_plugin: None,
		});
		if self.hooks.is_some() && !content.is_empty() {
			self.hook_before_send(nonce, channel_id, content, reply_to, files);
		} else {
			self.dispatch_send(nonce, channel_id, content, reply_to, files);
		}
	}

	fn hook_before_send(
		&self,
		nonce: Nonce,
		channel_id: Snowflake,
		content: String,
		reply_to: Option<Snowflake>,
		files: Vec<UploadFile>,
	) {
		let hooks = self.hooks.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let original = content.clone();
			let rewritten = guarded_blocking(&hooks, "before_send", move |hooks| {
				hooks.before_send(channel_id, &original)
			})
			.await
			.flatten()
			.filter(|rewritten| *rewritten != content);
			let _ = internal.send(Internal::BeforeSend {
				nonce,
				channel_id,
				content,
				reply_to,
				files,
				rewritten,
			});
		});
	}

	fn on_before_send(
		&mut self,
		nonce: Nonce,
		channel_id: Snowflake,
		content: String,
		reply_to: Option<Snowflake>,
		files: Vec<UploadFile>,
		rewritten: Option<String>,
	) {
		let Some(rewritten) = rewritten else {
			self.dispatch_send(nonce, channel_id, content, reply_to, files);
			return;
		};
		let by = self
			.hooks
			.as_deref()
			.map_or_else(String::new, |hooks| hooks.name().to_owned());
		self.apply_local(LocalEvent::PendingAltered {
			nonce: nonce.clone(),
			content: rewritten.clone(),
			by,
		});
		self.dispatch_send(nonce, channel_id, rewritten, reply_to, files);
	}

	fn edit_message(&self, channel_id: Snowflake, message_id: Snowflake, content: String) {
		let hooks = self.hooks.clone();
		let rest = self.rest.clone();
		self.runtime.spawn(async move {
			let original = content.clone();
			let content = guarded_blocking(&hooks, "before_edit", move |hooks| {
				hooks.before_edit(channel_id, message_id, &original)
			})
			.await
			.flatten()
			.unwrap_or(content);
			let body = EditMessage {
				content: Some(content),
				..EditMessage::default()
			};
			if let Err(error) = rest.edit_message(channel_id, message_id, body).await {
				tracing::warn!(%error, what = "edit message", "rest call failed");
			}
		});
	}

	fn render_messages(&mut self, messages: Vec<Arc<Message>>) {
		let mut state = self.state.write();
		for message in messages {
			let rendered = Arc::new(RenderedMessage::plain(&message.content, false, RENDER_REVISION));
			state.apply(&Event::Local(LocalEvent::MessageRendered {
				channel_id: message.channel_id,
				id: message.id,
				source: message,
				rendered,
			}));
		}
	}

	fn dispatch_send(
		&mut self,
		nonce: Nonce,
		channel_id: Snowflake,
		content: String,
		reply_to: Option<Snowflake>,
		files: Vec<UploadFile>,
	) {
		let body = CreateMessage {
			content: (!content.is_empty()).then_some(content),
			nonce: Some(nonce.0.clone()),
			message_reference: reply_to.map(|message_id| MessageReference {
				message_id: Some(message_id),
				channel_id: Some(channel_id),
				fail_if_not_exists: Some(false),
				..MessageReference::default()
			}),
			..CreateMessage::default()
		};
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let sent = if files.is_empty() {
				rest.create_message(channel_id, body).await
			} else {
				rest.create_message_with_files(channel_id, body, files).await
			};
			if let Err(error) = sent {
				tracing::warn!(%error, "message was not sent");
				let _ = internal.send(Internal::SendFailed(nonce, failures::send_failure(&error)));
			}
		});
	}

	fn typing(&mut self, channel_id: Snowflake) {
		let now = StdInstant::now();
		if self
			.typing_sent
			.get(&channel_id)
			.is_some_and(|sent| now.duration_since(*sent) < TYPING_THROTTLE)
		{
			return;
		}
		self.typing_sent.insert(channel_id, now);
		let hooks = self.hooks.clone();
		self.spawn_rest("typing", move |rest| async move {
			let allowed = guarded_blocking(&hooks, "allow_typing", move |hooks| {
				hooks.allow_typing(channel_id)
			})
			.await
			.unwrap_or(true);
			if !allowed {
				return Ok(());
			}
			rest.trigger_typing(channel_id).await
		});
	}

	fn ack_local(&mut self, channel_id: Snowflake, message_id: Snowflake) -> bool {
		let already_read = self
			.state
			.read()
			.read_state(channel_id)
			.is_some_and(|read_state| {
				read_state.last_message_id >= Some(message_id) && read_state.mention_count.unwrap_or(0) == 0
			});
		if already_read {
			return false;
		}
		self.apply_dispatch(&DispatchEvent::MessageAck(MessageAck {
			channel_id,
			message_id,
			version: None,
			mention_count: Some(0),
		}));
		self.persist_read_state(channel_id);
		true
	}

	fn subscription_channel(&self, guild_id: Snowflake) -> Option<Snowflake> {
		let state = self.state.read();
		state
			.selected()
			.filter(|selected| selected.guild_id == Some(guild_id))
			.map(|selected| selected.channel_id)
			.or_else(|| {
				state
					.guild(guild_id)?
					.channels
					.values()
					.find_map(|channel| (channel.kind == ChannelType::GuildText).then_some(channel.id))
			})
	}

	pub(crate) fn request_members(&mut self, guild_id: Snowflake, user_ids: Vec<Snowflake>) {
		self.member_requests.want_ids(guild_id, user_ids, Instant::now());
	}

	fn lazy_deadline(&self) -> Option<Instant> {
		[self.subscriptions.deadline(), self.member_requests.deadline()]
			.into_iter()
			.flatten()
			.min()
	}

	fn lazy_tick(&mut self) {
		let now = Instant::now();
		if self.subscriptions.deadline().is_some_and(|due| due <= now) {
			self.flush_subscriptions();
		}
		for request in self.member_requests.due(now) {
			let lookup = match &request.lookup {
				member_requests::Lookup::Ids(ids) => MemberLookup::Ids(ids),
				member_requests::Lookup::Query(query) => MemberLookup::Query {
					query,
					limit: member_requests::SEARCH_LIMIT,
				},
			};
			let sent = self.gateway.request_members(MemberRequest {
				guild_id: request.guild_id,
				nonce: &request.nonce,
				lookup,
			});
			if let Err(error) = sent {
				tracing::warn!(%error, "member request was not sent");
				self.member_requests.unsent(request, now);
			}
		}
	}

	fn flush_subscriptions(&mut self) {
		let Some(subscriptions) = self.subscriptions.flush() else {
			return;
		};
		if let Err(error) = self.gateway.update_guild_subscriptions(subscriptions) {
			tracing::warn!(%error, "guild subscriptions were not sent");
		}
	}

	fn lazy_connected(&mut self) {
		let now = Instant::now();
		let open = self
			.state
			.read()
			.selected()
			.and_then(|selected| Some((selected.guild_id?, selected.channel_id)));
		self.subscriptions.connected(open, now);
		self.member_requests.connected(now);
	}

	fn lazy_offline(&mut self) {
		self.subscriptions.offline();
		self.member_requests.offline();
	}

	fn on_rate_limited(&mut self, limited: &GatewayRateLimited) {
		tracing::info!(opcode = ?limited.opcode, retry_after = ?limited.retry_after, "gateway rate limited");
		if limited.opcode.map(Opcode::from) == Some(Opcode::RequestGuildMembers) {
			self.member_requests.rate_limited(limited, Instant::now());
		}
	}

	fn open_dm(&self, user_id: Snowflake) {
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			match rest.create_dm(user_id).await {
				Ok(channel) => {
					let _ = internal.send(Internal::DmOpened(Box::new(channel)));
				}
				Err(error) => tracing::warn!(%error, "could not open the conversation"),
			}
		});
	}

	fn search(&self, guild_id: Option<Snowflake>, channel_id: Option<Snowflake>, query: SearchQuery) {
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			match rest.search_messages(guild_id, channel_id, query).await {
				Ok(response) => {
					let _ = internal.send(Internal::Search(SearchResults::from_response(&response)));
				}
				Err(error) => tracing::warn!(%error, "search failed"),
			}
		});
	}
}
