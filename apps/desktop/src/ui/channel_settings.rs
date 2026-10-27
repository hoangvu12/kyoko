use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::{
	ARCHIVE_CHOICES, ChannelForm, ChannelPage, ChannelSettings, Inbox, OverwriteRow, SLOWMODE_CHOICES,
	channel_right_groups, channel_settings, invite_uses, overwrite_candidates, overwrite_from,
	overwrite_rows, right_flags,
};
use protocol::{PermissionOverwriteKind, Permissions, Snowflake};
use session::Command;
use state::{Changes, State, VIDEO_QUALITY_AUTO, VIDEO_QUALITY_FULL};
use zgx::reactive::{Runtime, Signal};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, record, text};
use crate::moderation::{Confirm, ConfirmAction};

const KBPS: u32 = 1_000;
const AUTOMATIC: &str = "Automatic";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Target {
	id: Snowflake,
	member: bool,
}

#[derive(Debug, Clone, Default)]
struct Open {
	channel_id: Option<Snowflake>,
	page: Option<ChannelPage>,
	editing: Option<Target>,
	query: String,
}

pub(crate) struct ChannelSettingsView {
	page: Shared,
	info: Shared,
	overview: Shared,
	overwrites: Shared,
	overwrite: Shared,
	candidates: Shared,
	invites: Shared,
	open: RefCell<Open>,
	shown: Cell<bool>,
	ready: Cell<bool>,
	wakes: Inbox<()>,
	images: RefCell<Vec<String>>,
	dimmed: Signal<bool>,
	_scrim: Rc<()>,
	_choices: Shared,
}

impl ChannelSettingsView {
	pub(crate) fn new(host: &ScriptHost, runtime: &Runtime) -> Self {
		let dimmed = runtime.signal(false);
		Self {
			page: host.shared("channel.page", ""),
			info: host.shared("channel.info", Value::Nil),
			overview: host.shared("channel.overview", Value::Nil),
			overwrites: host.shared("channel.overwrites", Value::Nil),
			overwrite: host.shared("channel.overwrite", Value::Nil),
			candidates: host.shared("channel.candidates", Value::List(Vec::new().into())),
			invites: host.shared("channel.invites", Value::Nil),
			open: RefCell::new(Open::default()),
			shown: Cell::new(false),
			ready: Cell::new(false),
			wakes: Inbox::new(),
			images: RefCell::new(Vec::new()),
			_scrim: crate::scrim::track(&dimmed),
			dimmed,
			_choices: host.shared("channel.choices", choices_value()),
		}
	}

	pub(crate) fn is_open(&self) -> bool {
		self.shown.get()
	}

	fn channel_id(&self) -> Option<Snowflake> {
		self.open.borrow().channel_id
	}
}

fn labels<T>(choices: &[(T, &str)]) -> Value {
	Value::List(choices.iter().map(|(_, label)| (*label).into()).collect())
}

fn choices_value() -> Value {
	record([
		("slowmode", labels(&SLOWMODE_CHOICES)),
		("archive", labels(&ARCHIVE_CHOICES)),
	])
}

fn snowflake_of(value: &Value) -> Option<Snowflake> {
	match value {
		Value::Str(text) => text.parse::<u64>().ok().map(Snowflake),
		_ => None,
	}
}

fn whole(value: &Value, key: &str) -> Option<u32> {
	match field(value, key)? {
		Value::Num(number) if number.is_finite() && *number >= 0. => Some(*number as u32),
		_ => None,
	}
}

fn truth(value: &Value, key: &str) -> Option<bool> {
	match field(value, key)? {
		Value::Bool(flag) => Some(*flag),
		_ => None,
	}
}

fn hex(color: u32) -> String {
	format!("#{color:06x}")
}

fn nearest(choices: &[(u32, &str)], value: u32) -> usize {
	choices
		.iter()
		.position(|(seconds, _)| *seconds >= value)
		.unwrap_or(choices.len() - 1)
}

