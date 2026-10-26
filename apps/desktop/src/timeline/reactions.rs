use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use bridge::{ListRequest, MessageEntry, ReactionChip, ReactionsList, Reactor};
use protocol::{PartialEmoji, Snowflake};
use session::Command;
use zgx::canvas::GradientStop;
use zgx::compose::prelude::*;
use zgx::compose::{NodeHandle, Placement, PlacementEdge, PlacementSide, Tooltip, virtual_list};
use zgx::input::InputEvent;
use zgx::reactive::{Runtime, Signal};
use zgx::scene::{Color, Rect};
use zgx::style::ObjectFit;

use super::Pictures;
use super::actions::aimed;
use super::attachments::picture;
use super::icons::icon;
use super::rows::{RowEnv, Sender, tok};
use crate::{emoji, kit, motion};

const CHIP_EMOJI: f32 = 16.;
const TIP_EMOJI: f32 = 32.;
const TIP_OFFSET: f32 = 8.;
const LIST_EMOJI: f32 = 20.;
const LIST_AVATAR: f32 = 32.;
const ROW_GAP: f32 = 2.;
const ROW_PITCH: f32 = 46.;
const SPARKLE: f32 = 12.;
const TAB_SPARKLE: f32 = 16.;
const ADD_ICON: f32 = 16.;
const REMOVE_ICON: f32 = 16.;
const BORDER: f32 = 2.;
const FILL_ALPHA: u8 = 51;
const OVERSCAN: usize = 4;
const MORE: u64 = u64::MAX;

type Names = Box<dyn Fn(Snowflake, &ReactionChip) -> Option<Vec<String>>>;
type Lists = Box<dyn Fn(&ListRequest) -> Option<ReactionsList>>;
type Asked = (Snowflake, String, bool, Option<Snowflake>);

pub struct Reactions {
	runtime: Runtime,
	revision: Signal<u64>,
	names: RefCell<Option<Names>>,
	lists: RefCell<Option<Lists>>,
	asked: RefCell<HashSet<Asked>>,
	request: Signal<Option<ListRequest>>,
	model: Signal<Option<Rc<ReactionsList>>>,
	open: Signal<bool>,
	offset: Signal<f32>,
	nitro: Cell<bool>,
	commands: Rc<Sender>,
	pictures: Rc<Pictures>,
	anchors: RefCell<Vec<(String, NodeHandle)>>,
}

fn bounds_of(handle: &NodeHandle) -> Option<Rect> {
	let scene = handle.scene()?;
	let scene = scene.borrow();
	scene.contains(handle.node()).then(|| scene.bounds(handle.node()))
}

impl Reactions {
	pub fn new(runtime: &Runtime, commands: Rc<Sender>, pictures: Rc<Pictures>) -> Self {
		Self {
			runtime: runtime.clone(),
			revision: runtime.signal(0),
			names: RefCell::new(None),
			lists: RefCell::new(None),
			asked: RefCell::default(),
			request: runtime.signal(None),
			model: runtime.signal(None),
			open: runtime.signal(false),
			offset: runtime.signal(0.),
			nitro: Cell::new(false),
			commands,
			pictures,
			anchors: RefCell::default(),
		}
	}

	fn remember(&self, id: String, node: NodeHandle) {
		let mut anchors = self.anchors.borrow_mut();
		anchors.retain(|(known, handle)| *known != id && bounds_of(handle).is_some());
		anchors.push((id, node));
	}

	pub fn anchor(&self, id: &str) -> Option<Rect> {
		self.anchors
			.borrow()
			.iter()
			.rev()
			.find(|(known, _)| known == id)
			.and_then(|(_, handle)| bounds_of(handle))
	}

	pub fn on_names(&self, names: impl Fn(Snowflake, &ReactionChip) -> Option<Vec<String>> + 'static) {
		*self.names.borrow_mut() = Some(Box::new(names));
	}

	pub fn on_lists(&self, lists: impl Fn(&ListRequest) -> Option<ReactionsList> + 'static) {
		*self.lists.borrow_mut() = Some(Box::new(lists));
	}

