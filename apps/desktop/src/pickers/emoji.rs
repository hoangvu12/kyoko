use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use bridge::{
	EmojiCell, EmojiPicker, EmojiSection, PickerSection, RailGroup, RailIcon, SEARCH_SECTION, TONE_SAMPLE,
	TONES,
};
use protocol::EmojiCategory;
use zgx::compose::VariableHeights;
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::reactive::{Effect, Runtime, Signal};

use super::{
	ExpressionPicker, FOOTER, FOOTER_IMAGE, Favorite, INNER_WIDTH, RAIL, RAIL_ICON, RAIL_ITEM,
	SECTION_HEADER, SECTION_ICON, footer_frame, footer_text, greyed, portrait_shaped, rail_divider,
	rail_frame, section_header,
};
use crate::emoji;
use crate::timeline::{icon, tok};

const COLUMNS: usize = 9;
const CELL: f32 = 48.;
pub(crate) const CELL_IMAGE: f32 = 40.;
const GRID_PADDING: f32 = 8.;
const GRID_WIDTH: f32 = INNER_WIDTH - RAIL;
const TONE_GLYPH: f32 = 24.;
const OVERSCAN: usize = 4;
const SEARCH_ICON: &str = "picker_search";

#[derive(Debug, Clone, PartialEq, Eq)]
enum HeaderIcon {
	Section(RailIcon),
	Search,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
	key: Option<PickerSection>,
	title: String,
	icon: HeaderIcon,
	locked: bool,
	cells: Vec<EmojiCell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
	Header(usize),
	Cells(usize, usize),
}

#[derive(Debug, Default, PartialEq)]
struct Grid {
	shown: Vec<Shown>,
	rows: Vec<Row>,
	anchors: Vec<(f32, PickerSection)>,
}

impl Grid {
	fn build(shown: Vec<Shown>, collapsed: &HashSet<PickerSection>) -> Grid {
		let mut rows = Vec::new();
		let mut anchors = Vec::new();
		let mut top = 0.;
		for (index, section) in shown.iter().enumerate() {
			if let Some(key) = section.key {
				anchors.push((top, key));
			}
			rows.push(Row::Header(index));
			top += SECTION_HEADER;
			if section.key.is_some_and(|key| collapsed.contains(&key)) {
				continue;
			}
			for start in (0..section.cells.len()).step_by(COLUMNS) {
				rows.push(Row::Cells(index, start));
				top += CELL;
			}
		}
		Grid { shown, rows, anchors }
	}

	fn height(row: Row) -> f32 {
		match row {
			Row::Header(_) => SECTION_HEADER,
			Row::Cells(..) => CELL,
		}
	}

	fn offset_of(&self, wanted: PickerSection) -> Option<f32> {
		self.anchors
			.iter()
			.find(|(_, section)| *section == wanted)
			.map(|(top, _)| *top)
	}

	fn current(&self, offset: f32) -> Option<PickerSection> {
		self.anchors
			.iter()
			.take_while(|(top, _)| *top <= offset + 1.)
			.last()
			.or_else(|| self.anchors.first())
			.map(|(_, section)| *section)
	}

	fn cells_between(&self, from: f32, to: f32) -> Vec<&EmojiCell> {
		let mut top = 0.;
		let mut cells = Vec::new();
		for row in &self.rows {
			let height = Grid::height(*row);
			if top + height >= from
				&& top <= to && let Row::Cells(section, start) = row
			{
				let section = &self.shown[*section].cells;
				cells.extend(section[*start..(*start + COLUMNS).min(section.len())].iter());
			}
			top += height;
		}
		cells
	}
}

pub(super) struct EmojiState {
	pub(super) model: Signal<Rc<EmojiPicker>>,
	pub(super) hovered: Signal<Option<EmojiCell>>,
	tone: Signal<usize>,
	tone_open: Signal<bool>,
	collapsed: Signal<HashSet<PickerSection>>,
	offset: Signal<f32>,
	rail_scroll: Signal<f32>,
	grid: Signal<Rc<Grid>>,
	pending: RefCell<Option<PickerSection>>,
	visible: RefCell<Option<Effect>>,
}

impl EmojiState {
	pub(super) fn new(runtime: &Runtime) -> Self {
		Self {
			model: runtime.signal(Rc::default()),
			hovered: runtime.signal(None),
			tone: runtime.signal(0),
			tone_open: runtime.signal(false),
			collapsed: runtime.signal(HashSet::new()),
			offset: runtime.signal(0.),
			rail_scroll: runtime.signal(0.),
			grid: runtime.signal(Rc::default()),
			pending: RefCell::new(None),
			visible: RefCell::new(None),
		}
	}

