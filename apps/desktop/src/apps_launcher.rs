use std::cell::RefCell;
use std::rc::Rc;

use bridge::{AppSearch, AppsLauncher, CommandEntry, LauncherApp};
use media::ImageName;
use protocol::Snowflake;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::input::{InputEvent, Key};
use zgx::reactive::{Effect, Runtime, Signal};
use zgx::style::ObjectFit;

use crate::motion;
use crate::timeline::{Pictures, icon, tok};

pub const WIDTH: f32 = 498.;
pub const FALLBACK_HEIGHT: f32 = 486.;
pub const COMPOSER_GAP: f32 = 8.;
const BORDER: f32 = 1.;
const PADDING: f32 = 16.;
const SECTION_GAP: f32 = 16.;
const HEADER_GAP: f32 = 8.;
const HEADER_LINE: f32 = 18.;
const SEARCH_HEIGHT: f32 = 38.;
const SEARCH_ICON: f32 = 16.;
const TILE: f32 = 48.;
const TILE_GAP: f32 = 12.;
const ROW: f32 = 56.;
const ROW_ICON: f32 = 32.;
const LIST_INSET: f32 = 4.;
const GROUP_LABEL: f32 = 28.;
const BACK_BUTTON: f32 = 32.;
const BACK_ICON: f32 = 20.;
const LETTER_SHARE: f32 = 0.45;
const PLACEHOLDER: &str = "Search Apps & Commands";
const RECENTS: &str = "Recents";
const SERVER_APPS: &str = "Apps in this Server";
const APPS: &str = "Apps";
const COMMANDS: &str = "Commands";
const VIEW_MORE: &str = "View More";
const NO_APPS: &str = "No apps here yet.";
const HUES: [&str; 8] = [
	"accent_3", "green_3", "yellow_3", "blue_3", "red_3", "purple_3", "green_2", "blue_2",
];

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
	App(Snowflake),
	Command(CommandEntry),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
	Home,
	Recents,
	Apps,
}

#[derive(Clone, PartialEq)]
enum Body {
	Home(Rc<AppsLauncher>, bool),
	More(Page, Rc<AppsLauncher>, bool),
	Results(Rc<AppSearch>),
}

pub struct AppsLauncherView {
	runtime: Runtime,
	pictures: Rc<Pictures>,
	open: Signal<bool>,
	anchor: Signal<(f32, f32)>,
	height: Signal<f32>,
	query: Signal<String>,
	page: Signal<Page>,
	in_server: Signal<bool>,
	model: Signal<Rc<AppsLauncher>>,
	search: Signal<Rc<AppSearch>>,
	images: RefCell<Vec<ImageName>>,
	pick: Hook<Pick>,
	searched: Hook<String>,
	images_changed: Hook<()>,
	watchers: RefCell<Vec<Effect>>,
}

impl AppsLauncherView {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Rc<Self> {
		let view = Rc::new(Self {
			runtime: runtime.clone(),
			pictures,
			open: runtime.signal(false),
			anchor: runtime.signal((0., 0.)),
			height: runtime.signal(FALLBACK_HEIGHT),
			query: runtime.signal(String::new()),
			page: runtime.signal(Page::Home),
			in_server: runtime.signal(true),
			model: runtime.signal(Rc::new(AppsLauncher::default())),
			search: runtime.signal(Rc::new(AppSearch::default())),
			images: RefCell::default(),
			pick: RefCell::new(None),
			searched: RefCell::new(None),
			images_changed: RefCell::new(None),
			watchers: RefCell::default(),
		});
		let weak = Rc::downgrade(&view);
		let (query, open) = (view.query.clone(), view.open.clone());
		let typed = runtime.effect(move || {
			let text = query.get();
			if !open.get() {
				return;
			}
			if let Some(view) = weak.upgrade() {
				view.runtime.untracked(|| {
					if let Some(searched) = view.searched.borrow().as_ref() {
						searched(text);
					}
				});
			}
		});
		let weak = Rc::downgrade(&view);
		let open = view.open.clone();
		let closed = runtime.effect(move || {
			if !open.get()
				&& let Some(view) = weak.upgrade()
			{
				view.runtime.untracked(|| view.set_images(Vec::new()));
			}
		});
		*view.watchers.borrow_mut() = vec![typed, closed];
		view
	}

