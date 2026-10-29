use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use url::Url;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

use crate::{PlatformError, Result};

const ALLOWED_SCHEMES: [&str; 2] = ["http", "https"];

pub fn open_url(raw: &str) -> Result<()> {
	let url = Url::parse(raw)?;
	if !ALLOWED_SCHEMES.contains(&url.scheme()) {
		return Err(PlatformError::UnsupportedScheme(url.scheme().to_owned()));
	}
	launch(url.as_str())
}

fn launch(target: &str) -> Result<()> {
	let result = unsafe {
		ShellExecuteW(
			None,
			w!("open"),
			&HSTRING::from(target),
			None,
			None,
			SW_SHOWNORMAL,
		)
	};
	if result.0 as isize > 32 {
		Ok(())
	} else {
		Err(PlatformError::Os(format!(
			"ShellExecuteW failed with code {}",
			result.0 as isize
		)))
	}
}

pub fn reveal_path(path: &Path) -> Result<()> {
	Command::new("explorer")
		.raw_arg(format!("/select,\"{}\"", path.display()))
		.spawn()?;
	Ok(())
}
