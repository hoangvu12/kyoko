use media::cdn::Quality;
use media::{Animations, Playback};
use protocol::User;
use serde_json::Value;
use state::State;
use store::{AnimationPlayback, Settings};

use crate::dm_list::DmStatus;
use crate::message::user_display;

const BULLET: char = '\u{2022}';
const MIN_HIDDEN: usize = 3;
const HIDDEN_LIMIT: usize = 12;
const PHONE_SHOWN: usize = 4;
const NAME_LIMIT: usize = 64;
const HOST_LIMIT: usize = 256;
const HOSTS_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
	Now,
	AfterRestart,
	Stored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingKey {
	Theme,
	Locale,
	CompactMessages,
	UiScale,
	ReducedMotion,
	FontScale,
	Saturation,
	TypingAnimation,
	GifAutoplay,
	AnimateEmoji,
	StickerAnimation,
	OriginalResolution,
	MemoryMb,
	InlineMedia,
	LinkPreviews,
	ShowReactions,
	Time24h,
	EmojiAutocomplete,
	LinkWarning,
	TrustedLinkHosts,
	SafetyLocks,
	NotificationsEnabled,
	NotificationSounds,
	NotificationContent,
	StreamerMode,
	StreamerHidePersonal,
	StreamerHideInvites,
	StreamerDisableSounds,
	ToggleWindow,
	OpenOnLogin,
	MinimizeToTray,
	DiscordLinks,
	DeveloperMode,
	ShareGifUsage,
}

impl SettingKey {
	pub const ALL: [SettingKey; 34] = [
		Self::Theme,
		Self::Locale,
		Self::CompactMessages,
		Self::UiScale,
		Self::ReducedMotion,
		Self::FontScale,
		Self::Saturation,
		Self::TypingAnimation,
		Self::GifAutoplay,
		Self::AnimateEmoji,
		Self::StickerAnimation,
		Self::OriginalResolution,
		Self::MemoryMb,
		Self::InlineMedia,
		Self::LinkPreviews,
		Self::ShowReactions,
		Self::Time24h,
		Self::EmojiAutocomplete,
		Self::LinkWarning,
		Self::TrustedLinkHosts,
		Self::SafetyLocks,
		Self::NotificationsEnabled,
		Self::NotificationSounds,
		Self::NotificationContent,
		Self::StreamerMode,
		Self::StreamerHidePersonal,
		Self::StreamerHideInvites,
		Self::StreamerDisableSounds,
		Self::ToggleWindow,
		Self::OpenOnLogin,
		Self::MinimizeToTray,
		Self::DiscordLinks,
		Self::DeveloperMode,
		Self::ShareGifUsage,
	];

