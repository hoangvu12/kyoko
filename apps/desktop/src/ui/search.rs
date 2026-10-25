use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::{
	Access, MessageAction, OpenChannel, QuickSwitcher, RecentTargets, SearchHit, SearchPanel, SwitcherTarget,
	parse_search, today,
};
use chrono::Utc;
use protocol::Snowflake;
use session::Command;
use state::{Changes, State};
use zgx::compose::View;
use zgx::reactive::Runtime;
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, number, record, snowflake, text};
use crate::switcher::SwitcherView;
use crate::timeline::Pictures;

const AVATAR_POINTS: f64 = 24.;

pub(super) struct Finder {
	panel: Shared,
	hits: Shared,
	chips_value: Shared,
	switcher: Rc<SwitcherView>,
	recent: RefCell<RecentTargets>,
	last_open: Cell<Option<OpenChannel>>,
	friend: Cell<Option<Snowflake>>,
	shown: RefCell<Option<SearchPanel>>,
	chips: RefCell<Vec<String>>,
	avatars: RefCell<Vec<String>>,
}

impl Finder {
	pub(super) fn new(host: &ScriptHost, runtime: &Runtime, pictures: Rc<Pictures>) -> Self {
		Self {
			panel: host.shared("search.panel", closed_panel()),
			hits: host.shared("search.hits", empty()),
			chips_value: host.shared("search.chips", empty()),
			switcher: SwitcherView::new(runtime, pictures),
			recent: RefCell::default(),
			last_open: Cell::new(None),
			friend: Cell::new(None),
			shown: RefCell::new(None),
			chips: RefCell::default(),
			avatars: RefCell::default(),
		}
	}

	pub(super) fn view(&self) -> View {
		self.switcher.view()
	}
}

fn empty() -> Value {
	Value::List(Rc::from(Vec::new()))
}

fn closed_panel() -> Value {
	record([
		("open", false.into()),
		("title", "".into()),
		("notice", "".into()),
		("busy", false.into()),
		("more", false.into()),
	])
}

