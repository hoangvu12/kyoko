mod emoji;
mod gif;
mod sticker;

use std::cell::RefCell;
use std::rc::Rc;

use bridge::{EmojiCell, EmojiPicker, GifView, MenuEntry, StickerCell, StickerPicker};
use media::ImageName;
use protocol::{FavoriteGif, Gif};
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::decoration::Corners;
use zgx::input::{EventPhase, InputEvent, Key, PointerButton};
use zgx::reactive::{Effect, Runtime, Signal};

use self::emoji::EmojiState;
use self::gif::GifState;
use self::sticker::StickerState;
use crate::motion;
use crate::plugin_menu::PluginMenu;
use crate::timeline::{icon, menu_panel, menu_row, tok};

pub(crate) const WIDTH: f32 = 498.;
pub(crate) const HEIGHT: f32 = 486.;
const BORDER: f32 = 1.;
pub(crate) const INNER_WIDTH: f32 = WIDTH - 2. * BORDER;
pub(crate) const ANCHOR_INSET: f32 = 35.;
pub(crate) const COMPOSER_GAP: f32 = 8.;
const REACTION_GAP: f32 = 8.;
const HEADER: f32 = 124.;
const HEADER_PADDING: f32 = 16.;
const TAB_HEIGHT: f32 = 28.;
const FIELD_ROW: f32 = 40.;
const FIELD_WIDTH: f32 = INNER_WIDTH - 2. * HEADER_PADDING;
const SEARCH_HEIGHT: f32 = 38.;
const SEARCH_ICON: f32 = 16.;
const BACK_BUTTON: f32 = 32.;
const BACK_ICON: f32 = 20.;
pub(crate) const RAIL: f32 = 48.;
pub(crate) const RAIL_ITEM: f32 = 32.;
pub(crate) const RAIL_ICON: f32 = 24.;
pub(crate) const RAIL_PILL: f32 = 4.;
pub(crate) const FOOTER: f32 = 47.;
pub(crate) const FOOTER_IMAGE: f32 = 28.;
pub(crate) const SECTION_HEADER: f32 = 28.;
pub(crate) const SECTION_ICON: f32 = 16.;
pub(crate) const SECTION_MARK: f32 = 12.;
pub(crate) const LOCKED_OPACITY: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
	Gif,
	Sticker,
	Emoji,
}

impl Tab {
	pub const ALL: [Tab; 3] = [Tab::Gif, Tab::Sticker, Tab::Emoji];

	pub fn named(name: &str) -> Option<Tab> {
		match name {
			"gif" => Some(Tab::Gif),
			"sticker" => Some(Tab::Sticker),
			"emoji" => Some(Tab::Emoji),
			_ => None,
		}
	}

