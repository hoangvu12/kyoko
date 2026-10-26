mod actions;
mod app_modal;
mod attachments;
mod body;
#[cfg(test)]
mod clone_tests;
#[cfg(test)]
mod component_tests;
mod components;
mod dialogs;
#[cfg(test)]
mod emoji_tests;
mod extras;
pub(crate) mod icons;
mod inline;
mod kept;
#[cfg(test)]
mod kept_tests;
mod lightbox;
mod links;
pub(crate) mod pictures;
mod polls;
mod popout;
mod reactions;
mod rows;
mod scroll;
mod unread;
mod uploads;
mod voice;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{
	Access, DeleteProgress, Delta, EmojiPicker, MessageAction, MessageEntry, Row, RowKey, Timeline,
};
use media::{Gallery, ImageName};
use protocol::Snowflake;
use session::Command;
use state::Viewport;
use zgx::compose::prelude::*;
use zgx::compose::{NodeHandle, Tasks, VariableHeights, measured_virtual_list};
use zgx::image::ImageData;
use zgx::input::{EventPhase, InputEvent, Key, PointerButton};
use zgx::platform::PlatformService;
use zgx::reactive::{Effect, Runtime, Signal};

pub use self::actions::Compose;
use self::actions::{MessageActions, Wiring};
use self::dialogs::Dialogs;
pub(crate) use self::icons::{icon, plugin_icon};
use self::lightbox::Lightbox;
use self::links::Links;
pub(crate) use self::pictures::Pictures;
pub use self::rows::Prefs;
use self::rows::{RowEnv, Sender};
pub(crate) use self::rows::{TypeSize, tok};
use self::scroll::{Anchor, Ends, Extent, Page, Paging, Reporter, Resize};
pub(crate) use crate::kit::{MENU_ICON, menu_panel, menu_row};
use crate::pickers::{ExpressionPicker, Mode};

const OVERSCAN: usize = 4;
const PAD_TOP: f32 = 8.;
const PAD_BOTTOM: f32 = 16.;
const DEFAULT_HEIGHT: f32 = 44.;
const AVATAR_MARGIN: usize = 2;
const REPORT_AFTER: Duration = Duration::from_millis(250);

type Callback<T> = RefCell<Option<Box<dyn Fn(T)>>>;

pub struct Setup {
	pub gallery: Gallery,
	pub cdn: String,
	pub mock_origin: Option<String>,
	pub code_font: String,
	pub prefs: Prefs,
	pub store: Option<Arc<store::Store>>,
	pub developer_mode: bool,
	pub pictures: Option<Rc<Pictures>>,
	pub display: bridge::Display,
}

struct Measured<'a>(&'a VariableHeights);

impl Extent for Measured<'_> {
	fn row_offset(&self, index: usize) -> f32 {
		self.0.row_offset(index)
	}

	fn content_height(&self) -> f32 {
		self.0.content_height()
	}

	fn row_at(&self, offset: f32) -> Option<usize> {
		self.0.row_at(offset)
	}
}

pub struct TimelineView {
	runtime: Runtime,
	offset: Signal<f32>,
	heights: VariableHeights,
	keys: Rc<RefCell<Vec<RowKey>>>,
	rows: RefCell<HashMap<RowKey, Signal<Rc<Row>>>>,
	size: Signal<(f32, f32)>,
	channel: Cell<Option<Snowflake>>,
	env: Rc<RowEnv>,
	ends: Cell<Ends>,
	paging: RefCell<Paging>,
	reporter: RefCell<Reporter>,
	pictures: Rc<Pictures>,
	visible_images: RefCell<Vec<ImageName>>,
	tasks: RefCell<Option<Tasks>>,
	pinned: Cell<Option<f32>>,
	layout_watcher: RefCell<Option<Effect>>,
	watcher: RefCell<Option<Effect>>,
	commands: Rc<Sender>,
	visible_changed: Callback<()>,
	actions: Rc<MessageActions>,
	lightbox: Rc<Lightbox>,
	pending_jump: Cell<Option<Snowflake>>,
	pending_action: Cell<Option<MessageAction>>,
	unread: Signal<Option<unread::Unread>>,
	app_modal: Rc<app_modal::AppModal>,
}

