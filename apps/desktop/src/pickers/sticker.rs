use std::rc::Rc;

use bridge::{StickerCell, StickerPicker, StickerSection, StickerSectionIcon, StickerSectionKey};
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::reactive::{Runtime, Signal};

use super::{ExpressionPicker, portrait, section_title};
use crate::timeline::{icon, tok};

const CELL: f32 = 104.;
const CELL_IMAGE: f32 = 96.;
const RAIL_ITEM: f32 = 40.;
const RAIL_ICON: f32 = 32.;
const FOOTER: f32 = 40.;
const LOTTIE_ICON: f32 = 48.;
const LOCKED_OPACITY: f32 = 0.35;
const PANE_WIDTH: f32 = super::WIDTH - 2.;
const GRID_HEIGHT: f32 = super::COMPOSER_PANE_HEIGHT - (8. + RAIL_ITEM + 8.) - FOOTER;

pub(super) struct StickerState {
	pub(super) model: Signal<Rc<StickerPicker>>,
	section: Signal<Option<StickerSectionKey>>,
	hovered: Signal<Option<StickerCell>>,
	scroll: Signal<f32>,
}

impl StickerState {
	pub(super) fn new(runtime: &Runtime) -> Self {
		Self {
			model: runtime.signal(Rc::default()),
			section: runtime.signal(None),
			hovered: runtime.signal(None),
			scroll: runtime.signal(0.),
		}
	}

	pub(super) fn reset(&self) {
		self.section.set(None);
		self.hovered.set(None);
		self.scroll.set(0.);
	}
}

impl ExpressionPicker {
	pub(super) fn sticker_pane(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (model, query, section) = (
			self.sticker.model.clone(),
			self.query.clone(),
			self.sticker.section.clone(),
		);
		column()
			.grow()
			.h_full()
			.min_h(0.)
			.child(switch(
				move || (model.get(), query.get(), section.get()),
				move |(model, query, section): (Rc<StickerPicker>, String, Option<StickerSectionKey>), _| {
					this.sticker_body(&model, &query, section)
				},
			))
			.child(self.sticker_footer())
	}

	fn sticker_body(
		self: &Rc<Self>,
		model: &StickerPicker,
		query: &str,
		section: Option<StickerSectionKey>,
	) -> View {
		let searching = !query.trim().is_empty();
		let active = model.section(section).map(|section| section.key);
		let (title, cells) = if searching {
			("Search Results".to_owned(), model.search(query))
		} else {
			model
				.section(section)
				.map(|section| (section.name.clone(), section.stickers.clone()))
				.unwrap_or_default()
		};
		let mut names: Vec<ImageName> = model
			.sections()
			.iter()
			.filter_map(|section| self.section_icon_name(section))
			.collect();
		names.extend(
			cells
				.iter()
				.filter_map(|cell| self.pictures.sticker_cell(&cell.item(), CELL_IMAGE)),
		);
		self.shown_images(names);
		let empty = cells.is_empty();
		column()
			.grow()
			.min_h(0.)
			.child(
				row()
					.shrink_0()
					.gap(4.)
					.px(12.)
					.py(8.)
					.overflow_hidden()
					.children(model.sections().iter().map(|section| {
						self.sticker_rail_item(section, !searching && Some(section.key) == active)
					})),
			)
			.child(
				scroll(self.sticker.scroll.clone())
					.w(PANE_WIDTH)
					.h(GRID_HEIGHT)
					.child(
						column()
							.w(PANE_WIDTH)
							.px(12.)
							.pb(12.)
							.gap(8.)
							.child(section_title(title))
							.child(
								row()
									.flex_wrap()
									.gap(4.)
									.children(cells.into_iter().map(|cell| self.sticker_cell(cell))),
							)
							.when(empty, |group| {
								group.child(
									text("No stickers here yet.")
										.p(32.)
										.text_size(tok("font_sm"))
										.text_color(tok("text_muted")),
								)
							}),
					),
			)
	}

