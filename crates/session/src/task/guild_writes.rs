use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::Duration;

use protocol::{Channel, Member, Role, Snowflake};
use rest::{RestClient, RestTuning, UploadFile};
use serde_json::{Map, Value};
use state::LocalEvent;
use tokio::time::Instant;

use super::actions::failure_reason;
use super::{Core, Internal};

pub(super) type Fields = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
	Guild,
	Channel(Snowflake),
	Role(Snowflake),
	RoleOrder,
	Member(Snowflake, Option<String>),
	MemberRole(Snowflake, Snowflake),
	Sticker(Snowflake),
	Emoji(Snowflake),
}

#[derive(Debug)]
pub(super) enum Call {
	ModifyGuild(Fields),
	CreateChannel(Value),
	ModifyChannel(Snowflake, Fields),
	DeleteChannel(Snowflake),
	CreateRole(Value),
	ModifyRole(Snowflake, Fields),
	DeleteRole(Snowflake),
	ReorderRoles(Vec<(Snowflake, i64)>),
	SetOverwrite {
		channel_id: Snowflake,
		target_id: Snowflake,
		body: Value,
	},
	DeleteOverwrite {
		channel_id: Snowflake,
		target_id: Snowflake,
	},
	Kick {
		user_id: Snowflake,
		reason: Option<String>,
	},
	Ban {
		user_id: Snowflake,
		delete_message_seconds: u32,
		reason: Option<String>,
	},
	Unban(Snowflake),
	ModifyMember {
		user_id: Snowflake,
		body: Fields,
		reason: Option<String>,
	},
	ModifyCurrentMember(Fields),
	MemberRole {
		user_id: Snowflake,
		role_id: Snowflake,
		add: bool,
	},
	CreateEmoji {
		name: String,
		image: String,
	},
	EditEmoji {
		id: Snowflake,
		name: String,
		roles: Vec<Snowflake>,
	},
	DeleteEmoji(Snowflake),
	CreateSticker {
		name: String,
		description: String,
		tags: String,
		file: UploadFile,
	},
	EditSticker {
		id: Snowflake,
		name: Option<String>,
		description: Option<String>,
		tags: Option<String>,
	},
	DeleteSticker(Snowflake),
	RevokeInvite(String),
	RemoveReaction {
		channel_id: Snowflake,
		message_id: Snowflake,
		emoji: String,
		burst: bool,
		user_id: Snowflake,
	},
}

impl Call {
	fn target(&self) -> Option<Target> {
		match self {
			Self::ModifyGuild(_) => Some(Target::Guild),
			Self::ModifyChannel(id, _) => Some(Target::Channel(*id)),
			Self::ModifyRole(id, _) => Some(Target::Role(*id)),
			Self::ReorderRoles(_) => Some(Target::RoleOrder),
			Self::ModifyMember { user_id, reason, .. } => Some(Target::Member(*user_id, reason.clone())),
			Self::MemberRole { user_id, role_id, .. } => Some(Target::MemberRole(*user_id, *role_id)),
			Self::EditSticker { id, .. } => Some(Target::Sticker(*id)),
			Self::EditEmoji { id, .. } => Some(Target::Emoji(*id)),
			_ => None,
		}
	}

	fn absorb(&mut self, newer: Call) -> Option<Call> {
		match (self, newer) {
			(Self::ModifyGuild(fields), Self::ModifyGuild(more))
			| (Self::ModifyChannel(_, fields), Self::ModifyChannel(_, more))
			| (Self::ModifyRole(_, fields), Self::ModifyRole(_, more))
			| (Self::ModifyMember { body: fields, .. }, Self::ModifyMember { body: more, .. }) => {
				fields.extend(more);
				None
			}
			(Self::ReorderRoles(positions), Self::ReorderRoles(more)) => {
				for (id, position) in more {
					positions.retain(|(known, _)| *known != id);
					positions.push((id, position));
				}
				None
			}
			(
				Self::EditSticker {
					name,
					description,
					tags,
					..
				},
				Self::EditSticker {
					name: newer_name,
					description: newer_description,
					tags: newer_tags,
					..
				},
			) => {
				*name = newer_name.or(name.take());
				*description = newer_description.or(description.take());
				*tags = newer_tags.or(tags.take());
				None
			}
			(current @ (Self::MemberRole { .. } | Self::EditEmoji { .. }), newer) => {
				*current = newer;
				None
			}
			(_, newer) => Some(newer),
		}
	}

