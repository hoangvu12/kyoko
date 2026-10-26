use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;

use crate::channel::{Channel, ChannelType, ThreadMember};
use crate::emoji::{Emoji, PartialEmoji, Sticker};
use crate::error::DecodeError;
use crate::fingerprint::IdentifyProperties;
use crate::guild::{Guild, UnavailableGuild};
use crate::lenient::{LenientVec, int_enum, nullable_string, timestamp};
use crate::member::Member;
use crate::message::{Message, MessageUpdate};
use crate::presence::Presence;
use crate::role::Role;
use crate::settings::{
	ReadStateContainer, Relationship, RelationshipKind, UserGuildSettings, UserGuildSettingsContainer,
};
use crate::snowflake::Snowflake;
use crate::token::Token;
use crate::user::User;
use crate::voice::VoiceState;

int_enum! {
	pub enum Opcode {
		Dispatch = 0,
		Heartbeat = 1,
		Identify = 2,
		PresenceUpdate = 3,
		VoiceStateUpdate = 4,
		Resume = 6,
		Reconnect = 7,
		RequestGuildMembers = 8,
		InvalidSession = 9,
		Hello = 10,
		HeartbeatAck = 11,
		StreamCreate = 18,
		StreamDelete = 19,
		StreamWatch = 20,
		StreamPing = 21,
		StreamSetPaused = 22,
		GuildSubscriptions = 14,
		GuildSubscriptionsBulk = 37,
		QosHeartbeat = 40,
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayPayload {
	pub op: i64,
	#[serde(default)]
	pub d: Option<Box<RawValue>>,
	#[serde(default)]
	pub s: Option<u64>,
	#[serde(default)]
	pub t: Option<String>,
}

impl GatewayPayload {
	pub fn new<T: Serialize>(opcode: Opcode, data: &T) -> Result<Self, serde_json::Error> {
		Ok(Self {
			op: opcode.into(),
			d: Some(serde_json::value::to_raw_value(data)?),
			s: None,
			t: None,
		})
	}

	pub fn opcode(&self) -> Opcode {
		Opcode::from(self.op)
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
	pub heartbeat_interval: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Identify {
	pub token: Token,
	pub properties: IdentifyProperties,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub compress: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub presence: Option<Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub capabilities: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub client_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientState {
	pub guild_versions: serde_json::Map<String, Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub read_state_version: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub user_guild_settings_version: Option<u64>,
	pub api_code_version: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Resume {
	pub token: Token,
	pub session_id: String,
	pub seq: u64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ready {
	pub v: Option<i64>,
	pub user: User,
	pub guilds: LenientVec<Guild>,
	pub private_channels: LenientVec<Channel>,
	pub relationships: LenientVec<Relationship>,
	pub read_state: ReadStateContainer,
	pub user_guild_settings: UserGuildSettingsContainer,
	pub session_id: String,
	pub resume_gateway_url: Option<String>,
	pub sessions: Vec<Value>,
	pub users: LenientVec<User>,
	pub merged_members: Vec<LenientVec<Member>>,
	pub user_settings_proto: Option<String>,
	pub user_settings: Option<Value>,
	pub apex_experiments: Value,
	#[serde(deserialize_with = "crate::lenient::optional_int")]
	pub api_code_version: Option<i64>,
}

#[derive(Deserialize)]
struct ReadyAuth {
	#[serde(default)]
	auth_token: Option<Token>,
}

pub fn refreshed_token(ready: &RawValue) -> Option<Token> {
	serde_json::from_str::<ReadyAuth>(ready.get())
		.ok()?
		.auth_token
		.filter(|token| !token.as_str().trim().is_empty())
}

impl Ready {
	pub fn installation_id(&self) -> Option<&str> {
		self.apex_experiments
			.get("installation")
			.and_then(Value::as_str)
			.filter(|id| crate::fingerprint::plausible_installation_id(id))
	}

	pub fn normalize_guilds(&mut self) {
		for guild in self.guilds.iter_mut() {
			*guild = std::mem::take(guild).normalized();
		}
	}
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MergedPresences {
	pub guilds: Vec<LenientVec<Presence>>,
	pub friends: LenientVec<Presence>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SupplementalGuild {
	pub id: Snowflake,
	pub voice_states: LenientVec<VoiceState>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadySupplemental {
	pub merged_presences: MergedPresences,
	pub merged_members: Vec<LenientVec<Member>>,
	pub guilds: LenientVec<SupplementalGuild>,
	pub lazy_private_channels: LenientVec<Channel>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageDelete {
	pub id: Snowflake,
	pub channel_id: Snowflake,
	pub guild_id: Option<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageDeleteBulk {
	pub ids: Vec<Snowflake>,
	pub channel_id: Snowflake,
	pub guild_id: Option<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageReactionAdd {
	pub user_id: Snowflake,
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub member: Option<Member>,
	pub emoji: PartialEmoji,
	pub message_author_id: Option<Snowflake>,
	pub burst: Option<bool>,
	#[serde(rename = "type")]
	pub kind: Option<i64>,
	pub burst_colors: LenientVec<String>,
}

impl MessageReactionAdd {
	pub fn is_burst(&self) -> bool {
		burst_reaction(self.burst, self.kind)
	}
}

impl MessageReactionRemove {
	pub fn is_burst(&self) -> bool {
		burst_reaction(self.burst, self.kind)
	}
}

fn burst_reaction(burst: Option<bool>, kind: Option<i64>) -> bool {
	kind.map_or(burst == Some(true), |kind| kind == 1)
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageReactionRemove {
	pub user_id: Snowflake,
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub emoji: PartialEmoji,
	pub burst: Option<bool>,
	#[serde(rename = "type")]
	pub kind: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageReactionRemoveAll {
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub guild_id: Option<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageReactionRemoveEmoji {
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub emoji: PartialEmoji,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageAck {
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
	pub version: Option<i64>,
	pub mention_count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelUnread {
	pub id: Snowflake,
	pub last_message_id: Option<Snowflake>,
	#[serde(deserialize_with = "timestamp")]
	pub last_pin_timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelUnreadUpdate {
	pub guild_id: Snowflake,
	pub channel_unread_updates: LenientVec<ChannelUnread>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelPinsUpdate {
	pub guild_id: Option<Snowflake>,
	pub channel_id: Snowflake,
	#[serde(deserialize_with = "timestamp")]
	pub last_pin_timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThreadDelete {
	pub id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub parent_id: Option<Snowflake>,
	#[serde(rename = "type")]
	pub kind: ChannelType,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThreadListSync {
	pub guild_id: Snowflake,
	pub channel_ids: Option<Vec<Snowflake>>,
	pub threads: LenientVec<Channel>,
	pub members: LenientVec<ThreadMember>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThreadMembersUpdate {
	pub id: Snowflake,
	pub guild_id: Snowflake,
	pub member_count: Option<i64>,
	pub added_members: LenientVec<ThreadMember>,
	pub removed_member_ids: Vec<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildMemberRemove {
	pub guild_id: Snowflake,
	pub user: User,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildMembersChunk {
	pub guild_id: Snowflake,
	pub members: LenientVec<Member>,
	pub chunk_index: Option<i64>,
	pub chunk_count: Option<i64>,
	pub not_found: Vec<Snowflake>,
	pub presences: LenientVec<Presence>,
	pub nonce: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitedMeta {
	pub guild_id: Option<Snowflake>,
	pub nonce: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayRateLimited {
	pub opcode: Option<i64>,
	pub retry_after: Option<f64>,
	pub meta: RateLimitedMeta,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MemberListGroup {
	pub id: String,
	pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildMemberListUpdate {
	pub guild_id: Snowflake,
	pub id: String,
	pub ops: Vec<Value>,
	pub groups: LenientVec<MemberListGroup>,
	pub member_count: Option<i64>,
	pub online_count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildRoleChange {
	pub guild_id: Snowflake,
	pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildRoleDelete {
	pub guild_id: Snowflake,
	pub role_id: Snowflake,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildEmojisUpdate {
	pub guild_id: Snowflake,
	pub emojis: LenientVec<Emoji>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildStickersUpdate {
	pub guild_id: Snowflake,
	pub stickers: LenientVec<Sticker>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TypingStart {
	pub channel_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub user_id: Snowflake,
	pub timestamp: Option<i64>,
	pub member: Option<Member>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RelationshipRemove {
	pub id: Snowflake,
	#[serde(rename = "type")]
	pub kind: Option<RelationshipKind>,
	pub nickname: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RelationshipUpdate {
	pub id: Snowflake,
	#[serde(rename = "type")]
	pub kind: Option<RelationshipKind>,
	#[serde(skip_serializing_if = "Option::is_none", deserialize_with = "nullable_string")]
	pub nickname: Option<Option<String>>,
	pub user: Option<User>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceServerUpdate {
	pub token: Option<Token>,
	pub guild_id: Option<Snowflake>,
	pub channel_id: Option<Snowflake>,
	pub endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamState {
	pub stream_key: String,
	pub rtc_server_id: Option<Snowflake>,
	pub rtc_channel_id: Option<Snowflake>,
	pub region: Option<String>,
	pub viewer_ids: Vec<Snowflake>,
	pub paused: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamServerUpdate {
	pub stream_key: String,
	pub token: Option<Token>,
	pub guild_id: Option<Snowflake>,
	pub endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamDelete {
	pub stream_key: String,
	pub reason: Option<String>,
	pub unavailable: Option<bool>,
}

#[derive(Debug)]
pub enum DispatchEvent {
	Ready(Box<Ready>),
	ReadySupplemental(Box<ReadySupplemental>),
	Resumed,
	MessageCreate(Box<Message>),
	MessageUpdate(Box<MessageUpdate>),
	MessageDelete(MessageDelete),
	MessageDeleteBulk(MessageDeleteBulk),
	MessageReactionAdd(Box<MessageReactionAdd>),
	MessageReactionRemove(MessageReactionRemove),
	MessageReactionRemoveAll(MessageReactionRemoveAll),
	MessageReactionRemoveEmoji(MessageReactionRemoveEmoji),
	MessageAck(MessageAck),
	ChannelCreate(Box<Channel>),
	ChannelUpdate(Box<Channel>),
	ChannelDelete(Box<Channel>),
	ChannelPinsUpdate(ChannelPinsUpdate),
	ChannelUnreadUpdate(Box<ChannelUnreadUpdate>),
	ThreadCreate(Box<Channel>),
	ThreadUpdate(Box<Channel>),
	ThreadDelete(ThreadDelete),
	ThreadListSync(Box<ThreadListSync>),
	ThreadMemberUpdate(Box<ThreadMember>),
	ThreadMembersUpdate(Box<ThreadMembersUpdate>),
	GuildCreate(Box<Guild>),
	GuildUpdate(Box<Guild>),
	GuildDelete(UnavailableGuild),
	GuildMemberAdd(Box<Member>),
	GuildMemberUpdate(Box<Member>),
	GuildMemberRemove(Box<GuildMemberRemove>),
	GuildMembersChunk(Box<GuildMembersChunk>),
	GuildMemberListUpdate(Box<GuildMemberListUpdate>),
	RateLimited(Box<GatewayRateLimited>),
	GuildRoleCreate(Box<GuildRoleChange>),
	GuildRoleUpdate(Box<GuildRoleChange>),
	GuildRoleDelete(GuildRoleDelete),
	GuildEmojisUpdate(GuildEmojisUpdate),
	GuildStickersUpdate(GuildStickersUpdate),
	PresenceUpdate(Box<Presence>),
	TypingStart(Box<TypingStart>),
	UserUpdate(Box<User>),
	UserSettingsUpdate(Value),
	UserSettingsProtoUpdate(Value),
	UserGuildSettingsUpdate(Box<UserGuildSettings>),
	RelationshipAdd(Box<Relationship>),
	RelationshipRemove(RelationshipRemove),
	RelationshipUpdate(Box<RelationshipUpdate>),
	VoiceStateUpdate(Box<VoiceState>),
	VoiceServerUpdate(VoiceServerUpdate),
	StreamCreate(Box<StreamState>),
	StreamUpdate(Box<StreamState>),
	StreamServerUpdate(StreamServerUpdate),
	StreamDelete(StreamDelete),
	CallCreate(Value),
	CallUpdate(Value),
	CallDelete(Value),
	SessionsReplace(Vec<Value>),
	InteractionCreate(Value),
	InteractionSuccess(Value),
	InteractionFailure(Value),
	Unknown { name: String, data: Box<RawValue> },
}

fn decode<T: DeserializeOwned>(name: &str, data: &RawValue) -> Result<T, DecodeError> {
	serde_json::from_str(data.get()).map_err(|error| DecodeError::new(name, error))
}

impl DispatchEvent {
	pub fn parse(name: &str, data: &RawValue) -> Result<Self, DecodeError> {
		let event = match name {
			"READY" => {
				let mut ready: Box<Ready> = decode(name, data)?;
				ready.normalize_guilds();
				Self::Ready(ready)
			}
			"READY_SUPPLEMENTAL" => Self::ReadySupplemental(decode(name, data)?),
			"RESUMED" => Self::Resumed,
			"MESSAGE_CREATE" => Self::MessageCreate(decode(name, data)?),
			"MESSAGE_UPDATE" => Self::MessageUpdate(decode(name, data)?),
			"MESSAGE_DELETE" => Self::MessageDelete(decode(name, data)?),
			"MESSAGE_DELETE_BULK" => Self::MessageDeleteBulk(decode(name, data)?),
			"MESSAGE_REACTION_ADD" => Self::MessageReactionAdd(decode(name, data)?),
			"MESSAGE_REACTION_REMOVE" => Self::MessageReactionRemove(decode(name, data)?),
			"MESSAGE_REACTION_REMOVE_ALL" => Self::MessageReactionRemoveAll(decode(name, data)?),
			"MESSAGE_REACTION_REMOVE_EMOJI" => Self::MessageReactionRemoveEmoji(decode(name, data)?),
			"MESSAGE_ACK" => Self::MessageAck(decode(name, data)?),
			"CHANNEL_CREATE" => Self::ChannelCreate(decode(name, data)?),
			"CHANNEL_UPDATE" => Self::ChannelUpdate(decode(name, data)?),
			"CHANNEL_DELETE" => Self::ChannelDelete(decode(name, data)?),
			"CHANNEL_PINS_UPDATE" => Self::ChannelPinsUpdate(decode(name, data)?),
			"CHANNEL_UNREAD_UPDATE" => Self::ChannelUnreadUpdate(decode(name, data)?),
			"THREAD_CREATE" => Self::ThreadCreate(decode(name, data)?),
			"THREAD_UPDATE" => Self::ThreadUpdate(decode(name, data)?),
			"THREAD_DELETE" => Self::ThreadDelete(decode(name, data)?),
			"THREAD_LIST_SYNC" => Self::ThreadListSync(decode(name, data)?),
			"THREAD_MEMBER_UPDATE" => Self::ThreadMemberUpdate(decode(name, data)?),
			"THREAD_MEMBERS_UPDATE" => Self::ThreadMembersUpdate(decode(name, data)?),
			"GUILD_CREATE" => Self::GuildCreate(decode_guild(name, data)?),
			"GUILD_UPDATE" => Self::GuildUpdate(decode_guild(name, data)?),
			"GUILD_DELETE" => Self::GuildDelete(decode(name, data)?),
			"GUILD_MEMBER_ADD" => Self::GuildMemberAdd(decode(name, data)?),
			"GUILD_MEMBER_UPDATE" => Self::GuildMemberUpdate(decode(name, data)?),
			"GUILD_MEMBER_REMOVE" => Self::GuildMemberRemove(decode(name, data)?),
			"GUILD_MEMBERS_CHUNK" => Self::GuildMembersChunk(decode(name, data)?),
			"GUILD_MEMBER_LIST_UPDATE" => Self::GuildMemberListUpdate(decode(name, data)?),
			"RATE_LIMITED" => Self::RateLimited(decode(name, data)?),
			"GUILD_ROLE_CREATE" => Self::GuildRoleCreate(decode(name, data)?),
			"GUILD_ROLE_UPDATE" => Self::GuildRoleUpdate(decode(name, data)?),
			"GUILD_ROLE_DELETE" => Self::GuildRoleDelete(decode(name, data)?),
			"GUILD_EMOJIS_UPDATE" => Self::GuildEmojisUpdate(decode(name, data)?),
			"GUILD_STICKERS_UPDATE" => Self::GuildStickersUpdate(decode(name, data)?),
			"PRESENCE_UPDATE" => Self::PresenceUpdate(decode(name, data)?),
			"TYPING_START" => Self::TypingStart(decode(name, data)?),
			"USER_UPDATE" => Self::UserUpdate(decode(name, data)?),
			"USER_SETTINGS_UPDATE" => Self::UserSettingsUpdate(decode(name, data)?),
			"USER_SETTINGS_PROTO_UPDATE" => Self::UserSettingsProtoUpdate(decode(name, data)?),
			"USER_GUILD_SETTINGS_UPDATE" => Self::UserGuildSettingsUpdate(decode(name, data)?),
			"RELATIONSHIP_ADD" => Self::RelationshipAdd(decode(name, data)?),
			"RELATIONSHIP_REMOVE" => Self::RelationshipRemove(decode(name, data)?),
			"RELATIONSHIP_UPDATE" => Self::RelationshipUpdate(decode(name, data)?),
			"VOICE_STATE_UPDATE" => Self::VoiceStateUpdate(decode(name, data)?),
			"VOICE_SERVER_UPDATE" => Self::VoiceServerUpdate(decode(name, data)?),
			"STREAM_CREATE" => Self::StreamCreate(decode(name, data)?),
			"STREAM_UPDATE" => Self::StreamUpdate(decode(name, data)?),
			"STREAM_SERVER_UPDATE" => Self::StreamServerUpdate(decode(name, data)?),
			"STREAM_DELETE" => Self::StreamDelete(decode(name, data)?),
			"CALL_CREATE" => Self::CallCreate(decode(name, data)?),
			"CALL_UPDATE" => Self::CallUpdate(decode(name, data)?),
			"CALL_DELETE" => Self::CallDelete(decode(name, data)?),
			"SESSIONS_REPLACE" => Self::SessionsReplace(decode(name, data)?),
			"INTERACTION_CREATE" => Self::InteractionCreate(decode(name, data)?),
			"INTERACTION_SUCCESS" => Self::InteractionSuccess(decode(name, data)?),
			"INTERACTION_FAILURE" => Self::InteractionFailure(decode(name, data)?),
			_ => Self::Unknown {
				name: name.to_owned(),
				data: data.to_owned(),
			},
		};
		Ok(event)
	}
}

fn decode_guild(name: &str, data: &RawValue) -> Result<Box<Guild>, DecodeError> {
	let guild: Guild = decode(name, data)?;
	Ok(Box::new(guild.normalized()))
}