	pub(super) fn reset(&self) {
		self.hovered.set(None);
		self.offset.set(0.);
		self.rail_scroll.set(0.);
		self.pending.replace(None);
	}

	pub(super) fn first(&self) -> Option<EmojiCell> {
		self.grid.with_untracked(|grid| {
			grid.shown
				.iter()
				.flat_map(|section| section.cells.iter())
				.find(|cell| !cell.locked)
				.cloned()
		})
	}

	pub(super) fn hover_first(&self) {
		let first = self.grid.with_untracked(|grid| {
			grid.shown
				.iter()
				.find_map(|section| section.cells.first().cloned())
		});
		self.hovered.set(first);
	}

	pub(super) fn reveal_locked(&self) {
		let found = self.grid.with_untracked(|grid| {
			grid.shown
				.iter()
				.find(|section| section.locked)
				.and_then(|section| {
					let top = grid.offset_of(section.key?)?;
					let cell = section.cells.iter().find(|cell| cell.locked)?.clone();
					Some((top, cell))
				})
		});
		if let Some((top, cell)) = found {
			self.offset.set(top);
			self.hovered.set(Some(cell));
		}
	}

	pub(super) fn watch(&self, picker: &Rc<ExpressionPicker>) {
		let weak: Weak<ExpressionPicker> = Rc::downgrade(picker);
		let (grid, offset) = (self.grid.clone(), self.offset.clone());
		let height = picker.body_height() - FOOTER;
		let effect = picker.pictures.runtime().effect(move || {
			let top = offset.get();
			let names: Vec<(u64, bool)> = grid.with(|grid| {
				grid.cells_between(top - CELL * 2., top + height + CELL * 2.)
					.into_iter()
					.filter_map(|cell| cell.custom.as_ref())
					.map(|custom| (custom.id.get(), custom.animated))
					.collect()
			});
			if let Some(picker) = weak.upgrade()
				&& picker.tab() == super::Tab::Emoji
			{
				let names = names
					.into_iter()
					.map(|(id, animated)| picker.pictures.emoji(id, animated, f64::from(CELL_IMAGE)))
					.collect();
				picker.shown_images(names);
			}
		});
		*self.visible.borrow_mut() = Some(effect);
	}
}

fn shown_sections(model: &EmojiPicker, query: &str, tone: usize) -> Vec<Shown> {
	if query.trim().is_empty() {
		return model
			.sections(tone)
			.into_iter()
			.map(|section: EmojiSection| Shown {
				key: Some(section.section),
				title: section.title,
				icon: HeaderIcon::Section(section.icon),
				locked: section.locked,
				cells: section.cells,
			})
			.collect();
	}
	let cells = model.search(query, tone);
	vec![Shown {
		key: None,
		title: SEARCH_SECTION.to_owned(),
		icon: HeaderIcon::Search,
		locked: false,
		cells,
	}]
}

fn category_icon(category: EmojiCategory) -> &'static str {
	match category {
		EmojiCategory::People => "picker_people",
		EmojiCategory::Nature => "picker_nature",
		EmojiCategory::Food => "picker_food",
		EmojiCategory::Activities => "picker_activities",
		EmojiCategory::Travel => "picker_travel",
		EmojiCategory::Objects => "picker_objects",
		EmojiCategory::Symbols => "picker_symbols",
		EmojiCategory::Flags => "picker_flags",
	}
}

impl ExpressionPicker {
	pub(super) fn emoji_pane(self: &Rc<Self>) -> View {
		let main = self.body_height() - FOOTER;
		column()
			.w(INNER_WIDTH)
			.h(self.body_height())
			.child(
				row()
					.w(INNER_WIDTH)
					.h(main)
					.child(self.emoji_rail(main))
					.child(self.emoji_grid(main)),
			)
			.child(self.emoji_footer())
	}

	fn rail_icon(&self, icon_kind: &RailIcon, title: &str, side: f32, color: &'static str) -> View {
		match icon_kind {
			RailIcon::Top => icon("picker_trophy", side, color),
			RailIcon::Favorites => icon("picker_star", side, color),
			RailIcon::Recent => icon("picker_clock", side, color),
			RailIcon::Category(category) => icon(category_icon(*category), side, color),
			RailIcon::Guild { guild_id, icon } => {
				portrait_shaped(&self.pictures, *guild_id, icon.as_deref(), title, side, side / 4.)
			}
		}
	}

