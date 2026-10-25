mod composer;
mod home;
mod sign_in;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use bridge::{Flow, Inbox, OpenChannel, Timeline};
use media::{Gallery, ImageName, Update};
use protocol::Snowflake;
use session::Alert;
use session::{Command, Session};

pub use sign_in::SignIn;
use state::{Changes, ConnectionStatus, DisconnectReason, State};
use zgx::compose::prelude::*;
use zgx::image::ImageData;
use zgx::reactive::{Effect, Runtime, Signal};
use zgx_desktop::{Application, Titlebar, WindowHandle, WindowInfo, WindowOptions};
use zgx_script::{ScriptHost, Shared, Value};

use crate::capture::{self, Capture, Opening};
use crate::dm_sidebar::{self, DmSidebarView};
use crate::frame::{self, Frame};
use crate::integration::{Desktop, Integration};
use crate::rail::{self, Icons, RailView};
use crate::sidebar::{self, SidebarView};
use crate::theme::Catalog;
use crate::timeline::{Prefs, Setup, TimelineView};

const UI_DIR_VARIABLE: &str = "KYOKO_UI_DIR";
const THEMES_DIR_VARIABLE: &str = "KYOKO_THEMES_DIR";
const FRAME_POLL: Duration = Duration::from_millis(50);
const FRAME_TRIES: usize = 100;
const TIMELINE_SLOT: &str = "timeline";
const RELOAD_EVERY: Duration = Duration::from_millis(250);
const AVATAR_POINTS: f64 = 32.;
const WIDTH: f64 = 1100.;
const HEIGHT: f64 = 720.;

type Observer = Box<dyn Fn(&Changes)>;

pub struct Images {
	pub gallery: Gallery,
	pub cdn: String,
	pub mock_origin: Option<String>,
	pub reduced_motion: bool,
	pub budget: usize,
	pub prefs: Prefs,
}

struct Messages {
	code_font: String,
	store: Option<Arc<store::Store>>,
}

pub struct Appearance {
	pub theme: String,
	pub user_themes: PathBuf,
}

pub struct Launch {
	pub opening: Option<Opening>,
	pub capture: Option<Capture>,
}

pub fn run(
	sign_in: SignIn,
	inbox: Inbox<Changes>,
	images: Images,
	appearance: Appearance,
	launch: Launch,
	desktop: Desktop,
	after_close: impl Fn() + 'static,
) -> Result<()> {
	let Launch { opening, capture } = launch;
	let script = ui_dir().join("app.luau");
	let (width, height) = match capture {
		Some(_) => (capture::WIDTH, capture::HEIGHT),
		None => (WIDTH, HEIGHT),
	};
	let mut application = Application::new();
	if let Some(id) = desktop.app_id {
		application = application
			.application_id(id)
			.map_err(|error| anyhow!("the application id was refused: {error}"))?
			.on_application_event(desktop.on_application_event());
	}
	application
		.window(WindowOptions {
			title: "Kyoko".into(),
			width,
			height,
			min_size: Some((640., 420.)),
			titlebar: Titlebar::Hidden,
			..Default::default()
		})
		.run(move |cx| {
			let catalog = Catalog::load(&[themes_dir(), appearance.user_themes.clone()]);
			let theme = catalog.install(&cx.ui.runtime, &appearance.theme);
			tracing::info!(%theme, themes = catalog.palettes.len(), "themes installed");
			let host = ScriptHost::new(&cx.ui.runtime, script);
			let window = Window {
				runtime: cx.ui.runtime.clone(),
				info: cx.window_info.clone(),
				handle: cx.window.clone(),
				frame: Cell::new(None),
				slot: cx.ui.runtime.signal(Slot::default()),
			};
			let messages = Messages {
				code_font: catalog.code_font(&theme),
				store: capture.is_none().then(|| desktop.store.clone()),
			};
			let link = Link::install(&host, window, sign_in, inbox.clone(), images, messages);
			link.opening.set(opening);
			link.rail.restore(desktop.store.clone());
			let drain = link.clone();
			cx.tasks.spawn(async move { drain.drain().await });
			cx.tasks.spawn(link.clone().follow_sign_in());
			cx.tasks.spawn(link.clone().follow_composer_files());
			cx.tasks.spawn(link.clone().tick_composer());
			if let Some(capture) = capture.clone() {
				cx.tasks.spawn(link.clone().signal_when_settled(capture));
			}
			let pictures = link.clone();
			cx.tasks.spawn(async move { pictures.show_images().await });
			cx.tasks.spawn(link.clone().attach_frame());
			if let Err(error) = host.reload() {
				tracing::warn!(%error, "the interface script did not load");
			}
			if cfg!(debug_assertions) {
				cx.tasks.spawn(host.watch(RELOAD_EVERY));
			}
			let integration = capture
				.is_none()
				.then(|| Integration::install(cx, &host, &link, desktop));
			let slot = link.window.slot.clone();
			let transcript = link.timeline_view.view();
			let dropped = Rc::downgrade(&link);
			let hovered = Rc::downgrade(&link);
			cx.on_closed(move || {
				frame::wake_until_loop_ends();
				if let Some(integration) = integration {
					integration.shutdown();
				}
				link.shutdown();
				after_close();
			});
			cx.render(
				overlay()
					.w_full()
					.h_full()
					.child(host.view())
					.child(timeline_slot(slot, transcript))
					.on_event(move |event| {
						if event.phase == zgx::input::EventPhase::Capture {
							return;
						}
						let hovering = match event.event {
							zgx::input::InputEvent::FileHover { .. } => true,
							zgx::input::InputEvent::FileHoverCancelled
							| zgx::input::InputEvent::FilesDrop { .. }
							| zgx::input::InputEvent::FilesDropRejected { .. } => false,
							_ => return,
						};
						if let Some(link) = hovered.upgrade() {
							link.files_hovering(hovering);
						}
					})
					.on_files_drop(move |paths, _| {
						if let Some(link) = dropped.upgrade() {
							link.files_dropped(paths);
						}
					}),
			);
		})
		.map_err(|error| anyhow!("the window failed: {error}"))
}