	pub fn on_pick(&self, pick: impl Fn(Pick) + 'static) {
		*self.pick.borrow_mut() = Some(Box::new(pick));
	}

	pub fn on_search(&self, searched: impl Fn(String) + 'static) {
		*self.searched.borrow_mut() = Some(Box::new(searched));
	}

	pub fn on_images_changed(&self, changed: impl Fn(()) + 'static) {
		*self.images_changed.borrow_mut() = Some(Box::new(changed));
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn query(&self) -> String {
		self.query.with_untracked(Clone::clone)
	}

	pub fn open(&self, at: (f32, f32), height: f32, model: AppsLauncher, in_server: bool) {
		self.runtime.batch(|| {
			self.anchor.set(at);
			self.height.set(height);
			self.page.set(Page::Home);
			self.in_server.set(in_server);
			self.query.set(String::new());
			self.search.set(Rc::new(AppSearch::default()));
			self.model.set(Rc::new(model));
			self.open.set(true);
		});
		self.refresh_images();
	}

	pub fn close(&self) {
		self.open.set(false);
	}

	pub fn set_model(&self, model: AppsLauncher) {
		if self.model.with_untracked(|shown| **shown != model) {
			self.model.set(Rc::new(model));
			self.refresh_images();
		}
	}

	pub fn set_search(&self, search: AppSearch) {
		if self.search.with_untracked(|shown| **shown != search) {
			self.search.set(Rc::new(search));
			self.refresh_images();
		}
	}

	pub fn images(&self) -> Vec<ImageName> {
		if self.is_open() {
			self.images.borrow().clone()
		} else {
			Vec::new()
		}
	}

	pub fn settled(&self) -> bool {
		self.is_open()
			&& self
				.images
				.borrow()
				.iter()
				.all(|name| self.pictures.is_settled(name))
	}

	fn refresh_images(&self) {
		let mut names = Vec::new();
		let model = self.model.with_untracked(Clone::clone);
		let search = self.search.with_untracked(Clone::clone);
		for app in model.recents.iter().chain(&model.apps).chain(&search.apps) {
			names.extend(self.icon_image(app, TILE));
		}
		names.sort_by_key(ToString::to_string);
		names.dedup();
		self.set_images(names);
	}

	fn set_images(&self, names: Vec<ImageName>) {
		if *self.images.borrow() == names {
			return;
		}
		*self.images.borrow_mut() = names;
		if let Some(changed) = self.images_changed.borrow().as_ref() {
			changed(());
		}
	}

	fn icon_image(&self, app: &LauncherApp, side: f32) -> Option<ImageName> {
		let (id, hash) = (app.id()?, app.app.icon.as_deref()?);
		Some(self.pictures.request(bridge::app_tile_request(
			self.pictures.cdn(),
			id,
			hash,
			f64::from(side),
			self.pictures.scale(),
		)))
	}

	fn choose(&self, pick: Pick) {
		self.close();
		if let Some(handler) = self.pick.borrow().as_ref() {
			handler(pick);
		}
	}

	fn choose_first(&self) -> bool {
		let search = self.search.with_untracked(Clone::clone);
		let first = search
			.apps
			.first()
			.and_then(LauncherApp::id)
			.map(Pick::App)
			.or_else(|| search.commands.first().cloned().map(Pick::Command));
		match first {
			Some(pick) => {
				self.choose(pick);
				true
			}
			None => false,
		}
	}

	pub fn view(self: &Rc<Self>) -> View {
		let anchor = self.anchor.clone();
		let spot = div().w(1.).h(1.).reactive_style(move || {
			let (x, y) = anchor.get();
			Styles::new().absolute().left(x - 1.).top(y)
		});
		let height = self.height.clone();
		let this = self.clone();
		let (query, page, model, search, in_server) = (
			self.query.clone(),
			self.page.clone(),
			self.model.clone(),
			self.search.clone(),
			self.in_server.clone(),
		);
		let body = switch(
			move || {
				let in_server = in_server.get();
				if query.with(|query| !query.trim().is_empty()) {
					return Body::Results(search.get());
				}
				match page.get() {
					Page::Home => Body::Home(model.get(), in_server),
					more => Body::More(more, model.get(), in_server),
				}
			},
			move |body: Body, _| this.body(body),
		);
		let inner = WIDTH - 2. * BORDER;
		div().absolute().left(0.).top(0.).w(0.).h(0.).child(
			motion::popover("Apps", self.open.clone(), spot)
				.placement(Placement {
					side: PlacementSide::Top,
					align: PlacementEdge::End,
					offset: COMPOSER_GAP,
				})
				.w(WIDTH)
				.p(0.)
				.gap(0.)
				.rounded(tok("radius_md"))
				.border(BORDER)
				.border_color(tok("border"))
				.bg(tok("bg_3"))
				.text_color(tok("text_3"))
				.overflow_hidden()
				.child(
					column()
						.id("apps-launcher")
						.w(inner)
						.p(PADDING)
						.gap(SECTION_GAP)
						.reactive_style(move || Styles::new().h(height.get() - 2. * BORDER))
						.child(self.search_field())
						.child(body.w_full().grow().min_h(0.)),
				),
		)
	}

	fn search_field(self: &Rc<Self>) -> View {
		let keys = self.clone();
		div()
			.relative()
			.w_full()
			.h(SEARCH_HEIGHT)
			.shrink_0()
			.child(
				text_input(PLACEHOLDER, self.query.clone())
					.id("apps-search")
					.placeholder(PLACEHOLDER)
					.autofocus(true)
					.on_event(move |event| {
						if matches!(event.event, InputEvent::KeyDown { key: Key::Enter, .. })
							&& keys.choose_first()
						{
							event.prevent_default();
						}
					})
					.w_full()
					.h(SEARCH_HEIGHT)
					.pl(12.)
					.pr(12. + SEARCH_ICON + 8.)
					.rounded(tok("radius_sm"))
					.border(BORDER)
					.border_color(tok("border"))
					.focus(|style| style.border_color(tok("accent_2")))
					.text_size(tok("font_size_md"))
					.text_color(tok("text_2"))
					.bg(tok("bg_4")),
			)
			.child(
				div()
					.absolute()
					.right(12.)
					.top((SEARCH_HEIGHT - SEARCH_ICON) / 2.)
					.size(SEARCH_ICON, SEARCH_ICON)
					.pointer_events(false)
					.child(icon("picker_search", SEARCH_ICON, "text_4")),
			)
	}

	fn body(self: &Rc<Self>, body: Body) -> View {
		match body {
			Body::Home(model, in_server) => self.home(&model, in_server),
			Body::More(page, model, in_server) => self.more(page, &model, in_server),
			Body::Results(search) => self.results(&search),
		}
	}

	fn home(self: &Rc<Self>, model: &AppsLauncher, in_server: bool) -> View {
		let recents = model.recent_tiles();
		let mut home = column().w_full().h_full().min_h(0.).gap(SECTION_GAP);
		if !recents.is_empty() {
			home = home.child(
				column()
					.w_full()
					.shrink_0()
					.gap(HEADER_GAP)
					.child(self.header(RECENTS, Some(Page::Recents), "apps-recents-more"))
					.child(
						row()
							.id("apps-recents")
							.w_full()
							.gap(TILE_GAP)
							.children(recents.iter().map(|app| self.recent_tile(app))),
					),
			);
		}
		let title = if in_server { SERVER_APPS } else { APPS };
		let more = (!model.apps.is_empty()).then_some(Page::Apps);
		home.child(
			column()
				.w_full()
				.grow()
				.min_h(0.)
				.gap(HEADER_GAP)
				.child(self.header(title, more, "apps-server-more"))
				.child(if model.apps.is_empty() {
					empty(NO_APPS)
				} else {
					self.list(
						model.apps.iter().map(|app| self.app_row(app)).collect(),
						model.apps.len() as f32 * ROW,
					)
				}),
		)
	}

	fn more(self: &Rc<Self>, page: Page, model: &AppsLauncher, in_server: bool) -> View {
		let (title, apps) = match page {
			Page::Recents => (RECENTS, &model.recents),
			_ => (if in_server { SERVER_APPS } else { APPS }, &model.apps),
		};
		let back = self.clone();
		column()
			.w_full()
			.h_full()
			.min_h(0.)
			.gap(HEADER_GAP)
			.child(
				row()
					.w_full()
					.shrink_0()
					.h(BACK_BUTTON)
					.items_center()
					.gap(8.)
					.child(
						div()
							.id("apps-back")
							.size(BACK_BUTTON, BACK_BUTTON)
							.items_center()
							.justify_center()
							.rounded(tok("radius_sm"))
							.cursor(Cursor::Pointer)
							.hover(|style| style.bg(tok("hover")))
							.on_click(move || {
								back.page.set(Page::Home);
							})
							.child(icon("picker_back", BACK_ICON, "text_3")),
					)
					.child(heading(title)),
			)
			.child(self.list(
				apps.iter().map(|app| self.app_row(app)).collect(),
				apps.len() as f32 * ROW,
			))
	}

	fn results(self: &Rc<Self>, search: &AppSearch) -> View {
		if search.is_empty() {
			return empty(&format!("No results for \u{201c}{}\u{201d}", self.query().trim()));
		}
		let mut rows = Vec::new();
		let labels = [!search.apps.is_empty(), !search.commands.is_empty()]
			.iter()
			.filter(|shown| **shown)
			.count();
		let content = (search.apps.len() + search.commands.len()) as f32 * ROW + labels as f32 * GROUP_LABEL;
		if !search.apps.is_empty() {
			rows.push(group_label(APPS));
			rows.extend(search.apps.iter().map(|app| self.app_row(app)));
		}
		if !search.commands.is_empty() {
			rows.push(group_label(COMMANDS));
			rows.extend(
				search
					.commands
					.iter()
					.map(|entry| self.command_row(entry, search)),
			);
		}
		column()
			.w_full()
			.h_full()
			.min_h(0.)
			.child(self.list(rows, content))
	}

	fn header(self: &Rc<Self>, title: &str, more: Option<Page>, id: &str) -> View {
		let this = self.clone();
		row()
			.w_full()
			.h(HEADER_LINE)
			.items_center()
			.justify_between()
			.child(heading(title))
			.when_some(more, |header, page| {
				header.child(
					text(VIEW_MORE)
						.id(id.to_owned())
						.cursor(Cursor::Pointer)
						.text_size(tok("font_size_sm"))
						.line_height(HEADER_LINE)
						.font_weight(500)
						.text_color(tok("link"))
						.on_click(move || {
							this.page.set(page);
						}),
				)
			})
	}

	fn list(&self, rows: Vec<View>, content: f32) -> View {
		scroll(self.runtime.signal(0.))
			.id("apps-list")
			.w_full()
			.h(content + 2. * (LIST_INSET + BORDER))
			.min_h(0.)
			.rounded(tok("radius_md"))
			.bg(tok("bg_4"))
			.border(BORDER)
			.border_color(tok("border"))
			.child(column().w_full().p(LIST_INSET).children(rows))
	}

	fn recent_tile(self: &Rc<Self>, app: &LauncherApp) -> View {
		let this = self.clone();
		let id = app.id();
		tile(self, app, TILE, "radius_md")
			.id(format!("apps-recent-{}", id.map_or(0, |id| id.0)))
			.cursor(Cursor::Pointer)
			.tooltip(app.app.name.clone())
			.on_click(move || {
				if let Some(id) = id {
					this.choose(Pick::App(id));
				}
			})
	}

	fn app_row(self: &Rc<Self>, app: &LauncherApp) -> View {
		let this = self.clone();
		let id = app.id();
		row_frame(format!("apps-app-{}", id.map_or(0, |id| id.0)))
			.on_click(move || {
				if let Some(id) = id {
					this.choose(Pick::App(id));
				}
			})
			.child(tile(self, app, ROW_ICON, "radius_sm"))
			.child(labels(app.app.name.clone(), app.description.clone()))
	}

	fn command_row(self: &Rc<Self>, entry: &CommandEntry, search: &AppSearch) -> View {
		let this = self.clone();
		let picked = entry.clone();
		let app = search
			.apps
			.iter()
			.find(|app| app.app == entry.app)
			.cloned()
			.unwrap_or_else(|| LauncherApp {
				app: entry.app.clone(),
				description: String::new(),
			});
		row_frame(format!("apps-command-{}", entry.key.replace('\u{0}', "-")))
			.on_click(move || this.choose(Pick::Command(picked.clone())))
			.child(tile(self, &app, ROW_ICON, "radius_sm"))
			.child(labels(format!("/{}", entry.name()), entry.description.clone()))
			.child(
				text(entry.app.name.clone())
					.shrink_0()
					.text_size(tok("font_size_xs"))
					.line_height(tok("line_height_xs"))
					.text_color(tok("text_4")),
			)
	}
}

fn heading(title: &str) -> View {
	text(title.to_owned())
		.truncate()
		.text_size(tok("font_size_sm"))
		.line_height(HEADER_LINE)
		.font_weight(600)
		.text_color(tok("text_2"))
}

fn group_label(title: &str) -> View {
	text(title.to_uppercase())
		.w_full()
		.shrink_0()
		.h(GROUP_LABEL)
		.px(12.)
		.pt(8.)
		.pb(4.)
		.text_size(tok("font_size_xs"))
		.line_height(tok("line_height_xs"))
		.font_weight(600)
		.text_color(tok("text_4"))
}

fn empty(message: &str) -> View {
	text(message.to_owned())
		.w_full()
		.p(8.)
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_sm"))
		.text_color(tok("text_4"))
}

fn row_frame(id: String) -> View {
	row()
		.id(id)
		.w_full()
		.h(ROW)
		.shrink_0()
		.items_center()
		.gap(12.)
		.px(12.)
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("hover")))
}