impl TimelineView {
	pub fn new(runtime: &Runtime, setup: Setup) -> Rc<Self> {
		let pictures = setup.pictures.clone().unwrap_or_else(|| {
			Rc::new(Pictures::new(
				runtime.clone(),
				setup.gallery,
				setup.cdn,
				setup.mock_origin,
			))
		});
		pictures.set_quality(setup.prefs.quality);
		let commands: Rc<Sender> = Rc::default();
		let links = Rc::new(Links::new(runtime, setup.store));
		let popout = Rc::new(popout::ExpressionPopout::new(runtime, pictures.clone()));
		let actions = Rc::new(MessageActions::new(
			runtime,
			Wiring {
				dialogs: Rc::new(Dialogs::new(runtime, commands.clone(), pictures.clone())),
				reactions: Rc::new(reactions::Reactions::new(
					runtime,
					commands.clone(),
					pictures.clone(),
				)),
				picker: ExpressionPicker::new(runtime, pictures.clone(), Mode::Reaction),
				popout,
				links: links.clone(),
				commands: commands.clone(),
				developer_mode: setup.developer_mode,
			},
		));
		let lightbox = Rc::new(Lightbox::new(runtime, pictures.clone(), links.clone()));
		let view = Rc::new(Self {
			runtime: runtime.clone(),
			offset: runtime.signal(0.),
			heights: VariableHeights::new(runtime, 0, DEFAULT_HEIGHT).anchor_end(),
			keys: Rc::default(),
			rows: RefCell::default(),
			size: runtime.signal((0., 0.)),
			channel: Cell::new(None),
			env: Rc::new(RowEnv {
				pictures: pictures.clone(),
				links,
				channel_name: runtime.signal(String::new()),
				code_font: setup.code_font,
				prefs: Cell::new(setup.prefs),
				revealed: RefCell::default(),
				opened: RefCell::default(),
				picks: RefCell::default(),
				peeks: RefCell::default(),
				commands: commands.clone(),
				actions: actions.clone(),
				lightbox: lightbox.clone(),
				display: setup.display,
				favorite_gifs: runtime.signal(Rc::default()),
				favorites_asked: Cell::new(false),
				gif_frames: RefCell::default(),
				selects: RefCell::default(),
			}),
			ends: Cell::new(Ends {
				has_older: false,
				detached: false,
				oldest: None,
				newest: None,
			}),
			paging: RefCell::default(),
			reporter: RefCell::default(),
			pictures,
			visible_images: RefCell::default(),
			tasks: RefCell::new(None),
			pinned: Cell::new(None),
			layout_watcher: RefCell::new(None),
			watcher: RefCell::new(None),
			commands: commands.clone(),
			visible_changed: RefCell::new(None),
			actions,
			lightbox,
			pending_jump: Cell::new(None),
			pending_action: Cell::new(None),
			unread: runtime.signal(None),
			app_modal: app_modal::AppModal::new(runtime, commands.clone()),
		});
		let jumper = Rc::downgrade(&view);
		view.actions.on_jump(move |message_id| {
			if let Some(view) = jumper.upgrade() {
				view.jump_to(message_id);
			}
		});
		let images = Rc::downgrade(&view);
		view.actions.picker.on_images_changed(move || {
			if let Some(view) = images.upgrade() {
				view.images_changed();
			}
		});
		let images = Rc::downgrade(&view);
		view.actions.popout.on_images_changed(move || {
			if let Some(view) = images.upgrade() {
				view.images_changed();
			}
		});
		let images = Rc::downgrade(&view);
		view.lightbox.on_changed(move || {
			if let Some(view) = images.upgrade() {
				view.images_changed();
			}
		});
		view
	}

