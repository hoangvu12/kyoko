use protocol::{Channel, ChannelFlags, ChannelType, PermissionOverwriteKind, Permissions, Snowflake};
use serde_json::{Value, json};
use state::{
	ARCHIVE_DURATIONS, BITRATE_MIN, CHANNEL_NAME_MAX, CHANNEL_TOPIC_MAX, Loaded, SLOWMODE_MAX,
	VIDEO_QUALITY_AUTO, VIDEO_QUALITY_FULL, bitrate_limit, user_limit_range,
};

use super::Core;
use super::guild_writes::{Call, Fields, Write};
use crate::command::ChannelChanges;

const NEWS: &str = "NEWS";
const TYPE_TEXT: i64 = 0;
const TYPE_ANNOUNCEMENT: i64 = 5;

struct Limits {
	news: bool,
	bitrate: u32,
}

fn is_audio(kind: ChannelType) -> bool {
	matches!(kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice)
}

fn has_topic(kind: ChannelType) -> bool {
	matches!(
		kind,
		ChannelType::GuildText
			| ChannelType::GuildAnnouncement
			| ChannelType::GuildForum
			| ChannelType::GuildMedia
			| ChannelType::GuildStageVoice
	)
}

fn archives(kind: ChannelType) -> bool {
	matches!(
		kind,
		ChannelType::GuildText
			| ChannelType::GuildAnnouncement
			| ChannelType::GuildForum
			| ChannelType::GuildMedia
	)
}

fn convertible(kind: ChannelType) -> bool {
	matches!(kind, ChannelType::GuildText | ChannelType::GuildAnnouncement)
}

fn clipped(text: &str, limit: usize) -> String {
	text.trim().chars().take(limit).collect()
}

fn channel_body(
	channel: &Channel,
	limits: &Limits,
	changes: ChannelChanges,
) -> (Fields, Option<&'static str>) {
	let kind = channel.kind;
	let mut body = Fields::new();
	let mut refusal = None;
	let mut put = |key: &str, value: Value| {
		body.insert(key.to_owned(), value);
	};
	if let Some(name) = changes.name.map(|name| clipped(&name, CHANNEL_NAME_MAX))
		&& !name.is_empty()
	{
		put("name", json!(name));
	}
	if let Some(topic) = changes.topic.filter(|_| has_topic(kind)) {
		put("topic", json!(clipped(&topic, CHANNEL_TOPIC_MAX)));
	}
	if let Some(nsfw) = changes.nsfw {
		put("nsfw", json!(nsfw));
	}
	if let Some(slowmode) = changes.slowmode {
		put("rate_limit_per_user", json!(slowmode.min(SLOWMODE_MAX)));
	}
	if let Some(position) = changes.position {
		put("position", json!(position));
	}
	if let Some(parent_id) = changes.parent_id {
		put("parent_id", json!(parent_id));
	}
	if let Some(announcement) = changes.announcement.filter(|_| convertible(kind)) {
		if limits.news {
			put(
				"type",
				json!(if announcement {
					TYPE_ANNOUNCEMENT
				} else {
					TYPE_TEXT
				}),
			);
		} else {
			refusal = Some("Announcement channels need a Community server");
		}
	}
	if let Some(spoiler) = changes.spoiler {
		let mut flags = channel.flags;
		flags.set(ChannelFlags::IS_SPOILER_CHANNEL, spoiler);
		put("flags", json!(flags.bits()));
	}
	if is_audio(kind) {
		if let Some(bitrate) = changes.bitrate {
			put("bitrate", json!(bitrate.clamp(BITRATE_MIN, limits.bitrate)));
		}
		if let Some(user_limit) = changes.user_limit {
			let (low, high) = user_limit_range(kind);
			put("user_limit", json!(user_limit.clamp(low, high)));
		}
		if let Some(region) = changes.rtc_region {
			put("rtc_region", json!(region.filter(|region| !region.is_empty())));
		}
		if let Some(quality) = changes
			.video_quality
			.filter(|quality| [VIDEO_QUALITY_AUTO, VIDEO_QUALITY_FULL].contains(quality))
		{
			put("video_quality_mode", json!(quality));
		}
	}
	if let Some(minutes) = changes
		.auto_archive
		.filter(|minutes| archives(kind) && ARCHIVE_DURATIONS.contains(minutes))
	{
		put("default_auto_archive_duration", json!(minutes));
	}
	(body, refusal)
}

