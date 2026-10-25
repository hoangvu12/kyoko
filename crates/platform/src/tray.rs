use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::{PlatformError, Result};

static CREATED: AtomicBool = AtomicBool::new(false);

const SHOW_ID: &str = "kyoko.show";
const HIDE_ID: &str = "kyoko.hide";
const QUIT_ID: &str = "kyoko.quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
	Toggle,
	Show,
	Hide,
	Quit,
}

#[derive(Debug, Clone)]
pub struct TrayLabels {
	pub show: String,
	pub hide: String,
	pub quit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayImage {
	pub rgba: Vec<u8>,
	pub width: u32,
	pub height: u32,
}

pub struct Tray {
	icon: TrayIcon,
}

impl Tray {
	pub fn new(
		tooltip: &str,
		image: TrayImage,
		labels: &TrayLabels,
		on_event: impl Fn(TrayEvent) + Send + Sync + 'static,
	) -> Result<Tray> {
		if CREATED.swap(true, Ordering::SeqCst) {
			return Err(PlatformError::AlreadyCreated("tray"));
		}
		let created = Self::create(tooltip, image, labels, on_event);
		if created.is_err() {
			CREATED.store(false, Ordering::SeqCst);
		}
		created
	}

	fn create(
		tooltip: &str,
		image: TrayImage,
		labels: &TrayLabels,
		on_event: impl Fn(TrayEvent) + Send + Sync + 'static,
	) -> Result<Tray> {
		let on_event = Arc::new(on_event);
		let menu_handler = on_event.clone();
		MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
			if let Some(event) = menu_event(event.id.as_ref()) {
				menu_handler(event);
			}
		}));
		TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
			if let TrayIconEvent::Click {
				button: MouseButton::Left,
				button_state: MouseButtonState::Up,
				..
			} = event
			{
				on_event(TrayEvent::Toggle);
			}
		}));
		let icon = TrayIconBuilder::new()
			.with_tooltip(tooltip)
			.with_icon(icon(image)?)
			.with_menu(Box::new(build_menu(labels)?))
			.with_menu_on_left_click(false)
			.build()?;
		Ok(Tray { icon })
	}

	pub fn set_image(&self, image: TrayImage) -> Result<()> {
		self.icon.set_icon(Some(icon(image)?))?;
		Ok(())
	}

	pub fn set_tooltip(&self, tooltip: &str) -> Result<()> {
		self.icon.set_tooltip(Some(tooltip))?;
		Ok(())
	}
}

impl Drop for Tray {
	fn drop(&mut self) {
		MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
		TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
		CREATED.store(false, Ordering::SeqCst);
	}
}

fn icon(image: TrayImage) -> Result<Icon> {
	Ok(Icon::from_rgba(image.rgba, image.width, image.height)?)
}

fn menu_event(id: &str) -> Option<TrayEvent> {
	match id {
		SHOW_ID => Some(TrayEvent::Show),
		HIDE_ID => Some(TrayEvent::Hide),
		QUIT_ID => Some(TrayEvent::Quit),
		_ => None,
	}
}

fn build_menu(labels: &TrayLabels) -> Result<Menu> {
	let menu = Menu::new();
	menu.append_items(&[
		&MenuItem::with_id(SHOW_ID, &labels.show, true, None),
		&MenuItem::with_id(HIDE_ID, &labels.hide, true, None),
		&PredefinedMenuItem::separator(),
		&MenuItem::with_id(QUIT_ID, &labels.quit, true, None),
	])
	.map_err(|error| PlatformError::Os(error.to_string()))?;
	Ok(menu)
}