	fn images_changed(&self) {
		if let Some(changed) = self.visible_changed.borrow().as_ref() {
			changed(());
		}
	}

	pub fn set_modal(&self, form: Option<bridge::components::ModalForm>) {
		self.app_modal.set(form);
	}

	pub fn modal_open(&self) -> bool {
		self.app_modal.is_open()
	}

	pub fn type_into_modal(&self, custom_id: &str, value: &str) {
		self.app_modal.type_into(custom_id, value);
	}

	pub fn pick_in_select(&self, message_id: Snowflake, custom_id: &str, values: &[&str], open: bool) {
		self.env.pick_in_select(message_id, custom_id, values, open);
	}

	pub fn submit_modal(&self) {
		self.app_modal.submit();
	}

	pub fn lightbox_view(&self) -> View {
		self.lightbox.view()
	}

	pub fn close_lightbox(&self) -> bool {
		self.lightbox.close()
	}

	pub fn open_lightbox(&self, image: bridge::LightboxImage) {
		self.lightbox.open(image);
	}

	pub fn lightbox_settled(&self) -> bool {
		self.lightbox.settled()
	}

	pub fn on_compose(&self, compose: impl Fn(Compose) + 'static) {
		self.actions.on_compose(compose);
	}

	pub fn on_profile(&self, profile: impl Fn((Snowflake, (f32, f32))) + 'static) {
		self.actions.on_profile(profile);
	}

	pub fn on_open_thread(&self, open: impl Fn(Snowflake) + 'static) {
		self.actions.on_open_thread(open);
	}

	pub fn on_create_thread(&self, create: impl Fn((Snowflake, Snowflake)) + 'static) {
		self.actions.on_create_thread(create);
	}

	pub fn on_expression_details(
		&self,
		source: impl Fn(&bridge::PopoutSubject) -> Option<bridge::ExpressionDetails> + 'static,
	) {
		self.actions.popout.on_details(source);
	}

	pub fn on_emoji_catalog(&self, source: impl Fn() -> Option<EmojiPicker> + 'static) {
		self.actions.on_emoji_catalog(source);
	}

	pub fn on_reaction_names(
		&self,
		names: impl Fn(Snowflake, &bridge::ReactionChip) -> Option<Vec<String>> + 'static,
	) {
		self.actions.reactions.on_names(names);
	}

	pub fn on_reaction_lists(
		&self,
		lists: impl Fn(&bridge::ListRequest) -> Option<bridge::ReactionsList> + 'static,
	) {
		self.actions.reactions.on_lists(lists);
	}

	pub fn set_nitro(&self, nitro: bool) {
		self.actions.reactions.set_nitro(nitro);
	}

	pub fn reactions_changed(&self) {
		self.actions.reactions.changed();
	}

	pub fn open_reactions(&self, entry: &MessageEntry, chip: Option<&bridge::ReactionChip>) {
		let moderator = self
			.actions
			.access
			.with_untracked(|access| access.manage_messages);
		self.actions.reactions.open_list(entry, chip, moderator);
	}

	pub fn reactions_settled(&self) -> bool {
		self.actions.reactions.list_settled()
	}

	pub fn reaction_anchor(&self, id: &str) -> Option<zgx::scene::Rect> {
		self.actions.reactions.anchor(id)
	}

	pub fn set_origin(&self, x: f32, y: f32) {
		self.actions.set_origin(x, y);
	}

	pub fn set_window(&self, width: f32, height: f32) {
		self.actions.set_window(width, height);
		self.lightbox.set_window(width, height);
	}

	pub fn set_access(&self, access: Access) {
		if self.actions.access.with_untracked(|current| *current != access) {
			self.actions.access.set(access);
		}
	}

