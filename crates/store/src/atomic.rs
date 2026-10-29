use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::Result;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
	write_through_temp(path, bytes, false)
}

pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
	write_through_temp(path, bytes, true)
}

fn write_through_temp(path: &Path, bytes: &[u8], private: bool) -> Result<()> {
	let temp_path = temp_path_for(path);
	let written = fill(&temp_path, bytes, private).and_then(|()| fs::rename(&temp_path, path));
	if written.is_err() {
		let _ = fs::remove_file(&temp_path);
	}
	Ok(written?)
}

fn fill(temp_path: &Path, bytes: &[u8], private: bool) -> std::io::Result<()> {
	let mut file = create(temp_path, private)?;
	file.write_all(bytes)?;
	file.sync_all()
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

#[cfg(windows)]
fn create(path: &Path, private: bool) -> std::io::Result<File> {
	use std::os::windows::fs::OpenOptionsExt;

	use windows::Win32::Foundation::GENERIC_WRITE;
	use windows::Win32::Storage::FileSystem::{READ_CONTROL, WRITE_DAC};

	let mut options = OpenOptions::new();
	options.write(true).create_new(true);
	if !private {
		return options.open(path);
	}
	let file = options
		.access_mode(GENERIC_WRITE.0 | WRITE_DAC.0 | READ_CONTROL.0)
		.open(path)?;
	crate::acl::restrict_to_current_user(&file)?;
	Ok(file)
}

#[cfg(not(windows))]
fn create(path: &Path, _private: bool) -> std::io::Result<File> {
	OpenOptions::new().write(true).create_new(true).open(path)
}