	pub fn name(self) -> &'static str {
		match self {
			Self::Theme => "theme",
			Self::Locale => "locale",
			Self::CompactMessages => "appearance.compact_messages",
			Self::UiScale => "appearance.ui_scale",
			Self::ReducedMotion => "accessibility.reduced_motion",
			Self::FontScale => "accessibility.font_scale",
			Self::Saturation => "accessibility.saturation",
			Self::TypingAnimation => "chat.typing_animation",
			Self::GifAutoplay => "media.gif_autoplay",
			Self::AnimateEmoji => "media.animate_emoji",
			Self::StickerAnimation => "media.sticker_animation",
			Self::OriginalResolution => "media.original_resolution",
			Self::MemoryMb => "media.memory_mb",
			Self::InlineMedia => "chat.inline_media",
			Self::LinkPreviews => "chat.link_previews",
			Self::ShowReactions => "chat.show_reactions",
			Self::Time24h => "chat.time_24h",
			Self::EmojiAutocomplete => "chat.emoji_autocomplete",
			Self::LinkWarning => "chat.link_warning",
			Self::TrustedLinkHosts => "trusted_link_hosts",
			Self::SafetyLocks => "safety_locks",
			Self::NotificationsEnabled => "notifications.enabled",
			Self::NotificationSounds => "notifications.sounds",
			Self::NotificationContent => "notifications.show_content",
			Self::StreamerMode => "streamer_mode.enabled",
			Self::StreamerHidePersonal => "streamer_mode.hide_personal",
			Self::StreamerHideInvites => "streamer_mode.hide_invites",
			Self::StreamerDisableSounds => "streamer_mode.disable_sounds",
			Self::ToggleWindow => "keybinds.toggle_window",
			Self::OpenOnLogin => "startup.open_on_login",
			Self::MinimizeToTray => "window.minimize_to_tray",
			Self::DiscordLinks => "integration.discord_links",
			Self::DeveloperMode => "developer.developer_mode",
			Self::ShareGifUsage => "advanced.share_gif_usage",
		}
	}

	pub fn named(name: &str) -> Option<SettingKey> {
		Self::ALL.into_iter().find(|key| key.name() == name)
	}

	pub fn applies(self) -> Applies {
		match self {
			Self::Locale | Self::SafetyLocks => Applies::AfterRestart,
			Self::CompactMessages
			| Self::UiScale
			| Self::FontScale
			| Self::Saturation
			| Self::Time24h
			| Self::EmojiAutocomplete
			| Self::StreamerHideInvites
			| Self::ShareGifUsage => Applies::Stored,
			_ => Applies::Now,
		}
	}

	pub fn read(self, settings: &Settings) -> Value {
		match self {
			Self::Theme => Value::from(settings.theme.as_str()),
			Self::Locale => Value::from(settings.locale.as_str()),
			Self::CompactMessages => Value::from(settings.appearance.compact_messages),
			Self::UiScale => Value::from(settings.appearance.ui_scale),
			Self::ReducedMotion => Value::from(settings.accessibility.reduced_motion),
			Self::FontScale => Value::from(settings.accessibility.font_scale),
			Self::Saturation => Value::from(settings.accessibility.saturation),
			Self::TypingAnimation => Value::from(settings.chat.typing_animation),
			Self::GifAutoplay => Value::from(settings.media.gif_autoplay),
			Self::AnimateEmoji => Value::from(settings.media.animate_emoji),
			Self::StickerAnimation => Value::from(playback_name(settings.media.sticker_animation)),
			Self::OriginalResolution => Value::from(settings.media.original_resolution),
			Self::MemoryMb => Value::from(settings.media.memory_mb),
			Self::InlineMedia => Value::from(settings.chat.inline_media),
			Self::LinkPreviews => Value::from(settings.chat.link_previews),
			Self::ShowReactions => Value::from(settings.chat.show_reactions),
			Self::Time24h => Value::from(settings.chat.time_24h),
			Self::EmojiAutocomplete => Value::from(settings.chat.emoji_autocomplete),
			Self::LinkWarning => Value::from(settings.chat.link_warning),
			Self::TrustedLinkHosts => Value::from(settings.trusted_link_hosts.clone()),
			Self::SafetyLocks => Value::from(settings.safety_locks),
			Self::NotificationsEnabled => Value::from(settings.notifications.enabled),
			Self::NotificationSounds => Value::from(settings.notifications.sounds),
			Self::NotificationContent => Value::from(settings.notifications.show_content),
			Self::StreamerMode => Value::from(settings.streamer_mode.enabled),
			Self::StreamerHidePersonal => Value::from(settings.streamer_mode.hide_personal),
			Self::StreamerHideInvites => Value::from(settings.streamer_mode.hide_invites),
			Self::StreamerDisableSounds => Value::from(settings.streamer_mode.disable_sounds),
			Self::ToggleWindow => Value::from(settings.keybinds.toggle_window.clone().unwrap_or_default()),
			Self::OpenOnLogin => Value::from(settings.startup.open_on_login),
			Self::MinimizeToTray => Value::from(settings.window.minimize_to_tray),
			Self::DiscordLinks => Value::from(settings.integration.discord_links),
			Self::DeveloperMode => Value::from(settings.developer.developer_mode),
			Self::ShareGifUsage => Value::from(settings.advanced.share_gif_usage),
		}
	}

	pub fn write(self, settings: &mut Settings, value: &Value) -> bool {
		match self {
			Self::Theme => set_text(&mut settings.theme, value, is_theme_name),
			Self::Locale => set_text(&mut settings.locale, value, is_locale),
			Self::CompactMessages => set_flag(&mut settings.appearance.compact_messages, value),
			Self::UiScale => set_ranged(&mut settings.appearance.ui_scale, value, 0.5, 2.),
			Self::ReducedMotion => set_flag(&mut settings.accessibility.reduced_motion, value),
			Self::FontScale => set_ranged(&mut settings.accessibility.font_scale, value, 0.8, 1.5),
			Self::Saturation => set_whole(&mut settings.accessibility.saturation, value, 0, 100),
			Self::TypingAnimation => set_flag(&mut settings.chat.typing_animation, value),
			Self::GifAutoplay => set_flag(&mut settings.media.gif_autoplay, value),
			Self::AnimateEmoji => set_flag(&mut settings.media.animate_emoji, value),
			Self::StickerAnimation => match value.as_str().and_then(playback_named) {
				Some(playback) => replace(&mut settings.media.sticker_animation, playback),
				None => false,
			},
			Self::OriginalResolution => set_flag(&mut settings.media.original_resolution, value),
			Self::MemoryMb => set_whole(&mut settings.media.memory_mb, value, 16, 4096),
			Self::InlineMedia => set_flag(&mut settings.chat.inline_media, value),
			Self::LinkPreviews => set_flag(&mut settings.chat.link_previews, value),
			Self::ShowReactions => set_flag(&mut settings.chat.show_reactions, value),
			Self::Time24h => set_flag(&mut settings.chat.time_24h, value),
			Self::EmojiAutocomplete => set_flag(&mut settings.chat.emoji_autocomplete, value),
			Self::LinkWarning => set_flag(&mut settings.chat.link_warning, value),
			Self::TrustedLinkHosts => match hosts(value) {
				Some(hosts) => replace(&mut settings.trusted_link_hosts, hosts),
				None => false,
			},
			Self::SafetyLocks => set_flag(&mut settings.safety_locks, value),
			Self::NotificationsEnabled => set_flag(&mut settings.notifications.enabled, value),
			Self::NotificationSounds => set_flag(&mut settings.notifications.sounds, value),
			Self::NotificationContent => set_flag(&mut settings.notifications.show_content, value),
			Self::StreamerMode => set_flag(&mut settings.streamer_mode.enabled, value),
			Self::StreamerHidePersonal => set_flag(&mut settings.streamer_mode.hide_personal, value),
			Self::StreamerHideInvites => set_flag(&mut settings.streamer_mode.hide_invites, value),
			Self::StreamerDisableSounds => set_flag(&mut settings.streamer_mode.disable_sounds, value),
			Self::ToggleWindow => match binding(value) {
				Some(binding) => replace(&mut settings.keybinds.toggle_window, binding),
				None => false,
			},
			Self::OpenOnLogin => set_flag(&mut settings.startup.open_on_login, value),
			Self::MinimizeToTray => set_flag(&mut settings.window.minimize_to_tray, value),
			Self::DiscordLinks => set_flag(&mut settings.integration.discord_links, value),
			Self::DeveloperMode => set_flag(&mut settings.developer.developer_mode, value),
			Self::ShareGifUsage => set_flag(&mut settings.advanced.share_gif_usage, value),
		}
	}
}

