use chrono::{DateTime, Utc};
use protocol::{Channel, Member, Permissions, Role, Snowflake};
use rest::api::image_data_uri;
use serde_json::{Value, json};
use state::Loaded;

use super::actions::{some, timestamp};
use super::guild_writes::{Call, Fields, Undo, Write, fields};
use super::{Command, Core};

const PNG: &str = "image/png";
const BANS_PAGE: u32 = 1000;

fn png(bytes: &[u8]) -> Value {
	Value::String(image_data_uri(PNG, bytes))
}

impl Core {
	pub(super) fn on_admin_command(&mut self, command: Command) {
		match command {
			Command::EditGuild {
				guild_id,
				name,
				icon_png,
				description,
			} => {
				if self.guild_allows(guild_id, Permissions::MANAGE_GUILD, "Manage Server") {
					let body = fields([
						("name", some(name)),
						("icon", icon_png.as_deref().map(png)),
						("description", some(description)),
					]);
					self.queue_guild_write(guild_id, Write::new("edit server", Call::ModifyGuild(body)));
				}
			}
			Command::CreateChannel {
				guild_id,
				name,
				kind,
				parent_id,
			} => {
				if self.guild_allows(guild_id, Permissions::MANAGE_CHANNELS, "Manage Channels") {
					let body = Value::Object(fields([
						("name", Some(json!(name))),
						("type", Some(json!(i64::from(kind)))),
						("parent_id", parent_id.map(|id| json!(id))),
					]));
					self.queue_guild_write(guild_id, Write::new("create channel", Call::CreateChannel(body)));
				}
			}
			Command::EditChannel { channel_id, changes } => self.edit_channel(channel_id, changes),
			Command::DeleteChannel { channel_id } => {
				if let Some(guild_id) =
					self.channel_manage_allows(channel_id, Permissions::MANAGE_CHANNELS, "Manage Channels")
				{
					self.queue_guild_write(
						guild_id,
						Write::new("delete channel", Call::DeleteChannel(channel_id)),
					);
				}
			}
			Command::CreateRole {
				guild_id,
				name,
				color,
				permissions,
				hoist,
				mentionable,
			} => {
				if self.guild_allows(guild_id, Permissions::MANAGE_ROLES, "Manage Roles")
					&& self.grants_only_held(guild_id, permissions)
				{
					let body = json!({
						"name": name,
						"color": color,
						"permissions": permissions,
						"hoist": hoist,
						"mentionable": mentionable,
					});
					self.queue_guild_write(guild_id, Write::new("create role", Call::CreateRole(body)));
				}
			}
			Command::EditRole {
				guild_id,
				role_id,
				name,
				color,
				permissions,
				hoist,
				mentionable,
			} => {
				if !self.may_manage_role(guild_id, role_id)
					|| permissions.is_some_and(|permissions| !self.grants_only_held(guild_id, permissions))
				{
					return;
				}
				let body = fields([
					("name", some(name)),
					("color", some(color)),
					("permissions", permissions.map(|permissions| json!(permissions))),
					("hoist", some(hoist)),
					("mentionable", some(mentionable)),
				]);
				let undo = self.edit_role_locally(guild_id, role_id, &body);
				let write = Write::new("edit role", Call::ModifyRole(role_id, body)).undo(undo);
				self.queue_guild_write(guild_id, write);
			}
			Command::DeleteRole { guild_id, role_id } => {
				if self.may_manage_role(guild_id, role_id) {
					self.queue_guild_write(guild_id, Write::new("delete role", Call::DeleteRole(role_id)));
				}
			}
			Command::ReorderRoles { guild_id, positions } => {
				let allowed = self.guild_allows(guild_id, Permissions::MANAGE_ROLES, "Manage Roles")
					&& positions
						.iter()
						.all(|(role_id, _)| self.role_is_below_me(guild_id, *role_id));
				if allowed {
					self.queue_guild_write(
						guild_id,
						Write::new("reorder roles", Call::ReorderRoles(positions)),
					);
				} else {
					self.reject("You can only move roles below your highest role");
				}
			}
			Command::SetChannelOverwrite {
				channel_id,
				target_id,
				kind,
				allow,
				deny,
			} => self.set_channel_overwrite(channel_id, target_id, kind, allow, deny),
			Command::DeleteChannelOverwrite {
				channel_id,
				target_id,
			} => self.delete_channel_overwrite(channel_id, target_id),
			Command::DeleteEmoji { guild_id, emoji_id } => {
				if self.guild_allows(
					guild_id,
					Permissions::MANAGE_GUILD_EXPRESSIONS,
					"Manage Expressions",
				) {
					self.queue_guild_write(guild_id, Write::new("delete emoji", Call::DeleteEmoji(emoji_id)));
				}
			}
			command => self.on_admin_read(command),
		}
	}

