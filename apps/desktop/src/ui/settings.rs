use std::cell::{Cell, RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;

use bridge::{AccountCard, SettingKey};
use media::ImageName;
use session::Command;
use state::{Changes, State};
use store::{Settings, Store};
use zgx::reactive::{Runtime, Signal};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, record, text};
use crate::capture;
use crate::theme::Catalog;
use crate::timeline::Prefs;

const ACCOUNT_AVATAR: f64 = 80.;
const WEB_APP: &str = "https://discord.com/channels/@me";
const DEFAULT_PAGE: &str = "my-account";
const PAGE_LIMIT: usize = 64;
const VOICE_PAGE: &str = "voice";
const SCROLL_POINT: f32 = 0.6;

pub type SettingHook = Box<dyn Fn(SettingKey, &Settings) -> Option<String>>;

pub struct SettingsView {
	store: Arc<Store>,
	catalog: RefCell<Catalog>,
	page: Shared,
	values: Shared,
	account: Shared,
	notes: Shared,
	about: Shared,
	open: Cell<bool>,
	last_page: RefCell<String>,
	dimmed: Signal<bool>,
	_scrim: Rc<()>,
	system_motion: Cell<bool>,
	avatar: RefCell<Option<ImageName>>,
	written: RefCell<Vec<(String, String)>>,
	hooks: RefCell<Vec<SettingHook>>,
}

impl SettingsView {
	pub fn new(host: &ScriptHost, runtime: &Runtime, store: Arc<Store>, catalog: Catalog) -> Self {
		let dimmed = runtime.signal(false);
		let view = Self {
			page: host.shared("settings.page", ""),
			values: host.shared("settings.values", Value::Nil),
			account: host.shared("settings.account", Value::Nil),
			notes: host.shared("settings.notes", Value::Nil),
			about: host.shared("settings.about", Value::Nil),
			open: Cell::new(false),
			last_page: RefCell::new(DEFAULT_PAGE.to_owned()),
			_scrim: crate::scrim::track(&dimmed),
			dimmed,
			system_motion: Cell::new(false),
			avatar: RefCell::new(None),
			written: RefCell::new(Vec::new()),
			hooks: RefCell::new(Vec::new()),
			catalog: RefCell::new(catalog),
			store,
		};
		view.about.set(record([
			(
				"data_dir",
				view.store.paths.data_dir().display().to_string().as_str().into(),
			),
			("version", env!("CARGO_PKG_VERSION").into()),
			(
				"platform",
				format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
					.as_str()
					.into(),
			),
		]));
		view.publish_values();
		view.publish_notes();
		view
	}

	pub fn catalog(&self) -> RefMut<'_, Catalog> {
		self.catalog.borrow_mut()
	}

	pub fn is_open(&self) -> bool {
		self.open.get()
	}

	pub fn settings(&self) -> Settings {
		self.store.settings.read().clone()
	}

	pub fn reduced_motion(&self) -> bool {
		self.store.settings.read().accessibility.reduced_motion || self.system_motion.get()
	}

	fn publish_values(&self) {
		let settings = self.settings();
		let json = serde_json::Value::Object(
			bridge::setting_values(&settings)
				.into_iter()
				.map(|(key, value)| (key.to_owned(), value))
				.collect(),
		);
		if let Some(value) = Value::from_json(&json.to_string()) {
			self.values.set(value);
		}
	}

	fn publish_notes(&self) {
		let notes = self.written.borrow();
		let json = serde_json::Value::Object(
			notes
				.iter()
				.map(|(key, note)| (key.clone(), serde_json::Value::from(note.as_str())))
				.collect(),
		);
		if let Some(value) = Value::from_json(&json.to_string()) {
			self.notes.set(value);
		}
	}

	pub fn note(&self, key: &str, note: Option<String>) {
		{
			let mut notes = self.written.borrow_mut();
			notes.retain(|(name, _)| name != key);
			if let Some(note) = note {
				notes.push((key.to_owned(), note));
			}
		}
		self.publish_notes();
	}

	pub fn on_change(&self, hook: impl Fn(SettingKey, &Settings) -> Option<String> + 'static) {
		self.hooks.borrow_mut().push(Box::new(hook));
	}

	fn publish_account(&self, card: Option<AccountCard>, avatar: &str) {
		let Some(card) = card else {
			self.account.set(Value::Nil);
			return;
		};
		let accent = card
			.accent
			.map(|color| format!("#{color:06x}"))
			.unwrap_or_default();
		self.account.set(record([
			("name", card.name.as_str().into()),
			("username", card.username.as_str().into()),
			("email", card.email.as_str().into()),
			("phone", card.phone.as_str().into()),
			("accent", accent.as_str().into()),
			("status", card.status.into()),
			("avatar", avatar.into()),
			("shares_activity", Value::Bool(card.shares_activity)),
		]));
	}
}

