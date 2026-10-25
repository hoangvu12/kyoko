use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use bridge::{FriendEntry, FriendsTab, FriendsView};
use media::ImageName;
use protocol::Snowflake;
use state::{Changes, State};
use zgx_script::{ScriptHost, Shared, Value};

use crate::rail::Icons;
use crate::ui::{Link, number, record};

const ROW_AVATAR: f64 = 36.;
const ACTIVE_AVATAR: f64 = 40.;

type AvatarKey = (Snowflake, Option<String>, u64);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Adding {
	Idle,
	Sending { name: String, baseline: u64 },
	Sent(String),
	Failed(String),
}

pub(crate) struct FriendsPage {
	view: RefCell<FriendsView>,
	tab: Cell<FriendsTab>,
	filter: RefCell<String>,
	adding: RefCell<Adding>,
	avatars: RefCell<HashMap<AvatarKey, ImageName>>,
	shown: RefCell<Vec<String>>,
	tab_value: Shared,
	rows: Shared,
	empty: Shared,
	pending: Shared,
	active: Shared,
	add: Shared,
}

impl FriendsPage {
	pub(crate) fn new(host: &ScriptHost) -> Self {
		Self {
			view: RefCell::new(FriendsView::default()),
			tab: Cell::new(FriendsTab::Online),
			filter: RefCell::new(String::new()),
			adding: RefCell::new(Adding::Idle),
			avatars: RefCell::new(HashMap::new()),
			shown: RefCell::new(Vec::new()),
			tab_value: host.shared("friends.tab", FriendsTab::Online.key()),
			rows: host.shared("friends.rows", Value::List(Rc::new([]))),
			empty: host.shared("friends.empty", ""),
			pending: host.shared("friends.pending", 0.),
			active: host.shared("friends.active", Value::List(Rc::new([]))),
			add: host.shared("friends.add", adding_value(&Adding::Idle)),
		}
	}

	pub(crate) fn reset(&self) {
		*self.view.borrow_mut() = FriendsView::default();
		self.tab.set(FriendsTab::Online);
		self.filter.borrow_mut().clear();
		*self.adding.borrow_mut() = Adding::Idle;
		self.avatars.borrow_mut().clear();
		self.shown.borrow_mut().clear();
		self.tab_value.set(FriendsTab::Online.key().into());
		self.rows.set(Value::List(Rc::new([])));
		self.empty.set("".into());
		self.pending.set(0.0.into());
		self.active.set(Value::List(Rc::new([])));
		self.add.set(adding_value(&Adding::Idle));
	}

	pub(crate) fn images(&self) -> Vec<String> {
		self.shown.borrow().clone()
	}

	pub(crate) fn apply(&self, state: &State, changes: &Changes) -> bool {
		let mut changed = self.view.borrow_mut().apply(state, changes);
		let settled = {
			let adding = self.adding.borrow();
			match &*adding {
				Adding::Sending { name, baseline } => {
					let refused = state
						.last_rejection()
						.filter(|rejection| changes.rejected && rejection.id > *baseline)
						.map(|rejection| Adding::Failed(rejection.reason.clone()));
					refused.or_else(|| changes.relationships.then(|| Adding::Sent(name.clone())))
				}
				_ => None,
			}
		};
		if let Some(next) = settled {
			self.add.set(adding_value(&next));
			*self.adding.borrow_mut() = next;
			changed = true;
		}
		changed
	}

	pub(crate) fn select_tab(&self, key: &str) -> bool {
		let Some(tab) = FriendsTab::named(key) else {
			return false;
		};
		if self.tab.get() == tab {
			return false;
		}
		self.tab.set(tab);
		self.tab_value.set(tab.key().into());
		true
	}

	pub(crate) fn set_filter(&self, filter: &str) -> bool {
		if *self.filter.borrow() == filter {
			return false;
		}
		*self.filter.borrow_mut() = filter.to_owned();
		true
	}

	pub(crate) fn start_adding(&self, name: &str, baseline: u64) -> bool {
		let name = name.trim();
		if name.is_empty() || matches!(*self.adding.borrow(), Adding::Sending { .. }) {
			return false;
		}
		let next = Adding::Sending {
			name: name.to_owned(),
			baseline,
		};
		self.add.set(adding_value(&next));
		*self.adding.borrow_mut() = next;
		true
	}

