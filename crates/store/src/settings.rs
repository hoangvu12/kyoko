use std::fs;
use std::io::ErrorKind;

use protocol::fingerprint::FALLBACK_BUILD_NUMBER;
use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::error::Result;
use crate::paths::Paths;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
	pub theme: String,
	pub locale: String,
	pub storage: StorageSettings,
	pub notifications: NotificationSettings,
	pub appearance: AppearanceSettings,
	pub voice: VoiceSettings,
	pub keybinds: KeybindSettings,
	pub window: WindowSettings,
	pub startup: StartupSettings,
	pub integration: IntegrationSettings,
	pub first_run_acknowledged: bool,
	pub plugins: PluginSettings,
	pub developer: DeveloperSettings,
	pub trusted_link_hosts: Vec<String>,
	pub advanced: AdvancedSettings,
	pub presence: PresenceSettings,
	pub updates: UpdateSettings,
	pub streamer_mode: StreamerModeSettings,
	pub accessibility: AccessibilitySettings,
	pub chat: ChatSettings,
	pub media: MediaSettings,
	pub client: ClientSettings,
	pub safety_locks: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
	pub plaintext_token: bool,
	pub cache_limit_mb: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationSettings {
	pub enabled: bool,
	pub sounds: bool,
	pub show_content: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
	pub compact_messages: bool,
	pub show_avatars_in_compact: bool,
	pub message_group_spacing: u8,
	pub ui_scale: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessibilitySettings {
	pub reduced_motion: bool,
	pub font_scale: f32,
	pub saturation: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatSettings {
	pub inline_media: bool,
	pub link_previews: bool,
	pub show_reactions: bool,
	pub time_24h: bool,
	pub emoji_autocomplete: bool,
	pub typing_animation: bool,
	pub link_warning: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationPlayback {
	#[default]
	Always,
	OnHover,
	Never,
	#[serde(other)]
	Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaSettings {
	pub animation: AnimationPlayback,
	pub original_resolution: bool,
	pub memory_mb: u32,
}

impl Default for MediaSettings {
	fn default() -> Self {
		Self {
			animation: AnimationPlayback::Always,
			original_resolution: false,
			memory_mb: 128,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamerModeSettings {
	pub enabled: bool,
	pub hide_personal: bool,
	pub hide_invites: bool,
	pub disable_sounds: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
	pub input_device: Option<String>,
	pub output_device: Option<String>,
	pub input_volume: u8,
	pub output_volume: u8,
	pub push_to_talk: bool,
	pub noise_suppression: bool,
	pub auto_sensitivity: bool,
	pub sensitivity_db: i8,
	pub camera_device: Option<String>,
	pub video_preset: u8,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindSettings {
	pub push_to_talk: Option<String>,
	pub toggle_mute: Option<String>,
	pub toggle_deafen: Option<String>,
	pub toggle_window: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
	pub width: u32,
	pub height: u32,
	pub x: Option<i32>,
	pub y: Option<i32>,
	pub maximized: bool,
	pub minimize_to_tray: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupSettings {
	pub open_on_login: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationSettings {
	pub discord_links: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AdvancedSettings {
	pub share_gif_usage: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PresenceSettings {
	pub idle_after_secs: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
	pub check_on_start: bool,
	pub channel: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientSettings {
	pub build_number: u64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginSettings {
	pub safe_mode: bool,
	pub enabled: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeveloperSettings {
	pub show_plugin_debug: bool,
	pub developer_mode: bool,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			theme: "dark".to_owned(),
			locale: "en-US".to_owned(),
			storage: StorageSettings::default(),
			notifications: NotificationSettings::default(),
			appearance: AppearanceSettings::default(),
			voice: VoiceSettings::default(),
			keybinds: KeybindSettings::default(),
			window: WindowSettings::default(),
			startup: StartupSettings::default(),
			integration: IntegrationSettings::default(),
			first_run_acknowledged: false,
			plugins: PluginSettings::default(),
			developer: DeveloperSettings::default(),
			trusted_link_hosts: Vec::new(),
			advanced: AdvancedSettings::default(),
			presence: PresenceSettings::default(),
			updates: UpdateSettings::default(),
			streamer_mode: StreamerModeSettings::default(),
			accessibility: AccessibilitySettings::default(),
			chat: ChatSettings::default(),
			media: MediaSettings::default(),
			client: ClientSettings::default(),
			safety_locks: true,
		}
	}
}

impl Default for ClientSettings {
	fn default() -> Self {
		Self {
			build_number: FALLBACK_BUILD_NUMBER,
		}
	}
}

impl Default for PresenceSettings {
	fn default() -> Self {
		Self { idle_after_secs: 600 }
	}
}

impl Default for UpdateSettings {
	fn default() -> Self {
		Self {
			check_on_start: true,
			channel: "stable".to_owned(),
		}
	}
}

impl Default for StorageSettings {
	fn default() -> Self {
		Self {
			plaintext_token: false,
			cache_limit_mb: 512,
		}
	}
}

impl Default for NotificationSettings {
	fn default() -> Self {
		Self {
			enabled: true,
			sounds: true,
			show_content: true,
		}
	}
}

impl Default for AppearanceSettings {
	fn default() -> Self {
		Self {
			compact_messages: false,
			show_avatars_in_compact: false,
			message_group_spacing: 16,
			ui_scale: 1.0,
		}
	}
}

impl StreamerModeSettings {
	pub fn hides_personal(&self) -> bool {
		self.enabled && self.hide_personal
	}

	pub fn hides_invites(&self) -> bool {
		self.enabled && self.hide_invites
	}

	pub fn mutes_sounds(&self) -> bool {
		self.enabled && self.disable_sounds
	}
}

impl Default for AccessibilitySettings {
	fn default() -> Self {
		Self {
			reduced_motion: false,
			font_scale: 1.0,
			saturation: 100,
		}
	}
}

impl Default for ChatSettings {
	fn default() -> Self {
		Self {
			inline_media: true,
			link_previews: true,
			show_reactions: true,
			time_24h: false,
			emoji_autocomplete: true,
			typing_animation: true,
			link_warning: true,
		}
	}
}

impl Default for VoiceSettings {
	fn default() -> Self {
		Self {
			input_device: None,
			output_device: None,
			input_volume: 100,
			output_volume: 100,
			push_to_talk: false,
			noise_suppression: true,
			auto_sensitivity: true,
			sensitivity_db: -50,
			camera_device: None,
			video_preset: 0,
		}
	}
}

impl Default for WindowSettings {
	fn default() -> Self {
		Self {
			width: 1280,
			height: 800,
			x: None,
			y: None,
			maximized: false,
			minimize_to_tray: true,
		}
	}
}

impl Settings {
	pub fn load(paths: &Paths) -> Self {
		let path = paths.settings_path();
		let bytes = match fs::read(&path) {
			Ok(bytes) => bytes,
			Err(error) if error.kind() == ErrorKind::NotFound => return Self::default(),
			Err(error) => {
				tracing::warn!(%error, path = %path.display(), "could not read settings, using defaults");
				return Self::default();
			}
		};
		serde_json::from_slice(&bytes).unwrap_or_else(|error| {
			tracing::warn!(%error, path = %path.display(), "settings file is malformed, using defaults");
			Self::default()
		})
	}

	pub fn save(&self, paths: &Paths) -> Result<()> {
		let bytes = serde_json::to_vec_pretty(self)?;
		write_atomic(&paths.settings_path(), &bytes)
	}
}
