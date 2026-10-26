use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use bridge::{DmEntry, DmList};
use media::ImageName;
use protocol::Snowflake;
use state::{Changes, State};
use zgx_script::{ScriptHost, Shared, Value};

use crate::rail::Icons;
use crate::ui::{Link, record};

const AVATAR_POINTS: f64 = 32.;
const PAGES: [&str; 4] = ["friends", "nitro", "shop", "requests"];
const LATER: [&str; 1] = ["dms.create_group"];

pub(crate) struct DmSidebarView {
	list: RefCell<DmList>,
	avatars: RefCell<HashMap<(Snowflake, Option<String>), ImageName>>,
	last_open: Cell<Option<Snowflake>>,
	page_name: RefCell<String>,
	rows: Shared,
	requests: Shared,
	page: Shared,
}

impl DmSidebarView {
	pub(crate) fn new(host: &ScriptHost) -> Self {
		Self {
			list: RefCell::new(DmList::default()),
			avatars: RefCell::new(HashMap::new()),
			last_open: Cell::new(None),
			page_name: RefCell::new(String::new()),
			rows: host.shared("dms.rows", Value::List(Rc::new([]))),
			requests: host.shared("dms.requests", 0.),
			page: host.shared("home.page", ""),
		}
	}

	pub(crate) fn reset(&self) {
		*self.list.borrow_mut() = DmList::default();
		self.avatars.borrow_mut().clear();
		self.last_open.set(None);
		self.page_name.borrow_mut().clear();
		self.rows.set(Value::List(Rc::new([])));
		self.requests.set(0.0.into());
		self.page.set("".into());
	}

	pub(crate) fn list(&self) -> Ref<'_, DmList> {
		self.list.borrow()
	}

	pub(crate) fn apply(&self, state: &State, changes: &Changes) -> bool {
		self.list.borrow_mut().apply(state, changes)
	}

	pub(crate) fn remember(&self, channel_id: Snowflake) {
		self.last_open.set(Some(channel_id));
	}

	pub(crate) fn home_target(&self) -> Option<Snowflake> {
		let list = self.list.borrow();
		self.last_open
			.get()
			.filter(|id| list.entry(*id).is_some())
			.or_else(|| list.most_recent())
	}

	pub(crate) fn page(&self) -> String {
		self.page_name.borrow().clone()
	}

	pub(crate) fn set_page(&self, page: &str) -> bool {
		let page = if PAGES.contains(&page) { page } else { "" };
		if *self.page_name.borrow() == page {
			return false;
		}
		*self.page_name.borrow_mut() = page.to_owned();
		self.page.set(page.into());
		true
	}

	pub(crate) fn title(&self, channel_id: Snowflake) -> Option<String> {
		self.list
			.borrow()
			.entry(channel_id)
			.map(|entry| entry.name.clone())
	}

	pub(crate) fn publish(&self, state: &State, icons: &Icons<'_>) -> Vec<String> {
		let list = self.list.borrow();
		let mut names = Vec::new();
		let rows = list
			.entries()
			.iter()
			.map(|entry| {
				let avatar = entry
					.recipient
					.and_then(|user_id| self.avatar(state, user_id, icons));
				names.extend(avatar.clone());
				dm_value(entry, avatar.as_deref())
			})
			.collect();
		self.rows.set(Value::List(rows));
		self.requests.set((list.requests() as f64).into());
		names
	}

	fn avatar(&self, state: &State, user_id: Snowflake, icons: &Icons<'_>) -> Option<String> {
		let user = state.user(user_id)?;
		let key = (user_id, user.avatar.clone());
		let mut avatars = self.avatars.borrow_mut();
		if let Some(name) = avatars.get(&key) {
			return Some(name.as_str().to_owned());
		}
		let request = bridge::avatar_request(icons.cdn, user, AVATAR_POINTS, icons.scale, false)?;
		let name = icons.gallery.request(request);
		let text = name.as_str().to_owned();
		avatars.insert(key, name);
		Some(text)
	}
}

pub(crate) fn install_actions(host: &ScriptHost, link: &Rc<Link>) {
	let actions = Rc::downgrade(link);
	host.action("dms.select", move |argument| {
		if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake(&argument)) {
			link.select_dm(channel_id);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("dms.close", move |argument| {
		if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake(&argument)) {
			link.close_dm(channel_id);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("dms.full_profile", move |argument| {
		if let (Some(link), Some(user_id)) = (actions.upgrade(), snowflake(&argument)) {
			link.open_full_profile(user_id);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("home.show_page", move |argument| {
		if let (Some(link), Value::Str(page)) = (actions.upgrade(), &argument) {
			link.show_home_page(page);
		}
	});
	for name in LATER {
		host.action(name, move |_| {
			tracing::debug!(action = name, "this home action is not built yet");
		});
	}
}

fn snowflake(value: &Value) -> Option<Snowflake> {
	match value {
		Value::Str(text) => text.parse::<u64>().ok().map(Snowflake),
		_ => None,
	}
}

fn dm_value(entry: &DmEntry, avatar: Option<&str>) -> Value {
	let avatar = avatar.map(|name| format!("host:{name}")).unwrap_or_default();
	record([
		("key", format!("d{}", entry.id).as_str().into()),
		("id", entry.id.to_string().as_str().into()),
		("name", entry.name.as_str().into()),
		("avatar", avatar.as_str().into()),
		("status", entry.status.as_str().into()),
		("group", entry.is_group().into()),
		("members", (entry.members.unwrap_or(0) as f64).into()),
		("unread", entry.unread.into()),
		("mentions", f64::from(entry.mentions).into()),
	])
}
