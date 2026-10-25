use std::path::Path;

use bridge::ToastText;
use platform::notifications::{self, Notification};

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

pub fn show(app_id: &str, text: ToastText, sound: bool, on_click: impl FnOnce() + Send + 'static) {
	let notification = Notification {
		title: text.title,
		body: text.body,
		icon_path: None,
		app_id: app_id.to_owned(),
		sound,
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