	async fn run(self, rest: RestClient, guild_id: Snowflake) -> rest::Result<()> {
		match self {
			Self::ModifyGuild(fields) => rest.modify_guild(guild_id, Value::Object(fields)).await.map(drop),
			Self::CreateChannel(body) => rest.create_guild_channel(guild_id, body).await.map(drop),
			Self::ModifyChannel(channel_id, fields) => rest
				.modify_channel(channel_id, Value::Object(fields))
				.await
				.map(drop),
			Self::DeleteChannel(channel_id) => rest.delete_channel(channel_id).await.map(drop),
			Self::CreateRole(body) => rest.create_role(guild_id, body).await.map(drop),
			Self::ModifyRole(role_id, fields) => rest
				.modify_role(guild_id, role_id, Value::Object(fields))
				.await
				.map(drop),
			Self::DeleteRole(role_id) => rest.delete_role(guild_id, role_id).await,
			Self::ReorderRoles(positions) => rest.reorder_roles(guild_id, &positions).await.map(drop),
			Self::SetOverwrite {
				channel_id,
				target_id,
				body,
			} => rest.set_channel_overwrite(channel_id, target_id, body).await,
			Self::DeleteOverwrite {
				channel_id,
				target_id,
			} => rest.delete_channel_overwrite(channel_id, target_id).await,
			Self::Kick { user_id, reason } => rest.kick_member(guild_id, user_id, reason.as_deref()).await,
			Self::Ban {
				user_id,
				delete_message_seconds,
				reason,
			} => {
				rest.ban_member(guild_id, user_id, delete_message_seconds, reason.as_deref())
					.await
			}
			Self::Unban(user_id) => rest.unban_member(guild_id, user_id).await,
			Self::ModifyMember {
				user_id,
				body,
				reason,
			} => rest
				.modify_member(guild_id, user_id, Value::Object(body), reason.as_deref())
				.await
				.map(drop),
			Self::ModifyCurrentMember(fields) => rest
				.modify_current_member(guild_id, Value::Object(fields))
				.await
				.map(drop),
			Self::MemberRole {
				user_id,
				role_id,
				add: true,
			} => rest.add_member_role(guild_id, user_id, role_id).await,
			Self::MemberRole {
				user_id,
				role_id,
				add: false,
			} => rest.remove_member_role(guild_id, user_id, role_id).await,
			Self::CreateEmoji { name, image } => rest.create_emoji(guild_id, &name, &image).await.map(drop),
			Self::EditEmoji { id, name, roles } => rest
				.edit_guild_emoji(guild_id, id, Some(&name), Some(&roles))
				.await
				.map(drop),
			Self::DeleteEmoji(id) => rest.delete_emoji(guild_id, id).await,
			Self::CreateSticker {
				name,
				description,
				tags,
				file,
			} => rest
				.create_guild_sticker(guild_id, &name, &description, &tags, file)
				.await
				.map(drop),
			Self::EditSticker {
				id,
				name,
				description,
				tags,
			} => rest
				.edit_guild_sticker(
					guild_id,
					id,
					name.as_deref(),
					description.as_deref(),
					tags.as_deref(),
				)
				.await
				.map(drop),
			Self::DeleteSticker(id) => rest.delete_guild_sticker(guild_id, id).await,
			Self::RevokeInvite(code) => rest.delete_invite(&code, None).await,
			Self::RemoveReaction {
				channel_id,
				message_id,
				emoji,
				burst,
				user_id,
			} => {
				let reaction = rest::api::ReactionRef {
					channel_id,
					message_id,
					emoji: &emoji,
					burst,
				};
				rest.remove_user_reaction(reaction, user_id).await
			}
		}
	}
}

#[derive(Debug, Clone)]
pub(super) enum Undo {
	Role(Box<Role>),
	Member(Box<Member>),
	Channel(Box<Channel>),
}

#[derive(Debug)]
pub(super) struct Write {
	what: &'static str,
	call: Call,
	undo: Option<Undo>,
}

impl Write {
	pub(super) fn new(what: &'static str, call: Call) -> Self {
		Self {
			what,
			call,
			undo: None,
		}
	}

	pub(super) fn undo(mut self, undo: Option<Undo>) -> Self {
		self.undo = undo;
		self
	}
}

struct InFlight {
	what: &'static str,
	target: Option<Target>,
	undo: Option<Undo>,
	follow_up: FollowUp,
}

#[derive(Debug, Clone, Copy)]
enum FollowUp {
	None,
	Unbanned(Snowflake),
	InvitesChanged,
}

pub(super) struct GuildWrites {
	gap: Duration,
	queues: BTreeMap<Snowflake, VecDeque<Write>>,
	in_flight: HashMap<Snowflake, InFlight>,
	next_allowed: HashMap<Snowflake, Instant>,
}

pub(super) enum Settled {
	Saved,
	Failed { reason: String, undo: Option<Undo> },
}

impl GuildWrites {
	pub(super) fn new(tuning: &RestTuning) -> Self {
		Self {
			gap: tuning.guild_write_gap,
			queues: BTreeMap::new(),
			in_flight: HashMap::new(),
			next_allowed: HashMap::new(),
		}
	}

	fn earlier_undo(&self, guild_id: Snowflake, target: Option<&Target>) -> Option<Option<Undo>> {
		let target = target?;
		let queued = self
			.queues
			.get(&guild_id)
			.into_iter()
			.flatten()
			.find(|write| write.call.target().as_ref() == Some(target))
			.map(|write| write.undo.clone());
		queued.or_else(|| {
			self.in_flight
				.get(&guild_id)
				.filter(|flight| flight.target.as_ref() == Some(target))
				.map(|flight| flight.undo.clone())
		})
	}

