use std::cell::RefCell;
use std::rc::Rc;

use bridge::{EmojiCell, EmojiPicker, PickerSection, RailEntry, RailIcon, TONE_SAMPLE, TONES};
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::reactive::{Runtime, Signal};

use super::{ExpressionPicker, portrait, section_title};
use crate::emoji;
use crate::timeline::{icon, tok};

const CELL: f32 = 44.;
const CELL_IMAGE: f32 = 32.;
const RAIL: f32 = 48.;
const RAIL_ITEM: f32 = 40.;
const PORTRAIT: f32 = 32.;
const PREVIEW: f32 = 24.;
const RAIL_GLYPH: f32 = 20.;
const TONE_GLYPH: f32 = 18.;
const FOOTER: f32 = 40.;
const LOCKED_OPACITY: f32 = 0.35;

pub(super) struct EmojiState {
	pub(super) model: Signal<Rc<EmojiPicker>>,
	section: Signal<Option<PickerSection>>,
	hovered: Signal<Option<EmojiCell>>,
	tone: Signal<usize>,
	tone_open: Signal<bool>,
	scroll: Signal<f32>,
	rail_scroll: Signal<f32>,
	shown: RefCell<Vec<EmojiCell>>,
}

impl EmojiState {
	pub(super) fn new(runtime: &Runtime) -> Self {
		Self {
			model: runtime.signal(Rc::default()),
			section: runtime.signal(None),
			hovered: runtime.signal(None),
			tone: runtime.signal(0),
			tone_open: runtime.signal(false),
			scroll: runtime.signal(0.),
			rail_scroll: runtime.signal(0.),
			shown: RefCell::default(),
		}
	}

	pub(super) fn reset(&self) {
		self.section.set(None);
		self.hovered.set(None);
		self.scroll.set(0.);
		self.rail_scroll.set(0.);
	}

	pub(super) fn first(&self) -> Option<EmojiCell> {
		self.shown.borrow().first().cloned()
	}
}

impl ExpressionPicker {
	pub(super) fn emoji_pane(self: &Rc<Self>) -> View {
		row()
			.w(super::WIDTH - 2.)
			.h(self.pane_height())
			.child(self.emoji_rail())
			.child(self.emoji_main())
	}