	fn emoji_rail(self: &Rc<Self>, height: f32) -> View {
		let this = self.clone();
		let model = self.emoji.model.clone();
		scroll(self.emoji.rail_scroll.clone())
			.w(RAIL)
			.h(height)
			.shrink_0()
			.bg(tok("bg_4"))
			.child(switch(
				move || model.get(),
				move |model: Rc<EmojiPicker>, _| {
					let mut items = Vec::new();
					let mut last: Option<RailGroup> = None;
					for entry in model.rail() {
						if last.is_some_and(|group| group != entry.group) {
							items.push(rail_divider());
						}
						last = Some(entry.group);
						items.push(this.emoji_rail_item(entry.section, &entry.icon, &entry.title));
					}
					column().w(RAIL).items_center().gap(8.).py(8.).children(items)
				},
			))
	}

	fn emoji_rail_item(self: &Rc<Self>, target: PickerSection, kind: &RailIcon, title: &str) -> View {
		let (grid, offset, query) = (
			self.emoji.grid.clone(),
			self.emoji.offset.clone(),
			self.query.clone(),
		);
		let current = move || {
			query.with(String::is_empty) && grid.with(|grid| grid.current(offset.get())) == Some(target)
		};
		let active = current.clone();
		let content = match kind {
			RailIcon::Guild { .. } => self.rail_icon(kind, title, RAIL_ITEM, "text_4"),
			_ => {
				let (kind, title, this) = (kind.clone(), title.to_owned(), self.clone());
				switch(active, move |active: bool, _| {
					this.rail_icon(&kind, &title, RAIL_ICON, if active { "text_3" } else { "text_4" })
				})
			}
		};
		let this = self.clone();
		rail_frame(current, content)
			.id(format!("picker-rail-{}", rail_id(target)))
			.on_click(move || this.jump_to(target))
			.tooltip(title.to_owned())
	}

	fn jump_to(&self, target: PickerSection) {
		self.query.set(String::new());
		let collapsed = self
			.emoji
			.collapsed
			.with_untracked(|collapsed| collapsed.contains(&target));
		if collapsed {
			self.emoji.pending.replace(Some(target));
			self.emoji.collapsed.update(|collapsed| {
				collapsed.remove(&target);
			});
			return;
		}
		if let Some(top) = self.emoji.grid.with_untracked(|grid| grid.offset_of(target)) {
			self.emoji.offset.set(top);
		}
	}

	fn emoji_grid(self: &Rc<Self>, height: f32) -> View {
		let this = self.clone();
		let state = &self.emoji;
		let (model, query, tone, collapsed) = (
			state.model.clone(),
			self.query.clone(),
			state.tone.clone(),
			state.collapsed.clone(),
		);
		column().w(GRID_WIDTH).h(height).bg(tok("bg_3")).child(switch(
			move || (model.get(), query.get(), tone.get(), collapsed.get()),
			move |(model, query, tone, collapsed): (
				Rc<EmojiPicker>,
				String,
				usize,
				HashSet<PickerSection>,
			),
			      _| {
				let grid = Rc::new(Grid::build(shown_sections(&model, &query, tone), &collapsed));
				this.emoji.grid.set(grid.clone());
				if let Some(target) = this.emoji.pending.take()
					&& let Some(top) = grid.offset_of(target)
				{
					this.emoji.offset.set(top);
				}
				if !query.trim().is_empty() {
					this.emoji.offset.set(0.);
				}
				this.emoji_list(grid, height)
			},
		))
	}

	fn emoji_list(self: &Rc<Self>, grid: Rc<Grid>, height: f32) -> View {
		let runtime = self.pictures.runtime().clone();
		let heights = VariableHeights::new(&runtime, grid.rows.len().max(1), CELL);
		let _ = heights.replace(grid.rows.iter().map(|row| Grid::height(*row)));
		let (this, built) = (self.clone(), grid.clone());
		variable_virtual_list(
			self.emoji.offset.clone(),
			heights,
			OVERSCAN,
			|index| index,
			move |_, index: usize, _| match built.rows.get(index) {
				Some(Row::Header(section)) => this.emoji_section_header(&built.shown[*section]),
				Some(Row::Cells(section, start)) => {
					let cells = &built.shown[*section].cells;
					let end = (*start + COLUMNS).min(cells.len());
					row().w(GRID_WIDTH).h(CELL).px(GRID_PADDING).children(
						cells[*start..end]
							.iter()
							.cloned()
							.map(|cell| this.emoji_cell(cell)),
					)
				}
				None => div(),
			},
		)
		.w(GRID_WIDTH)
		.h(height)
	}

