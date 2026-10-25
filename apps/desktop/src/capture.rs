use std::path::{Path, PathBuf};
use std::time::Duration;

pub const UNKNOWN_SCREEN: u8 = 3;
pub const WIDTH: f64 = 1280.;
pub const HEIGHT: f64 = 800.;
pub const POLL: Duration = Duration::from_millis(100);
pub const SETTLE: Duration = Duration::from_millis(1500);
pub const LIMIT: Duration = Duration::from_secs(30);
pub const SCROLL_PASSES: usize = 10;
pub const LAST_MESSAGE: (f32, f32) = (640., 690.);
pub const USER_PANEL_AVATAR: (f32, f32) = (99., 774.);
pub const PROFILE_ANCHOR: (f32, f32) = (348., 404.);
pub const SEARCH_QUERY: &str = "the";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opening {
	Busiest,
	First,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
	FirstChannel,
	RailFolders,
	Composer,
	ComposerReply,
	LoginTos,
	LoginToken,
	SidebarCollapsed,
	NewestImage,
	Showcase(&'static str),
	DmSidebar,
	DmGroup,
	MessageHover,
	MessageMenu,
	MemberList,
	MemberListScrolled,
	Settings(&'static str),
	UserPanel,
	StatusMenu,
	CustomStatus,
	ProfilePopout,
	FullProfile,
	QuickSwitcher,
	Search,
}

impl Screen {
	pub fn named(name: &str) -> Option<Screen> {
		match name {
			"app-frame" | "server-rail" | "channel-sidebar" | "timeline" => Some(Screen::FirstChannel),
			"server-rail-folder" => Some(Screen::RailFolders),
			"composer" => Some(Screen::Composer),
			"composer-reply" => Some(Screen::ComposerReply),
			"login-tos" => Some(Screen::LoginTos),
			"login-token" => Some(Screen::LoginToken),
			"channel-sidebar-collapsed" => Some(Screen::SidebarCollapsed),
			"timeline-media" => Some(Screen::NewestImage),
			"dm-sidebar" => Some(Screen::DmSidebar),
			"dm-group" => Some(Screen::DmGroup),
			"message-hover" => Some(Screen::MessageHover),
			"message-context-menu" => Some(Screen::MessageMenu),
			"member-list" => Some(Screen::MemberList),
			"member-list-scrolled" => Some(Screen::MemberListScrolled),
			"user-panel" => Some(Screen::UserPanel),
			"status-menu" => Some(Screen::StatusMenu),
			"custom-status" => Some(Screen::CustomStatus),
			"profile-popout" => Some(Screen::ProfilePopout),
			"full-profile" => Some(Screen::FullProfile),
			"quick-switcher" => Some(Screen::QuickSwitcher),
			"search" => Some(Screen::Search),
			other => settings_page(other)
				.map(Screen::Settings)
				.or_else(|| mockcord::fixtures::showcase::screen_section(other).map(Screen::Showcase)),
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
			| Screen::DmGroup
			| Screen::Composer
			| Screen::ComposerReply
			| Screen::MessageHover
			| Screen::MessageMenu
			| Screen::MemberList
			| Screen::MemberListScrolled
			| Screen::Settings(_)
			| Screen::UserPanel
			| Screen::StatusMenu
			| Screen::CustomStatus
			| Screen::ProfilePopout
			| Screen::FullProfile
			| Screen::QuickSwitcher
			| Screen::Search => Some(Opening::First),
			Screen::LoginTos | Screen::LoginToken => None,
		}
	}

	pub fn is_login(self) -> bool {
		matches!(self, Screen::LoginTos | Screen::LoginToken)
	}
}

fn settings_page(name: &str) -> Option<&'static str> {
	match name {
		"settings-my-account" => Some("my-account"),
		"settings-profiles" => Some("profiles"),
		"settings-privacy" => Some("privacy"),
		"settings-devices" => Some("devices"),
		"settings-connections" => Some("connections"),
		"settings-clips" => Some("clips"),
		"settings-friend-requests" => Some("friend-requests"),
		"settings-appearance" => Some("appearance"),
		"settings-accessibility" => Some("accessibility"),
		"settings-chat" => Some("chat"),
		"settings-notifications" => Some("notifications"),
		"settings-keybinds" => Some("keybinds"),
		"settings-language" => Some("language"),
		"settings-streamer-mode" => Some("streamer-mode"),
		"settings-advanced" => Some("advanced"),
		"settings-plugins" => Some("plugins"),
		_ => None,
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
