use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

use bridge::{DeepLink, Inbox, SettingKey, parse_deep_link};
use platform::hotkeys::{HotkeyEvent, Hotkeys};
use platform::tray::TrayEvent;
use protocol::Snowflake;
use session::Command;
use state::Changes;
use store::{Settings, Store};
use zgx_desktop::{ApplicationEvent, TaskSpawner, WindowContext, WindowHandle};
use zgx_script::{ScriptHost, Value};

use crate::notify;
use crate::tray::TrayIcon;
use crate::ui::Link;

pub const APP_ID: &str = "io.github.hoangvu12.Kyoko";
pub const MOCK_APP_ID: &str = "io.github.hoangvu12.Kyoko.Mock";
pub const BACKGROUND_FLAG: &str = "--background";
const DISCORD_SCHEME: &str = "discord";
const DISCORD_SCHEME_NAME: &str = "Discord Protocol";
const TOGGLE_WINDOW: &str = "toggle_window";
const FOCUS_POLL: Duration = Duration::from_millis(500);
const ATTENTION_DELAY: Duration = Duration::from_millis(750);
const HOST_PREFIX: &str = "host:";
const KEYBIND_INVALID: &str = "That key combination can't be used.";
const KEYBINDS_UNAVAILABLE: &str = "Global keybinds are unavailable on this system.";
const TEST_BODY: &str = "Desktop notifications are working.";
const TEST_NOTE_KEY: &str = "notifications.test";
const TEST_SENT: &str = "Notification sent.";

pub enum Request {
	Toggle,
	Show,
	Hide,
	Quit,
	Reopen,
	Links(Vec<String>),
	Open {
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
	},
}

impl Request {
	fn name(&self) -> &'static str {
		match self {
			Request::Toggle => "toggle",
			Request::Show => "show",
			Request::Hide => "hide",
			Request::Quit => "quit",
			Request::Reopen => "reopen",
			Request::Links(_) => "links",
			Request::Open { .. } => "open",
		}
	}
}

pub struct Desktop {
	pub store: Arc<Store>,
	pub requests: Inbox<Request>,
	pub app_id: Option<&'static str>,
	pub toast_id: &'static str,
	pub background: bool,
}

impl Desktop {
	pub fn on_application_event(
		&self,
	) -> impl FnMut(ApplicationEvent, &zgx_desktop::WindowFactory) + 'static {
		let requests = self.requests.clone();
		move |event, _| match event {
			ApplicationEvent::OpenUrls(urls) => requests.push(Request::Links(urls)),
			ApplicationEvent::Reopen => requests.push(Request::Reopen),
		}
	}
}

pub fn apply_startup_settings(settings: &Settings) {
	let exe = match std::env::current_exe() {
		Ok(exe) => exe,
		Err(error) => {
			tracing::warn!(%error, "the executable path is unknown, skipping startup integration");
			return;
		}
	};
	if let Err(error) = platform::autostart::set_enabled(settings.startup.open_on_login, &[BACKGROUND_FLAG]) {
		tracing::warn!(%error, "the start-at-login setting could not be applied");
	}
	let links = if settings.integration.discord_links {
		platform::url_scheme::register(DISCORD_SCHEME, DISCORD_SCHEME_NAME, &exe).map(|()| true)
	} else {
		platform::url_scheme::unregister_if_ours(DISCORD_SCHEME, &exe)
	};
	match links {
		Ok(changed) => tracing::debug!(
			handled = settings.integration.discord_links,
			changed,
			"discord links"
		),
		Err(error) => tracing::warn!(%error, "the discord link handler could not be updated"),
	}
}

pub struct Integration {
	window: WindowHandle,
	tasks: TaskSpawner,
	link: Rc<Link>,
	store: Arc<Store>,
	requests: Inbox<Request>,
	toast_id: &'static str,
	tray: RefCell<Option<TrayIcon>>,
	hotkeys: RefCell<Option<Hotkeys>>,
	visible: Cell<bool>,
	viewing: Cell<Option<bool>>,
	generation: Cell<u64>,
	quitting: Cell<bool>,
	attention_due: Cell<bool>,
	own_launch_links: Cell<bool>,
}