impl Link {
	pub(super) fn install_search(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("search.run", move |argument| {
			if let (Some(link), Some(query)) = (actions.upgrade(), text(&argument)) {
				link.run_search(query);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("search.close", move |_| {
			if let Some(link) = actions.upgrade() {
				link.close_search();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("search.more", move |_| {
			if let Some(link) = actions.upgrade() {
				link.send(Command::SearchMore);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("search.open_result", move |argument| {
			if let (Some(link), Some(message_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.open_result(message_id, None);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("search.menu", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let message_id = field(&argument, "id").and_then(snowflake);
			let at = number(&argument, "x").zip(number(&argument, "y"));
			if let (Some(message_id), Some(at)) = (message_id, at) {
				link.result_menu(message_id, at);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("dms.search", move |_| {
			if let Some(link) = actions.upgrade() {
				link.open_switcher();
			}
		});
		let actions = Rc::downgrade(link);
		link.finder.switcher.on_pick(move |target| {
			if let Some(link) = actions.upgrade() {
				link.switch_to(target);
			}
		});
		let actions = Rc::downgrade(link);
		link.finder.switcher.on_images_changed(move |()| {
			if let Some(link) = actions.upgrade() {
				link.refresh_visible();
			}
		});
		let actions = Rc::downgrade(link);
		link.timeline_view.on_reveal(move |(message_id, action)| {
			if let Some(link) = actions.upgrade() {
				link.open_result(message_id, Some(action));
			}
		});
	}

	pub(crate) fn open_switcher(&self) {
		let Some(shared) = self.state() else {
			return;
		};
		if self.finder.switcher.is_open() {
			self.finder.switcher.close();
			return;
		}
		let model = QuickSwitcher::of(&shared.read(), &self.finder.recent.borrow());
		self.finder.switcher.open(model);
	}

	fn switch_to(&self, target: SwitcherTarget) {
		match target {
			SwitcherTarget::Channel {
				guild_id: Some(guild_id),
				channel_id,
			} => self.open(Some(guild_id), channel_id),
			SwitcherTarget::Channel {
				guild_id: None,
				channel_id,
			} => self.select_dm(channel_id),
			SwitcherTarget::Guild(guild_id) => self.select_guild(guild_id),
			SwitcherTarget::Friend(user_id) => match self.dm_with(user_id) {
				Some(channel_id) => self.select_dm(channel_id),
				None => {
					self.finder.friend.set(Some(user_id));
					self.send(Command::OpenDm { user_id });
				}
			},
		}
	}

	fn dm_with(&self, user_id: Snowflake) -> Option<Snowflake> {
		let shared = self.state()?;
		let state = shared.read();
		state
			.dm_rows()
			.into_iter()
			.find(|row| row.recipients == [user_id])
			.map(|row| row.channel_id)
	}

	pub(super) fn remember_open(&self, open: Option<OpenChannel>) {
		if open == self.finder.last_open.get() {
			return;
		}
		self.finder.last_open.set(open);
		if let Some(open) = open {
			self.finder.recent.borrow_mut().remember(SwitcherTarget::Channel {
				guild_id: open.guild_id,
				channel_id: open.channel_id,
			});
		}
	}

	fn run_search(&self, text: &str) {
		let text = text.trim();
		let Some(shared) = self.state() else {
			return;
		};
		let Some(open) = self.timeline.borrow().open() else {
			return;
		};
		if text.is_empty() {
			return;
		}
		let channel_id = open.guild_id.is_none().then_some(open.channel_id);
		let parsed = parse_search(text, &shared.read(), open.guild_id, channel_id);
		*self.finder.chips.borrow_mut() = parsed.chips.iter().map(|chip| chip.label()).collect();
		self.send(Command::Search {
			guild_id: open.guild_id,
			channel_id,
			query: parsed.query,
		});
	}

	fn close_search(&self) {
		self.send(Command::CloseSearch);
		self.clear_search();
	}

	fn clear_search(&self) {
		self.finder.shown.borrow_mut().take();
		self.window.runtime.batch(|| {
			self.finder.panel.set(closed_panel());
			self.finder.hits.set(empty());
			self.finder.chips_value.set(empty());
		});
		self.set_search_avatars(Vec::new());
	}

	pub(super) fn follow_friend(&self, changes: &Changes) {
		if changes.private_channels
			&& let Some(user_id) = self.finder.friend.get()
			&& let Some(channel_id) = self.dm_with(user_id)
		{
			self.finder.friend.set(None);
			self.select_dm(channel_id);
		}
	}

	pub(super) fn search_apply(&self, state: &State, changes: &Changes) {
		if !changes.search {
			return;
		}
		let results = self
			.flow
			.borrow()
			.session()
			.and_then(|session| session.search_results());
		let Some(results) = results else {
			self.clear_search();
			return;
		};
		let panel = SearchPanel::of(&results, state, today());
		let pictures = self.timeline_view.pictures();
		let mut avatars = Vec::new();
		let hits: Rc<[Value]> = panel
			.hits
			.iter()
			.map(|hit| {
				let avatar = pictures
					.avatar(&hit.entry.message.author, AVATAR_POINTS)
					.map(|name| name.to_string());
				avatars.extend(avatar.clone());
				hit_value(hit, avatar.as_deref())
			})
			.collect();
		let chips: Rc<[Value]> = self
			.finder
			.chips
			.borrow()
			.iter()
			.map(|chip| Value::from(chip.as_str()))
			.collect();
		self.finder.panel.set(record([
			("open", true.into()),
			("title", panel.title.as_str().into()),
			("notice", panel.notice.as_deref().unwrap_or_default().into()),
			("busy", panel.busy.into()),
			("more", panel.more.into()),
		]));
		self.finder.hits.set(Value::List(hits));
		self.finder.chips_value.set(Value::List(chips));
		*self.finder.shown.borrow_mut() = Some(panel);
		self.set_search_avatars(avatars);
	}

	fn set_search_avatars(&self, avatars: Vec<String>) {
		if *self.finder.avatars.borrow() != avatars {
			*self.finder.avatars.borrow_mut() = avatars;
			self.refresh_visible();
		}
	}

	pub(super) fn search_images(&self) -> Vec<String> {
		let mut names = self.finder.avatars.borrow().clone();
		names.extend(self.finder.switcher.images().iter().map(ToString::to_string));
		names
	}

	pub(super) fn search_settled(&self) -> bool {
		let shown = self.shown.borrow();
		let panel = self.finder.shown.borrow();
		panel.as_ref().is_some_and(|panel| !panel.busy)
			&& self
				.finder
				.avatars
				.borrow()
				.iter()
				.all(|name| shown.contains(name))
	}

	pub(super) fn switcher_settled(&self) -> bool {
		self.finder.switcher.is_open() && self.finder.switcher.settled()
	}

	fn hit(&self, message_id: Snowflake) -> Option<SearchHit> {
		self.finder
			.shown
			.borrow()
			.as_ref()
			.and_then(|panel| panel.hit(message_id).cloned())
	}

	fn open_result(&self, message_id: Snowflake, then: Option<MessageAction>) {
		let Some(hit) = self.hit(message_id) else {
			return;
		};
		let open = self.timeline.borrow().open();
		if open.map(|open| open.channel_id) != Some(hit.channel_id) {
			match hit.guild_id {
				Some(guild_id) => self.open(Some(guild_id), hit.channel_id),
				None => self.select_dm(hit.channel_id),
			}
			if let Some(shared) = self.state() {
				self.publish_actions(&shared.read(), &Changes::default());
			}
		}
		if self.timeline.borrow().open().map(|open| open.channel_id) != Some(hit.channel_id) {
			tracing::debug!("the channel of the search result cannot be opened");
			return;
		}
		match then {
			Some(action) => self.timeline_view.jump_then(message_id, action),
			None => self.timeline_view.jump_to(message_id),
		}
	}

	fn result_menu(&self, message_id: Snowflake, at: (f32, f32)) {
		let (Some(hit), Some(shared)) = (self.hit(message_id), self.state()) else {
			return;
		};
		let access = Access::of(&shared.read(), hit.guild_id, hit.channel_id, Utc::now());
		self.timeline_view.open_menu_at(&hit.entry, access, at);
	}

	pub(super) fn drive_search(&self, query: &str) {
		self.run_search(query);
	}
}

fn hit_value(hit: &SearchHit, avatar: Option<&str>) -> Value {
	let avatar = avatar.map(|name| format!("host:{name}")).unwrap_or_default();
	record([
		("key", format!("s{}", hit.entry.id).as_str().into()),
		("id", hit.entry.id.to_string().as_str().into()),
		("author", hit.author.as_str().into()),
		("avatar", avatar.as_str().into()),
		("time", hit.time.as_str().into()),
		("place", hit.place.as_str().into()),
		("text", hit.text.as_str().into()),
	])
}