	fn on_admin_read(&mut self, command: Command) {
		match command {
			Command::LoadAuditLog {
				guild_id,
				before,
				action_type,
				user_id,
			} => {
				if self.guild_allows(guild_id, Permissions::VIEW_AUDIT_LOG, "View Audit Log") {
					self.spawn_load("audit log", move |rest| async move {
						let raw = rest.audit_log(guild_id, before, action_type, user_id).await?;
						Ok(Loaded::AuditLog {
							guild_id,
							raw,
							append: before.is_some(),
						})
					});
				}
			}
			Command::LoadBans { guild_id } => self.load_bans(guild_id, None),
			Command::LoadMoreBans { guild_id } => {
				let after = {
					let state = self.state.read();
					state
						.ban_list(guild_id)
						.filter(|list| list.has_more)
						.and_then(|list| list.entries.last().map(|ban| ban.user.id))
				};
				if let Some(after) = after {
					self.load_bans(guild_id, Some(after));
				}
			}
			Command::LoadInvites { guild_id } => {
				if self.guild_allows(guild_id, Permissions::MANAGE_GUILD, "Manage Server") {
					self.load_guild_invites(guild_id);
				}
			}
			Command::RevokeInvite { guild_id, code } => {
				if self.guild_allows(guild_id, Permissions::MANAGE_GUILD, "Manage Server") {
					self.queue_guild_write(guild_id, Write::new("revoke invite", Call::RevokeInvite(code)));
				}
			}
			Command::LoadChannelInvites { channel_id } => self.load_channel_invites(channel_id),
			Command::RevokeChannelInvite { channel_id, code } => self.revoke_channel_invite(channel_id, code),
			Command::LoadVoiceRegions { guild_id } => self.load_voice_regions(guild_id),
			command => self.on_moderation_command(command),
		}
	}

	fn load_bans(&mut self, guild_id: Snowflake, after: Option<Snowflake>) {
		if self.guild_allows(guild_id, Permissions::BAN_MEMBERS, "Ban Members") {
			self.spawn_load("bans", move |rest| async move {
				let raw = rest.guild_bans(guild_id, after, BANS_PAGE).await?;
				Ok(Loaded::Bans {
					guild_id,
					raw,
					append: after.is_some(),
					limit: BANS_PAGE,
				})
			});
		}
	}

	pub(super) fn load_guild_invites(&self, guild_id: Snowflake) {
		self.spawn_load("invites", move |rest| async move {
			let raw = rest.guild_invites(guild_id).await?;
			Ok(Loaded::GuildInvites { guild_id, raw })
		});
	}