	fn section_icon_name(&self, section: &StickerSection) -> Option<ImageName> {
		match &section.icon {
			StickerSectionIcon::Guild { guild_id, icon } => icon
				.as_deref()
				.map(|hash| self.pictures.guild_icon(*guild_id, hash, RAIL_ICON)),
			StickerSectionIcon::Sticker(item) => self.pictures.sticker_cell(item, RAIL_ICON),
		}
	}

	fn sticker_rail_item(&self, section: &StickerSection, active: bool) -> View {
		let content = match &section.icon {
			StickerSectionIcon::Guild { guild_id, icon: None } => {
				portrait(&self.pictures, *guild_id, None, &section.name, RAIL_ICON)
			}
			_ => {
				let picture = self
					.section_icon_name(section)
					.map(|name| self.pictures.picture(&name));
				div()
					.size(RAIL_ICON, RAIL_ICON)
					.rounded(tok("radius_sm"))
					.overflow_hidden()
					.when_some(picture, |frame, picture| {
						frame.reactive_style(move || match picture.get() {
							Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
							None => Styles::new(),
						})
					})
			}
		};
		let (target, chosen, scroll) = (
			section.key,
			self.sticker.section.clone(),
			self.sticker.scroll.clone(),
		);
		let query = self.query.clone();
		div()
			.size(RAIL_ITEM, RAIL_ITEM)
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_sm"))
			.cursor(Cursor::Pointer)
			.when(active, |item| item.bg(tok("modifier_selected")))
			.when(!active, |item| {
				item.hover(|style| style.bg(tok("modifier_hover")))
			})
			.on_click(move || {
				chosen.set(Some(target));
				query.set(String::new());
				scroll.set(0.);
			})
			.child(content)
			.tooltip(section.name.clone())
	}

	fn sticker_cell(self: &Rc<Self>, cell: StickerCell) -> View {
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
		let visual = match picture {
			Some((_, picture)) => {
				div()
					.size(CELL_IMAGE, CELL_IMAGE)
					.reactive_style(move || match picture.get() {
						Some(image) => Styles::new().bg_image(image, ObjectFit::Contain),
						None => Styles::new(),
					})
			}
			None => icon("sticker", LOTTIE_ICON, "interactive_muted"),
		};
		div()
			.size(CELL, CELL)
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_md"))
			.reactive_style(move || {
				if styled.get().is_some_and(|hovered| hovered.sticker.id == id) {
					Styles::new().bg(tok("modifier_selected"))
				} else {
					Styles::new()
				}
			})
			.when(locked, |cell| {
				cell.opacity(LOCKED_OPACITY)
					.tooltip("Get Nitro to use this sticker everywhere")
			})
			.when(!locked, |cell| cell.cursor(Cursor::Pointer))
			.on_event(move |event| match event.event {
				InputEvent::PointerEnter => {
					hovered.set(Some(shown.clone()));
					pictures.hover(hover_name.as_ref());
				}
				InputEvent::PointerLeave
					if hovered.with_untracked(|current| {
						current.as_ref().is_some_and(|over| over.sticker.id == id)
					}) =>
				{
					hovered.set(None);
					pictures.hover(None);
				}
				_ => {}
			})
			.on_click(move || chooser.chose_sticker(cell.clone()))
			.child(visual)
	}

	fn sticker_footer(&self) -> View {
		let hovered = self.sticker.hovered.clone();
		row()
			.shrink_0()
			.h(FOOTER)
			.px(12.)
			.gap(8.)
			.items_center()
			.border_edges(zgx::scene::Insets {
				top: 1.,
				..Default::default()
			})
			.border_color(tok("divider"))
			.text_size(tok("font_sm"))
			.child(switch(
				move || hovered.get(),
				move |hovered: Option<StickerCell>, _| match hovered {
					Some(cell) => row()
						.items_center()
						.gap(8.)
						.min_w(0.)
						.child(
							text(cell.sticker.name.clone())
								.font_weight(600)
								.text_color(tok("header_primary")),
						)
						.child(text(cell.tags()).truncate().text_color(tok("text_muted"))),
					None => div(),
				},
			))
	}
}