pub fn hand_over(app_id: &str) -> Result<()> {
	Application::new()
		.application_id(app_id)
		.map_err(|error| anyhow!("the application id was refused: {error}"))?
		.run(|_| {})
		.map_err(|error| anyhow!("the running kyoko did not answer: {error}"))
}

fn timeline_slot(slot: Signal<Slot>, transcript: View) -> View {
	div().child(transcript).reactive_style(move || {
		let slot = slot.get();
		Styles::new()
			.absolute()
			.left(slot.x)
			.top(slot.y)
			.w(slot.width)
			.h(slot.height)
	})
}

fn themes_dir() -> PathBuf {
	std::env::var_os(THEMES_DIR_VARIABLE)
		.map(PathBuf::from)
		.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes"))
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Slot {
	x: f32,
	y: f32,
	width: f32,
	height: f32,
}

fn ui_dir() -> PathBuf {
	std::env::var_os(UI_DIR_VARIABLE)
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"))
}

struct Window {
	runtime: Runtime,
	info: WindowInfo,
	handle: WindowHandle,
	frame: Cell<Option<Frame>>,
	slot: Signal<Slot>,
}

pub(crate) struct Link {
	window: Window,
	host: ScriptHost,
	flow: RefCell<Flow>,
	account: sign_in::Account,
	inbox: Inbox<Changes>,
	images: Images,
	image_wakes: Inbox<()>,
	avatar: RefCell<Option<ImageName>>,
	avatar_requested_at: Cell<Option<Instant>>,
	avatar_shown: Cell<bool>,
	opening: Cell<Option<Opening>>,
	sidebar: SidebarView,
	sidebar_avatars: RefCell<Vec<String>>,
	dms: DmSidebarView,
	dm_avatars: RefCell<Vec<String>>,
	reported_slot: Cell<Slot>,
	timeline: RefCell<Timeline>,
	timeline_view: Rc<TimelineView>,
	composer: composer::ComposerView,
	luau_visible: RefCell<Vec<String>>,
	rail: RailView,
	rail_icons: RefCell<Vec<String>>,
	shown: RefCell<HashSet<String>>,
	status: Shared,
	open_channel: Shared,
	channel_name: Shared,
	channel_topic: Shared,
	avatar_ready: Shared,
	avatar_name: Shared,
	observer: RefCell<Option<Observer>>,
	maximized: Shared,
	frame_effect: RefCell<Option<Effect>>,
}

impl Link {
	fn install(
		host: &ScriptHost,
		window: Window,
		sign_in: SignIn,
		inbox: Inbox<Changes>,
		images: Images,
		messages: Messages,
	) -> Rc<Link> {
		let image_wakes = Inbox::new();
		let wakes = image_wakes.clone();
		images.gallery.set_waker(move || wakes.push(()));
		let timeline_view = TimelineView::new(
			&window.runtime,
			Setup {
				gallery: images.gallery.clone(),
				cdn: images.cdn.clone(),
				mock_origin: images.mock_origin.clone(),
				code_font: messages.code_font,
				prefs: images.prefs,
				store: messages.store,
			},
		);
		let composer = composer::ComposerView::install(host, sign_in.runtime.clone());
		let link = Rc::new(Link {
			window,
			host: host.clone(),
			flow: RefCell::new(sign_in.flow),
			account: sign_in::Account::install(host, sign_in.runtime, sign_in.storage_warnings),
			inbox,
			images,
			image_wakes,
			avatar: RefCell::new(None),
			avatar_requested_at: Cell::new(None),
			avatar_shown: Cell::new(false),
			opening: Cell::new(None),
			sidebar: SidebarView::new(host),
			sidebar_avatars: RefCell::new(Vec::new()),
			dms: DmSidebarView::new(host),
			dm_avatars: RefCell::new(Vec::new()),
			reported_slot: Cell::new(Slot::default()),
			timeline: RefCell::new(Timeline::default()),
			timeline_view,
			composer,
			luau_visible: RefCell::new(Vec::new()),
			rail: RailView::new(host),
			rail_icons: RefCell::new(Vec::new()),
			shown: RefCell::new(HashSet::new()),
			status: host.shared("session.status", "Connecting"),
			open_channel: host.shared("timeline.channel_id", ""),
			channel_name: host.shared("timeline.channel_name", ""),
			channel_topic: host.shared("timeline.channel_topic", ""),
			avatar_ready: host.shared("user.avatar_ready", false),
			avatar_name: host.shared("user.avatar", ""),
			maximized: host.shared("window.maximized", false),
			frame_effect: RefCell::new(None),
			observer: RefCell::new(None),
		});
		Link::install_window_actions(host, &link);
		rail::install_actions(host, &link);
		Link::install_sign_in(host, &link);
		sidebar::install_actions(host, &link);
		dm_sidebar::install_actions(host, &link);
		Link::install_composer(host, &link);
		let actions = Rc::downgrade(&link);
		host.action("rail.select_guild", move |argument| {
			if let (Some(link), Some(guild_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.select_guild(guild_id);
			}
		});
		let actions = Rc::downgrade(&link);
		host.function("media.visible", move |names| {
			if let Some(link) = actions.upgrade() {
				*link.luau_visible.borrow_mut() = texts(&names).into_iter().map(str::to_owned).collect();
				link.refresh_visible();
			}
			Value::Nil
		});
		let actions = Rc::downgrade(&link);
		link.timeline_view.on_command(move |command| {
			if let Some(link) = actions.upgrade() {
				link.send(command);
			}
		});
		let actions = Rc::downgrade(&link);
		link.timeline_view.on_visible_changed(move |()| {
			if let Some(link) = actions.upgrade() {
				link.refresh_visible();
			}
		});
		let gallery = link.images.gallery.clone();
		host.function("media.hover", move |name| {
			gallery.set_hovered(text(&name).filter(|name| !name.is_empty()));
			Value::Nil
		});
		let gallery = link.images.gallery.clone();
		let reduced_setting = link.images.reduced_motion;
		host.function("media.reduced_motion", move |system| {
			gallery.set_reduced_motion(reduced_setting || matches!(system, Value::Bool(true)));
			Value::Nil
		});
		link.images.gallery.set_reduced_motion(reduced_setting);
		link
	}

	fn install_window_actions(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("window.minimize", move |_| {
			if let Some(link) = actions.upgrade() {
				link.window.handle.set_minimized(true);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("window.maximize", move |_| {
			if let Some(link) = actions.upgrade() {
				let maximized = link.window.frame.get().is_some_and(Frame::is_maximized);
				link.window.handle.set_maximized(!maximized);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("window.close", move |_| {
			if let Some(link) = actions.upgrade() {
				link.window.handle.request_close();
			}
		});
		let actions = Rc::downgrade(link);
		host.function("frame.slot", move |argument| {
			if let (Some(link), Some((name, slot))) = (actions.upgrade(), slot(&argument))
				&& name == TIMELINE_SLOT
			{
				link.place_slot(slot);
			}
			Value::Nil
		});
	}

	async fn attach_frame(self: Rc<Self>) {
		for _ in 0..FRAME_TRIES {
			if let Some(frame) = Frame::attach() {
				self.window.frame.set(Some(frame));
				let link = Rc::downgrade(&self);
				let bounds = self.window.info.bounds.clone();
				let effect = self.window.runtime.effect_now(move || {
					bounds.get();
					if let Some(link) = link.upgrade() {
						link.maximized.set(frame.is_maximized().into());
					}
				});
				*self.frame_effect.borrow_mut() = Some(effect);
				return;
			}
			zgx::timer::sleep(FRAME_POLL).await;
		}
		tracing::warn!("the native window was not found; close requests rely on zgx alone");
	}

	async fn show_images(self: Rc<Self>) {
		while self.image_wakes.next().await.is_some() {
			for update in self.images.gallery.drain(Instant::now()) {
				self.show_image(update);
			}
		}
	}

	fn show_image(&self, update: Update) {
		match update {
			Update::Show { name, pixels } => {
				self.shown.borrow_mut().insert(name.as_str().to_owned());
				match ImageData::new(pixels.width, pixels.height, pixels.rgba) {
					Ok(image) => {
						let image = Arc::new(image);
						self.timeline_view.show_image(&name, Some(image.clone()));
						self.host.image(&name, Some(image));
					}
					Err(error) => tracing::warn!(error, "a decoded image was not usable"),
				}
				if self.avatar.borrow().as_ref() == Some(&name) {
					self.avatar_shown();
				}
			}
			Update::Clear { name } => {
				self.timeline_view.show_image(&name, None);
				self.shown.borrow_mut().remove(name.as_str());
				self.host.image(&name, None);
			}
		}
	}

	fn avatar_shown(&self) {
		let Some(requested) = self.avatar_requested_at.take() else {
			return;
		};
		self.avatar_shown.set(true);
		self.avatar_ready.set(true.into());
		tracing::info!(
			elapsed_ms = requested.elapsed().as_millis() as u64,
			"avatar decoded and handed to the window"
		);
	}

	async fn drain(self: Rc<Self>) {
		while let Some(batch) = self.inbox.next().await {
			tracing::trace!(items = batch.len(), "inbox batch");
			let mut changes = Changes::default();
			for item in batch {
				changes.merge(item);
			}
			self.apply(&changes);
		}
	}

	pub(crate) fn observe(&self, observer: impl Fn(&Changes) + 'static) {
		*self.observer.borrow_mut() = Some(Box::new(observer));
	}

	pub(crate) fn gallery(&self) -> &Gallery {
		&self.images.gallery
	}

	pub(crate) fn rail(&self) -> &RailView {
		&self.rail
	}

	pub(crate) fn sidebar(&self) -> &SidebarView {
		&self.sidebar
	}

	pub(crate) fn take_alerts(&self) -> Vec<Alert> {
		self.flow
			.borrow()
			.session()
			.map(Session::take_alerts)
			.unwrap_or_default()
	}

	pub(crate) fn open(&self, guild_id: Option<Snowflake>, channel_id: Snowflake) {
		match guild_id {
			Some(guild_id) => {
				self.select_guild(guild_id);
				self.select_channel(channel_id);
			}
			None => self.select_dm(channel_id),
		}
	}

	fn apply(&self, changes: &Changes) {
		if let Some(observer) = self.observer.borrow().as_ref() {
			observer(changes);
		}
		self.watch_session(changes);
		let opened = self.publish(changes);
		if let Some((opening, open)) = opened {
			if let Some(guild_id) = open.guild_id {
				self.select_guild(guild_id);
			}
			self.select_channel(open.channel_id);
			tracing::info!(
				?opening,
				guilds = self.sidebar.sidebar().guilds().len(),
				channels = self.sidebar.sidebar().channels().count(),
				"opened a channel on launch"
			);
		}
	}

	async fn signal_when_settled(self: Rc<Self>, capture: Capture) {
		self.drive_capture(capture.screen);
		self.until_settled(&capture, false).await;
		match &capture.screen {
			capture::Screen::RailFolders => self.rail.open_all(),
			capture::Screen::NewestImage => self.reveal(None).await,
			capture::Screen::Showcase(section) => {
				self.reveal(Some(&mockcord::fixtures::showcase::marker(section)))
					.await
			}
			capture::Screen::SidebarCollapsed => self.collapse_first_category(),
			capture::Screen::DmSidebar | capture::Screen::DmGroup => {
				self.drive_home(capture.screen);
				self.until_settled(&capture, true).await;
			}
			capture::Screen::Composer | capture::Screen::ComposerReply => {
				self.drive_composer(&capture.screen);
				let started = Instant::now();
				while !self.composer_settled(&capture.screen) && started.elapsed() < capture::LIMIT {
					zgx::timer::sleep(capture::POLL).await;
				}
			}
			capture::Screen::FirstChannel | capture::Screen::LoginTos | capture::Screen::LoginToken => {}
		}
		zgx::timer::sleep(capture::SETTLE).await;
		match capture::signal_ready(&capture.ready) {
			Ok(()) => tracing::info!(screen = %capture.name, "capture screen ready"),
			Err(error) => tracing::warn!(%error, "could not signal the capture screen"),
		}
	}

	async fn until_settled(&self, capture: &Capture, home: bool) {
		let started = Instant::now();
		while !self.settled(capture.screen) || (home && !self.home_settled()) {
			if started.elapsed() >= capture::LIMIT {
				tracing::warn!(screen = %capture.name, "the capture screen did not settle in time");
				break;
			}
			zgx::timer::sleep(capture::POLL).await;
		}
	}

	fn collapse_first_category(&self) {
		let command = {
			let sidebar = self.sidebar.sidebar();
			sidebar
				.sections()
				.iter()
				.find_map(|section| section.category.as_ref())
				.and_then(|category| sidebar.toggle_command(category.id))
		};
		if let Some(command) = command {
			self.send(command);
		}
	}

	async fn reveal(self: &Rc<Self>, marker: Option<&str>) {
		if marker.is_some() {
			let showcase = self
				.sidebar
				.sidebar()
				.channels()
				.find(|channel| channel.name == mockcord::fixtures::showcase::CHANNEL_NAME)
				.map(|channel| channel.id);
			match showcase {
				Some(channel_id) => self.select_channel(channel_id),
				None => tracing::warn!("the showcase channel is missing"),
			}
		}
		let started = Instant::now();
		let target = loop {
			let found = {
				let timeline = self.timeline.borrow();
				let mut messages = timeline.messages();
				match marker {
					Some(marker) => messages
						.find(|entry| entry.content == marker)
						.map(|entry| entry.id),
					None => messages
						.rev()
						.find(|entry| entry.message.attachments.iter().any(bridge::is_image))
						.map(|entry| entry.id),
				}
			};
			if found.is_some() || started.elapsed() >= capture::LIMIT {
				break found;
			}
			zgx::timer::sleep(capture::POLL).await;
		};
		let Some(target) = target else {
			tracing::warn!("the capture target message was not found");
			return;
		};
		for _ in 0..capture::SCROLL_PASSES {
			self.timeline_view.scroll_to(&bridge::RowKey::Message(target));
			zgx::timer::sleep(capture::POLL).await;
		}
		while !self.settled(capture::Screen::FirstChannel) && started.elapsed() < capture::LIMIT {
			zgx::timer::sleep(capture::POLL).await;
		}
		self.timeline_view.scroll_to(&bridge::RowKey::Message(target));
	}

	fn settled(&self, screen: capture::Screen) -> bool {
		if screen.is_login() {
			return self.login_settled(screen);
		}
		self.opening.get().is_none()
			&& self.timeline.borrow().messages().next().is_some()
			&& self.timeline_view.settled()
			&& self.avatar_shown.get()
			&& self.rail_icons_shown()
	}

	fn rail_icons_shown(&self) -> bool {
		let shown = self.shown.borrow();
		self.rail_icons
			.borrow()
			.iter()
			.chain(self.sidebar_avatars.borrow().iter())
			.chain(self.dm_avatars.borrow().iter())
			.all(|name| shown.contains(name))
	}

	fn publish(&self, changes: &Changes) -> Option<(Opening, OpenChannel)> {
		let state = self.state()?;
		let state = state.read();
		let refresh = self.sidebar.apply(&state, changes);
		let dms = self.dms.apply(&state, changes);
		let moved = refresh.selected && self.open_guild() != self.sidebar.sidebar().selected();
		let messages = if moved {
			self.timeline.borrow_mut().close()
		} else {
			self.timeline.borrow_mut().apply(&state, changes)
		};
		if changes.current_user {
			self.request_avatar(&state);
		}
		let scale = self.scale_factor();
		self.window.runtime.batch(|| {
			let icons = Icons {
				gallery: &self.images.gallery,
				cdn: &self.images.cdn,
				scale,
			};
			if let Some(names) = self.rail.apply(&state, changes, icons) {
				*self.rail_icons.borrow_mut() = names;
				self.refresh_visible();
			}
			if changes.session || changes.current_user {
				self.status.set(status_text(&state).as_str().into());
			}
			if refresh.channels || refresh.selected || refresh.guilds {
				self.publish_selection(&state);
			}
			if messages || moved {
				self.publish_timeline(&state);
			}
			if dms {
				self.publish_dms(&state);
			}
			self.composer_apply(&state, changes);
		});
		let opening = self.opening.get()?;
		if self.sidebar.sidebar().guilds().is_empty() {
			return None;
		}
		self.opening.set(None);
		let open = match opening {
			Opening::Busiest => bridge::busiest_channel(&state),
			Opening::First => bridge::first_channel(&state),
		};
		open.map(|open| (opening, open))
	}

	fn open_guild(&self) -> Option<Snowflake> {
		self.timeline.borrow().open().and_then(|open| open.guild_id)
	}

	fn select_guild(&self, guild_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		self.rail.reveal(guild_id);
		self.leave_home_page();
		let command = {
			let state = shared.read();
			let command = self.sidebar.select(&state, guild_id);
			if command.is_some() {
				self.timeline.borrow_mut().close();
			}
			self.window.runtime.batch(|| {
				self.publish_selection(&state);
				self.publish_timeline(&state);
			});
			command
		};
		if let Some(command) = command {
			self.send(command);
		}
	}

	pub(crate) fn select_channel(&self, channel_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let command = {
			let state = shared.read();
			let sidebar = self.sidebar.sidebar();
			let openable = sidebar
				.channel(channel_id)
				.is_some_and(|channel| channel.selectable() && channel.kind.opens_timeline());
			if !openable {
				return;
			}
			let guild_id = sidebar.selected();
			drop(sidebar);
			let command = self.timeline.borrow_mut().select(&state, guild_id, channel_id);
			self.window.runtime.batch(|| self.publish_timeline(&state));
			command
		};
		if let Some(command) = command {
			self.send(command);
		}
	}

	fn request_avatar(&self, state: &State) {
		if self.avatar.borrow().is_some() {
			return;
		}
		let Some(user) = state.current_user() else {
			return;
		};
		let scale = self.scale_factor();
		let Some(request) = bridge::avatar_request(&self.images.cdn, user, AVATAR_POINTS, scale, true) else {
			return;
		};
		self.avatar_requested_at.set(Some(Instant::now()));
		let name = self.images.gallery.request(request);
		self.avatar_name.set(format!("host:{name}").as_str().into());
		tracing::info!(scale, "avatar requested");
		*self.avatar.borrow_mut() = Some(name);
		self.refresh_visible();
	}

	fn scale_factor(&self) -> f64 {
		let info = &self.window.info;
		let current = info.current_display.with_untracked(|index| *index);
		info.displays.with_untracked(|displays| {
			current
				.and_then(|index| displays.get(index))
				.or_else(|| displays.iter().find(|display| display.primary))
				.map_or(1., |display| display.scale_factor)
		})
	}

	pub(crate) fn state(&self) -> Option<state::SharedState> {
		self.flow
			.borrow()
			.session()
			.map(|session| session.state().clone())
	}

	pub(crate) fn send(&self, command: Command) {
		if let Some(session) = self.flow.borrow().session() {
			session.send(command);
		}
	}

	fn publish_selection(&self, state: &State) {
		let icons = Icons {
			gallery: &self.images.gallery,
			cdn: &self.images.cdn,
			scale: self.scale_factor(),
		};
		let names = self.sidebar.publish(state, &icons);
		if *self.sidebar_avatars.borrow() != names {
			*self.sidebar_avatars.borrow_mut() = names;
			self.refresh_visible();
		}
	}

	fn publish_timeline(&self, state: &State) {
		let timeline = self.timeline.borrow();
		let open = timeline.open();
		let id = open.map(|open| open.channel_id.to_string()).unwrap_or_default();
		self.open_channel.set(id.as_str().into());
		if self.sidebar.set_open(open.map(|open| open.channel_id)) {
			self.publish_selection(state);
		}
		let channel = open.and_then(|open| state.channel(open.channel_id));
		let name = open
			.map(|open| self.channel_title(state, open))
			.unwrap_or_default();
		self.channel_name.set(name.as_str().into());
		let topic = channel
			.and_then(|channel| channel.topic.as_deref())
			.unwrap_or_default();
		self.channel_topic.set(topic.into());
		drop(timeline);
		self.timeline_view.set_scale(self.scale_factor());
		let delta = self.timeline.borrow_mut().take_delta();
		if let Some(delta) = delta {
			let timeline = self.timeline.borrow();
			tracing::debug!(
				reset = delta.reset,
				prepended = delta.prepended,
				appended = delta.appended,
				rows = timeline.rows().len(),
				"timeline rows published"
			);
			self.timeline_view.apply(&timeline, &delta, &name);
		}
		self.composer_apply(state, &Changes::default());
	}

	fn refresh_visible(&self) {
		let mut names: Vec<String> = self.luau_visible.borrow().clone();
		names.extend(self.composer.images());
		names.extend(self.avatar.borrow().iter().map(|name| name.to_string()));
		names.extend(self.rail_icons.borrow().iter().cloned());
		names.extend(self.sidebar_avatars.borrow().iter().cloned());
		names.extend(self.dm_avatars.borrow().iter().cloned());
		names.extend(
			self.timeline_view
				.visible_images()
				.iter()
				.map(|name| name.to_string()),
		);
		self.images.gallery.set_visible(names.iter().map(String::as_str));
	}

	fn shutdown(&self) {
		tracing::debug!("window closed, stopping the session");
		self.inbox.close();
		let session = self.flow.borrow_mut().shutdown();
		if let Some(session) = session {
			session.shutdown();
		}
		tracing::debug!("session stopped");
	}
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
	match value {
		Value::Map(fields) => fields
			.iter()
			.find(|(name, _)| name.as_ref() == key)
			.map(|(_, value)| value),
		_ => None,
	}
}

fn number(value: &Value, key: &str) -> Option<f32> {
	match field(value, key)? {
		Value::Num(number) if number.is_finite() => Some(*number as f32),
		_ => None,
	}
}

fn slot(value: &Value) -> Option<(&str, Slot)> {
	let name = field(value, "name").and_then(text)?;
	Some((
		name,
		Slot {
			x: number(value, "x")?,
			y: number(value, "y")?,
			width: number(value, "w")?.max(0.),
			height: number(value, "h")?.max(0.),
		},
	))
}

fn text(value: &Value) -> Option<&str> {
	match value {
		Value::Str(text) => Some(text),
		_ => None,
	}
}

fn texts(value: &Value) -> Vec<&str> {
	match value {
		Value::List(items) => items.iter().filter_map(text).collect(),
		other => text(other).into_iter().collect(),
	}
}

fn snowflake(value: &Value) -> Option<Snowflake> {
	match value {
		Value::Str(text) => text.parse::<u64>().ok().map(Snowflake),
		_ => None,
	}
}

fn status_text(state: &State) -> String {
	match &state.session().connection {
		ConnectionStatus::Connecting => "Connecting".to_owned(),
		ConnectionStatus::Resuming => "Resuming".to_owned(),
		ConnectionStatus::Connected => match state.current_user() {
			Some(user) => format!("Signed in as {}", user.username),
			None => "Connected".to_owned(),
		},
		ConnectionStatus::Reconnecting { attempt, retry_in } => {
			format!(
				"Reconnecting in {}s, attempt {attempt}",
				retry_in.as_secs().max(1)
			)
		}
		ConnectionStatus::Disconnected(DisconnectReason::AuthenticationFailed) => {
			"Signed out: the token was rejected".to_owned()
		}
		ConnectionStatus::Disconnected(DisconnectReason::Rejected { message, .. }) => {
			format!("Disconnected: {message}")
		}
	}
}

pub(crate) fn record<const N: usize>(mut fields: [(&str, Value); N]) -> Value {
	fields.sort_by(|left, right| left.0.cmp(right.0));
	Value::Map(
		fields
			.into_iter()
			.map(|(key, value)| (Rc::from(key), value))
			.collect(),
	)
}
