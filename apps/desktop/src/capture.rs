use std::path::{Path, PathBuf};
use std::time::Duration;

pub const UNKNOWN_SCREEN: u8 = 3;
pub const WIDTH: f64 = 1280.;
pub const HEIGHT: f64 = 800.;
pub const POLL: Duration = Duration::from_millis(100);
pub const SETTLE: Duration = Duration::from_millis(1500);
pub const LIMIT: Duration = Duration::from_secs(30);
pub const SCROLL_PASSES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opening {
	Busiest,
	First,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
	FirstChannel,
	RailFolders,
	LoginTos,
	LoginToken,
	SidebarCollapsed,
	NewestImage,
	Showcase(&'static str),
	DmSidebar,
	DmGroup,
}

impl Screen {
	pub fn named(name: &str) -> Option<Screen> {
		match name {
			"app-frame" | "server-rail" | "channel-sidebar" | "timeline" => Some(Screen::FirstChannel),
			"server-rail-folder" => Some(Screen::RailFolders),
			"login-tos" => Some(Screen::LoginTos),
			"login-token" => Some(Screen::LoginToken),
			"channel-sidebar-collapsed" => Some(Screen::SidebarCollapsed),
			"timeline-media" => Some(Screen::NewestImage),
			"dm-sidebar" => Some(Screen::DmSidebar),
			"dm-group" => Some(Screen::DmGroup),
			other => mockcord::fixtures::showcase::screen_section(other).map(Screen::Showcase),
		}
	}

	pub fn opening(self) -> Option<Opening> {
		match self {
			Screen::FirstChannel
			| Screen::RailFolders
			| Screen::SidebarCollapsed
			| Screen::NewestImage
			| Screen::Showcase(_)
			| Screen::DmSidebar
			| Screen::DmGroup => Some(Opening::First),
			Screen::LoginTos | Screen::LoginToken => None,
		}
	}

	pub fn is_home(self) -> bool {
		matches!(self, Screen::DmSidebar | Screen::DmGroup)
	}

	pub fn is_login(self) -> bool {
		matches!(self, Screen::LoginTos | Screen::LoginToken)
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
