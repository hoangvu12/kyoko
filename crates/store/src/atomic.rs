use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::error::Result;

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
	let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
	temp_name.push(".tmp");
	let temp_path = path.with_file_name(temp_name);
	let mut file = create(&temp_path)?;
	file.write_all(bytes)?;
	file.sync_all()?;
	drop(file);
	fs::rename(&temp_path, path).inspect_err(|_| {
		let _ = fs::remove_file(&temp_path);
	})?;
	Ok(())
}

fn create(path: &Path) -> std::io::Result<File> {
	OpenOptions::new()
		.write(true)
		.create(true)
		.truncate(true)
		.open(path)
}