	fn on_moderation_command(&mut self, command: Command) {
		match command {
			Command::KickMember {
				guild_id,
				user_id,
				reason,
			} => {
				if self.may_moderate(guild_id, user_id, Permissions::KICK_MEMBERS, "Kick Members") {
					self.queue_guild_write(guild_id, Write::new("kick", Call::Kick { user_id, reason }));
				}
			}
			Command::BanMember {
				guild_id,
				user_id,
				delete_message_seconds,
				reason,
			} => {
				if self.may_moderate_or_absent(guild_id, user_id, Permissions::BAN_MEMBERS, "Ban Members") {
					let call = Call::Ban {
						user_id,
						delete_message_seconds,
						reason,
					};
					self.queue_guild_write(guild_id, Write::new("ban", call));
				}
			}
			Command::UnbanMember { guild_id, user_id } => {
				if self.guild_allows(guild_id, Permissions::BAN_MEMBERS, "Ban Members") {
					self.queue_guild_write(guild_id, Write::new("unban", Call::Unban(user_id)));
				}
			}
			Command::TimeoutMember {
				guild_id,
				user_id,
				until,
				reason,
			} => {
				if self.may_moderate(
					guild_id,
					user_id,
					Permissions::MODERATE_MEMBERS,
					"Timeout Members",
				) {
					let body = fields([("communication_disabled_until", Some(json!(until.map(timestamp))))]);
					self.edit_member(guild_id, user_id, body, reason, "timeout");
				}
			}
			Command::SetNickname {
				guild_id,
				user_id,
				nick,
			} => self.set_nickname(guild_id, user_id, nick),
			Command::AddMemberRole {
				guild_id,
				user_id,
				role_id,
			}
			| Command::RemoveMemberRole {
				guild_id,
				user_id,
				role_id,
			} if !self.may_assign_role(guild_id, user_id, role_id) => {}
			Command::AddMemberRole {
				guild_id,
				user_id,
				role_id,
			} => self.member_role(guild_id, user_id, role_id, true),
			Command::RemoveMemberRole {
				guild_id,
				user_id,
				role_id,
			} => self.member_role(guild_id, user_id, role_id, false),
			Command::MoveMember {
				guild_id,
				user_id,
				channel_id,
			} => {
				if self.may_moderate(guild_id, user_id, Permissions::MOVE_MEMBERS, "Move Members") {
					let body = fields([("channel_id", Some(json!(channel_id)))]);
					self.edit_member(guild_id, user_id, body, None, "move");
				}
			}
			Command::ServerMuteMember {
				guild_id,
				user_id,
				muted,
			} => {
				if self.may_moderate(guild_id, user_id, Permissions::MUTE_MEMBERS, "Mute Members") {
					self.edit_member(
						guild_id,
						user_id,
						fields([("mute", Some(json!(muted)))]),
						None,
						"server mute",
					);
				}
			}
			Command::ServerDeafenMember {
				guild_id,
				user_id,
				deafened,
			} if self.may_moderate(guild_id, user_id, Permissions::DEAFEN_MEMBERS, "Deafen Members") => {
				let body = fields([("deaf", Some(json!(deafened)))]);
				self.edit_member(guild_id, user_id, body, None, "server deafen");
			}
			_ => {}
		}
	}

	fn set_nickname(&mut self, guild_id: Snowflake, user_id: Snowflake, nick: Option<String>) {
		let me = self.state.read().current_user_id();
		if me == Some(user_id) {
			if self.guild_allows(guild_id, Permissions::CHANGE_NICKNAME, "Change Nickname") {
				let body = fields([("nick", Some(json!(nick)))]);
				let undo = self.edit_member_locally(guild_id, user_id, &body);
				let write = Write::new("nickname", Call::ModifyCurrentMember(body)).undo(undo);
				self.queue_guild_write(guild_id, write);
			}
		} else if self.may_moderate(
			guild_id,
			user_id,
			Permissions::MANAGE_NICKNAMES,
			"Manage Nicknames",
		) {
			self.edit_member(
				guild_id,
				user_id,
				fields([("nick", Some(json!(nick)))]),
				None,
				"nickname",
			);
		}
	}

	fn edit_member(
		&mut self,
		guild_id: Snowflake,
		user_id: Snowflake,
		body: Fields,
		reason: Option<String>,
		what: &'static str,
	) {
		let reason = reason.filter(|reason| !reason.trim().is_empty());
		let undo = self.edit_member_locally(guild_id, user_id, &body);
		let call = Call::ModifyMember {
			user_id,
			body,
			reason,
		};
		self.queue_guild_write(guild_id, Write::new(what, call).undo(undo));
	}

	fn member_role(&mut self, guild_id: Snowflake, user_id: Snowflake, role_id: Snowflake, add: bool) {
		let roles = {
			let state = self.state.read();
			state
				.guild(guild_id)
				.and_then(|guild| guild.members.get(&user_id))
				.map(|member| {
					let mut roles = member.roles.clone();
					roles.retain(|role| *role != role_id);
					if add {
						roles.push(role_id);
					}
					roles
				})
		};
		let undo = roles.and_then(|roles| {
			self.edit_member_locally(guild_id, user_id, &fields([("roles", Some(json!(roles)))]))
		});
		let what = if add { "add role" } else { "remove role" };
		let call = Call::MemberRole {
			user_id,
			role_id,
			add,
		};
		self.queue_guild_write(guild_id, Write::new(what, call).undo(undo));
	}