	pub fn set_progress(&self, progress: Option<DeleteProgress>) {
		let shown = self
			.actions
			.progress
			.with_untracked(|current| current.as_ref().map(|shown| shown.batch));
		let progress = progress
			.filter(|progress| progress.total > 1 || progress.refused > 0 || progress.failure.is_some());
		let fresh = progress.as_ref().map(|progress| progress.batch);
		let dismissed = shown.is_none() && progress.as_ref().is_some_and(DeleteProgress::finished);
		if fresh.is_some() && (shown == fresh || !dismissed) {
			self.actions.progress.set(progress);
		}
	}

	pub fn confirm_unpin(&self, channel_id: Snowflake, message_id: Snowflake) {
		self.actions.confirm_unpin(channel_id, message_id);
	}

	pub fn acted_recently(&self) -> bool {
		self.actions.acted_recently()
	}

	pub fn rejected(&self, reason: &str) {
		self.actions.rejected(reason);
	}

	pub fn on_forward(&self, forward: impl Fn((Snowflake, Snowflake)) + 'static) {
		self.actions.on_forward(forward);
	}

	pub fn on_reveal(&self, reveal: impl Fn((Snowflake, MessageAction)) + 'static) {
		self.actions.on_reveal(reveal);
	}

	pub fn aim(&self, target: state::Expression, at: (f32, f32)) {
		self.actions.aim(target, at);
	}

	pub fn set_plugin_menu(&self, menu: Rc<crate::plugin_menu::PluginMenu>) {
		self.actions.set_plugin_menu(menu);
	}

	pub fn open_menu_at(&self, entry: &MessageEntry, access: Access, at: (f32, f32)) -> bool {
		self.actions.open_menu_at(entry, access, at)
	}

	pub fn jump_then(&self, message_id: Snowflake, action: MessageAction) {
		self.pending_action.set(Some(action));
		self.jump_to(message_id);
		if self.pending_jump.get().is_none() {
			self.run_pending(message_id);
		}
	}

	fn run_pending(&self, message_id: Snowflake) {
		let Some(action) = self.pending_action.take() else {
			return;
		};
		let row = self
			.rows
			.borrow()
			.get(&RowKey::Message(message_id))
			.map(|row| row.with_untracked(Rc::clone));
		if let Some(Row::Message(entry)) = row.as_deref() {
			self.actions.run(action, entry, zgx::input::Modifiers::default());
		}
	}

	pub fn jump_to(&self, message_id: Snowflake) {
		if self.scroll_to(&RowKey::Message(message_id)) {
			self.pending_jump.set(None);
			return;
		}
		let Some(channel_id) = self.channel.get() else {
			return;
		};
		self.pending_jump.set(Some(message_id));
		self.send(Command::LoadAround {
			channel_id,
			message_id,
		});
	}

	pub fn on_command(&self, send: impl Fn(Command) + 'static) {
		*self.commands.borrow_mut() = Some(Box::new(send));
	}

	pub fn on_visible_changed(&self, changed: impl Fn(()) + 'static) {
		*self.visible_changed.borrow_mut() = Some(Box::new(changed));
	}

	pub fn resize(&self, width: f32, height: f32) {
		let (_, before) = self.viewport();
		let after = (height - PAD_TOP - PAD_BOTTOM).max(0.);
		let decision = self.runtime.untracked(|| {
			let offset = self.offset.with_untracked(|offset| *offset);
			scroll::resized(&Measured(&self.heights), offset, before, after)
		});
		self.size.set((width, height));
		self.actions.popout.set_bounds(width, height);
		if decision == Resize::FollowEnd {
			self.pinned.set(Some(height));
			self.offset.set(f32::MAX);
		}
	}

	fn follow_layout(self: &Rc<Self>, node: &NodeHandle) {
		let Some(laid_out) = node.observe_size() else {
			return;
		};
		let view = Rc::downgrade(self);
		let effect = self.runtime.effect(move || {
			let (_, height) = laid_out.get();
			let Some(view) = view.upgrade() else {
				return;
			};
			if view
				.pinned
				.get()
				.is_some_and(|wanted| (wanted - height).abs() < 1.)
			{
				view.pinned.set(None);
				view.offset.set(f32::MAX);
			}
		});
		*self.layout_watcher.borrow_mut() = Some(effect);
	}