pub fn setting_values(settings: &Settings) -> Vec<(&'static str, Value)> {
	SettingKey::ALL
		.into_iter()
		.map(|key| (key.name(), key.read(settings)))
		.collect()
}

pub fn change_setting(settings: &mut Settings, name: &str, value: &Value) -> Option<SettingKey> {
	let key = SettingKey::named(name)?;
	key.write(settings, value).then_some(key)
}

pub fn media_quality(settings: &Settings) -> Quality {
	if settings.media.original_resolution {
		Quality::Original
	} else {
		Quality::Fitted
	}
}

pub fn media_playback(settings: &Settings) -> Animations {
	let hovered_unless = |always: bool| {
		if always {
			Playback::Always
		} else {
			Playback::OnHover
		}
	};
	Animations {
		gifs: hovered_unless(settings.media.gif_autoplay),
		emoji: hovered_unless(settings.media.animate_emoji),
		stickers: match settings.media.sticker_animation {
			AnimationPlayback::OnHover => Playback::OnHover,
			AnimationPlayback::Never => Playback::Never,
			AnimationPlayback::Always | AnimationPlayback::Unknown => Playback::Always,
		},
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountCard {
	pub name: String,
	pub username: String,
	pub email: String,
	pub phone: String,
	pub accent: Option<u32>,
	pub status: &'static str,
	pub shares_activity: bool,
}

pub fn account_card(state: &State, settings: &Settings) -> Option<AccountCard> {
	let user = state.current_user()?;
	let hidden = settings.streamer_mode.hides_personal();
	let email = user.email.as_deref().map(mask_email).unwrap_or_default();
	let phone = phone_of(user).map(mask_phone).unwrap_or_default();
	let status = match DmStatus::of(state, user.id) {
		DmStatus::Offline => DmStatus::Online,
		status => status,
	};
	Some(AccountCard {
		name: user_display(user),
		username: user.username.clone(),
		email: if hidden { cover(&email) } else { email },
		phone: if hidden { cover(&phone) } else { phone },
		accent: user
			.accent_color
			.and_then(|color| u32::try_from(color).ok())
			.filter(|color| *color <= 0x00ff_ffff),
		status: status.as_str(),
		shares_activity: state.status_settings().shares_activity(),
	})
}

fn phone_of(user: &User) -> Option<&str> {
	user.phone
		.as_ref()
		.and_then(Option::as_deref)
		.filter(|phone| !phone.is_empty())
}

pub fn mask_email(email: &str) -> String {
	match email.split_once('@') {
		Some((local, domain)) => {
			let first: String = local.chars().take(1).collect();
			let hidden = local.chars().count().saturating_sub(1).max(MIN_HIDDEN);
			format!("{first}{}@{domain}", bullets(hidden))
		}
		None => cover(email),
	}
}

pub fn mask_phone(phone: &str) -> String {
	let digits: Vec<char> = phone.chars().filter(char::is_ascii_digit).collect();
	let shown = digits.len().min(PHONE_SHOWN);
	let tail: String = digits[digits.len() - shown..].iter().collect();
	format!("{}{tail}", bullets(digits.len() - shown))
}

fn cover(text: &str) -> String {
	bullets(text.chars().count().min(HIDDEN_LIMIT))
}

fn bullets(count: usize) -> String {
	std::iter::repeat_n(BULLET, count).collect()
}

fn playback_name(playback: AnimationPlayback) -> &'static str {
	match playback {
		AnimationPlayback::OnHover => "on_hover",
		AnimationPlayback::Never => "never",
		AnimationPlayback::Always | AnimationPlayback::Unknown => "always",
	}
}

fn playback_named(name: &str) -> Option<AnimationPlayback> {
	match name {
		"always" => Some(AnimationPlayback::Always),
		"on_hover" => Some(AnimationPlayback::OnHover),
		"never" => Some(AnimationPlayback::Never),
		_ => None,
	}
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
	if *slot == value {
		return false;
	}
	*slot = value;
	true
}

fn set_flag(slot: &mut bool, value: &Value) -> bool {
	value.as_bool().is_some_and(|flag| replace(slot, flag))
}

fn set_text(slot: &mut String, value: &Value, valid: fn(&str) -> bool) -> bool {
	match value.as_str().map(str::trim).filter(|text| valid(text)) {
		Some(text) => replace(slot, text.to_owned()),
		None => false,
	}
}

fn set_ranged(slot: &mut f32, value: &Value, low: f32, high: f32) -> bool {
	match value
		.as_f64()
		.map(|number| number as f32)
		.filter(|number| number.is_finite())
	{
		Some(number) => replace(slot, number.clamp(low, high)),
		None => false,
	}
}

fn set_whole<T: TryFrom<i64> + PartialEq>(slot: &mut T, value: &Value, low: i64, high: i64) -> bool {
	let number = value
		.as_f64()
		.filter(|number| number.is_finite())
		.map(|number| (number.round() as i64).clamp(low, high))
		.and_then(|number| T::try_from(number).ok());
	match number {
		Some(number) => replace(slot, number),
		None => false,
	}
}

fn is_theme_name(name: &str) -> bool {
	!name.is_empty()
		&& name.len() <= NAME_LIMIT
		&& name
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ' '))
}

fn is_locale(name: &str) -> bool {
	!name.is_empty()
		&& name.len() <= NAME_LIMIT
		&& name
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn binding(value: &Value) -> Option<Option<String>> {
	match value {
		Value::Null => Some(None),
		Value::String(text) => {
			let text = text.trim();
			if text.is_empty() {
				Some(None)
			} else if text.len() <= NAME_LIMIT {
				Some(Some(text.to_owned()))
			} else {
				None
			}
		}
		_ => None,
	}
}

fn hosts(value: &Value) -> Option<Vec<String>> {
	let items = match value {
		Value::Array(items) => items.as_slice(),
		Value::Object(fields) if fields.is_empty() => &[],
		_ => return None,
	};
	let mut hosts: Vec<String> = Vec::new();
	for item in items.iter().take(HOSTS_LIMIT) {
		let Some(host) = item
			.as_str()
			.map(|host| host.trim().to_ascii_lowercase())
			.filter(|host| !host.is_empty() && host.len() <= HOST_LIMIT)
		else {
			continue;
		};
		if !hosts.contains(&host) {
			hosts.push(host);
		}
	}
	Some(hosts)
}
