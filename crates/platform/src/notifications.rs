use std::path::PathBuf;

use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::HSTRING;

use crate::Result;

const SOUND_NAME: &str = "Default";

#[derive(Debug, Clone)]
pub struct Notification {
	pub title: String,
	pub body: String,
	pub icon_path: Option<PathBuf>,
	pub app_id: String,
	pub sound: bool,
}

pub fn show(notification: &Notification) -> Result<()> {
	let mut toast = notify_rust::Notification::new();
	toast
		.summary(&notification.title)
		.body(&notification.body)
		.appname(&notification.app_id)
		.app_id(&notification.app_id);
	if let Some(icon_path) = &notification.icon_path {
		toast.image_path(&icon_path.to_string_lossy());
	}
	if notification.sound {
		toast.sound_name(SOUND_NAME);
	}
	toast.show()?;
	Ok(())
}

pub fn set_app_user_model_id(id: &str) -> Result<()> {
	unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(id)) }
		.map_err(|error| crate::PlatformError::Os(error.to_string()))
}