	pub(super) fn want(&mut self, guild_id: Snowflake, mut write: Write) {
		let target = write.call.target();
		if let Some(undo) = self.earlier_undo(guild_id, target.as_ref()) {
			write.undo = undo;
		}
		self.next_allowed.entry(guild_id).or_insert_with(Instant::now);
		let queue = self.queues.entry(guild_id).or_default();
		let mut call = Some(write.call);
		if let Some(target) = &target
			&& let Some(queued) = queue
				.iter_mut()
				.find(|queued| queued.call.target().as_ref() == Some(target))
			&& let Some(newer) = call.take()
		{
			call = queued.call.absorb(newer);
		}
		if let Some(call) = call {
			queue.push_back(Write {
				what: write.what,
				call,
				undo: write.undo,
			});
		}
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		self.queues
			.iter()
			.filter(|(guild_id, queue)| !queue.is_empty() && !self.in_flight.contains_key(guild_id))
			.filter_map(|(guild_id, _)| self.next_allowed.get(guild_id).copied())
			.min()
	}

	fn take_ready(&mut self, now: Instant) -> Vec<(Snowflake, Call)> {
		let mut ready = Vec::new();
		for (guild_id, queue) in &mut self.queues {
			if self.in_flight.contains_key(guild_id)
				|| self
					.next_allowed
					.get(guild_id)
					.is_some_and(|allowed| *allowed > now)
			{
				continue;
			}
			let Some(write) = queue.pop_front() else {
				continue;
			};
			let follow_up = match &write.call {
				Call::Unban(user_id) => FollowUp::Unbanned(*user_id),
				Call::RevokeInvite(_) => FollowUp::InvitesChanged,
				_ => FollowUp::None,
			};
			self.in_flight.insert(
				*guild_id,
				InFlight {
					what: write.what,
					target: write.call.target(),
					undo: write.undo,
					follow_up,
				},
			);
			ready.push((*guild_id, write.call));
		}
		self.queues.retain(|_, queue| !queue.is_empty());
		ready
	}

	fn finished(&mut self, guild_id: Snowflake, outcome: rest::Result<()>) -> Option<(Settled, FollowUp)> {
		let flight = self.in_flight.remove(&guild_id)?;
		self.next_allowed.insert(guild_id, Instant::now() + self.gap);
		let settled = match outcome {
			Ok(()) => Settled::Saved,
			Err(error) => {
				tracing::warn!(%error, what = flight.what, "a server write failed");
				let newer = self.earlier_undo(guild_id, flight.target.as_ref()).is_some();
				Settled::Failed {
					reason: failure_reason(capitalized(flight.what).as_str(), &error),
					undo: if newer { None } else { flight.undo },
				}
			}
		};
		Some((settled, flight.follow_up))
	}
}

fn capitalized(what: &str) -> String {
	let mut chars = what.chars();
	chars
		.next()
		.map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

impl Core {
	pub(super) fn queue_guild_write(&mut self, guild_id: Snowflake, write: Write) {
		self.guild_writes.want(guild_id, write);
	}

	pub(super) fn flush_guild_writes(&mut self) {
		for (guild_id, call) in self.guild_writes.take_ready(Instant::now()) {
			let rest = self.rest.clone();
			let internal = self.internal.clone();
			self.runtime.spawn(async move {
				let outcome = call.run(rest, guild_id).await;
				let _ = internal.send(Internal::GuildWritten { guild_id, outcome });
			});
		}
	}

	pub(super) fn on_guild_written(&mut self, guild_id: Snowflake, outcome: rest::Result<()>) {
		let Some((settled, follow_up)) = self.guild_writes.finished(guild_id, outcome) else {
			return;
		};
		match settled {
			Settled::Saved => match follow_up {
				FollowUp::Unbanned(user_id) => self.apply_local(LocalEvent::BanRevoked { guild_id, user_id }),
				FollowUp::InvitesChanged => self.load_guild_invites(guild_id),
				FollowUp::None => {}
			},
			Settled::Failed { reason, undo } => {
				if let Some(undo) = undo {
					self.restore(guild_id, undo);
				}
				self.reject(reason);
			}
		}
	}

	fn restore(&mut self, guild_id: Snowflake, undo: Undo) {
		let event = match undo {
			Undo::Role(role) => {
				protocol::DispatchEvent::GuildRoleUpdate(Box::new(protocol::GuildRoleChange {
					guild_id,
					role: *role,
				}))
			}
			Undo::Member(member) => protocol::DispatchEvent::GuildMemberUpdate(member),
			Undo::Channel(channel) => protocol::DispatchEvent::ChannelUpdate(channel),
		};
		self.apply_dispatch(&event);
	}
}

pub(super) fn fields<const N: usize>(entries: [(&str, Option<Value>); N]) -> Fields {
	entries
		.into_iter()
		.filter_map(|(key, value)| Some((key.to_owned(), value?)))
		.collect()
}
