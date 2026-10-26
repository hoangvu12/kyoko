use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use bridge::{Inbox, MemberChange, MemberEntry, MemberHeights, MemberListEntry, MemberPanel, OpenMembers};
use media::ImageName;
use protocol::Snowflake;
use session::Command;
use state::{Changes, State};
use zgx_script::{ScriptHost, Shared, Value};

use crate::rail::Icons;
use crate::ui::{Link, field, number, record};

const AVATAR_POINTS: f64 = 32.;
const FAR_END: f64 = 1e9;

pub(crate) struct MemberListView {
	panel: RefCell<MemberPanel>,
	avatars: RefCell<HashMap<(Snowflake, Option<String>), ImageName>>,
	wakes: Inbox<()>,
	jumps: Cell<u32>,
	heights: Shared,
	window: Shared,
	jump: Shared,
}

impl MemberListView {
	pub(crate) fn new(host: &ScriptHost) -> Self {
		Self {
			panel: RefCell::new(MemberPanel::default()),
			avatars: RefCell::new(HashMap::new()),
			wakes: Inbox::new(),
			jumps: Cell::new(0),
			heights: host.shared("members.heights", Value::List(Rc::new([]))),
			window: host.shared("members.window", empty_window()),
			jump: host.shared("members.jump", jump_value(0, 0.)),
		}
	}

	pub(crate) fn reset(&self) {
		self.panel.borrow_mut().reset();
		self.avatars.borrow_mut().clear();
		self.heights.set(Value::List(Rc::new([])));
		self.window.set(empty_window());
	}