	pub(crate) fn publish(&self, state: &State, icons: &Icons<'_>) -> bool {
		let view = self.view.borrow();
		let tab = self.tab.get();
		let sections = view.sections(tab, &self.filter.borrow());
		let mut names = Vec::new();
		let mut rows = Vec::new();
		for section in &sections {
			rows.push(record([
				("kind", "title".into()),
				("key", format!("t{}", section.title).as_str().into()),
				("title", section.title.as_str().into()),
			]));
			for entry in &section.entries {
				let avatar = self.avatar(state, entry.id, ROW_AVATAR, icons);
				names.extend(avatar.clone());
				rows.push(person_value(entry, avatar.as_deref()));
			}
		}
		let empty = if sections.is_empty() && tab != FriendsTab::Add {
			tab.empty_text()
		} else {
			""
		};
		let active: Vec<Value> = view
			.active_now()
			.into_iter()
			.map(|entry| {
				let avatar = self.avatar(state, entry.id, ACTIVE_AVATAR, icons);
				names.extend(avatar.clone());
				person_value(entry, avatar.as_deref())
			})
			.collect();
		self.rows.set(Value::List(rows.into()));
		self.empty.set(empty.into());
		self.pending.set((view.pending_count() as f64).into());
		self.active.set(Value::List(active.into()));
		let changed = *self.shown.borrow() != names;
		*self.shown.borrow_mut() = names;
		changed
	}

	fn avatar(&self, state: &State, user_id: Snowflake, points: f64, icons: &Icons<'_>) -> Option<String> {
		let user = state.user(user_id)?;
		let key = (user_id, user.avatar.clone(), points as u64);
		let mut avatars = self.avatars.borrow_mut();
		if let Some(name) = avatars.get(&key) {
			return Some(name.as_str().to_owned());
		}
		let request = bridge::avatar_request(icons.cdn, user, points, icons.scale, false)?;
		let name = icons.gallery.request(request);
		let text = name.as_str().to_owned();
		avatars.insert(key, name);
		Some(text)
	}
}

fn person_value(entry: &FriendEntry, avatar: Option<&str>) -> Value {
	let avatar = avatar.map(|name| format!("host:{name}")).unwrap_or_default();
	record([
		("kind", "person".into()),
		("key", format!("p{}", entry.id).as_str().into()),
		("id", entry.id.to_string().as_str().into()),
		("name", entry.name.as_str().into()),
		("username", entry.username.as_str().into()),
		("avatar", avatar.as_str().into()),
		("status", entry.status.as_str().into()),
		("subtitle", entry.subtitle().into()),
		("relation", entry.kind.as_str().into()),
	])
}

fn adding_value(adding: &Adding) -> Value {
	let (phase, text) = match adding {
		Adding::Idle => ("idle", String::new()),
		Adding::Sending { .. } => ("sending", String::new()),
		Adding::Sent(name) => (
			"sent",
			format!("Success! Your friend request to {name} was sent."),
		),
		Adding::Failed(reason) => ("failed", reason.clone()),
	};
	record([("phase", phase.into()), ("text", text.as_str().into())])
}

fn snowflake(value: &Value) -> Option<Snowflake> {
	match value {
		Value::Str(text) => text.parse::<u64>().ok().map(Snowflake),
		_ => None,
	}
}

fn text(value: &Value) -> Option<&str> {
	match value {
		Value::Str(text) => Some(text),
		_ => None,
	}
}

pub(crate) fn install_actions(host: &ScriptHost, link: &Rc<Link>) {
	let actions = Rc::downgrade(link);
	host.action("friends.select_tab", move |argument| {
		if let (Some(link), Some(key)) = (actions.upgrade(), text(&argument)) {
			link.select_friends_tab(key);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("friends.filter", move |argument| {
		if let (Some(link), Some(filter)) = (actions.upgrade(), text(&argument)) {
			link.filter_friends(filter);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("friends.add", move |argument| {
		if let (Some(link), Some(username)) = (actions.upgrade(), text(&argument)) {
			link.add_friend(username);
		}
	});
	for (name, act) in [
		("friends.accept", Link::accept_friend as fn(&Link, Snowflake)),
		("friends.remove", Link::remove_relationship),
		("friends.block", Link::block_user),
		("friends.message", Link::message_friend),
		("friends.copy_id", Link::copy_user_id),
	] {
		let actions = Rc::downgrade(link);
		host.action(name, move |argument| {
			if let (Some(link), Some(user_id)) = (actions.upgrade(), snowflake(&argument)) {
				act(&link, user_id);
			}
		});
	}
	let actions = Rc::downgrade(link);
	host.action("friends.open_profile", move |argument| {
		let user_id = crate::ui::field(&argument, "id").and_then(snowflake);
		let at = number(&argument, "x").zip(number(&argument, "y"));
		if let (Some(link), Some(user_id), Some(at)) = (actions.upgrade(), user_id, at) {
			link.open_friend_profile(user_id, at);
		}
	});
}