impl Integration {
	pub fn install(
		cx: &mut WindowContext,
		host: &ScriptHost,
		link: &Rc<Link>,
		desktop: Desktop,
	) -> Rc<Integration> {
		let tray = {
			let requests = desktop.requests.clone();
			TrayIcon::create(move |event| {
				requests.push(match event {
					TrayEvent::Toggle => Request::Toggle,
					TrayEvent::Show => Request::Show,
					TrayEvent::Hide => Request::Hide,
					TrayEvent::Quit => Request::Quit,
				})
			})
		};
		let hotkeys = hotkeys(&desktop.store.settings.read(), &desktop.requests);
		let integration = Rc::new(Integration {
			window: cx.window.clone(),
			tasks: cx.tasks.clone(),
			link: link.clone(),
			hotkeys: RefCell::new(hotkeys),
			store: desktop.store,
			requests: desktop.requests,
			toast_id: desktop.toast_id,
			tray: RefCell::new(tray),
			visible: Cell::new(true),
			viewing: Cell::new(None),
			generation: Cell::new(link.session_generation()),
			quitting: Cell::new(false),
			attention_due: Cell::new(false),
			own_launch_links: Cell::new(launch_has_links()),
		});
		let observer = Rc::downgrade(&integration);
		link.observe(move |changes| {
			if let Some(integration) = observer.upgrade() {
				integration.on_changes(changes);
			}
		});
		let gallery = link.gallery().clone();
		host.action("media.copy_image", move |name| copy_image(&gallery, &name));
		let closing = Rc::downgrade(&integration);
		cx.on_close_requested(move || {
			closing
				.upgrade()
				.is_none_or(|integration| integration.close_requested())
		});
		let requests = Rc::downgrade(&integration);
		cx.tasks.spawn(async move { serve(requests).await });
		let focus = Rc::downgrade(&integration);
		cx.tasks.spawn(async move { watch_focus(focus).await });
		integration.watch_settings(host);
		if desktop.background && integration.tray.borrow().is_some() {
			integration.hide();
		}
		integration.schedule_attention();
		integration
	}

	pub fn shutdown(&self) {
		self.requests.close();
		self.tray.borrow_mut().take();
		self.hotkeys.borrow_mut().take();
	}

	fn handle(&self, request: Request) {
		tracing::debug!(request = request.name(), "desktop request");
		match request {
			Request::Toggle => self.toggle(),
			Request::Show | Request::Reopen => self.show(),
			Request::Hide => self.hide(),
			Request::Quit => self.quit(),
			Request::Links(urls) => self.open_links(&urls),
			Request::Open { guild_id, channel_id } => {
				self.show();
				self.link.open(guild_id, channel_id);
			}
		}
	}

	fn watch_settings(self: &Rc<Self>, host: &ScriptHost) {
		let watcher = Rc::downgrade(self);
		self.link.settings_view().on_change(move |key, settings| {
			let integration = watcher.upgrade()?;
			match key {
				SettingKey::ToggleWindow => integration.rebind(settings.keybinds.toggle_window.as_deref()),
				SettingKey::OpenOnLogin | SettingKey::DiscordLinks => {
					apply_startup_settings(settings);
					None
				}
				_ => None,
			}
		});
		let tester = Rc::downgrade(self);
		host.action("settings.test_notification", move |_| {
			if let Some(integration) = tester.upgrade() {
				integration.test_notification();
			}
		});
	}