impl Link {
	pub(super) fn install_settings(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("settings.open", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.open_settings(text(&argument).unwrap_or_default());
			}
		});
		let actions = Rc::downgrade(link);
		host.action("settings.close", move |_| {
			if let Some(link) = actions.upgrade() {
				link.close_settings();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("settings.set", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let Some(key) = super::field(&argument, "key").and_then(text) else {
				return;
			};
			let value = super::field(&argument, "value")
				.and_then(|value| serde_json::from_str(&value.to_json()).ok())
				.unwrap_or(serde_json::Value::Null);
			link.set_setting(key, &value);
		});
		let actions = Rc::downgrade(link);
		host.action("settings.open_data_folder", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Err(error) = platform::open::reveal_path(link.settings.store.paths.data_dir())
			{
				tracing::warn!(%error, "could not open the data folder");
			}
		});
		let actions = Rc::downgrade(link);
		host.action("settings.share_activity", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.send(Command::SetActivitySharing(matches!(argument, Value::Bool(true))));
			}
		});
		host.action("settings.open_web", move |_| {
			if let Err(error) = platform::open::open_url(WEB_APP) {
				tracing::warn!(%error, "could not open the web app");
			}
		});
	}

	pub(crate) fn settings_view(&self) -> &SettingsView {
		&self.settings
	}

	pub(crate) fn open_settings(&self, page: &str) {
		let page = if is_page_key(page) {
			page.to_owned()
		} else {
			self.settings.last_page.borrow().clone()
		};
		self.settings.page.set(page.as_str().into());
		self.voice_page_shown(&page);
		*self.settings.last_page.borrow_mut() = page;
		if !self.settings.open.replace(true) {
			self.settings.publish_values();
			self.publish_settings_account();
			self.settings.dimmed.set(true);
			self.refresh_visible();
		}
	}

	pub(crate) fn close_settings(&self) {
		if !self.settings.open.replace(false) {
			return;
		}
		self.settings.page.set("".into());
		self.stop_mic_test();
		self.settings.dimmed.set(false);
		self.settings.avatar.borrow_mut().take();
		self.refresh_visible();
	}

	pub(crate) fn set_setting(&self, name: &str, value: &serde_json::Value) {
		let mut changed = None;
		let saved = self
			.settings
			.store
			.update_settings(|settings| changed = bridge::change_setting(settings, name, value));
		if let Err(error) = saved {
			tracing::warn!(%error, "the settings could not be saved");
		}
		let Some(key) = changed else {
			self.settings.publish_values();
			return;
		};
		tracing::debug!(setting = key.name(), "setting changed");
		self.apply_setting(key);
		self.settings.publish_values();
	}

	fn apply_setting(&self, key: SettingKey) {
		let settings = self.settings.settings();
		match key {
			SettingKey::Theme => {
				let applied = self
					.settings
					.catalog()
					.apply(&self.window.runtime, &settings.theme);
				tracing::info!(theme = %applied, "theme switched");
				self.publish_themes();
			}
			SettingKey::ReducedMotion => {
				self.images
					.gallery
					.set_reduced_motion(self.settings.reduced_motion());
				self.motion.set_reduced(self.settings.reduced_motion());
			}
			SettingKey::GifAutoplay | SettingKey::AnimateEmoji | SettingKey::StickerAnimation => {
				self.images
					.gallery
					.set_animations(bridge::media_playback(&settings));
				self.refresh_timeline_prefs(&settings);
			}
			SettingKey::MemoryMb => self
				.images
				.gallery
				.set_budget(settings.media.memory_mb.max(1) as usize * 1024 * 1024),
			SettingKey::InlineMedia
			| SettingKey::LinkPreviews
			| SettingKey::ShowReactions
			| SettingKey::OriginalResolution
			| SettingKey::LinkWarning
			| SettingKey::TrustedLinkHosts => self.refresh_timeline_prefs(&settings),
			SettingKey::StreamerMode | SettingKey::StreamerHidePersonal => self.publish_settings_account(),
			SettingKey::DeveloperMode => self
				.timeline_view
				.set_developer_mode(settings.developer.developer_mode),
			key if key.is_voice() => self.send(Command::ApplyVoiceSettings),
			_ => {}
		}
		let notes: Vec<Option<String>> = self
			.settings
			.hooks
			.borrow()
			.iter()
			.map(|hook| hook(key, &settings))
			.collect();
		if let Some(note) = notes.into_iter().flatten().next() {
			self.settings.note(key.name(), Some(note));
		} else {
			self.settings.note(key.name(), None);
		}
	}

	fn refresh_timeline_prefs(&self, settings: &Settings) {
		let timeline = self.timeline.borrow();
		let name = timeline
			.open()
			.zip(self.state())
			.map(|(open, state)| self.channel_title(&state.read(), open))
			.unwrap_or_default();
		self.timeline_view.set_prefs(
			Prefs::of(settings),
			&settings.trusted_link_hosts,
			&timeline,
			&name,
		);
		drop(timeline);
		self.refresh_visible();
	}

	pub(super) fn system_reduced_motion(&self, system: bool) {
		self.settings.system_motion.set(system);
		self.images
			.gallery
			.set_reduced_motion(self.settings.reduced_motion());
		self.motion.set_reduced(self.settings.reduced_motion());
	}

	pub(super) fn settings_images(&self) -> Option<String> {
		if !self.settings.is_open() {
			return None;
		}
		self.settings.avatar.borrow().as_ref().map(ToString::to_string)
	}

	pub(super) fn settings_apply(&self, state: &State, changes: &Changes) {
		if self.settings.is_open()
			&& (changes.current_user || !changes.presence.is_empty() || changes.session)
		{
			self.publish_account_from(state);
		}
	}

	fn publish_settings_account(&self) {
		match self.state() {
			Some(state) => self.publish_account_from(&state.read()),
			None => self.settings.publish_account(None, ""),
		}
	}

	fn publish_account_from(&self, state: &State) {
		let settings = self.settings.settings();
		let card = bridge::account_card(state, &settings);
		let avatar = self.settings_avatar(state);
		self.settings.publish_account(card, &avatar);
	}

	fn settings_avatar(&self, state: &State) -> String {
		if let Some(name) = self.settings.avatar.borrow().as_ref() {
			return format!("host:{name}");
		}
		let Some(user) = state.current_user() else {
			return String::new();
		};
		let scale = self.scale_factor();
		let Some(request) = bridge::avatar_request(&self.images.cdn, user, ACCOUNT_AVATAR, scale, true)
		else {
			return String::new();
		};
		let name = self.images.gallery.request(request);
		let shown = format!("host:{name}");
		*self.settings.avatar.borrow_mut() = Some(name);
		self.refresh_visible();
		shown
	}

	pub(super) fn settings_settled(&self) -> bool {
		let shown = self.shown.borrow();
		self.settings
			.avatar
			.borrow()
			.as_ref()
			.is_none_or(|name| shown.contains(name.as_str()))
	}

	pub(super) fn scroll_settings(&self, by: f32) {
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(zgx::widgets::WeakUi::upgrade) else {
			return;
		};
		let window = {
			let scene = ui.scene.borrow();
			scene.bounds(scene.root())
		};
		ui.dispatch(zgx::input::InputEvent::Scroll {
			x: window.width * SCROLL_POINT,
			y: window.height * SCROLL_POINT,
			delta_x: 0.,
			delta_y: by,
		});
	}

	pub(super) fn drive_settings(&self, screen: capture::Screen) {
		match screen {
			capture::Screen::Settings(page) => self.open_settings(page),
			capture::Screen::PushToTalkSettings => {
				self.set_setting("voice.push_to_talk", &serde_json::Value::Bool(true));
				self.set_setting("keybinds.push_to_talk", &serde_json::Value::from("Mouse4"));
				self.open_settings(VOICE_PAGE);
			}
			_ => {}
		}
	}
}

fn is_page_key(key: &str) -> bool {
	!key.is_empty()
		&& key.len() <= PAGE_LIMIT
		&& key.chars().all(|character| {
			character.is_ascii_lowercase()
				|| character.is_ascii_digit()
				|| matches!(character, '-' | '.' | '_')
		})
}