fn regions(state: &State, guild_id: Snowflake, current: &str) -> Vec<(String, String)> {
	let mut found: Vec<(String, String)> = vec![(String::new(), AUTOMATIC.to_owned())];
	found.extend(
		state
			.voice_regions(guild_id)
			.unwrap_or_default()
			.iter()
			.map(|region| (region.id.clone(), region.name.clone())),
	);
	if !current.is_empty() && !found.iter().any(|(id, _)| id == current) {
		found.push((current.to_owned(), current.to_owned()));
	}
	found
}

fn wanted_form(settings: &ChannelSettings, regions: &[(String, String)], argument: &Value) -> ChannelForm {
	let mut form = settings.form.clone();
	if let Some(name) = field(argument, "name").and_then(text) {
		form.name = name.to_owned();
	}
	if let Some(topic) = field(argument, "topic").and_then(text) {
		form.topic = topic.to_owned();
	}
	if let Some(index) = whole(argument, "slowmode") {
		form.slowmode = SLOWMODE_CHOICES
			.get(index.saturating_sub(1) as usize)
			.map_or(form.slowmode, |(seconds, _)| *seconds);
	}
	match field(argument, "visibility").and_then(text) {
		Some("default") => (form.nsfw, form.spoiler) = (false, false),
		Some("spoiler") => (form.nsfw, form.spoiler) = (false, true),
		Some("age") => (form.nsfw, form.spoiler) = (true, false),
		_ => {}
	}
	if let Some(nsfw) = truth(argument, "nsfw") {
		form.nsfw = nsfw;
	}
	if let Some(announcement) = truth(argument, "announcement") {
		form.announcement = announcement;
	}
	if let Some(index) = whole(argument, "archive") {
		form.auto_archive = ARCHIVE_CHOICES
			.get(index.saturating_sub(1) as usize)
			.map_or(form.auto_archive, |(minutes, _)| *minutes);
	}
	if let Some(kbps) = whole(argument, "bitrate") {
		form.bitrate = (kbps * KBPS).clamp(settings.bitrate_min, settings.bitrate_max);
	}
	if let Some(limit) = whole(argument, "user_limit") {
		form.user_limit = limit.min(settings.user_limit_max);
	}
	if let Some(index) = whole(argument, "region")
		&& let Some((id, _)) = regions.get(index.saturating_sub(1) as usize)
	{
		form.region = id.clone();
	}
	if let Some(quality) = whole(argument, "video_quality").map(i64::from)
		&& [VIDEO_QUALITY_AUTO, VIDEO_QUALITY_FULL].contains(&quality)
	{
		form.video_quality = quality;
	}
	if let Some(index) = whole(argument, "category").filter(|index| *index > 0)
		&& let Some((id, _)) = settings.categories.get(index as usize - 1)
	{
		form.parent_id = Some(*id);
	}
	form
}

fn row_value(row: &OverwriteRow) -> Value {
	record([
		(
			"key",
			format!("{}|{}", row.id, if row.member { "m" } else { "r" })
				.as_str()
				.into(),
		),
		("id", row.id.to_string().as_str().into()),
		("name", row.name.as_str().into()),
		("color", row.color.map(hex).unwrap_or_default().as_str().into()),
		("member", row.member.into()),
		("everyone", row.everyone.into()),
		("editable", row.editable.into()),
	])
}

fn choice_map(allow: Permissions, deny: Permissions) -> Value {
	let mut entries: Vec<(Rc<str>, Value)> = right_flags()
		.iter()
		.enumerate()
		.filter_map(|(index, flag)| {
			let choice = if allow.contains(*flag) {
				1.
			} else if deny.contains(*flag) {
				-1.
			} else {
				return None;
			};
			Some((Rc::from((index + 1).to_string().as_str()), Value::Num(choice)))
		})
		.collect();
	entries.sort_by(|left, right| left.0.cmp(&right.0));
	Value::Map(entries.into_iter().collect())
}