impl Core {
	pub(super) fn channel_manage_allows(
		&mut self,
		channel_id: Snowflake,
		needed: Permissions,
		what: &str,
	) -> Option<Snowflake> {
		let (guild_id, held) = {
			let state = self.state.read();
			(state.channel_guild(channel_id), state.my_permissions(channel_id))
		};
		let Some(guild_id) = guild_id else {
			self.reject("This only works in a server");
			return None;
		};
		if held.contains(Permissions::ADMINISTRATOR) || held.contains(needed) {
			Some(guild_id)
		} else {
			self.reject(format!("You need the {what} permission"));
			None
		}
	}

	pub(super) fn edit_channel(&mut self, channel_id: Snowflake, changes: ChannelChanges) {
		let Some(guild_id) =
			self.channel_manage_allows(channel_id, Permissions::MANAGE_CHANNELS, "Manage Channels")
		else {
			return;
		};
		let found = {
			let state = self.state.read();
			let channel = state.channel(channel_id).cloned();
			let limits = state
				.guild(guild_id)
				.zip(channel.as_ref())
				.map(|(guild, channel)| Limits {
					news: guild.features.iter().any(|feature| feature == NEWS),
					bitrate: bitrate_limit(guild.premium_tier, &guild.features, channel.kind),
				});
			channel.zip(limits)
		};
		let Some((channel, limits)) = found else {
			return;
		};
		let (body, refusal) = channel_body(&channel, &limits, changes);
		if let Some(refusal) = refusal {
			self.reject(refusal);
		}
		if body.is_empty() {
			return;
		}
		let undo = self.edit_channel_locally(channel_id, &body);
		let write = Write::new("edit channel", Call::ModifyChannel(channel_id, body)).undo(undo);
		self.queue_guild_write(guild_id, write);
	}

	pub(super) fn set_channel_overwrite(
		&mut self,
		channel_id: Snowflake,
		target_id: Snowflake,
		kind: PermissionOverwriteKind,
		allow: Permissions,
		deny: Permissions,
	) {
		let Some(guild_id) =
			self.channel_manage_allows(channel_id, Permissions::MANAGE_ROLES, "Manage Permissions")
		else {
			return;
		};
		let held = self.state.read().my_permissions(channel_id);
		if !held.contains(Permissions::ADMINISTRATOR) && !held.contains(allow | deny) {
			self.reject("You can only allow or deny permissions you have yourself");
			return;
		}
		let body = json!({ "type": i64::from(kind), "allow": allow, "deny": deny });
		let call = Call::SetOverwrite {
			channel_id,
			target_id,
			body,
		};
		self.queue_guild_write(guild_id, Write::new("channel permissions", call));
	}

	pub(super) fn delete_channel_overwrite(&mut self, channel_id: Snowflake, target_id: Snowflake) {
		if let Some(guild_id) =
			self.channel_manage_allows(channel_id, Permissions::MANAGE_ROLES, "Manage Permissions")
		{
			let call = Call::DeleteOverwrite {
				channel_id,
				target_id,
			};
			self.queue_guild_write(guild_id, Write::new("channel permissions", call));
		}
	}

	pub(super) fn load_channel_invites(&mut self, channel_id: Snowflake) {
		if self
			.channel_manage_allows(channel_id, Permissions::MANAGE_CHANNELS, "Manage Channels")
			.is_some()
		{
			self.spawn_load("invites", move |rest| async move {
				let raw = rest.channel_invites(channel_id).await?;
				Ok(Loaded::ChannelInvites { channel_id, raw })
			});
		}
	}

	pub(super) fn revoke_channel_invite(&mut self, channel_id: Snowflake, code: String) {
		if let Some(guild_id) =
			self.channel_manage_allows(channel_id, Permissions::MANAGE_CHANNELS, "Manage Channels")
		{
			let call = Call::RevokeChannelInvite { channel_id, code };
			self.queue_guild_write(guild_id, Write::new("revoke invite", call));
		}
	}

	pub(super) fn load_voice_regions(&mut self, guild_id: Snowflake) {
		self.spawn_load("voice regions", move |rest| async move {
			let raw = rest.guild_voice_regions(guild_id).await?;
			Ok(Loaded::VoiceRegions { guild_id, raw })
		});
	}
}
