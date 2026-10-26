use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant as StdInstant};

use chrono::Utc;
use gateway::{
	ClientStateSource, GatewayConfig, GatewayError, GatewayEvent, GatewayHandle, MemberLookup, MemberRequest,
};
use parking_lot::Mutex;
use protocol::fingerprint::channel_referer;
use protocol::{
	Channel, ChannelType, DISCORD_EPOCH_MS, DispatchEvent, EditMessage, GatewayRateLimited, Message,
	MessageAck, MessagesQuery, Nonce, Opcode, Snowflake, Token, User,
};
use rest::{RestClient, RestTuning};
use state::{
	Changes, ConnectionStatus, DeleteStatus, DisconnectReason, Event, LoadDirection, Loaded, LocalEvent,
	RenderedMessage, SendFailure, SharedState, TYPING_TIMEOUT,
};
use store::Store;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, sleep_until};
use url::Url;

use crate::alerts::{Alert, AlertPacing, AlertQueue, Pacing};
use crate::command::Command;
use crate::hooks::{Hooks, Rewrite, guarded, guarded_blocking};
use crate::identity;
use crate::notifier::Notifier;
use crate::persist::{self, Job, Persistor};
use crate::search::SearchResults;
use crate::theme_links::{ThemeLinkTuning, ThemeLinks};

mod accounts;
mod actions;
mod alerts;
mod backfill;
mod collapse;
mod commands;
mod community;
mod components;
mod deletes;
mod embeds;
mod emoji_rank;
mod expressions;
mod failures;
mod favorite_writes;
mod guild_writes;
mod hidden;
mod interactions;
mod layout;
mod locks;
mod member_requests;
mod moderation;
mod outbox;
mod profiles;
mod reactions;
mod reads;
mod search;
mod status;
mod storage;
mod subscriptions;
mod themes;
mod uploads;

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
	SendFailed(Nonce, SendFailure, Box<outbox::Outgoing>),
	Uploading(Nonce, u64),
	Searched {
		serial: u64,
		offset: u32,
		outcome: search::Outcome,
	},
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
		outgoing: Box<outbox::Outgoing>,
		rewritten: Option<Rewrite>,
	},
	Edited {
		channel_id: Snowflake,
		message_id: Snowflake,
		altered_by: Option<String>,
	},
	CommandIndex {
		scope: Snowflake,
		index: Option<Box<protocol::ApplicationCommandIndex>>,
	},
	AutocompleteDue(u64),
	AutocompleteFailed(u64),
	CommandFailed {
		nonce: Nonce,
		reason: String,
	},
	InteractionExpired {
		channel_id: Snowflake,
		id: Snowflake,
	},
	ComponentFailed(String),
	ModalFailed(String),
	ThemeLinksChanged,
	Alert(Alert),
	AlertVetoed(StdInstant),
	AccountSwitch(Snowflake),
	SecretHealth(store::SecretHealth),
	DeleteProgressed {
		batch: u64,
		message_id: Snowflake,
		status: DeleteStatus,
	},
	Backfilled {
		job: backfill::Job,
		outcome: backfill::Outcome,
	},
	Kept {
		channel_id: Snowflake,
		kept: persist::Kept,
	},
	LayoutSaved(bool),
	CollapseSaved(bool),
	GuildWritten {
		guild_id: Snowflake,
		outcome: rest::Result<()>,
	},
	StatusSaved(Option<String>),
	Profile {
		user_id: Snowflake,
		guild_id: Option<Snowflake>,
		outcome: profiles::ProfileOutcome,
	},
	EmojiRanked(Option<state::EmojiRanking>),
	FavoritesSaved(favorite_writes::FavoriteKind, bool),
}

