use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use bridge::{
	Access, ChannelKind, DeleteProgress, EmojiPicker, ForumCard, ForumSort, Inbox, OpenChannel, TagChip,
	ThreadHeader, Timeline, today,
};
use chrono::Utc;
use protocol::Snowflake;
use rest::UploadFile;
use session::{Command, Formatting};
use state::{Changes, State};
use zgx::compose::prelude::*;
use zgx::reactive::{Runtime, Signal};
use zgx_script::{ScriptHost, Shared, Value};

use super::composer::{self, ComposerView, Pane, content_type};
use super::{Link, Slot, field, number, record, snowflake, text};
use crate::capture;
use crate::profile::Anchor;
use crate::threads::{PinsShown, Request, ThreadsLayer, ThreadsShown};
use crate::timeline::{Compose, Setup, TimelineView};

const FORUM_AVATAR: f64 = 24.;
const STARTER_AVATAR: f64 = 40.;
const STARTER_TEXT: usize = 260;
const MAX_POST_FILES: usize = 10;

#[derive(Debug, Clone, PartialEq)]
struct ThreadDraft {
	serial: u64,
	channel_id: Snowflake,
	message_id: Option<Snowflake>,
	name: String,
	author: String,
	time: String,
	text: String,
	avatar: String,
	can_private: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct PanelShown {
	header: Option<ThreadHeader>,
	draft: Option<ThreadDraft>,
	post: bool,
	tags: Vec<String>,
}
pub(super) const THREAD_SLOT: &str = "thread";

pub(super) struct ThreadPane {
	timeline: RefCell<Timeline>,
	view: Rc<TimelineView>,
	composer: ComposerView,
	slot: Signal<Slot>,
	reported: Cell<Slot>,
	panel: Shared,
	header: RefCell<Option<ThreadHeader>>,
	shown: RefCell<Option<PanelShown>>,
	draft: RefCell<Option<ThreadDraft>>,
	drafts: Cell<u64>,
	first_message: RefCell<Option<(Snowflake, String)>>,
	forum: Cell<Option<OpenChannel>>,
	forum_shown: Signal<bool>,
	sort: Cell<ForumSort>,
	gallery: Cell<bool>,
	filter: RefCell<HashSet<Snowflake>>,
	forum_view: Shared,
	forum_files: Shared,
	composing: Cell<bool>,
	post_serial: Cell<u64>,
	draft_title: RefCell<String>,
	uploads: RefCell<Vec<UploadFile>>,
	forum_tags: Shared,
	forum_cards: Shared,
	forum_published: RefCell<Option<(Vec<ForumCard>, Vec<TagChip>)>>,
	forum_avatars: RefCell<Vec<String>>,
	channel_icon: Shared,
	layer: Rc<ThreadsLayer>,
	files: Inbox<UploadFile>,
	follow: Cell<Option<Snowflake>>,
}

impl ThreadPane {
	pub(super) fn set_plugin_menu(&self, menu: Rc<crate::plugin_menu::PluginMenu>) {
		self.view.set_plugin_menu(menu);
	}

	pub(super) fn new(
		host: &ScriptHost,
		runtime: &Runtime,
		handle: tokio::runtime::Handle,
		setup: Setup,
	) -> Self {
		let mut timeline = Timeline::default();
		timeline.set_display(setup.display.clone());
		let view = TimelineView::new(runtime, setup);
		let layer = ThreadsLayer::new(runtime, view.pictures());
		Self {
			timeline: RefCell::new(timeline),
			view,
			composer: ComposerView::install(host, handle, &composer::THREAD),
			slot: runtime.signal(Slot::default()),
			reported: Cell::new(Slot::default()),
			panel: host.shared("threads.panel", closed_panel()),
			header: RefCell::new(None),
			shown: RefCell::new(None),
			draft: RefCell::new(None),
			drafts: Cell::new(0),
			first_message: RefCell::new(None),
			forum: Cell::new(None),
			forum_shown: runtime.signal(false),
			sort: Cell::new(ForumSort::Activity),
			gallery: Cell::new(false),
			filter: RefCell::default(),
			forum_view: host.shared("forum.view", closed_forum()),
			forum_files: host.shared("forum.files", Value::List(Rc::from(Vec::new()))),
			composing: Cell::new(false),
			post_serial: Cell::new(0),
			draft_title: RefCell::new(String::new()),
			uploads: RefCell::default(),
			forum_tags: host.shared("forum.tags", Value::List(Rc::from(Vec::new()))),
			forum_cards: host.shared("forum.cards", Value::List(Rc::from(Vec::new()))),
			forum_published: RefCell::new(None),
			forum_avatars: RefCell::default(),
			channel_icon: host.shared("timeline.channel_icon", "hash"),
			layer,
			files: Inbox::new(),
			follow: Cell::new(None),
		}
	}

	pub(super) fn view(&self) -> View {
		self.view.view()
	}

	pub(super) fn layer_view(&self) -> View {
		self.layer.view()
	}

	pub(super) fn forum_shown(&self) -> Signal<bool> {
		self.forum_shown.clone()
	}

