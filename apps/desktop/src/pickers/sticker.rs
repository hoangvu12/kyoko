use std::rc::Rc;

use bridge::{
	SEARCH_SECTION, StickerCell, StickerPicker, StickerSection, StickerSectionIcon, StickerSectionKey,
};
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::reactive::{Runtime, Signal};

use super::{
	ExpressionPicker, FOOTER, FOOTER_IMAGE, INNER_WIDTH, RAIL, RAIL_ICON, RAIL_ITEM, SECTION_HEADER,
	SECTION_ICON, footer_frame, footer_text, greyed, portrait_shaped, rail_divider, rail_frame,
	section_header,
};
use crate::timeline::{icon, tok};

const COLUMNS: usize = 4;
const CELL: f32 = 100.;
const CELL_IMAGE: f32 = 84.;
const GRID_PADDING: f32 = 8.;
const GRID_WIDTH: f32 = INNER_WIDTH - RAIL;
const LOTTIE_ICON: f32 = 48.;

#[derive(Debug, Clone, PartialEq)]
struct Shown {
	key: Option<StickerSectionKey>,
	title: String,
	icon: Option<StickerSectionIcon>,
	cells: Vec<StickerCell>,
}

impl Shown {
	fn height(&self) -> f32 {
		SECTION_HEADER + self.cells.len().div_ceil(COLUMNS) as f32 * CELL
	}
}

pub(super) struct StickerState {
	pub(super) model: Signal<Rc<StickerPicker>>,
	hovered: Signal<Option<(StickerCell, String)>>,
	scroll: Signal<f32>,
	anchors: Signal<Rc<Vec<(f32, StickerSectionKey)>>>,
}

impl StickerState {
	pub(super) fn new(runtime: &Runtime) -> Self {
		Self {
			model: runtime.signal(Rc::default()),
			hovered: runtime.signal(None),
			scroll: runtime.signal(0.),
			anchors: runtime.signal(Rc::default()),
		}
	}

	pub(super) fn reset(&self) {
		self.hovered.set(None);
		self.scroll.set(0.);
	}

	pub(super) fn hover_first(&self) {
		let first = self.model.with_untracked(|model| {
			model
				.sections()
				.first()
				.and_then(|section| Some((section.stickers.first()?.clone(), section.name.clone())))
		});
		self.hovered.set(first);
	}
}

fn shown_sections(model: &StickerPicker, query: &str) -> Vec<Shown> {
	if query.trim().is_empty() {
		return model
			.sections()
			.iter()
			.map(|section: &StickerSection| Shown {
				key: Some(section.key),
				title: section.name.clone(),
				icon: Some(section.icon.clone()),
				cells: section.stickers.clone(),
			})
			.collect();
	}
	vec![Shown {
		key: None,
		title: SEARCH_SECTION.to_owned(),
		icon: None,
		cells: model.search(query),
	}]
}

fn current(anchors: &[(f32, StickerSectionKey)], offset: f32) -> Option<StickerSectionKey> {
	anchors
		.iter()
		.take_while(|(top, _)| *top <= offset + 1.)
		.last()
		.or_else(|| anchors.first())
		.map(|(_, key)| *key)
}

impl ExpressionPicker {
	pub(super) fn sticker_pane(self: &Rc<Self>) -> View {
		let main = self.body_height() - FOOTER;
		column()
			.w(INNER_WIDTH)
			.h(self.body_height())
			.child(
				row()
					.w(INNER_WIDTH)
					.h(main)
					.child(self.sticker_rail(main))
					.child(self.sticker_grid(main)),
			)
			.child(self.sticker_footer())
	}

	fn section_icon_name(&self, icon_kind: &StickerSectionIcon, side: f32) -> Option<ImageName> {
		match icon_kind {
			StickerSectionIcon::Guild { guild_id, icon } => icon
				.as_deref()
				.map(|hash| self.pictures.guild_icon(*guild_id, hash, side)),
			StickerSectionIcon::Sticker(item) => self.pictures.sticker_cell(item, side),
			StickerSectionIcon::Recent => None,
		}
	}

