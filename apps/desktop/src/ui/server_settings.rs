use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use bridge::{
	AUDIT_FILTERS, GuildAccess, Inbox, MODERATION_PERMISSIONS, RIGHT_GROUPS, RoleChip, SERVER_GROUPS,
	ServerPage, audit_filter, audit_rows, ban_rows, member_rows, moderation_items, role_color, role_rows,
	role_swap,
};
use chrono::Utc;
use media::ImageName;
use protocol::{ChannelType, Permissions, Snowflake};
use session::Command;
use state::{Changes, State};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, Slot, field, number, record, text};
use crate::moderation::{ChannelDraft, Confirm, ConfirmAction, MemberMenu, Request};

const ROW_AVATAR: f64 = 32.;
const ICON_SIDE: u32 = 512;
const ICON_LIMIT: u64 = 10 * 1024 * 1024;
const ICON_PREVIEW: &str = "server/icon";
const UPLOAD_PREVIEW: &str = "server/upload";
const EMOJI_SIDE: u32 = 128;
const STICKER_SIDE: u32 = 320;
const EMOJI_POINTS: f64 = 48.;
const STICKER_POINTS: f64 = 104.;
const EMOJI_SLOTS: usize = 50;
const STICKER_SLOTS: usize = 5;
const EXPRESSION_NAME_MAX: usize = 32;
const ICON_POINTS: f64 = 96.;
const TINT_ALPHA: u32 = 0x2e;

struct Upload {
	png: Vec<u8>,
	name: String,
	serial: u64,
}

#[derive(Debug, Clone, Default)]
struct Open {
	guild_id: Option<Snowflake>,
	page: Option<ServerPage>,
	members_query: String,
	bans_query: String,
	audit_filter: usize,
	role: Option<Snowflake>,
}

pub(crate) struct ServerSettingsView {
	page: Shared,
	guild: Shared,
	overview: Shared,
	roles: Shared,
	role: Shared,
	members: Shared,
	bans: Shared,
	audit: Shared,
	emoji: Shared,
	stickers: Shared,
	invites: Shared,
	upload: RefCell<Option<Upload>>,
	open: RefCell<Open>,
	shown: Cell<bool>,
	ready: Cell<bool>,
	wakes: Inbox<()>,
	avatars: RefCell<HashMap<(Snowflake, Option<String>), ImageName>>,
	icon: RefCell<Option<ImageName>>,
	pending_icon: RefCell<Option<Vec<u8>>>,
	images: RefCell<Vec<String>>,
	_rights: Shared,
	_filters: Shared,
}

impl ServerSettingsView {
	pub(crate) fn new(host: &ScriptHost) -> Self {
		Self {
			page: host.shared("server.page", ""),
			guild: host.shared("server.guild", Value::Nil),
			overview: host.shared("server.overview", Value::Nil),
			roles: host.shared("server.roles", Value::Nil),
			role: host.shared("server.role", Value::Nil),
			members: host.shared("server.members", Value::Nil),
			bans: host.shared("server.bans", Value::Nil),
			audit: host.shared("server.audit", Value::Nil),
			emoji: host.shared("server.emoji", Value::Nil),
			stickers: host.shared("server.stickers", Value::Nil),
			invites: host.shared("server.invites", Value::Nil),
			upload: RefCell::new(None),
			open: RefCell::new(Open::default()),
			shown: Cell::new(false),
			ready: Cell::new(false),
			wakes: Inbox::new(),
			avatars: RefCell::new(HashMap::new()),
			icon: RefCell::new(None),
			pending_icon: RefCell::new(None),
			images: RefCell::new(Vec::new()),
			_rights: host.shared("server.rights", rights_value()),
			_filters: host.shared("server.audit_filters", audit_filters_value()),
		}
	}

	pub(crate) fn is_open(&self) -> bool {
		self.shown.get()
	}

	fn guild_id(&self) -> Option<Snowflake> {
		self.open.borrow().guild_id
	}
}

fn rights_value() -> Value {
	let mut index = 0;
	Value::List(
		RIGHT_GROUPS
			.iter()
			.map(|group| {
				let rights = group
					.rights
					.iter()
					.map(|right| {
						index += 1;
						record([
							("index", Value::Num(f64::from(index))),
							("name", right.name.into()),
							("hint", right.hint.into()),
						])
					})
					.collect();
				record([("title", group.title.into()), ("rights", Value::List(rights))])
			})
			.collect(),
	)
}

fn right_flags() -> Vec<Permissions> {
	RIGHT_GROUPS
		.iter()
		.flat_map(|group| group.rights.iter().map(|right| right.flag))
		.collect()
}

fn audit_filters_value() -> Value {
	Value::List(AUDIT_FILTERS.iter().map(|(_, label)| (*label).into()).collect())
}

fn hex(color: u32) -> String {
	format!("#{color:06x}")
}

fn tint(color: u32) -> String {
	format!("#{color:06x}{TINT_ALPHA:02x}")
}

fn chip_value(chip: &RoleChip) -> Value {
	record([
		("name", chip.name.as_str().into()),
		("color", chip.color.map(hex).unwrap_or_default().as_str().into()),
		("tint", chip.color.map(tint).unwrap_or_default().as_str().into()),
	])
}

