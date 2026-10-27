use std::cell::RefCell;
use std::rc::{Rc, Weak};

use bridge::{
	SEARCH_SECTION, StickerCell, StickerPicker, StickerSection, StickerSectionIcon, StickerSectionKey,
};
use media::ImageName;
use zgx::compose::VariableHeights;
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::reactive::{Effect, Runtime, Signal};

use super::{
	ExpressionPicker, FOOTER, FOOTER_IMAGE, INNER_WIDTH, RAIL_ICON, RAIL_ITEM, RailRow, SECTION_HEADER,
	SECTION_ICON, Tab, footer_frame, footer_text, greyed, portrait_shaped, rail_beside, rail_frame,
	rail_list, section_header,
};
use crate::timeline::{icon, tok};

const COLUMNS: usize = 4;
const CELL: f32 = 100.;
const CELL_IMAGE: f32 = 84.;
const GRID_PADDING: f32 = 8.;
const GRID_WIDTH: f32 = INNER_WIDTH - super::RAIL;
const LOTTIE_ICON: f32 = 48.;
const OVERSCAN: usize = 2;

#[derive(Debug, Clone, PartialEq)]
struct Shown {
	key: Option<StickerSectionKey>,
	title: String,
	icon: Option<StickerSectionIcon>,
	cells: Vec<StickerCell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
	Header(usize),
	Cells(usize, usize),
	End,
}

#[derive(Debug, Default, PartialEq)]
struct Grid {
	shown: Vec<Shown>,
	rows: Vec<Row>,
	anchors: Vec<(f32, StickerSectionKey)>,
}

impl Grid {
	fn build(shown: Vec<Shown>) -> Grid {
		let mut rows = Vec::new();
		let mut anchors = Vec::new();
		let mut top = 0.;
		for (index, section) in shown.iter().enumerate() {
			if let Some(key) = section.key {
				anchors.push((top, key));
			}
			rows.push(Row::Header(index));
			top += SECTION_HEADER;
			for start in (0..section.cells.len()).step_by(COLUMNS) {
				rows.push(Row::Cells(index, start));
				top += CELL;
			}
		}
		rows.push(Row::End);
		Grid { shown, rows, anchors }
	}

	fn height(row: Row) -> f32 {
		match row {
			Row::Header(_) => SECTION_HEADER,
			Row::Cells(..) => CELL,
			Row::End => GRID_PADDING,
		}
	}

	fn cells(&self, row: Row) -> &[StickerCell] {
		match row {
			Row::Cells(section, start) => {
				let cells = &self.shown[section].cells;
				&cells[start..(start + COLUMNS).min(cells.len())]
			}
			_ => &[],
		}
	}

	fn cells_between(&self, from: f32, to: f32) -> Vec<&StickerCell> {
		let mut top = 0.;
		let mut cells = Vec::new();
		for row in &self.rows {
			let height = Grid::height(*row);
			if top + height >= from && top <= to {
				cells.extend(self.cells(*row));
			}
			top += height;
		}
		cells
	}

	fn empty(&self) -> bool {
		self.shown.iter().all(|section| section.cells.is_empty())
	}
}

pub(super) struct StickerState {
	pub(super) model: Signal<Rc<StickerPicker>>,
	hovered: Signal<Option<(StickerCell, String)>>,
	scroll: Signal<f32>,
	rail_scroll: Signal<f32>,
	grid: Signal<Rc<Grid>>,
	visible: RefCell<Option<Effect>>,
}

impl StickerState {
	pub(super) fn new(runtime: &Runtime) -> Self {
		Self {
			model: runtime.signal(Rc::default()),
			hovered: runtime.signal(None),
			scroll: runtime.signal(0.),
			rail_scroll: runtime.signal(0.),
			grid: runtime.signal(Rc::default()),
			visible: RefCell::new(None),
		}
	}