	fn emoji_rail(self: &Rc<Self>) -> View {
		let this = self.clone();
		let model = self.emoji.model.clone();
		scroll(self.emoji.rail_scroll.clone())
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
							.map(|entry| this.emoji_rail_item(entry, &model)),
					)
				},
			))
	}

	fn emoji_rail_item(self: &Rc<Self>, entry: RailEntry, model: &Rc<EmojiPicker>) -> View {
		let state = &self.emoji;
		let (section, query, model) = (state.section.clone(), self.query.clone(), model.clone());
		let target = entry.section;
		let scroll = state.scroll.clone();
		let content = match &entry.icon {
			RailIcon::Recent => icon("clock", 20., "interactive_normal"),
			RailIcon::Glyph(glyph) => match emoji::svg_of(glyph) {
				Some(art) => svg(*glyph, art).size(RAIL_GLYPH, RAIL_GLYPH),
				None => text(*glyph).text_size(tok("font_lg")),
			},
			RailIcon::Guild { guild_id, icon } => {
				portrait(&self.pictures, *guild_id, icon.as_deref(), &entry.title, PORTRAIT)
			}
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

	fn emoji_main(self: &Rc<Self>) -> View {
		let this = self.clone();
		let state = &self.emoji;
		let (model, query, section, tone) = (
			state.model.clone(),
			self.query.clone(),
			state.section.clone(),
			state.tone.clone(),
		);
		let grid_height = self.pane_height() - FOOTER;
		column()
			.grow()
			.min_w(0.)
			.child(column().h(grid_height).child(switch(
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
					this.emoji_grid(title, cells, grid_height)
				},
			)))
			.child(self.emoji_footer())
	}

	fn emoji_grid(self: &Rc<Self>, title: String, cells: Vec<EmojiCell>, height: f32) -> View {
		let names: Vec<ImageName> = cells
			.iter()
			.filter_map(|cell| cell.custom.as_ref())
			.map(|custom| {
				self.pictures
					.emoji(custom.id.get(), custom.animated, f64::from(CELL_IMAGE))
			})
			.collect();
		self.shown_images(names);
		*self.emoji.shown.borrow_mut() = cells.clone();
		column()
			.h(height)
			.child(section_title(title).shrink_0().px(12.).py(8.))
			.child(
				scroll(self.emoji.scroll.clone()).grow().min_h(0.).child(
					row()
						.flex_wrap()
						.px(8.)
						.pb(8.)
						.children(cells.into_iter().map(|cell| self.emoji_cell(cell))),
				),
			)
	}

	fn emoji_cell(self: &Rc<Self>, cell: EmojiCell) -> View {
		let locked = cell.locked;
		let visual = self.emoji_visual(&cell, CELL_IMAGE, "font_xl");
		let (hovered, chosen) = (self.emoji.hovered.clone(), self.clone());
		let shown = cell.clone();
		let hover_name = cell.custom.as_ref().map(|custom| {
			self.pictures
				.emoji(custom.id.get(), custom.animated, f64::from(CELL_IMAGE))
		});
		let pictures = self.pictures.clone();
		let target = bridge::cell_expression(&cell);
		let view = div()
			.size(CELL, CELL)
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_sm"))
			.when(locked, |cell| {
				cell.opacity(LOCKED_OPACITY)
					.tooltip("Get Nitro to use this emoji everywhere")
			})
			.when(!locked, |cell| {
				cell.cursor(Cursor::Pointer)
					.hover(|style| style.bg(tok("modifier_selected")))
			})
			.on_event(move |event| match event.event {
				InputEvent::PointerEnter => {
					hovered.set(Some(shown.clone()));
					pictures.hover(hover_name.as_ref());
				}
				InputEvent::PointerLeave => pictures.hover(None),
				_ => {}
			})
			.on_click(move || chosen.chose_emoji(cell.clone()))
			.child(visual);
		self.expression_menu(view, target)
	}

	fn emoji_visual(&self, cell: &EmojiCell, side: f32, text_size: &str) -> View {
		match (&cell.glyph, &cell.custom) {
			(Some(glyph), _) => match emoji::svg_of(glyph) {
				Some(art) => svg(glyph.clone(), art).size(side, side),
				None => text(glyph.clone()).text_size(tok(text_size)),
			},
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

	fn emoji_footer(self: &Rc<Self>) -> View {
		let this = self.clone();
		let hovered = self.emoji.hovered.clone();
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
					Some(cell) => this.emoji_preview(&cell),
					None => div(),
				},
			))
			.child(self.tone_button())
	}

	fn emoji_preview(&self, cell: &EmojiCell) -> View {
		row()
			.items_center()
			.gap(8.)
			.min_w(0.)
			.child(self.emoji_visual(cell, PREVIEW, "font_xl"))
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
		let tone = self.emoji.tone.clone();
		let label = self.emoji.tone.clone();
		let open = self.emoji.tone_open.clone();
		let trigger = row()
			.h(24.)
			.px(6.)
			.items_center()
			.rounded(tok("radius_sm"))
			.cursor(Cursor::Pointer)
			.hover(|style| style.bg(tok("modifier_hover")))
			.on_click(move || {
				open.update(|open| *open = !*open);
			})
			.child(switch(
				move || label.get().min(TONES.len() - 1),
				|tone: usize, _| {
					let sample = format!("{TONE_SAMPLE}{}", TONES[tone]);
					match emoji::svg_of(&sample) {
						Some(art) => svg(sample, art).size(TONE_GLYPH, TONE_GLYPH),
						None => text(sample),
					}
				},
			))
			.tooltip("Skin tone");
		menu("Skin tone", self.emoji.tone_open.clone(), trigger)
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