	fn me(&self) -> Option<Snowflake> {
		self.state.read().current_user_id()
	}

	fn may_moderate(
		&mut self,
		guild_id: Snowflake,
		user_id: Snowflake,
		needed: Permissions,
		what: &str,
	) -> bool {
		if !self.guild_allows(guild_id, needed, what) {
			return false;
		}
		let outranks = self
			.me()
			.is_some_and(|me| self.state.read().outranks(guild_id, me, user_id));
		if !outranks {
			self.reject("You can only moderate members whose highest role is below yours");
		}
		outranks
	}

	fn may_moderate_or_absent(
		&mut self,
		guild_id: Snowflake,
		user_id: Snowflake,
		needed: Permissions,
		what: &str,
	) -> bool {
		let absent = {
			let state = self.state.read();
			let owner = state.guild(guild_id).and_then(|guild| guild.owner_id);
			state
				.guild(guild_id)
				.is_some_and(|guild| guild.members.get(&user_id).is_none())
				&& owner != Some(user_id)
				&& state.current_user_id() != Some(user_id)
		};
		if absent {
			self.guild_allows(guild_id, needed, what)
		} else {
			self.may_moderate(guild_id, user_id, needed, what)
		}
	}

	fn role_is_below_me(&self, guild_id: Snowflake, role_id: Snowflake) -> bool {
		let state = self.state.read();
		let managed = state
			.guild(guild_id)
			.and_then(|guild| guild.roles.get(&role_id))
			.is_none_or(|role| role.managed);
		!managed
			&& state
				.current_user_id()
				.is_some_and(|me| state.role_below(guild_id, me, role_id))
	}

	fn may_manage_role(&mut self, guild_id: Snowflake, role_id: Snowflake) -> bool {
		if !self.guild_allows(guild_id, Permissions::MANAGE_ROLES, "Manage Roles") {
			return false;
		}
		let everyone = role_id == guild_id;
		let allowed = everyone || self.role_is_below_me(guild_id, role_id);
		if !allowed {
			self.reject("You can only change roles below your highest role");
		}
		allowed
	}

	fn grants_only_held(&mut self, guild_id: Snowflake, permissions: Permissions) -> bool {
		let held = self.me().map_or(Permissions::empty(), |me| {
			self.state.read().guild_permissions(guild_id, me)
		});
		let allowed = held.contains(Permissions::ADMINISTRATOR) || held.contains(permissions);
		if !allowed {
			self.reject("A role can only be given permissions you have yourself");
		}
		allowed
	}

	fn may_assign_role(&mut self, guild_id: Snowflake, user_id: Snowflake, role_id: Snowflake) -> bool {
		if !self.guild_allows(guild_id, Permissions::MANAGE_ROLES, "Manage Roles") {
			return false;
		}
		if !self.role_is_below_me(guild_id, role_id) {
			self.reject("You can only give roles below your highest role");
			return false;
		}
		let own = self.me() == Some(user_id);
		let reachable = own
			|| self
				.me()
				.is_some_and(|me| self.state.read().outranks(guild_id, me, user_id));
		if !reachable {
			self.reject("You can only moderate members whose highest role is below yours");
		}
		reachable
	}

	fn edit_role_locally(&mut self, guild_id: Snowflake, role_id: Snowflake, body: &Fields) -> Option<Undo> {
		let before: Role = self.state.read().guild(guild_id)?.roles.get(&role_id)?.clone();
		let mut role = before.clone();
		if let Some(name) = body.get("name").and_then(Value::as_str) {
			role.name = name.to_owned();
		}
		if let Some(color) = body.get("color").and_then(Value::as_i64) {
			role.color = color;
			role.colors = None;
		}
		if let Some(permissions) = body
			.get("permissions")
			.and_then(|value| serde_json::from_value::<Permissions>(value.clone()).ok())
		{
			role.permissions = permissions;
		}
		if let Some(hoist) = body.get("hoist").and_then(Value::as_bool) {
			role.hoist = hoist;
		}
		if let Some(mentionable) = body.get("mentionable").and_then(Value::as_bool) {
			role.mentionable = mentionable;
		}
		self.apply_dispatch(&protocol::DispatchEvent::GuildRoleUpdate(Box::new(
			protocol::GuildRoleChange { guild_id, role },
		)));
		Some(Undo::Role(Box::new(before)))
	}