	pub(super) fn reset(&self) {
		self.hovered.set(None);
		self.scroll.set(0.);
		self.rail_scroll.set(0.);
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

	pub(super) fn watch(&self, picker: &Rc<ExpressionPicker>) {
		let weak: Weak<ExpressionPicker> = Rc::downgrade(picker);
		let (grid, offset) = (self.grid.clone(), self.scroll.clone());
		let (open, tab) = (picker.open.clone(), picker.tab.clone());
		let height = picker.body_height() - FOOTER;
		let effect = picker.pictures.runtime().effect(move || {
			if !open.get() || tab.get() != Tab::Sticker {
				return;
			}
			let top = offset.get();
			let cells: Vec<StickerCell> = grid.with(|grid| {
				grid.cells_between(top, top + height)
					.into_iter()
					.cloned()
					.collect()
			});
			if let Some(picker) = weak.upgrade() {
				let names = cells
					.iter()
					.filter_map(|cell| picker.sticker_image(cell))
					.collect();
				picker.shown_images(names);
			}
		});
		*self.visible.borrow_mut() = Some(effect);
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
			.child(rail_beside(
				self.sticker_rail(main),
				self.sticker_grid(main),
				INNER_WIDTH,
				main,
			))
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
		let (offset, runtime) = (self.sticker.rail_scroll.clone(), self.pictures.runtime().clone());
		div().w(super::RAIL).h(height).child(switch(
			move || model.get(),
			move |model: Rc<StickerPicker>, _| {
				let mut rows = Vec::new();
				let mut recent = false;
				for (index, section) in model.sections().iter().enumerate() {
					if recent {
						rows.push(RailRow::Divider);
					}
					recent = section.key == StickerSectionKey::Recent;
					rows.push(RailRow::Item(index));
				}
				let (this, model) = (this.clone(), model.clone());
				rail_list(&runtime, offset.clone(), height, rows, move |index| {
					this.sticker_rail_item(&model.sections()[index])
				})
			},
		))
	}

	fn sticker_rail_item(self: &Rc<Self>, section: &StickerSection) -> View {
		let target = section.key;
		let (grid, offset, query) = (
			self.sticker.grid.clone(),
			self.sticker.scroll.clone(),
			self.query.clone(),
		);
		let active = move || {
			query.with(String::is_empty)
				&& grid.with(|grid| current(&grid.anchors, offset.get())) == Some(target)
		};
		let side = match section.icon {
			StickerSectionIcon::Recent => RAIL_ICON,
			_ => RAIL_ITEM,
		};
		let content = self.sticker_section_icon(&section.icon, &section.name, side, "text_4");
		let (jump, scroll, query) = (
			self.sticker.grid.clone(),
			self.sticker.scroll.clone(),
			self.query.clone(),
		);
		rail_frame(active, content)
			.on_click(move || {
				query.set(String::new());
				let top = jump.with_untracked(|grid| {
					grid.anchors
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
		let grid = Rc::new(Grid::build(shown_sections(model, query)));
		self.sticker.grid.set(grid.clone());
		if !query.trim().is_empty() {
			self.sticker.scroll.set(0.);
		}
		if grid.empty() {
			return column().w(GRID_WIDTH).h(height).child(
				text("No stickers here yet.")
					.p(32.)
					.text_size(tok("font_size_sm"))
					.text_color(tok("text_4")),
			);
		}
		let heights = VariableHeights::new(self.pictures.runtime(), grid.rows.len(), CELL);
		let _ = heights.replace(grid.rows.iter().map(|row| Grid::height(*row)));
		let this = self.clone();
		variable_virtual_list(
			self.sticker.scroll.clone(),
			heights,
			OVERSCAN,
			|index| index,
			move |_, index: usize, _| match grid.rows.get(index).copied() {
				Some(Row::Header(section)) => this.sticker_section_header(&grid.shown[section]),
				Some(shown @ Row::Cells(section, _)) => {
					let title = grid.shown[section].title.clone();
					row().w(GRID_WIDTH).h(CELL).px(GRID_PADDING).children(
						grid.cells(shown)
							.iter()
							.cloned()
							.map(|cell| this.sticker_cell(cell, title.clone())),
					)
				}
				_ => div().w(GRID_WIDTH).h(GRID_PADDING),
			},
		)
		.w(GRID_WIDTH)
		.h(height)
	}

	fn sticker_section_header(&self, section: &Shown) -> View {
		let icon_view = match &section.icon {
			Some(kind) => self.sticker_section_icon(kind, &section.title, SECTION_ICON, "text_4"),
			None => icon("picker_search", SECTION_ICON, "text_4"),
		};
		let locked = section.cells.iter().any(|cell| cell.locked);
		div()
			.w(GRID_WIDTH)
			.h(SECTION_HEADER)
			.px(GRID_PADDING)
			.child(section_header(icon_view, section.title.clone(), locked, false))
	}
	pub(super) fn sticker_image(&self, cell: &StickerCell) -> Option<ImageName> {
		self.pictures.sticker_cell(&cell.item(), CELL_IMAGE)
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
		self.expression_menu(view, (CELL, CELL), target, None)
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