	fn sticker_section_icon(
		&self,
		icon_kind: &StickerSectionIcon,
		title: &str,
		side: f32,
		color: &'static str,
	) -> View {
		match icon_kind {
			StickerSectionIcon::Recent => icon("picker_clock", side, color),
			StickerSectionIcon::Guild { guild_id, icon } => {
				portrait_shaped(&self.pictures, *guild_id, icon.as_deref(), title, side, side / 4.)
			}
			StickerSectionIcon::Sticker(_) => {
				let picture = self
					.section_icon_name(icon_kind, side)
					.map(|name| self.pictures.picture(&name));
				div()
					.size(side, side)
					.shrink_0()
					.when_some(picture, |frame, picture| {
						frame.reactive_style(move || match picture.get() {
							Some(image) => Styles::new().bg_image(image, ObjectFit::Contain),
							None => Styles::new(),
						})
					})
			}
		}
	}

	fn sticker_rail(self: &Rc<Self>, height: f32) -> View {
		let this = self.clone();
		let model = self.sticker.model.clone();
		column()
			.w(RAIL)
			.h(height)
			.shrink_0()
			.bg(tok("bg_4"))
			.overflow_hidden()
			.child(switch(
				move || model.get(),
				move |model: Rc<StickerPicker>, _| {
					let mut items = Vec::new();
					let mut recent = false;
					for section in model.sections() {
						if recent {
							items.push(rail_divider());
						}
						recent = section.key == StickerSectionKey::Recent;
						items.push(this.sticker_rail_item(section));
					}
					column().w(RAIL).items_center().gap(8.).py(8.).children(items)
				},
			))
	}

	fn sticker_rail_item(self: &Rc<Self>, section: &StickerSection) -> View {
		let target = section.key;
		let (anchors, offset, query) = (
			self.sticker.anchors.clone(),
			self.sticker.scroll.clone(),
			self.query.clone(),
		);
		let active = move || {
			query.with(String::is_empty)
				&& anchors.with(|anchors| current(anchors, offset.get())) == Some(target)
		};
		let side = match section.icon {
			StickerSectionIcon::Recent => RAIL_ICON,
			_ => RAIL_ITEM,
		};
		let content = self.sticker_section_icon(&section.icon, &section.name, side, "text_4");
		let (jump, scroll, query) = (
			self.sticker.anchors.clone(),
			self.sticker.scroll.clone(),
			self.query.clone(),
		);
		rail_frame(active, content)
			.on_click(move || {
				query.set(String::new());
				let top = jump.with_untracked(|anchors| {
					anchors
						.iter()
						.find(|(_, key)| *key == target)
						.map(|(top, _)| *top)
				});
				if let Some(top) = top {
					scroll.set(top);
				}
			})
			.tooltip(section.name.clone())
	}

	fn sticker_grid(self: &Rc<Self>, height: f32) -> View {
		let this = self.clone();
		let (model, query) = (self.sticker.model.clone(), self.query.clone());
		column().w(GRID_WIDTH).h(height).bg(tok("bg_3")).child(switch(
			move || (model.get(), query.get()),
			move |(model, query): (Rc<StickerPicker>, String), _| this.sticker_body(&model, &query, height),
		))
	}

	fn sticker_body(self: &Rc<Self>, model: &StickerPicker, query: &str, height: f32) -> View {
		let shown = shown_sections(model, query);
		let mut top = 0.;
		let mut anchors = Vec::new();
		for section in &shown {
			if let Some(key) = section.key {
				anchors.push((top, key));
			}
			top += section.height();
		}
		self.sticker.anchors.set(Rc::new(anchors));
		let mut names: Vec<ImageName> = model
			.sections()
			.iter()
			.filter_map(|section| self.section_icon_name(&section.icon, RAIL_ITEM))
			.collect();
		names.extend(
			shown
				.iter()
				.flat_map(|section| section.cells.iter())
				.filter_map(|cell| self.pictures.sticker_cell(&cell.item(), CELL_IMAGE)),
		);
		self.shown_images(names);
		let empty = shown.iter().all(|section| section.cells.is_empty());
		scroll(self.sticker.scroll.clone()).w(GRID_WIDTH).h(height).child(
			column()
				.w(GRID_WIDTH)
				.px(GRID_PADDING)
				.pb(GRID_PADDING)
				.children(shown.into_iter().map(|section| self.sticker_section(section)))
				.when(empty, |grid| {
					grid.child(
						text("No stickers here yet.")
							.p(32.)
							.text_size(tok("font_size_sm"))
							.text_color(tok("text_4")),
					)
				}),
		)
	}

