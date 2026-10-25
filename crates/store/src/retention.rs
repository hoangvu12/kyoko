use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::database::{KeptLimits, MessageLimits};
use crate::error::Result;
use crate::settings::StorageSettings;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);
const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
	pub messages_per_channel: u32,
	pub messages_total: u32,
	pub message_max_age: Duration,
	pub media_bytes: u64,
	pub media_max_age: Duration,
	pub kept_deleted_per_channel: u32,
	pub kept_versions_per_message: u32,
	pub kept_versions_per_channel: u32,
	pub kept_max_age: Duration,
	pub interval: Duration,
}

impl Default for Retention {
	fn default() -> Self {
		Self {
			messages_per_channel: 500,
			messages_total: 50_000,
			message_max_age: 30 * DAY,
			media_bytes: 512 * MIB,
			media_max_age: 30 * DAY,
			kept_deleted_per_channel: 100,
			kept_versions_per_message: 10,
			kept_versions_per_channel: 500,
			kept_max_age: 7 * DAY,
			interval: Duration::from_secs(60 * 60),
		}
	}
}

impl Retention {
	pub fn for_settings(storage: &StorageSettings) -> Self {
		Self {
			media_bytes: u64::from(storage.cache_limit_mb.max(1)) * MIB,
			..Self::default()
		}
	}

	pub(crate) fn message_limits(&self) -> MessageLimits {
		MessageLimits {
			per_channel: self.messages_per_channel,
			total: self.messages_total,
			max_age: self.message_max_age,
		}
	}

	pub(crate) fn kept_limits(&self) -> KeptLimits {
		KeptLimits {
			deleted_per_channel: self.kept_deleted_per_channel,
			versions_per_message: self.kept_versions_per_message,
			versions_per_channel: self.kept_versions_per_channel,
			max_age: self.kept_max_age,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pruned {
	pub messages: usize,
	pub kept: usize,
	pub media_files: usize,
	pub media_bytes: u64,
}

struct CachedFile {
	path: PathBuf,
	bytes: u64,
	used: SystemTime,
}

pub(crate) fn prune_media(root: &Path, budget: u64, max_age: Duration, pruned: &mut Pruned) -> Result<()> {
	let cutoff = SystemTime::now()
		.checked_sub(max_age)
		.unwrap_or(SystemTime::UNIX_EPOCH);
	let mut kept = Vec::new();
	let mut total = 0u64;
	for file in cached_files(root)? {
		if file.used < cutoff {
			remove(file, pruned);
		} else {
			total += file.bytes;
			kept.push(file);
		}
	}
	kept.sort_unstable_by_key(|file| file.used);
	for file in kept {
		if total <= budget {
			break;
		}
		total -= file.bytes;
		remove(file, pruned);
	}
	Ok(())
}

pub(crate) fn wipe_media(root: &Path) -> Result<()> {
	match fs::remove_dir_all(root) {
		Ok(()) => Ok(()),
		Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
		Err(error) => Err(error.into()),
	}
}

fn remove(file: CachedFile, pruned: &mut Pruned) {
	match fs::remove_file(&file.path) {
		Ok(()) => {
			pruned.media_files += 1;
			pruned.media_bytes += file.bytes;
		}
		Err(error) if error.kind() == ErrorKind::NotFound => {}
		Err(error) => tracing::debug!(%error, "cached media file was not removed"),
	}
}

fn cached_files(root: &Path) -> Result<Vec<CachedFile>> {
	let shards = match fs::read_dir(root) {
		Ok(shards) => shards,
		Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
		Err(error) => return Err(error.into()),
	};
	let mut files = Vec::new();
	for shard in shards.flatten() {
		if !shard.file_type().is_ok_and(|kind| kind.is_dir()) {
			continue;
		}
		let Ok(entries) = fs::read_dir(shard.path()) else {
			continue;
		};
		for entry in entries.flatten() {
			let Ok(metadata) = entry.metadata() else {
				continue;
			};
			if !metadata.is_file() {
				continue;
			}
			files.push(CachedFile {
				path: entry.path(),
				bytes: metadata.len(),
				used: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
			});
		}
	}
	Ok(files)
}