	pub fn newest_gif(&self) -> Option<zgx::scene::Rect> {
		self.env.gif_frames.borrow().iter().rev().find_map(|handle| {
			let scene = handle.scene()?;
			let scene = scene.borrow();
			scene.contains(handle.node()).then(|| scene.bounds(handle.node()))
		})
	}

	pub fn popout_anchor(
		&self,
		wanted: impl Fn(&bridge::PopoutSubject) -> bool,
	) -> Option<(bridge::PopoutSubject, zgx::scene::Rect)> {
		self.actions.popout.anchor(wanted)
	}

	pub fn run_action(&self, action: bridge::MessageAction, entry: &bridge::MessageEntry) {
		self.actions.run(action, entry, zgx::input::Modifiers::default());
	}

	pub fn open_popout(&self, subject: bridge::PopoutSubject, at: zgx::scene::Rect) {
		self.actions.popout.open(subject, at);
	}

	pub fn set_favorite_gifs(&self, urls: HashSet<String>) {
		if self.env.favorite_gifs.with_untracked(|shown| **shown != urls) {
			self.env.favorite_gifs.set(Rc::new(urls));
		}
	}

	pub fn set_developer_mode(&self, enabled: bool) {
		self.actions.set_developer_mode(enabled);
	}

	pub fn set_prefs(
		self: &Rc<Self>,
		prefs: Prefs,
		trusted: &[String],
		timeline: &Timeline,
		channel_name: &str,
	) {
		self.env.links.set_trusted(trusted, prefs.link_warning);
		if self.env.prefs.replace(prefs) == prefs {
			return;
		}
		self.pictures.set_quality(prefs.quality);
		let delta = Delta {
			updated: timeline.rows().iter().map(|row| row.key.clone()).collect(),
			..Delta::default()
		};
		self.apply(timeline, &delta, channel_name);
		self.visible_images.borrow_mut().clear();
		self.runtime.untracked(|| self.scrolled());
	}

	pub fn pictures(&self) -> Rc<Pictures> {
		self.pictures.clone()
	}

	pub fn set_scale(&self, scale: f64) {
		self.pictures.set_scale(scale);
	}

	pub fn show_image(&self, name: &ImageName, image: Option<Arc<ImageData>>) {
		self.pictures.show(name, image);
	}

	pub fn visible_images(&self) -> Vec<ImageName> {
		let mut names = self.visible_images.borrow().clone();
		names.extend(self.actions.picker.images());
		names.extend(self.lightbox.images());
		names.extend(self.actions.popout.images());
		names
	}

	pub fn settled(&self) -> bool {
		!self.keys.borrow().is_empty()
			&& self
				.visible_images
				.borrow()
				.iter()
				.all(|name| self.pictures.is_settled(name))
	}

	pub fn scroll_to(&self, key: &RowKey) -> bool {
		let index = self.keys.borrow().iter().position(|candidate| candidate == key);
		let Some(index) = index else {
			return false;
		};
		let offset = self.runtime.untracked(|| self.heights.row_offset(index));
		self.offset.set(offset);
		true
	}

	pub fn scroll_to_end(&self, key: &RowKey) -> bool {
		let index = self.keys.borrow().iter().position(|candidate| candidate == key);
		let Some(index) = index else {
			return false;
		};
		let (_, viewport) = self.viewport();
		let offset = self.runtime.untracked(|| {
			let end =
				self.heights.row_offset(index) + self.heights.row_height(index).unwrap_or(DEFAULT_HEIGHT);
			(end - viewport).max(0.)
		});
		self.offset.set(offset);
		true
	}