	fn rebind(&self, binding: Option<&str>) -> Option<String> {
		let mut slot = self.hotkeys.borrow_mut();
		if let Some(hotkeys) = slot.as_mut()
			&& let Err(error) = hotkeys.unregister(TOGGLE_WINDOW)
		{
			tracing::debug!(%error, "the old show/hide hotkey was not registered");
		}
		let binding = binding.map(str::trim).filter(|binding| !binding.is_empty())?;
		if slot.is_none() {
			*slot = new_hotkeys(&self.requests);
		}
		let Some(hotkeys) = slot.as_mut() else {
			return Some(KEYBINDS_UNAVAILABLE.to_owned());
		};
		match hotkeys.register(binding, TOGGLE_WINDOW) {
			Ok(()) => {
				tracing::info!(binding, "show/hide hotkey registered");
				None
			}
			Err(error) => {
				tracing::warn!(%error, binding, "the show/hide hotkey could not be registered");
				Some(KEYBIND_INVALID.to_owned())
			}
		}
	}

	fn test_notification(&self) {
		let sound = {
			let settings = self.store.settings.read();
			settings.notifications.sounds && !settings.streamer_mode.mutes_sounds()
		};
		let text = bridge::ToastText {
			title: "Kyoko".to_owned(),
			body: TEST_BODY.to_owned(),
		};
		notify::show(self.toast_id, text, sound, || {});
		self.link
			.settings_view()
			.note(TEST_NOTE_KEY, Some(TEST_SENT.to_owned()));
	}

	fn close_requested(&self) -> bool {
		if self.quitting.get() {
			return true;
		}
		let to_tray = self.store.settings.read().window.minimize_to_tray && self.tray.borrow().is_some();
		if to_tray {
			self.hide();
		}
		!to_tray
	}

	fn toggle(&self) {
		if self.visible.get() && platform::foreground::belongs_to_this_process() {
			self.hide();
		} else {
			self.show();
		}
	}

	fn show(&self) {
		self.visible.set(true);
		self.window.set_visible(true);
		self.window.set_minimized(false);
		self.window.request_focus();
	}

	fn hide(&self) {
		self.visible.set(false);
		self.window.set_visible(false);
		self.update_viewing(false);
	}

	fn quit(&self) {
		self.quitting.set(true);
		self.window.close();
	}

	fn open_links(&self, urls: &[String]) {
		let links: Vec<DeepLink> = urls.iter().filter_map(|url| parse_deep_link(url)).collect();
		let own = self.own_launch_links.replace(false);
		tracing::info!(urls = urls.len(), links = links.len(), own, "links received");
		if !own {
			self.show();
		}
		let target = links.iter().find_map(|link| match *link {
			DeepLink::Channel {
				guild_id, channel_id, ..
			} => Some((guild_id, channel_id)),
			DeepLink::Home => None,
		});
		if let Some((guild_id, channel_id)) = target {
			self.link.open(guild_id, channel_id);
		}
	}

	fn on_changes(self: &Rc<Self>, changes: &Changes) {
		if changes.alerts {
			self.raise_alerts();
		}
		let badges = changes.read_state
			|| changes.guild_list
			|| changes.private_channels
			|| changes.user_settings
			|| !changes.channel_lists.is_empty();
		if badges {
			self.schedule_attention();
		}
	}

	fn raise_alerts(&self) {
		let alerts = self.link.take_alerts();
		if alerts.is_empty() {
			return;
		}
		let (enabled, show_content, sound) = {
			let settings = self.store.settings.read();
			(
				settings.notifications.enabled,
				settings.notifications.show_content,
				settings.notifications.sounds && !settings.streamer_mode.mutes_sounds(),
			)
		};
		tracing::debug!(alerts = alerts.len(), enabled, "notifications raised");
		if !enabled {
			return;
		}
		let Some(state) = self.link.state() else {
			return;
		};
		for alert in alerts {
			let text = bridge::toast_text(&state.read(), &alert, show_content);
			let requests = self.requests.clone();
			let (guild_id, channel_id) = (alert.guild_id, alert.channel_id());
			notify::show(self.toast_id, text, sound, move || {
				requests.push(Request::Open { guild_id, channel_id });
			});
		}
	}