impl Link {
	pub(super) fn install_channel_settings(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("channel.open", move |argument| {
			if let Some(link) = actions.upgrade() {
				let page = text(&argument).and_then(ChannelPage::named);
				link.switch_channel_page(page);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.close", move |_| {
			if let Some(link) = actions.upgrade() {
				link.close_channel_settings();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.save", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.save_channel(&argument);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.delete", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(channel_id) = link.channel_settings.channel_id()
			{
				link.confirm_delete_channel(channel_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.overwrite_open", move |argument| {
			if let Some(link) = actions.upgrade() {
				let target = field(&argument, "id").and_then(snowflake_of).map(|id| Target {
					id,
					member: truth(&argument, "member") == Some(true),
				});
				link.channel_settings.open.borrow_mut().editing = target;
				link.republish_channel();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.overwrite_add", move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(id) = field(&argument, "id").and_then(snowflake_of)
			{
				let member = truth(&argument, "member") == Some(true);
				link.add_overwrite(Target { id, member });
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.overwrite_save", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.save_overwrite(&argument);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.overwrite_remove", move |_| {
			if let Some(link) = actions.upgrade() {
				link.remove_overwrite();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.candidates", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.channel_settings.open.borrow_mut().query =
					text(&argument).unwrap_or_default().to_owned();
				link.republish_channel();
			}
		});
		host.action("channel.invite_copy", move |argument| {
			if let Some(code) = text(&argument) {
				crate::toasts::copy(&format!("https://discord.gg/{code}"), "Invite link copied");
			}
		});
		let actions = Rc::downgrade(link);
		host.action("channel.invite_revoke", move |argument| {
			if let (Some(link), Some(code)) = (actions.upgrade(), text(&argument))
				&& let Some(channel_id) = link.channel_settings.channel_id()
			{
				link.moderation.confirm(Confirm {
					title: "Revoke this invite?".to_owned(),
					body: format!("The invite {code} will stop working for everyone."),
					label: "Revoke Invite",
					danger: true,
					action: ConfirmAction::RevokeChannelInvite {
						channel_id,
						code: code.to_owned(),
					},
				});
			}
		});
	}

	pub(crate) fn open_channel_settings(&self, channel_id: Snowflake, page: Option<ChannelPage>) {
		let Some(shared) = self.state() else {
			return;
		};
		let Some(settings) = channel_settings(&shared.read(), channel_id) else {
			return;
		};
		let page = page
			.filter(|page| settings.pages.contains(page))
			.unwrap_or(ChannelPage::Overview);
		*self.channel_settings.open.borrow_mut() = Open {
			channel_id: Some(channel_id),
			page: Some(page),
			..Open::default()
		};
		self.load_channel_page(&shared.read(), &settings, page);
		if !self.channel_settings.shown.replace(true) {
			self.channel_settings.dimmed.set(true);
		}
		self.republish_channel();
	}

	fn switch_channel_page(&self, page: Option<ChannelPage>) {
		let Some(channel_id) = self.channel_settings.channel_id() else {
			return;
		};
		self.open_channel_settings(channel_id, page);
	}

	fn load_channel_page(&self, state: &State, settings: &ChannelSettings, page: ChannelPage) {
		match page {
			ChannelPage::Overview if settings.audio && state.voice_regions(settings.guild_id).is_none() => {
				self.send(Command::LoadVoiceRegions {
					guild_id: settings.guild_id,
				});
			}
			ChannelPage::Invites => self.send(Command::LoadChannelInvites {
				channel_id: settings.id,
			}),
			_ => {}
		}
	}

	pub(crate) fn close_channel_settings(&self) {
		if !self.channel_settings.shown.replace(false) {
			return;
		}
		*self.channel_settings.open.borrow_mut() = Open::default();
		self.channel_settings.page.set("".into());
		self.channel_settings.images.borrow_mut().clear();
		self.channel_settings.dimmed.set(false);
		self.refresh_visible();
	}

	fn save_channel(&self, argument: &Value) {
		let (Some(shared), Some(channel_id)) = (self.state(), self.channel_settings.channel_id()) else {
			return;
		};
		let changes = {
			let state = shared.read();
			let Some(settings) = channel_settings(&state, channel_id) else {
				return;
			};
			let regions = regions(&state, settings.guild_id, &settings.form.region);
			settings.form.changes(&wanted_form(&settings, &regions, argument))
		};
		if !changes.is_empty() {
			self.send(Command::EditChannel { channel_id, changes });
		}
		self.republish_channel();
	}

	fn add_overwrite(&self, target: Target) {
		let Some(channel_id) = self.channel_settings.channel_id() else {
			return;
		};
		self.send(Command::SetChannelOverwrite {
			channel_id,
			target_id: target.id,
			kind: if target.member {
				PermissionOverwriteKind::Member
			} else {
				PermissionOverwriteKind::Role
			},
			allow: Permissions::empty(),
			deny: Permissions::empty(),
		});
		let mut open = self.channel_settings.open.borrow_mut();
		open.editing = Some(target);
		open.query.clear();
		drop(open);
		self.republish_channel();
	}

	fn save_overwrite(&self, argument: &Value) {
		let (Some(shared), Some(channel_id)) = (self.state(), self.channel_settings.channel_id()) else {
			return;
		};
		let Some(target) = self.channel_settings.open.borrow().editing else {
			return;
		};
		let base = overwrite_rows(&shared.read(), channel_id)
			.into_iter()
			.find(|row| row.id == target.id)
			.map_or((Permissions::empty(), Permissions::empty()), |row| {
				(row.allow, row.deny)
			});
		let choices: Vec<(usize, i8)> = match field(argument, "choices") {
			Some(Value::List(entries)) => entries
				.iter()
				.filter_map(|entry| {
					let index = whole(entry, "index")? as usize;
					let value = match field(entry, "value") {
						Some(Value::Num(value)) => value.signum() as i8,
						_ => 0,
					};
					Some((index, value))
				})
				.collect(),
			_ => return,
		};
		let (allow, deny) = overwrite_from(base, &choices);
		if (allow, deny) == base {
			return;
		}
		self.send(Command::SetChannelOverwrite {
			channel_id,
			target_id: target.id,
			kind: if target.member {
				PermissionOverwriteKind::Member
			} else {
				PermissionOverwriteKind::Role
			},
			allow,
			deny,
		});
	}

	fn remove_overwrite(&self) {
		let Some(channel_id) = self.channel_settings.channel_id() else {
			return;
		};
		let Some(target) = self.channel_settings.open.borrow_mut().editing.take() else {
			return;
		};
		self.send(Command::DeleteChannelOverwrite {
			channel_id,
			target_id: target.id,
		});
		self.republish_channel();
	}

	pub(super) fn channel_settings_apply(&self, state: &State, changes: &Changes) {
		let Some(channel_id) = self.channel_settings.channel_id() else {
			return;
		};
		let guild_id = state.channel_guild(channel_id);
		let touched = changes.guild_list
			|| changes.current_user
			|| changes.loaded.contains(channel_id)
			|| guild_id.is_none_or(|guild_id| {
				changes.loaded.contains(guild_id)
					|| changes.members.contains(guild_id)
					|| changes.channel_lists.contains(guild_id)
			});
		if touched {
			self.publish_channel(state);
		}
	}

	fn republish_channel(&self) {
		self.channel_settings.wakes.push(());
	}

	pub(super) async fn follow_channel_settings(self: Rc<Self>) {
		while self.channel_settings.wakes.next().await.is_some() {
			if let Some(shared) = self.state() {
				let state = shared.read();
				self.window.runtime.batch(|| self.publish_channel(&state));
			}
		}
	}

	fn publish_channel(&self, state: &State) {
		let open = self.channel_settings.open.borrow().clone();
		let (Some(channel_id), Some(page)) = (open.channel_id, open.page) else {
			return;
		};
		let Some(settings) = channel_settings(state, channel_id) else {
			self.close_channel_settings();
			return;
		};
		let pages: Vec<Value> = settings
			.pages
			.iter()
			.map(|page| {
				record([
					("key", page.key().into()),
					("title", page.title().into()),
					("icon", page.icon().into()),
				])
			})
			.collect();
		self.channel_settings.info.set(record([
			("id", channel_id.to_string().as_str().into()),
			("name", settings.form.name.as_str().into()),
			("label", settings.label.into()),
			("icon", settings.icon.into()),
			("pages", Value::List(pages.into())),
		]));
		let mut images = Vec::new();
		let ready = match page {
			ChannelPage::Overview => {
				self.publish_channel_overview(state, &settings);
				!settings.audio || state.voice_regions(settings.guild_id).is_some()
			}
			ChannelPage::Permissions => {
				self.publish_overwrites(state, &settings, &open);
				true
			}
			ChannelPage::Invites => {
				self.publish_channel_invites(state, &settings, &mut images);
				state.channel_invites(channel_id).is_some()
			}
		};
		self.channel_settings.ready.set(ready);
		self.channel_settings.page.set(page.key().into());
		if *self.channel_settings.images.borrow() != images {
			*self.channel_settings.images.borrow_mut() = images;
			self.refresh_visible();
		}
	}

	fn publish_channel_overview(&self, state: &State, settings: &ChannelSettings) {
		let form = &settings.form;
		let regions = regions(state, settings.guild_id, &form.region);
		let region = regions.iter().position(|(id, _)| *id == form.region).unwrap_or(0);
		let category = form
			.parent_id
			.and_then(|parent| settings.categories.iter().position(|(id, _)| *id == parent))
			.map_or(0, |index| index + 1);
		let visibility = if form.nsfw {
			"age"
		} else if form.spoiler {
			"spoiler"
		} else {
			"default"
		};
		self.channel_settings.overview.set(record([
			("id", settings.id.to_string().as_str().into()),
			("name", form.name.as_str().into()),
			("topic", form.topic.as_str().into()),
			(
				"slowmode",
				Value::Num((nearest(&SLOWMODE_CHOICES, form.slowmode) + 1) as f64),
			),
			("visibility", visibility.into()),
			("nsfw", form.nsfw.into()),
			("announcement", form.announcement.into()),
			(
				"archive",
				Value::Num((nearest(&ARCHIVE_CHOICES, form.auto_archive) + 1) as f64),
			),
			("bitrate", Value::Num(f64::from(form.bitrate / KBPS))),
			("bitrate_min", Value::Num(f64::from(settings.bitrate_min / KBPS))),
			("bitrate_max", Value::Num(f64::from(settings.bitrate_max / KBPS))),
			("user_limit", Value::Num(f64::from(form.user_limit))),
			("user_limit_max", Value::Num(f64::from(settings.user_limit_max))),
			("video_quality", Value::Num(form.video_quality as f64)),
			("region", Value::Num((region + 1) as f64)),
			(
				"regions",
				Value::List(regions.iter().map(|(_, name)| name.as_str().into()).collect()),
			),
			("category", Value::Num(category as f64)),
			(
				"categories",
				Value::List(
					settings
						.categories
						.iter()
						.map(|(_, name)| name.as_str().into())
						.collect(),
				),
			),
			("audio", settings.audio.into()),
			("has_topic", settings.topic.into()),
			("archives", settings.archives.into()),
			("announces", settings.announces.into()),
		]));
	}

	fn publish_overwrites(&self, state: &State, settings: &ChannelSettings, open: &Open) {
		let rows = overwrite_rows(state, settings.id);
		let groups: Vec<Value> = channel_right_groups(settings.kind)
			.iter()
			.map(|group| {
				let rights = group
					.rights
					.iter()
					.map(|right| {
						record([
							("index", Value::Num(right.index as f64)),
							("name", right.name.into()),
							("hint", right.hint.into()),
						])
					})
					.collect();
				record([("title", group.title.into()), ("rights", Value::List(rights))])
			})
			.collect();
		let editing = open.editing.and_then(|target| {
			rows.iter().find(|row| row.id == target.id).cloned().or_else(|| {
				overwrite_candidates(state, settings.id, "")
					.into_iter()
					.find(|candidate| candidate.id == target.id)
					.map(|candidate| OverwriteRow {
						id: candidate.id,
						member: candidate.member,
						everyone: false,
						name: candidate.name,
						color: candidate.color,
						allow: Permissions::empty(),
						deny: Permissions::empty(),
						editable: true,
					})
			})
		});
		self.channel_settings.overwrites.set(record([
			("rows", Value::List(rows.iter().map(row_value).collect())),
			("groups", Value::List(groups.into())),
			(
				"editing",
				editing
					.as_ref()
					.map(|row| row.id.to_string())
					.unwrap_or_default()
					.as_str()
					.into(),
			),
		]));
		self.channel_settings.overwrite.set(match &editing {
			Some(row) => record([
				("id", row.id.to_string().as_str().into()),
				("name", row.name.as_str().into()),
				("color", row.color.map(hex).unwrap_or_default().as_str().into()),
				("member", row.member.into()),
				("everyone", row.everyone.into()),
				("editable", row.editable.into()),
				("choices", choice_map(row.allow, row.deny)),
				(
					"serial",
					format!("{}|{}|{}", row.id, row.allow.bits(), row.deny.bits())
						.as_str()
						.into(),
				),
			]),
			None => Value::Nil,
		});
		let candidates: Vec<Value> = overwrite_candidates(state, settings.id, &open.query)
			.iter()
			.map(|candidate| {
				record([
					("id", candidate.id.to_string().as_str().into()),
					("name", candidate.name.as_str().into()),
					(
						"color",
						candidate.color.map(hex).unwrap_or_default().as_str().into(),
					),
					("member", candidate.member.into()),
				])
			})
			.collect();
		self.channel_settings
			.candidates
			.set(Value::List(candidates.into()));
	}

	fn publish_channel_invites(&self, state: &State, settings: &ChannelSettings, images: &mut Vec<String>) {
		let today = bridge::today();
		let loaded = state.channel_invites(settings.id);
		let rows: Vec<Value> = loaded
			.unwrap_or_default()
			.iter()
			.map(|invite| {
				let avatar = invite.inviter_id.and_then(|id| self.server_avatar(state, id));
				images.extend(avatar.clone());
				let expires = invite
					.expires_at
					.map_or_else(|| "Never".to_owned(), |at| bridge::message_time(at, today));
				let inviter = invite
					.inviter_id
					.map(|id| state.display_name(Some(settings.guild_id), id))
					.unwrap_or_default();
				let color = invite
					.inviter_id
					.and_then(|id| state.role_color(settings.guild_id, id))
					.map(hex)
					.unwrap_or_default();
				record([
					("id", invite.code.as_str().into()),
					("inviter", inviter.as_str().into()),
					("color", color.as_str().into()),
					(
						"avatar",
						avatar
							.map(|name| format!("host:{name}"))
							.unwrap_or_default()
							.as_str()
							.into(),
					),
					("channel", "".into()),
					("uses", invite_uses(invite).as_str().into()),
					("expires", expires.as_str().into()),
					("temporary", invite.temporary.into()),
				])
			})
			.collect();
		self.channel_settings.invites.set(record([
			("loaded", loaded.is_some().into()),
			("rows", Value::List(rows.into())),
		]));
	}

	pub(super) fn drive_channel_settings(&self, page: &str) {
		let Some(shared) = self.state() else {
			return;
		};
		let channel_id = {
			let state = shared.read();
			if page == "voice" {
				self.open_guild()
					.and_then(|guild_id| state.guild(guild_id))
					.and_then(|guild| {
						let mut voice: Vec<_> = guild
							.channels
							.values()
							.filter(|channel| channel.kind == protocol::ChannelType::GuildVoice)
							.map(|channel| (channel.position.unwrap_or(0), channel.id))
							.collect();
						voice.sort();
						voice.first().map(|(_, id)| *id)
					})
			} else {
				self.timeline.borrow().open().map(|open| open.channel_id)
			}
		};
		if let Some(channel_id) = channel_id {
			self.open_channel_settings(channel_id, ChannelPage::named(page));
		}
	}

	pub(super) fn channel_settings_images(&self) -> Vec<String> {
		if !self.channel_settings.is_open() {
			return Vec::new();
		}
		self.channel_settings.images.borrow().clone()
	}

	pub(super) fn channel_settings_settled(&self) -> bool {
		let shown = self.shown.borrow();
		self.channel_settings.ready.get()
			&& self
				.channel_settings
				.images
				.borrow()
				.iter()
				.all(|name| shown.contains(name))
	}
}
