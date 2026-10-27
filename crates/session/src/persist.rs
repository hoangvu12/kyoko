use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use chrono::{DateTime, Utc};
use protocol::{
	EntryContainer, LenientVec, Message, MessageUpdate, ReadStateContainer, ReadStateKind, Ready, Snowflake,
	UserGuildSettingsContainer,
};
use serde::de::DeserializeOwned;
use state::{CacheLimits, KeptWrite, MessageVersion};
use store::{DeletedRow, ReadState, Store, Table};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const QUEUE_CAPACITY: usize = 1024;
const BATCH_LIMIT: usize = 256;
const READY_KEY: &str = "session.ready";
const READ_STATE_VERSION_KEY: &str = "client_state.read_state_version";
const GUILD_SETTINGS_KEY: &str = "client_state.user_guild_settings";
pub(crate) const CACHED_MESSAGES: u32 = 50;
pub(crate) const RECENT_EMOJI_KEY: &str = "expressions.recent.emoji";
pub(crate) const RECENT_GIFS_KEY: &str = "expressions.recent.gifs";
pub(crate) const RECENT_STICKERS_KEY: &str = "expressions.recent.stickers";
pub(crate) const LAST_CHANNELS_KEY: &str = "navigation.last_channels";

pub(crate) enum Job {
	Ready(Box<Ready>),
	Messages {
		channel_id: Snowflake,
		messages: Vec<Arc<Message>>,
	},
	Kept(Vec<KeptWrite>),
	EditStored(Box<MessageUpdate>),
	ReadState {
		state: ReadState,
		version: Option<u64>,
	},
	ReadStates {
		states: Vec<ReadState>,
		version: Option<u64>,
	},
	GuildSettings(UserGuildSettingsContainer),
	ForgetVersions,
	Draft {
		channel_id: Snowflake,
		content: String,
	},
	Recent {
		key: &'static str,
		json: Vec<u8>,
	},
}

pub(crate) struct Persistor {
	queue: mpsc::Sender<Job>,
	task: JoinHandle<()>,
	lost: Arc<AtomicBool>,
}

impl Persistor {
	pub(crate) fn spawn(store: Arc<Store>, runtime: &Handle) -> Self {
		let (queue, jobs) = mpsc::channel(QUEUE_CAPACITY);
		let lost = Arc::new(AtomicBool::new(false));
		let task = runtime.spawn(write_loop(store, jobs, lost.clone()));
		Self { queue, task, lost }
	}

	pub(crate) fn submit(&self, job: Job) {
		if self.queue.try_send(job).is_err() {
			self.lost.store(true, Ordering::Relaxed);
			tracing::warn!("cache write queue is full, dropping a write");
		}
	}

	pub(crate) async fn close(self) {
		drop(self.queue);
		let _ = self.task.await;
	}
}

async fn write_loop(store: Arc<Store>, mut jobs: mpsc::Receiver<Job>, lost: Arc<AtomicBool>) {
	let mut batch = Vec::new();
	while jobs.recv_many(&mut batch, BATCH_LIMIT).await > 0 {
		let pending = mem::take(&mut batch);
		let store = store.clone();
		let lost = lost.clone();
		let written = tokio::task::spawn_blocking(move || {
			for job in pending {
				let trusted = !lost.load(Ordering::Relaxed);
				if let Err(error) = write(&store, job, trusted) {
					lost.store(true, Ordering::Relaxed);
					tracing::warn!(%error, "cache write failed");
				}
			}
		})
		.await;
		if written.is_err() {
			tracing::warn!("cache writer panicked");
		}
	}
}

fn write(store: &Store, job: Job, trusted: bool) -> Result<()> {
	match job {
		Job::Ready(ready) => write_ready(store, &ready),
		Job::Messages { channel_id, messages } => {
			let encoded = messages
				.iter()
				.map(|message| Ok((message.id.get(), serde_json::to_vec(&**message)?)))
				.collect::<Result<Vec<_>>>()?;
			let rows: Vec<(u64, &[u8])> = encoded.iter().map(|(id, json)| (*id, json.as_slice())).collect();
			Ok(store.db.put_messages_batch(channel_id.get(), &rows)?)
		}
		Job::Kept(writes) => write_kept(store, writes),
		Job::EditStored(update) => edit_stored(store, &update),
		Job::ReadState { state, version } => {
			store.db.set_read_state(&state)?;
			write_read_state_version(store, version.filter(|_| trusted))
		}
		Job::ReadStates { states, version } => {
			store.db.replace_read_states(&states)?;
			write_read_state_version(store, version.filter(|_| trusted))
		}
		Job::GuildSettings(mut container) => {
			if !trusted {
				container.version = None;
			}
			Ok(store
				.db
				.kv_set(GUILD_SETTINGS_KEY, &serde_json::to_vec(&container)?)?)
		}
		Job::ForgetVersions => forget_versions(store),
		Job::Recent { key, json } => Ok(store.db.kv_set(key, &json)?),
		Job::Draft { channel_id, content } => {
			if content.is_empty() {
				Ok(store.db.clear_draft(channel_id.get())?)
			} else {
				Ok(store.db.set_draft(channel_id.get(), &content)?)
			}
		}
	}
}

