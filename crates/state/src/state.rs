use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use indexmap::IndexMap;
use parking_lot::RwLock;
use protocol::{
	ApplicationCommandIndex, Channel, Message, MessageType, Presence, ReadState, Relationship, Snowflake,
	User, UserGuildSettings,
};
use serde_json::Value;
use smallvec::SmallVec;

use crate::caps::{CacheLimits, Recent};
use crate::changes::Changes;
use crate::deletions::DeleteBatch;
use crate::event::{Event, LoadDirection, LocalEvent};
use crate::guild::GuildState;
use crate::kept::KeptWrite;
use crate::loaded::Lazy;
use crate::locks::RiskyLocks;
use crate::messages::ChannelMessages;
use crate::references::StoredReference;
use crate::settings::GuildFolder;
use crate::versions::CacheVersions;
use crate::views::RenderedMessage;
use crate::views::UserOverride;
use crate::voice::{Call, MicTest, StreamSummary, VoiceLevels, VoiceSession, VoiceStats};

pub const TYPING_TIMEOUT: Duration = Duration::from_secs(10);

pub type SharedState = Arc<RwLock<State>>;

#[derive(Debug, Clone, Default)]
pub struct SessionInfo {
	pub session_id: String,
	pub resume_url: Option<String>,
	pub connected: bool,
	pub connection: ConnectionStatus,
	pub latency: Option<Duration>,
	pub secrets: SecretStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SecretStorage {
	pub plaintext: bool,
	pub keyring_error: Option<String>,
	pub cache_in_memory: bool,
}

impl SessionInfo {
	pub(crate) fn set_connection(&mut self, status: ConnectionStatus) {
		self.connected = status == ConnectionStatus::Connected;
		self.connection = status;
	}
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConnectionStatus {
	#[default]
	Connecting,
	Resuming,
	Connected,
	Reconnecting {
		attempt: u32,
		retry_in: Duration,
	},
	Disconnected(DisconnectReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectReason {
	AuthenticationFailed,
	Rejected { code: Option<u16>, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
	pub guild_id: Option<Snowflake>,
	pub channel_id: Snowflake,
}

#[derive(Debug, Default)]
pub struct State {
	pub(crate) current_user: Option<User>,
	pub(crate) guilds: IndexMap<Snowflake, GuildState>,
	pub(crate) private_channels: IndexMap<Snowflake, Channel>,
	pub(crate) channel_guilds: HashMap<Snowflake, Option<Snowflake>>,
	pub(crate) users: Recent<Snowflake, Arc<User>>,
	pub(crate) presences: HashMap<Snowflake, Presence>,
	pub(crate) relationships: IndexMap<Snowflake, Relationship>,
	pub(crate) read_states: HashMap<Snowflake, ReadState>,
	pub(crate) guild_settings: HashMap<Snowflake, UserGuildSettings>,
	pub(crate) global_settings: Option<UserGuildSettings>,
	pub(crate) messages: HashMap<Snowflake, ChannelMessages>,
	pub(crate) typing: HashMap<Snowflake, HashMap<Snowflake, Instant>>,
	pub(crate) session: SessionInfo,
	pub(crate) guild_folders: Vec<GuildFolder>,
	pub(crate) guild_positions: Vec<Snowflake>,
	pub(crate) user_settings: Value,
	pub(crate) status_settings: protocol::ProtoStatusSettings,
	pub(crate) collapsed_categories: HashSet<Snowflake>,
	pub(crate) drafts: HashMap<Snowflake, String>,
	pub(crate) selected: Option<Selection>,
	pub(crate) voice: Option<VoiceSession>,
	pub(crate) mic_test: Option<MicTest>,
	pub(crate) account_switch: Option<Snowflake>,
	pub(crate) calls: HashMap<Snowflake, Call>,
	pub(crate) altered: HashMap<Snowflake, String>,
	pub(crate) sent_by_plugin: HashMap<Snowflake, String>,
	pub(crate) user_overrides: HashMap<(Option<Snowflake>, Snowflake), Option<UserOverride>>,
	pub(crate) unrendered: Vec<Arc<Message>>,
	pub(crate) lazy: Lazy,
	pub(crate) streams: HashMap<String, StreamSummary>,
	pub(crate) rejection: Option<Rejection>,
	pub(crate) rejection_count: u64,
	pub(crate) deletions: VecDeque<DeleteBatch>,
	pub(crate) limits: CacheLimits,
	pub(crate) warm: Vec<Snowflake>,
	pub(crate) risky: RiskyLocks,
	pub(crate) versions: CacheVersions,
	pub(crate) ack_versions: HashMap<Snowflake, u64>,
	pub(crate) referenced: Recent<Snowflake, StoredReference>,
	pub(crate) show_deleted: bool,
	pub(crate) kept_writes: Vec<KeptWrite>,
	pub(crate) commands: HashMap<Snowflake, Arc<ApplicationCommandIndex>>,
	pub(crate) autocomplete: Option<crate::composer::Autocomplete>,
	pub(crate) app_modal: Option<crate::modal::AppModal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
	pub id: u64,
	pub reason: String,
}

pub(crate) fn intern_user(users: &mut Recent<Snowflake, Arc<User>>, user: &User) {
	if user.username.is_empty() {
		return;
	}
	if users.get(&user.id).is_some_and(|existing| **existing == *user) {
		users.touch(&user.id);
		return;
	}
	users.insert(user.id, Arc::new(user.clone()));
}

impl State {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_limits(limits: CacheLimits) -> Self {
		Self {
			limits,
			..Self::default()
		}
	}

	pub fn shared() -> SharedState {
		Self::shared_with(CacheLimits::default())
	}

	pub fn shared_with(limits: CacheLimits) -> SharedState {
		Arc::new(RwLock::new(Self::with_limits(limits)))
	}

	pub fn limits(&self) -> &CacheLimits {
		&self.limits
	}

	pub fn apply(&mut self, event: &Event<'_>) -> Changes {
		let mut changes = self.apply_event(event);
		self.enforce_caps(&mut changes);
		changes
	}

	fn apply_event(&mut self, event: &Event<'_>) -> Changes {
		match event {
			Event::Ready(ready) => self.apply_ready(ready),
			Event::ReadySupplemental(supplemental) => self.apply_supplemental(supplemental),
			Event::Dispatch(dispatch) => self.apply_dispatch(dispatch),
			Event::MessagesLoaded {
				channel_id,
				messages,
				direction,
				reached_start,
			} => self.apply_messages_loaded(*channel_id, messages, *direction, *reached_start),
			Event::Local(local) => self.apply_local(local),
		}
	}

	pub fn expire(&mut self, now: Instant) -> Changes {
		let mut changes = Changes::default();
		self.typing.retain(|&channel_id, users| {
			let before = users.len();
			users.retain(|_, started| now.saturating_duration_since(*started) < TYPING_TIMEOUT);
			if users.len() != before {
				changes.typing.insert(channel_id);
			}
			!users.is_empty()
		});
		changes
	}

	pub fn current_user(&self) -> Option<&User> {
		self.current_user.as_ref()
	}

	pub fn current_user_id(&self) -> Option<Snowflake> {
		self.current_user.as_ref().map(|user| user.id)
	}

	pub fn guild(&self, id: Snowflake) -> Option<&GuildState> {
		self.guilds.get(&id)
	}

	pub fn guilds(&self) -> impl Iterator<Item = &GuildState> {
		self.guilds.values()
	}

	pub fn channel(&self, id: Snowflake) -> Option<&Channel> {
		match self.channel_guilds.get(&id)? {
			Some(guild_id) => self.guilds.get(guild_id)?.channel(id),
			None => self.private_channels.get(&id),
		}
	}

	pub fn channel_guild(&self, channel_id: Snowflake) -> Option<Snowflake> {
		self.channel_guilds.get(&channel_id).copied().flatten()
	}

	pub fn private_channels(&self) -> impl Iterator<Item = &Channel> {
		self.private_channels.values()
	}

	pub fn user(&self, id: Snowflake) -> Option<&Arc<User>> {
		self.users.get(&id)
	}

	pub fn presence(&self, user_id: Snowflake) -> Option<&Presence> {
		self.presences.get(&user_id)
	}

	pub fn relationships(&self) -> impl Iterator<Item = &Relationship> {
		self.relationships.values()
	}

	pub fn read_state(&self, channel_id: Snowflake) -> Option<&ReadState> {
		self.read_states.get(&channel_id)
	}

	pub fn guild_settings(&self, guild_id: Option<Snowflake>) -> Option<&UserGuildSettings> {
		match guild_id {
			Some(id) => self.guild_settings.get(&id),
			None => self.global_settings.as_ref(),
		}
	}

	pub fn messages(&self, channel_id: Snowflake) -> Option<&ChannelMessages> {
		self.messages.get(&channel_id)
	}

	pub fn altered_by(&self, message_id: Snowflake) -> Option<&str> {
		self.altered.get(&message_id).map(String::as_str)
	}

	pub fn sent_by_plugin(&self, message_id: Snowflake) -> Option<&str> {
		self.sent_by_plugin.get(&message_id).map(String::as_str)
	}

	pub fn user_override(&self, guild_id: Option<Snowflake>, user_id: Snowflake) -> Option<&UserOverride> {
		self.user_overrides.get(&(guild_id, user_id))?.as_ref()
	}

	pub fn has_override_entry(&self, guild_id: Option<Snowflake>, user_id: Snowflake) -> bool {
		self.user_overrides.contains_key(&(guild_id, user_id))
	}

	pub fn take_unrendered(&mut self) -> Vec<Arc<Message>> {
		std::mem::take(&mut self.unrendered)
	}

	pub fn stale_rendered(&self, channel_id: Snowflake, revision: u64) -> Vec<Arc<Message>> {
		let Some(cache) = self.messages.get(&channel_id) else {
			return Vec::new();
		};
		cache
			.ordered
			.iter()
			.filter(|message| {
				cache
					.rendered
					.get(&message.id)
					.is_none_or(|rendered| rendered.revision < revision)
			})
			.cloned()
			.collect()
	}

	pub fn last_rejection(&self) -> Option<&Rejection> {
		self.rejection.as_ref()
	}

	pub fn selected(&self) -> Option<Selection> {
		self.selected
	}

	pub fn account_switch_requested(&self) -> Option<Snowflake> {
		self.account_switch
	}

	pub fn session(&self) -> &SessionInfo {
		&self.session
	}

	pub fn draft(&self, channel_id: Snowflake) -> Option<&str> {
		self.drafts.get(&channel_id).map(String::as_str)
	}

	pub fn guild_folders(&self) -> &[GuildFolder] {
		&self.guild_folders
	}

	pub fn is_collapsed(&self, category_id: Snowflake) -> bool {
		self.collapsed_categories.contains(&category_id)
	}

	pub fn typing_users(&self, channel_id: Snowflake, now: Instant) -> SmallVec<[Snowflake; 4]> {
		let me = self.current_user_id();
		self.typing
			.get(&channel_id)
			.into_iter()
			.flatten()
			.filter(|(user_id, started)| {
				Some(**user_id) != me && now.saturating_duration_since(**started) < TYPING_TIMEOUT
			})
			.map(|(user_id, _)| *user_id)
			.collect()
	}

	pub(crate) fn channel_mut(&mut self, id: Snowflake) -> Option<&mut Channel> {
		match self.channel_guilds.get(&id)? {
			Some(guild_id) => self.guilds.get_mut(guild_id)?.channel_mut(id),
			None => self.private_channels.get_mut(&id),
		}
	}

	pub(crate) fn mark_channel_dirty(&self, channel_id: Snowflake, changes: &mut Changes) {
		match self.channel_guilds.get(&channel_id) {
			Some(Some(guild_id)) => {
				changes.channel_lists.insert(*guild_id);
				changes.guild_list = true;
			}
			Some(None) => changes.private_channels = true,
			None => {}
		}
	}

	fn apply_messages_loaded(
		&mut self,
		channel_id: Snowflake,
		messages: &[Message],
		direction: LoadDirection,
		reached_start: bool,
	) -> Changes {
		let guild_id = self.channel_guild(channel_id);
		let mut loaded = Vec::with_capacity(messages.len());
		for message in messages {
			self.ingest_message_people(guild_id, message);
			loaded.push(Arc::new(message.clone()));
		}
		self.unrendered.extend(loaded.iter().cloned());
		let channel_last = self
			.channel(channel_id)
			.and_then(|channel| channel.last_message_id);
		let open = self.is_open(channel_id);
		let mut changes = Changes::default();
		self.warm_up(channel_id, &mut changes);
		let limits = self.limits;
		let spliced = self.messages.entry(channel_id).or_default().merge_loaded(
			loaded,
			direction,
			reached_start,
			channel_last,
			open,
			&limits,
		);
		self.unrendered.extend(spliced);
		changes.messages.insert(channel_id);
		changes
	}

	fn apply_local(&mut self, event: &LocalEvent) -> Changes {
		let mut changes = Changes::default();
		match event {
			LocalEvent::PendingMessage {
				channel_id,
				nonce,
				content,
				stickers,
				poll,
				author,
				by_plugin,
				files,
			} => {
				let rendered = Arc::new(RenderedMessage::plain(content, false, 0));
				let message = Message {
					channel_id: *channel_id,
					guild_id: self.channel_guild(*channel_id),
					author: (**author).clone(),
					content: content.clone(),
					sticker_items: stickers.clone().into(),
					poll: poll.as_deref().cloned(),
					timestamp: Some(Utc::now()),
					nonce: Some(nonce.clone()),
					..Message::default()
				};
				self.warm_up(*channel_id, &mut changes);
				self.messages.entry(*channel_id).or_default().add_pending(
					Arc::new(message),
					by_plugin.clone(),
					rendered,
					files.clone(),
				);
				changes.messages.insert(*channel_id);
			}
			LocalEvent::UploadProgress { nonce, sent } => {
				for (&channel_id, cache) in &mut self.messages {
					if cache.upload_progressed(nonce, *sent) {
						changes.messages.insert(channel_id);
					}
				}
			}
			LocalEvent::PendingFailed { nonce, reason } => {
				for (&channel_id, cache) in &mut self.messages {
					if cache.fail_pending(nonce, reason.clone()) {
						changes.messages.insert(channel_id);
					}
				}
			}
			LocalEvent::PendingAltered { nonce, content, by } => {
				for (&channel_id, cache) in &mut self.messages {
					if cache.alter_pending(nonce, content.clone(), by.clone()) {
						changes.messages.insert(channel_id);
					}
				}
			}
			LocalEvent::EditAltered { channel_id, id, by } => {
				let changed = match by {
					Some(by) => self.altered.insert(*id, by.clone()).as_ref() != Some(by),
					None => self.altered.remove(id).is_some(),
				};
				if changed {
					changes.messages.insert(*channel_id);
				}
			}
			LocalEvent::MessageRendered {
				channel_id,
				id,
				source,
				rendered,
			} => {
				if let Some(cache) = self.messages.get_mut(channel_id)
					&& cache
						.by_id
						.get(id)
						.is_some_and(|current| current.content == source.content)
				{
					cache.rendered.insert(*id, Arc::clone(rendered));
					changes.messages.insert(*channel_id);
				}
			}
			LocalEvent::UserOverrides { entries } => {
				for (guild_id, user_id, over) in entries {
					self.user_overrides.insert((*guild_id, *user_id), over.clone());
				}
				changes.messages.mark_all();
				changes.members.mark_all();
			}
			LocalEvent::UserOverridesCleared => {
				if !self.user_overrides.is_empty() {
					self.user_overrides.clear();
					changes.messages.mark_all();
					changes.members.mark_all();
				}
			}
			LocalEvent::PendingCommand {
				channel_id,
				nonce,
				name,
				application,
				user,
			} => {
				let message = Message {
					channel_id: *channel_id,
					guild_id: self.channel_guild(*channel_id),
					author: (**application).clone(),
					kind: MessageType::ChatInputCommand,
					timestamp: Some(Utc::now()),
					nonce: Some(nonce.clone()),
					interaction_metadata: serde_json::json!({ "name": name, "user": user }),
					..Message::default()
				};
				self.warm_up(*channel_id, &mut changes);
				self.messages
					.entry(*channel_id)
					.or_default()
					.add_pending_command(Arc::new(message), Arc::new(RenderedMessage::plain("", false, 0)));
				changes.messages.insert(*channel_id);
			}
			LocalEvent::CommandAdvanced { nonce, stage } => {
				for (&channel_id, cache) in &mut self.messages {
					if cache.advance_command(nonce, stage.clone()) {
						changes.messages.insert(channel_id);
					}
				}
			}
			LocalEvent::EphemeralDismissed { channel_id, id } => {
				if let Some(cache) = self.messages.get_mut(channel_id)
					&& cache.dismiss_ephemeral(*id)
				{
					changes.messages.insert(*channel_id);
				}
			}
			LocalEvent::ComponentInteraction {
				channel_id,
				id,
				failed,
			} => {
				if let Some(cache) = self.messages.get_mut(channel_id)
					&& cache.component_failed(*id, *failed)
				{
					changes.messages.insert(*channel_id);
				}
			}
			LocalEvent::ComponentWaiting {
				channel_id,
				id,
				custom_id,
			} => {
				if let Some(cache) = self.messages.get_mut(channel_id)
					&& cache.component_waiting(*id, custom_id.clone())
				{
					changes.messages.insert(*channel_id);
				}
			}
			LocalEvent::InteractionExpired { channel_id, id } => {
				if let Some(cache) = self.messages.get_mut(channel_id)
					&& cache.expire(*id)
				{
					changes.messages.insert(*channel_id);
				}
			}
			LocalEvent::PendingDiscarded { nonce } => {
				for (&channel_id, cache) in &mut self.messages {
					if cache.remove_pending(nonce) {
						changes.messages.insert(channel_id);
					}
				}
			}
			LocalEvent::DraftChanged { channel_id, content } => {
				if content.is_empty() {
					self.drafts.remove(channel_id);
				} else {
					self.drafts.insert(*channel_id, content.clone());
				}
				changes.drafts.insert(*channel_id);
			}
			LocalEvent::ChannelSelected { guild_id, channel_id } => {
				let previous = self.selected.map(|selected| selected.channel_id);
				self.selected = Some(Selection {
					guild_id: *guild_id,
					channel_id: *channel_id,
				});
				changes.selection = true;
				if let Some(previous) = previous.filter(|previous| previous != channel_id) {
					self.leave_channel(previous, &mut changes);
				}
				self.warm_up(*channel_id, &mut changes);
			}
			LocalEvent::ViewportChanged { channel_id, viewport } => {
				self.viewport_changed(*channel_id, *viewport, &mut changes)
			}
			LocalEvent::CategoryCollapsed {
				channel_id,
				collapsed,
			} => {
				let changed = if *collapsed {
					self.collapsed_categories.insert(*channel_id)
				} else {
					self.collapsed_categories.remove(channel_id)
				};
				if changed {
					self.mark_channel_dirty(*channel_id, &mut changes);
				}
			}
			LocalEvent::Connection(status) => {
				self.session.set_connection(status.clone());
				changes.session = true;
			}
			LocalEvent::LatencyMeasured(latency) => {
				self.session.latency = Some(*latency);
				changes.session = true;
			}
			LocalEvent::SecretStorage(secrets) => {
				changes.session = self.session.secrets != *secrets;
				self.session.secrets = secrets.clone();
			}
			LocalEvent::VoiceConnection {
				status,
				channel_id,
				guild_id,
			} => return self.voice_connection(status, *channel_id, *guild_id),
			LocalEvent::VoiceSelf { self_mute, self_deaf } => return self.voice_self(*self_mute, *self_deaf),
			LocalEvent::Speaking { user_id, speaking } => return self.voice_speaking(*user_id, *speaking),
			LocalEvent::VoiceStats {
				ping_ms,
				loss,
				jitter,
				e2ee,
				video_bitrate_bps,
			} => {
				return self.voice_stats(VoiceStats {
					ping_ms: *ping_ms,
					loss: *loss,
					jitter_ms: *jitter,
					e2ee: *e2ee,
					video_bitrate_bps: *video_bitrate_bps,
				});
			}
			LocalEvent::MicTestStarted { seconds } => {
				return self.mic_test_changed(Some(MicTest {
					level: 0.0,
					remaining_secs: *seconds,
				}));
			}
			LocalEvent::MicTestLevel {
				level,
				remaining_secs,
			} => {
				return self.mic_test_changed(Some(MicTest {
					level: *level,
					remaining_secs: *remaining_secs,
				}));
			}
			LocalEvent::MicTestFinished => return self.mic_test_changed(None),
			LocalEvent::AccountSwitchRequested(user_id) => {
				self.account_switch = Some(*user_id);
				changes.session = true;
			}
			LocalEvent::VoiceLevels { input, output } => {
				return self.voice_levels(VoiceLevels {
					input: *input,
					output: *output,
				});
			}
			LocalEvent::CameraChanged(on) => return self.camera_changed(*on),
			LocalEvent::ScreenShareChanged(info) => return self.screen_share_changed(info.clone()),
			LocalEvent::Watching(stream_key) => return self.watching_changed(stream_key.clone()),
			LocalEvent::RemoteVideo {
				user_id,
				kind,
				active,
			} => {
				return self.remote_video_changed(*user_id, *kind, *active);
			}
			LocalEvent::Loaded(loaded) => return self.apply_loaded(loaded),
			LocalEvent::RecentMentionRemoved { message_id } => {
				return self.remove_recent_mention(*message_id);
			}
			LocalEvent::CommandRejected { reason } => {
				self.rejection_count += 1;
				self.rejection = Some(Rejection {
					id: self.rejection_count,
					reason: reason.clone(),
				});
				changes.rejected = true;
			}
			LocalEvent::BanRevoked { guild_id, user_id } => return self.ban_revoked(*guild_id, *user_id),
			LocalEvent::DeletesQueued(batch) => self.queue_deletes(batch, &mut changes),
			LocalEvent::DeleteProgressed {
				batch,
				message_id,
				status,
			} => self.delete_progressed(*batch, *message_id, status, &mut changes),
			LocalEvent::RiskyConfirmed { target, reason } => {
				changes.locks = self.confirm_risky(*target, *reason);
			}
			LocalEvent::ReferenceFetched {
				channel_id,
				message_id,
				message,
			} => self.reference_fetched(*channel_id, *message_id, message.as_deref(), &mut changes),
			LocalEvent::UserFetched { channel_id, user } => {
				self.user_fetched(*channel_id, user, &mut changes)
			}
			LocalEvent::KeptRestored {
				channel_id,
				deleted,
				versions,
			} => self.restore_kept(*channel_id, deleted, versions, &mut changes),
			LocalEvent::ShowDeletedMessages(show) => self.show_deleted_changed(*show, &mut changes),
			LocalEvent::MediaRefreshed { channel_id, message } => {
				self.media_refreshed(*channel_id, message, &mut changes)
			}
			LocalEvent::CommandIndexLoaded { scope, index } => {
				self.command_index_loaded(*scope, index, &mut changes)
			}
			LocalEvent::StatusSettings(settings) => changes.merge(self.set_status_settings(settings.clone())),
			LocalEvent::Autocomplete(autocomplete) => self.autocomplete_changed(autocomplete, &mut changes),
			LocalEvent::ModalOpened(modal) => self.modal_opened(modal, &mut changes),
			LocalEvent::ModalChanged { status, missing } => self.modal_changed(status, missing, &mut changes),
			LocalEvent::ModalClosed => self.modal_closed(&mut changes),
		}
		changes
	}
}