	pub(crate) fn panel(&self) -> Ref<'_, MemberPanel> {
		self.panel.borrow()
	}

	pub(crate) fn wakes(&self) -> &Inbox<()> {
		&self.wakes
	}

	pub(crate) fn due(&self) -> Option<Instant> {
		self.panel.borrow().due()
	}

	pub(crate) fn show(&self, open: Option<OpenMembers>) {
		if self.panel.borrow_mut().show(open, Instant::now()) {
			self.wakes.push(());
		}
	}

	pub(crate) fn set_display(&self, display: bridge::Display) {
		self.panel.borrow_mut().set_display(display);
	}

	pub(crate) fn redraw(&self) {
		if self.panel.borrow_mut().redraw(Instant::now()) {
			self.wakes.push(());
		}
	}

	pub(crate) fn apply(&self, state: &State, changes: &Changes) {
		if self.panel.borrow_mut().apply(state, changes, Instant::now()) {
			self.wakes.push(());
		}
	}

	pub(crate) fn scroll(&self, offset: f64, height: f64, heights: Option<MemberHeights>) -> Option<Command> {
		let now = Instant::now();
		let mut panel = self.panel.borrow_mut();
		if let Some(heights) = heights {
			panel.set_heights(heights, now);
		}
		let command = panel.scroll(offset, height, now);
		if panel.due().is_some() {
			self.wakes.push(());
		}
		command
	}

	pub(crate) fn search(&self, query: &str) {
		if self.panel.borrow_mut().set_query(query, Instant::now()) {
			self.wakes.push(());
		}
	}

	pub(crate) fn search_command(&self, state: &State, query: &str) -> Option<Command> {
		let panel = self.panel.borrow();
		(panel.query() == query.trim().to_lowercase())
			.then(|| panel.search_command(state))
			.flatten()
	}

	pub(crate) fn scroll_to_end(&self) {
		let serial = self.jumps.get() + 1;
		self.jumps.set(serial);
		self.jump.set(jump_value(serial, FAR_END));
	}

	pub(crate) fn flush(&self, state: &State) -> Option<MemberChange> {
		self.panel.borrow_mut().flush(state, Instant::now())
	}

	pub(crate) fn publish(&self, change: MemberChange, state: &State, icons: &Icons<'_>) -> Vec<String> {
		let panel = self.panel.borrow();
		if change.layout {
			let heights: Rc<[Value]> = panel.heights().map(Value::from).collect();
			self.heights.set(Value::List(heights));
		}
		let mut names = Vec::new();
		let rows: Rc<[Value]> = panel
			.entries()
			.iter()
			.enumerate()
			.map(|(offset, entry)| {
				let index = panel.first() + offset;
				match entry {
					MemberListEntry::Group(group) => record([
						("kind", "group".into()),
						("key", format!("g{}", group.id).as_str().into()),
						("title", group.title().as_str().into()),
					]),
					MemberListEntry::Member(member) => {
						let avatar = self.avatar(state, member.user_id, icons);
						names.extend(avatar.clone());
						member_value(member, avatar.as_deref())
					}
					MemberListEntry::Placeholder => record([
						("kind", "placeholder".into()),
						("key", format!("p{index}").as_str().into()),
						("index", (index as f64).into()),
					]),
				}
			})
			.collect();
		self.window.set(record([
			("first", (panel.first() as f64).into()),
			("rows", Value::List(rows)),
		]));
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
	host.function("members.scroll", move |argument| {
		if let (Some(link), Some(offset), Some(height)) = (
			actions.upgrade(),
			number(&argument, "offset"),
			number(&argument, "height"),
		) {
			let heights = number(&argument, "member")
				.zip(number(&argument, "group"))
				.filter(|(member, group)| *member > 0. && *group > 0.)
				.map(|(member, group)| MemberHeights {
					member: f64::from(member),
					group: f64::from(group),
				});
			if let Some(command) = link
				.members()
				.scroll(f64::from(offset), f64::from(height), heights)
			{
				link.send(command);
			}
		}
		Value::Nil
	});
	let actions = Rc::downgrade(link);
	host.action("members.search", move |argument| {
		if let (Some(link), Value::Str(query)) = (actions.upgrade(), &argument) {
			link.members().search(query);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("members.search_remote", move |argument| {
		if let (Some(link), Value::Str(query)) = (actions.upgrade(), &argument) {
			let command = link
				.state()
				.and_then(|shared| link.members().search_command(&shared.read(), query));
			if let Some(command) = command {
				link.send(command);
			}
		}
	});
	let actions = Rc::downgrade(link);
	host.action("members.open_profile", move |argument| {
		let user_id = match field(&argument, "id") {
			Some(Value::Str(text)) => text.parse::<u64>().ok().map(Snowflake),
			_ => None,
		};
		let at = (
			number(&argument, "x").unwrap_or(0.),
			number(&argument, "y").unwrap_or(0.),
		);
		if let (Some(link), Some(user_id)) = (actions.upgrade(), user_id) {
			link.open_member_profile(user_id, at);
		}
	});
}

fn member_value(member: &MemberEntry, avatar: Option<&str>) -> Value {
	let avatar = avatar.map(|name| format!("host:{name}")).unwrap_or_default();
	let color = member
		.color
		.map(|color| format!("#{:06x}", color & 0xff_ffff))
		.unwrap_or_default();
	record([
		("kind", "member".into()),
		("key", format!("m{}", member.user_id).as_str().into()),
		("id", member.user_id.to_string().as_str().into()),
		("name", member.name.as_str().into()),
		("extra", member.extra.as_deref().unwrap_or_default().into()),
		("color", color.as_str().into()),
		("status", member.status.as_str().into()),
		("activity", member.activity.as_deref().unwrap_or_default().into()),
		("bot", member.bot.into()),
		("badge", member.badge.as_deref().unwrap_or_default().into()),
		("avatar", avatar.as_str().into()),
	])
}

fn empty_window() -> Value {
	record([("first", 0.0.into()), ("rows", Value::List(Rc::new([])))])
}

fn jump_value(serial: u32, offset: f64) -> Value {
	record([("serial", f64::from(serial).into()), ("offset", offset.into())])
}
