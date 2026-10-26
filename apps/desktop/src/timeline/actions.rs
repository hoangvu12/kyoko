use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

use bridge::{
	Access, DeleteProgress, EmojiPicker, MenuContext, MenuEntry, MessageAction, MessageEntry, ReactionToggle,
};
use protocol::Snowflake;
use session::Command;
use state::{Expression, RiskyReason, RiskyTarget};
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::input::{EventPhase, InputEvent, Modifiers, PointerButton};
use zgx::reactive::{Runtime, Signal};

use super::dialogs::{Confirm, Dialogs};
use super::icons::{glyph, icon};
use super::links::Links;
use super::popout::ExpressionPopout;
use super::rows::{Sender, tok};
use crate::pickers::{ExpressionPicker, Tab};
use crate::plugin_menu::{PluginMenu, Shown};
use crate::toasts;

const MENU_WIDTH: f32 = 190.;
const MENU_ITEM: f32 = 28.;
pub const MENU_ICON: f32 = 12.;
const MENU_MARGIN: f32 = 7.;
const MENU_PADDING: f32 = 4.;
const MENU_BORDER: f32 = 1.;
const SEPARATOR: f32 = 7.;
const TOOLBAR_RIGHT: f32 = 12.;
const TOOLBAR_RISE: f32 = 16.;
const TOOLBAR_BUTTON: f32 = 32.;
const TOOLBAR_ICON: f32 = 20.;
const REJECTION_WINDOW: Duration = Duration::from_secs(15);

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;
type Source<T> = RefCell<Option<Box<dyn Fn() -> Option<T>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compose {
	Reply(Snowflake),
	Edit(Snowflake),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldReaction {
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub emoji: String,
	pub target: RiskyTarget,
	pub reason: RiskyReason,
}

#[derive(Clone)]
pub struct ContextMenu {
	serial: u64,
	entry: Rc<MessageEntry>,
	access: Access,
	at: (f32, f32),
	open: Signal<bool>,
	shown: Rc<Shown>,
}

impl PartialEq for ContextMenu {
	fn eq(&self, other: &Self) -> bool {
		self.serial == other.serial
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
	Held(HeldReaction),
	Failed(String),
}

pub struct MessageActions {
	runtime: Runtime,
	pub access: Signal<Access>,
	pub selection: Signal<BTreeSet<Snowflake>>,
	pub progress: Signal<Option<DeleteProgress>>,
	pub notice: Signal<Option<Notice>>,
	context: Signal<Option<ContextMenu>>,
	menus: Cell<u64>,
	pub dialogs: Rc<Dialogs>,
	pub picker: Rc<ExpressionPicker>,
	pub popout: Rc<ExpressionPopout>,
	developer_mode: Cell<bool>,
	links: Rc<Links>,
	commands: Rc<Sender>,
	compose: Hook<Compose>,
	jump: Hook<Snowflake>,
	reveal: Hook<(Snowflake, MessageAction)>,
	forward: Hook<(Snowflake, Snowflake)>,
	profile: Hook<(Snowflake, (f32, f32))>,
	open_thread: Hook<Snowflake>,
	create_thread: Hook<(Snowflake, Snowflake)>,
	emoji: Source<EmojiPicker>,
	pointer: Cell<(f32, f32)>,
	origin: Cell<(f32, f32)>,
	window: Cell<(f32, f32)>,
	acted_at: Cell<Option<Instant>>,
	plugin_menu: RefCell<Option<Rc<PluginMenu>>>,
	aimed: RefCell<Option<(Expression, (f32, f32))>>,
}

pub struct Wiring {
	pub dialogs: Rc<Dialogs>,
	pub picker: Rc<ExpressionPicker>,
	pub popout: Rc<ExpressionPopout>,
	pub links: Rc<Links>,
	pub commands: Rc<Sender>,
	pub developer_mode: bool,
}

impl MessageActions {
	pub fn set_developer_mode(&self, enabled: bool) {
		self.developer_mode.set(enabled);
	}

	pub fn new(runtime: &Runtime, wiring: Wiring) -> Self {
		Self {
			runtime: runtime.clone(),
			access: runtime.signal(Access::default()),
			selection: runtime.signal(BTreeSet::new()),
			progress: runtime.signal(None),
			notice: runtime.signal(None),
			context: runtime.signal(None),
			menus: Cell::new(0),
			dialogs: wiring.dialogs,
			picker: wiring.picker,
			popout: wiring.popout,
			developer_mode: Cell::new(wiring.developer_mode),
			links: wiring.links,
			commands: wiring.commands,
			compose: RefCell::new(None),
			jump: RefCell::new(None),
			reveal: RefCell::new(None),
			forward: RefCell::new(None),
			profile: RefCell::new(None),
			open_thread: RefCell::new(None),
			create_thread: RefCell::new(None),
			emoji: RefCell::new(None),
			pointer: Cell::new((0., 0.)),
			origin: Cell::new((0., 0.)),
			window: Cell::new((f32::MAX, f32::MAX)),
			acted_at: Cell::new(None),
			plugin_menu: RefCell::new(None),
			aimed: RefCell::new(None),
		}
	}

	pub fn set_plugin_menu(&self, menu: Rc<PluginMenu>) {
		self.picker.set_plugin_menu(menu.clone());
		*self.plugin_menu.borrow_mut() = Some(menu);
	}

	pub fn plugin_menu(&self) -> Option<Rc<PluginMenu>> {
		self.plugin_menu.borrow().clone()
	}

	pub fn aim(&self, target: Expression, at: (f32, f32)) {
		*self.aimed.borrow_mut() = Some((target, at));
	}

	fn shown(&self, target: Option<Expression>) -> Shown {
		self.plugin_menu
			.borrow()
			.as_ref()
			.map(|menu| menu.shown(target))
			.unwrap_or_default()
	}

	pub fn on_compose(&self, hook: impl Fn(Compose) + 'static) {
		*self.compose.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_jump(&self, hook: impl Fn(Snowflake) + 'static) {
		*self.jump.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_reveal(&self, hook: impl Fn((Snowflake, MessageAction)) + 'static) {
		*self.reveal.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_forward(&self, hook: impl Fn((Snowflake, Snowflake)) + 'static) {
		*self.forward.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_profile(&self, hook: impl Fn((Snowflake, (f32, f32))) + 'static) {
		*self.profile.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_open_thread(&self, hook: impl Fn(Snowflake) + 'static) {
		*self.open_thread.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_create_thread(&self, hook: impl Fn((Snowflake, Snowflake)) + 'static) {
		*self.create_thread.borrow_mut() = Some(Box::new(hook));
	}

	pub fn confirm_unpin(&self, channel_id: Snowflake, message_id: Snowflake) {
		self.acted_at.set(Some(Instant::now()));
		self.dialogs.ask(Confirm::Pin {
			channel_id,
			message_id,
			pin: false,
		});
	}

	pub fn open_thread(&self, thread_id: Snowflake) {
		match self.open_thread.borrow().as_ref() {
			Some(open) => open(thread_id),
			None => tracing::debug!(%thread_id, "no thread hook"),
		}
	}

	pub fn on_emoji_catalog(&self, source: impl Fn() -> Option<EmojiPicker> + 'static) {
		*self.emoji.borrow_mut() = Some(Box::new(source));
	}

	pub fn set_origin(&self, x: f32, y: f32) {
		self.origin.set((x, y));
	}

	pub fn set_window(&self, width: f32, height: f32) {
		self.window.set((width, height));
	}

	pub fn pointer_at(&self, x: f32, y: f32) {
		self.pointer.set((x, y));
	}

	fn local_pointer(&self) -> (f32, f32) {
		let ((x, y), (left, top)) = (self.pointer.get(), self.origin.get());
		(x - left, y - top)
	}

	fn send(&self, command: Command) {
		self.acted_at.set(Some(Instant::now()));
		if let Some(send) = self.commands.borrow().as_ref() {
			send(command);
		}
	}

	pub fn jump(&self, message_id: Snowflake) {
		if let Some(jump) = self.jump.borrow().as_ref() {
			jump(message_id);
		}
	}

	pub fn profile(&self, user_id: Snowflake) {
		match self.profile.borrow().as_ref() {
			Some(profile) => profile((user_id, self.pointer.get())),
			None => tracing::debug!(%user_id, "no profile hook"),
		}
	}

	fn compose(&self, compose: Compose) {
		match self.compose.borrow().as_ref() {
			Some(hook) => hook(compose),
			None => tracing::debug!(?compose, "no composer to hand the message to"),
		}
	}

	pub fn acted_recently(&self) -> bool {
		self.acted_at
			.get()
			.is_some_and(|at| at.elapsed() < REJECTION_WINDOW)
	}

	pub fn rejected(&self, reason: &str) {
		if self.acted_recently() {
			self.notice.set(Some(Notice::Failed(reason.to_owned())));
		}
	}

	pub fn has_menu_open(&self, message_id: Snowflake) -> bool {
		self.context.with(|context| {
			context
				.as_ref()
				.is_some_and(|context| context.entry.id == message_id && context.open.get())
		})
	}

	pub fn is_selected(&self, message_id: Snowflake) -> bool {
		self.selection.with(|selection| selection.contains(&message_id))
	}

	fn selecting(&self) -> bool {
		self.selection.with_untracked(|selection| !selection.is_empty())
	}

	pub fn toggle_selected(&self, message_id: Snowflake) {
		self.selection.update(|selection| {
			if !selection.remove(&message_id) {
				selection.insert(message_id);
			}
		});
	}

	pub fn clear_selection(&self) -> bool {
		let had = self.selecting();
		if had {
			self.selection.set(BTreeSet::new());
		}
		had
	}

	pub fn reset(&self) {
		self.clear_selection();
		self.notice.set(None);
		self.context.set(None);
		self.picker.close();
		self.popout.close();
	}

	pub fn delete_selected(&self) {
		let Some(channel_id) = self.access.with_untracked(|access| access.channel_id) else {
			return;
		};
		let ids: Vec<Snowflake> = self
			.selection
			.with_untracked(|selection| selection.iter().copied().collect());
		if ids.is_empty() {
			return;
		}
		self.selection.set(BTreeSet::new());
		self.acted_at.set(Some(Instant::now()));
		self.dialogs.ask(Confirm::DeleteMany { channel_id, ids });
	}

	pub fn pressed(&self, entry: &MessageEntry, modifiers: Modifiers) -> bool {
		if !bridge::has_actions(entry) || entry.deleted {
			return false;
		}
		if modifiers.alt {
			self.send(Command::MarkUnread {
				channel_id: entry.message.channel_id,
				message_id: entry.id,
			});
			return true;
		}
		let access = self.access.with_untracked(|access| *access);
		if bridge::can_delete(entry, &access) && (modifiers.shift || self.selecting()) {
			self.toggle_selected(entry.id);
			return true;
		}
		false
	}

	pub fn run(self: &Rc<Self>, action: MessageAction, entry: &MessageEntry, modifiers: Modifiers) {
		let access = self.access.with_untracked(|access| *access);
		self.run_in(action, entry, modifiers, access);
	}

	fn is_open(&self, access: &Access) -> bool {
		self.access
			.with_untracked(|open| open.channel_id.is_some() && open.channel_id == access.channel_id)
	}

	fn act(
		self: &Rc<Self>,
		action: MessageAction,
		entry: &MessageEntry,
		modifiers: Modifiers,
		access: Access,
	) {
		if self.is_open(&access) || !needs_open_channel(action) {
			self.run_in(action, entry, modifiers, access);
			return;
		}
		match self.reveal.borrow().as_ref() {
			Some(reveal) => reveal((entry.id, action)),
			None => tracing::debug!(?action, "nothing opens the message to act on it"),
		}
	}

	fn run_in(
		self: &Rc<Self>,
		action: MessageAction,
		entry: &MessageEntry,
		modifiers: Modifiers,
		access: Access,
	) {
		let channel_id = entry.message.channel_id;
		let message_id = entry.id;
		match action {
			MessageAction::React => self.open_picker(entry),
			MessageAction::Reply => self.compose(Compose::Reply(message_id)),
			MessageAction::Edit => self.compose(Compose::Edit(message_id)),
			MessageAction::CopyText => toasts::copy(&entry.message.content, "Text copied"),
			MessageAction::CopyLink => toasts::copy(
				&bridge::message_link(access.guild_id, channel_id, message_id),
				"Link copied",
			),
			MessageAction::CopyId => toasts::copy(&message_id.to_string(), "ID copied"),
			MessageAction::Forward => {
				if let Some(forward) = self.forward.borrow().as_ref() {
					forward((channel_id, message_id));
				}
			}
			MessageAction::CreateThread => match self.create_thread.borrow().as_ref() {
				Some(create) => create((channel_id, message_id)),
				None => tracing::debug!("no thread creation hook"),
			},
			MessageAction::Pin | MessageAction::Unpin => {
				self.acted_at.set(Some(Instant::now()));
				self.dialogs.ask(Confirm::Pin {
					channel_id,
					message_id,
					pin: action == MessageAction::Pin,
				});
			}
			MessageAction::MarkUnread => self.send(Command::MarkUnread {
				channel_id,
				message_id,
			}),
			MessageAction::Select | MessageAction::Deselect => self.toggle_selected(message_id),
			MessageAction::DeleteSelected => self.delete_selected(),
			MessageAction::Delete if modifiers.shift => self.send(Command::DeleteMessage {
				channel_id,
				message_id,
			}),
			MessageAction::Delete => {
				self.acted_at.set(Some(Instant::now()));
				self.dialogs.ask(Confirm::Delete {
					channel_id,
					message_id,
				});
			}
			MessageAction::Report => self.links.download(bridge::REPORT_URL),
			MessageAction::RemoveEmbeds => self.send(Command::SuppressEmbeds {
				channel_id,
				message_id,
			}),
		}
	}

	fn open_picker(self: &Rc<Self>, entry: &MessageEntry) {
		let model = self.emoji.borrow().as_ref().and_then(|source| source());
		let Some(model) = model else {
			return;
		};
		let actions = Rc::downgrade(self);
		let entry = Rc::new(entry.clone());
		self.picker.set_emoji(model);
		self.picker.on_emoji(move |cell| {
			if let Some(actions) = actions.upgrade() {
				actions.toggle_reaction(&entry, &cell.reaction);
			}
		});
		self.picker.open(Tab::Emoji, self.local_pointer());
	}

	pub fn toggle_reaction(&self, entry: &MessageEntry, emoji: &str) {
		let access = self.access.with_untracked(|access| *access);
		let toggle = bridge::reaction_toggle(entry, emoji);
		if !bridge::reaction_allowed(entry, &access, &toggle) {
			let reason = match toggle {
				ReactionToggle::Add(_) if !access.read_history => {
					"You need the Read Message History permission to react here"
				}
				_ => "You need the Add Reactions permission to react with a new emoji here",
			};
			self.notice.set(Some(Notice::Failed(reason.to_owned())));
			return;
		}
		let (channel_id, message_id) = (entry.message.channel_id, entry.id);
		match toggle {
			ReactionToggle::Add(emoji) => {
				if let Some(lock) = access.lock {
					self.notice.set(Some(Notice::Held(HeldReaction {
						channel_id,
						message_id,
						emoji,
						target: lock.target,
						reason: lock.reason,
					})));
					return;
				}
				self.send(Command::AddReaction {
					channel_id,
					message_id,
					emoji,
				});
			}
			ReactionToggle::Remove(emoji) => self.send(Command::RemoveReaction {
				channel_id,
				message_id,
				emoji,
			}),
		}
	}

	fn release(&self, held: &HeldReaction) {
		self.notice.set(None);
		self.send(Command::ConfirmRisky {
			target: held.target,
			reason: held.reason,
		});
		self.send(Command::AddReaction {
			channel_id: held.channel_id,
			message_id: held.message_id,
			emoji: held.emoji.clone(),
		});
	}

	pub fn open_menu(&self, entry: &MessageEntry) -> bool {
		let access = self.access.with_untracked(|access| *access);
		self.open_menu_at(entry, access, self.pointer.get())
	}

	pub fn open_menu_at(&self, entry: &MessageEntry, access: Access, (x, y): (f32, f32)) -> bool {
		let target = self
			.aimed
			.borrow_mut()
			.take()
			.filter(|(_, aimed)| *aimed == (x, y))
			.map(|(target, _)| target);
		if !bridge::has_actions(entry) {
			return false;
		}
		let shown = Rc::new(self.shown(target));
		let entries = self.menu_entries(entry, &access, &shown);
		if entries.is_empty() {
			return false;
		}
		let serial = self.menus.get() + 1;
		self.menus.set(serial);
		self.pointer.set((x, y));
		let ((left, top), (width, height)) = (self.origin.get(), self.window.get());
		let x = (x - MENU_BORDER).min(width - MENU_MARGIN - MENU_WIDTH).max(0.);
		let y = (y - MENU_BORDER)
			.min(height - MENU_MARGIN - menu_height(&entries))
			.max(0.);
		self.context.set(Some(ContextMenu {
			serial,
			entry: Rc::new(entry.clone()),
			access,
			at: (x - left, y - top),
			open: self.runtime.signal(true),
			shown,
		}));
		true
	}

	pub fn context_menu(self: &Rc<Self>) -> View {
		let this = self.clone();
		div().child_when(
			{
				let context = self.context.clone();
				move || context.get()
			},
			move |menu: ContextMenu, _| {
				let (x, y) = menu.at;
				let spot = div().absolute().left(x).top(y).w(0.).h(0.);
				menu_panel(
					zgx::compose::menu("Message menu", menu.open.clone(), spot).placement(Placement {
						side: PlacementSide::Right,
						align: PlacementEdge::Start,
						offset: 0.,
					}),
				)
				.children(this.menu_views(&menu.entry, menu.access, &menu.shown))
			},
		)
	}

	fn selection_in(&self, access: &Access) -> usize {
		if self.is_open(access) {
			self.selection.with_untracked(BTreeSet::len)
		} else {
			0
		}
	}

	fn menu_entries(&self, entry: &MessageEntry, access: &Access, shown: &Shown) -> Vec<MenuEntry> {
		let context = MenuContext {
			developer_mode: self.developer_mode.get(),
			selected: self.is_open(access)
				&& self
					.selection
					.with_untracked(|selection| selection.contains(&entry.id)),
			selection: self.selection_in(access),
		};
		let mut menu = bridge::message_menu(entry, access, context);
		bridge::append_plugin_entries(&mut menu, &shown.items, &shown.facts);
		menu
	}

	pub fn menu_items(self: &Rc<Self>, entry: &Rc<MessageEntry>, access: Access) -> Vec<View> {
		let shown = self.shown(None);
		self.menu_views(entry, access, &shown)
	}

	fn menu_views(self: &Rc<Self>, entry: &Rc<MessageEntry>, access: Access, shown: &Shown) -> Vec<View> {
		let selection = self.selection_in(&access);
		let menu = self.plugin_menu.borrow().clone();
		let message = Some((entry.message.channel_id, entry.id));
		self.menu_entries(entry, &access, shown)
			.into_iter()
			.map(|item| match (item, menu.as_ref()) {
				(MenuEntry::Separator, _) => separator(),
				(MenuEntry::Action(action), _) => self.menu_item(action, entry, selection, access),
				(MenuEntry::Plugin(index), Some(menu)) => menu.item(shown, index, message, || {}),
				(MenuEntry::Plugin(_), None) => div(),
			})
			.collect()
	}

	fn menu_item(
		self: &Rc<Self>,
		action: MessageAction,
		entry: &Rc<MessageEntry>,
		selection: usize,
		access: Access,
	) -> View {
		let label = action.label(selection);
		let color = if action.is_danger() {
			"danger"
		} else {
			"text_normal"
		};
		let (this, entry) = (self.clone(), entry.clone());
		menu_row(label, icon(action.icon(), MENU_ICON, color), color)
			.on_click_with(move |modifiers| this.act(action, &entry, modifiers, access))
	}

	pub fn toolbar(self: &Rc<Self>, entry: &Rc<MessageEntry>, more_open: Signal<bool>) -> View {
		let access = self.access.with_untracked(|access| *access);
		let buttons = bridge::toolbar_actions(entry, &access).into_iter().map(|action| {
			let (this, entry) = (self.clone(), entry.clone());
			let hovered = self.runtime.signal(false);
			toolbar_button(action.icon(), hovered)
				.id(format!(
					"message-action-{}",
					action.label(0).to_lowercase().replace(' ', "-")
				))
				.on_click_with(move |modifiers| this.run(action, &entry, modifiers))
				.tooltip(action.label(0))
		});
		let toggle = more_open.clone();
		let hovered = self.runtime.signal(false);
		let more = toolbar_button("more", hovered)
			.on_click(move || flip(&toggle))
			.tooltip("More");
		row()
			.absolute()
			.right(TOOLBAR_RIGHT)
			.top(-TOOLBAR_RISE)
			.items_center()
			.rounded(tok("radius_sm"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.bg(tok("bg_3"))
			.children(buttons)
			.child(menu_panel(menu("More", more_open, more)).children(self.menu_items(entry, access)))
	}

	pub fn footer(self: &Rc<Self>) -> View {
		let (selection, progress, notice) =
			(self.selection.clone(), self.progress.clone(), self.notice.clone());
		let this = self.clone();
		column()
			.absolute()
			.left(0.)
			.right(0.)
			.bottom(0.)
			.gap(8.)
			.child_when(move || notice.get(), {
				let this = this.clone();
				move |notice: Notice, _| this.notice_bar(notice)
			})
			.child_when(
				move || {
					let count = selection.with(BTreeSet::len);
					let progress = progress.get();
					(count > 0 || progress.is_some()).then_some((count, progress))
				},
				move |(count, progress): (usize, Option<DeleteProgress>), _| {
					this.selection_bar(count, progress)
				},
			)
	}

	fn selection_bar(self: &Rc<Self>, count: usize, progress: Option<DeleteProgress>) -> View {
		let bar = bar();
		if count > 0 {
			let (clear, delete) = (self.clone(), self.clone());
			return bar
				.child(
					text(format!("{count} selected"))
						.text_size(tok("font_md"))
						.text_color(tok("text_normal")),
				)
				.child(
					row()
						.items_center()
						.gap(8.)
						.child(bar_button("Cancel", None).on_click(move || {
							clear.clear_selection();
						}))
						.child(
							bar_button("Delete", Some("danger")).on_click(move || delete.delete_selected()),
						),
				);
		}
		let Some(progress) = progress else {
			return div();
		};
		let dismiss = self.progress.clone();
		let finished = progress.finished();
		let failed = progress.failure.is_some() || progress.refused > 0;
		bar.child(
			text(progress.summary())
				.text_size(tok("font_md"))
				.text_color(tok(if failed { "danger" } else { "text_normal" })),
		)
		.when(finished, |bar| {
			bar.child(bar_button("Dismiss", None).on_click(move || clear(&dismiss)))
		})
	}

	fn notice_bar(self: &Rc<Self>, notice: Notice) -> View {
		let dismiss = self.notice.clone();
		match notice {
			Notice::Failed(reason) => bar()
				.child(
					text(reason)
						.grow()
						.min_w(0.)
						.text_wrap(true)
						.text_size(tok("font_sm"))
						.text_color(tok("danger")),
				)
				.child(bar_button("Dismiss", None).on_click(move || clear(&dismiss))),
			Notice::Held(held) => {
				let gate = held.reason.is_gate();
				let explanation = if gate {
					format!(
						"{} Finish this in the official Discord client first.",
						held.reason.explanation()
					)
				} else {
					held.reason.explanation().to_owned()
				};
				let this = self.clone();
				bar()
					.child(
						text(explanation)
							.grow()
							.min_w(0.)
							.text_wrap(true)
							.text_size(tok("font_sm"))
							.text_color(tok("text_normal")),
					)
					.child(
						row()
							.items_center()
							.gap(8.)
							.shrink_0()
							.child(bar_button("Cancel", None).on_click(move || clear(&dismiss)))
							.when(!gate, move |buttons| {
								buttons.child(
									bar_button("React anyway", Some("danger"))
										.on_click(move || this.release(&held)),
								)
							}),
					)
			}
		}
	}
}

fn needs_open_channel(action: MessageAction) -> bool {
	matches!(
		action,
		MessageAction::React
			| MessageAction::Reply
			| MessageAction::Edit
			| MessageAction::Select
			| MessageAction::Deselect
			| MessageAction::DeleteSelected
			| MessageAction::CreateThread
	)
}

pub fn aimed(view: View, actions: &Rc<MessageActions>, target: Option<Expression>) -> View {
	let Some(target) = target else {
		return view;
	};
	let actions = actions.clone();
	view.id(format!("expression-{}", target.id()))
		.on_event(move |event| {
			if event.phase != EventPhase::Capture
				&& let InputEvent::PointerDown {
					x,
					y,
					button: PointerButton::Secondary,
				} = event.event
			{
				actions.aim(target.clone(), (x, y));
			}
		})
}

pub fn menu_row(label: String, glyph: View, color: &str) -> View {
	menu_item(label.clone())
		.h(MENU_ITEM)
		.px(12.)
		.py(0.)
		.rounded(tok("radius_sm"))
		.child(
			row()
				.items_center()
				.gap(5.)
				.child(glyph)
				.child(text(label).text_size(tok("font_sm")).text_color(tok(color))),
		)
}

pub fn menu_panel(panel: View) -> View {
	panel
		.w(MENU_WIDTH)
		.px(0.)
		.py(MENU_PADDING)
		.rounded(tok("radius_sm"))
		.bg(tok("background_floating"))
		.border(MENU_BORDER)
		.border_color(tok("divider"))
}

fn menu_height(entries: &[MenuEntry]) -> f32 {
	let rows: f32 = entries
		.iter()
		.map(|entry| match entry {
			MenuEntry::Action(_) | MenuEntry::Plugin(_) => MENU_ITEM,
			MenuEntry::Separator => SEPARATOR,
		})
		.sum();
	rows + 2. * (MENU_PADDING + MENU_BORDER)
}

fn separator() -> View {
	div().h(1.).my((SEPARATOR - 1.) / 2.).bg(tok("divider"))
}

fn toolbar_button(name: &'static str, hovered: Signal<bool>) -> View {
	let tracker = hovered.clone();
	div()
		.size(TOOLBAR_BUTTON, TOOLBAR_BUTTON)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_xs"))
		.cursor(Cursor::Pointer)
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
		.child(glyph(name, TOOLBAR_ICON).reactive_style(move || {
			Styles::new().text_color(tok(if hovered.get() { "text_2" } else { "text_4" }))
		}))
}

fn bar() -> View {
	row()
		.items_center()
		.justify_between()
		.gap(12.)
		.mx(16.)
		.mb(8.)
		.px(16.)
		.py(8.)
		.rounded(tok("radius_md"))
		.bg(tok("background_floating"))
		.border(1.)
		.border_color(tok("divider"))
}

fn bar_button(label: &str, background: Option<&str>) -> View {
	button()
		.h(32.)
		.px(16.)
		.rounded(tok("radius_sm"))
		.text_size(tok("font_sm"))
		.font_weight(500)
		.cursor(Cursor::Pointer)
		.when_some(background, |button, background| {
			button.bg(tok(background)).text_color(tok("text_on_brand"))
		})
		.when(background.is_none(), |button| {
			button
				.bg(tok("background_secondary"))
				.text_color(tok("text_normal"))
		})
		.child(text(label.to_owned()))
}

pub fn flip(signal: &Signal<bool>) {
	signal.update(|value| *value = !*value);
}

fn clear<T: PartialEq + 'static>(signal: &Signal<Option<T>>) {
	signal.set(None);
}