	pub fn view(self: &Rc<Self>) -> View {
		let this = self.clone();
		component(move |cx| {
			let tasks = cx.tasks();
			this.pictures.listening().run(&tasks);
			*this.tasks.borrow_mut() = Some(tasks);
			this.env.links.attach(cx.try_service::<PlatformService>());
			this.watch();
			let keys = this.keys.clone();
			let builder = Rc::downgrade(&this);
			let list = measured_virtual_list(
				this.offset.clone(),
				this.heights.clone(),
				OVERSCAN,
				move |index| keys.borrow()[index].clone(),
				move |_, key, cx| match builder.upgrade() {
					Some(view) => view.row(&key, cx),
					None => div(),
				},
			)
			.w_full()
			.h_full()
			.pt(PAD_TOP)
			.pb(PAD_BOTTOM)
			.scrollbar(true)
			.selectable(true);
			let mounted = Rc::downgrade(&this);
			let actions = this.actions.clone();
			div()
				.relative()
				.w_full()
				.h_full()
				.on_event(move |event| {
					if event.phase == EventPhase::Capture {
						if let InputEvent::PointerDown { x, y, .. } = event.event {
							actions.popout.pressed(x, y);
						}
						return;
					}
					if let InputEvent::KeyDown { key: Key::Escape, .. } = event.event
						&& (actions.popout.close() || actions.clear_selection())
					{
						event.stop_propagation();
					}
				})
				.child(list)
				.child(this.new_messages_bar())
				.child(this.actions.footer())
				.child(this.actions.context_menu())
				.child(this.actions.picker.view())
				.child(this.env.links.view())
				.child(this.actions.dialogs.view())
				.child(this.app_modal.view())
				.child(this.actions.reactions.view())
				.child(this.actions.popout.view())
				.on_mount(move |node| {
					if let Some(view) = mounted.upgrade() {
						view.actions.popout.set_root(node.clone());
						view.follow_layout(&node);
					}
				})
		})
	}

	fn new_messages_bar(self: &Rc<Self>) -> View {
		let (shown, size) = (self.unread.clone(), self.size.clone());
		let (jumper, reader) = (Rc::downgrade(self), Rc::downgrade(self));
		let actions = unread::Bar {
			jump: Rc::new(move || {
				if let Some(view) = jumper.upgrade() {
					view.scroll_to(&RowKey::Unread);
				}
			}),
			mark_read: Rc::new(move || {
				if let Some(view) = reader.upgrade()
					&& let Some(channel_id) = view.channel.get()
				{
					view.send(Command::MarkChannelRead { channel_id });
				}
			}),
		};
		div().child_when(
			move || shown.get(),
			move |unread: unread::Unread, _| unread::bar(&unread, size.clone(), &actions),
		)
	}

	fn row(&self, key: &RowKey, cx: &mut Context) -> View {
		let Some(signal) = self.rows.borrow().get(key).cloned() else {
			return div();
		};
		let hovered = cx.state(false);
		let bump = cx.state(0u32);
		let tracker = hovered.clone();
		let env = self.env.clone();
		let watched = bump.clone();
		let pressed = signal.clone();
		let actions = self.actions.clone();
		div()
			.w_full()
			.on_event(move |event| match event.event {
				InputEvent::PointerEnter => {
					tracker.set(true);
				}
				InputEvent::PointerLeave => {
					tracker.set(false);
				}
				InputEvent::PointerDown { x, y, button } if event.phase != EventPhase::Capture => {
					actions.pointer_at(x, y);
					if event.default_prevented() {
						return;
					}
					let handled = pressed.with_untracked(|row| match (row.as_ref(), button) {
						(Row::Message(entry), PointerButton::Secondary) => actions.open_menu(entry),
						(Row::Message(entry), PointerButton::Primary) => {
							actions.pressed(entry, event.pointer_modifiers())
						}
						_ => false,
					});
					if handled {
						event.prevent_default();
						event.stop_propagation();
					}
				}
				_ => {}
			})
			.child(switch(
				move || (signal.get(), watched.get()),
				move |(row, _): (Rc<Row>, u32), _| rows::render(&row, &env, &hovered, &bump),
			))
	}