	fn label(self) -> &'static str {
		match self {
			Tab::Gif => "GIFs",
			Tab::Sticker => "Stickers",
			Tab::Emoji => "Emoji",
		}
	}

	fn placeholder(self) -> String {
		match self {
			Tab::Gif => format!("Search {}", rest::api::GIF_PROVIDER.name),
			Tab::Sticker => "Find the perfect sticker".to_owned(),
			Tab::Emoji => "Find the perfect emoji".to_owned(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	Reaction,
	Composer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Search {
	Typed(Tab, String),
	Submitted(Tab, String),
	Category(String),
	Trending,
	Favorites,
	Back,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Favorite {
	Gif(FavoriteGif, bool),
	Emoji(String, bool),
}

type Handler<T> = RefCell<Option<Box<dyn Fn(T)>>>;

pub struct ExpressionPicker {
	pictures: Rc<crate::timeline::Pictures>,
	mode: Mode,
	open: Signal<bool>,
	tab: Signal<Tab>,
	query: Signal<String>,
	anchor: Signal<(f32, f32)>,
	emoji: EmojiState,
	gif: GifState,
	sticker: StickerState,
	on_emoji: Handler<EmojiCell>,
	on_gif: Handler<Gif>,
	on_sticker: Handler<StickerCell>,
	on_search: Handler<Search>,
	on_favorite: Handler<Favorite>,
	on_add_emoji: Handler<()>,
	can_add_emoji: Signal<bool>,
	images: RefCell<Vec<ImageName>>,
	images_changed: RefCell<Option<Box<dyn Fn()>>>,
	watcher: RefCell<Option<Effect>>,
	plugin_menu: RefCell<Option<Rc<PluginMenu>>>,
}

impl ExpressionPicker {
	pub fn new(runtime: &Runtime, pictures: Rc<crate::timeline::Pictures>, mode: Mode) -> Rc<Self> {
		let picker = Rc::new(Self {
			pictures,
			mode,
			open: runtime.signal(false),
			tab: runtime.signal(Tab::Emoji),
			query: runtime.signal(String::new()),
			anchor: runtime.signal((0., 0.)),
			emoji: EmojiState::new(runtime),
			gif: GifState::new(runtime),
			sticker: StickerState::new(runtime),
			on_emoji: RefCell::new(None),
			on_gif: RefCell::new(None),
			on_sticker: RefCell::new(None),
			on_search: RefCell::new(None),
			on_favorite: RefCell::new(None),
			on_add_emoji: RefCell::new(None),
			can_add_emoji: runtime.signal(false),
			images: RefCell::default(),
			images_changed: RefCell::new(None),
			watcher: RefCell::new(None),
			plugin_menu: RefCell::new(None),
		});
		let watched = Rc::downgrade(&picker);
		let (query, tab, open) = (picker.query.clone(), picker.tab.clone(), picker.open.clone());
		let effect = runtime.effect(move || {
			let typed = query.get();
			let tab = tab.get();
			if !open.get() {
				return;
			}
			if let Some(picker) = watched.upgrade() {
				picker.search(Search::Typed(tab, typed));
			}
		});
		*picker.watcher.borrow_mut() = Some(effect);
		picker.emoji.watch(&picker);
		picker
	}

	pub fn set_plugin_menu(&self, menu: Rc<PluginMenu>) {
		*self.plugin_menu.borrow_mut() = Some(menu);
	}

	fn expression_menu(
		self: &Rc<Self>,
		cell: View,
		target: Option<state::Expression>,
		favorite: Option<Favorite>,
	) -> View {
		let favorite = favorite.filter(|_| self.on_favorite.borrow().is_some());
		let plugins = self.plugin_menu.borrow().clone().and_then(|menu| {
			let shown = menu.shown(target);
			shown.facts.target.as_ref()?;
			let entries = shown.entries();
			(!entries.is_empty()).then_some((menu, shown, entries))
		});
		if favorite.is_none() && plugins.is_none() {
			return cell;
		}
		let open = self.pictures.runtime().signal(false);
		let opener = open.clone();
		let anchor = cell.on_event(move |event| {
			if event.phase != EventPhase::Capture
				&& let InputEvent::PointerDown {
					button: PointerButton::Secondary,
					..
				} = event.event
			{
				opener.set(true);
				event.prevent_default();
			}
		});
		let mut items: Vec<View> = Vec::new();
		if let Some(favorite) = favorite {
			let label = match &favorite {
				Favorite::Gif(_, true) | Favorite::Emoji(_, true) => "Favorite",
				Favorite::Gif(_, false) | Favorite::Emoji(_, false) => "Unfavorite",
			};
			let (this, closing) = (self.clone(), open.clone());
			items.push(
				menu_row(
					label.to_owned(),
					icon("picker_star_outline", 12., "text_normal"),
					"text_normal",
				)
				.id("picker-favorite")
				.on_click(move || {
					this.favorite(favorite.clone());
					closing.set(false);
				}),
			);
		}
		if let Some((menu, shown, entries)) = plugins {
			items.extend(entries.into_iter().filter_map(|entry| match entry {
				MenuEntry::Plugin(index) => {
					let picker = self.clone();
					Some(menu.item(&shown, index, None, move || picker.close()))
				}
				_ => None,
			}));
		}
		menu_panel(zgx::compose::menu("Expression menu", open, anchor)).children(items)
	}

	pub fn on_images_changed(&self, changed: impl Fn() + 'static) {
		*self.images_changed.borrow_mut() = Some(Box::new(changed));
	}

	pub fn on_emoji(&self, pick: impl Fn(EmojiCell) + 'static) {
		*self.on_emoji.borrow_mut() = Some(Box::new(pick));
	}

	pub fn on_gif(&self, pick: impl Fn(Gif) + 'static) {
		*self.on_gif.borrow_mut() = Some(Box::new(pick));
	}

	pub fn on_sticker(&self, pick: impl Fn(StickerCell) + 'static) {
		*self.on_sticker.borrow_mut() = Some(Box::new(pick));
	}

	pub fn on_search(&self, search: impl Fn(Search) + 'static) {
		*self.on_search.borrow_mut() = Some(Box::new(search));
	}

	pub fn on_favorite(&self, favorite: impl Fn(Favorite) + 'static) {
		*self.on_favorite.borrow_mut() = Some(Box::new(favorite));
	}

	pub fn on_add_emoji(&self, add: impl Fn(()) + 'static) {
		*self.on_add_emoji.borrow_mut() = Some(Box::new(add));
	}

	pub fn allow_add_emoji(&self, allowed: bool) {
		if self.can_add_emoji.with_untracked(|shown| *shown != allowed) {
			self.can_add_emoji.set(allowed);
		}
	}

	pub fn set_emoji(&self, model: EmojiPicker) {
		if self.emoji.model.with_untracked(|shown| **shown != model) {
			self.emoji.model.set(Rc::new(model));
		}
	}

	pub fn set_gifs(&self, view: GifView) {
		if self.gif.view.with_untracked(|shown| **shown != view) {
			self.gif.view.set(Rc::new(view));
		}
	}

	pub fn set_stickers(&self, model: StickerPicker) {
		if self.sticker.model.with_untracked(|shown| **shown != model) {
			self.sticker.model.set(Rc::new(model));
		}
	}

	pub fn open(&self, tab: Tab, at: (f32, f32)) {
		self.emoji.reset();
		self.gif.reset();
		self.sticker.reset();
		self.anchor.set(at);
		self.query.set(String::new());
		self.tab.set(tab);
		self.open.set(true);
	}

	pub fn close(&self) {
		self.open.set(false);
		self.pictures.hover(None);
		self.set_images(Vec::new());
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn tab(&self) -> Tab {
		self.tab.with_untracked(|tab| *tab)
	}

	pub fn hover_first(&self) {
		match self.tab() {
			Tab::Emoji => self.emoji.hover_first(),
			Tab::Gif => self.gif.hover_first(),
			Tab::Sticker => self.sticker.hover_first(),
		}
	}

	pub fn reveal_locked(&self) {
		self.emoji.reveal_locked();
	}

	pub fn type_query(&self, text: &str) {
		self.query.set(text.to_owned());
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

	fn set_images(&self, names: Vec<ImageName>) {
		if *self.images.borrow() == names {
			return;
		}
		*self.images.borrow_mut() = names;
		if let Some(changed) = self.images_changed.borrow().as_ref() {
			changed();
		}
	}

	fn shown_images(&self, names: Vec<ImageName>) {
		if self.is_open() {
			self.set_images(names);
		}
	}

	fn header_height(&self) -> f32 {
		match self.mode {
			Mode::Composer => HEADER,
			Mode::Reaction => HEADER - TAB_HEIGHT - HEADER_PADDING,
		}
	}

	fn body_height(&self) -> f32 {
		HEIGHT - 2. * BORDER - self.header_height()
	}

	fn search(&self, search: Search) {
		if let Some(handler) = self.on_search.borrow().as_ref() {
			handler(search);
		}
	}

	fn favorite(&self, favorite: Favorite) {
		if let Some(handler) = self.on_favorite.borrow().as_ref() {
			handler(favorite);
		}
	}

	fn chose_emoji(&self, cell: EmojiCell) {
		if cell.locked {
			return;
		}
		if let Some(pick) = self.on_emoji.borrow().as_ref() {
			pick(cell);
		}
		self.close();
	}

	fn chose_gif(&self, gif: Gif) {
		if let Some(pick) = self.on_gif.borrow().as_ref() {
			pick(gif);
		}
		self.close();
	}

	fn chose_sticker(&self, cell: StickerCell) {
		if cell.locked {
			return;
		}
		if let Some(pick) = self.on_sticker.borrow().as_ref() {
			pick(cell);
		}
		self.close();
	}

	fn submitted(&self) -> bool {
		let tab = self.tab();
		match tab {
			Tab::Emoji => match self.emoji.first() {
				Some(first) => {
					self.chose_emoji(first);
					true
				}
				None => false,
			},
			Tab::Gif => {
				self.search(Search::Submitted(tab, self.query.with_untracked(Clone::clone)));
				true
			}
			Tab::Sticker => false,
		}
	}

	pub fn view(self: &Rc<Self>) -> View {
		let anchor = self.anchor.clone();
		let spot = div().w(1.).h(1.).reactive_style(move || {
			let (x, y) = anchor.get();
			Styles::new().absolute().left(x - 1.).top(y)
		});
		let placement = match self.mode {
			Mode::Reaction => Placement {
				side: PlacementSide::Left,
				align: PlacementEdge::Start,
				offset: REACTION_GAP,
			},
			Mode::Composer => Placement {
				side: PlacementSide::Top,
				align: PlacementEdge::End,
				offset: COMPOSER_GAP,
			},
		};
		let label = match self.mode {
			Mode::Reaction => "Add Reaction",
			Mode::Composer => "Expression picker",
		};
		let this = self.clone();
		let tab = self.tab.clone();
		let body = self.body_height();
		motion::popover(label, self.open.clone(), spot)
			.placement(placement)
			.w(WIDTH)
			.h(HEIGHT)
			.p(0.)
			.gap(0.)
			.rounded(tok("radius_md"))
			.border(BORDER)
			.border_color(tok("border"))
			.bg(tok("bg_3"))
			.text_color(tok("text_3"))
			.overflow_hidden()
			.child(self.header())
			.child(column().w(INNER_WIDTH).h(body).min_h(0.).child(switch(
				move || tab.get(),
				move |tab: Tab, _| match tab {
					Tab::Emoji => this.emoji_pane(),
					Tab::Gif => this.gif_pane(),
					Tab::Sticker => this.sticker_pane(),
				},
			)))
	}

	fn header(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (tab, view) = (self.tab.clone(), self.gif.view.clone());
		column()
			.shrink_0()
			.w(INNER_WIDTH)
			.h(self.header_height())
			.gap(HEADER_PADDING)
			.pt(HEADER_PADDING)
			.px(HEADER_PADDING)
			.bg(tok("bg_3"))
			.border_edges(zgx::scene::Insets {
				bottom: BORDER,
				..Default::default()
			})
			.border_color(tok("border"))
			.when(self.mode == Mode::Composer, |header| header.child(self.tabs()))
			.child(row().w(FIELD_WIDTH).h(FIELD_ROW).items_center().child(switch(
				move || {
					let tab = tab.get();
					let browsing = if tab == Tab::Gif {
						view.with(|view| match &**view {
							GifView::Results { browsing, query, .. } => browsing.then(|| query.clone()),
							GifView::Home { .. } => None,
						})
					} else {
						None
					};
					(tab, browsing)
				},
				move |(tab, browsing): (Tab, Option<String>), _| match browsing {
					Some(title) => this.back_row(title),
					None => this.field_row(tab),
				},
			)))
	}

	fn back_row(self: &Rc<Self>, title: String) -> View {
		let this = self.clone();
		row()
			.w(FIELD_WIDTH)
			.h(FIELD_ROW)
			.items_center()
			.gap(8.)
			.child(
				div()
					.id("picker-back")
					.size(BACK_BUTTON, BACK_BUTTON)
					.items_center()
					.justify_center()
					.rounded(tok("radius_sm"))
					.cursor(Cursor::Pointer)
					.hover(|style| style.bg(tok("hover")))
					.on_click(move || this.search(Search::Back))
					.child(icon("picker_back", BACK_ICON, "text_3")),
			)
			.child(
				text(title_case(&title))
					.truncate()
					.text_size(tok("font_size_lg"))
					.line_height(24.)
					.font_weight(600)
					.text_color(tok("text_2")),
			)
	}

	fn field_row(self: &Rc<Self>, tab: Tab) -> View {
		row()
			.w(FIELD_WIDTH)
			.h(FIELD_ROW)
			.items_center()
			.gap(12.)
			.child(self.search_field())
			.when(tab == Tab::Emoji, |row| {
				let this = self.clone();
				row.child(self.tone_button()).child(switch(
					{
						let allowed = self.can_add_emoji.clone();
						move || allowed.get()
					},
					move |allowed: bool, _| if allowed { this.add_emoji_button() } else { div() },
				))
			})
	}

	fn search_field(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (tab, hovered) = (self.tab.clone(), self.emoji.hovered.clone());
		div()
			.relative()
			.grow()
			.min_w(0.)
			.h(SEARCH_HEIGHT)
			.child(
				text_input("Search", self.query.clone())
					.id("picker-search")
					.autofocus(true)
					.placeholder_when(move || {
						let tab = tab.get();
						let hovered = hovered.with(|hovered| hovered.as_ref().map(|cell| cell.label.clone()));
						match (tab, hovered) {
							(Tab::Emoji, Some(label)) => label,
							_ => tab.placeholder(),
						}
					})
					.on_event(move |event| {
						if matches!(event.event, InputEvent::KeyDown { key: Key::Enter, .. })
							&& this.submitted()
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

	fn add_emoji_button(self: &Rc<Self>) -> View {
		let this = self.clone();
		row()
			.id("picker-add-emoji")
			.shrink_0()
			.h(FIELD_ROW)
			.px(16.)
			.items_center()
			.justify_center()
			.rounded(tok("radius_sm"))
			.bg(tok("bg_2"))
			.border(BORDER)
			.border_color(tok("border_light"))
			.cursor(Cursor::Pointer)
			.hover(|style| style.bg(tok("bg_1")))
			.on_click(move || {
				if let Some(add) = this.on_add_emoji.borrow().as_ref() {
					add(());
				}
				this.close();
			})
			.child(
				text("Add Emoji")
					.text_size(tok("font_size_sm"))
					.font_weight(500)
					.text_color(tok("text_3")),
			)
	}

	fn tabs(&self) -> View {
		row()
			.gap(8.)
			.h(TAB_HEIGHT)
			.children(Tab::ALL.into_iter().map(|choice| {
				let (tab, query) = (self.tab.clone(), self.query.clone());
				let active = self.tab.clone();
				row()
					.id(format!("picker-tab-{}", choice.label().to_lowercase()))
					.h(TAB_HEIGHT)
					.px(8.)
					.items_center()
					.rounded(tok("radius_sm"))
					.cursor(Cursor::Pointer)
					.text_size(tok("font_size_sm"))
					.line_height(18.)
					.font_weight(600)
					.reactive_style(move || {
						if active.get() == choice {
							Styles::new().bg(tok("bg_2")).text_color(tok("text_2"))
						} else {
							Styles::new().text_color(tok("text_4"))
						}
					})
					.hover(|style| style.text_color(tok("text_3")))
					.on_click(move || {
						if tab.with_untracked(|current| *current) != choice {
							query.set(String::new());
							tab.set(choice);
						}
					})
					.child(text(choice.label()))
			}))
	}
}

fn title_case(title: &str) -> String {
	let mut chars = title.chars();
	match chars.next() {
		Some(first) => first.to_uppercase().chain(chars).collect(),
		None => String::new(),
	}
}

fn rail_pill() -> View {
	div()
		.absolute()
		.left(0.)
		.top((RAIL_ITEM - RAIL_ICON) / 2.)
		.w(RAIL_PILL)
		.h(RAIL_ICON)
		.rounded_corners(Corners {
			top_left: 0.,
			top_right: RAIL_PILL,
			bottom_right: RAIL_PILL,
			bottom_left: 0.,
		})
		.bg(tok("text_3"))
}

pub(crate) fn rail_frame(active: impl FnMut() -> bool + 'static, content: View) -> View {
	div()
		.relative()
		.shrink_0()
		.w(RAIL)
		.h(RAIL_ITEM)
		.items_center()
		.justify_center()
		.cursor(Cursor::Pointer)
		.child(content)
		.child(switch(
			active,
			|active: bool, _| if active { rail_pill() } else { div() },
		))
}

pub(crate) fn rail_divider() -> View {
	div().shrink_0().w(24.).h(BORDER).bg(tok("border"))
}

pub(crate) fn section_header(icon_view: View, title: String, locked: bool, collapsed: bool) -> View {
	row()
		.w_full()
		.h(SECTION_HEADER)
		.px(4.)
		.gap(6.)
		.items_center()
		.cursor(Cursor::Pointer)
		.child(icon_view)
		.child(
			text(title)
				.truncate()
				.text_size(tok("font_size_sm"))
				.line_height(18.)
				.font_weight(600)
				.text_color(tok("text_3")),
		)
		.when(locked, |header| {
			header.child(icon("picker_lock", SECTION_MARK, "text_4"))
		})
		.child(icon(
			if collapsed {
				"chevron_right"
			} else {
				"picker_chevron"
			},
			SECTION_MARK,
			"text_4",
		))
}

pub(crate) fn footer_frame(content: View) -> View {
	row()
		.shrink_0()
		.w(INNER_WIDTH)
		.h(FOOTER)
		.px(16.)
		.gap(10.)
		.items_center()
		.bg(tok("bg_4"))
		.border_edges(zgx::scene::Insets {
			top: BORDER,
			..Default::default()
		})
		.border_color(tok("border"))
		.child(content)
}

pub(crate) fn footer_text(name: String, note: Option<String>) -> View {
	column()
		.grow()
		.min_w(0.)
		.child(
			text(name)
				.truncate()
				.text_size(tok("font_size_md"))
				.line_height(20.)
				.font_weight(600)
				.text_color(tok("text_2")),
		)
		.when_some(note, |column, note| {
			column.child(
				text(note)
					.truncate()
					.text_size(tok("font_size_xs"))
					.line_height(16.)
					.text_color(tok("text_4")),
			)
		})
}

pub(crate) fn greyed(view: View, locked: bool) -> View {
	if locked {
		div().saturate(0.).opacity(LOCKED_OPACITY).child(view)
	} else {
		view
	}
}

pub(crate) fn portrait(
	pictures: &crate::timeline::Pictures,
	guild_id: protocol::Snowflake,
	icon: Option<&str>,
	name: &str,
	side: f32,
) -> View {
	portrait_shaped(pictures, guild_id, icon, name, side, side / 2.)
}

pub(crate) fn portrait_shaped(
	pictures: &crate::timeline::Pictures,
	guild_id: protocol::Snowflake,
	icon: Option<&str>,
	name: &str,
	side: f32,
	radius: f32,
) -> View {
	let initials = bridge::acronym(name);
	let picture = icon.map(|hash| pictures.picture(&pictures.guild_icon(guild_id, hash, side)));
	let frame = div()
		.size(side, side)
		.shrink_0()
		.rounded(radius)
		.overflow_hidden()
		.items_center()
		.justify_center();
	match picture {
		Some(picture) => frame.bg(tok("bg_2")).reactive_style(move || match picture.get() {
			Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
			None => Styles::new(),
		}),
		None => frame.bg(tok("accent_3")).child(
			text(initials)
				.text_size(if side < 24. { 8. } else { 10. })
				.font_weight(700)
				.text_color(tok("text_0")),
		),
	}
}
