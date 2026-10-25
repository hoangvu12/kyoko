use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::Result;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
	let temp_path = temp_path_for(path);
	let mut file = create(&temp_path)?;
	file.write_all(bytes)?;
	file.sync_all()?;
	drop(file);
	fs::rename(&temp_path, path).inspect_err(|_| {
		let _ = fs::remove_file(&temp_path);
	})?;
	Ok(())
}

fn temp_path_for(path: &Path) -> PathBuf {
	let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
	temp_name.push(format!(
		".{}-{}.tmp",
		std::process::id(),
		TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
	));
	path.with_file_name(temp_name)
}

fn create(path: &Path) -> std::io::Result<File> {
	OpenOptions::new().write(true).create_new(true).open(path)
}
