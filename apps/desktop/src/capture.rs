use std::path::{Path, PathBuf};
use std::time::Duration;

pub const UNKNOWN_SCREEN: u8 = 3;
pub const WIDTH: f64 = 1280.;
pub const HEIGHT: f64 = 800.;
pub const POLL: Duration = Duration::from_millis(100);
pub const SETTLE: Duration = Duration::from_millis(1500);
pub const LIMIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opening {
	Busiest,
	First,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
	FirstChannel,
}

impl Screen {
	pub fn named(name: &str) -> Option<Screen> {
		match name {
			"app-frame" | "server-rail" | "channel-sidebar" | "timeline" => Some(Screen::FirstChannel),
			_ => None,
		}
	}

	pub fn opening(self) -> Opening {
		match self {
			Screen::FirstChannel => Opening::First,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Capture {
	pub name: String,
	pub screen: Screen,
	pub ready: PathBuf,
}

pub fn signal_ready(path: &Path) -> std::io::Result<()> {
	std::fs::write(path, std::process::id().to_string())
}