	fn watch(self: &Rc<Self>) {
		if self.watcher.borrow().is_some() {
			return;
		}
		let weak = Rc::downgrade(self);
		let (offset, heights, size) = (self.offset.clone(), self.heights.clone(), self.size.clone());
		let runtime = self.runtime.clone();
		let effect = self.runtime.effect(move || {
			offset.get();
			heights.len();
			size.get();
			if let Some(view) = weak.upgrade() {
				runtime.untracked(|| view.scrolled());
			}
		});
		*self.watcher.borrow_mut() = Some(effect);
	}

	fn viewport(&self) -> (f32, f32) {
		let (width, height) = self.size.with_untracked(|size| *size);
		(width, (height - PAD_TOP - PAD_BOTTOM).max(0.))
	}

	pub fn apply(&self, timeline: &Timeline, delta: &Delta, channel_name: &str) {
		let channel = timeline.open().map(|open| open.channel_id);
		let previous = self.channel.replace(channel);
		let switched = channel != previous;
		if let Some(left) = previous.filter(|_| switched) {
			self.pictures.listening().left(left);
		}
		if switched {
			self.pending_jump.set(None);
			self.pending_action.set(None);
			self.actions.reset();
		}
		let reset = delta.reset || switched;
		let messages = || timeline.messages().filter(|message| !message.pending);
		self.ends.set(Ends {
			has_older: timeline.has_older(),
			detached: timeline.is_detached(),
			oldest: messages().next().map(|message| message.id),
			newest: messages().next_back().map(|message| message.id),
		});
		let (width, viewport) = self.viewport();
		let rows = timeline.rows();
		let unread = unread::Unread::of(rows, timeline.is_detached());
		if self.unread.with_untracked(|shown| *shown != unread) {
			self.unread.set(unread);
		}
		let keys: Vec<RowKey> = rows.iter().map(|row| row.key.clone()).collect();
		let (anchor, heights) = self.runtime.untracked(|| {
			let old = self.keys.borrow();
			let measured = Measured(&self.heights);
			let offset = self.offset.with_untracked(|offset| *offset);
			let following = scroll::at_bottom(&measured, offset, viewport);
			let anchor = (!reset && !following && viewport > 0.)
				.then(|| Anchor::capture(&old, &measured, offset, viewport))
				.flatten();
			let known: HashMap<&RowKey, f32> = old
				.iter()
				.enumerate()
				.filter_map(|(index, key)| Some((key, self.heights.row_height(index)?)))
				.collect();
			let changed: HashSet<&RowKey> = delta.updated.iter().collect();
			let heights: Vec<f32> = rows
				.iter()
				.map(|row| {
					known
						.get(&row.key)
						.copied()
						.filter(|_| !changed.contains(&row.key))
						.unwrap_or_else(|| rows::estimate(&row.row, width))
				})
				.collect();
			(anchor, heights)
		});
		self.runtime.batch(|| {
			self.env.channel_name.set(channel_name.to_owned());
			{
				let mut store = self.rows.borrow_mut();
				let current: HashSet<&RowKey> = keys.iter().collect();
				store.retain(|key, _| current.contains(key));
				for row in rows {
					let value = Rc::new(row.row.clone());
					match store.get(&row.key) {
						Some(signal) => {
							signal.set(value);
						}
						None => {
							store.insert(row.key.clone(), self.runtime.signal(value));
						}
					}
				}
			}
			*self.keys.borrow_mut() = keys;
			if self.heights.replace(heights).is_err() {
				tracing::warn!("the timeline row heights were not usable");
			}
			if reset {
				self.paging.borrow_mut().reset();
				self.reporter.borrow_mut().reset();
				self.offset.set(f32::MAX);
			} else if let Some(anchor) = anchor {
				let restored = self
					.runtime
					.untracked(|| anchor.restore(&self.keys.borrow(), &Measured(&self.heights)));
				if let Some(offset) = restored {
					self.offset.set(offset);
				}
			}
		});
		if let Some(message_id) = self.pending_jump.get()
			&& self.scroll_to(&RowKey::Message(message_id))
		{
			self.pending_jump.set(None);
			self.run_pending(message_id);
		}
	}

