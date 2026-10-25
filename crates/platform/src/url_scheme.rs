use std::path::Path;

use windows::Win32::System::Registry::HKEY_CURRENT_USER;

use crate::Result;
use crate::registry::{self, Key};

const CLASSES: &str = r"Software\Classes";

fn scheme_key(scheme: &str) -> String {
	format!(r"{CLASSES}\{scheme}")
}

fn command_key(scheme: &str) -> String {
	format!(r"{CLASSES}\{scheme}\shell\open\command")
}

pub fn command_for(exe: &Path) -> String {
	format!("\"{}\" \"%1\"", exe.display())
}

pub fn handler(scheme: &str) -> Option<String> {
	registry::read_string(HKEY_CURRENT_USER, &command_key(scheme), "")
}

pub fn is_ours(scheme: &str, exe: &Path) -> bool {
	handler(scheme).is_some_and(|command| command.eq_ignore_ascii_case(&command_for(exe)))
}

pub fn register(scheme: &str, description: &str, exe: &Path) -> Result<()> {
	let root = Key::create(HKEY_CURRENT_USER, &scheme_key(scheme))?;
	root.set_string("", &format!("URL:{description}"))?;
	root.set_string("URL Protocol", "")?;
	let icon = Key::create(HKEY_CURRENT_USER, &format!(r"{}\DefaultIcon", scheme_key(scheme)))?;
	icon.set_string("", &format!("\"{}\",0", exe.display()))?;
	Key::create(HKEY_CURRENT_USER, &command_key(scheme))?.set_string("", &command_for(exe))
}

pub fn unregister_if_ours(scheme: &str, exe: &Path) -> Result<bool> {
	if !is_ours(scheme, exe) {
		return Ok(false);
	}
	registry::delete_tree(HKEY_CURRENT_USER, &scheme_key(scheme))?;
	Ok(true)
}
