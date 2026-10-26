mod emoji;
mod gif;
mod sticker;

use std::cell::RefCell;
use std::rc::Rc;

use bridge::{EmojiCell, EmojiPicker, GifView, MenuEntry, StickerCell, StickerPicker};
use media::ImageName;
use protocol::Gif;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::input::{EventPhase, InputEvent, Key, PointerButton};
use zgx::reactive::{Effect, Runtime, Signal};

use self::emoji::EmojiState;
use self::gif::GifState;
use self::sticker::StickerState;
use crate::motion;
use crate::plugin_menu::PluginMenu;
use crate::timeline::{Pictures, menu_panel, tok};

const WIDTH: f32 = 456.;
const HEIGHT: f32 = 440.;
const REACTION_GAP: f32 = 8.;
const COMPOSER_GAP: f32 = 12.;
const SEARCH_HEIGHT: f32 = 28.;
const TAB_HEIGHT: f32 = 32.;
const COMPOSER_PANE_HEIGHT: f32 = HEIGHT - 2. - (12. + TAB_HEIGHT + 8. + SEARCH_HEIGHT + 12. + 1.);

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

	fn placeholder(self) -> &'static str {
		match self {
			Tab::Gif => "Search for GIFs",
			Tab::Sticker => "Find the perfect sticker",
			Tab::Emoji => "Find the perfect emoji",
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
}

type Handler<T> = RefCell<Option<Box<dyn Fn(T)>>>;

pub struct ExpressionPicker {
	pictures: Rc<Pictures>,
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
	images: RefCell<Vec<ImageName>>,
	images_changed: RefCell<Option<Box<dyn Fn()>>>,
	watcher: RefCell<Option<Effect>>,
	plugin_menu: RefCell<Option<Rc<PluginMenu>>>,
}

impl ExpressionPicker {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>, mode: Mode) -> Rc<Self> {
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
		picker
	}

	pub fn set_plugin_menu(&self, menu: Rc<PluginMenu>) {
		*self.plugin_menu.borrow_mut() = Some(menu);
	}

	fn expression_menu(self: &Rc<Self>, cell: View, target: Option<state::Expression>) -> View {
		let Some(menu) = self.plugin_menu.borrow().clone() else {
			return cell;
		};
		let shown = menu.shown(target);
		if shown.facts.target.is_none() {
			return cell;
		}
		let entries = shown.entries();
		if entries.is_empty() {
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
		let items = entries.into_iter().filter_map(|entry| match entry {
			MenuEntry::Plugin(index) => {
				let picker = self.clone();
				Some(menu.item(&shown, index, None, move || picker.close()))
			}
			_ => None,
		});
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

	pub fn set_emoji(&self, model: EmojiPicker) {
		self.emoji.model.set(Rc::new(model));
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

	fn pane_height(&self) -> f32 {
		match self.mode {
			Mode::Composer => COMPOSER_PANE_HEIGHT,
			Mode::Reaction => COMPOSER_PANE_HEIGHT + TAB_HEIGHT + 8.,
		}
	}

	fn search(&self, search: Search) {
		if let Some(handler) = self.on_search.borrow().as_ref() {
			handler(search);
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
			Styles::new().absolute().left(x).top(y)
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
		motion::popover(label, self.open.clone(), spot)
			.placement(placement)
			.w(WIDTH)
			.h(HEIGHT)
			.p(0.)
			.gap(0.)
			.rounded(tok("radius_md"))
			.border(1.)
			.border_color(tok("divider"))
			.bg(tok("background_floating"))
			.text_color(tok("text_normal"))
			.overflow_hidden()
			.child(self.header())
			.child(column().grow().min_h(0.).child(switch(
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
		let placeholder = self.tab.clone();
		column()
			.shrink_0()
			.gap(8.)
			.p(12.)
			.border_edges(zgx::scene::Insets {
				bottom: 1.,
				..Default::default()
			})
			.border_color(tok("divider"))
			.w_full()
			.when(self.mode == Mode::Composer, |header| header.child(self.tabs()))
			.child(
				text_input("Search", self.query.clone())
					.placeholder_when(move || placeholder.get().placeholder().to_owned())
					.on_event(move |event| {
						if matches!(event.event, InputEvent::KeyDown { key: Key::Enter, .. })
							&& this.submitted()
						{
							event.prevent_default();
						}
					})
					.h(SEARCH_HEIGHT)
					.px(8.)
					.rounded(tok("radius_sm"))
					.border(1.)
					.border_color(tok("divider"))
					.focus(|style| style.border_color(tok("brand")))
					.text_size(tok("font_sm"))
					.bg(tok("input_background")),
			)
	}

	fn tabs(&self) -> View {
		row().gap(8.).children(Tab::ALL.into_iter().map(|choice| {
			let (tab, query) = (self.tab.clone(), self.query.clone());
			let active = self.tab.clone();
			row()
				.h(TAB_HEIGHT)
				.px(12.)
				.items_center()
				.rounded(tok("radius_sm"))
				.border(1.)
				.cursor(Cursor::Pointer)
				.text_size(tok("font_sm"))
				.font_weight(500)
				.reactive_style(move || {
					if active.get() == choice {
						Styles::new()
							.border_color(tok("brand"))
							.bg(tok("brand_selected"))
							.text_color(tok("interactive_active"))
					} else {
						Styles::new()
							.border_color(tok("divider"))
							.bg(tok("background_secondary"))
							.text_color(tok("interactive_normal"))
					}
				})
				.hover(|style| style.bg(tok("modifier_hover")))
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

pub(crate) fn section_title(title: String) -> View {
	text(title.to_uppercase())
		.text_size(tok("font_tiny"))
		.font_weight(700)
		.text_color(tok("header_secondary"))
}

pub(crate) fn portrait(
	pictures: &Pictures,
	guild_id: protocol::Snowflake,
	icon: Option<&str>,
	name: &str,
	side: f32,
) -> View {
	let initials = bridge::acronym(name);
	let picture = icon.map(|hash| pictures.picture(&pictures.guild_icon(guild_id, hash, side)));
	let circle = div()
		.size(side, side)
		.rounded(side / 2.)
		.overflow_hidden()
		.items_center()
		.justify_center();
	match picture {
		Some(picture) => circle
			.bg(tok("background_secondary"))
			.reactive_style(move || match picture.get() {
				Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
				None => Styles::new(),
			}),
		None => circle.bg(tok("brand")).child(
			text(initials)
				.text_size(tok("font_sm"))
				.font_weight(600)
				.text_color(tok("text_on_brand")),
		),
	}
}