	fn scrolled(self: &Rc<Self>) {
		let Some(channel_id) = self.channel.get() else {
			return;
		};
		let (_, viewport) = self.viewport();
		if viewport <= 0. {
			return;
		}
		let measured = Measured(&self.heights);
		let offset = self.offset.with_untracked(|offset| *offset);
		let bottom = scroll::at_bottom(&measured, offset, viewport);
		let Some((first, last)) = scroll::visible(&measured, offset, viewport) else {
			return;
		};
		let (page, report, images) = {
			let keys = self.keys.borrow();
			if keys.is_empty() {
				return;
			}
			let last = last.min(keys.len() - 1);
			tracing::trace!(
				first = ?keys[first],
				top = measured.row_offset(first) - offset.min(scroll::limit(&measured, viewport)),
				bottom,
				rows = keys.len(),
				"timeline scrolled"
			);
			let page =
				self.paging
					.borrow_mut()
					.wanted(self.ends.get(), first, keys.len(), bottom, Instant::now());
			let ids: Vec<Snowflake> = keys[first..=last]
				.iter()
				.filter_map(|key| match key {
					RowKey::Message(id) => Some(*id),
					_ => None,
				})
				.collect();
			let report = match (ids.first(), ids.last()) {
				(Some(&first), Some(&last)) => Some(Viewport {
					first,
					last,
					at_bottom: bottom,
				}),
				_ => None,
			};
			let around = first.saturating_sub(AVATAR_MARGIN)..(last + AVATAR_MARGIN + 1).min(keys.len());
			(page, report, self.images_in(&keys[around]))
		};
		if let Some(page) = page {
			self.send(match page {
				Page::Older => Command::LoadOlder { channel_id },
				Page::Newer => Command::LoadNewer { channel_id },
			});
		}
		if let Some(viewport) = report
			&& self.reporter.borrow_mut().offer(channel_id, viewport)
		{
			self.schedule_report();
		}
		if *self.visible_images.borrow() != images {
			*self.visible_images.borrow_mut() = images;
			if let Some(changed) = self.visible_changed.borrow().as_ref() {
				changed(());
			}
		}
	}

	fn images_in(&self, keys: &[RowKey]) -> Vec<ImageName> {
		let store = self.rows.borrow();
		let inline_media = self.env.prefs.get().inline_media;
		let mut names: Vec<ImageName> = keys
			.iter()
			.filter_map(|key| store.get(key).map(|row| row.with_untracked(Rc::clone)))
			.flat_map(|row| match row.as_ref() {
				Row::Message(entry) => self.pictures.names_in(entry, inline_media),
				_ => Vec::new(),
			})
			.collect();
		names.sort_by(|left, right| left.as_str().cmp(right.as_str()));
		names.dedup();
		names
	}

	fn schedule_report(self: &Rc<Self>) {
		let Some(tasks) = self.tasks.borrow().clone() else {
			return;
		};
		let weak: Weak<Self> = Rc::downgrade(self);
		tasks.spawn(async move {
			zgx::timer::sleep(REPORT_AFTER).await;
			let Some(view) = weak.upgrade() else {
				return;
			};
			let flushed = view.reporter.borrow_mut().flush();
			if let Some((channel_id, viewport)) = flushed
				&& view.channel.get() == Some(channel_id)
			{
				view.send(Command::SetViewport { channel_id, viewport });
			}
		});
	}

	fn send(&self, command: Command) {
		if let Some(send) = self.commands.borrow().as_ref() {
			send(command);
		}
	}
}
