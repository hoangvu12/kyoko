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
pub const PAGE_EVERY: Duration = Duration::from_secs(1);
pub const SCROLL_PASSES: usize = 10;
pub const BUG_TITLE_TEXT: &str = "Reaction tooltip wraps to three lines";
pub const LAST_MESSAGE: (f32, f32) = (640., 690.);
pub const USER_PANEL_AVATAR: (f32, f32) = (115., 760.);
pub const RAIL_FIRST_GUILD: (f32, f32) = (45., 155.);
pub const SIDEBAR_FIRST_VOICE: (f32, f32) = (200., 459.);
pub const SETTINGS_CONTENT: (f32, f32) = (760., 420.);
pub const SETTINGS_VIDEO_SCROLL: f32 = 640.;
pub const SILENT_TYPING_BUTTON: (f32, f32) = (838., 744.);
pub const GAME_ACTIVITY_BUTTON: (f32, f32) = (194., 760.);
pub const TEXT_REPLACE_SETTINGS: (f32, f32) = (480., 728.);
pub const CLEAR_URLS_SWITCH: (f32, f32) = (723., 476.);
pub const PROFILE_ANCHOR: (f32, f32) = (348., 404.);
pub const SEARCH_QUERY: &str = "the";
pub const SWITCHER_QUERY: &str = "gen";
pub const COMPOSER_PICKER: (f32, f32) = (954., 755.);
pub const TOP_BAR_THREADS: (f32, f32) = (721., 37.);
pub const TOP_BAR_PINS: (f32, f32) = (801., 37.);
pub const TOP_BAR_INBOX: (f32, f32) = (1137., 37.);
pub const TOAST: &str = "You are sending messages too quickly.";
pub const PLUGINS_VARIABLE: &str = "KYOKO_CAPTURE_PLUGINS";
pub const THEMES_VARIABLE: &str = "KYOKO_CAPTURE_THEMES";
pub const KEPT_DELETED: &str = "spoiler alert: the ship sinks at the end";
pub const KEPT_EDITS: [&str; 3] = ["See you at 6", "See you at 7", "See you at 7:30, sorry!"];
pub const GIF_LINK: &str = "https://klipy.com/gifs/cat-dance";
pub const LINK_PREVIEW: &str = "docs are up https://github.com/refact0r/midnight-discord";
pub const FAVOURITE_EMOJI: &str = "\u{1f64f}";
pub const JUMBO_EMOJI: &str = "\u{1f62d}";
pub const OWN_MESSAGE: &str = "can someone ping me";
pub const MARKDOWN_DRAFT: &str = "**Midnight 2.1.2** is *almost* ready\n> thanks to all who tested the pickers\n`--emoji-size-ui` is in the ||secret|| notes";
pub const UPLOADING: &str = "here it is";
pub const UPLOAD_FAILED: &str = "and the other angle";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opening {
	Busiest,
	First,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
	FirstChannel,
	RailFolders,
	RailHover,
	Composer,
	ComposerReply,
	SilentTyping,
	TextReplaceSettings,
	PluginPermissions,
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
	PickerShot(PickerShot),
	ThreadsList,
	ThreadPanel,
	ThreadCreate,
	Pins,
	Inbox,
	Forum,
	ForumPost,
	ForumPostCreate,
	Community(Community),
	ServerSettings(&'static str),
	ChannelSettings(&'static str),
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
	ComposerShot(ComposerShot),
	HiddenChannel,
	MemberNotes,
	Reactions(ReactionShot),
	FriendConfirm(FriendShot),
	VoiceMessage,
	VideoPlayer,
	Components(ComponentShot),
	Polls(PollShot),
	AppsLauncher,
	GameActivity,
	ServerProfile,
	ProfileMenu,
	VoicePanel,
	VoiceCall,
	VoiceVideo,
	VoiceStream,
	VoiceStreamWatching,
	VoiceStreamViewer,
	VoiceRowHover,
	PushToTalkSettings,
	VoiceCamera,
	SettingsCamera,
	GoLive,
	GoLivePreview,
	GoingLive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentShot {
	Buttons,
	MultiSelect,
	V2,
	Modal,
	ModalError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollShot {
	Create,
	Messages,
	Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendShot {
	Remove,
	Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionShot {
	Chips,
	List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerShot {
	GifResults,
	EmojiSearch,
	EmojiNoNitro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionShot {
	CustomEmoji,
	UnicodeEmoji,
	Sticker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerShot {
	OverLimit,
	Uploads,
	Drop,
	UploadFailed,
	PlusMenu,
	EmojiAutocomplete,
	CommandPicker,
	CommandOptions,
	CommandOptional,
	CommandRemind,
	CommandInvalid,
	CommandChoices,
	CommandChoicesLoading,
	CommandChoicesFailed,
	CommandFile,
	CommandFilePicked,
	CommandOnlyOptional,
	CommandReplies,
	CommandBuiltIn,
	BuiltInGif,
	BuiltInSticker,
	BuiltInThread,
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
			"server-rail-hover" => Some(Screen::RailHover),
			"composer" => Some(Screen::Composer),
			"composer-reply" => Some(Screen::ComposerReply),
			"composer-silent-typing" => Some(Screen::SilentTyping),
			"settings-text-replace" => Some(Screen::TextReplaceSettings),
			"settings-plugin-permissions" => Some(Screen::PluginPermissions),
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
			"gif-picker-results" => Some(Screen::PickerShot(PickerShot::GifResults)),
			"emoji-picker-search" => Some(Screen::PickerShot(PickerShot::EmojiSearch)),
			"emoji-picker-no-nitro" => Some(Screen::PickerShot(PickerShot::EmojiNoNitro)),
			"threads-list" => Some(Screen::ThreadsList),
			"thread-panel" => Some(Screen::ThreadPanel),
			"thread-create" => Some(Screen::ThreadCreate),
			"pins" => Some(Screen::Pins),
			"inbox" => Some(Screen::Inbox),
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
			"composer-markdown-over-limit" => Some(Screen::ComposerShot(ComposerShot::OverLimit)),
			"composer-uploads" => Some(Screen::ComposerShot(ComposerShot::Uploads)),
			"composer-drop" => Some(Screen::ComposerShot(ComposerShot::Drop)),
			"composer-upload-failed" => Some(Screen::ComposerShot(ComposerShot::UploadFailed)),
			"composer-plus-menu" => Some(Screen::ComposerShot(ComposerShot::PlusMenu)),
			"emoji-autocomplete" => Some(Screen::ComposerShot(ComposerShot::EmojiAutocomplete)),
			"command-picker" => Some(Screen::ComposerShot(ComposerShot::CommandPicker)),
			"command-options" => Some(Screen::ComposerShot(ComposerShot::CommandOptions)),
			"command-options-optional" => Some(Screen::ComposerShot(ComposerShot::CommandOptional)),
			"command-options-remind" => Some(Screen::ComposerShot(ComposerShot::CommandRemind)),
			"command-choices-invalid" => Some(Screen::ComposerShot(ComposerShot::CommandInvalid)),
			"command-choices-and-autocomplete" => Some(Screen::ComposerShot(ComposerShot::CommandChoices)),
			"command-choices-loading" => Some(Screen::ComposerShot(ComposerShot::CommandChoicesLoading)),
			"command-choices-failed" => Some(Screen::ComposerShot(ComposerShot::CommandChoicesFailed)),
			"command-attachment-option" => Some(Screen::ComposerShot(ComposerShot::CommandFile)),
			"command-attachment-option-picked" => Some(Screen::ComposerShot(ComposerShot::CommandFilePicked)),
			"command-options-only-optional" => Some(Screen::ComposerShot(ComposerShot::CommandOnlyOptional)),
			"command-response-states" => Some(Screen::ComposerShot(ComposerShot::CommandReplies)),
			"command-picker-built-in" => Some(Screen::ComposerShot(ComposerShot::CommandBuiltIn)),
			"built-in-gif" => Some(Screen::ComposerShot(ComposerShot::BuiltInGif)),
			"built-in-sticker" => Some(Screen::ComposerShot(ComposerShot::BuiltInSticker)),
			"built-in-thread" => Some(Screen::ComposerShot(ComposerShot::BuiltInThread)),
			"hidden-channel" => Some(Screen::HiddenChannel),
			"member-notes" => Some(Screen::MemberNotes),
			"server-rail-read-all" => Some(Screen::FirstChannel),
			"reactions" => Some(Screen::Reactions(ReactionShot::Chips)),
			"reactions-list" => Some(Screen::Reactions(ReactionShot::List)),
			"friend-remove-dialog" => Some(Screen::FriendConfirm(FriendShot::Remove)),
			"friend-block-dialog" => Some(Screen::FriendConfirm(FriendShot::Block)),
			"voice-message" => Some(Screen::VoiceMessage),
			"video-player" => Some(Screen::VideoPlayer),
			"message-components" => Some(Screen::Components(ComponentShot::Buttons)),
			"message-components-multi-select" => Some(Screen::Components(ComponentShot::MultiSelect)),
			"message-components-v2" => Some(Screen::Components(ComponentShot::V2)),
			"app-modal" => Some(Screen::Components(ComponentShot::Modal)),
			"app-modal-error" => Some(Screen::Components(ComponentShot::ModalError)),
			"poll-create" => Some(Screen::Polls(PollShot::Create)),
			"poll-in-messages" => Some(Screen::Polls(PollShot::Messages)),
			"poll-in-messages-ended" => Some(Screen::Polls(PollShot::Ended)),
			"apps-launcher" => Some(Screen::AppsLauncher),
			"name-decorations" => Some(Screen::MemberList),
			"name-decorations-profile" => Some(Screen::ProfilePopout),
			"user-panel-game-activity" => Some(Screen::GameActivity),
			"server-profile" => Some(Screen::ServerProfile),
			"profile-menu" => Some(Screen::ProfileMenu),
			"voice-panel" => Some(Screen::VoicePanel),
			"voice-call" => Some(Screen::VoiceCall),
			"voice-video" => Some(Screen::VoiceVideo),
			"voice-stream" => Some(Screen::VoiceStream),
			"voice-stream-watching" => Some(Screen::VoiceStreamWatching),
			"voice-stream-viewer" => Some(Screen::VoiceStreamViewer),
			"voice-camera" => Some(Screen::VoiceCamera),
			"settings-voice-camera" => Some(Screen::SettingsCamera),
			"voice-go-live" => Some(Screen::GoLive),
			"voice-go-live-preview" => Some(Screen::GoLivePreview),
			"voice-going-live" => Some(Screen::GoingLive),
			"channel-sidebar-voice-hover" => Some(Screen::VoiceRowHover),
			"settings-voice-push-to-talk" => Some(Screen::PushToTalkSettings),
			other => settings_page(other)
				.map(Screen::Settings)
				.or_else(|| server_page(other).map(Screen::ServerSettings))
				.or_else(|| channel_page(other).map(Screen::ChannelSettings))
				.or_else(|| moderation_dialog(other).map(Screen::Moderation))
				.or_else(|| mockcord::fixtures::showcase::screen_section(other).map(Screen::Showcase)),
		}
	}

	pub fn opening(self) -> Option<Opening> {
		match self {
			Screen::FirstChannel
			| Screen::RailFolders
			| Screen::RailHover
			| Screen::SidebarCollapsed
			| Screen::NewestImage
			| Screen::Showcase(_)
			| Screen::DmSidebar
			| Screen::DmGroup
			| Screen::Composer
			| Screen::ComposerReply
			| Screen::SilentTyping
			| Screen::TextReplaceSettings
			| Screen::PluginPermissions
			| Screen::MessageHover
			| Screen::MessageMenu
			| Screen::OwnMessage(_)
			| Screen::MemberList
			| Screen::MemberListScrolled
			| Screen::Settings(_)
			| Screen::PushToTalkSettings
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
			| Screen::PickerShot(_)
			| Screen::ThreadsList
			| Screen::ThreadPanel
			| Screen::ThreadCreate
			| Screen::Pins
			| Screen::Inbox
			| Screen::Forum
			| Screen::ForumPost
			| Screen::ForumPostCreate
			| Screen::Community(_)
			| Screen::ServerSettings(_)
			| Screen::ChannelSettings(_)
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
			| Screen::ComposerShot(_)
			| Screen::HiddenChannel
			| Screen::MemberNotes
			| Screen::Reactions(_)
			| Screen::FriendConfirm(_)
			| Screen::VoiceMessage
			| Screen::VideoPlayer
			| Screen::Components(_)
			| Screen::Polls(_)
			| Screen::AppsLauncher
			| Screen::GameActivity
			| Screen::ServerProfile
			| Screen::ProfileMenu
			| Screen::VoicePanel
			| Screen::VoiceCall
			| Screen::VoiceVideo
			| Screen::VoiceStream
			| Screen::VoiceStreamWatching
			| Screen::VoiceStreamViewer
			| Screen::VoiceRowHover
			| Screen::VoiceCamera
			| Screen::SettingsCamera
			| Screen::GoLive
			| Screen::GoLivePreview
			| Screen::GoingLive => Some(Opening::First),
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
		"settings-safety" => Some("safety"),
		"settings-storage" => Some("storage"),
		"settings-voice" => Some("voice"),
		"settings-system" => Some("system"),
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

fn channel_page(name: &str) -> Option<&'static str> {
	match name.strip_prefix("channel-settings-")? {
		"overview" => Some("overview"),
		"voice" => Some("voice"),
		"permissions" => Some("permissions"),
		"invites" => Some("invites"),
		_ => None,
	}
}

fn moderation_dialog(name: &str) -> Option<&'static str> {
	match name.strip_prefix("moderation-")? {
		"nickname" => Some("nickname"),
		"timeout" => Some("timeout"),
		"kick" => Some("kick"),
		"ban" => Some("ban"),
		"roles-submenu" => Some("roles-submenu"),
		_ => None,
	}
}

#[derive(Debug, Clone)]
pub struct Capture {
	pub name: String,
	pub screen: Screen,
	pub ready: PathBuf,
	pub plugins: Vec<String>,
	pub themes: Vec<PathBuf>,
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

pub fn themes_from_env() -> Vec<PathBuf> {
	std::env::var(THEMES_VARIABLE)
		.unwrap_or_default()
		.split(',')
		.map(str::trim)
		.filter(|path| !path.is_empty())
		.map(PathBuf::from)
		.collect()
}

pub fn signal_ready(path: &Path) -> std::io::Result<()> {
	std::fs::write(path, std::process::id().to_string())
}