fn snowflake_of(value: &Value) -> Option<Snowflake> {
	match value {
		Value::Str(text) => text.parse::<u64>().ok().map(Snowflake),
		_ => None,
	}
}

fn id_field(value: &Value, key: &str) -> Option<Snowflake> {
	field(value, key).and_then(snowflake_of)
}

impl Link {
	pub(super) fn install_server_settings(host: &ScriptHost, link: &Rc<Link>) {
		let hooks = Rc::downgrade(link);
		link.moderation.on_request(move |request| {
			if let Some(link) = hooks.upgrade() {
				link.moderation_request(request);
			}
		});
		let window = Rc::downgrade(link);
		let bounds = link.window.info.bounds.clone();
		let effect = link.window.runtime.effect_now(move || {
			let (width, height) = bounds.get().size;
			if let Some(link) = window.upgrade() {
				let scale = link.scale_factor() as f32;
				if scale > 0. {
					link.moderation
						.set_window(width as f32 / scale, height as f32 / scale);
				}
			}
		});
		*link.moderation_effect.borrow_mut() = Some(effect);
		Link::install_sidebar_admin(host, link);
		Link::install_server_actions(host, link);
	}

	fn install_sidebar_admin(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("sidebar.server_settings", move |_| {
			if let Some(link) = actions.upgrade() {
				link.open_server_settings(None);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.create_channel", move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(guild_id) = link.sidebar().sidebar().selected()
			{
				link.moderation.create_channel(guild_id, snowflake_of(&argument));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.edit_channel", move |argument| {
			if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake_of(&argument)) {
				link.edit_channel(channel_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.delete_channel", move |argument| {
			if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake_of(&argument)) {
				link.confirm_delete_channel(channel_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("members.menu", move |argument| {
			if let (Some(link), Some(user_id), Some(x), Some(y)) = (
				actions.upgrade(),
				id_field(&argument, "id"),
				number(&argument, "x"),
				number(&argument, "y"),
			) {
				link.open_member_menu(user_id, (x, y), true);
			}
		});
	}

	fn install_server_actions(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("server.open", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.open_server_settings(text(&argument).and_then(ServerPage::named));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.close", move |_| {
			if let Some(link) = actions.upgrade() {
				link.close_server_settings();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.save_overview", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.save_overview(&argument);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.icon", move |argument| {
			if let (Some(link), Some(path)) = (actions.upgrade(), text(&argument)) {
				link.pick_icon(std::path::Path::new(path));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.reset_icon", move |_| {
			if let Some(link) = actions.upgrade() {
				link.server.pending_icon.borrow_mut().take();
				link.republish_server();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.upload_pick", move |argument| {
			if let (Some(link), Some(path)) = (actions.upgrade(), text(&argument)) {
				link.pick_upload(std::path::Path::new(path));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.upload_cancel", move |_| {
			if let Some(link) = actions.upgrade() {
				link.server.upload.borrow_mut().take();
				link.republish_server();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.upload_send", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.send_upload(&argument);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.emoji_delete", move |argument| {
			if let (Some(link), Some(id)) = (actions.upgrade(), id_field(&argument, "id")) {
				let name = field(&argument, "name")
					.and_then(text)
					.unwrap_or_default()
					.to_owned();
				link.confirm_expression(true, id, &name);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.sticker_delete", move |argument| {
			if let (Some(link), Some(id)) = (actions.upgrade(), id_field(&argument, "id")) {
				let name = field(&argument, "name")
					.and_then(text)
					.unwrap_or_default()
					.to_owned();
				link.confirm_expression(false, id, &name);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.sticker_edit", move |argument| {
			if let (Some(link), Some(sticker_id)) = (actions.upgrade(), snowflake_of(&argument)) {
				link.edit_sticker(sticker_id);
			}
		});
		host.action("server.invite_copy", move |argument| {
			if let Some(code) = text(&argument)
				&& let Err(error) = platform::clipboard::write_text(&format!("https://discord.gg/{code}"))
			{
				tracing::warn!(%error, "the invite link was not copied");
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.invite_revoke", move |argument| {
			if let (Some(link), Some(code)) = (actions.upgrade(), text(&argument)) {
				link.confirm_revoke(code);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.role_open", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.server.open.borrow_mut().role = snowflake_of(&argument);
				link.republish_server();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.role_create", move |_| {
			if let Some(link) = actions.upgrade() {
				link.create_role();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.role_delete", move |argument| {
			if let (Some(link), Some(role_id)) = (actions.upgrade(), snowflake_of(&argument)) {
				link.confirm_delete_role(role_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.role_move", move |argument| {
			if let (Some(link), Some(role_id)) = (actions.upgrade(), id_field(&argument, "id")) {
				let up = matches!(field(&argument, "up"), Some(Value::Bool(true)));
				link.move_role(role_id, up);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.role_save", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.save_role(&argument);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.members_search", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.server.open.borrow_mut().members_query = text(&argument).unwrap_or_default().to_owned();
				link.republish_server();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.members_remote", move |argument| {
			if let (Some(link), Some(query)) = (actions.upgrade(), text(&argument))
				&& let Some(guild_id) = link.server.guild_id()
				&& !query.trim().is_empty()
			{
				link.send(Command::SearchMembers {
					guild_id,
					query: query.trim().to_owned(),
				});
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.member_menu", move |argument| {
			if let (Some(link), Some(user_id), Some(x), Some(y)) = (
				actions.upgrade(),
				id_field(&argument, "id"),
				number(&argument, "x"),
				number(&argument, "y"),
			) {
				link.open_member_menu(user_id, (x, y), false);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.bans_search", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.server.open.borrow_mut().bans_query = text(&argument).unwrap_or_default().to_owned();
				link.republish_server();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.bans_more", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(guild_id) = link.server.guild_id()
			{
				link.send(Command::LoadMoreBans { guild_id });
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.unban", move |argument| {
			if let (Some(link), Some(user_id)) = (actions.upgrade(), id_field(&argument, "id")) {
				let name = field(&argument, "name")
					.and_then(text)
					.unwrap_or_default()
					.to_owned();
				link.confirm_unban(user_id, &name);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.audit_filter", move |argument| {
			if let (Some(link), Value::Num(index)) = (actions.upgrade(), &argument) {
				link.set_audit_filter(*index as usize);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("server.audit_more", move |_| {
			if let Some(link) = actions.upgrade() {
				link.load_more_audit();
			}
		});
	}

	fn moderation_request(&self, request: Request) {
		match request {
			Request::Command(command) => self.send(command),
			Request::Profile(user_id, at) => self.open_member_profile(user_id, at),
			Request::Message(user_id) => self.message_user(user_id),
			Request::CopyId(user_id) => {
				if let Err(error) = platform::clipboard::write_text(&user_id.to_string()) {
					tracing::warn!(%error, "the user id was not copied");
				}
			}
		}
	}

	fn moderation_guild(&self) -> Option<Snowflake> {
		self.server
			.guild_id()
			.filter(|_| self.server.is_open())
			.or_else(|| self.open_guild())
	}

	pub(crate) fn open_member_menu(&self, user_id: Snowflake, at: (f32, f32), head: bool) {
		let (Some(shared), Some(guild_id)) = (self.state(), self.moderation_guild()) else {
			return;
		};
		let menu = {
			let state = shared.read();
			let nick = state
				.guild(guild_id)
				.and_then(|guild| guild.members.get(&user_id))
				.and_then(|member| member.nick.clone())
				.unwrap_or_default();
			MemberMenu {
				guild_id,
				user_id,
				name: state.display_name(Some(guild_id), user_id),
				nick,
				head,
				developer: head && self.settings_view().settings().developer.developer_mode,
				items: moderation_items(&state, guild_id, user_id, Utc::now()),
			}
		};
		self.moderation.open_menu(menu, at);
	}

	pub(crate) fn open_moderation_dialog(&self, user_id: Snowflake, kind: &str) {
		let (Some(shared), Some(guild_id)) = (self.state(), self.open_guild()) else {
			return;
		};
		let (name, nick) = {
			let state = shared.read();
			let nick = state
				.guild(guild_id)
				.and_then(|guild| guild.members.get(&user_id))
				.and_then(|member| member.nick.clone());
			let name = state.display_name(Some(guild_id), user_id);
			(name.clone(), nick.unwrap_or(name))
		};
		match kind {
			"nickname" => self.moderation.nickname(guild_id, user_id, &nick),
			"timeout" => self.moderation.timeout(guild_id, user_id, &name),
			"kick" => self.moderation.kick(guild_id, user_id, &name),
			_ => self.moderation.ban(guild_id, user_id, &name),
		}
	}

	fn edit_channel(&self, channel_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let draft = {
			let state = shared.read();
			let Some(channel) = state.channel(channel_id) else {
				return;
			};
			let categories = channel
				.guild_id
				.and_then(|guild_id| state.guild(guild_id))
				.map(|guild| {
					let mut categories: Vec<(i64, Snowflake, String)> = guild
						.channels
						.values()
						.filter(|found| found.kind == ChannelType::GuildCategory)
						.map(|found| {
							(
								found.position.unwrap_or(0),
								found.id,
								found.name.clone().unwrap_or_default(),
							)
						})
						.collect();
					categories.sort();
					categories.into_iter().map(|(_, id, name)| (id, name)).collect()
				})
				.unwrap_or_default();
			ChannelDraft {
				channel_id,
				kind: channel.kind,
				name: channel.name.clone().unwrap_or_default(),
				topic: channel.topic.clone().unwrap_or_default(),
				slowmode: u32::try_from(channel.rate_limit_per_user.unwrap_or(0)).unwrap_or(0),
				nsfw: channel.nsfw == Some(true),
				parent_id: channel.parent_id,
				categories,
			}
		};
		self.moderation.edit_channel(draft);
	}

	fn confirm_delete_channel(&self, channel_id: Snowflake) {
		let name = self
			.state()
			.and_then(|shared| shared.read().channel(channel_id)?.name.clone())
			.unwrap_or_default();
		self.moderation.confirm(Confirm {
			title: format!("Delete #{name}?"),
			body: "This cannot be undone. The channel and its messages are removed for everyone.".to_owned(),
			label: "Delete Channel",
			danger: true,
			action: ConfirmAction::DeleteChannel(channel_id),
		});
	}

	fn confirm_delete_role(&self, role_id: Snowflake) {
		let Some(guild_id) = self.server.guild_id() else {
			return;
		};
		self.moderation.confirm(Confirm {
			title: "Delete this role?".to_owned(),
			body: "Members lose this role and the permissions that come with it.".to_owned(),
			label: "Delete Role",
			danger: true,
			action: ConfirmAction::DeleteRole { guild_id, role_id },
		});
		self.server.open.borrow_mut().role = None;
		self.republish_server();
	}

	fn confirm_unban(&self, user_id: Snowflake, name: &str) {
		let Some(guild_id) = self.server.guild_id() else {
			return;
		};
		self.moderation.confirm(Confirm {
			title: format!("Revoke the ban on {name}?"),
			body: "They will be able to join the server again with a new invite.".to_owned(),
			label: "Revoke Ban",
			danger: false,
			action: ConfirmAction::Unban { guild_id, user_id },
		});
	}

	fn pick_upload(&self, path: &std::path::Path) {
		let side = match self.server.open.borrow().page {
			Some(ServerPage::Stickers) => STICKER_SIDE,
			_ => EMOJI_SIDE,
		};
		let Some((png, preview)) = picked_png(path, side) else {
			tracing::warn!("the picked image could not be read");
			return;
		};
		let name: String = path
			.file_stem()
			.map(|stem| stem.to_string_lossy().into_owned())
			.unwrap_or_default()
			.chars()
			.filter(|character| character.is_ascii_alphanumeric() || *character == '_')
			.take(EXPRESSION_NAME_MAX)
			.collect();
		let serial = self
			.server
			.upload
			.borrow()
			.as_ref()
			.map_or(0, |upload| upload.serial)
			+ 1;
		self.host.image(UPLOAD_PREVIEW, Some(Arc::new(preview)));
		*self.server.upload.borrow_mut() = Some(Upload { png, name, serial });
		self.republish_server();
	}

	fn send_upload(&self, argument: &Value) {
		let Some(guild_id) = self.server.guild_id() else {
			return;
		};
		let page = self.server.open.borrow().page;
		let Some(upload) = self.server.upload.borrow_mut().take() else {
			return;
		};
		let text_of = |key: &str| {
			field(argument, key)
				.and_then(text)
				.unwrap_or_default()
				.trim()
				.to_owned()
		};
		let name = text_of("name");
		match page {
			Some(ServerPage::Stickers) => self.send(Command::CreateSticker {
				guild_id,
				filename: format!("{name}.png"),
				name,
				description: text_of("description"),
				tags: text_of("tags"),
				file_png_or_apng: upload.png,
			}),
			_ => self.send(Command::CreateEmoji {
				guild_id,
				name,
				image_png: upload.png,
			}),
		}
		self.republish_server();
	}

	fn confirm_expression(&self, emoji: bool, id: Snowflake, name: &str) {
		let Some(guild_id) = self.server.guild_id() else {
			return;
		};
		let confirm = if emoji {
			Confirm {
				title: "Delete emoji?".to_owned(),
				body: format!(":{name}: will be removed from this server."),
				label: "Delete",
				danger: true,
				action: ConfirmAction::DeleteEmoji {
					guild_id,
					emoji_id: id,
				},
			}
		} else {
			Confirm {
				title: "Delete sticker?".to_owned(),
				body: format!("{name} will be removed from this server."),
				label: "Delete",
				danger: true,
				action: ConfirmAction::DeleteSticker {
					guild_id,
					sticker_id: id,
				},
			}
		};
		self.moderation.confirm(confirm);
	}

	fn confirm_revoke(&self, code: &str) {
		let Some(guild_id) = self.server.guild_id() else {
			return;
		};
		self.moderation.confirm(Confirm {
			title: "Revoke this invite?".to_owned(),
			body: format!("The invite {code} will stop working for everyone."),
			label: "Revoke Invite",
			danger: true,
			action: ConfirmAction::RevokeInvite {
				guild_id,
				code: code.to_owned(),
			},
		});
	}

	fn edit_sticker(&self, sticker_id: Snowflake) {
		let (Some(shared), Some(guild_id)) = (self.state(), self.server.guild_id()) else {
			return;
		};
		let sticker = shared
			.read()
			.guild_stickers(guild_id)
			.and_then(|stickers| stickers.iter().find(|sticker| sticker.id == sticker_id).cloned());
		if let Some(sticker) = sticker {
			self.moderation.edit_sticker(guild_id, &sticker);
		}
	}

	pub(crate) fn open_server_settings(&self, page: Option<ServerPage>) {
		let (Some(shared), Some(guild_id)) = (self.state(), self.sidebar().sidebar().selected()) else {
			return;
		};
		let access = GuildAccess::of(&shared.read(), guild_id);
		let Some(page) = ServerPage::opening(page, &access) else {
			return;
		};
		*self.server.open.borrow_mut() = Open {
			guild_id: Some(guild_id),
			page: Some(page),
			..Open::default()
		};
		self.load_server_page(guild_id, page);
		if !self.server.shown.replace(true) {
			self.window.slot.set(Slot::default());
		}
		self.republish_server();
	}

	fn load_server_page(&self, guild_id: Snowflake, page: ServerPage) {
		match page {
			ServerPage::Bans => self.send(Command::LoadBans { guild_id }),
			ServerPage::AuditLog => self.send(Command::LoadAuditLog {
				guild_id,
				before: None,
				action_type: audit_filter(self.server.open.borrow().audit_filter),
			}),
			ServerPage::Invites => self.send(Command::LoadInvites { guild_id }),
			ServerPage::Stickers => self.send(Command::LoadGuildStickers { guild_id }),
			_ => {}
		}
	}

	pub(crate) fn close_server_settings(&self) {
		if !self.server.shown.replace(false) {
			return;
		}
		*self.server.open.borrow_mut() = Open::default();
		self.server.page.set("".into());
		self.server.images.borrow_mut().clear();
		self.server.icon.borrow_mut().take();
		self.server.pending_icon.borrow_mut().take();
		self.server.upload.borrow_mut().take();
		if self.dms.page().is_empty() && !self.settings_view().is_open() {
			self.window.slot.set(self.reported_slot.get());
		}
		self.refresh_visible();
	}

	fn save_overview(&self, argument: &Value) {
		let Some(guild_id) = self.server.guild_id() else {
			return;
		};
		let name = field(argument, "name").and_then(text).map(str::to_owned);
		let description = field(argument, "description").and_then(text).map(str::to_owned);
		let icon_png = self.server.pending_icon.borrow_mut().take();
		self.send(Command::EditGuild {
			guild_id,
			name,
			icon_png,
			description,
		});
		self.republish_server();
	}

	fn pick_icon(&self, path: &std::path::Path) {
		match picked_png(path, ICON_SIDE) {
			Some((png, preview)) => {
				self.host.image(ICON_PREVIEW, Some(Arc::new(preview)));
				*self.server.pending_icon.borrow_mut() = Some(png);
				self.republish_server();
			}
			None => tracing::warn!("the picked server icon could not be read"),
		}
	}

	fn create_role(&self) {
		let (Some(shared), Some(guild_id)) = (self.state(), self.server.guild_id()) else {
			return;
		};
		let permissions = shared
			.read()
			.guild(guild_id)
			.and_then(|guild| guild.roles.get(&guild_id))
			.map_or(Permissions::empty(), |role| role.permissions);
		self.send(Command::CreateRole {
			guild_id,
			name: "new role".to_owned(),
			color: 0,
			permissions,
			hoist: false,
			mentionable: false,
		});
	}

	fn move_role(&self, role_id: Snowflake, up: bool) {
		let (Some(shared), Some(guild_id)) = (self.state(), self.server.guild_id()) else {
			return;
		};
		let positions = role_swap(&shared.read(), guild_id, role_id, up);
		if let Some(positions) = positions {
			self.send(Command::ReorderRoles { guild_id, positions });
		}
	}

	fn save_role(&self, argument: &Value) {
		let (Some(shared), Some(guild_id), Some(role_id)) =
			(self.state(), self.server.guild_id(), id_field(argument, "id"))
		else {
			return;
		};
		let Some(role) = shared
			.read()
			.guild(guild_id)
			.and_then(|guild| guild.roles.get(&role_id).cloned())
		else {
			return;
		};
		let name = field(argument, "name")
			.and_then(text)
			.and_then(|name| bridge::trimmed(name, 100))
			.filter(|name| *name != role.name && role_id != guild_id);
		let color = field(argument, "color")
			.and_then(text)
			.and_then(parse_hex)
			.filter(|color| *color != role_color(&role));
		let hoist = match field(argument, "hoist") {
			Some(Value::Bool(hoist)) if *hoist != role.hoist => Some(*hoist),
			_ => None,
		};
		let mentionable = match field(argument, "mentionable") {
			Some(Value::Bool(mentionable)) if *mentionable != role.mentionable => Some(*mentionable),
			_ => None,
		};
		let permissions = match field(argument, "granted") {
			Some(Value::List(granted)) => {
				let flags = right_flags();
				let mut permissions = role.permissions;
				for (index, flag) in flags.iter().enumerate() {
					permissions.remove(*flag);
					let wanted = granted
						.iter()
						.any(|value| matches!(value, Value::Num(number) if *number as usize == index + 1));
					if wanted {
						permissions.insert(*flag);
					}
				}
				Some(permissions).filter(|permissions| *permissions != role.permissions)
			}
			_ => None,
		};
		if name.is_none()
			&& color.is_none()
			&& hoist.is_none()
			&& mentionable.is_none()
			&& permissions.is_none()
		{
			return;
		}
		self.send(Command::EditRole {
			guild_id,
			role_id,
			name,
			color,
			permissions,
			hoist,
			mentionable,
		});
	}

	fn set_audit_filter(&self, index: usize) {
		let Some(guild_id) = self.server.guild_id() else {
			return;
		};
		let index = index.min(AUDIT_FILTERS.len() - 1);
		self.server.open.borrow_mut().audit_filter = index;
		self.send(Command::LoadAuditLog {
			guild_id,
			before: None,
			action_type: audit_filter(index),
		});
		self.republish_server();
	}

	fn load_more_audit(&self) {
		let (Some(shared), Some(guild_id)) = (self.state(), self.server.guild_id()) else {
			return;
		};
		let before = shared
			.read()
			.audit_log(guild_id)
			.and_then(|log| log.entries.last().map(|entry| entry.id));
		self.send(Command::LoadAuditLog {
			guild_id,
			before,
			action_type: audit_filter(self.server.open.borrow().audit_filter),
		});
	}

	pub(super) fn server_settings_apply(&self, state: &State, changes: &Changes) {
		let Some(guild_id) = self.server.guild_id() else {
			return;
		};
		let touched = changes.guild_list
			|| changes.current_user
			|| changes.loaded.contains(guild_id)
			|| changes.members.contains(guild_id)
			|| changes.channel_lists.contains(guild_id);
		if touched {
			self.publish_server(state);
		}
	}

	fn republish_server(&self) {
		self.server.wakes.push(());
	}

	pub(super) async fn follow_server_settings(self: Rc<Self>) {
		while self.server.wakes.next().await.is_some() {
			if let Some(shared) = self.state() {
				let state = shared.read();
				self.window.runtime.batch(|| self.publish_server(&state));
			}
		}
	}

	fn publish_server(&self, state: &State) {
		let open = self.server.open.borrow().clone();
		let (Some(guild_id), Some(page)) = (open.guild_id, open.page) else {
			return;
		};
		let Some(guild) = state.guild(guild_id) else {
			drop(open);
			self.close_server_settings();
			return;
		};
		let access = GuildAccess::of(state, guild_id);
		let mut images = Vec::new();
		let groups: Vec<Value> = SERVER_GROUPS
			.iter()
			.filter_map(|(title, pages)| {
				let pages: Vec<Value> = pages
					.iter()
					.filter(|page| page.allowed(&access))
					.map(|page| {
						record([
							("key", page.key().into()),
							("title", page.title().into()),
							("icon", page.icon().into()),
						])
					})
					.collect();
				(!pages.is_empty())
					.then(|| record([("title", (*title).into()), ("pages", Value::List(pages.into()))]))
			})
			.collect();
		self.server.guild.set(record([
			("id", guild_id.to_string().as_str().into()),
			("name", guild.name.as_str().into()),
			("groups", Value::List(groups.into())),
		]));
		match page {
			ServerPage::Overview => {
				let icon = self.server_icon(state, guild_id, guild.icon.as_deref());
				images.extend(icon.clone());
				let pending = self.server.pending_icon.borrow().is_some();
				let shown = if pending {
					format!("host:{ICON_PREVIEW}")
				} else {
					icon.map(|name| format!("host:{name}")).unwrap_or_default()
				};
				self.server.overview.set(record([
					("name", guild.name.as_str().into()),
					(
						"description",
						guild.description.as_deref().unwrap_or_default().into(),
					),
					("icon", shown.as_str().into()),
					("pending", pending.into()),
				]));
			}
			ServerPage::Roles => self.publish_roles(state, guild_id, open.role),
			ServerPage::Members => {
				let (rows, total) = member_rows(state, guild_id, &open.members_query);
				let rows = rows
					.iter()
					.map(|row| {
						let avatar = self.server_avatar(state, row.id);
						images.extend(avatar.clone());
						record([
							("id", row.id.to_string().as_str().into()),
							("name", row.name.as_str().into()),
							("username", row.username.as_str().into()),
							("color", row.color.map(hex).unwrap_or_default().as_str().into()),
							(
								"avatar",
								avatar
									.map(|name| format!("host:{name}"))
									.unwrap_or_default()
									.as_str()
									.into(),
							),
							("roles", Value::List(row.roles.iter().map(chip_value).collect())),
						])
					})
					.collect();
				self.server.members.set(record([
					("total", Value::Num(total as f64)),
					("rows", Value::List(rows)),
					("actions", access.has_any(MODERATION_PERMISSIONS).into()),
				]));
			}
			ServerPage::Bans => {
				let list = state.ban_list(guild_id);
				let rows = ban_rows(state, guild_id, &open.bans_query)
					.iter()
					.map(|row| {
						let avatar = self.server_avatar(state, row.id);
						images.extend(avatar.clone());
						record([
							("id", row.id.to_string().as_str().into()),
							("name", row.name.as_str().into()),
							(
								"reason",
								row.reason.as_deref().unwrap_or("No reason given").into(),
							),
							(
								"avatar",
								avatar
									.map(|name| format!("host:{name}"))
									.unwrap_or_default()
									.as_str()
									.into(),
							),
						])
					})
					.collect();
				self.server.bans.set(record([
					("loaded", list.is_some().into()),
					("has_more", list.is_some_and(|list| list.has_more).into()),
					("rows", Value::List(rows)),
				]));
			}
			ServerPage::AuditLog => {
				let rows = audit_rows(state, guild_id, bridge::today());
				let has_more = state.audit_log(guild_id).is_some_and(|log| log.has_more);
				let rows: Vec<Value> = rows
					.iter()
					.flatten()
					.map(|row| {
						let avatar = row.actor_id.and_then(|id| self.server_avatar(state, id));
						images.extend(avatar.clone());
						let changes = row
							.changes
							.iter()
							.map(|change| {
								record([
									("key", change.key.as_str().into()),
									("old", change.old.as_str().into()),
									("new", change.new.as_str().into()),
								])
							})
							.collect();
						record([
							("id", row.id.to_string().as_str().into()),
							("sentence", row.sentence.as_str().into()),
							("when", row.when.as_str().into()),
							(
								"avatar",
								avatar
									.map(|name| format!("host:{name}"))
									.unwrap_or_default()
									.as_str()
									.into(),
							),
							("changes", Value::List(changes)),
							("reason", row.reason.as_deref().unwrap_or_default().into()),
						])
					})
					.collect();
				self.server.audit.set(record([
					("loaded", state.audit_log(guild_id).is_some().into()),
					("has_more", has_more.into()),
					("filter", Value::Num(open.audit_filter as f64)),
					("rows", Value::List(rows.into())),
				]));
			}
			ServerPage::Emoji => self.publish_emoji(state, guild_id, &mut images),
			ServerPage::Stickers => self.publish_stickers(state, guild_id, &mut images),
			ServerPage::Invites => self.publish_invites(state, guild_id, &mut images),
		}
		let ready = match page {
			ServerPage::Bans => state.ban_list(guild_id).is_some(),
			ServerPage::AuditLog => state.audit_log(guild_id).is_some(),
			ServerPage::Stickers => state.guild_stickers(guild_id).is_some(),
			ServerPage::Invites => !state.guild_invites(guild_id).is_empty(),
			ServerPage::Members => guild.members.len() > 1,
			_ => true,
		};
		self.server.ready.set(ready);
		self.server.page.set(page.key().into());
		if *self.server.images.borrow() != images {
			*self.server.images.borrow_mut() = images;
			self.refresh_visible();
		}
	}

	fn upload_value(&self) -> Value {
		match self.server.upload.borrow().as_ref() {
			Some(upload) => record([
				("serial", Value::Num(upload.serial as f64)),
				("name", upload.name.as_str().into()),
				("preview", format!("host:{UPLOAD_PREVIEW}").as_str().into()),
			]),
			None => Value::Nil,
		}
	}

	fn publish_emoji(&self, state: &State, guild_id: Snowflake, images: &mut Vec<String>) {
		let Some(guild) = state.guild(guild_id) else {
			return;
		};
		let scale = self.scale_factor();
		let animated = guild
			.emojis
			.iter()
			.filter(|emoji| emoji.animated == Some(true))
			.count();
		let rows: Vec<Value> = guild
			.emojis
			.iter()
			.filter_map(|emoji| {
				let (id, name) = (emoji.id?, emoji.name.clone()?);
				let request = bridge::emoji_request(
					&self.images.cdn,
					id,
					emoji.animated == Some(true),
					EMOJI_POINTS,
					scale,
				);
				let image = self.server_request(request).as_str().to_owned();
				images.push(image.clone());
				let uploader = emoji.user.as_ref().map(|user| user.id);
				let avatar = uploader.and_then(|id| self.server_avatar(state, id));
				images.extend(avatar.clone());
				let uploader_name = uploader
					.map(|id| state.display_name(Some(guild_id), id))
					.unwrap_or_default();
				Some(record([
					("id", id.to_string().as_str().into()),
					("name", name.as_str().into()),
					("image", format!("host:{image}").as_str().into()),
					("uploader", uploader_name.as_str().into()),
					(
						"avatar",
						avatar
							.map(|name| format!("host:{name}"))
							.unwrap_or_default()
							.as_str()
							.into(),
					),
				]))
			})
			.collect();
		self.server.emoji.set(record([
			("still", Value::Num((guild.emojis.len() - animated) as f64)),
			("animated", Value::Num(animated as f64)),
			("slots", Value::Num(EMOJI_SLOTS as f64)),
			("rows", Value::List(rows.into())),
			("upload", self.upload_value()),
		]));
	}

	fn publish_stickers(&self, state: &State, guild_id: Snowflake, images: &mut Vec<String>) {
		let scale = self.scale_factor();
		let stickers = state.guild_stickers(guild_id).unwrap_or_default();
		let rows: Vec<Value> = stickers
			.iter()
			.map(|sticker| {
				let image = bridge::sticker_request(&bridge::sticker_item(sticker), STICKER_POINTS, scale)
					.map(|request| self.server_request(request).as_str().to_owned());
				images.extend(image.clone());
				record([
					("id", sticker.id.to_string().as_str().into()),
					("name", sticker.name.as_str().into()),
					("tags", sticker.tags.as_deref().unwrap_or_default().into()),
					(
						"image",
						image
							.map(|name| format!("host:{name}"))
							.unwrap_or_default()
							.as_str()
							.into(),
					),
				])
			})
			.collect();
		self.server.stickers.set(record([
			("count", Value::Num(stickers.len() as f64)),
			("slots", Value::Num(STICKER_SLOTS as f64)),
			("rows", Value::List(rows.into())),
			("upload", self.upload_value()),
		]));
	}

	fn publish_invites(&self, state: &State, guild_id: Snowflake, images: &mut Vec<String>) {
		let today = bridge::today();
		let rows: Vec<Value> = state
			.guild_invites(guild_id)
			.iter()
			.map(|invite| {
				let avatar = invite.inviter_id.and_then(|id| self.server_avatar(state, id));
				images.extend(avatar.clone());
				let uses = if invite.max_uses > 0 {
					format!("{} / {}", invite.uses, invite.max_uses)
				} else {
					invite.uses.to_string()
				};
				let expires = invite
					.expires_at
					.map_or_else(|| "Never".to_owned(), |at| bridge::message_time(at, today));
				let channel = invite
					.channel_id
					.and_then(|id| state.channel(id)?.name.clone())
					.map(|name| format!("#{name}"))
					.unwrap_or_default();
				let inviter = invite
					.inviter_id
					.map(|id| state.display_name(Some(guild_id), id))
					.unwrap_or_default();
				record([
					("id", invite.code.as_str().into()),
					("inviter", inviter.as_str().into()),
					(
						"avatar",
						avatar
							.map(|name| format!("host:{name}"))
							.unwrap_or_default()
							.as_str()
							.into(),
					),
					("channel", channel.as_str().into()),
					("uses", uses.as_str().into()),
					("expires", expires.as_str().into()),
					("temporary", invite.temporary.into()),
				])
			})
			.collect();
		self.server
			.invites
			.set(record([("rows", Value::List(rows.into()))]));
	}

	fn publish_roles(&self, state: &State, guild_id: Snowflake, editing: Option<Snowflake>) {
		let rows = role_rows(state, guild_id);
		let values = rows
			.iter()
			.map(|row| {
				record([
					("id", row.id.to_string().as_str().into()),
					("name", row.name.as_str().into()),
					("color", row.color.map(hex).unwrap_or_default().as_str().into()),
					("members", Value::Num(row.members as f64)),
					("everyone", row.everyone.into()),
					("movable", row.movable.into()),
				])
			})
			.collect();
		self.server.roles.set(record([
			("count", Value::Num(rows.len() as f64)),
			("rows", Value::List(values)),
		]));
		let role = editing.and_then(|role_id| state.guild(guild_id)?.roles.get(&role_id).cloned());
		let Some(role) = role else {
			self.server.role.set(Value::Nil);
			return;
		};
		let granted: Vec<Value> = right_flags()
			.iter()
			.enumerate()
			.filter(|(_, flag)| role.permissions.contains(**flag))
			.map(|(index, _)| Value::Num((index + 1) as f64))
			.collect();
		let color = role_color(&role);
		let everyone = role.id == guild_id;
		self.server.role.set(record([
			("id", role.id.to_string().as_str().into()),
			("name", role.name.as_str().into()),
			(
				"title",
				if everyone { "@everyone" } else { role.name.as_str() }.into(),
			),
			(
				"color",
				if color == 0 { String::new() } else { hex(color) }
					.as_str()
					.into(),
			),
			("hoist", role.hoist.into()),
			("mentionable", role.mentionable.into()),
			("everyone", everyone.into()),
			("deletable", (!everyone && !role.managed).into()),
			("granted", Value::List(granted.into())),
		]));
	}

	fn server_avatar(&self, state: &State, user_id: Snowflake) -> Option<String> {
		let user = state.user(user_id)?;
		let key = (user_id, user.avatar.clone());
		let mut avatars = self.server.avatars.borrow_mut();
		if let Some(name) = avatars.get(&key) {
			return Some(name.as_str().to_owned());
		}
		let request = bridge::avatar_request(&self.images.cdn, user, ROW_AVATAR, self.scale_factor(), false)?;
		let name = self.server_request(request);
		let text = name.as_str().to_owned();
		avatars.insert(key, name);
		Some(text)
	}

	fn server_icon(&self, _state: &State, guild_id: Snowflake, hash: Option<&str>) -> Option<String> {
		let hash = hash?;
		let request = bridge::guild_icon_request(
			&self.images.cdn,
			guild_id,
			hash,
			ICON_POINTS,
			self.scale_factor(),
			false,
		);
		let name = self.server_request(request);
		let text = name.as_str().to_owned();
		*self.server.icon.borrow_mut() = Some(name);
		Some(text)
	}

	fn server_request(&self, mut request: media::ImageRequest) -> ImageName {
		request.url = bridge::local_media(&request.url, self.images.mock_origin.as_deref());
		self.images.gallery.request(request)
	}

	pub(super) fn server_images(&self) -> Vec<String> {
		if !self.server.is_open() {
			return Vec::new();
		}
		self.server.images.borrow().clone()
	}

	pub(super) fn server_settled(&self) -> bool {
		let shown = self.shown.borrow();
		self.server.ready.get()
			&& self
				.server
				.images
				.borrow()
				.iter()
				.all(|name| shown.contains(name))
	}
}

fn picked_png(path: &std::path::Path, side: u32) -> Option<(Vec<u8>, zgx::image::ImageData)> {
	let size = std::fs::metadata(path).ok()?.len();
	if size > ICON_LIMIT {
		return None;
	}
	let bytes = std::fs::read(path).ok()?;
	let image = image::load_from_memory(&bytes).ok()?;
	let fitted = if image.width() > side || image.height() > side {
		image.thumbnail(side, side)
	} else {
		image
	};
	let rgba = fitted.to_rgba8();
	let mut encoded = std::io::Cursor::new(Vec::new());
	rgba.write_to(&mut encoded, image::ImageFormat::Png).ok()?;
	let preview = zgx::image::ImageData::new(rgba.width(), rgba.height(), rgba.into_raw()).ok()?;
	Some((encoded.into_inner(), preview))
}

fn parse_hex(text: &str) -> Option<u32> {
	let digits = text.trim().trim_start_matches('#');
	(digits.len() == 6)
		.then(|| u32::from_str_radix(digits, 16).ok())
		.flatten()
}
