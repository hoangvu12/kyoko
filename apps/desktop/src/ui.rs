mod apps;
mod cloner;
mod community;
mod component_shots;
mod composer;
mod connection;
#[cfg(test)]
mod cursors;
mod display;
mod expression_shots;
mod expressions;
mod friends;
mod hidden;
mod home;
mod inbox;
mod kept;
mod landing;
mod members;
mod message_actions;
mod notices;
mod plugin_slots;
mod plugins;
mod poll_shots;
#[cfg(test)]
mod profile_menus;
mod profiles;
mod qr_code;
mod reaction_shots;
#[cfg(test)]
mod root;
#[cfg(test)]
mod scripts;
mod search;
mod server_settings;
mod settings;
mod sign_in;
mod themes;
mod threads;
mod voice_shots;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use bridge::{ConnectionWatch, Display, Flow, Inbox, OpenChannel, Timeline};
use media::{Gallery, ImageName, Update};
use protocol::Snowflake;
use session::Alert;
use session::{Command, Formatting, Session};

pub use sign_in::SignIn;
use state::{Changes, ConnectionStatus, DisconnectReason, State};
use zgx::compose::prelude::*;
use zgx::image::ImageData;
use zgx::reactive::{Effect, Runtime, Signal};
use zgx_desktop::{Application, Titlebar, WindowHandle, WindowInfo, WindowOptions};
use zgx_script::{ScriptHost, Shared, Value};

use crate::capture::{self, Capture, Opening};
use crate::community::CommunityLayer;
use crate::dm_panel::DmPanelView;
use crate::dm_sidebar::{self, DmSidebarView};
use crate::frame::{self, Frame};
use crate::friend_dialog::FriendDialog;
use crate::friends::{self as friends_page, FriendsPage};
use crate::integration::{Desktop, Integration};
use crate::member_list::{self, MemberListView};
use crate::moderation::ModerationLayer;
use crate::motion::Motion;
use crate::profile::ProfileLayer;
use crate::rail::{self, Icons, RailView};
use crate::sidebar::{self, SidebarView};
use crate::timeline::{Prefs, Setup, TimelineView};
use crate::toasts::ToastLayer;

const UI_DIR_VARIABLE: &str = "KYOKO_UI_DIR";
const THEMES_DIR_VARIABLE: &str = "KYOKO_THEMES_DIR";
const FRAME_POLL: Duration = Duration::from_millis(50);
const FRAME_TRIES: usize = 100;
const TIMELINE_SLOT: &str = "timeline";
const WEB_LOGIN_SLOT: &str = "web_login";
const COMPOSER_FLOAT: &str = "composer_float";
const RELOAD_EVERY: Duration = Duration::from_millis(250);
const AVATAR_POINTS: f64 = 32.;
const WIDTH: f64 = 1100.;
const HEIGHT: f64 = 720.;

type Observer = Box<dyn Fn(&Changes)>;

pub struct Images {
	pub gallery: Gallery,
	pub cdn: String,
	pub mock_origin: Option<String>,
	pub budget: usize,
	pub prefs: Prefs,
}

struct Messages {
	developer_mode: bool,
	code_font: String,
	store: Option<Arc<store::Store>>,
}

pub struct Appearance {
	pub theme: String,
}

struct Views {
	settings: settings::SettingsView,
	plugins: plugins::PluginsView,
	themes: themes::ThemesView,
	display: Display,
	display_wakes: Inbox<Formatting>,
}