	pub(super) fn slot(&self) -> Signal<Slot> {
		self.slot.clone()
	}
}

fn closed_panel() -> Value {
	panel_map(closed_fields())
}

fn panel_map(fields: Vec<(&'static str, Value)>) -> Value {
	let map: std::collections::BTreeMap<&str, Value> = fields.into_iter().collect();
	Value::Map(
		map.into_iter()
			.map(|(key, value)| (Rc::from(key), value))
			.collect(),
	)
}

fn closed_fields() -> Vec<(&'static str, Value)> {
	vec![
		("open", false.into()),
		("id", "".into()),
		("name", "".into()),
		("members", 0.0.into()),
		("archived", false.into()),
		("joined", true.into()),
		("can_manage", false.into()),
		("creating", false.into()),
		("serial", 0.0.into()),
		("draft_name", "".into()),
		("starter_author", "".into()),
		("starter_time", "".into()),
		("starter_text", "".into()),
		("starter_avatar", "".into()),
		("can_private", false.into()),
		("post", false.into()),
		("tags", Value::List(Rc::from(Vec::new()))),
	]
}

fn panel_value(shown: &PanelShown) -> Value {
	let mut fields = match (&shown.header, &shown.draft) {
		(Some(header), _) => vec![
			("open", true.into()),
			("id", header.id.to_string().as_str().into()),
			("name", header.name.as_str().into()),
			("members", (header.members as f64).into()),
			("archived", header.archived.into()),
			("joined", header.joined.into()),
			("can_manage", header.can_manage.into()),
		],
		(None, Some(_)) => vec![("open", true.into())],
		(None, None) => return closed_panel(),
	};
	let draft = shown.draft.as_ref().filter(|_| shown.header.is_none());
	if let Some(draft) = draft {
		fields.extend([
			("creating", true.into()),
			("serial", (draft.serial as f64).into()),
			("draft_name", draft.name.as_str().into()),
			("starter_author", draft.author.as_str().into()),
			("starter_time", draft.time.as_str().into()),
			("starter_text", draft.text.as_str().into()),
			("starter_avatar", draft.avatar.as_str().into()),
			("can_private", draft.can_private.into()),
		]);
	}
	let tags: Vec<Value> = shown.tags.iter().map(|tag| tag.as_str().into()).collect();
	fields.extend([("post", shown.post.into()), ("tags", Value::List(tags.into()))]);
	let mut all = closed_fields();
	all.extend(fields);
	panel_map(all)
}

fn closed_forum() -> Value {
	record([
		("open", false.into()),
		("id", "".into()),
		("sort", ForumSort::Activity.label().into()),
		("gallery", false.into()),
		("composing", false.into()),
		("serial", 0.0.into()),
		("draft_title", "".into()),
		("require_tag", false.into()),
	])
}

pub(super) fn thread_slot(main: Signal<Slot>, slot: Signal<Slot>, transcript: View) -> View {
	div().child(transcript).reactive_style(move || {
		let shown = main.get().width > 0. && main.get().height > 0.;
		let slot = if shown { slot.get() } else { Slot::default() };
		Styles::new()
			.absolute()
			.left(slot.x)
			.top(slot.y)
			.w(slot.width)
			.h(slot.height)
	})
}

fn point(argument: &Value) -> (f32, f32) {
	(
		number(argument, "x").unwrap_or_default(),
		number(argument, "y").unwrap_or_default(),
	)
}

impl Link {
	pub(super) fn threads_acted_recently(&self) -> bool {
		self.thread_open().is_some() && self.threads.view.acted_recently()
	}

	pub(super) fn thread_composer(&self) -> &ComposerView {
		&self.threads.composer
	}

	pub(super) fn thread_open(&self) -> Option<OpenChannel> {
		self.threads.timeline.borrow().open()
	}

	pub(super) fn forum_open(&self) -> Option<OpenChannel> {
		self.threads.forum.get()
	}

	pub(super) fn view_showing(&self, channel_id: Snowflake) -> Option<Rc<TimelineView>> {
		if self.thread_open().map(|open| open.channel_id) == Some(channel_id) {
			return Some(self.threads.view.clone());
		}
		(self.timeline.borrow().open().map(|open| open.channel_id) == Some(channel_id))
			.then(|| self.timeline_view.clone())
	}

	pub(super) fn install_threads(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("threads.open_list", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.open_thread_list(point(&argument));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("threads.open_pins", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.open_pins(point(&argument));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("threads.close", move |_| {
			if let Some(link) = actions.upgrade() {
				link.close_thread_pane(true);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("threads.create", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let read = |key: &str| {
				field(&argument, key)
					.and_then(text)
					.unwrap_or_default()
					.to_owned()
			};
			let private = matches!(field(&argument, "private"), Some(Value::Bool(true)));
			link.create_from_draft(&read("name"), &read("message"), private);
		});
		let actions = Rc::downgrade(link);
		host.action("threads.join", move |argument| {
			if let (Some(link), Some(thread_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.send(Command::JoinThread { thread_id });
			}
		});
		let actions = Rc::downgrade(link);
		host.action("threads.leave", move |argument| {
			if let (Some(link), Some(thread_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.send(Command::LeaveThread { thread_id });
			}
		});
		let actions = Rc::downgrade(link);
		host.action("threads.archive", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let archived = matches!(field(&argument, "archived"), Some(Value::Bool(true)));
			if let Some(thread_id) = field(&argument, "id").and_then(snowflake) {
				link.send(Command::ArchiveThread { thread_id, archived });
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.new_post", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.open_new_post(text(&argument).unwrap_or_default());
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.cancel_post", move |_| {
			if let Some(link) = actions.upgrade() {
				link.close_new_post();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.attach", move |_| {
			if let Some(link) = actions.upgrade() {
				link.threads.layer.pick_files();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.remove_file", move |argument| {
			if let (Some(link), Value::Num(index)) = (actions.upgrade(), argument) {
				link.remove_post_file(index as usize);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.post", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let read = |key: &str| {
				field(&argument, key)
					.and_then(text)
					.unwrap_or_default()
					.to_owned()
			};
			let tags = match field(&argument, "tags") {
				Some(Value::List(tags)) => tags.iter().filter_map(snowflake).collect(),
				_ => Vec::new(),
			};
			link.submit_post(&read("title"), &read("body"), tags);
		});
		let actions = Rc::downgrade(link);
		host.action("forum.open_post", move |argument| {
			if let (Some(link), Some(post_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.open_thread(post_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.toggle_tag", move |argument| {
			if let (Some(link), Some(tag_id)) = (actions.upgrade(), snowflake(&argument)) {
				{
					let mut filter = link.threads.filter.borrow_mut();
					if !filter.remove(&tag_id) {
						filter.insert(tag_id);
					}
				}
				link.refresh_forum();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.sort", move |argument| {
			if let Some(link) = actions.upgrade() {
				let sort = match argument {
					Value::Num(index) if index >= 2. => ForumSort::Creation,
					_ => ForumSort::Activity,
				};
				link.threads.sort.set(sort);
				link.refresh_forum();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.layout", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.threads.gallery.set(matches!(argument, Value::Bool(true)));
				link.refresh_forum();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("forum.more", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(forum) = link.forum_open()
			{
				link.send(Command::LoadForumPosts {
					channel_id: forum.channel_id,
					more: true,
				});
			}
		});
		Link::wire_thread_view(link);
		Link::wire_layer(link);
		for view in [link.timeline_view.clone(), link.threads.view.clone()] {
			let actions = Rc::downgrade(link);
			view.on_open_thread(move |thread_id| {
				if let Some(link) = actions.upgrade() {
					link.open_thread(thread_id);
				}
			});
			let actions = Rc::downgrade(link);
			view.on_create_thread(move |(channel_id, message_id)| {
				if let Some(link) = actions.upgrade() {
					link.open_create_thread(channel_id, Some(message_id));
				}
			});
		}
	}

	fn wire_thread_view(link: &Rc<Link>) {
		let view = link.threads.view.clone();
		let actions = Rc::downgrade(link);
		view.on_command(move |command| {
			if let Some(link) = actions.upgrade() {
				link.send(command);
			}
		});
		let actions = Rc::downgrade(link);
		view.on_visible_changed(move |()| {
			if let Some(link) = actions.upgrade() {
				link.refresh_visible();
			}
		});
		let actions = Rc::downgrade(link);
		view.on_compose(move |compose| {
			if let Some(link) = actions.upgrade() {
				match compose {
					Compose::Reply(message_id) => link.pane_reply(Pane::Thread, message_id),
					Compose::Edit(message_id) => link.pane_edit(Pane::Thread, message_id),
				}
			}
		});
		let actions = Rc::downgrade(link);
		view.on_profile(move |(user_id, at)| {
			if let Some(link) = actions.upgrade() {
				link.open_profile(user_id, at, Anchor::TopLeft);
			}
		});
		let actions = Rc::downgrade(link);
		view.on_emoji_catalog(move || {
			let link = actions.upgrade()?;
			let shared = link.state()?;
			let guild_id = link.thread_open()?.guild_id;
			Some(EmojiPicker::of(&shared.read(), guild_id))
		});
		let actions = Rc::downgrade(link);
		view.on_reaction_names(move |message_id, chip| {
			let link = actions.upgrade()?;
			let shared = link.state()?;
			let guild_id = link.thread_open()?.guild_id;
			bridge::tip_names(&shared.read(), guild_id, message_id, chip)
		});
		let actions = Rc::downgrade(link);
		view.on_reaction_lists(move |request| {
			let link = actions.upgrade()?;
			let shared = link.state()?;
			let guild_id = link.thread_open()?.guild_id;
			bridge::ReactionsList::of(&shared.read(), guild_id, request)
		});
	}

	fn wire_layer(link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		link.threads.layer.on_request(move |request| {
			if let Some(link) = actions.upgrade() {
				link.layer_request(request);
			}
		});
		let actions = Rc::downgrade(link);
		link.threads.layer.on_images_changed(move |()| {
			if let Some(link) = actions.upgrade() {
				link.refresh_visible();
			}
		});
	}

	pub(super) async fn follow_post_files(self: Rc<Self>) {
		while let Some(batch) = self.threads.files.next().await {
			for file in batch {
				self.add_post_file(file);
			}
		}
	}

	fn layer_request(&self, request: Request) {
		match request {
			Request::OpenThread(thread_id) => {
				self.threads.layer.close_popout();
				self.open_thread(thread_id);
			}
			Request::NewThread(channel_id) => self.open_create_thread(channel_id, None),
			Request::Jump {
				channel_id,
				message_id,
			} => {
				self.threads.layer.close_popout();
				match self.thread_open() {
					Some(open) if open.channel_id == channel_id => self.threads.view.jump_to(message_id),
					_ => self.timeline_view.jump_to(message_id),
				}
			}
			Request::Unpin {
				channel_id,
				message_id,
			} => self.timeline_view.confirm_unpin(channel_id, message_id),
			Request::ReadFiles(paths) => self.read_post_files(paths),
		}
	}

	fn read_post_files(&self, paths: Vec<PathBuf>) {
		for path in paths {
			let files = self.threads.files.clone();
			self.composer.runtime().spawn_blocking(move || {
				let name = path
					.file_name()
					.map(|name| name.to_string_lossy().into_owned())
					.unwrap_or_else(|| "file".to_owned());
				match std::fs::read(&path) {
					Ok(bytes) => files.push(UploadFile::new(name.clone(), bytes, content_type(&name))),
					Err(error) => tracing::debug!(%error, "a post attachment could not be read"),
				}
			});
		}
	}

	pub(super) fn route_channel(&self, channel_id: Snowflake) -> bool {
		let kind = {
			let Some(shared) = self.state() else {
				return true;
			};
			let state = shared.read();
			let Some(channel) = state.channel(channel_id) else {
				return false;
			};
			ChannelKind::of(channel.kind)
		};
		match kind {
			ChannelKind::Thread => {
				self.open_thread(channel_id);
				true
			}
			ChannelKind::Forum => {
				let selectable = self
					.sidebar
					.sidebar()
					.channel(channel_id)
					.is_some_and(|channel| channel.selectable());
				if selectable {
					self.open_forum(channel_id);
				}
				true
			}
			_ => {
				let reselect = self.timeline.borrow().open().map(|open| open.channel_id) == Some(channel_id);
				if self.thread_open().is_some() {
					self.close_thread_pane(reselect);
				}
				if self.forum_open().is_some() {
					self.leave_forum();
				}
				false
			}
		}
	}

	pub(crate) fn open_thread(&self, thread_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let (guild_id, parent, parent_is_forum) = {
			let state = shared.read();
			let Some(thread) = state.channel(thread_id) else {
				return;
			};
			if !bridge::is_thread(thread.kind) {
				return;
			}
			let parent = thread.parent_id;
			let forum = parent
				.and_then(|parent| state.channel(parent))
				.is_some_and(|parent| bridge::is_forum(parent.kind));
			(state.channel_guild(thread_id), parent, forum)
		};
		let Some(guild_id) = guild_id else {
			return;
		};
		if self.sidebar.sidebar().selected() != Some(guild_id) {
			self.select_guild(guild_id);
		}
		if let Some(parent) = parent {
			if parent_is_forum {
				if self.forum_open().map(|open| open.channel_id) != Some(parent) {
					self.open_forum(parent);
				}
			} else if self.timeline.borrow().open().map(|open| open.channel_id) != Some(parent) {
				if self.thread_open().is_some() {
					self.close_thread_pane(false);
				}
				self.select_channel(parent);
			}
		}
		let command = {
			let state = shared.read();
			let command = self
				.threads
				.timeline
				.borrow_mut()
				.select(&state, Some(guild_id), thread_id);
			let selected = state.selected().map(|selection| selection.channel_id) == Some(thread_id);
			self.window.runtime.batch(|| self.publish_thread(&state));
			command.filter(|_| !selected)
		};
		if let Some(command) = command {
			self.send(command);
		}
		self.send_first_message(thread_id, parent);
	}

	pub(super) fn close_thread_pane(&self, reselect: bool) {
		self.threads.draft.borrow_mut().take();
		if !self.threads.timeline.borrow_mut().close() && self.thread_open().is_none() {
			self.publish_thread_closed();
			return;
		}
		self.publish_thread_closed();
		if !reselect {
			return;
		}
		let back = self.timeline.borrow().open().or(self.forum_open());
		if let Some(open) = back {
			self.send(Command::SelectChannel {
				guild_id: open.guild_id,
				channel_id: open.channel_id,
			});
		}
	}

	fn publish_thread_closed(&self) {
		let Some(shared) = self.state() else {
			return;
		};
		let state = shared.read();
		self.window.runtime.batch(|| self.publish_thread(&state));
	}

	fn open_forum(&self, forum_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let guild_id = shared.read().channel_guild(forum_id);
		let Some(guild_id) = guild_id else {
			return;
		};
		if self.sidebar.sidebar().selected() != Some(guild_id) {
			self.select_guild(guild_id);
		}
		if self.thread_open().is_some() {
			self.close_thread_pane(false);
		}
		let open = OpenChannel {
			guild_id: Some(guild_id),
			channel_id: forum_id,
		};
		self.threads.forum.set(Some(open));
		self.threads.filter.borrow_mut().clear();
		self.threads.gallery.set(false);
		{
			let state = shared.read();
			self.threads.sort.set(bridge::default_sort(&state, forum_id));
			self.timeline.borrow_mut().close();
			self.window.runtime.batch(|| {
				self.threads.forum_shown.set(true);
				self.publish_timeline(&state);
				self.publish_forum(&state);
			});
		}
		self.send(Command::SelectChannel {
			guild_id: Some(guild_id),
			channel_id: forum_id,
		});
		self.send(Command::LoadForumPosts {
			channel_id: forum_id,
			more: false,
		});
	}

	pub(super) fn leave_forum(&self) {
		if self.threads.forum.take().is_none() {
			return;
		}
		self.threads.composing.set(false);
		self.threads.uploads.borrow_mut().clear();
		self.publish_post_files();
		self.threads.forum_published.borrow_mut().take();
		self.threads.forum_shown.set(false);
		self.threads.forum_view.set(closed_forum());
		self.set_forum_avatars(Vec::new());
	}

	pub(super) fn sync_threads(&self, open: Option<OpenChannel>) {
		let guild = self.sidebar.sidebar().selected();
		if let Some(forum) = self.forum_open()
			&& (open.is_some_and(|open| open.channel_id != forum.channel_id) || forum.guild_id != guild)
		{
			self.leave_forum();
		}
		let main_guild = self
			.timeline
			.borrow()
			.open()
			.and_then(|open| open.guild_id)
			.or(self.forum_open().and_then(|open| open.guild_id));
		if let Some(thread) = self.thread_open()
			&& (main_guild.is_none() || thread.guild_id != main_guild)
		{
			self.threads.timeline.borrow_mut().close();
			self.threads.header.borrow_mut().take();
			self.threads.shown.borrow_mut().take();
			self.threads.panel.set(closed_panel());
			self.threads.slot.set(Slot::default());
		}
		let stale = self
			.threads
			.draft
			.borrow()
			.as_ref()
			.is_some_and(|draft| open.is_some_and(|open| open.channel_id != draft.channel_id));
		if stale {
			self.drop_draft();
		}
	}

	pub(super) fn top_channel(&self, open: Option<OpenChannel>) -> Option<OpenChannel> {
		open.or(self.forum_open())
	}

	pub(super) fn publish_channel_icon(&self, state: &State, open: Option<OpenChannel>) {
		let icon = match open {
			Some(open) if open.guild_id.is_none() => "at_sign",
			Some(open) => state
				.channel(open.channel_id)
				.map(|channel| bridge::channel_icon(channel.kind))
				.unwrap_or("hash"),
			None => "hash",
		};
		self.threads.channel_icon.set(icon.into());
	}

	fn open_thread_list(&self, at: (f32, f32)) {
		let Some(open) = self.timeline.borrow().open() else {
			return;
		};
		if open.guild_id.is_none() {
			return;
		}
		self.size_layer();
		self.threads.layer.open_threads(open.channel_id, at);
		self.send(Command::LoadArchivedThreads {
			channel_id: open.channel_id,
			more: false,
		});
		self.refresh_layer();
	}

	fn open_pins(&self, at: (f32, f32)) {
		let Some(open) = self.thread_open().or(self.timeline.borrow().open()) else {
			return;
		};
		self.size_layer();
		self.threads.layer.open_pins(open.channel_id, at);
		self.send(Command::LoadPins {
			channel_id: open.channel_id,
		});
		self.refresh_layer();
	}

	pub(super) fn open_create_thread(&self, channel_id: Snowflake, message_id: Option<Snowflake>) {
		let Some(shared) = self.state() else {
			return;
		};
		self.threads.layer.close_popout();
		if self.thread_open().is_some() {
			self.close_thread_pane(false);
		}
		let serial = self.threads.drafts.get() + 1;
		self.threads.drafts.set(serial);
		let draft = {
			let state = shared.read();
			let mut draft = ThreadDraft {
				serial,
				channel_id,
				message_id,
				name: message_id
					.and_then(|message_id| bridge::starter_preview(&state, channel_id, message_id))
					.unwrap_or_default(),
				author: String::new(),
				time: String::new(),
				text: String::new(),
				avatar: String::new(),
				can_private: message_id.is_none() && bridge::can_create_private_threads(&state, channel_id),
			};
			let starter = message_id.and_then(|message_id| {
				state
					.messages(channel_id)
					.and_then(|cache| cache.by_id.get(&message_id).cloned())
			});
			if let Some(message) = starter {
				let guild_id = state.channel_guild(channel_id);
				let entry = bridge::detached_entry(&state, guild_id, &message);
				draft.author = entry.author.clone();
				draft.time = bridge::message_time(entry.sent_at, today());
				draft.text = bridge::snippet(&entry.body.content, &entry.names, STARTER_TEXT);
				draft.avatar = self
					.timeline_view
					.pictures()
					.avatar(&message.author, STARTER_AVATAR)
					.map(|name| format!("host:{name}"))
					.unwrap_or_default();
			}
			draft
		};
		*self.threads.draft.borrow_mut() = Some(draft);
		self.publish_thread_closed();
		self.refresh_visible();
	}

	fn drop_draft(&self) {
		if self.threads.draft.borrow_mut().take().is_some() {
			self.publish_thread_closed();
			self.refresh_visible();
		}
	}

	fn draft_settled(&self) -> bool {
		let shown = self.shown.borrow();
		self.draft_images().iter().all(|name| shown.contains(name))
	}

	pub(super) fn thread_drafting(&self) -> bool {
		self.threads.draft.borrow().is_some()
	}

	pub(super) fn draft_images(&self) -> Vec<String> {
		self.threads
			.draft
			.borrow()
			.iter()
			.filter_map(|draft| draft.avatar.strip_prefix("host:").map(str::to_owned))
			.collect()
	}

	fn create_from_draft(&self, name: &str, message: &str, private: bool) {
		let Some(draft) = self.threads.draft.borrow_mut().take() else {
			return;
		};
		let message = message.trim().to_owned();
		let mut name = bridge::trimmed_name(name);
		if name.is_empty() {
			name = bridge::trimmed_name(&message);
		}
		if name.is_empty() {
			name = bridge::trimmed_name(&draft.name);
		}
		if name.is_empty() {
			*self.threads.draft.borrow_mut() = Some(draft);
			return;
		}
		if !message.is_empty() {
			*self.threads.first_message.borrow_mut() = Some((draft.channel_id, message));
		}
		self.send(Command::CreateThread {
			channel_id: draft.channel_id,
			message_id: draft.message_id,
			name,
			auto_archive_minutes: bridge::ARCHIVE_CHOICES[bridge::DEFAULT_ARCHIVE_CHOICE].0,
			private: private && draft.can_private,
		});
		self.publish_thread_closed();
		self.refresh_visible();
	}

	pub(super) fn await_first_message(&self, parent: Snowflake, content: String) {
		*self.threads.first_message.borrow_mut() = Some((parent, content));
	}

	fn send_first_message(&self, thread_id: Snowflake, parent: Option<Snowflake>) {
		let waiting = self
			.threads
			.first_message
			.borrow()
			.as_ref()
			.is_some_and(|(channel_id, _)| Some(*channel_id) == parent);
		if !waiting {
			return;
		}
		if let Some((_, content)) = self.threads.first_message.borrow_mut().take() {
			self.send(Command::SendMessage {
				channel_id: thread_id,
				content,
				reply_to: None,
				ping_reply: false,
				files: Vec::new(),
			});
		}
	}

	fn open_new_post(&self, title: &str) {
		if self.forum_open().is_none() {
			return;
		}
		self.threads.post_serial.set(self.threads.post_serial.get() + 1);
		*self.threads.draft_title.borrow_mut() = title.trim().to_owned();
		self.threads.uploads.borrow_mut().clear();
		self.threads.composing.set(true);
		self.publish_post_files();
		self.refresh_forum();
	}

	fn close_new_post(&self) {
		self.threads.composing.set(false);
		self.threads.uploads.borrow_mut().clear();
		self.publish_post_files();
		self.refresh_forum();
	}

	pub(super) fn forum_composing(&self) -> bool {
		self.threads.composing.get()
	}

	fn add_post_file(&self, file: UploadFile) {
		{
			let mut uploads = self.threads.uploads.borrow_mut();
			if uploads.len() >= MAX_POST_FILES {
				return;
			}
			uploads.push(file);
		}
		self.publish_post_files();
	}

	fn remove_post_file(&self, index: usize) {
		{
			let mut uploads = self.threads.uploads.borrow_mut();
			if index < uploads.len() {
				uploads.remove(index);
			}
		}
		self.publish_post_files();
	}

	fn publish_post_files(&self) {
		let files: Vec<Value> = self
			.threads
			.uploads
			.borrow()
			.iter()
			.enumerate()
			.map(|(index, file)| {
				record([
					("key", format!("{index}-{}", file.name).as_str().into()),
					("name", file.name.as_str().into()),
					("size", bridge::file_size(file.bytes.len() as u64).as_str().into()),
				])
			})
			.collect();
		self.threads.forum_files.set(Value::List(files.into()));
	}

	fn submit_post(&self, title: &str, body: &str, tags: Vec<Snowflake>) {
		let (Some(forum), Some(shared)) = (self.forum_open(), self.state()) else {
			return;
		};
		let name = bridge::trimmed_name(title);
		let form = bridge::new_post_form(&shared.read(), forum.channel_id);
		if name.is_empty() || (form.require_tag && tags.is_empty()) {
			return;
		}
		let files = std::mem::take(&mut *self.threads.uploads.borrow_mut());
		self.send(Command::CreateForumPost {
			channel_id: forum.channel_id,
			name,
			content: body.trim().to_owned(),
			tags,
			files,
		});
		self.close_new_post();
	}

	pub(super) fn threads_escape(&self) -> bool {
		if self.threads.layer.escape() {
			return true;
		}
		if self.thread_drafting() {
			self.drop_draft();
			return true;
		}
		false
	}

	pub(super) fn threads_apply(&self, state: &State, changes: &Changes) {
		let messages = self.threads.timeline.borrow_mut().apply(state, changes);
		if messages || !changes.channel_lists.is_empty() || changes.current_user {
			self.publish_thread(state);
		}
		if let Some(open) = self.thread_open() {
			self.threads
				.view
				.set_access(Access::of(state, open.guild_id, open.channel_id, Utc::now()));
			self.threads.view.set_nitro(state.has_nitro());
			if !changes.loaded.is_empty() || !changes.messages.is_empty() {
				self.threads.view.reactions_changed();
			}
			if changes.deletions {
				self.threads
					.view
					.set_progress(DeleteProgress::of(state, open.channel_id));
			}
			if changes.rejected
				&& let Some(rejection) = state.last_rejection()
			{
				self.threads.view.rejected(&rejection.reason);
			}
		}
		if self.forum_open().is_some()
			&& (!changes.channel_lists.is_empty()
				|| !changes.loaded.is_empty()
				|| !changes.members.is_empty()
				|| !changes.messages.is_empty())
		{
			self.publish_forum(state);
		}
		if self.threads.layer.is_open() {
			self.publish_layer(state);
		}
		if changes.selection
			&& let Some(selected) = state.selected()
			&& self.thread_open().map(|open| open.channel_id) != Some(selected.channel_id)
			&& state
				.channel(selected.channel_id)
				.is_some_and(|channel| bridge::is_thread(channel.kind))
		{
			self.threads.follow.set(Some(selected.channel_id));
		}
	}

	pub(super) fn follow_threads(&self) {
		if let Some(thread_id) = self.threads.follow.take() {
			self.open_thread(thread_id);
		}
	}

	fn refresh_layer(&self) {
		if let Some(shared) = self.state() {
			self.publish_layer(&shared.read());
		}
	}

	fn publish_layer(&self, state: &State) {
		let layer = &self.threads.layer;
		let Some(popout) = layer.popout() else {
			return;
		};
		match popout.kind {
			crate::threads::PopoutKind::Threads => {
				let sections = bridge::thread_sections(state, popout.channel_id, today());
				let mut avatars: Vec<(Snowflake, media::ImageName)> = Vec::new();
				for user_id in sections
					.iter()
					.flat_map(|section| section.entries.iter().filter_map(|entry| entry.author_id))
				{
					if avatars.iter().any(|(known, _)| *known == user_id) {
						continue;
					}
					if let Some(name) = state.user(user_id).and_then(|user| layer.row_avatar(user)) {
						avatars.push((user_id, name));
					}
				}
				layer.show_threads(ThreadsShown {
					sections,
					avatars,
					can_create: bridge::can_create_threads(state, popout.channel_id),
				});
			}
			crate::threads::PopoutKind::Pins => {
				let pins = bridge::pin_entries(state, popout.channel_id, today());
				let entries = pins
					.unwrap_or_default()
					.into_iter()
					.map(|entry| {
						let avatar = state
							.messages(popout.channel_id)
							.and_then(|cache| {
								cache.by_id.get(&entry.id).map(|message| message.author.clone())
							})
							.or_else(|| {
								state.pins(popout.channel_id).and_then(|pins| {
									pins.iter()
										.find(|message| message.id == entry.id)
										.map(|message| message.author.clone())
								})
							})
							.and_then(|author| layer.pin_avatar(&author));
						(entry, avatar)
					})
					.collect();
				layer.show_pins(PinsShown {
					loaded: state.pins(popout.channel_id).is_some(),
					entries,
					can_manage: state.can_manage_messages(popout.channel_id),
				});
			}
		}
	}

	pub(super) fn redraw_thread(&self, state: &State, formatting: Formatting) {
		if self.threads.timeline.borrow_mut().redraw(state, formatting) {
			self.publish_thread(state);
		}
	}

	fn publish_thread(&self, state: &State) {
		let open = self.thread_open();
		let header = open.and_then(|open| bridge::thread_header(state, open.channel_id));
		let post = header
			.as_ref()
			.and_then(|header| header.parent_id)
			.and_then(|parent| state.channel(parent))
			.is_some_and(|parent| bridge::is_forum(parent.kind));
		let tags = match (&header, post) {
			(Some(header), true) => {
				let applied = state
					.channel(header.id)
					.map(|thread| thread.applied_tags.clone())
					.unwrap_or_default();
				header
					.parent_id
					.map(|forum| bridge::forum_tags(state, forum))
					.unwrap_or_default()
					.into_iter()
					.filter(|tag| applied.contains(&tag.id))
					.map(|tag| tag.label)
					.collect()
			}
			_ => Vec::new(),
		};
		let shown = PanelShown {
			header: header.clone(),
			draft: self.threads.draft.borrow().clone(),
			post,
			tags,
		};
		if self.threads.shown.borrow().as_ref() != Some(&shown) {
			self.threads.panel.set(panel_value(&shown));
			*self.threads.shown.borrow_mut() = Some(shown);
		}
		*self.threads.header.borrow_mut() = header;
		if open.is_none() {
			self.threads.slot.set(Slot::default());
		} else {
			self.threads.slot.set(self.threads.reported.get());
		}
		self.threads.view.set_scale(self.scale_factor());
		let delta = self.threads.timeline.borrow_mut().take_delta();
		if let Some(delta) = delta {
			let timeline = self.threads.timeline.borrow();
			let name = header_name(&self.threads.header.borrow());
			self.threads.view.apply(&timeline, &delta, &name);
		}
		self.composer_apply(state, &Changes::default());
	}

	pub(super) fn place_thread_slot(&self, slot: Slot) {
		self.threads.reported.set(slot);
		self.threads.view.resize(slot.width, slot.height);
		self.threads.view.set_origin(slot.x, slot.y);
		let (width, height) = self.window.info.bounds.with_untracked(|bounds| bounds.size);
		let scale = self.scale_factor() as f32;
		if width > 0 && height > 0 && scale > 0. {
			self.threads
				.view
				.set_window(width as f32 / scale, height as f32 / scale);
		}
		if self.thread_open().is_some() {
			self.threads.slot.set(slot);
		}
	}

	fn size_layer(&self) {
		let (width, height) = self.window.info.bounds.with_untracked(|bounds| bounds.size);
		let scale = self.scale_factor() as f32;
		if width > 0 && height > 0 && scale > 0. {
			self.threads
				.layer
				.set_window(width as f32 / scale, height as f32 / scale);
		}
	}

	fn refresh_forum(&self) {
		if let Some(shared) = self.state() {
			self.publish_forum(&shared.read());
		}
	}

	fn publish_forum(&self, state: &State) {
		let Some(forum) = self.forum_open() else {
			return;
		};
		let sort = self.threads.sort.get();
		let filter = self.threads.filter.borrow().clone();
		let cards = bridge::forum_cards(state, forum.channel_id, sort, &filter, today());
		let tags = bridge::forum_tags(state, forum.channel_id);
		self.threads.forum_view.set(record([
			("open", true.into()),
			("id", forum.channel_id.to_string().as_str().into()),
			("sort", sort.label().into()),
			("gallery", self.threads.gallery.get().into()),
			("composing", self.threads.composing.get().into()),
			("serial", (self.threads.post_serial.get() as f64).into()),
			("draft_title", self.threads.draft_title.borrow().as_str().into()),
			(
				"require_tag",
				bridge::new_post_form(state, forum.channel_id).require_tag.into(),
			),
		]));
		let published = Some((cards.clone(), tags.clone()));
		let same = *self.threads.forum_published.borrow() == published;
		let tag_values: Vec<Value> = tags
			.iter()
			.map(|tag| {
				record([
					("key", tag.id.to_string().as_str().into()),
					("id", tag.id.to_string().as_str().into()),
					("label", tag.label.as_str().into()),
					("active", filter.contains(&tag.id).into()),
				])
			})
			.collect();
		self.threads.forum_tags.set(Value::List(tag_values.into()));
		if same {
			return;
		}
		let pictures = self.timeline_view.pictures();
		let mut avatars = Vec::new();
		let card_values: Vec<Value> = cards
			.iter()
			.map(|card| {
				let avatar = card
					.author_id
					.and_then(|id| state.user(id))
					.and_then(|user| pictures.avatar(user, FORUM_AVATAR))
					.map(|name| name.to_string());
				avatars.extend(avatar.clone());
				let badges: Vec<Value> = card
					.tags
					.iter()
					.map(|tag| {
						record([
							("key", tag.id.to_string().as_str().into()),
							("label", tag.label.as_str().into()),
							("moderated", tag.moderated.into()),
						])
					})
					.collect();
				record([
					("key", card.id.to_string().as_str().into()),
					("id", card.id.to_string().as_str().into()),
					("title", card.title.as_str().into()),
					("excerpt", card.excerpt.as_str().into()),
					("tags", Value::List(badges.into())),
					("author", card.author.as_str().into()),
					(
						"avatar",
						avatar
							.map(|name| format!("host:{name}"))
							.unwrap_or_default()
							.as_str()
							.into(),
					),
					("replies", card.replies.as_str().into()),
					("count", card.count.as_str().into()),
					("activity", card.activity.as_str().into()),
					("pinned", card.pinned.into()),
					("archived", card.archived.into()),
				])
			})
			.collect();
		self.threads.forum_cards.set(Value::List(card_values.into()));
		*self.threads.forum_published.borrow_mut() = published;
		self.set_forum_avatars(avatars);
	}

	fn set_forum_avatars(&self, avatars: Vec<String>) {
		if *self.threads.forum_avatars.borrow() != avatars {
			*self.threads.forum_avatars.borrow_mut() = avatars;
			self.refresh_visible();
		}
	}

	pub(super) fn thread_images(&self) -> Vec<String> {
		let mut names: Vec<String> = self
			.threads
			.view
			.visible_images()
			.iter()
			.map(ToString::to_string)
			.collect();
		names.extend(self.threads.layer.images().iter().map(ToString::to_string));
		names.extend(self.threads.forum_avatars.borrow().iter().cloned());
		names.extend(self.draft_images());
		names.extend(self.threads.composer.images());
		names
	}

	pub(super) async fn drive_threads(&self, screen: capture::Screen) {
		match screen {
			capture::Screen::ThreadsList => self.open_thread_list(capture::TOP_BAR_THREADS),
			capture::Screen::Pins => self.open_pins(capture::TOP_BAR_PINS),
			capture::Screen::ThreadCreate => {
				let open = self.timeline.borrow().open();
				if let Some(open) = open {
					let newest = self.timeline.borrow().messages().last().map(|entry| entry.id);
					self.open_create_thread(open.channel_id, newest);
				}
			}
			capture::Screen::ThreadPanel => {
				if let Some(thread_id) =
					self.first_thread_of(self.timeline.borrow().open().map(|open| open.channel_id))
				{
					self.open_thread(thread_id);
				}
			}
			capture::Screen::Forum | capture::Screen::ForumPost | capture::Screen::ForumPostCreate => {
				let Some(forum) = self.first_forum() else {
					tracing::warn!("the guild has no forum");
					return;
				};
				self.select_channel(forum);
				self.until_capture(|link| link.forum_settled()).await;
				match screen {
					capture::Screen::ForumPost => {
						if let Some(post) = self.first_thread_of(Some(forum)) {
							self.open_thread(post);
						}
					}
					capture::Screen::ForumPostCreate => self.open_new_post(""),
					_ => {}
				}
			}
			_ => return,
		}
		let started = Instant::now();
		while !self.threads_settled(screen) && started.elapsed() < capture::LIMIT {
			zgx::timer::sleep(capture::POLL).await;
		}
	}

	fn first_forum(&self) -> Option<Snowflake> {
		let shared = self.state()?;
		let state = shared.read();
		let guild_id = self.sidebar.sidebar().selected()?;
		state
			.guild_channel_tree(guild_id)
			.into_iter()
			.flat_map(|node| node.children)
			.find(|row| !row.locked && bridge::is_forum(row.kind))
			.map(|row| row.id)
	}

	fn first_thread_of(&self, parent: Option<Snowflake>) -> Option<Snowflake> {
		let parent = parent?;
		let shared = self.state()?;
		let state = shared.read();
		let guild = state.channel_guild(parent)?;
		state
			.guild(guild)?
			.threads
			.values()
			.filter(|thread| thread.parent_id == Some(parent))
			.map(|thread| thread.id)
			.min()
	}

	fn forum_settled(&self) -> bool {
		let Some(forum) = self.forum_open() else {
			return false;
		};
		let loaded = self
			.state()
			.is_some_and(|shared| shared.read().forum_page(forum.channel_id).is_some());
		let shown = self.shown.borrow();
		loaded
			&& self
				.threads
				.forum_avatars
				.borrow()
				.iter()
				.all(|name| shown.contains(name))
	}

	fn threads_settled(&self, screen: capture::Screen) -> bool {
		match screen {
			capture::Screen::ThreadsList => self.threads.layer.popout().is_some(),
			capture::Screen::Pins => {
				self.threads.layer.popout().is_some()
					&& self.threads.layer.settled()
					&& self
						.state()
						.zip(self.threads.layer.popout())
						.is_some_and(|(shared, popout)| shared.read().pins(popout.channel_id).is_some())
			}
			capture::Screen::ThreadCreate => self.thread_drafting() && self.draft_settled(),
			capture::Screen::ForumPostCreate => self.forum_composing(),
			capture::Screen::ThreadPanel | capture::Screen::ForumPost => {
				self.threads.timeline.borrow().messages().next().is_some() && self.threads.view.settled()
			}
			capture::Screen::Forum => self.forum_settled(),
			_ => true,
		}
	}
}

fn header_name(header: &Option<ThreadHeader>) -> String {
	header
		.as_ref()
		.map(|header| header.name.clone())
		.unwrap_or_default()
}
