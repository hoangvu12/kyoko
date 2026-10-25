use std::cell::RefCell;
use std::rc::Rc;

use bridge::{EmojiCell, EmojiPicker, PickerSection, RailEntry, RailIcon, TONE_SAMPLE, TONES};
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::input::{InputEvent, Key};
use zgx::reactive::{Runtime, Signal};

use super::actions::flip;
use super::icons::icon;
use super::pictures::Pictures;
use super::rows::tok;

const WIDTH: f32 = 456.;
const HEIGHT: f32 = 440.;
const CELL: f32 = 44.;
const CELL_IMAGE: f32 = 32.;
const RAIL: f32 = 48.;
const RAIL_ITEM: f32 = 40.;
const PORTRAIT: f32 = 32.;
const PREVIEW: f32 = 24.;
const FOOTER: f32 = 40.;
const LOCKED_OPACITY: f32 = 0.35;

type Pick = RefCell<Option<Box<dyn Fn(EmojiCell)>>>;

pub struct ReactionPicker {
	pictures: Rc<Pictures>,
	open: Signal<bool>,
	model: Signal<Rc<EmojiPicker>>,
	query: Signal<String>,
	section: Signal<Option<PickerSection>>,
	hovered: Signal<Option<EmojiCell>>,
	tone: Signal<usize>,
	tone_open: Signal<bool>,
	anchor: Signal<(f32, f32)>,
	scroll: Signal<f32>,
	rail_scroll: Signal<f32>,
	pick: Pick,
	images: RefCell<Vec<ImageName>>,
	shown: RefCell<Vec<EmojiCell>>,
	images_changed: RefCell<Option<Box<dyn Fn()>>>,
}

impl ReactionPicker {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Self {
		Self {
			pictures,
			open: runtime.signal(false),
			model: runtime.signal(Rc::default()),
			query: runtime.signal(String::new()),
			section: runtime.signal(None),
			hovered: runtime.signal(None),
			tone: runtime.signal(0),
			tone_open: runtime.signal(false),
			anchor: runtime.signal((0., 0.)),
			scroll: runtime.signal(0.),
			rail_scroll: runtime.signal(0.),
			pick: RefCell::new(None),
			images: RefCell::default(),
			shown: RefCell::default(),
			images_changed: RefCell::new(None),
		}
	}

	pub fn on_images_changed(&self, changed: impl Fn() + 'static) {
		*self.images_changed.borrow_mut() = Some(Box::new(changed));
	}

	pub fn open(&self, model: EmojiPicker, at: (f32, f32), pick: impl Fn(EmojiCell) + 'static) {
		*self.pick.borrow_mut() = Some(Box::new(pick));
		self.query.set(String::new());
		self.section.set(None);
		self.hovered.set(None);
		self.scroll.set(0.);
		self.rail_scroll.set(0.);
		self.model.set(Rc::new(model));
		self.anchor.set(at);
		self.open.set(true);
	}