	fn sticker_section(self: &Rc<Self>, section: Shown) -> View {
		let icon_view = match &section.icon {
			Some(kind) => self.sticker_section_icon(kind, &section.title, SECTION_ICON, "text_4"),
			None => icon("picker_search", SECTION_ICON, "text_4"),
		};
		let locked = section.cells.iter().any(|cell| cell.locked);
		let title = section.title.clone();
		column()
			.w(GRID_WIDTH - GRID_PADDING * 2.)
			.child(section_header(icon_view, section.title, locked, false))
			.child(
				row().flex_wrap().children(
					section
						.cells
						.into_iter()
						.map(|cell| self.sticker_cell(cell, title.clone())),
				),
			)
	}

	fn sticker_cell(self: &Rc<Self>, cell: StickerCell, from: String) -> View {
		let picture = self
			.pictures
			.sticker_cell(&cell.item(), CELL_IMAGE)
			.map(|name| (name.clone(), self.pictures.picture(&name)));
		let locked = cell.locked;
		let (hovered, styled, chooser, pictures) = (
			self.sticker.hovered.clone(),
			self.sticker.hovered.clone(),
			self.clone(),
			self.pictures.clone(),
		);
		let (shown, id) = (cell.clone(), cell.sticker.id);
		let hover_name = picture.as_ref().map(|(name, _)| name.clone());
		let target = bridge::sticker_cell_expression(&cell);
		let visual = match picture {
			Some((_, picture)) => div()
				.size(CELL_IMAGE, CELL_IMAGE)
				.rounded(tok("radius_md"))
				.reactive_style(move || match picture.get() {
					Some(image) => Styles::new().bg_image(image, ObjectFit::Contain),
					None => Styles::new(),
				}),
			None => icon("sticker", LOTTIE_ICON, "text_5"),
		};
		let view = div()
			.id(format!("picker-sticker-{id}"))
			.size(CELL, CELL)
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_sm"))
			.reactive_style(move || {
				if styled.with(|hovered| hovered.as_ref().is_some_and(|(over, _)| over.sticker.id == id)) {
					Styles::new().bg(tok("active"))
				} else {
					Styles::new()
				}
			})
			.when(!locked, |cell| cell.cursor(Cursor::Pointer))
			.on_event(move |event| match event.event {
				InputEvent::PointerEnter => {
					hovered.set(Some((shown.clone(), from.clone())));
					pictures.hover(hover_name.as_ref());
				}
				InputEvent::PointerLeave => pictures.hover(None),
				_ => {}
			})
			.on_click(move || chooser.chose_sticker(cell.clone()))
			.child(greyed(visual, locked));
		self.expression_menu(view, target, None)
	}

	fn sticker_footer(&self) -> View {
		let hovered = self.sticker.hovered.clone();
		let pictures = self.pictures.clone();
		footer_frame(switch(
			move || hovered.get(),
			move |hovered: Option<(StickerCell, String)>, _| match hovered {
				Some((cell, from)) => {
					let picture = pictures
						.sticker_cell(&cell.item(), CELL_IMAGE)
						.map(|name| pictures.picture(&name));
					let image = div()
						.size(FOOTER_IMAGE, FOOTER_IMAGE)
						.shrink_0()
						.rounded(tok("radius_xs"))
						.when_some(picture, |frame, picture| {
							frame.reactive_style(move || match picture.get() {
								Some(image) => Styles::new().bg_image(image, ObjectFit::Contain),
								None => Styles::new(),
							})
						});
					let note = cell.lock.map_or_else(|| format!("from {from}"), str::to_owned);
					row()
						.grow()
						.min_w(0.)
						.gap(10.)
						.items_center()
						.child(greyed(image, cell.locked))
						.child(footer_text(cell.sticker.name.clone(), Some(note)))
				}
				None => div(),
			},
		))
	}
}