fn write_kept(store: &Store, writes: Vec<KeptWrite>) -> Result<()> {
	let mut deleted = Vec::new();
	for write in writes {
		match write {
			KeptWrite::Deleted {
				channel_id,
				message_id,
				message,
				at,
			} => {
				let json = message.map(|message| serde_json::to_vec(&*message)).transpose()?;
				deleted.push((channel_id, message_id, at, json));
			}
			KeptWrite::Forgotten { message_id } => store.db.forget_message(message_id.get())?,
			KeptWrite::Version {
				channel_id,
				message_id,
				version,
			} => store
				.db
				.keep_version(channel_id.get(), message_id.get(), &serde_json::to_vec(&version)?)?,
		}
	}
	if deleted.is_empty() {
		return Ok(());
	}
	let rows: Vec<DeletedRow<'_>> = deleted
		.iter()
		.map(|(channel_id, message_id, at, json)| DeletedRow {
			id: message_id.get(),
			channel_id: channel_id.get(),
			deleted_at: at.timestamp_millis(),
			json: json.as_deref(),
		})
		.collect();
	Ok(store.db.keep_deleted(&rows)?)
}

fn edit_stored(store: &Store, update: &MessageUpdate) -> Result<()> {
	let Some(json) = store.db.get_json(Table::Messages, update.id.get())? else {
		return Ok(());
	};
	let Ok(mut message) = serde_json::from_slice::<Message>(&json) else {
		return Ok(());
	};
	if let Some(version) = state::replaced_version(&message, update) {
		store.db.keep_version(
			update.channel_id.get(),
			update.id.get(),
			&serde_json::to_vec(&version)?,
		)?;
	}
	state::apply_update(&mut message, update);
	Ok(store.db.put_json(
		Table::Messages,
		update.id.get(),
		Some(update.channel_id.get()),
		&serde_json::to_vec(&message)?,
	)?)
}

fn write_read_state_version(store: &Store, version: Option<u64>) -> Result<()> {
	match version {
		Some(version) => store
			.db
			.kv_set(READ_STATE_VERSION_KEY, &serde_json::to_vec(&version)?)?,
		None => store.db.kv_delete(READ_STATE_VERSION_KEY)?,
	}
	Ok(())
}

fn forget_versions(store: &Store) -> Result<()> {
	store.db.kv_delete(READ_STATE_VERSION_KEY)?;
	if let Some(mut container) = load_guild_settings(store) {
		container.version = None;
		store
			.db
			.kv_set(GUILD_SETTINGS_KEY, &serde_json::to_vec(&container)?)?;
	}
	Ok(())
}

fn write_ready(store: &Store, ready: &Ready) -> Result<()> {
	let db = &store.db;
	for guild in ready.guilds.iter() {
		db.put_json(Table::Guilds, guild.id.get(), None, &serde_json::to_vec(guild)?)?;
		for channel in guild.channels.iter().chain(guild.threads.iter()) {
			db.put_json(
				Table::Channels,
				channel.id.get(),
				Some(guild.id.get()),
				&serde_json::to_vec(channel)?,
			)?;
		}
	}
	for channel in ready.private_channels.iter() {
		db.put_json(
			Table::Channels,
			channel.id.get(),
			None,
			&serde_json::to_vec(channel)?,
		)?;
	}
	for user in ready.users.iter() {
		db.put_json(Table::Users, user.id.get(), None, &serde_json::to_vec(user)?)?;
	}
	let snapshot = Ready {
		read_state: ReadStateContainer::default(),
		user_guild_settings: UserGuildSettingsContainer::default(),
		..ready.clone()
	};
	db.kv_set(READY_KEY, &serde_json::to_vec(&snapshot)?)?;
	Ok(())
}

pub(crate) fn stored_read_state(read_state: &protocol::ReadState) -> ReadState {
	ReadState {
		channel_id: read_state.id.get(),
		last_message_id: read_state.last_message_id.map(Snowflake::get),
		mention_count: read_state
			.mention_count
			.map_or(0, |count| u32::try_from(count).unwrap_or(0)),
		last_viewed: read_state.last_viewed,
	}
}

fn cached_read_state(stored: &ReadState) -> protocol::ReadState {
	protocol::ReadState {
		id: Snowflake(stored.channel_id),
		last_message_id: stored.last_message_id.map(Snowflake),
		mention_count: Some(i64::from(stored.mention_count)),
		last_viewed: stored.last_viewed,
		read_state_type: Some(ReadStateKind::Channel),
		..protocol::ReadState::default()
	}
}

