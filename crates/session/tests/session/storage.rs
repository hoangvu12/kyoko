use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use media::DiskCache;
use mockcord::Reply;
use protocol::Snowflake;
use session::{Command, Retention};
use state::{ConnectionStatus, DisconnectReason, SecretStorage};
use store::{Keyring, MemoryKeyring, Paths, Store, Table};

use crate::harness::{Options, Running};

const MEDIA_FILE_BYTES: usize = 1000;
const CLIENT_STATE_KEYS: [&str; 3] = [
	"session.ready",
	"client_state.read_state_version",
	"client_state.user_guild_settings",
];
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

fn files_under(root: &Path) -> Vec<PathBuf> {
	let mut files = Vec::new();
	let mut pending = vec![root.to_path_buf()];
	while let Some(dir) = pending.pop() {
		let Ok(entries) = fs::read_dir(&dir) else {
			continue;
		};
		for entry in entries.flatten() {
			let path = entry.path();
			if path.is_dir() {
				pending.push(path);
			} else {
				files.push(path);
			}
		}
	}
	files
}

fn files_containing(root: &Path, secret: &str) -> Vec<PathBuf> {
	files_under(root)
		.into_iter()
		.filter(|path| {
			fs::read(path).is_ok_and(|bytes| {
				bytes
					.windows(secret.len())
					.any(|window| window == secret.as_bytes())
			})
		})
		.collect()
}

fn account_user(user_id: Snowflake) -> String {
	format!("account-{user_id}")
}

fn current_user_id(running: &Running) -> Snowflake {
	running
		.session()
		.state()
		.read()
		.current_user_id()
		.expect("a current user after READY")
}

fn cached_message_ids(store: &Store, channel_id: Snowflake) -> Vec<u64> {
	store
		.db
		.recent_messages(channel_id.get(), 1000)
		.expect("cached messages are readable")
		.into_iter()
		.map(|(id, _)| id)
		.collect()
}

fn kv_present(store: &Store, key: &str) -> bool {
	store.db.kv_get(key).ok().flatten().is_some()
}

fn secrets(running: &Running) -> SecretStorage {
	running.session().state().read().session().secrets.clone()
}

fn media_url(index: usize) -> String {
	format!("https://cdn.discordapp.com/attachments/1/{index}/image.png")
}

fn seed_media(paths: &Paths, ages: &[Duration]) {
	let disk = DiskCache::new(paths.cache_dir());
	for index in 0..ages.len() {
		disk.put(&media_url(index), &media_body(index))
			.expect("media is cached");
	}
	let now = SystemTime::now();
	for path in files_under(&paths.media_cache_dir()) {
		let index = media_index(&path).expect("seeded media is readable");
		fs::File::options()
			.write(true)
			.open(&path)
			.and_then(|file| file.set_modified(now - ages[index]))
			.expect("media access time is set");
	}
}

fn media_body(index: usize) -> Vec<u8> {
	vec![b'a' + index as u8; MEDIA_FILE_BYTES]
}

fn media_index(path: &Path) -> Option<usize> {
	let bytes = fs::read(path).ok()?;
	Some(usize::from(*bytes.first()? - b'a'))
}