	fn schedule_attention(self: &Rc<Self>) {
		if self.attention_due.replace(true) {
			return;
		}
		let integration = Rc::downgrade(self);
		self.tasks.spawn(async move {
			zgx::timer::sleep(ATTENTION_DELAY).await;
			if let Some(integration) = integration.upgrade() {
				integration.attention_due.set(false);
				integration.show_attention();
			}
		});
	}

	fn show_attention(&self) {
		let attention = self
			.link
			.state()
			.map(|state| state.read().attention())
			.unwrap_or_default();
		if let Some(tray) = self.tray.borrow().as_ref() {
			tray.show(attention);
		}
	}

	fn update_viewing(&self, viewing: bool) {
		let generation = self.link.session_generation();
		let new_session = self.generation.replace(generation) != generation;
		if new_session {
			self.show_attention();
		}
		if self.viewing.replace(Some(viewing)) != Some(viewing) || new_session {
			self.link.send(Command::SetViewing(viewing));
		}
	}
}

fn launch_has_links() -> bool {
	std::env::args_os().skip(1).any(|arg| {
		let text = arg.to_string_lossy();
		!text.starts_with('-')
			&& (std::path::Path::new(&arg).exists()
				|| text.split_once(':').is_some_and(|(scheme, _)| scheme.len() > 1))
	})
}

fn hotkeys(settings: &Settings, requests: &Inbox<Request>) -> Option<Hotkeys> {
	let binding = settings.keybinds.toggle_window.as_deref()?.trim();
	if binding.is_empty() {
		return None;
	}
	let mut hotkeys = new_hotkeys(requests)?;
	match hotkeys.register(binding, TOGGLE_WINDOW) {
		Ok(()) => Some(hotkeys),
		Err(error) => {
			tracing::warn!(%error, binding, "the show/hide hotkey could not be registered");
			None
		}
	}
}

fn new_hotkeys(requests: &Inbox<Request>) -> Option<Hotkeys> {
	let requests = requests.clone();
	let created = Hotkeys::new(move |event: HotkeyEvent| {
		if event.pressed && event.id == TOGGLE_WINDOW {
			requests.push(Request::Toggle);
		}
	});
	match created {
		Ok(hotkeys) => Some(hotkeys),
		Err(error) => {
			tracing::warn!(%error, "global hotkeys are unavailable");
			None
		}
	}
}

async fn serve(integration: Weak<Integration>) {
	let Some(requests) = integration
		.upgrade()
		.map(|integration| integration.requests.clone())
	else {
		return;
	};
	while let Some(batch) = requests.next().await {
		let Some(integration) = integration.upgrade() else {
			return;
		};
		for request in batch {
			integration.handle(request);
		}
	}
}

async fn watch_focus(integration: Weak<Integration>) {
	loop {
		zgx::timer::sleep(FOCUS_POLL).await;
		let Some(integration) = integration.upgrade() else {
			return;
		};
		let viewing = integration.visible.get() && platform::foreground::belongs_to_this_process();
		integration.update_viewing(viewing);
	}
}

fn copy_image(gallery: &media::Gallery, name: &Value) {
	let Value::Str(name) = name else {
		return;
	};
	let name = name.strip_prefix(HOST_PREFIX).unwrap_or(name);
	let Some(pixels) = gallery.pixels(name) else {
		tracing::warn!("the image to copy is not loaded");
		return;
	};
	let image = platform::clipboard::RgbaImage {
		width: pixels.width as usize,
		height: pixels.height as usize,
		bytes: pixels.rgba.to_vec(),
	};
	let spawned = std::thread::Builder::new()
		.name("kyoko-clipboard".to_owned())
		.spawn(move || match platform::clipboard::write_image(&image) {
			Ok(()) => tracing::info!(width = image.width, height = image.height, "image copied"),
			Err(error) => tracing::warn!(%error, "the image could not be copied"),
		});
	if let Err(error) = spawned {
		tracing::warn!(%error, "the clipboard thread did not start");
	}
}