pub(crate) struct CoreParts {
	pub(crate) state: SharedState,
	pub(crate) rest: RestClient,
	pub(crate) gateway: GatewayHandle,
	pub(crate) store: Arc<Store>,
	pub(crate) notifier: Notifier,
	pub(crate) runtime: Handle,
	pub(crate) search: SharedSearch,
	pub(crate) alerts: AlertQueue,
	pub(crate) pacing: AlertPacing,
	pub(crate) stats: Arc<Stats>,
	pub(crate) internal: mpsc::UnboundedSender<Internal>,
	pub(crate) hooks: Hooks,
	pub(crate) themes: Arc<ThemeLinks>,
	pub(crate) theme_tuning: ThemeLinkTuning,
	pub(crate) locale: String,
	pub(crate) token: Arc<Token>,
	pub(crate) remember_account: bool,
	pub(crate) tuning: RestTuning,
	pub(crate) retention: store::Retention,
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
	alerts: AlertQueue,
	pacing: Pacing,
	stats: Arc<Stats>,
	internal: mpsc::UnboundedSender<Internal>,
	typing_sent: HashMap<Snowflake, StdInstant>,
	loading: HashSet<(Snowflake, Page)>,
	typing_expiry: Option<Instant>,
	nonce_sequence: u64,
	status: status::StatusWriter,
	profiles: profiles::Profiles,
	afk: bool,
	hooks: Hooks,
	themes: Arc<ThemeLinks>,
	theme_tuning: ThemeLinkTuning,
	locale: String,
	token: Arc<Token>,
	remember_account: bool,
	signed_in: Arc<Mutex<bool>>,
	sign_out: Option<storage::SignOut>,
	retention: store::Retention,
	account_saved: bool,
	select_created_guild: bool,
	startup: Option<Startup>,
	acks: reads::Acks,
	layout: layout::LayoutWriter,
	collapse: collapse::CollapseWriter,
	guild_writes: guild_writes::GuildWrites,
	searcher: search::Searcher,
	viewing: bool,
	manual_unread: HashSet<Snowflake>,
	deletes: mpsc::UnboundedSender<deletes::DeleteJob>,
	delete_batches: u64,
	subscriptions: subscriptions::Subscriptions,
	member_requests: member_requests::MemberRequests,
	backfill: backfill::Backfill,
	outbox: outbox::Outbox,
	interactions: interactions::Interactions,
	components: components::Components,
	ranker: emoji_rank::EmojiRanker,
	favorite_writes: favorite_writes::FavoriteWrites,
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
			alerts: parts.alerts,
			pacing: Pacing::new(parts.pacing),
			stats: parts.stats,
			internal: parts.internal,
			typing_sent: HashMap::new(),
			loading: HashSet::new(),
			typing_expiry: None,
			nonce_sequence: 0,
			status: status::StatusWriter::new(&parts.tuning),
			profiles: profiles::Profiles::default(),
			afk: false,
			hooks: parts.hooks,
			themes: parts.themes,
			theme_tuning: parts.theme_tuning,
			locale: parts.locale,
			token: parts.token,
			remember_account: parts.remember_account,
			signed_in: Arc::new(Mutex::new(true)),
			sign_out: None,
			retention: parts.retention,
			account_saved: !parts.remember_account,
			select_created_guild: false,
			startup: Some(parts.startup),
			acks: reads::Acks::new(&parts.tuning),
			layout: layout::LayoutWriter::new(&parts.tuning),
			collapse: collapse::CollapseWriter::new(&parts.tuning),
			guild_writes: guild_writes::GuildWrites::new(&parts.tuning),
			searcher: search::Searcher::new(&parts.tuning),
			viewing: true,
			manual_unread: HashSet::new(),
			deletes,
			delete_batches: 0,
			subscriptions: subscriptions::Subscriptions::default(),
			member_requests: member_requests::MemberRequests::default(),
			backfill: backfill::Backfill::new(&parts.tuning),
			outbox: outbox::Outbox::default(),
			interactions: interactions::Interactions::default(),
			components: components::Components::default(),
			ranker: emoji_rank::EmojiRanker::default(),
			favorite_writes: favorite_writes::FavoriteWrites::new(&parts.tuning),
		}
	}

	pub(crate) async fn run(
		mut self,
		mut gateway_events: mpsc::Receiver<GatewayEvent>,
		mut commands: mpsc::Receiver<Command>,
		mut internal: mpsc::UnboundedReceiver<Internal>,
	) {
		self.publish_secret_health(self.store.secret_health());
		let store = self.store.clone();
		let cached = tokio::task::spawn_blocking(move || persist::load_cache(&store));
		let startup = self.startup.take();
		if let Some(source) = startup.as_ref().and_then(|startup| startup.build_source.as_ref()) {
			identity::discover_build_number(self.rest.identity(), &self.store, source).await;
		}
		if let Ok(Some(ready)) = cached.await {
			self.apply(Event::Ready(&ready));
		}
		self.apply_local(LocalEvent::Connection(ConnectionStatus::Connecting));
		if let Some(startup) = startup {
			self.start_gateway(startup);
		}
		let cookie_saver = self.runtime.spawn(identity::keep_cookies_saved(
			self.rest.identity().clone(),
			self.store.clone(),
		));
		self.restore_recents().await;
		let pruner = self.start_pruning();
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
		if self.acks.unconfirmed() {
			self.persist.submit(Job::ForgetVersions);
		}
		cookie_saver.abort();
		pruner.abort();
		let identity = self.rest.identity().clone();
		let store = self.store.clone();
		let _ = tokio::task::spawn_blocking(move || identity::save_cookies(&identity, &store)).await;
		let sign_out = self.sign_out;
		let store = self.store.clone();
		self.persist.close().await;
		storage::finish_sign_out(store, sign_out).await;
	}

	fn start_gateway(&mut self, mut startup: Startup) {
		let state = self.state.clone();
		startup.gateway_config.client_state = ClientStateSource::new(move || state.read().client_state());
		let _ = startup.gateway.send(startup.gateway_config);
	}

	fn persist_versioned(&self) {
		let (states, version, settings) = {
			let state = self.state.read();
			let states = state.read_states().map(persist::stored_read_state).collect();
			(states, state.versions().read_state, state.guild_settings_cache())
		};
		self.persist.submit(Job::ReadStates { states, version });
		self.persist.submit(Job::GuildSettings(settings));
	}

	fn apply(&mut self, event: Event<'_>) {
		let (changes, fresh, kept) = {
			let mut state = self.state.write();
			let changes = state.apply(&event);
			(changes, state.take_unrendered(), state.take_kept_writes())
		};
		if !kept.is_empty() {
			self.persist.submit(Job::Kept(kept));
		}
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
		self.forget_rejected_token();
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
				let event = DispatchEvent::Ready(ready);
				guarded(&self.hooks, "on_dispatch", |hooks| hooks.on_dispatch(&event));
				let DispatchEvent::Ready(ready) = event else {
					return;
				};
				self.remember_account();
				self.lazy_connected();
				self.persist.submit(Job::Ready(ready));
				self.persist_versioned();
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
				self.consider_alert(&message);
				let channel_id = message.channel_id;
				let message_id = message.id;
				self.watch_deferred_reply(&message);
				if !message.is_ephemeral() {
					self.persist.submit(Job::Messages {
						channel_id,
						messages: vec![Arc::from(message)],
					});
				}
				self.persist_read_state(channel_id);
				self.read_if_viewing(channel_id);
				self.backfill_channel(channel_id, Some(message_id));
			}
			DispatchEvent::MessageUpdate(update) => {
				let (channel_id, message_id) = (update.channel_id, update.id);
				if !self.persist_cached_message(channel_id, message_id) {
					self.persist.submit(Job::EditStored(update));
				}
				self.backfill_channel(channel_id, Some(message_id));
			}
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
			DispatchEvent::MessagePollVoteAdd(vote) | DispatchEvent::MessagePollVoteRemove(vote) => {
				self.persist_cached_message(vote.channel_id, vote.message_id);
			}
			DispatchEvent::MessageAck(ack) => {
				self.keep_own_ack(&ack);
				self.persist_read_state(ack.channel_id);
			}
			DispatchEvent::UserGuildSettingsUpdate(_) => {
				self.keep_intended_collapse();
				let settings = self.state.read().guild_settings_cache();
				self.persist.submit(Job::GuildSettings(settings));
			}
			DispatchEvent::UserSettingsProtoUpdate(_) => self.keep_intended_status(),
			DispatchEvent::ChannelPinsUpdate(update) => self.reload_pins(update.channel_id),
			DispatchEvent::GuildCreate(guild) => self.select_new_guild(guild.id),
			DispatchEvent::GuildMembersChunk(chunk) => {
				self.member_requests.answered(&chunk, Instant::now());
				self.backfill_members_answered(&chunk);
			}
			DispatchEvent::RateLimited(limited) => self.on_rate_limited(&limited),
			DispatchEvent::InteractionCreate(value) => self.interaction_created(&value),
			DispatchEvent::InteractionSuccess(value) => self.interaction_settled(&value, false),
			DispatchEvent::InteractionFailure(value) => self.interaction_settled(&value, true),
			DispatchEvent::AutocompleteResponse(response) => self.autocomplete_answered(response),
			DispatchEvent::InteractionModalCreate(modal) => self.modal_created(*modal),
			_ => {}
		}
	}

	fn persist_cached_message(&self, channel_id: Snowflake, message_id: Snowflake) -> bool {
		let (message, deleted) = {
			let state = self.state.read();
			let cache = state.messages(channel_id);
			(
				cache.and_then(|cache| cache.by_id.get(&message_id).cloned()),
				cache.is_some_and(|cache| cache.deleted(message_id).is_some()),
			)
		};
		let Some(message) = message else {
			return false;
		};
		if !deleted {
			self.persist.submit(Job::Messages {
				channel_id,
				messages: vec![message],
			});
		}
		true
	}

	fn persist_read_state(&self, channel_id: Snowflake) {
		let (stored, version) = {
			let state = self.state.read();
			let stored = state.read_state(channel_id).map(persist::stored_read_state);
			(stored, state.versions().read_state)
		};
		if let Some(stored) = stored {
			self.persist.submit(Job::ReadState {
				state: stored,
				version,
			});
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
			Internal::SendFailed(nonce, reason, outgoing) => self.send_failed(nonce, reason, *outgoing),
			Internal::Uploading(nonce, sent) => self.apply_local(LocalEvent::UploadProgress { nonce, sent }),
			Internal::CommandIndex { scope, index } => self.command_index_arrived(scope, index),
			Internal::AutocompleteDue(request) => self.autocomplete_due(request),
			Internal::AutocompleteFailed(request) => self.autocomplete_failed(request),
			Internal::CommandFailed { nonce, reason } => self.command_failed(nonce, reason),
			Internal::InteractionExpired { channel_id, id } => {
				self.apply_local(LocalEvent::InteractionExpired { channel_id, id });
			}
			Internal::ComponentFailed(nonce) => self.component_failed(&nonce),
			Internal::ModalFailed(nonce) => self.modal_failed(&nonce),
			Internal::Searched {
				serial,
				offset,
				outcome,
			} => self.on_searched(serial, offset, outcome),
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
				outgoing,
				rewritten,
			} => self.on_before_send(nonce, *outgoing, rewritten),
			Internal::Edited {
				channel_id,
				message_id,
				altered_by,
			} => self.apply_local(LocalEvent::EditAltered {
				channel_id,
				id: message_id,
				by: altered_by,
			}),
			Internal::Alert(alert) => self.push_alert(alert),
			Internal::AlertVetoed(reserved) => self.release_alert(reserved),
			Internal::ThemeLinksChanged => self.notifier.push(Changes {
				theme_links: true,
				..Changes::default()
			}),
			Internal::AccountSwitch(user_id) => self.apply_local(LocalEvent::AccountSwitchRequested(user_id)),
			Internal::SecretHealth(health) => self.publish_secret_health(health),
			Internal::DeleteProgressed {
				batch,
				message_id,
				status,
			} => self.apply_local(LocalEvent::DeleteProgressed {
				batch,
				message_id,
				status,
			}),
			Internal::Backfilled { job, outcome } => self.on_backfilled(job, outcome),
			Internal::LayoutSaved(saved) => self.on_layout_saved(saved),
			Internal::FavoritesSaved(kind, saved) => self.on_favorites_saved(kind, saved),
			Internal::CollapseSaved(saved) => self.on_collapse_saved(saved),
			Internal::GuildWritten { guild_id, outcome } => self.on_guild_written(guild_id, outcome),
			Internal::StatusSaved(failure) => self.on_status_saved(failure),
			Internal::Profile {
				user_id,
				guild_id,
				outcome,
			} => self.on_profile(user_id, guild_id, outcome),
			Internal::Kept { channel_id, kept } => self.apply_local(LocalEvent::KeptRestored {
				channel_id,
				deleted: kept.deleted,
				versions: kept.versions,
			}),
			Internal::EmojiRanked(ranking) => self.emoji_ranked(ranking),
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
		self.backfill_channel(channel_id, None);
		if !cache {
			return;
		}
		let stored: Vec<Arc<Message>> = self
			.state
			.read()
			.messages(channel_id)
			.map(|cached| {
				ids.iter()
					.filter(|id| cached.deleted(**id).is_none())
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
		if self.refuses_hidden(&command) || self.held_by_lock(&command) {
			return true;
		}
		match command {
			Command::ConfirmRisky { target, reason } => self.confirm_risky(target, reason),
			Command::SelectChannel { guild_id, channel_id } => self.select_channel(guild_id, channel_id),
			Command::LoadOlder { channel_id } => self.load_older(channel_id),
			Command::LoadNewer { channel_id } => self.load_newer(channel_id),
			Command::RefreshMedia {
				channel_id,
				message_id,
			} => self.refresh_media(channel_id, message_id),
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
				ping_reply,
				files,
			} => self.send_message(outbox::Outgoing {
				channel_id,
				content,
				reply_to,
				ping_reply,
				files,
				stickers: Vec::new(),
				poll: None,
			}),
			Command::RetryMessage { channel_id, nonce } => self.retry_message(channel_id, nonce),
			Command::DiscardMessage { channel_id, nonce } => self.discard_message(channel_id, nonce),
			Command::DismissMessage {
				channel_id,
				message_id,
			} => self.dismiss_message(channel_id, message_id),
			Command::LoadApplicationCommands { channel_id } => self.load_commands(channel_id),
			Command::RunApplicationCommand {
				channel_id,
				command_id,
				options,
				files,
			} => self.run_command(channel_id, command_id, options, files),
			Command::AutocompleteCommand {
				channel_id,
				command_id,
				options,
				request,
			} => self.autocomplete(channel_id, command_id, options, request),
			Command::PressComponent {
				channel_id,
				message_id,
				custom_id,
				kind,
				values,
			} => self.press_component(channel_id, message_id, custom_id, kind, values),
			Command::SubmitModal { answers } => self.submit_modal(answers),
			Command::CloseModal => self.close_modal(),
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
				burst,
			} => self.add_reaction(channel_id, message_id, emoji, burst),
			Command::RemoveReaction {
				channel_id,
				message_id,
				emoji,
				burst,
			} => self.remove_reaction(channel_id, message_id, emoji, burst),
			Command::RemoveUserReaction {
				channel_id,
				message_id,
				emoji,
				burst,
				user_id,
			} => self.remove_user_reaction(channel_id, message_id, emoji, burst, user_id),
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
			Command::SearchMore => self.search_more(),
			Command::CloseSearch => self.close_search(),
			Command::ToggleCategory {
				guild_id,
				category_id,
			} => self.toggle_category(guild_id, category_id),
			Command::SwitchAccount(user_id) => self.switch_account(user_id),
			Command::RefreshThemeLinks { urls } => self.refresh_theme_links(urls),
			Command::Reconnect => self.gateway.reconnect(),
			Command::ShowDeletedMessages(show) => self.apply_local(LocalEvent::ShowDeletedMessages(show)),
			Command::Logout => {
				self.begin_sign_out();
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
		let (has_messages, has_draft, forum) = {
			let state = self.state.read();
			(
				state
					.messages(channel_id)
					.is_some_and(|cache| !cache.ordered.is_empty()),
				state.draft(channel_id).is_some(),
				state.channel(channel_id).is_some_and(|channel| {
					matches!(channel.kind, ChannelType::GuildForum | ChannelType::GuildMedia)
				}),
			)
		};
		if forum {
			return;
		}
		if !has_messages {
			self.load_initial(channel_id);
		}
		if !has_draft {
			self.load_draft(channel_id);
		}
		self.on_channel_opened(channel_id);
		self.backfill_opened(channel_id);
	}

	fn load_initial(&self, channel_id: Snowflake) {
		let rest = self.rest.clone();
		let store = self.store.clone();
		let internal = self.internal.clone();
		let limits = *self.state.read().limits();
		self.runtime.spawn(async move {
			let (kept, cached) = tokio::task::spawn_blocking(move || {
				(
					persist::load_kept(&store, channel_id, &limits),
					persist::load_messages(&store, channel_id),
				)
			})
			.await
			.unwrap_or_default();
			if !kept.is_empty() {
				let _ = internal.send(Internal::Kept { channel_id, kept });
			}
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

	fn edit_message(&self, channel_id: Snowflake, message_id: Snowflake, content: String) {
		let hooks = self.hooks.clone();
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let original = content.clone();
			let rewritten = guarded_blocking(&hooks, "before_edit", move |hooks| {
				hooks.before_edit(channel_id, message_id, &original)
			})
			.await
			.flatten()
			.filter(|rewritten| rewritten.content != content);
			let owner = hooks.as_deref().map(|hooks| hooks.name().to_owned());
			let (content, altered_by) = match rewritten {
				Some(rewritten) => (rewritten.content, rewritten.by.or(owner)),
				None => (content, None),
			};
			let body = EditMessage {
				content: Some(content),
				..EditMessage::default()
			};
			match rest.edit_message(channel_id, message_id, body).await {
				Ok(_) if hooks.is_some() => {
					let _ = internal.send(Internal::Edited {
						channel_id,
						message_id,
						altered_by,
					});
				}
				Ok(_) => {}
				Err(error) => {
					tracing::warn!(%error, what = "edit message", "rest call failed");
					let _ = internal.send(Internal::Rejected(actions::failure_reason("Edit", &error)));
				}
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
			.filter(|channel_id| !state.is_hidden(*channel_id))
			.or_else(|| {
				state.guild(guild_id)?.channels.values().find_map(|channel| {
					(channel.kind == ChannelType::GuildText && !state.is_hidden(channel.id))
						.then_some(channel.id)
				})
			})
	}

	pub(crate) fn request_members(&mut self, guild_id: Snowflake, user_ids: Vec<Snowflake>) {
		self.member_requests.want_ids(guild_id, user_ids, Instant::now());
	}

	fn lazy_deadline(&self) -> Option<Instant> {
		[
			self.subscriptions.deadline(),
			self.member_requests.deadline(),
			self.backfill.deadline(),
			self.layout.deadline(),
			self.collapse.deadline(),
			self.guild_writes.deadline(),
			self.status.deadline(),
			self.status_deadline(),
			self.searcher.deadline(),
			self.ranker.deadline(),
			self.favorite_writes.deadline(),
		]
		.into_iter()
		.flatten()
		.min()
	}

	fn lazy_tick(&mut self) {
		let now = Instant::now();
		if self.subscriptions.deadline().is_some_and(|due| due <= now) {
			self.flush_subscriptions();
		}
		self.run_backfill(now);
		if self.layout.deadline().is_some_and(|due| due <= now) {
			self.flush_layout();
		}
		if self.collapse.deadline().is_some_and(|due| due <= now) {
			self.flush_collapse();
		}
		if self.guild_writes.deadline().is_some_and(|due| due <= now) {
			self.flush_guild_writes();
		}
		if self.status.deadline().is_some_and(|due| due <= now) {
			self.flush_status();
		}
		self.expire_custom_status();
		if self.searcher.deadline().is_some_and(|due| due <= now) {
			self.flush_search();
		}
		if self.ranker.deadline().is_some_and(|due| due <= now) {
			self.flush_emoji_rank();
		}
		if self.favorite_writes.deadline().is_some_and(|due| due <= now) {
			self.flush_favorites();
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
}
