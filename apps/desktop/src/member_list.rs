use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use bridge::{
	Avatar, Inbox, MemberChange, MemberEntry, MemberHeights, MemberListEntry, MemberPanel, OpenMembers,
};
use chrono::Utc;
use media::ImageName;
use protocol::Snowflake;
use session::{Badge, Command};
use state::{Changes, State};
use zgx_script::{ScriptHost, Shared, Value};

use crate::rail::Icons;
use crate::stable::StableRows;
use crate::ui::{Link, field, number, record};

const AVATAR_POINTS: f64 = 32.;
const FAR_END: f64 = 1e9;

pub(crate) struct MemberListView {
	panel: RefCell<MemberPanel>,
	avatars: RefCell<HashMap<(Avatar, bool), ImageName>>,
	hovered: RefCell<Option<String>>,
	wakes: Inbox<()>,
	jumps: Cell<u32>,
	heights: Shared,
	window: Shared,
	jump: Shared,
	stable: StableRows,
}

impl MemberListView {
	pub(crate) fn new(host: &ScriptHost) -> Self {
		Self {
			panel: RefCell::new(MemberPanel::default()),
			avatars: RefCell::new(HashMap::new()),
			hovered: RefCell::new(None),
			wakes: Inbox::new(),
			jumps: Cell::new(0),
			heights: host.shared("members.heights", Value::List(Rc::new([]))),
			window: host.shared("members.window", empty_window()),
			jump: host.shared("members.jump", jump_value(0, 0.)),
			stable: StableRows::default(),
		}
	}

	pub(crate) fn reset(&self) {
		self.panel.borrow_mut().reset();
		self.avatars.borrow_mut().clear();
		self.hovered.borrow_mut().take();
		self.stable.forget();
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

	pub(crate) fn set_moderator_notes(&self, shown: bool) {
		if self.panel.borrow_mut().set_moderator_notes(shown, Instant::now()) {
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
		let guild_id = panel.open().map(|open| open.guild_id);
		let rows: Vec<Value> = panel
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
						let avatar = state
							.user(member.user_id)
							.map(|user| Avatar::of(state, guild_id, user));
						let still = avatar.as_ref().map(|avatar| self.avatar(avatar, false, icons));
						let moving = avatar
							.as_ref()
							.filter(|avatar| avatar.is_animated())
							.map(|avatar| self.avatar(avatar, true, icons));
						names.extend(still.clone());
						member_value(member, still.as_deref(), moving.as_deref())
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
			("rows", Value::List(self.stable.rows(rows))),
		]));
		names
	}

	fn avatar(&self, avatar: &Avatar, animate: bool, icons: &Icons<'_>) -> String {
		let key = (avatar.clone(), animate);
		let mut avatars = self.avatars.borrow_mut();
		if let Some(name) = avatars.get(&key) {
			return name.as_str().to_owned();
		}
		let name = icons
			.gallery
			.request(avatar.request(icons.cdn, AVATAR_POINTS, icons.scale, animate));
		let text = name.as_str().to_owned();
		avatars.insert(key, name);
		text
	}

	pub(crate) fn hover_avatar(&self, name: &str, inside: bool) -> bool {
		let mut hovered = self.hovered.borrow_mut();
		match (inside, hovered.as_deref() == Some(name)) {
			(true, false) => {
				*hovered = Some(name.to_owned());
				true
			}
			(false, true) => {
				*hovered = None;
				true
			}
			_ => false,
		}
	}

	pub(crate) fn hovered_avatar(&self) -> Option<String> {
		self.hovered.borrow().clone()
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
	host.function("members.avatar_hover", move |argument| {
		let name = match field(&argument, "name") {
			Some(Value::Str(text)) => text.strip_prefix("host:").map(str::to_owned),
			_ => None,
		};
		let inside = matches!(field(&argument, "inside"), Some(Value::Bool(true)));
		if let (Some(link), Some(name)) = (actions.upgrade(), name) {
			link.hover_member_avatar(&name, inside);
		}
		Value::Nil
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

fn member_value(member: &MemberEntry, avatar: Option<&str>, moving: Option<&str>) -> Value {
	let avatar = avatar.map(|name| format!("host:{name}")).unwrap_or_default();
	let moving = moving.map(|name| format!("host:{name}")).unwrap_or_default();
	let timeout = member.notes.timeout_text(Utc::now()).unwrap_or_default();
	let notes = member.notes.flag_text().unwrap_or_default();
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
		("avatar_moving", moving.as_str().into()),
		("timeout", timeout.as_str().into()),
		("notes", notes.as_str().into()),
		("crown", member.crown.into()),
		("decorations", decorations_value(&member.decorations)),
	])
}

pub(crate) fn decorations_value(badges: &[Badge]) -> Value {
	Value::List(
		badges
			.iter()
			.map(|badge| {
				let source = badge
					.icon
					.as_deref()
					.map(|icon| format!("code:{icon}"))
					.unwrap_or_default();
				record([
					("key", format!("{}_{}", badge.plugin, badge.key).as_str().into()),
					("src", source.as_str().into()),
					(
						"color",
						format!("${}", crate::theme::token(&badge.color)).as_str().into(),
					),
					("tooltip", badge.tooltip.as_str().into()),
				])
			})
			.collect(),
	)
}

fn empty_window() -> Value {
	record([("first", 0.0.into()), ("rows", Value::List(Rc::new([])))])
}

fn jump_value(serial: u32, offset: f64) -> Value {
	record([("serial", f64::from(serial).into()), ("offset", offset.into())])
}