	fn emoji_section_header(self: &Rc<Self>, section: &Shown) -> View {
		let icon_view = match &section.icon {
			HeaderIcon::Search => icon(SEARCH_ICON, SECTION_ICON, "text_4"),
			HeaderIcon::Section(kind) => self.rail_icon(kind, &section.title, SECTION_ICON, "text_4"),
		};
		let collapsed = section.key.is_some_and(|key| {
			self.emoji
				.collapsed
				.with_untracked(|collapsed| collapsed.contains(&key))
		});
		let toggle = self.emoji.collapsed.clone();
		let key = section.key;
		div().w(GRID_WIDTH).h(SECTION_HEADER).px(GRID_PADDING).child(
			section_header(icon_view, section.title.clone(), section.locked, collapsed).on_click(move || {
				if let Some(key) = key {
					toggle.update(|collapsed| {
						if !collapsed.remove(&key) {
							collapsed.insert(key);
						}
					});
				}
			}),
		)
	}

	fn emoji_cell(self: &Rc<Self>, cell: EmojiCell) -> View {
		let locked = cell.locked;
		let visual = greyed(self.emoji_visual(&cell, CELL_IMAGE), locked);
		let (hovered, styled, chosen) = (
			self.emoji.hovered.clone(),
			self.emoji.hovered.clone(),
			self.clone(),
		);
		let (shown, key) = (cell.clone(), cell.key.clone());
		let hover_name = cell.custom.as_ref().map(|custom| {
			self.pictures
				.emoji(custom.id.get(), custom.animated, f64::from(CELL_IMAGE))
		});
		let pictures = self.pictures.clone();
		let target = bridge::cell_expression(&cell);
		let favorite = Favorite::Emoji(cell.favorite_entry.clone(), !cell.favorite);
		let view = div()
			.id(format!("picker-emoji-{}", cell.key))
			.size(CELL, CELL)
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_sm"))
			.reactive_style(move || {
				if styled.with(|hovered| hovered.as_ref().is_some_and(|over| over.key == key)) {
					Styles::new().bg(tok("active"))
				} else {
					Styles::new()
				}
			})
			.when(!locked, |cell| cell.cursor(Cursor::Pointer))
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
		self.expression_menu(view, target, Some(favorite))
	}

	pub(crate) fn emoji_visual(&self, cell: &EmojiCell, side: f32) -> View {
		match (&cell.glyph, &cell.custom) {
			(Some(glyph), _) => emoji::glyph(glyph.clone(), side),
			(None, Some(custom)) => {
				let picture = self.pictures.picture(&self.pictures.emoji(
					custom.id.get(),
					custom.animated,
					f64::from(CELL_IMAGE),
				));
				div()
					.size(side, side)
					.shrink_0()
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
		footer_frame(switch(
			move || hovered.get(),
			move |hovered: Option<EmojiCell>, _| match hovered {
				Some(cell) => row()
					.grow()
					.min_w(0.)
					.gap(10.)
					.items_center()
					.child(greyed(this.emoji_visual(&cell, FOOTER_IMAGE), cell.locked))
					.child(footer_text(cell.label.clone(), cell.footer_note())),
				None => div(),
			},
		))
	}

	pub(super) fn tone_button(&self) -> View {
		let tone = self.emoji.tone.clone();
		let label = self.emoji.tone.clone();
		let open = self.emoji.tone_open.clone();
		let trigger = div()
			.id("picker-skin-tone")
			.shrink_0()
			.size(TONE_GLYPH, TONE_GLYPH)
			.cursor(Cursor::Pointer)
			.on_click(move || {
				open.update(|open| *open = !*open);
			})
			.child(switch(
				move || label.get().min(TONES.len() - 1),
				|tone: usize, _| emoji::glyph(format!("{TONE_SAMPLE}{}", TONES[tone]), TONE_GLYPH),
			))
			.tooltip("Skin tone");
		menu("Skin tone", self.emoji.tone_open.clone(), trigger)
			.p(4.)
			.rounded(tok("radius_sm"))
			.bg(tok("bg_3"))
			.border(1.)
			.border_color(tok("border"))
			.children(TONES.iter().enumerate().map(move |(index, modifier)| {
				let tone = tone.clone();
				menu_item(format!("{TONE_SAMPLE}{modifier}")).on_click(move || {
					tone.set(index);
				})
			}))
	}
}

fn rail_id(section: PickerSection) -> String {
	match section {
		PickerSection::Top => "top".to_owned(),
		PickerSection::Favorites => "favorites".to_owned(),
		PickerSection::Recent => "recent".to_owned(),
		PickerSection::Guild(id) => format!("guild-{id}"),
		PickerSection::Unicode(index) => format!("unicode-{index}"),
	}
}