fn cached_media(paths: &Paths) -> BTreeSet<usize> {
	files_under(&paths.media_cache_dir())
		.iter()
		.filter_map(|path| media_index(path))
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn by_default_the_token_lives_only_in_the_keyring() {
	let running = Running::start("storage-keyring").await;
	running.connected().await;
	let user_id = current_user_id(&running);
	let token = running.token().to_owned();
	running
		.eventually("the account token to reach the keyring", || {
			running
				.keyring()
				.secret(&account_user(user_id))
				.is_some_and(|stored| stored.as_str() == token)
		})
		.await;
	let store = running.store();
	assert_eq!(
		store.accounts.token_for(user_id).ok().flatten(),
		Some(token.clone())
	);
	assert!(running.keyring().secret("cache-key").is_some());
	assert!(!store.paths.token_path().exists());
	assert!(!store.paths.cache_key_path().exists());
	assert_eq!(files_containing(running.scratch(), &token), Vec::<PathBuf>::new());
	assert_eq!(secrets(&running), SecretStorage::default());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_keyring_error_surfaces_and_never_writes_a_plaintext_file() {
	let keyring = Arc::new(MemoryKeyring::new());
	keyring
		.set("cache-key", &"07".repeat(32))
		.expect("the cache key is seeded");
	keyring.set_failing_writes(true);
	let running = Running::start_with(Options::new("storage-keyring-error").keyring(keyring)).await;
	running.connected().await;
	running
		.until("the keyring error to be published", |state| {
			state.session().secrets.keyring_error.is_some()
		})
		.await;
	let channel_id = running.open_text_channel().await;
	running
		.send_and_wait(channel_id, "still signed in", "still signed in")
		.await;
	let token = running.token().to_owned();
	let user_id = current_user_id(&running);
	let state = secrets(&running);
	assert!(!state.plaintext);
	assert!(!state.cache_in_memory);
	assert_eq!(running.keyring().users(), vec!["cache-key".to_owned()]);
	assert!(
		running
			.store()
			.accounts
			.token_for(user_id)
			.ok()
			.flatten()
			.is_none()
	);
	assert!(!running.store().paths.token_path().exists());
	assert_eq!(files_containing(running.scratch(), &token), Vec::<PathBuf>::new());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreachable_keyring_keeps_the_cache_in_memory() {
	let keyring = Arc::new(MemoryKeyring::new());
	keyring.set_failing(true);
	let running = Running::start_with(Options::new("storage-keyring-down").keyring(keyring)).await;
	let channel_id = running.open_text_channel().await;
	let state = secrets(&running);
	assert!(state.keyring_error.is_some());
	assert!(state.cache_in_memory);
	assert!(!state.plaintext);
	running
		.eventually("the history to reach the in-memory cache", || {
			!cached_message_ids(running.store(), channel_id).is_empty()
		})
		.await;
	let paths = &running.store().paths;
	assert!(!paths.database_path().exists());
	assert!(!paths.cache_key_path().exists());
	assert!(!paths.token_path().exists());
	assert_eq!(
		files_containing(running.scratch(), running.token()),
		Vec::<PathBuf>::new()
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_plaintext_opt_in_writes_a_file_and_warns() {
	let running = Running::start_with(Options::new("storage-plaintext").plaintext_secrets(true)).await;
	running.connected().await;
	let user_id = current_user_id(&running);
	let token = running.token().to_owned();
	running
		.eventually("the account token to reach a file", || {
			running
				.store()
				.accounts
				.token_for(user_id)
				.ok()
				.flatten()
				.as_deref() == Some(token.as_str())
		})
		.await;
	assert!(secrets(&running).plaintext);
	assert!(running.keyring().users().is_empty());
	assert!(running.store().paths.cache_key_path().exists());
	assert!(!files_containing(&running.store().paths.config_dir().join("account-tokens"), &token).is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turning_the_opt_in_on_and_off_moves_the_secrets() {
	let running = Running::start("storage-plaintext-toggle").await;
	running.connected().await;
	let user_id = current_user_id(&running);
	let token = running.token().to_owned();
	let account = account_user(user_id);
	running
		.eventually("the account token to reach the keyring", || {
			running.keyring().secret(&account).is_some()
		})
		.await;
	let store = running.store();
	store
		.update_settings(|settings| settings.storage.plaintext_token = true)
		.expect("secrets move to files");
	assert!(running.keyring().users().is_empty());
	assert!(store.paths.cache_key_path().exists());
	assert_eq!(
		store.accounts.token_for(user_id).ok().flatten(),
		Some(token.clone())
	);
	assert!(store.secret_health().plaintext);

	store
		.update_settings(|settings| settings.storage.plaintext_token = false)
		.expect("secrets move back to the keyring");
	assert_eq!(running.keyring().users(), vec![account, "cache-key".to_owned()]);
	assert!(!store.paths.cache_key_path().exists());
	assert_eq!(files_containing(running.scratch(), &token), Vec::<PathBuf>::new());
	assert!(!store.secret_health().plaintext);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cached_messages_are_trimmed_per_channel() {
	let retention = Retention {
		messages_per_channel: 3,
		interval: Duration::from_millis(50),
		..Retention::default()
	};
	let running = Running::start_with(Options::new("storage-prune-count").retention(retention)).await;
	let channel_id = running.open_text_channel().await;
	let newest: Vec<u64> = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.expect("the channel is cached")
		.ordered
		.iter()
		.rev()
		.take(3)
		.map(|message| message.id.get())
		.collect();
	running
		.eventually("the cached channel to shrink to three messages", || {
			cached_message_ids(running.store(), channel_id) == newest
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_cached_messages_are_dropped_at_startup() {
	let mut running = Running::start("storage-prune-age").await;
	let channel_id = running.open_text_channel().await;
	running
		.eventually("the history to reach the store", || {
			!cached_message_ids(running.store(), channel_id).is_empty()
		})
		.await;
	running
		.restart_with_retention(Retention {
			message_max_age: Duration::from_millis(1),
			..Retention::default()
		})
		.await;
	running
		.eventually("stale messages to be pruned", || {
			cached_message_ids(running.store(), channel_id).is_empty()
		})
		.await;
	running.connected().await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_media_cache_is_pruned_by_age_and_budget() {
	let minute = Duration::from_secs(60);
	let ages = [3 * DAY, minute, 40 * minute, 30 * minute, 10 * minute];
	let retention = Retention {
		media_bytes: 2 * MEDIA_FILE_BYTES as u64 + MEDIA_FILE_BYTES as u64 / 2,
		media_max_age: DAY,
		..Retention::default()
	};
	let running = Running::start_with(
		Options::new("storage-prune-media")
			.retention(retention)
			.seed(move |paths| seed_media(paths, &ages)),
	)
	.await;
	let paths = running.store().paths.clone();
	running
		.eventually("the media cache to be pruned", || {
			cached_media(&paths) == BTreeSet::from([1, 4])
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signing_out_wipes_the_token_account_messages_and_media() {
	let running = Running::start_with(
		Options::new("storage-sign-out").seed(|paths| seed_media(paths, &[Duration::ZERO])),
	)
	.await;
	let channel_id = running.open_text_channel().await;
	let user_id = current_user_id(&running);
	let guild_id = running
		.session()
		.state()
		.read()
		.guilds()
		.next()
		.map(|guild| guild.id)
		.expect("a guild");
	let store = running.store();
	running
		.eventually("the account and history to reach the store", || {
			running.keyring().secret(&account_user(user_id)).is_some()
				&& !cached_message_ids(store, channel_id).is_empty()
				&& store
					.db
					.get_json(Table::Guilds, guild_id.get())
					.ok()
					.flatten()
					.is_some() && CLIENT_STATE_KEYS.iter().all(|key| kv_present(store, key))
		})
		.await;
	assert!(!cached_media(&store.paths).is_empty());
	let installation_id = store.installation_id();

	running.session().send(Command::Logout);
	running
		.eventually("sign-out to wipe the account's data", || {
			store.accounts.list().is_empty()
				&& cached_message_ids(store, channel_id).is_empty()
				&& !store.paths.media_cache_dir().exists()
				&& store
					.db
					.get_json(Table::Guilds, guild_id.get())
					.ok()
					.flatten()
					.is_none() && CLIENT_STATE_KEYS.iter().all(|key| !kv_present(store, key))
		})
		.await;
	assert!(store.tokens.load().ok().flatten().is_none());
	assert!(store.accounts.token_for(user_id).ok().flatten().is_none());
	assert_eq!(running.keyring().users(), vec!["cache-key".to_owned()]);
	assert!(
		store
			.db
			.all_read_state()
			.expect("read state is readable")
			.is_empty()
	);
	assert_eq!(store.installation_id(), installation_id);
	assert!(store.paths.settings_path().exists());
	assert_eq!(
		files_containing(running.scratch(), running.token()),
		Vec::<PathBuf>::new()
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_token_is_forgotten_but_the_cache_stays() {
	let running = Running::start("storage-rejected").await;
	let channel_id = running.open_text_channel().await;
	let user_id = current_user_id(&running);
	let account = account_user(user_id);
	running
		.eventually("the account and history to reach the store", || {
			running.keyring().secret(&account).is_some()
				&& !cached_message_ids(running.store(), channel_id).is_empty()
		})
		.await;
	running.mock().respond_once(
		"POST",
		&format!("/channels/{channel_id}/messages"),
		Reply::unauthorized(),
	);
	running.session().send(Command::SendMessage {
		channel_id,
		content: "rejected".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: Vec::new(),
	});
	running
		.until("the signed-out status", |state| {
			state.session().connection
				== ConnectionStatus::Disconnected(DisconnectReason::AuthenticationFailed)
		})
		.await;
	running
		.eventually("the rejected token to be forgotten", || {
			running.keyring().secret(&account).is_none()
		})
		.await;
	let store = running.store();
	assert!(store.accounts.token_for(user_id).ok().flatten().is_none());
	assert!(store.accounts.list().iter().any(|known| known.user_id == user_id));
	assert!(!cached_message_ids(store, channel_id).is_empty());
	running.stop().await;
}
