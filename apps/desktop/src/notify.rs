use std::path::Path;

use audio::Cue;
use bridge::{AlertOutlet, Inbox, ToastText};
use platform::notifications::{self, Notification};
use session::Alert;

use crate::integration::Request;

const DISPLAY_NAME: &str = "Kyoko";
const ICON_FILE: &str = "kyoko-notification.png";
const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon.png");

pub fn register(app_id: &str, icon_dir: &Path) {
	let icon = icon_dir.join(ICON_FILE);
	let icon = match std::fs::write(&icon, ICON_PNG) {
		Ok(()) => Some(icon),
		Err(error) => {
			tracing::warn!(%error, "the notification icon could not be written");
			None
		}
	};
	if let Err(error) = notifications::register_app_id(app_id, DISPLAY_NAME, icon.as_deref()) {
		tracing::warn!(%error, "the notification identity could not be registered");
	}
}

pub fn show(app_id: &str, text: ToastText, on_click: impl FnOnce() + Send + 'static) {
	let notification = Notification {
		title: text.title,
		body: text.body,
		icon_path: None,
		app_id: app_id.to_owned(),
	};
	let spawned = std::thread::Builder::new()
		.name("kyoko-notify".to_owned())
		.spawn(move || match notifications::show(&notification, on_click) {
			Ok(()) => tracing::debug!("notification shown"),
			Err(error) => tracing::warn!(%error, "the notification could not be shown"),
		});
	if let Err(error) = spawned {
		tracing::warn!(%error, "the notification thread did not start");
	}
}

pub struct Desk {
	pub app_id: &'static str,
	pub chime: Cue,
	pub requests: Inbox<Request>,
}

impl AlertOutlet for Desk {
	fn chime(&self) {
		self.chime.play();
	}

	fn toast(&self, alert: &Alert, text: ToastText) {
		let requests = self.requests.clone();
		let (guild_id, channel_id) = (alert.guild_id, alert.channel_id());
		show(self.app_id, text, move || {
			requests.push(Request::Open { guild_id, channel_id });
		});
	}
}