fn labels(name: String, description: String) -> View {
	column()
		.grow()
		.min_w(0.)
		.child(
			text(name)
				.truncate()
				.text_size(tok("font_size_sm"))
				.line_height(tok("line_height_sm"))
				.font_weight(600)
				.text_color(tok("text_2")),
		)
		.when(!description.is_empty(), |labels| {
			labels.child(
				text(description)
					.truncate()
					.text_size(tok("font_size_xs"))
					.line_height(tok("line_height_xs"))
					.text_color(tok("text_4")),
			)
		})
}

fn tile(view: &AppsLauncherView, app: &LauncherApp, side: f32, radius: &str) -> View {
	let hue = HUES[app.app.hue % HUES.len()];
	let letter: String = app
		.app
		.name
		.chars()
		.take(1)
		.flat_map(char::to_uppercase)
		.collect();
	let frame = div()
		.size(side, side)
		.shrink_0()
		.rounded(tok(radius))
		.overflow_hidden()
		.items_center()
		.justify_center();
	match view.icon_image(app, TILE) {
		Some(name) => {
			let picture = view.pictures.picture(&name);
			frame.bg(tok("bg_2")).reactive_style(move || match picture.get() {
				Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
				None => Styles::new(),
			})
		}
		None => frame.bg(tok(hue)).child(
			text(letter)
				.text_size(side * LETTER_SHARE)
				.font_weight(700)
				.text_color(tok("text_0")),
		),
	}
}