	pub fn close(&self) {
		self.open.set(false);
		self.set_images(Vec::new());
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn images(&self) -> Vec<ImageName> {
		if self.is_open() {
			self.images.borrow().clone()
		} else {
			Vec::new()
		}
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

	fn chosen(&self, cell: EmojiCell) {
		if cell.locked {
			return;
		}
		if let Some(pick) = self.pick.borrow().as_ref() {
			pick(cell);
		}
		self.close();
	}

	pub fn view(self: &Rc<Self>) -> View {
		let anchor = self.anchor.clone();
		let spot = div().w(1.).h(1.).reactive_style(move || {
			let (x, y) = anchor.get();
			Styles::new().absolute().left(x).top(y)
		});
		let this = self.clone();
		popover("Add Reaction", self.open.clone(), spot)
			.placement(Placement {
				side: PlacementSide::Left,
				align: PlacementEdge::Start,
				offset: 8.,
			})
			.w(WIDTH)
			.h(HEIGHT)
			.rounded(tok("radius_md"))
			.border(1.)
			.border_color(tok("divider"))
			.bg(tok("background_floating"))
			.text_color(tok("text_normal"))
			.overflow_hidden()
			.child(self.search())
			.child(row().grow().min_h(0.).child(this.rail()).child(this.main()))
	}

	fn search(self: &Rc<Self>) -> View {
		let this = self.clone();
		column()
			.shrink_0()
			.gap(8.)
			.p(12.)
			.border_edges(zgx::scene::Insets {
				bottom: 1.,
				..Default::default()
			})
			.border_color(tok("divider"))
			.child(
				text_input("Find the perfect emoji", self.query.clone())
					.on_event(move |event| {
						if !matches!(event.event, InputEvent::KeyDown { key: Key::Enter, .. }) {
							return;
						}
						let first = this.shown.borrow().first().cloned();
						if let Some(first) = first {
							event.prevent_default();
							this.chosen(first);
						}
					})
					.h(28.)
					.px(8.)
					.text_size(tok("font_sm"))
					.bg(tok("input_background")),
			)
	}

	fn rail(self: &Rc<Self>) -> View {
		let this = self.clone();
		let model = self.model.clone();
		scroll(self.rail_scroll.clone())
			.w(RAIL)
			.shrink_0()
			.h_full()
			.border_edges(zgx::scene::Insets {
				right: 1.,
				..Default::default()
			})
			.border_color(tok("divider"))
			.child(switch(
				move || model.get(),
				move |model: Rc<EmojiPicker>, _| {
					column().items_center().gap(2.).py(8.).children(
						model
							.rail()
							.into_iter()
							.map(|entry| this.rail_item(entry, &model)),
					)
				},
			))
	}

	fn rail_item(self: &Rc<Self>, entry: RailEntry, model: &Rc<EmojiPicker>) -> View {
		let (section, query, model) = (self.section.clone(), self.query.clone(), model.clone());
		let target = entry.section;
		let scroll = self.scroll.clone();
		let content = match &entry.icon {
			RailIcon::Recent => icon("clock", 20., "interactive_normal"),
			RailIcon::Glyph(glyph) => text(*glyph).text_size(tok("font_lg")),
			RailIcon::Guild { guild_id, icon } => self.portrait(*guild_id, icon.as_deref(), &entry.title),
		};
		div()
			.size(RAIL_ITEM, RAIL_ITEM)
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_sm"))
			.cursor(Cursor::Pointer)
			.reactive_style({
				let (section, query, model) = (section.clone(), query.clone(), model.clone());
				move || {
					let active = query.with(String::is_empty)
						&& section.get().unwrap_or_else(|| model.default_section()) == target;
					if active {
						Styles::new().bg(tok("modifier_selected"))
					} else {
						Styles::new()
					}
				}
			})
			.hover(|style| style.bg(tok("modifier_hover")))
			.on_click(move || {
				section.set(Some(target));
				query.set(String::new());
				scroll.set(0.);
			})
			.child(content)
			.tooltip(entry.title)
	}

	fn portrait(&self, guild_id: protocol::Snowflake, icon: Option<&str>, name: &str) -> View {
		let initials = bridge::acronym(name);
		let picture = icon.map(|hash| {
			self.pictures
				.picture(&self.pictures.guild_icon(guild_id, hash, PORTRAIT))
		});
		let circle = div()
			.size(PORTRAIT, PORTRAIT)
			.rounded(PORTRAIT / 2.)
			.overflow_hidden()
			.bg(tok("background_primary"))
			.items_center()
			.justify_center()
			.child(
				text(initials)
					.text_size(11.)
					.font_weight(600)
					.text_color(tok("header_primary")),
			);
		match picture {
			Some(picture) => circle.reactive_style(move || match picture.get() {
				Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
				None => Styles::new(),
			}),
			None => circle,
		}
	}

	fn main(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (model, query, section, tone) = (
			self.model.clone(),
			self.query.clone(),
			self.section.clone(),
			self.tone.clone(),
		);
		column()
			.grow()
			.min_w(0.)
			.child(switch(
				move || (model.get(), query.get(), section.get(), tone.get()),
				move |(model, query, section, tone): (
					Rc<EmojiPicker>,
					String,
					Option<PickerSection>,
					usize,
				),
				      _| {
					let (title, cells) = if query.trim().is_empty() {
						model.section(section.unwrap_or_else(|| model.default_section()), tone)
					} else {
						("Search Results".to_owned(), model.search(&query, tone))
					};
					this.grid(title, cells)
				},
			))
			.child(self.footer())
	}

	fn grid(self: &Rc<Self>, title: String, cells: Vec<EmojiCell>) -> View {
		let names: Vec<ImageName> = cells
			.iter()
			.filter_map(|cell| cell.custom.as_ref())
			.map(|custom| {
				self.pictures
					.emoji(custom.id.get(), custom.animated, f64::from(CELL_IMAGE))
			})
			.collect();
		if self.is_open() {
			self.set_images(names);
		}
		*self.shown.borrow_mut() = cells.clone();
		column()
			.grow()
			.min_h(0.)
			.child(
				text(title.to_uppercase())
					.shrink_0()
					.px(12.)
					.py(8.)
					.text_size(tok("font_tiny"))
					.font_weight(700)
					.text_color(tok("header_secondary")),
			)
			.child(
				scroll(self.scroll.clone()).grow().min_h(0.).child(
					row()
						.flex_wrap()
						.px(8.)
						.pb(8.)
						.children(cells.into_iter().map(|cell| self.cell(cell))),
				),
			)
	}

	fn cell(self: &Rc<Self>, cell: EmojiCell) -> View {
		let locked = cell.locked;
		let visual = self.visual(&cell, CELL_IMAGE, "font_xl");
		let (hovered, chosen) = (self.hovered.clone(), self.clone());
		let shown = cell.clone();
		div()
			.size(CELL, CELL)
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_sm"))
			.when(locked, |cell| cell.opacity(LOCKED_OPACITY))
			.when(!locked, |cell| {
				cell.cursor(Cursor::Pointer)
					.hover(|style| style.bg(tok("modifier_selected")))
			})
			.on_event(move |event| {
				if matches!(event.event, InputEvent::PointerEnter) {
					hovered.set(Some(shown.clone()));
				}
			})
			.on_click(move || chosen.chosen(cell.clone()))
			.child(visual)
	}

	fn visual(&self, cell: &EmojiCell, side: f32, text_size: &str) -> View {
		match (&cell.glyph, &cell.custom) {
			(Some(glyph), _) => text(glyph.clone()).text_size(tok(text_size)),
			(None, Some(custom)) => {
				let picture = self.pictures.picture(&self.pictures.emoji(
					custom.id.get(),
					custom.animated,
					f64::from(CELL_IMAGE),
				));
				div()
					.size(side, side)
					.reactive_style(move || match picture.get() {
						Some(image) => Styles::new().bg_image(image, ObjectFit::Contain),
						None => Styles::new(),
					})
			}
			(None, None) => div(),
		}
	}

	fn footer(self: &Rc<Self>) -> View {
		let this = self.clone();
		let hovered = self.hovered.clone();
		row()
			.shrink_0()
			.h(FOOTER)
			.px(12.)
			.items_center()
			.justify_between()
			.border_edges(zgx::scene::Insets {
				top: 1.,
				..Default::default()
			})
			.border_color(tok("divider"))
			.child(switch(
				move || hovered.get(),
				move |hovered: Option<EmojiCell>, _| match hovered {
					Some(cell) => this.preview(&cell),
					None => div(),
				},
			))
			.child(self.tone_button())
	}

	fn preview(&self, cell: &EmojiCell) -> View {
		row()
			.items_center()
			.gap(8.)
			.min_w(0.)
			.child(self.visual(cell, PREVIEW, "font_xl"))
			.child(
				text(cell.label.clone())
					.truncate()
					.text_size(tok("font_sm"))
					.font_weight(600)
					.text_color(tok("header_primary")),
			)
			.when_some(cell.detail.clone(), |preview, detail| {
				preview.child(
					text(format!("from {detail}"))
						.truncate()
						.text_size(tok("font_sm"))
						.text_color(tok("text_muted")),
				)
			})
	}

	fn tone_button(&self) -> View {
		let tone = self.tone.clone();
		let label = self.tone.clone();
		let open = self.tone_open.clone();
		let trigger = row()
			.h(24.)
			.px(6.)
			.items_center()
			.rounded(tok("radius_sm"))
			.cursor(Cursor::Pointer)
			.hover(|style| style.bg(tok("modifier_hover")))
			.on_click(move || flip(&open))
			.child(text_signal(move || {
				format!("{TONE_SAMPLE}{}", TONES[label.get().min(TONES.len() - 1)])
			}))
			.tooltip("Skin tone");
		menu("Skin tone", self.tone_open.clone(), trigger)
			.p(4.)
			.rounded(tok("radius_sm"))
			.bg(tok("background_floating"))
			.children(TONES.iter().enumerate().map(move |(index, modifier)| {
				let tone = tone.clone();
				menu_item(format!("{TONE_SAMPLE}{modifier}")).on_click(move || {
					tone.set(index);
				})
			}))
	}
}
