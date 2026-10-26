use std::path::{Path, PathBuf};
use std::time::Duration;

use bridge::FriendsTab;

use crate::pickers::Tab;

pub const UNKNOWN_SCREEN: u8 = 3;
pub const WIDTH: f64 = 1280.;
pub const HEIGHT: f64 = 800.;
pub const POLL: Duration = Duration::from_millis(100);
pub const SETTLE: Duration = Duration::from_millis(1500);
pub const LIMIT: Duration = Duration::from_secs(30);
pub const SCROLL_PASSES: usize = 10;
pub const LAST_MESSAGE: (f32, f32) = (640., 690.);
pub const USER_PANEL_AVATAR: (f32, f32) = (115., 760.);
pub const SILENT_TYPING_BUTTON: (f32, f32) = (848., 742.);
pub const TEXT_REPLACE_SETTINGS: (f32, f32) = (480., 728.);
pub const PROFILE_ANCHOR: (f32, f32) = (348., 404.);
pub const SEARCH_QUERY: &str = "the";
pub const COMPOSER_PICKER: (f32, f32) = (952., 742.);
pub const TOP_BAR_THREADS: (f32, f32) = (721., 37.);
pub const TOP_BAR_PINS: (f32, f32) = (801., 37.);
pub const TOAST: &str = "You are sending messages too quickly.";
pub const PLUGINS_VARIABLE: &str = "KYOKO_CAPTURE_PLUGINS";
pub const KEPT_DELETED: &str = "spoiler alert: the ship sinks at the end";
pub const KEPT_EDITS: [&str; 3] = ["See you at 6", "See you at 7", "See you at 7:30, sorry!"];
pub const GIF_LINK: &str = "https://klipy.com/gifs/cat-dance";
pub const LINK_PREVIEW: &str = "docs are up https://github.com/refact0r/midnight-discord";
pub const FAVOURITE_EMOJI: &str = "\u{1f64f}";
pub const JUMBO_EMOJI: &str = "\u{1f62d}";
pub const OWN_MESSAGE: &str = "can someone ping me";

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
	SilentTyping,
	TextReplaceSettings,
	LoginTos,
	LoginToken,
	LoginQrScanned,
	LoginQrExpired,
	LoginWeb,
	LoginEmail,
	Login2fa,
	LoginCaptcha,
	LoginUnsaved,
	SidebarCollapsed,
	NewestImage,
	Showcase(&'static str),
	DmSidebar,
	DmGroup,
	MessageHover,
	MessageMenu,
	OwnMessage(OwnShot),
	MemberList,
	MemberListScrolled,
	Settings(&'static str),
	UserPanel,
	StatusMenu,
	AccountSwitcher,
	CustomStatus,
	ProfilePopout,
	FullProfile,
	QuickSwitcher,
	Search,
	Lightbox,
	Picker(Tab),
	ThreadsList,
	ThreadPanel,
	ThreadCreate,
	Pins,
	Forum,
	ForumPost,
	ForumPostCreate,
	Community(Community),
	ServerSettings(&'static str),
	Moderation(&'static str),
	Toast,
	Disconnected,
	Friends(FriendsTab),
	CloneMenu,
	CloneDialog,
	KeptDeleted,
	KeptEdits,
	Expressions(ExpressionShot),
	LinkPreview,
	HiddenChannel,
	MemberNotes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionShot {
	CustomEmoji,
	UnicodeEmoji,
	Sticker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnShot {
	Menu,
	DeleteDialog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Community {
	AddServer,
	Forward,
	GuildNotifications,
}

impl Screen {
	pub fn named(name: &str) -> Option<Screen> {
		match name {
			"app-frame" | "server-rail" | "channel-sidebar" | "timeline" => Some(Screen::FirstChannel),
			"server-rail-folder" => Some(Screen::RailFolders),
			"composer" => Some(Screen::Composer),
			"composer-reply" => Some(Screen::ComposerReply),
			"composer-silent-typing" => Some(Screen::SilentTyping),
			"settings-text-replace" => Some(Screen::TextReplaceSettings),
			"login-tos" => Some(Screen::LoginTos),
			"login-token" => Some(Screen::LoginToken),
			"login-qr-scanned" => Some(Screen::LoginQrScanned),
			"login-qr-expired" => Some(Screen::LoginQrExpired),
			"login-web" => Some(Screen::LoginWeb),
			"login-email" => Some(Screen::LoginEmail),
			"login-2fa" => Some(Screen::Login2fa),
			"login-captcha" => Some(Screen::LoginCaptcha),
			"login-unsaved" => Some(Screen::LoginUnsaved),
			"channel-sidebar-collapsed" => Some(Screen::SidebarCollapsed),
			"timeline-media" => Some(Screen::NewestImage),
			"dm-sidebar" => Some(Screen::DmSidebar),
			"dm-group" => Some(Screen::DmGroup),
			"message-hover" => Some(Screen::MessageHover),
			"message-context-menu" => Some(Screen::MessageMenu),
			"message-context-menu-own" => Some(Screen::OwnMessage(OwnShot::Menu)),
			"delete-message-dialog" => Some(Screen::OwnMessage(OwnShot::DeleteDialog)),
			"member-list" => Some(Screen::MemberList),
			"member-list-scrolled" => Some(Screen::MemberListScrolled),
			"user-panel" => Some(Screen::UserPanel),
			"status-menu" => Some(Screen::StatusMenu),
			"account-switcher" => Some(Screen::AccountSwitcher),
			"custom-status" => Some(Screen::CustomStatus),
			"profile-popout" => Some(Screen::ProfilePopout),
			"full-profile" => Some(Screen::FullProfile),
			"quick-switcher" => Some(Screen::QuickSwitcher),
			"search" => Some(Screen::Search),
			"lightbox" => Some(Screen::Lightbox),
			"emoji-picker" => Some(Screen::Picker(Tab::Emoji)),
			"gif-picker" => Some(Screen::Picker(Tab::Gif)),
			"sticker-picker" => Some(Screen::Picker(Tab::Sticker)),
			"threads-list" => Some(Screen::ThreadsList),
			"thread-panel" => Some(Screen::ThreadPanel),
			"thread-create" => Some(Screen::ThreadCreate),
			"pins" => Some(Screen::Pins),
			"forum" => Some(Screen::Forum),
			"forum-post" => Some(Screen::ForumPost),
			"forum-post-create" => Some(Screen::ForumPostCreate),
			"add-server" => Some(Screen::Community(Community::AddServer)),
			"forward" => Some(Screen::Community(Community::Forward)),
			"guild-notifications" => Some(Screen::Community(Community::GuildNotifications)),
			"toast" => Some(Screen::Toast),
			"connection-offline" => Some(Screen::Disconnected),
			"friends-online" => Some(Screen::Friends(FriendsTab::Online)),
			"friends-all" => Some(Screen::Friends(FriendsTab::All)),
			"friends-pending" => Some(Screen::Friends(FriendsTab::Pending)),
			"friends-blocked" => Some(Screen::Friends(FriendsTab::Blocked)),
			"friends-add" => Some(Screen::Friends(FriendsTab::Add)),
			"message-menu-clone" => Some(Screen::CloneMenu),
			"clone-dialog" => Some(Screen::CloneDialog),
			"message-logger-deleted" => Some(Screen::KeptDeleted),
			"message-logger-edits" => Some(Screen::KeptEdits),
			"expressions-in-messages" => Some(Screen::Expressions(ExpressionShot::CustomEmoji)),
			"expressions-in-messages-emoji-popout" => Some(Screen::Expressions(ExpressionShot::UnicodeEmoji)),
			"expressions-in-messages-sticker-popout" => Some(Screen::Expressions(ExpressionShot::Sticker)),
			"link-preview-while-typing" => Some(Screen::LinkPreview),
			"hidden-channel" => Some(Screen::HiddenChannel),
			"member-notes" => Some(Screen::MemberNotes),
			other => settings_page(other)
				.map(Screen::Settings)
				.or_else(|| server_page(other).map(Screen::ServerSettings))
				.or_else(|| moderation_dialog(other).map(Screen::Moderation))
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
			| Screen::SilentTyping
			| Screen::TextReplaceSettings
			| Screen::MessageHover
			| Screen::MessageMenu
			| Screen::OwnMessage(_)
			| Screen::MemberList
			| Screen::MemberListScrolled
			| Screen::Settings(_)
			| Screen::UserPanel
			| Screen::StatusMenu
			| Screen::AccountSwitcher
			| Screen::CustomStatus
			| Screen::ProfilePopout
			| Screen::FullProfile
			| Screen::QuickSwitcher
			| Screen::Search
			| Screen::Lightbox
			| Screen::Picker(_)
			| Screen::ThreadsList
			| Screen::ThreadPanel
			| Screen::ThreadCreate
			| Screen::Pins
			| Screen::Forum
			| Screen::ForumPost
			| Screen::ForumPostCreate
			| Screen::Community(_)
			| Screen::ServerSettings(_)
			| Screen::Moderation(_)
			| Screen::Toast
			| Screen::Disconnected
			| Screen::Friends(_)
			| Screen::CloneMenu
			| Screen::CloneDialog
			| Screen::KeptDeleted
			| Screen::KeptEdits
			| Screen::Expressions(_)
			| Screen::LinkPreview
			| Screen::HiddenChannel
			| Screen::MemberNotes => Some(Opening::First),
			Screen::LoginTos
			| Screen::LoginToken
			| Screen::LoginQrScanned
			| Screen::LoginQrExpired
			| Screen::LoginWeb
			| Screen::LoginEmail
			| Screen::Login2fa
			| Screen::LoginCaptcha
			| Screen::LoginUnsaved => None,
		}
	}

	pub fn is_login(self) -> bool {
		matches!(
			self,
			Screen::LoginTos
				| Screen::LoginToken
				| Screen::LoginQrScanned
				| Screen::LoginQrExpired
				| Screen::LoginWeb
				| Screen::LoginEmail
				| Screen::Login2fa
				| Screen::LoginCaptcha
				| Screen::LoginUnsaved
		)
	}

	pub fn phone(self) -> Option<&'static str> {
		match self {
			Screen::LoginQrScanned => Some("scan"),
			Screen::LoginQrExpired => Some("expire"),
			_ => None,
		}
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
		"settings-themes" => Some("themes"),
		_ => None,
	}
}

fn server_page(name: &str) -> Option<&'static str> {
	match name.strip_prefix("server-settings-")? {
		"overview" => Some("overview"),
		"roles" => Some("roles"),
		"emoji" => Some("emoji"),
		"stickers" => Some("stickers"),
		"members" => Some("members"),
		"invites" => Some("invites"),
		"bans" => Some("bans"),
		"audit-log" => Some("audit-log"),
		_ => None,
	}
}

fn moderation_dialog(name: &str) -> Option<&'static str> {
	match name.strip_prefix("moderation-")? {
		"nickname" => Some("nickname"),
		"timeout" => Some("timeout"),
		"kick" => Some("kick"),
		"ban" => Some("ban"),
		_ => None,
	}
}

#[derive(Debug, Clone)]
pub struct Capture {
	pub name: String,
	pub screen: Screen,
	pub ready: PathBuf,
	pub plugins: Vec<String>,
}

pub fn plugins_from_env() -> Vec<String> {
	std::env::var(PLUGINS_VARIABLE)
		.unwrap_or_default()
		.split(',')
		.map(str::trim)
		.filter(|id| !id.is_empty())
		.map(str::to_owned)
		.collect()
}

pub fn signal_ready(path: &Path) -> std::io::Result<()> {
	std::fs::write(path, std::process::id().to_string())
}