pub(crate) fn load_cache(store: &Store) -> Option<Ready> {
	let mut ready = load_ready(store)?;
	let entries: Vec<protocol::ReadState> = load_read_states(store).iter().map(cached_read_state).collect();
	ready.read_state = EntryContainer {
		entries: LenientVec::from(entries),
		version: load_read_state_version(store),
		partial: false,
	};
	ready.user_guild_settings = load_guild_settings(store).unwrap_or_else(|| EntryContainer {
		version: None,
		..mem::take(&mut ready.user_guild_settings)
	});
	ready.session_id.clear();
	ready.resume_gateway_url = None;
	Some(ready)
}

fn load_read_state_version(store: &Store) -> Option<u64> {
	let bytes = store.db.kv_get(READ_STATE_VERSION_KEY).ok().flatten()?;
	serde_json::from_slice(&bytes).ok()
}

fn load_guild_settings(store: &Store) -> Option<UserGuildSettingsContainer> {
	let bytes = store.db.kv_get(GUILD_SETTINGS_KEY).ok().flatten()?;
	serde_json::from_slice(&bytes).ok()
}

fn load_ready(store: &Store) -> Option<Ready> {
	let bytes = store
		.db
		.kv_get(READY_KEY)
		.ok()
		.flatten()
		.filter(|bytes| !bytes.is_empty())?;
	match serde_json::from_slice::<Ready>(&bytes) {
		Ok(mut ready) => {
			ready.normalize_guilds();
			Some(ready)
		}
		Err(error) => {
			tracing::warn!(%error, "cached session snapshot is unreadable");
			None
		}
	}
}

fn load_read_states(store: &Store) -> Vec<ReadState> {
	store.db.all_read_state().unwrap_or_else(|error| {
		tracing::warn!(%error, "could not read cached read state");
		Vec::new()
	})
}

pub(crate) fn load_messages(store: &Store, channel_id: Snowflake) -> Vec<Message> {
	let rows = store
		.db
		.recent_messages(channel_id.get(), CACHED_MESSAGES)
		.unwrap_or_else(|error| {
			tracing::warn!(%error, "could not read cached messages");
			Vec::new()
		});
	rows.iter()
		.filter_map(|(_, json)| serde_json::from_slice(json).ok())
		.collect()
}

#[derive(Debug, Default)]
pub(crate) struct Kept {
	pub(crate) deleted: Vec<(Message, DateTime<Utc>)>,
	pub(crate) versions: Vec<(Snowflake, Vec<MessageVersion>)>,
}

impl Kept {
	pub(crate) fn is_empty(&self) -> bool {
		self.deleted.is_empty() && self.versions.is_empty()
	}
}

pub(crate) fn load_kept(store: &Store, channel_id: Snowflake, limits: &CacheLimits) -> Kept {
	let deleted = store
		.db
		.kept_deleted(channel_id.get(), cap(limits.kept_deleted))
		.unwrap_or_else(|error| {
			tracing::warn!(%error, "could not read kept deleted messages");
			Vec::new()
		})
		.into_iter()
		.filter_map(|row| {
			let message = serde_json::from_slice(&row.json).ok()?;
			Some((message, DateTime::from_timestamp_millis(row.deleted_at)?))
		})
		.collect();
	let rows = store
		.db
		.kept_versions(
			channel_id.get(),
			cap(limits.kept_edited.saturating_mul(limits.kept_versions)),
		)
		.unwrap_or_else(|error| {
			tracing::warn!(%error, "could not read kept message versions");
			Vec::new()
		});
	let mut versions: Vec<(Snowflake, Vec<MessageVersion>)> = Vec::new();
	for row in rows {
		let Ok(version) = serde_json::from_slice::<MessageVersion>(&row.json) else {
			continue;
		};
		let message_id = Snowflake(row.message_id);
		match versions.iter_mut().find(|(id, _)| *id == message_id) {
			Some((_, history)) => history.push(version),
			None => versions.push((message_id, vec![version])),
		}
	}
	Kept { deleted, versions }
}

fn cap(limit: usize) -> u32 {
	u32::try_from(limit).unwrap_or(u32::MAX)
}

pub(crate) fn load_draft(store: &Store, channel_id: Snowflake) -> Option<String> {
	store.db.get_draft(channel_id.get()).ok().flatten()
}

pub(crate) fn load_recent<T: DeserializeOwned>(store: &Store, key: &str) -> Vec<T> {
	store
		.db
		.kv_get(key)
		.ok()
		.flatten()
		.and_then(|bytes| serde_json::from_slice(&bytes).ok())
		.unwrap_or_default()
}

pub(crate) fn load_last_channels(store: &Store) -> Vec<(Snowflake, Snowflake)> {
	load_recent::<serde_json::Value>(store, LAST_CHANNELS_KEY)
		.into_iter()
		.filter_map(|pair| serde_json::from_value(pair).ok())
		.collect()
}