	fn member_snapshot(&self, guild_id: Snowflake, user_id: Snowflake) -> Option<Member> {
		let state = self.state.read();
		let mut member = state.guild(guild_id)?.members.get(&user_id)?.clone();
		member.user = Some(state.user(user_id)?.as_ref().clone());
		member.guild_id = Some(guild_id);
		Some(member)
	}

	fn edit_member_locally(
		&mut self,
		guild_id: Snowflake,
		user_id: Snowflake,
		body: &Fields,
	) -> Option<Undo> {
		let before = self.member_snapshot(guild_id, user_id)?;
		let mut member = before.clone();
		if let Some(nick) = body.get("nick") {
			member.nick = nick.as_str().filter(|nick| !nick.is_empty()).map(str::to_owned);
		}
		if let Some(until) = body.get("communication_disabled_until") {
			member.communication_disabled_until = until
				.as_str()
				.and_then(|until| DateTime::parse_from_rfc3339(until).ok())
				.map(|until| until.with_timezone(&Utc));
		}
		if let Some(roles) = body
			.get("roles")
			.and_then(|roles| serde_json::from_value::<Vec<Snowflake>>(roles.clone()).ok())
		{
			member.roles = roles;
		}
		if let Some(mute) = body.get("mute").and_then(Value::as_bool) {
			member.mute = mute;
		}
		if let Some(deaf) = body.get("deaf").and_then(Value::as_bool) {
			member.deaf = deaf;
		}
		self.apply_dispatch(&protocol::DispatchEvent::GuildMemberUpdate(Box::new(member)));
		Some(Undo::Member(Box::new(before)))
	}

	pub(super) fn edit_channel_locally(&mut self, channel_id: Snowflake, body: &Fields) -> Option<Undo> {
		let before: Channel = self.state.read().channel(channel_id)?.clone();
		let mut channel = before.clone();
		if let Some(name) = body.get("name").and_then(Value::as_str) {
			channel.name = Some(name.to_owned());
		}
		if let Some(topic) = body.get("topic") {
			channel.topic = topic
				.as_str()
				.filter(|topic| !topic.is_empty())
				.map(str::to_owned);
		}
		if let Some(nsfw) = body.get("nsfw").and_then(Value::as_bool) {
			channel.nsfw = Some(nsfw);
		}
		if let Some(slowmode) = body.get("rate_limit_per_user").and_then(Value::as_i64) {
			channel.rate_limit_per_user = Some(slowmode);
		}
		if let Some(position) = body.get("position").and_then(Value::as_i64) {
			channel.position = Some(position);
		}
		if let Some(bitrate) = body.get("bitrate").and_then(Value::as_i64) {
			channel.bitrate = Some(bitrate);
		}
		if let Some(user_limit) = body.get("user_limit").and_then(Value::as_i64) {
			channel.user_limit = Some(user_limit);
		}
		if let Some(region) = body.get("rtc_region") {
			channel.rtc_region = region.as_str().map(str::to_owned);
		}
		if let Some(quality) = body.get("video_quality_mode").and_then(Value::as_i64) {
			channel.video_quality_mode = Some(quality);
		}
		if let Some(minutes) = body.get("default_auto_archive_duration").and_then(Value::as_i64) {
			channel.default_auto_archive_duration = Some(minutes);
		}
		if let Some(flags) = body.get("flags").and_then(Value::as_u64) {
			channel.flags = protocol::ChannelFlags::from_bits_retain(flags);
		}
		if let Some(kind) = body.get("type").and_then(Value::as_i64) {
			channel.kind = protocol::ChannelType::from(kind);
		}
		if let Some(parent) = body
			.get("parent_id")
			.and_then(|parent| serde_json::from_value::<Snowflake>(parent.clone()).ok())
		{
			channel.parent_id = Some(parent);
		}
		self.apply_dispatch(&protocol::DispatchEvent::ChannelUpdate(Box::new(channel)));
		Some(Undo::Channel(Box::new(before)))
	}
}