	pub fn set_nitro(&self, nitro: bool) {
		self.nitro.set(nitro);
	}

	pub fn nitro(&self) -> bool {
		self.nitro.get()
	}

	pub fn changed(&self) {
		self.revision.update(|revision| *revision += 1);
		self.refresh();
	}

	pub fn reset(&self) {
		self.asked.borrow_mut().clear();
		self.open.set(false);
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	fn send(&self, command: Command) {
		if let Some(send) = self.commands.borrow().as_ref() {
			send(command);
		}
	}

	fn load(
		&self,
		channel_id: Snowflake,
		message_id: Snowflake,
		chip: &ReactionChip,
		after: Option<Snowflake>,
	) {
		let wanted = (message_id, chip.key.clone(), chip.burst, after);
		if !self.asked.borrow_mut().insert(wanted) {
			return;
		}
		self.send(Command::LoadReactionUsers {
			channel_id,
			message_id,
			emoji: chip.key.clone(),
			burst: chip.burst,
			after,
		});
	}

	fn names_of(&self, message_id: Snowflake, chip: &ReactionChip) -> Option<Vec<String>> {
		self.names.borrow().as_ref()?(message_id, chip)
	}

	pub fn want_names(&self, channel_id: Snowflake, message_id: Snowflake, chip: &ReactionChip) {
		if self.names_of(message_id, chip).is_none() {
			self.load(channel_id, message_id, chip, None);
		}
	}

	pub fn open_list(&self, entry: &MessageEntry, chip: Option<&ReactionChip>, moderator: bool) {
		self.request.set(Some(ListRequest {
			channel_id: entry.message.channel_id,
			message_id: entry.id,
			super_tab: false,
			selected: chip.map(|chip| (chip.key.clone(), chip.burst)),
			moderator,
		}));
		self.offset.set(0.);
		self.open.set(true);
		self.refresh();
	}

	pub fn list_settled(&self) -> bool {
		self.is_open()
			&& self
				.model
				.with_untracked(|model| model.as_ref().is_some_and(|model| model.loaded))
	}

	fn select(&self, change: impl FnOnce(&mut ListRequest)) {
		let Some(mut request) = self.request.with_untracked(Clone::clone) else {
			return;
		};
		change(&mut request);
		self.request.set(Some(request));
		self.offset.set(0.);
		self.refresh();
	}

	fn refresh(&self) {
		if !self.is_open() {
			return;
		}
		let Some(request) = self.request.with_untracked(Clone::clone) else {
			return;
		};
		let model = self.lists.borrow().as_ref().and_then(|lists| lists(&request));
		if let Some(model) = &model
			&& !model.loaded
			&& let Some(chip) = &model.selected
		{
			self.load(request.channel_id, request.message_id, chip, None);
		}
		let model = model.map(Rc::new);
		if self.model.with_untracked(|shown| *shown != model) {
			self.model.set(model);
		}
	}

	fn more(&self) {
		let (Some(request), Some(model)) = (
			self.request.with_untracked(Clone::clone),
			self.model.with_untracked(Clone::clone),
		) else {
			return;
		};
		if let Some(chip) = &model.selected
			&& model.has_more
		{
			self.load(request.channel_id, request.message_id, chip, model.after());
		}
	}

	fn remove(&self, reactor: &Reactor) {
		let (Some(request), Some(model)) = (
			self.request.with_untracked(Clone::clone),
			self.model.with_untracked(Clone::clone),
		) else {
			return;
		};
		let Some(chip) = &model.selected else {
			return;
		};
		self.send(Command::RemoveUserReaction {
			channel_id: request.channel_id,
			message_id: request.message_id,
			emoji: chip.key.clone(),
			burst: chip.burst,
			user_id: reactor.user.id,
		});
	}

	fn tip(self: &Rc<Self>, message_id: Snowflake, chip: &ReactionChip) -> View {
		let (this, shown) = (self.clone(), chip.clone());
		let label = bridge::emoji_label(&chip.emoji);
		let sentence = rich_text_signal(move || {
			this.revision.get();
			let names = this.names_of(message_id, &shown);
			let tip = bridge::reaction_tip(names.as_deref(), shown.count, shown.burst, label.clone());
			vec![
				text_span(tip.who),
				text_span(tip.label).font_weight(600).text_color(tok("text_2")),
			]
		});
		row()
			.w(tok("reaction_tooltip_width"))
			.items_center()
			.gap(tok("space_8"))
			.px(tok("space_12"))
			.py(tok("space_8"))
			.rounded(tok("radius_sm"))
			.bg(tok("bg_3"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.child(emoji_view(&self.pictures, &chip.emoji, TIP_EMOJI))
			.child(
				sentence
					.grow()
					.min_w(0.)
					.text_wrap(true)
					.text_size(tok("font_size_sm"))
					.line_height(tok("line_height_sm"))
					.font_weight(500)
					.text_color(tok("text_3")),
			)
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (this, model, open) = (self.clone(), self.model.clone(), self.open.clone());
		kit::dialog(motion::modal("Reactions", self.open.clone()))
			.w(tok("modal_width_sm"))
			.h(tok("reactions_list_height"))
			.child_when(
				move || if open.get() { model.get() } else { None },
				move |model: Rc<ReactionsList>, _| this.content(&model),
			)
	}

	fn content(self: &Rc<Self>, model: &Rc<ReactionsList>) -> View {
		let super_tab = self
			.request
			.with_untracked(|request| request.as_ref().is_some_and(|request| request.super_tab));
		column()
			.id("reactions-list")
			.w_full()
			.h_full()
			.child(self.tabs(super_tab, model.has_super))
			.child(
				row()
					.w_full()
					.grow()
					.min_h(0.)
					.child(self.column_view(model))
					.child(self.reactor_list(model)),
			)
	}

	fn tabs(self: &Rc<Self>, super_tab: bool, has_super: bool) -> View {
		let tab = |label: &'static str, selected: bool, burst: bool, this: Rc<Self>| {
			row()
				.id(format!("reactions-tab-{}", if burst { "super" } else { "all" }))
				.items_center()
				.gap(tok("space_4"))
				.pt(tok("space_12"))
				.pb(if selected { 10. } else { 12. })
				.cursor(Cursor::Pointer)
				.when(selected, |tab| {
					tab.border_edges(zgx::scene::Insets {
						bottom: BORDER,
						..Default::default()
					})
					.border_color(tok("accent_2"))
				})
				.child(
					text(label)
						.text_size(tok("font_size_md"))
						.line_height(tok("line_height_label"))
						.font_weight(500)
						.text_color(tok(if selected { "text_2" } else { "text_4" })),
				)
				.when(burst, |tab| tab.child(icon("sparkle", TAB_SPARKLE, "purple_2")))
				.on_click(move || {
					this.select(|request| {
						request.super_tab = burst;
						request.selected = None;
					})
				})
		};
		row()
			.w_full()
			.shrink_0()
			.gap(tok("space_24"))
			.px(tok("space_16"))
			.border_edges(zgx::scene::Insets {
				bottom: 1.,
				..Default::default()
			})
			.border_color(tok("border"))
			.child(tab("Reactions", !super_tab, false, self.clone()))
			.when(has_super, |tabs| {
				tabs.child(tab("Super Reactions", super_tab, true, self.clone()))
			})
	}

	fn column_view(self: &Rc<Self>, model: &ReactionsList) -> View {
		let items = model.column.iter().map(|chip| {
			let selected = model.selected.as_ref() == Some(chip);
			let (this, key, burst) = (self.clone(), chip.key.clone(), chip.burst);
			row()
				.id(format!("reactions-column-{}", chip.id()))
				.h(tok("reactions_list_item_height"))
				.shrink_0()
				.items_center()
				.gap(tok("space_8"))
				.px(tok("space_8"))
				.rounded(tok("radius_sm"))
				.cursor(Cursor::Pointer)
				.when(selected, |item| item.bg(tok("active")))
				.when(!selected, |item| item.hover(|style| style.bg(tok("hover"))))
				.child(emoji_view(&self.pictures, &chip.emoji, LIST_EMOJI))
				.child(
					text(chip.count.to_string())
						.grow()
						.text_size(tok("font_size_sm"))
						.line_height(tok("line_height_sm"))
						.font_weight(600)
						.text_color(tok(if selected { "text_2" } else { "text_3" })),
				)
				.when(chip.burst, |item| {
					item.child(icon("sparkle", TAB_SPARKLE, "purple_2"))
				})
				.on_click(move || this.select(|request| request.selected = Some((key.clone(), burst))))
		});
		column()
			.w(tok("reactions_list_column_width"))
			.shrink_0()
			.h_full()
			.p(tok("space_8"))
			.gap(ROW_GAP)
			.border_edges(zgx::scene::Insets {
				right: 1.,
				..Default::default()
			})
			.border_color(tok("border"))
			.children(items)
	}

	fn reactor_list(self: &Rc<Self>, model: &Rc<ReactionsList>) -> View {
		let count = model.reactors.len() + usize::from(model.has_more);
		let (keys, rows) = (model.clone(), model.clone());
		let this = self.clone();
		virtual_list(
			self.offset.clone(),
			ROW_PITCH,
			OVERSCAN,
			move || count,
			move |index| {
				keys.reactors
					.get(index)
					.map_or(MORE, |reactor| reactor.user.id.get())
			},
			move |_, key, _| match rows
				.reactors
				.iter()
				.position(|reactor| reactor.user.id.get() == key)
			{
				Some(index) => this.reactor_row(&rows.reactors[index], index),
				None => {
					this.more();
					div().h(ROW_PITCH)
				}
			},
		)
		.grow()
		.min_w(0.)
		.h_full()
		.p(tok("space_8"))
	}

	fn reactor_row(self: &Rc<Self>, reactor: &Reactor, index: usize) -> View {
		let hovered = self.runtime.signal(false);
		let (tracker, shown) = (hovered.clone(), hovered.clone());
		let (this, gone, keeper) = (self.clone(), reactor.clone(), self.clone());
		row()
			.id(format!("reactor-{}", reactor.user.id))
			.on_mount(move |node| keeper.remember(format!("reactor-row-{index}"), node))
			.w_full()
			.h(tok("reactions_list_row_height"))
			.mb(ROW_GAP)
			.items_center()
			.gap(tok("space_12"))
			.px(tok("space_8"))
			.rounded(tok("radius_sm"))
			.hover(|style| style.bg(tok("hover")))
			.on_event(move |event| match event.event {
				InputEvent::PointerEnter => {
					tracker.set(true);
				}
				InputEvent::PointerLeave => {
					tracker.set(false);
				}
				_ => {}
			})
			.child(self.avatar(reactor))
			.child(
				row()
					.grow()
					.min_w(0.)
					.items_baseline()
					.gap(tok("space_6"))
					.overflow_hidden()
					.child(
						text(reactor.name.clone())
							.shrink_0()
							.text_size(tok("font_size_lg"))
							.line_height(tok("line_height_label"))
							.font_weight(500)
							.text_color(tok("text_2")),
					)
					.child(
						text(reactor.username.clone())
							.min_w(0.)
							.truncate()
							.text_size(tok("font_size_md"))
							.line_height(tok("line_height_label"))
							.text_color(tok("text_4")),
					),
			)
			.when(reactor.removable, move |row| {
				row.child_when(
					move || shown.get().then_some(()),
					move |(), _| {
						let (this, gone) = (this.clone(), gone.clone());
						button()
							.id(format!("reactor-remove-{}", gone.user.id))
							.w(tok("button_size"))
							.h(tok("button_size"))
							.shrink_0()
							.p(0.)
							.border(0.)
							.items_center()
							.justify_center()
							.bg(kit::clear())
							.cursor(Cursor::Pointer)
							.child(icon("close", REMOVE_ICON, "text_3"))
							.on_click(move || this.remove(&gone))
							.tooltip("Remove Reaction")
					},
				)
			})
	}

	fn avatar(&self, reactor: &Reactor) -> View {
		let circle = div()
			.size(LIST_AVATAR, LIST_AVATAR)
			.shrink_0()
			.rounded(LIST_AVATAR / 2.)
			.overflow_hidden()
			.bg(tok("bg_3"));
		match self.pictures.avatar(&reactor.user, f64::from(LIST_AVATAR)) {
			Some(name) => {
				let picture = self.pictures.picture(&name);
				circle.reactive_style(move || match picture.get() {
					Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
					None => Styles::new(),
				})
			}
			None => circle,
		}
	}
}

pub fn emoji_view(pictures: &Rc<Pictures>, emoji: &PartialEmoji, side: f32) -> View {
	match pictures.reaction(emoji) {
		Some(name) => {
			div()
				.size(side, side)
				.shrink_0()
				.child(picture(pictures, Some(name), None, ObjectFit::Contain))
		}
		None => emoji::glyph(emoji.name.clone().unwrap_or_default(), side),
	}
}

fn theme_color(env: &RowEnv, name: &str) -> Color {
	zgx::theme::Themes::of(env.pictures.runtime())
		.get(tok(name))
		.color()
		.unwrap_or(Color(0, 0, 0, 0))
}

fn rgb_color(value: u32) -> Color {
	Color((value >> 16) as u8, (value >> 8) as u8, value as u8, 255)
}

fn burst_ends(chip: &ReactionChip, env: &RowEnv) -> (Color, Color) {
	match (chip.colors.first(), chip.colors.last()) {
		(Some(start), Some(end)) => (rgb_color(*start), rgb_color(*end)),
		_ => (theme_color(env, "purple_2"), theme_color(env, "blue_2")),
	}
}

fn stops(start: Color, end: Color, alpha: Option<u8>) -> Vec<GradientStop> {
	let fade = |Color(red, green, blue, opaque): Color| Color(red, green, blue, alpha.unwrap_or(opaque));
	vec![
		GradientStop {
			offset: 0.,
			color: fade(start),
		},
		GradientStop {
			offset: 1.,
			color: fade(end),
		},
	]
}

fn shell(content: View) -> View {
	content
		.relative()
		.shrink_0()
		.h(tok("reaction_height"))
		.items_center()
		.gap(tok("space_6"))
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
}

fn count_text(count: i64, me: bool) -> View {
	text(count.to_string())
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_xs"))
		.font_weight(600)
		.text_color(tok(if me { "text_1" } else { "text_3" }))
}

fn normal_chip(chip: &ReactionChip, env: &RowEnv) -> View {
	let (rest, lit, edge, edge_lit) = if chip.me {
		(
			"reaction_background_reacted",
			"reaction_background_reacted_hover",
			"accent_2",
			"accent_2",
		)
	} else {
		("hover", "active", "border_light", "bg_2")
	};
	shell(row())
		.px(tok("space_6"))
		.border(tok("reaction_border_width"))
		.border_color(tok(edge))
		.bg(tok(rest))
		.hover(move |style| style.bg(tok(lit)).border_color(tok(edge_lit)))
		.child(emoji_view(&env.pictures, &chip.emoji, CHIP_EMOJI))
		.child(count_text(chip.count, chip.me))
}

fn super_chip(chip: &ReactionChip, env: &RowEnv) -> View {
	let (start, end) = burst_ends(chip, env);
	let inner_radius = (theme_size(env, "radius_sm") - BORDER).max(0.);
	let fill = row()
		.h_full()
		.items_center()
		.gap(tok("space_6"))
		.px(tok("space_6"))
		.rounded(inner_radius)
		.child(emoji_view(&env.pictures, &chip.emoji, CHIP_EMOJI))
		.child(count_text(chip.count, chip.me))
		.child(icon("sparkle", SPARKLE, "purple_2"));
	let fill = if chip.me {
		fill.bg_gradient(FRAC_PI_2, stops(start, end, Some(FILL_ALPHA)))
	} else {
		fill.bg(tok("hover")).hover(|style| style.bg(tok("active")))
	};
	shell(row())
		.p(BORDER)
		.bg_gradient(FRAC_PI_2, stops(start, end, None))
		.child(div().h_full().rounded(inner_radius).bg(tok("bg_4")).child(fill))
}

fn theme_size(env: &RowEnv, name: &str) -> f32 {
	zgx::theme::Themes::of(env.pictures.runtime())
		.get(tok(name))
		.number()
		.unwrap_or(0.)
}

fn add_button(id: &str, glyph: &'static str, label: &str, action: impl FnMut() + 'static) -> View {
	shell(row())
		.id(id.to_owned())
		.px(tok("space_8"))
		.border(tok("reaction_border_width"))
		.border_color(tok("border_light"))
		.bg(tok("hover"))
		.hover(|style| style.bg(tok("active")).border_color(tok("bg_2")))
		.child(icon(glyph, ADD_ICON, "text_4"))
		.on_click(action)
		.tooltip(label.to_owned())
}

pub fn chips(entry: &MessageEntry, env: &Rc<RowEnv>, hovered: &Signal<bool>) -> Option<View> {
	let chips = bridge::reaction_chips(&entry.message);
	if chips.is_empty() || !env.prefs.get().show_reactions {
		return None;
	}
	let shared = Rc::new(entry.clone());
	let reactions = env.actions.reactions.clone();
	let views = chips.iter().map(|chip| {
		let view = if chip.burst {
			super_chip(chip, env)
		} else {
			normal_chip(chip, env)
		};
		let (channel_id, message_id) = (entry.message.channel_id, entry.id);
		let (asker, wanted) = (reactions.clone(), chip.clone());
		let (keeper, anchor) = (reactions.clone(), format!("reaction-{}-{}", entry.id, chip.id()));
		let view = view
			.id(anchor.clone())
			.on_mount(move |node| keeper.remember(anchor.clone(), node))
			.on_event(move |event| {
				if let InputEvent::PointerEnter = event.event {
					asker.want_names(channel_id, message_id, &wanted);
				}
			})
			.when(!entry.deleted && !entry.pending, |view| {
				let (actions, entry, key, burst) =
					(env.actions.clone(), shared.clone(), chip.key.clone(), chip.burst);
				view.on_click(move || actions.toggle_reaction_of(&entry, &key, burst))
			});
		let view = aimed(view, &env.actions, bridge::reaction_expression(&chip.emoji));
		view.tooltip(Tooltip::from(reactions.tip(entry.id, chip)).placement(Placement {
			side: PlacementSide::Top,
			align: PlacementEdge::Start,
			offset: TIP_OFFSET,
		}))
	});
	let (actions, shown, entry) = (env.actions.clone(), hovered.clone(), shared.clone());
	let nitro = reactions.nitro();
	Some(
		row()
			.flex_wrap()
			.gap(tok("space_4"))
			.pt(tok("space_4"))
			.pb(tok("space_2"))
			.children(views)
			.child_when(
				move || {
					let access = actions.access.with_untracked(|access| *access);
					(shown.get() && bridge::can_add_reaction(&entry, &access)).then_some(())
				},
				{
					let (actions, entry) = (env.actions.clone(), shared);
					move |(), _| {
						let access = actions.access.with_untracked(|access| *access);
						let (normal, burst) = (actions.clone(), actions.clone());
						let (first, second) = (entry.clone(), entry.clone());
						row()
							.gap(tok("space_4"))
							.child(add_button(
								&format!("reaction-add-{}", entry.id),
								"add_reaction",
								"Add Reaction",
								move || normal.open_reaction_picker(&first, false),
							))
							.when(
								bridge::can_add_super_reaction(&entry, &access, nitro),
								|buttons| {
									buttons.child(add_button(
										&format!("reaction-add-super-{}", entry.id),
										"add_super_reaction",
										"Add Super Reaction",
										move || burst.open_reaction_picker(&second, true),
									))
								},
							)
					}
				},
			),
	)
}
