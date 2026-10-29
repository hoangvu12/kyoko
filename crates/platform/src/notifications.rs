use std::path::{Path, PathBuf};
use std::thread;

use notify_rust::NotificationResponse;
use windows::Win32::System::Registry::HKEY_CURRENT_USER;

use crate::Result;
use crate::registry::Key;

const APP_IDS_KEY: &str = r"Software\Classes\AppUserModelId";

#[derive(Debug, Clone)]
pub struct Notification {
	pub title: String,
	pub body: String,
	pub icon_path: Option<PathBuf>,
	pub app_id: String,
}

pub fn show(notification: &Notification, on_click: impl FnOnce() + Send + 'static) -> Result<()> {
	let mut toast = notify_rust::Notification::new();
	toast
		.summary(&notification.title)
		.body(&notification.body)
		.appname(&notification.app_id)
		.app_id(&notification.app_id);
	if let Some(icon_path) = &notification.icon_path {
		toast.image_path(&icon_path.to_string_lossy());
	}
	let handle = toast.show()?;
	thread::Builder::new()
		.name("kyoko-toast".to_owned())
		.spawn(move || {
			let _ = handle.wait_for_response(|response: &NotificationResponse| {
				if matches!(response, NotificationResponse::Default) {
					on_click();
				}
			});
		})?;
	Ok(())
}

pub fn register_app_id(app_id: &str, display_name: &str, icon: Option<&Path>) -> Result<()> {
	let key = Key::create(HKEY_CURRENT_USER, &format!(r"{APP_IDS_KEY}\{app_id}"))?;
	key.set_string("DisplayName", display_name)?;
	if let Some(icon) = icon {
		key.set_string("IconUri", &icon.to_string_lossy())?;
	}
	Ok(())
}
