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

const AVATAR_POINTS: f64 = 40.;
const TABS: [&str; 5] = ["messages", "media", "links", "files", "pins"];

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
	text: RefCell<String>,
	terms: RefCell<Vec<String>>,
	tab: Cell<&'static str>,
	oldest: Cell<bool>,
	page: Cell<usize>,
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
			text: RefCell::default(),
			terms: RefCell::default(),
			tab: Cell::new(TABS[0]),
			oldest: Cell::new(false),
			page: Cell::new(1),
		}
	}

	pub(super) fn view(&self) -> View {
		self.switcher.view()
	}

	pub(super) fn reset(&self) {
		self.switcher.close();
		*self.recent.borrow_mut() = RecentTargets::default();
		self.last_open.set(None);
		self.friend.set(None);
		self.shown.borrow_mut().take();
		self.chips.borrow_mut().clear();
		self.avatars.borrow_mut().clear();
		self.panel.set(closed_panel());
		self.hits.set(empty());
		self.chips_value.set(empty());
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
		("tab", TABS[0].into()),
		("oldest", false.into()),
		("page", 1.0.into()),
		("pages", 1.0.into()),
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
		host.action("search.tab", move |argument| {
			let (Some(link), Some(tab)) = (actions.upgrade(), text(&argument)) else {
				return;
			};
			if let Some(tab) = TABS.iter().find(|known| **known == tab) {
				link.finder.tab.set(tab);
				link.send_search();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("search.sort", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.finder.oldest.set(matches!(argument, Value::Bool(true)));
				link.send_search();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("search.page", move |argument| {
			if let (Some(link), Value::Num(page)) = (actions.upgrade(), argument) {
				link.show_results_page(page.max(1.) as usize);
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

	pub(super) fn type_switcher(&self, query: &str) {
		self.finder.switcher.type_query(query);
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
			SwitcherTarget::Guild(guild_id) => self.visit_guild(guild_id),
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
		if text.is_empty() || self.timeline.borrow().open().is_none() {
			return;
		}
		*self.finder.text.borrow_mut() = text.to_owned();
		self.finder.tab.set(TABS[0]);
		self.finder.oldest.set(false);
		self.send_search();
	}

	fn send_search(&self) {
		let Some(shared) = self.state() else {
			return;
		};
		let Some(open) = self.timeline.borrow().open() else {
			return;
		};
		let text = self.finder.text.borrow().clone();
		if text.is_empty() {
			return;
		}
		let channel_id = open.guild_id.is_none().then_some(open.channel_id);
		let parsed = parse_search(&text, &shared.read(), open.guild_id, channel_id);
		*self.finder.chips.borrow_mut() = parsed.chips.iter().map(|chip| chip.label()).collect();
		*self.finder.terms.borrow_mut() = bridge::search_terms(parsed.query.content.as_deref());
		let mut query = parsed.query;
		match self.finder.tab.get() {
			"media" => query.has.push("image".to_owned()),
			"links" => query.has.push("link".to_owned()),
			"files" => query.has.push("file".to_owned()),
			"pins" => query.pinned = Some(true),
			_ => {}
		}
		query.oldest_first = self.finder.oldest.get();
		self.finder.page.set(1);
		self.send(Command::Search {
			guild_id: open.guild_id,
			channel_id,
			query,
		});
	}

	fn show_results_page(&self, page: usize) {
		let Some(total) = self.finder.shown.borrow().as_ref().map(|panel| panel.total) else {
			return;
		};
		self.finder.page.set(page.min(bridge::result_pages(total)));
		if let Some(shared) = self.state() {
			self.publish_search(&shared.read());
		}
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
		self.publish_search(state);
	}

	fn publish_search(&self, state: &State) {
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
		let pages = bridge::result_pages(panel.total);
		let page = self.finder.page.get().min(pages);
		let wanted = page * bridge::RESULTS_PAGE;
		if panel.hits.len() < wanted && panel.more && !panel.busy {
			self.send(Command::SearchMore);
		}
		let pictures = self.timeline_view.pictures();
		let terms = self.finder.terms.borrow().clone();
		let mut avatars = Vec::new();
		let mut previous = None;
		let hits: Rc<[Value]> = panel
			.hits
			.iter()
			.skip((page - 1) * bridge::RESULTS_PAGE)
			.take(bridge::RESULTS_PAGE)
			.map(|hit| {
				let avatar = Some(
					pictures
						.member_avatar(&hit.entry.avatar, AVATAR_POINTS, false)
						.to_string(),
				);
				avatars.extend(avatar.clone());
				let group = if previous == Some(hit.channel_id) {
					String::new()
				} else {
					previous = Some(hit.channel_id);
					group_name(state, hit)
				};
				hit_value(hit, avatar.as_deref(), &group, &terms, state)
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
			("tab", self.finder.tab.get().into()),
			("oldest", self.finder.oldest.get().into()),
			("page", (page as f64).into()),
			("pages", (pages as f64).into()),
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
		if self.view_showing(hit.channel_id).is_none() {
			match hit.guild_id {
				Some(guild_id) => self.open(Some(guild_id), hit.channel_id),
				None => self.select_dm(hit.channel_id),
			}
			if let Some(shared) = self.state() {
				self.publish_actions(&shared.read(), &Changes::default());
			}
		}
		let Some(view) = self.view_showing(hit.channel_id) else {
			tracing::debug!("the channel of the search result cannot be opened");
			return;
		};
		match then {
			Some(action) => view.jump_then(message_id, action),
			None => view.jump_to(message_id),
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

fn group_name(state: &State, hit: &SearchHit) -> String {
	match hit.place.strip_prefix('#') {
		Some(name) => name.to_owned(),
		None => state
			.channel(hit.channel_id)
			.and_then(|channel| channel.name.clone())
			.unwrap_or_else(|| "Direct Message".to_owned()),
	}
}

fn hit_value(hit: &SearchHit, avatar: Option<&str>, group: &str, terms: &[String], state: &State) -> Value {
	let avatar = avatar.map(|name| format!("host:{name}")).unwrap_or_default();
	let group_icon = state
		.channel(hit.channel_id)
		.filter(|_| hit.guild_id.is_some())
		.map_or("at_sign", |channel| bridge::channel_icon(channel.kind));
	let runs: Vec<Value> = bridge::marked_runs(&hit.text, terms)
		.into_iter()
		.map(|(text, mark)| record([("text", text.as_str().into()), ("mark", mark.into())]))
		.collect();
	record([
		("key", format!("s{}", hit.entry.id).as_str().into()),
		("id", hit.entry.id.to_string().as_str().into()),
		("author", hit.author.as_str().into()),
		("avatar", avatar.as_str().into()),
		("time", hit.time.as_str().into()),
		("place", hit.place.as_str().into()),
		("text", hit.text.as_str().into()),
		("runs", Value::List(runs.into())),
		("group", group.into()),
		("group_icon", group_icon.into()),
	])
}