pub struct Launch {
	pub opening: Option<Opening>,
	pub capture: Option<Capture>,
	pub plugins: ::plugins::PluginHost,
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
	let Launch {
		opening,
		capture,
		plugins: plugin_host,
	} = launch;
	let script = ui_dir().join("app.luau");
	let (width, height) = match capture {
		Some(_) => (capture::WIDTH, capture::HEIGHT),
		None => (WIDTH, HEIGHT),
	};
	let mut application = crate::fonts::bundled()
		.into_iter()
		.fold(Application::new(), Application::font);
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
			let (themes, catalog) = themes::ThemesView::startup(&desktop.store, &[themes_dir()]);
			let theme = catalog.install(&cx.ui.runtime, &appearance.theme);
			tracing::info!(%theme, themes = catalog.palettes.len(), "themes installed");
			let host = ScriptHost::new(&cx.ui.runtime, script);
			let window = Window {
				runtime: cx.ui.runtime.clone(),
				info: cx.window_info.clone(),
				handle: cx.window.clone(),
				frame: Cell::new(None),
				slot: cx.ui.runtime.signal(Slot::default()),
				float: cx.ui.runtime.signal(Slot::default()),
			};
			let messages = Messages {
				developer_mode: capture.is_none() && desktop.store.settings.read().developer.developer_mode,
				code_font: catalog.code_font(&theme),
				store: capture.is_none().then(|| desktop.store.clone()),
			};
			let settings = settings::SettingsView::new(&host, &cx.ui.runtime, desktop.store.clone(), catalog);
			let plugins = plugins::PluginsView::new(&host, plugin_host.clone(), sign_in.runtime.clone());
			let themes = themes::ThemesView::new(&host, desktop.store.clone(), themes);
			let (display, display_wakes) = display::start(&plugin_host);
			let link = Link::install(
				&host,
				window,
				sign_in,
				inbox.clone(),
				images,
				messages,
				Views {
					settings,
					plugins,
					themes,
					display,
					display_wakes,
				},
			);
			link.opening.set(opening);
			link.attach_ui(cx.ui.downgrade());
			let drain = link.clone();
			cx.tasks.spawn(async move { drain.drain().await });
			cx.tasks.spawn(link.clone().follow_sign_in());
			cx.tasks
				.spawn(link.clone().follow_composer_files(composer::Pane::Main));
			cx.tasks
				.spawn(link.clone().follow_composer_files(composer::Pane::Thread));
			cx.tasks.spawn(link.clone().follow_post_files());
			cx.tasks.spawn(link.clone().tick_composer());
			cx.tasks.spawn(link.clone().follow_plugins());
			cx.tasks.spawn(link.clone().follow_plugin_requests());
			cx.tasks.spawn(link.clone().follow_clones());
			cx.tasks.spawn(link.clone().follow_theme_folder());
			cx.tasks.spawn(link.clone().follow_display());
			cx.tasks.spawn(link.clone().follow_gif_search());
			cx.tasks.spawn(link.clone().follow_server_settings());
			cx.tasks.spawn(link.clone().follow_connection());
			if let Some(capture) = capture.clone() {
				cx.tasks.spawn(link.clone().signal_when_settled(capture));
			}
			let pictures = link.clone();
			cx.tasks.spawn(async move { pictures.show_images().await });
			cx.tasks.spawn(link.clone().flush_members());
			cx.tasks.spawn(link.clone().attach_frame());
			if let (None, Some(half)) = (&capture, platform::caret::blink_time()) {
				cx.tasks.spawn(crate::caret::run(
					cx.ui.downgrade(),
					crate::caret::Blink::new(half),
				));
			}
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
			let float = link.window.float.clone();
			let transcript = link.timeline_view.view();
			let forum_shown = link.threads.forum_shown();
			let hidden_shown = link.hidden.shown();
			let thread_view = threads::thread_slot(slot.clone(), link.threads.slot(), link.threads.view());
			let thread_layer = link.threads.layer_view();
			let profiles = link.profile.view();
			let lightbox = link.timeline_view.lightbox_view();
			let expressions = link.expressions.view();
			let moderation = link.moderation.view();
			let escaped = Rc::downgrade(&link);
			let switcher = link.finder.view();
			let community = link.community.view();
			let toasts = link.toasts.view();
			let friend_dialog = link.friend_dialog.view();
			let inbox_layer = link.inbox_popout.view();
			let scrim = link.scrim.view();
			let composer_layer = link.composer_layer.view();
			let apps = link.apps.view();
			let motion = link.motion.clone();
			let dropped = Rc::downgrade(&link);
			let hovered = Rc::downgrade(&link);
			let keys = Rc::downgrade(&link);
			let pressed = Rc::downgrade(&link);
			cx.on_closed(move || {
				frame::wake_until_loop_ends();
				if let Some(integration) = integration {
					integration.shutdown();
				}
				link.shutdown();
				after_close();
			});
			cx.render(provide(
				motion,
				layered(
					host.view(),
					[
						timeline_slot(slot, transcript, [forum_shown, hidden_shown], float),
						thread_view,
						scrim,
						profiles,
						switcher,
						thread_layer,
						community,
						expressions,
						lightbox,
						moderation,
						toasts,
						composer_layer,
						friend_dialog,
						apps,
						inbox_layer,
					],
				)
				.on_event(move |event| {
					if event.phase == zgx::input::EventPhase::Capture {
						if matches!(
							event.event,
							zgx::input::InputEvent::PointerDown { .. }
								| zgx::input::InputEvent::KeyDown { .. }
						) && let Some(link) = pressed.upgrade()
						{
							link.plugin_input();
						}
						return;
					}
					if let zgx::input::InputEvent::KeyDown {
						key: zgx::input::Key::Escape,
						..
					} = event.event && escaped.upgrade().is_some_and(|link| link.escape_overlay())
					{
						event.stop_propagation();
						return;
					}
					if is_switcher_key(&event.event) {
						if let Some(link) = keys.upgrade() {
							link.open_switcher();
						}
						event.prevent_default();
						event.stop_propagation();
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
			));
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

fn layered(luau: View, layers: impl IntoIterator<Item = View>) -> View {
	layers
		.into_iter()
		.fold(overlay().w_full().h_full().child(luau), |root, layer| {
			root.child(div().absolute().left(0.).top(0.).w(0.).h(0.).child(layer))
		})
}

fn timeline_slot(
	slot: Signal<Slot>,
	transcript: View,
	covers: [Signal<bool>; 2],
	float: Signal<Slot>,
) -> View {
	let shown = move || {
		let covered = covers.iter().any(Signal::get);
		if covered { Slot::default() } else { slot.get() }
	};
	let inner = shown.clone();
	let full = div().child(transcript).reactive_style(move || {
		let slot = inner();
		Styles::new().w(slot.width).h(slot.height)
	});
	div().overflow_hidden().child(full).reactive_style(move || {
		let slot = shown();
		Styles::new()
			.absolute()
			.left(slot.x)
			.top(slot.y)
			.w(slot.width)
			.h(visible_height(slot, float.get()))
	})
}

fn visible_height(slot: Slot, float: Slot) -> f32 {
	if float.height <= 0. {
		return slot.height;
	}
	(float.y - slot.y).clamp(0., slot.height)
}

fn is_switcher_key(event: &zgx::input::InputEvent) -> bool {
	match event {
		zgx::input::InputEvent::KeyDown {
			key: zgx::input::Key::Character(character),
			modifiers,
			repeat: false,
		} => modifiers.control && !modifiers.alt && character.eq_ignore_ascii_case("k"),
		_ => false,
	}
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
	float: Signal<Slot>,
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
	friends: FriendsPage,
	friend_dialog: Rc<FriendDialog>,
	dm_panel: DmPanelView,
	members: MemberListView,
	member_avatars: RefCell<Vec<String>>,
	reported_slot: Cell<Slot>,
	timeline: RefCell<Timeline>,
	timeline_view: Rc<TimelineView>,
	profile: Rc<ProfileLayer>,
	community: Rc<CommunityLayer>,
	cloner: cloner::Cloner,
	toasts: Rc<ToastLayer>,
	scrim: Rc<crate::scrim::Scrim>,
	composer_layer: Rc<crate::composer_layer::ComposerLayer>,
	toast_window: RefCell<Option<Effect>>,
	motion: Motion,
	connection: RefCell<ConnectionWatch>,
	connection_pinned: Cell<bool>,
	connection_banner: Shared,
	connection_wakes: Inbox<()>,
	joins: community::Joins,
	profile_effect: RefCell<Option<Effect>>,
	panel: Shared,
	panel_shown: RefCell<Option<[String; 6]>>,
	panel_emoji: RefCell<Option<String>>,
	pending_dm: Cell<Option<Snowflake>>,
	composer: composer::ComposerView,
	expressions: expressions::Expressions,
	apps: apps::Apps,
	threads: threads::ThreadPane,
	settings: settings::SettingsView,
	finder: search::Finder,
	plugins: plugins::PluginsView,
	themes: themes::ThemesView,
	display: Display,
	display_wakes: Inbox<Formatting>,
	moderation: Rc<ModerationLayer>,
	moderation_effect: RefCell<Option<Effect>>,
	hidden: hidden::HiddenPane,
	server: server_settings::ServerSettingsView,
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
	ui: RefCell<Option<zgx::widgets::WeakUi>>,
	inbox_popout: Rc<crate::inbox::InboxLayer>,
}

impl Link {
	fn install(
		host: &ScriptHost,
		window: Window,
		sign_in: SignIn,
		inbox: Inbox<Changes>,
		images: Images,
		messages: Messages,
		views: Views,
	) -> Rc<Link> {
		let Views {
			settings,
			plugins,
			themes,
			display,
			display_wakes,
		} = views;
		let image_wakes = Inbox::new();
		let wakes = image_wakes.clone();
		images.gallery.set_waker(move || wakes.push(()));
		let timeline_view = TimelineView::new(
			&window.runtime,
			Setup {
				gallery: images.gallery.clone(),
				cdn: images.cdn.clone(),
				mock_origin: images.mock_origin.clone(),
				code_font: messages.code_font.clone(),
				prefs: images.prefs,
				store: messages.store.clone(),
				developer_mode: messages.developer_mode,
				pictures: None,
				display: display.clone(),
			},
		);
		let thread_pane = threads::ThreadPane::new(
			host,
			&window.runtime,
			sign_in.runtime.clone(),
			Setup {
				gallery: images.gallery.clone(),
				cdn: images.cdn.clone(),
				mock_origin: images.mock_origin.clone(),
				code_font: messages.code_font,
				prefs: images.prefs,
				store: messages.store,
				developer_mode: messages.developer_mode,
				pictures: Some(timeline_view.pictures()),
				display: display.clone(),
			},
		);
		let composer = composer::ComposerView::install(host, sign_in.runtime.clone(), &composer::MAIN);
		let profile = ProfileLayer::new(&window.runtime, timeline_view.pictures());
		let community = CommunityLayer::new(&window.runtime, timeline_view.pictures());
		let finder = search::Finder::new(host, &window.runtime, timeline_view.pictures());
		let expressions = expressions::Expressions::new(&window.runtime, timeline_view.pictures());
		let apps = apps::Apps::new(&window.runtime, timeline_view.pictures());
		let moderation = ModerationLayer::new(&window.runtime);
		let hidden = hidden::HiddenPane::new(host, &window.runtime);
		let mut timeline = Timeline::default();
		timeline.set_display(display.clone());
		let members = MemberListView::new(host);
		members.set_display(display.clone());
		let toasts = ToastLayer::new(&window.runtime);
		let scrim = crate::scrim::Scrim::new(&window.runtime);
		let composer_layer =
			crate::composer_layer::ComposerLayer::new(&window.runtime, timeline_view.pictures());
		let friend_dialog = FriendDialog::new(&window.runtime);
		let motion = Motion::new(window.runtime.signal(false));
		let server = server_settings::ServerSettingsView::new(host, &window.runtime);
		let inbox_popout = crate::inbox::InboxLayer::new(&window.runtime, timeline_view.pictures());
		let link = Rc::new(Link {
			window,
			host: host.clone(),
			flow: RefCell::new(sign_in.flow),
			account: sign_in::Account::install(
				host,
				sign_in.runtime,
				sign_in.storage_warnings,
				sign_in.qr,
				sign_in.web,
			),
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
			friends: FriendsPage::new(host),
			friend_dialog,
			dm_panel: DmPanelView::new(host),
			members,
			member_avatars: RefCell::new(Vec::new()),
			reported_slot: Cell::new(Slot::default()),
			timeline: RefCell::new(timeline),
			timeline_view,
			profile,
			community,
			cloner: cloner::Cloner::new(),
			toasts,
			toast_window: RefCell::new(None),
			scrim,
			composer_layer,
			motion,
			connection: RefCell::new(ConnectionWatch::default()),
			connection_pinned: Cell::new(false),
			connection_banner: host.shared("session.connection", connection::empty_banner()),
			connection_wakes: Inbox::new(),
			joins: community::Joins::default(),
			profile_effect: RefCell::new(None),
			panel: host.shared("user.panel", profiles::empty_panel()),
			panel_shown: RefCell::new(None),
			panel_emoji: RefCell::new(None),
			pending_dm: Cell::new(None),
			composer,
			expressions,
			apps,
			threads: thread_pane,
			settings,
			finder,
			plugins,
			themes,
			display,
			display_wakes,
			moderation,
			moderation_effect: RefCell::new(None),
			hidden,
			server,
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
			ui: RefCell::new(None),
			inbox_popout,
		});
		Link::install_window_actions(host, &link);
		rail::install_actions(host, &link);
		Link::install_sign_in(host, &link);
		sidebar::install_actions(host, &link);
		dm_sidebar::install_actions(host, &link);
		friends_page::install_actions(host, &link);
		Link::install_friend_dialog(&link);
		Link::install_composer(host, &link);
		Link::install_expressions(host, &link);
		Link::install_apps(host, &link);
		Link::install_message_actions(&link);
		member_list::install_actions(host, &link);
		Link::install_server_settings(host, &link);
		Link::install_settings(host, &link);
		Link::install_plugins(host, &link);
		Link::install_cloner(&link);
		Link::install_themes(host, &link);
		Link::install_profiles(host, &link);
		Link::install_search(host, &link);
		Link::install_threads(host, &link);
		Link::install_community(host, &link);
		Link::install_connection(host, &link);
		link.install_toasts();
		Link::install_inbox(host, &link);
		let actions = Rc::downgrade(&link);
		host.action("rail.select_guild", move |argument| {
			if let (Some(link), Some(guild_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.visit_guild(guild_id);
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
		let actions = Rc::downgrade(&link);
		host.function("media.reduced_motion", move |system| {
			if let Some(link) = actions.upgrade() {
				link.system_reduced_motion(matches!(system, Value::Bool(true)));
			}
			Value::Nil
		});
		link.system_reduced_motion(false);
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
			if let (Some(link), Some((name, slot))) = (actions.upgrade(), slot(&argument)) {
				match name {
					TIMELINE_SLOT => link.place_slot(slot),
					WEB_LOGIN_SLOT => link.place_web(slot),
					threads::THREAD_SLOT => link.place_thread_slot(slot),
					expressions::COMPOSER_SLOT => link.place_composer(slot),
					COMPOSER_FLOAT => {
						link.window.float.set(slot);
					}
					_ => {}
				}
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
				self.login_image_shown(&name);
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

	pub(crate) fn members(&self) -> &MemberListView {
		&self.members
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
		let shown = self.top_channel(self.timeline.borrow().open());
		let before = self.sidebar.sidebar().selected();
		let opened = self.publish(changes);
		let launched = opened.is_some();
		self.replace_removed(shown);
		self.community_apply(changes);
		if let Some(shared) = self.state() {
			self.cloner_apply(&shared.read(), changes);
		}
		self.themes_apply(changes);
		self.follow_friend(changes);
		self.follow_threads();
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
		if !launched {
			self.follow_selection(before);
		}
	}

	async fn signal_when_settled(self: Rc<Self>, capture: Capture) {
		self.drive_capture(capture.screen);
		self.enable_capture_plugins(&capture.plugins).await;
		self.enable_capture_themes(&capture.themes);
		self.until_settled(&capture, false).await;
		match &capture.screen {
			capture::Screen::RailFolders => self.rail.open_all(),
			capture::Screen::NewestImage => self.reveal(None).await,
			capture::Screen::Showcase(section) => {
				self.reveal(Some(&mockcord::fixtures::showcase::marker(section)))
					.await
			}
			capture::Screen::SidebarCollapsed => self.collapse_first_category(),
			capture::Screen::HiddenChannel => self.drive_hidden_channel().await,
			capture::Screen::GameActivity => self.drive_game_activity().await,
			capture::Screen::MemberList
			| capture::Screen::MemberListScrolled
			| capture::Screen::MemberNotes => self.until_members(&capture).await,
			capture::Screen::DmSidebar | capture::Screen::DmGroup => {
				self.drive_home(capture.screen);
				self.until_settled(&capture, true).await;
			}
			capture::Screen::Friends(_) => {
				self.select_home();
				self.until_settled(&capture, true).await;
				self.drive_friends(capture.screen);
				self.until_capture(|link| link.friends_settled()).await;
			}
			capture::Screen::FriendConfirm(shot) => {
				self.select_home();
				self.until_settled(&capture, true).await;
				self.drive_friends(capture::Screen::Friends(bridge::FriendsTab::Online));
				self.until_capture(|link| link.friends_settled()).await;
				self.drive_friend_confirm(*shot).await;
			}
			capture::Screen::Composer | capture::Screen::ComposerReply => {
				self.drive_composer(&capture.screen);
				let started = Instant::now();
				while !self.composer_settled(&capture.screen) && started.elapsed() < capture::LIMIT {
					zgx::timer::sleep(capture::POLL).await;
				}
			}
			capture::Screen::SilentTyping => self.drive_silent_typing().await,
			capture::Screen::TextReplaceSettings => self.drive_text_replace().await,
			capture::Screen::MessageHover => self.point_at_last_message(false).await,
			capture::Screen::MessageMenu => self.point_at_last_message(true).await,
			capture::Screen::OwnMessage(shot) => self.drive_own_message(*shot).await,
			capture::Screen::CloneMenu => self.drive_clone_menu().await,
			capture::Screen::KeptDeleted | capture::Screen::KeptEdits => {
				self.drive_kept(capture.screen).await
			}
			capture::Screen::Expressions(shot) => self.drive_expression_shot(*shot).await,
			capture::Screen::LinkPreview => self.drive_link_preview().await,
			capture::Screen::VoiceMessage => self.drive_voice_message().await,
			capture::Screen::Components(shot) => self.drive_component_shot(*shot).await,
			capture::Screen::Polls(shot) => self.drive_polls(*shot).await,
			capture::Screen::AppsLauncher => self.drive_apps().await,
			capture::Screen::Reactions(shot) => self.drive_reaction_shot(*shot).await,
			capture::Screen::ComposerShot(shot) => self.drive_composer_shot(*shot).await,
			capture::Screen::CloneDialog => {
				self.drive_clone_dialog().await;
				self.until_capture(|link| link.community_settled()).await;
			}
			capture::Screen::Settings(_) => {
				self.drive_settings(capture.screen);
				let started = Instant::now();
				while !self.settings_settled() && started.elapsed() < capture::LIMIT {
					zgx::timer::sleep(capture::POLL).await;
				}
			}
			capture::Screen::StatusMenu
			| capture::Screen::AccountSwitcher
			| capture::Screen::CustomStatus
			| capture::Screen::ProfilePopout
			| capture::Screen::FullProfile
			| capture::Screen::ServerProfile
			| capture::Screen::ProfileMenu => self.drive_profiles(capture.screen).await,
			capture::Screen::UserPanel => {}
			capture::Screen::QuickSwitcher => {
				self.open_switcher();
				self.type_switcher(capture::SWITCHER_QUERY);
				self.until_capture(|link| link.switcher_settled()).await;
			}
			capture::Screen::Community(screen) => {
				match screen {
					capture::Community::AddServer => self.drive_add_server(),
					capture::Community::Forward => self.drive_forward(),
					capture::Community::GuildNotifications => self.drive_guild_notifications(),
				}
				self.until_capture(|link| link.community_settled()).await;
			}
			capture::Screen::Search => {
				self.drive_search(capture::SEARCH_QUERY);
				self.until_capture(|link| link.search_settled()).await;
			}
			capture::Screen::Lightbox => {
				self.drive_lightbox();
				self.until_capture(|link| link.timeline_view.lightbox_settled())
					.await;
			}
			capture::Screen::Picker(tab) => {
				let tab = *tab;
				self.drive_expressions(tab);
				self.until_capture(|link| link.expressions_settled(tab)).await;
				if tab != crate::pickers::Tab::Gif {
					self.hover_first_expression();
				}
			}
			capture::Screen::PickerShot(shot) => self.drive_picker_shot(*shot).await,
			capture::Screen::ThreadsList
			| capture::Screen::ThreadPanel
			| capture::Screen::ThreadCreate
			| capture::Screen::Pins
			| capture::Screen::Forum
			| capture::Screen::ForumPost
			| capture::Screen::ForumPostCreate => self.drive_threads(capture.screen).await,
			capture::Screen::ServerSettings(page) => {
				self.open_server_settings(bridge::ServerPage::named(page));
				self.until_capture(|link| link.server_settled()).await;
			}
			capture::Screen::Moderation("roles-submenu") => self.drive_roles_submenu(&capture).await,
			capture::Screen::Moderation(kind) => {
				if let Some(author) = self.newest_other_author() {
					self.open_moderation_dialog(author, kind);
				}
			}
			capture::Screen::Toast => {
				self.drive_toast();
				self.until_capture(|link| link.toast_settled()).await;
			}
			capture::Screen::Disconnected => self.drive_disconnected(),
			capture::Screen::Inbox => self.drive_inbox().await,
			capture::Screen::FirstChannel
			| capture::Screen::LoginTos
			| capture::Screen::LoginToken
			| capture::Screen::LoginQrScanned
			| capture::Screen::LoginQrExpired
			| capture::Screen::LoginWeb
			| capture::Screen::LoginEmail
			| capture::Screen::Login2fa
			| capture::Screen::LoginCaptcha
			| capture::Screen::LoginUnsaved => {}
		}
		self.until_capture(|link| link.display.pending() == 0).await;
		zgx::timer::sleep(capture::SETTLE).await;
		match capture::signal_ready(&capture.ready) {
			Ok(()) => tracing::info!(screen = %capture.name, "capture screen ready"),
			Err(error) => tracing::warn!(%error, "could not signal the capture screen"),
		}
	}

	pub(super) async fn until_capture(&self, ready: impl Fn(&Self) -> bool) {
		let started = Instant::now();
		while !ready(self) && started.elapsed() < capture::LIMIT {
			zgx::timer::sleep(capture::POLL).await;
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

	fn drive_lightbox(&self) {
		let target = {
			let timeline = self.timeline.borrow();
			timeline.messages().rev().find_map(|entry| {
				entry.message.attachments.iter().find_map(|attachment| {
					bridge::LightboxImage::attachment(attachment, entry.message.channel_id, entry.id)
				})
			})
		};
		match target {
			Some(image) => self.timeline_view.open_lightbox(image),
			None => tracing::warn!("no image to open in the lightbox"),
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
			.chain(self.member_avatars.borrow().iter())
			.all(|name| shown.contains(name))
	}

	fn publish(&self, changes: &Changes) -> Option<(Opening, OpenChannel)> {
		let state = self.state()?;
		let state = state.read();
		let refresh = self.sidebar.apply(&state, changes);
		let dms = self.dms.apply(&state, changes);
		self.members.apply(&state, changes);
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
			self.friends_apply(&state, changes);
			self.dm_panel_apply(&state, changes);
			self.composer_apply(&state, changes);
			self.expressions_apply(&state, changes);
			self.apps_apply(&state, changes);
			self.publish_actions(&state, changes);
			self.settings_apply(&state, changes);
			self.publish_profiles(&state, changes);
			self.search_apply(&state, changes);
			self.threads_apply(&state, changes);
			self.server_settings_apply(&state, changes);
			self.connection_apply(&state, changes);
			self.toasts_apply(&state, changes);
			self.inbox_apply(&state, changes);
			self.plugins_apply(&state);
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

	fn replace_removed(&self, shown: Option<OpenChannel>) {
		let replacement = shown.zip(self.state()).and_then(|(shown, shared)| {
			let state = shared.read();
			bridge::replacement_channel(&state, shown)
		});
		if let Some(replacement) = replacement {
			tracing::info!("the open channel was deleted, opening the server's first channel");
			self.select_channel(replacement.channel_id);
		}
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
		self.offer_gate(guild_id);
	}

	pub(crate) fn select_channel(&self, channel_id: Snowflake) {
		if self.route_channel(channel_id) {
			return;
		}
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

	pub(crate) fn session_generation(&self) -> u64 {
		self.flow.borrow().generation()
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
		self.sync_hidden(state, false);
	}

	fn publish_timeline(&self, state: &State) {
		let timeline = self.timeline.borrow();
		self.sync_threads(timeline.open());
		let open = self.top_channel(timeline.open());
		self.sync_hidden(state, open.is_some() || self.forum_open().is_some());
		self.publish_channel_icon(state, open);
		self.remember_open(open);
		let id = open.map(|open| open.channel_id.to_string()).unwrap_or_default();
		self.open_channel.set(id.as_str().into());
		self.show_members(open);
		self.sync_dm_panel(state, open);
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
		names.extend(self.expressions.images());
		names.extend(self.apps.images());
		names.extend(self.avatar.borrow().iter().map(|name| name.to_string()));
		names.extend(self.rail_icons.borrow().iter().cloned());
		names.extend(self.sidebar_avatars.borrow().iter().cloned());
		names.extend(self.dm_avatars.borrow().iter().cloned());
		names.extend(self.member_avatars.borrow().iter().cloned());
		names.extend(self.members.hovered_avatar());
		names.extend(self.settings_images());
		names.extend(self.profile_images());
		names.extend(self.search_images());
		names.extend(self.thread_images());
		names.extend(self.community_images());
		names.extend(self.server_images());
		names.extend(self.friends_images());
		names.extend(self.dm_panel.images());
		names.extend(self.composer_layer.images().iter().map(|name| name.to_string()));
		names.extend(self.login_images());
		names.extend(self.inbox_images());
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
		self.close_web();
		self.inbox.close();
		self.connection_wakes.close();
		let session = self.flow.borrow_mut().shutdown();
		if let Some(session) = session {
			session.shutdown();
		}
		self.plugins.host().detach();
		tracing::debug!("session stopped");
	}
}

pub(crate) fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
	match value {
		Value::Map(fields) => fields
			.iter()
			.find(|(name, _)| name.as_ref() == key)
			.map(|(_, value)| value),
		_ => None,
	}
}

pub(crate) fn number(value: &Value, key: &str) -> Option<f32> {
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
